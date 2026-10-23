//! The run manifest: the environment a run executed in (`_collect_manifest`
//! and `_task_workspace_stats` in `harness/agent/loop.py`, plus the shared
//! task-metadata helpers of `harness/task_metadata.py`).

use std::path::Path;
use std::process::Command;
use std::time::Duration;

use crate::deadline::{clamp_timeout, Deadline};
use crate::json::{obj, Value};
use crate::process;
use crate::pyfmt::py_splitlines;
use crate::tasks::{walk_files, Task};

const MANIFEST_TOOLS: [&str; 6] = ["git", "ruff", "bandit", "radon", "go", "node"];

pub const REPO_SCALES: [&str; 5] = ["micro", "small", "medium", "large", "xlarge"];
pub const TASK_COMPLEXITIES: [&str; 4] = ["localized", "multi_file", "system", "architecture"];

/// `(suggested_max_steps, suggested_timeout_s)` per repo scale.
const SCALE_DEFAULTS: [(&str, i64, f64); 5] = [
    ("micro", 50, 300.0),
    ("small", 60, 600.0),
    ("medium", 100, 1200.0),
    ("large", 150, 2700.0),
    ("xlarge", 200, 3600.0),
];
pub const DEFAULT_MAX_STEPS: i64 = 50;

const CODE_SUFFIXES: [&str; 19] = [
    "py", "go", "ts", "tsx", "js", "jsx", "mjs", "cjs", "java", "rs", "rb", "cs", "c", "h", "cc",
    "cpp", "cxx", "hh", "hpp",
];
const WORKSPACE_SKIP: [&str; 5] = ["issue.md", ".vulcanbenchignore", ".coverage", ".gitignore", ".gitattributes"];

pub fn repo_scale(metadata: &Value) -> &'static str {
    metadata
        .get("repo_scale")
        .and_then(Value::as_str)
        .and_then(|raw| REPO_SCALES.iter().copied().find(|s| *s == raw))
        .unwrap_or("micro")
}

pub fn task_complexity(metadata: &Value) -> &'static str {
    metadata
        .get("task_complexity")
        .and_then(Value::as_str)
        .and_then(|raw| TASK_COMPLEXITIES.iter().copied().find(|s| *s == raw))
        .unwrap_or("localized")
}

fn agent_hints(metadata: &Value) -> Option<&Value> {
    metadata
        .get("agent_hints")
        .filter(|h| matches!(h, Value::Object(_)))
}

fn scale_defaults(metadata: &Value) -> (i64, f64) {
    let scale = repo_scale(metadata);
    SCALE_DEFAULTS
        .iter()
        .find(|(s, _, _)| *s == scale)
        .map_or((DEFAULT_MAX_STEPS, 0.0), |(_, steps, timeout)| (*steps, *timeout))
}

/// Step budget from metadata; a CLI value caps it unless `override` is set.
pub fn resolve_max_steps(metadata: &Value, cli_max_steps: Option<i64>, override_budgets: bool) -> i64 {
    if let (true, Some(cli)) = (override_budgets, cli_max_steps) {
        return cli;
    }
    let raw = agent_hints(metadata)
        .and_then(|h| h.get("suggested_max_steps"))
        .or_else(|| metadata.get("max_steps"))
        .and_then(Value::as_i64);
    let metadata_steps = match raw {
        Some(n) if n > 0 => n,
        _ => scale_defaults(metadata).0,
    };
    cli_max_steps.map_or(metadata_steps, |cli| cli.min(metadata_steps))
}

/// Wall-clock budget from hints or scale defaults; a CLI value caps it.
pub fn resolve_agent_timeout_s(metadata: &Value, cli_timeout: Option<f64>, override_budgets: bool) -> Option<f64> {
    if let (true, Some(cli)) = (override_budgets, cli_timeout) {
        return Some(cli);
    }
    let hint = agent_hints(metadata)
        .and_then(|h| h.get("suggested_timeout_s"))
        .and_then(Value::as_f64)
        .filter(|t| *t > 0.0);
    let timeout = hint.or_else(|| {
        let (_, default) = scale_defaults(metadata);
        (default > 0.0).then_some(default)
    });
    match (cli_timeout, timeout) {
        (Some(cli), Some(t)) => Some(cli.min(t)),
        (Some(cli), None) => Some(cli),
        (None, t) => t,
    }
}

/// Per-task verifier subprocess budget (`test_timeout_s`), not agent wall-clock.
pub fn resolve_verifier_timeout_s(metadata: &Value, default: i64) -> i64 {
    match metadata.get("test_timeout_s").and_then(Value::as_f64) {
        Some(t) if t > 0.0 => t as i64,
        _ => default,
    }
}

/// The base prompt, extended with navigation advice on medium and large repos.
pub fn system_prompt_for_task(metadata: &Value, base_prompt: &str) -> String {
    let scale = repo_scale(metadata);
    if scale == "medium" || scale == "large" {
        return format!(
            "{base_prompt}\n\nThis is a {scale} codebase: use search_code and partial \
             read_file (line ranges) to explore before editing. Prefer small, targeted \
             changes across the modules that own the behavior."
        );
    }
    base_prompt.to_owned()
}

fn host_command_first_line(args: &[&str], timeout: i64) -> Option<String> {
    process::which(args[0])?;
    let mut command = Command::new(args[0]);
    command.args(&args[1..]);
    let done = process::run(&mut command, Some(Duration::from_secs(timeout.max(1) as u64))).ok()?;
    if done.timed_out {
        return None;
    }
    let output = if done.stdout.is_empty() { &done.stderr } else { &done.stdout };
    process::first_output_line(output)
}

