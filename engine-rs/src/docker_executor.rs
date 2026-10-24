//! Docker-backed tool executor (`harness/sandbox/docker_executor.py`): the
//! agent's shell commands run inside an isolated, non-root, network-off,
//! resource-limited container, while file operations go through the
//! host-side [`LocalToolExecutor`] over the bind-mounted workspace.
//!
//! Everything goes through the `docker` CLI, the same binary the Python
//! harness drives, so no Docker SDK is needed on either side.

use std::cell::Cell;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use crate::error::EngineError;
use crate::json::{obj, Value};
use crate::local_executor::{py_os_error, LocalToolExecutor};
use crate::process::{self, Completed};
use crate::protocol::{RunCommandArgs, ToolCall, ToolObservation};
use crate::pyfmt::py_repr_str;
use crate::tasks::Task;

const CONTAINER_WORKDIR: &str = "/workspace";

/// Exit status of a SIGKILLed process: the signature of an OOM kill, and of
/// any other KILL, so it only counts together with the cgroup's kill counter.
const SIGKILL_EXIT: i32 = 137;

/// The docker CLI's own exit status when the daemon, not the command, fails.
const DOCKER_CLI_ERROR_EXIT: i32 = 125;

const PROBE_TIMEOUT: Duration = Duration::from_secs(30);
const LIFECYCLE_TIMEOUT: Duration = Duration::from_secs(60);

/// Writable paths for non-root containers running as the host UID/GID.
const SANDBOX_ENV: [(&str, &str); 4] = [
    ("HOME", "/tmp"),
    ("GOCACHE", "/tmp/go-build"),
    ("GOPATH", "/tmp/go"),
    ("CARGO_HOME", "/tmp/cargo"),
];

pub fn default_image() -> String {
    std::env::var("VULCANBENCH_SANDBOX_IMAGE")
        .unwrap_or_else(|_| "vulcanbench/sandbox:base".to_owned())
}

pub fn default_rust_image() -> String {
    std::env::var("VULCANBENCH_SANDBOX_RUST_IMAGE")
        .unwrap_or_else(|_| "vulcanbench/sandbox:rust".to_owned())
}

pub fn docker_bin() -> String {
    std::env::var("VULCANBENCH_DOCKER_BIN").unwrap_or_else(|_| "docker".to_owned())
}

/// `resolve_sandbox_image`: explicit `--image`, else `metadata.image`, else
/// the Rust tier for Rust tasks, else the base image.
pub fn resolve_sandbox_image(task: &Task, cli_image: Option<&str>) -> String {
    if let Some(image) = cli_image.filter(|i| !i.is_empty()) {
        return image.to_owned();
    }
    if let Some(image) = task
        .metadata
        .get("image")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|i| !i.is_empty())
    {
        return image.to_owned();
    }
    let is_rust = task
        .metadata
        .get("languages")
        .and_then(Value::as_array)
        .is_some_and(|langs| langs.iter().any(|l| l.as_str() == Some("rust")));
    if is_rust {
        default_rust_image()
    } else {
        default_image()
    }
}

/// Floor/ceiling resource band for the sandbox container.
#[derive(Debug, Clone, PartialEq)]
pub struct ResourceSpec {
    pub mem_floor: Option<String>,
    pub mem_ceiling: String,
    pub cpu_floor: Option<f64>,
    pub cpu_ceiling: f64,
    pub pids_limit: i64,
}

impl Default for ResourceSpec {
    fn default() -> Self {
        Self {
            mem_floor: None,
            mem_ceiling: "2g".to_owned(),
            cpu_floor: None,
            cpu_ceiling: 2.0,
            pids_limit: 512,
        }
    }
}

impl ResourceSpec {
    pub fn to_dict(&self) -> Value {
        obj! {
            "mem_floor" => self.mem_floor.as_deref(),
            "mem_ceiling" => self.mem_ceiling.as_str(),
            "cpu_floor" => self.cpu_floor,
            "cpu_ceiling" => self.cpu_ceiling,
            "pids_limit" => self.pids_limit,
        }
    }
}

