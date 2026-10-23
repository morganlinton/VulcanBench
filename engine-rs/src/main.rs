//! `vulcanbench-engine`: a zero-crate Rust spike of the VulcanBench run engine.
//!
//! Only `run --task <id> --model mock:<name> --sandbox local --no-judges` is
//! implemented. The Python harness (`harness/`) remains the engine of record;
//! this binary exists to measure the cost and the parity risk of a port.

mod deadline;
mod dotenv;
mod effort;
mod engine;
mod error;
mod evaluator;
mod json;
mod local_executor;
mod manifest;
mod process;
mod protocol;
mod provenance;
mod provider;
mod pyfmt;
mod redaction;
mod settings;
mod sha256;
mod tasks;
mod toml;
mod tracer;
mod verifier;

use std::path::{Component, Path, PathBuf};

use crate::engine::{run_agent, RunOptions};
use crate::json::Value;
use crate::provenance::{source_provenance, ALLOW_DIRTY_ENV};
use crate::pyfmt::{py_repr_str, py_str};
use crate::tasks::list_task_ids;

const USAGE: &str = "usage: vulcanbench-engine run --task <id> --model <provider:model> [--sandbox local] \
[--no-judges] [--output-dir <dir>] [--max-steps <n>] [--timeout <s>] [--effort <level>] \
[--tasks-root <dir>] [--override-budgets] [--allow-dirty] [--dry-run]";

/// Options of the `run` subcommand (the subset of `vulcanbench run` the spike takes).
#[derive(Debug)]
struct RunArgs {
    task: Option<String>,
    suite: Option<String>,
    tasks_root: PathBuf,
    model: Option<String>,
    output_dir: PathBuf,
    max_steps: Option<i64>,
    judges: bool,
    judge_model: Option<String>,
    sandbox: String,
    network: bool,
    timeout: Option<f64>,
    override_budgets: bool,
    effort: Option<String>,
    allow_dirty: bool,
    dry_run: bool,
}

/// `pathlib.Path(text)`: drops `.` components and duplicate or trailing slashes.
fn pathlib_path(text: &str) -> PathBuf {
    let mut out = PathBuf::new();
    for component in Path::new(text).components() {
        if component != Component::CurDir {
            out.push(component.as_os_str());
        }
    }
    if out.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        out
    }
}

fn usage_error(message: &str) -> i32 {
    eprintln!("{USAGE}");
    eprintln!("vulcanbench-engine run: error: {message}");
    2
}

fn parse_run_args(args: &[String]) -> Result<RunArgs, String> {
    let mut parsed = RunArgs {
        task: None,
        suite: None,
        tasks_root: pathlib_path(
            &std::env::var("VULCANBENCH_TASKS_ROOT").unwrap_or_else(|_| "tasks/v1".to_owned()),
        ),
        model: None,
        output_dir: pathlib_path(
            &std::env::var("VULCANBENCH_RUNS_DIR").unwrap_or_else(|_| "./runs".to_owned()),
        ),
        max_steps: None,
        judges: true,
        judge_model: None,
        sandbox: "docker".to_owned(),
        network: false,
        timeout: None,
        override_budgets: false,
        effort: None,
        allow_dirty: false,
        dry_run: false,
    };
    let mut i = 0;
    while i < args.len() {
        let raw = &args[i];
        let (flag, inline_value) = match raw.split_once('=') {
            Some((f, v)) if f.starts_with("--") => (f, Some(v.to_owned())),
            _ => (raw.as_str(), None),
        };
        let take_value = |i: &mut usize| -> Result<String, String> {
            if let Some(v) = &inline_value {
                return Ok(v.clone());
            }
            *i += 1;
            args.get(*i)
                .cloned()
                .ok_or_else(|| format!("argument {flag}: expected one argument"))
        };
        match flag {
            "--task" | "-t" => parsed.task = Some(take_value(&mut i)?),
            "--suite" => parsed.suite = Some(take_value(&mut i)?),
            "--tasks-root" => parsed.tasks_root = pathlib_path(&take_value(&mut i)?),
            "--model" | "-m" => parsed.model = Some(take_value(&mut i)?),
            "--output-dir" | "-o" => parsed.output_dir = pathlib_path(&take_value(&mut i)?),
            "--max-steps" => {
                let v = take_value(&mut i)?;
                parsed.max_steps = Some(v.parse().map_err(|_| {
                    format!("argument --max-steps: invalid int value: {}", py_repr_str(&v))
                })?);
            }
            "--timeout" => {
                let v = take_value(&mut i)?;
                parsed.timeout = Some(v.parse().map_err(|_| {
                    format!("argument --timeout: invalid float value: {}", py_repr_str(&v))
                })?);
            }
            "--judges" => parsed.judges = true,
            "--no-judges" => parsed.judges = false,
            "--judge-model" => parsed.judge_model = Some(take_value(&mut i)?),
            "--sandbox" => parsed.sandbox = take_value(&mut i)?,
            "--network" => parsed.network = true,
            "--no-network" => parsed.network = false,
            "--override-budgets" => parsed.override_budgets = true,
            "--effort" => parsed.effort = Some(take_value(&mut i)?),
            "--allow-dirty" => parsed.allow_dirty = true,
            "--dry-run" => parsed.dry_run = true,
            other => {
                return Err(format!(
                    "unrecognized arguments: {other} (the spike supports a subset of vulcanbench run)"
                ))
            }
        }
        i += 1;
    }
    if parsed.model.is_none() {
        return Err("the following arguments are required: --model/-m".to_owned());
    }
    Ok(parsed)
}