fn tool_version(cmd: &str, timeout: i64) -> Option<String> {
    if cmd == "go" {
        host_command_first_line(&["go", "version"], timeout)
    } else {
        host_command_first_line(&[cmd, "--version"], timeout)
    }
}

/// `platform.python_version()` of the interpreter the run would use: the
/// `python` (else `python3`) on PATH.
pub fn runtime_python() -> String {
    for name in ["python", "python3"] {
        if let Some(line) = host_command_first_line(&[name, "--version"], 10) {
            if let Some(version) = line.strip_prefix("Python ") {
                return version.trim().to_owned();
            }
        }
    }
    "unknown".to_owned()
}

/// `sys.platform` for this host.
pub fn host_platform() -> &'static str {
    match std::env::consts::OS {
        "macos" => "darwin",
        "windows" => "win32",
        other => other,
    }
}

/// `(file_count, loc)` under a repo directory or extracted workspace.
pub fn measure_repo_path(root: &Path) -> (i64, i64) {
    let (mut file_count, mut loc) = (0i64, 0i64);
    for rel in walk_files(root).unwrap_or_default() {
        let in_git = rel.components().any(|c| c.as_os_str() == ".git");
        let name = rel
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if in_git || WORKSPACE_SKIP.contains(&name.as_str()) {
            continue;
        }
        file_count += 1;
        let suffix = rel
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        if CODE_SUFFIXES.contains(&suffix.as_str()) {
            if let Ok(bytes) = std::fs::read(root.join(&rel)) {
                let text = String::from_utf8_lossy(&bytes);
                loc += i64::try_from(py_splitlines(&text, false).len()).unwrap_or(0);
            }
        }
    }
    (file_count, loc)
}

/// `{"task": {...}}`: scale, complexity, languages, difficulty and workspace size.
pub fn task_workspace_stats(task: &Task, workspace: &Path) -> Value {
    let (file_count, loc) = measure_repo_path(workspace);
    obj! {
        "task" => obj! {
            "repo_scale" => repo_scale(&task.metadata),
            "task_complexity" => task_complexity(&task.metadata),
            "languages" => task.metadata.get("languages").cloned().unwrap_or_else(|| Value::Array(Vec::new())),
            "difficulty" => task.metadata.get("difficulty").cloned().unwrap_or(Value::Null),
            "file_count" => file_count,
            "loc" => loc,
        },
    }
}

fn sandbox_block(network: bool) -> Value {
    obj! {
        "mode" => "local",
        "image" => Value::Null,
        "network" => network,
        "resources" => Value::Null,
        "oom_kills" => Value::Null,
    }
}

/// The manifest when the budget ran out before any probe could run.
pub fn minimal_manifest(model: &str, judge_model: Option<&str>, network: bool) -> Value {
    let python = runtime_python();
    let platform = host_platform();
    let mut tools = Value::object();
    for tool in MANIFEST_TOOLS {
        tools.insert(tool, Value::Null);
    }
    obj! {
        "model" => model,
        "judge_model" => judge_model.unwrap_or(model),
        "runtime" => obj! {
            "python" => python.as_str(),
            "platform" => platform,
            "host_python" => python.as_str(),
            "host_platform" => platform,
        },
        "sandbox" => sandbox_block(network),
        "tools" => tools,
    }
}

/// Records the environment a run executed in, for audit and reproducibility.
pub fn collect_manifest(model: &str, judge_model: Option<&str>, network: bool, deadline: &Deadline) -> Value {
    if clamp_timeout(10, deadline.remaining_s()).is_none() {
        let mut manifest = minimal_manifest(model, judge_model, network);
        manifest.insert("budget_exceeded", true);
        return manifest;
    }
    let mut tools = Value::object();
    for tool in MANIFEST_TOOLS {
        let version = clamp_timeout(10, deadline.remaining_s()).and_then(|timeout| tool_version(tool, timeout));
        tools.insert(tool, version);
    }
    let python = runtime_python();
    let platform = host_platform();
    obj! {
        "model" => model,
        "judge_model" => judge_model.unwrap_or(model),
        "runtime" => obj! {
            "python" => python.as_str(),
            "platform" => platform,
            "host_python" => python.as_str(),
            "host_platform" => platform,
        },
        "sandbox" => sandbox_block(network),
        "tools" => tools,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json::parse;

    #[test]
    fn budgets_follow_hints_then_scale() {
        let micro = parse(r#"{"difficulty": "trivial"}"#).expect("json");
        assert_eq!(resolve_max_steps(&micro, None, false), 50);
        assert_eq!(resolve_max_steps(&micro, Some(20), false), 20);
        assert_eq!(resolve_max_steps(&micro, Some(80), false), 50);
        assert_eq!(resolve_max_steps(&micro, Some(80), true), 80);
        assert_eq!(resolve_agent_timeout_s(&micro, None, false), Some(300.0));
        let hinted = parse(r#"{"repo_scale": "large", "agent_hints": {"suggested_max_steps": 120, "suggested_timeout_s": 5400}}"#).expect("json");
        assert_eq!(resolve_max_steps(&hinted, None, false), 120);
        assert_eq!(resolve_agent_timeout_s(&hinted, Some(100.0), false), Some(100.0));
        assert_eq!(resolve_verifier_timeout_s(&hinted, 120), 120);
        assert!(system_prompt_for_task(&hinted, "base").starts_with("base\n\nThis is a large codebase"));
        assert_eq!(system_prompt_for_task(&micro, "base"), "base");
    }
}