/// `shlex.quote`: unchanged when safe, otherwise single-quoted.
pub fn shlex_quote(text: &str) -> String {
    if text.is_empty() {
        return "''".to_owned();
    }
    let safe = text.chars().all(|c| {
        c.is_ascii_alphanumeric()
            || matches!(c, '_' | '@' | '%' | '+' | '=' | ':' | ',' | '.' | '/' | '-')
    });
    if safe {
        return text.to_owned();
    }
    format!("'{}'", text.replace('\'', "'\"'\"'"))
}

/// `docker <args>`; a missing binary reads like Python's `OSError` text.
fn run_docker(args: &[&str], timeout: Option<Duration>) -> Result<Completed, String> {
    let binary = docker_bin();
    let mut command = Command::new(&binary);
    command.args(args);
    process::run(&mut command, timeout).map_err(|e| py_os_error(&e, Path::new(&binary)))
}

fn probe_daemon() -> Result<Completed, String> {
    run_docker(
        &["version", "--format", "{{.Server.Version}}"],
        Some(PROBE_TIMEOUT),
    )
}

/// Whether a Docker daemon is reachable (used by `--sandbox auto`).
pub fn docker_available() -> bool {
    probe_daemon().is_ok_and(|done| done.exit_code == 0 && !done.timed_out)
}

fn is_daemon_error(done: &Completed) -> bool {
    done.exit_code == DOCKER_CLI_ERROR_EXIT
        && (done.stderr.contains("Error response from daemon")
            || done.stderr.contains("Cannot connect to the Docker daemon"))
}

/// A long-lived sandbox container with the workspace bind-mounted at `/workspace`.
#[derive(Debug)]
pub struct DockerExecutor {
    files: LocalToolExecutor,
    pub image: String,
    pub resources: ResourceSpec,
    container_id: Option<String>,
    closed: Cell<bool>,
    oom_kill_count: Cell<i64>,
    oom_baseline: Cell<Option<i64>>,
    default_timeout: i64,
}

impl DockerExecutor {
    /// The `docker run` argument vector for the sandbox container. The host
    /// UID and GID come from the workspace directory's owner, which the engine
    /// just created, since std exposes no `getuid`.
    pub fn run_argv(
        workspace: &Path,
        image: &str,
        network: bool,
        spec: &ResourceSpec,
    ) -> Vec<String> {
        let mut argv: Vec<String> = [
            "run",
            "--detach",
            "--workdir",
            CONTAINER_WORKDIR,
            "--volume",
            &format!("{}:{CONTAINER_WORKDIR}:rw", workspace.display()),
            "--memory",
            &spec.mem_ceiling,
            "--cpus",
            &crate::json::format_float(spec.cpu_ceiling),
            "--pids-limit",
            &spec.pids_limit.to_string(),
            "--security-opt",
            "no-new-privileges",
            "--cap-drop",
            "ALL",
        ]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
        for (key, value) in SANDBOX_ENV {
            argv.push("--env".to_owned());
            argv.push(format!("{key}={value}"));
        }
        if !network {
            argv.push("--network".to_owned());
            argv.push("none".to_owned());
        }
        if let Ok(meta) = std::fs::metadata(workspace) {
            argv.push("--user".to_owned());
            argv.push(format!("{}:{}", meta.uid(), meta.gid()));
        }
        if let Some(floor) = &spec.mem_floor {
            argv.push("--memory-reservation".to_owned());
            argv.push(floor.clone());
        }
        if let Some(cpu_floor) = spec.cpu_floor {
            argv.push("--cpu-shares".to_owned());
            argv.push(((cpu_floor * 1024.0) as i64).max(2).to_string());
        }
        argv.push(image.to_owned());
        argv.push("sleep".to_owned());
        argv.push("infinity".to_owned());
        argv
    }

