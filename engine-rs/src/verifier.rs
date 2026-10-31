//! Verification: the legacy per-task `verifier.py` runner
//! (`harness/evaluator/scorer.py::run_verifier`) and the declarative
//! `fail_to_pass` / `pass_to_pass` verifier (`harness/verifier.py`).

use std::path::Path;
use std::process::Command;
use std::time::Duration;

use crate::error::EngineError;
use crate::json::{self, obj, Value};
use crate::process;
use crate::pyfmt::{py_round, py_str};
use crate::tasks::{install_hidden_tests, Task};

pub const DEFAULT_TIMEOUT: i64 = 120;

/// One verifier command's captured result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunnerOutcome {
    pub exit_code: i64,
    pub stdout: String,
    pub stderr: String,
}

/// Runs a task's `verifier.py` in the workspace with the venv's Python and
/// parses its JSON stdout into a `{exit_code, stdout, stderr, scores}` payload.
pub fn run_verifier(verifier: &Path, workspace: &Path, timeout: i64) -> Value {
    let timeout = timeout.max(1);
    let python = process::which("python")
        .or_else(|| process::which("python3"))
        .unwrap_or_else(|| "python3".into());
    let script = std::fs::canonicalize(verifier).unwrap_or_else(|_| verifier.to_path_buf());
    let mut command = Command::new(python);
    command.arg(&script).current_dir(workspace);
    let done = match process::run(&mut command, Some(Duration::from_secs(timeout as u64))) {
        Ok(done) => done,
        Err(err) => {
            return obj! {
                "exit_code" => 1,
                "stdout" => "",
                "stderr" => err.to_string(),
                "scores" => obj! { "functional" => 0.0, "error" => "verifier did not emit valid JSON" },
            }
        }
    };
    if done.timed_out {
        return obj! {
            "exit_code" => 124,
            "stdout" => done.stdout,
            "stderr" => done.stderr,
            "scores" => obj! {
                "functional" => 0.0,
                "error" => format!("verifier timed out after {timeout}s"),
            },
        };
    }
    let scores = json::parse(&done.stdout).unwrap_or_else(|_| {
        obj! { "functional" => 0.0, "error" => "verifier did not emit valid JSON" }
    });
    obj! {
        "exit_code" => i64::from(done.exit_code),
        "stdout" => done.stdout,
        "stderr" => done.stderr,
        "scores" => scores,
    }
}

const MISSING_COMMANDS: [&str; 7] = ["python", "python3", "pytest", "go", "cargo", "npm", "node"];

fn infrastructure_reason(cmd: &str, outcome: &RunnerOutcome) -> Option<&'static str> {
    let output = format!("{}\n{}", outcome.stdout, outcome.stderr).to_lowercase();
    if outcome.exit_code == 124 {
        return Some("verifier command timed out");
    }
    if output.contains("no module named pytest") || output.contains("no module named 'pytest'") {
        return Some("pytest is unavailable in the verifier environment");
    }
    let missing_exit = matches!(outcome.exit_code, 126 | 127);
    let not_found = MISSING_COMMANDS.iter().any(|tool| {
        output.contains(&format!("{tool}: not found"))
            || output.contains(&format!("{tool}: command not found"))
    });
    if missing_exit && not_found {
        return Some("verifier toolchain command is unavailable");
    }
    if missing_exit
        && MISSING_COMMANDS
            .iter()
            .any(|tool| cmd.trim_start().starts_with(tool))
    {
        return Some("verifier toolchain command is unavailable");
    }
    None
}