fn run_command(args: &[String]) -> i32 {
    let parsed = match parse_run_args(args) {
        Ok(parsed) => parsed,
        Err(message) => return usage_error(&message),
    };
    let model = parsed.model.clone().expect("checked by the parser");
    if let Err(err) = settings::check_effort_allowed(parsed.effort.as_deref(), &settings::settings_path()) {
        println!("error {err}");
        return 1;
    }
    if parsed.task.is_none() == parsed.suite.is_none() {
        println!("error pass exactly one of --task or --suite");
        return 1;
    }
    if parsed.suite.is_some() {
        println!("error suite runs are not implemented in the Rust spike; pass --task");
        return 1;
    }
    if !matches!(parsed.sandbox.as_str(), "local" | "docker" | "auto") {
        println!(
            "error --sandbox must be local|docker|auto, got {}",
            py_repr_str(&parsed.sandbox)
        );
        return 1;
    }
    let task = parsed.task.clone().expect("checked above");
    let known = list_task_ids(&parsed.tasks_root);
    if !known.contains(&task) {
        let available = if known.is_empty() { "(none)".to_owned() } else { known.join(", ") };
        println!("unknown task {}. Available: {available}", py_repr_str(&task));
        return 1;
    }
    if parsed.dry_run {
        let effort_note = parsed
            .effort
            .as_deref()
            .map(|e| format!(" effort={e}"))
            .unwrap_or_default();
        println!(
            "dry-run would run task={task} model={model} sandbox={}{effort_note}",
            parsed.sandbox
        );
        return 0;
    }
    if parsed.allow_dirty {
        std::env::set_var(ALLOW_DIRTY_ENV, "1");
    }
    if source_provenance().dirty() {
        println!(
            "warning uncommitted changes to scoring-relevant paths; \
             this run records dirty: true and is not publishable"
        );
    }
    let options = RunOptions {
        task_id: task,
        model,
        output_dir: parsed.output_dir,
        tasks_root: parsed.tasks_root,
        max_steps: parsed.max_steps,
        judges: parsed.judges,
        judge_model: parsed.judge_model,
        sandbox: parsed.sandbox,
        network: parsed.network,
        timeout_s: parsed.timeout,
        effort: parsed.effort,
        override_budgets: parsed.override_budgets,
    };
    match run_agent(&options) {
        Ok(result) => {
            let scores = result.summary.get("scores").cloned().unwrap_or_else(Value::object);
            let economics = result.summary.get("economics").cloned().unwrap_or_else(Value::object);
            let field = |v: &Value, key: &str| v.get(key).map(py_str).unwrap_or_else(|| "None".to_owned());
            println!("run complete {}", result.run_id);
            println!(
                "functional={} quality={} security={} human_like={} total={} marginal_cash=${} api_equivalent=${}",
                field(&scores, "functional"),
                field(&scores, "quality"),
                field(&scores, "security"),
                field(&scores, "human_like"),
                field(&scores, "total"),
                field(&economics, "marginal_cash_usd"),
                field(&economics, "api_equivalent_cost_usd"),
            );
            println!("replay: not generated (replay.html is out of scope for the spike)");
            0
        }
        Err(err) => {
            println!("{} {err}", err.cli_prefix());
            err.exit_code()
        }
    }
}

fn main() {
    // The Python CLI loads the nearest .env at import time; shell variables win.
    dotenv::load(Path::new(env!("CARGO_MANIFEST_DIR")));
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = match args.first().map(String::as_str) {
        Some("run") => run_command(&args[1..]),
        Some("--version" | "-v") => {
            println!("vulcanbench-engine {}", env!("CARGO_PKG_VERSION"));
            0
        }
        Some("--help" | "-h") => {
            println!("{USAGE}");
            0
        }
        None => {
            eprintln!("{USAGE}");
            2
        }
        Some(other) => {
            eprintln!("{USAGE}");
            eprintln!("vulcanbench-engine: error: unknown command {}", py_repr_str(other));
            2
        }
    };
    std::process::exit(code);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_normalize_like_pathlib() {
        assert_eq!(pathlib_path("./runs/"), PathBuf::from("runs"));
        assert_eq!(pathlib_path("a//b/./c"), PathBuf::from("a/b/c"));
        assert_eq!(pathlib_path("."), PathBuf::from("."));
        assert_eq!(pathlib_path("/abs/dir/"), PathBuf::from("/abs/dir"));
    }

    #[test]
    fn run_args_subset() {
        let args: Vec<String> = ["--task", "hello-world", "-m", "mock:synthetic", "--sandbox=local", "--no-judges", "-o", "./out"]
            .iter()
            .map(|s| (*s).to_owned())
            .collect();
        let parsed = parse_run_args(&args).expect("parses");
        assert_eq!(parsed.task.as_deref(), Some("hello-world"));
        assert_eq!(parsed.sandbox, "local");
        assert!(!parsed.judges);
        assert_eq!(parsed.output_dir, PathBuf::from("out"));
        let missing = parse_run_args(&["--task".to_owned(), "x".to_owned()]).expect_err("needs model");
        assert!(missing.contains("--model"));
        let bad = parse_run_args(&["-m".to_owned(), "a:b".to_owned(), "--repeat".to_owned(), "2".to_owned()]).expect_err("unsupported");
        assert!(bad.contains("unrecognized"));
    }
}