    /// Probes the daemon and starts the container.
    ///
    /// # Errors
    ///
    /// `EngineError::Sandbox` with the Python messages when the daemon is
    /// unreachable or the container does not start.
    pub fn new(
        workspace: &Path,
        image: &str,
        network: bool,
        resources: ResourceSpec,
    ) -> Result<Self, EngineError> {
        let files = LocalToolExecutor::new(workspace)?;
        let workspace = files.workspace().to_path_buf();
        let probe = probe_daemon().map_err(|e| {
            EngineError::Sandbox(format!(
                "could not connect to the Docker daemon ({e}). Is Docker running?"
            ))
        })?;
        if probe.exit_code != 0 || probe.timed_out {
            let detail = match probe.stderr.trim() {
                "" => format!("docker exited {}", probe.exit_code),
                text => text.to_owned(),
            };
            return Err(EngineError::Sandbox(format!(
                "could not connect to the Docker daemon ({detail}). Is Docker running?"
            )));
        }
        let argv = Self::run_argv(&workspace, image, network, &resources);
        let argv_refs: Vec<&str> = argv.iter().map(String::as_str).collect();
        let started = run_docker(&argv_refs, None).map_err(|e| {
            EngineError::Sandbox(format!(
                "failed to start sandbox container from {}: {e}",
                py_repr_str(image)
            ))
        })?;
        if started.exit_code != 0 {
            let detail = match started.stderr.trim() {
                "" => format!("docker exited {}", started.exit_code),
                text => text.to_owned(),
            };
            return Err(EngineError::Sandbox(format!(
                "failed to start sandbox container from {}: {detail}",
                py_repr_str(image)
            )));
        }
        let container_id = started
            .stdout
            .trim()
            .lines()
            .last()
            .unwrap_or_default()
            .to_owned();
        Ok(Self {
            files,
            image: image.to_owned(),
            resources,
            container_id: Some(container_id),
            closed: Cell::new(false),
            oom_kill_count: Cell::new(0),
            oom_baseline: Cell::new(None),
            default_timeout: 120,
        })
    }

    pub fn files(&self) -> &LocalToolExecutor {
        &self.files
    }

    pub fn oom_kill_count(&self) -> i64 {
        self.oom_kill_count.get()
    }

    fn docker_exec(&self, cmd: &[&str]) -> Result<Completed, String> {
        let Some(container_id) = &self.container_id else {
            return Err("sandbox container is not running".to_owned());
        };
        let mut args = vec![
            "exec",
            "--workdir",
            CONTAINER_WORKDIR,
            container_id.as_str(),
        ];
        args.extend_from_slice(cmd);
        let done = run_docker(&args, None).map_err(|e| format!("container exec failed: {e}"))?;
        if is_daemon_error(&done) {
            return Err(format!("container exec failed: {}", done.stderr.trim()));
        }
        Ok(done)
    }

    fn exec_shell(&self, shell_cmd: &str) -> Result<Value, String> {
        let done = self.docker_exec(&["sh", "-c", shell_cmd])?;
        let mut payload = obj! {
            "stdout" => done.stdout,
            "stderr" => done.stderr,
            "exit_code" => i64::from(done.exit_code),
        };
        if done.exit_code == SIGKILL_EXIT {
            let new_kills = self.new_oom_kills();
            self.oom_kill_count
                .set(self.oom_kill_count.get() + new_kills);
            payload.insert("oom_killed", new_kills > 0);
        }
        Ok(payload)
    }

    fn read_oom_kills(&self) -> i64 {
        let Ok(done) = self.docker_exec(&[
            "sh",
            "-c",
            "cat /sys/fs/cgroup/memory.events /sys/fs/cgroup/memory/memory.oom_control 2>/dev/null || true",
        ]) else {
            return 0;
        };
        done.stdout
            .lines()
            .filter_map(|line| {
                let mut parts = line.split_whitespace();
                match (parts.next(), parts.next(), parts.next()) {
                    (Some("oom_kill"), Some(count), None) => count.parse::<i64>().ok(),
                    _ => None,
                }
            })
            .max()
            .unwrap_or(0)
    }

    fn new_oom_kills(&self) -> i64 {
        let current = self.read_oom_kills();
        let baseline = self.oom_baseline.get().unwrap_or(0);
        self.oom_baseline.set(Some(current));
        (current - baseline).max(0)
    }