fn run_group(
    entries: &[Value],
    timeout: i64,
    runner: &dyn Fn(&str, i64) -> Result<RunnerOutcome, EngineError>,
) -> Result<Value, EngineError> {
    let mut results = Value::object();
    for (i, entry) in entries.iter().enumerate() {
        let name = match entry.get("name") {
            Some(n) if !py_str(n).is_empty() && !n.is_null() => py_str(n),
            _ => format!("test_{i}"),
        };
        let cmd = entry.get("cmd").map(py_str).unwrap_or_default();
        if cmd.is_empty() {
            results.insert(name, false);
            continue;
        }
        let outcome = runner(&cmd, timeout)?;
        if let Some(reason) = infrastructure_reason(&cmd, &outcome) {
            return Err(EngineError::VerifierInfrastructure(format!(
                "{reason}: {name} ({cmd})"
            )));
        }
        results.insert(name, outcome.exit_code == 0);
    }
    Ok(results)
}

fn all_true(results: &Value) -> bool {
    results
        .as_object()
        .unwrap_or(&[])
        .iter()
        .all(|(_, v)| v.as_bool() == Some(true))
}

/// Runs a task's declarative tests through `runner` and returns the scores payload.
///
/// # Errors
///
/// `VerifierInfrastructure` when a command's toolchain is missing, or I/O
/// failures while installing the hidden tests.
pub fn run_declarative_verifier(
    task: &Task,
    workspace: &Path,
    runner: &dyn Fn(&str, i64) -> Result<RunnerOutcome, EngineError>,
    timeout: i64,
) -> Result<Value, EngineError> {
    install_hidden_tests(task, workspace)?;
    let spec = task.tests_spec().cloned().unwrap_or_else(Value::object);
    let group = |key: &str| -> Vec<Value> {
        spec.get(key)
            .and_then(Value::as_array)
            .map(<[Value]>::to_vec)
            .unwrap_or_default()
    };
    let fail_to_pass = group("fail_to_pass");
    let pass_to_pass = group("pass_to_pass");
    if fail_to_pass.is_empty() {
        return Ok(obj! {
            "scores" => obj! { "functional" => 0.0, "error" => "no fail_to_pass tests declared" },
        });
    }
    let p2p_results = run_group(&pass_to_pass, timeout, runner)?;
    let f2p_results = run_group(&fail_to_pass, timeout, runner)?;
    let p2p_ok = all_true(&p2p_results);
    let f2p_entries = f2p_results.as_object().unwrap_or(&[]);
    let f2p_passing = f2p_entries
        .iter()
        .filter(|(_, v)| v.as_bool() == Some(true))
        .count();
    let functional = if p2p_ok {
        py_round(f2p_passing as f64 / f2p_entries.len() as f64, 4)
    } else {
        0.0
    };
    let mut details = Vec::new();
    if !p2p_ok {
        let broke: Vec<String> = p2p_results
            .as_object()
            .unwrap_or(&[])
            .iter()
            .filter(|(_, v)| v.as_bool() != Some(true))
            .map(|(n, _)| crate::pyfmt::py_repr_str(n))
            .collect();
        details.push(Value::Str(format!(
            "regression: pass_to_pass failing: [{}]",
            broke.join(", ")
        )));
    }
    details.push(Value::Str(format!(
        "fail_to_pass {f2p_passing}/{} passing",
        f2p_entries.len()
    )));
    Ok(obj! {
        "scores" => obj! { "functional" => functional },
        "fail_to_pass" => f2p_results,
        "pass_to_pass" => p2p_results,
        "pass_to_pass_ok" => p2p_ok,
        "details" => details,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn infrastructure_reasons() {
        let timed_out = RunnerOutcome {
            exit_code: 124,
            stdout: String::new(),
            stderr: String::new(),
        };
        assert_eq!(
            infrastructure_reason("pytest", &timed_out),
            Some("verifier command timed out")
        );
        let missing = RunnerOutcome {
            exit_code: 127,
            stdout: String::new(),
            stderr: "sh: cargo: command not found".to_owned(),
        };
        assert_eq!(
            infrastructure_reason("cargo test", &missing),
            Some("verifier toolchain command is unavailable")
        );
        let failing = RunnerOutcome {
            exit_code: 1,
            stdout: "FAILED".to_owned(),
            stderr: String::new(),
        };
        assert_eq!(infrastructure_reason("pytest", &failing), None);
    }
}
