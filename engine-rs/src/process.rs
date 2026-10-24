//! Subprocess helpers: PATH lookup like `shutil.which` and captured runs with
//! a wall-clock timeout like `subprocess.run(..., timeout=...)`, optionally
//! feeding the child's stdin.

use std::env;
use std::io::{self, Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

/// Result of a captured subprocess, decoded lossily as UTF-8.
#[derive(Debug, Clone)]
pub struct Completed {
    /// Exit code; a signal death reads as the negative signal number, as in Python.
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
}

/// Finds an executable on `PATH`; a name with a slash is checked directly.
pub fn which(name: &str) -> Option<PathBuf> {
    if name.contains('/') {
        let path = PathBuf::from(name);
        return is_executable(&path).then_some(path);
    }
    let path_var = env::var_os("PATH")?;
    env::split_paths(&path_var)
        .map(|dir| dir.join(name))
        .find(|candidate| is_executable(candidate))
}

fn is_executable(path: &Path) -> bool {
    path.is_file()
        && path
            .metadata()
            .map(|m| m.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
}

/// Runs a command with captured output and an optional timeout. On timeout the
/// child is killed and the partial output is returned with `timed_out` set.
///
/// # Errors
///
/// Fails when the process cannot be spawned (missing binary, bad cwd).
pub fn run(command: &mut Command, timeout: Option<Duration>) -> io::Result<Completed> {
    run_with_input(command, None, timeout)
}

/// [`run`] with bytes written to the child's stdin (closed afterwards). The
/// writer runs on its own thread so a child that does not read before it
/// writes cannot deadlock the parent.
///
/// # Errors
///
/// Fails when the process cannot be spawned.
pub fn run_with_input(
    command: &mut Command,
    input: Option<Vec<u8>>,
    timeout: Option<Duration>,
) -> io::Result<Completed> {
    command.stdin(if input.is_some() {
        Stdio::piped()
    } else {
        Stdio::null()
    });
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());
    let mut child = command.spawn()?;
    let writer = match (input, child.stdin.take()) {
        (Some(bytes), Some(mut stdin)) => Some(thread::spawn(move || {
            let _ = stdin.write_all(&bytes);
        })),
        _ => None,
    };
    let stdout = child.stdout.take().expect("stdout was piped");
    let stderr = child.stderr.take().expect("stderr was piped");
    let out_reader = thread::spawn(move || read_all(stdout));
    let err_reader = thread::spawn(move || read_all(stderr));
    let started = Instant::now();
    let mut timed_out = false;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if timeout.is_some_and(|limit| started.elapsed() >= limit) {
            timed_out = true;
            child.kill()?;
            break child.wait()?;
        }
        thread::sleep(Duration::from_millis(5));
    };
    if let Some(writer) = writer {
        let _ = writer.join();
    }
    let stdout = out_reader.join().unwrap_or_default();
    let stderr = err_reader.join().unwrap_or_default();
    let exit_code = status
        .code()
        .or_else(|| status.signal().map(|sig| -sig))
        .unwrap_or(-1);
    Ok(Completed {
        exit_code,
        stdout: String::from_utf8_lossy(&stdout).into_owned(),
        stderr: String::from_utf8_lossy(&stderr).into_owned(),
        timed_out,
    })
}

fn read_all(mut reader: impl Read) -> Vec<u8> {
    let mut buf = Vec::new();
    let _ = reader.read_to_end(&mut buf);
    buf
}

/// `/bin/sh -c cmd` in `cwd`, the shape `subprocess.run(cmd, shell=True)` uses.
pub fn shell(cmd: &str, cwd: &Path, timeout: Option<Duration>) -> io::Result<Completed> {
    let mut command = Command::new("/bin/sh");
    command.arg("-c").arg(cmd).current_dir(cwd);
    run(&mut command, timeout)
}

/// The first non-blank-trimmed line of a tool's output, or `None` when empty.
pub fn first_output_line(output: &str) -> Option<String> {
    output
        .trim()
        .lines()
        .next()
        .map(|line| line.trim().to_owned())
}

/// `TimeoutExpired`'s message shape for a timed out shell command.
pub fn timeout_message(cmd: &str, seconds: i64) -> String {
    format!("Command '{cmd}' timed out after {seconds} seconds")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captures_and_times_out() {
        let ok = shell("printf hi; printf err >&2; exit 3", Path::new("/"), None).expect("runs");
        assert_eq!(
            (ok.exit_code, ok.stdout.as_str(), ok.stderr.as_str()),
            (3, "hi", "err")
        );
        assert!(!ok.timed_out);
        let slow = shell(
            "printf partial; exec sleep 5",
            Path::new("/"),
            Some(Duration::from_millis(200)),
        )
        .expect("runs");
        assert!(slow.timed_out);
        assert_eq!(slow.stdout, "partial");
        assert!(which("sh").is_some());
        assert!(which("definitely-not-a-binary-xyz").is_none());
        assert_eq!(
            first_output_line("\n  v 1.0 \nmore"),
            Some("v 1.0".to_owned())
        );
        assert_eq!(first_output_line("  "), None);
    }

    #[test]
    fn feeds_stdin() {
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "cat; printf ' and more'"]);
        let done = run_with_input(&mut command, Some(b"from stdin".to_vec()), None).expect("runs");
        assert_eq!(done.stdout, "from stdin and more");
        assert_eq!(done.exit_code, 0);
    }
}