    /// `subprocess`-style shell command inside the container, time-boxed by
    /// the container's own `timeout`.
    ///
    /// # Errors
    ///
    /// The Python exception text for a path outside the workspace or a daemon failure.
    pub fn run_command(&self, args: &RunCommandArgs) -> Result<Value, String> {
        let mut workdir = CONTAINER_WORKDIR.to_owned();
        if let Some(cwd) = args.cwd.as_deref().filter(|c| !c.is_empty()) {
            let resolved = self.files.resolve(cwd)?;
            let relative = resolved
                .strip_prefix(self.files.workspace())
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default();
            workdir = format!("{CONTAINER_WORKDIR}/{relative}");
        }
        let timeout = args
            .timeout
            .filter(|t| *t != 0)
            .unwrap_or(self.default_timeout);
        let inner = format!(
            "cd {} && timeout {timeout}s sh -c {}",
            shlex_quote(&workdir),
            shlex_quote(&args.cmd)
        );
        self.exec_shell(&inner)
    }

    /// File tools on the host, execution tools in the container.
    pub fn execute(&self, call: &ToolCall) -> ToolObservation {
        match call.tool.as_str() {
            "run_command" => {
                match RunCommandArgs::from_args(&call.args).and_then(|a| self.run_command(&a)) {
                    Ok(value) => ToolObservation::ok(value),
                    Err(message) => ToolObservation::err(message),
                }
            }
            _ => self.files.execute(call),
        }
    }

    /// Stops and removes the container. Safe to call more than once.
    pub fn close(&self) {
        if self.closed.replace(true) {
            return;
        }
        let Some(container_id) = &self.container_id else {
            return;
        };
        let _ = run_docker(
            &["stop", "--time", "5", container_id],
            Some(LIFECYCLE_TIMEOUT),
        );
        let _ = run_docker(&["rm", "--force", container_id], Some(LIFECYCLE_TIMEOUT));
    }
}

impl Drop for DockerExecutor {
    fn drop(&mut self) {
        self.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_argv_matches_the_python_vector() {
        let spec = ResourceSpec {
            mem_floor: Some("1g".to_owned()),
            cpu_floor: Some(1.5),
            ..ResourceSpec::default()
        };
        let argv =
            DockerExecutor::run_argv(Path::new("/tmp"), "vulcanbench/sandbox:base", false, &spec);
        let joined = argv.join(" ");
        assert!(joined.starts_with(
            "run --detach --workdir /workspace --volume /tmp:/workspace:rw --memory 2g --cpus 2.0 --pids-limit 512 --security-opt no-new-privileges --cap-drop ALL --env HOME=/tmp --env GOCACHE=/tmp/go-build --env GOPATH=/tmp/go --env CARGO_HOME=/tmp/cargo --network none --user "
        ), "{joined}");
        assert!(
            joined.ends_with(
                "--memory-reservation 1g --cpu-shares 1536 vulcanbench/sandbox:base sleep infinity"
            ),
            "{joined}"
        );
        let networked =
            DockerExecutor::run_argv(Path::new("/tmp"), "img", true, &ResourceSpec::default());
        assert!(!networked.contains(&"--network".to_owned()));
        assert_eq!(
            ResourceSpec::default().to_dict().to_json(),
            r#"{"mem_floor": null, "mem_ceiling": "2g", "cpu_floor": null, "cpu_ceiling": 2.0, "pids_limit": 512}"#
        );
    }

    #[test]
    fn shlex_quote_matches_python() {
        assert_eq!(shlex_quote("python -m pytest -q"), "'python -m pytest -q'");
        assert_eq!(shlex_quote("/workspace/sub-dir_1"), "/workspace/sub-dir_1");
        assert_eq!(shlex_quote("it's"), "'it'\"'\"'s'");
        assert_eq!(shlex_quote(""), "''");
    }

    #[test]
    fn image_resolution_order() {
        let mut task = Task {
            task_id: "t".to_owned(),
            root: std::path::PathBuf::from("."),
            metadata: obj! { "languages" => vec![Value::from("rust")] },
            issue: String::new(),
            verifier: None,
        };
        assert_eq!(resolve_sandbox_image(&task, Some("custom:1")), "custom:1");
        assert_eq!(resolve_sandbox_image(&task, None), default_rust_image());
        task.metadata.insert("image", " tier:x ");
        assert_eq!(resolve_sandbox_image(&task, None), "tier:x");
        task.metadata = obj! { "languages" => vec![Value::from("python")] };
        assert_eq!(resolve_sandbox_image(&task, None), default_image());
    }
}
