//! Generate the cheap release workload matrix without running benchmarks.

use std::path::{Path, PathBuf};
use std::process::Command;

pub(crate) fn run(args: &[String]) {
    let config = match parse_args(args) {
        Ok(config) => config,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(2);
        }
    };
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let output = config.output.unwrap_or_else(|| {
        PathBuf::from("release/evidence/benchmarks/release-workload-matrix.json")
    });
    let runner = cargo_runner(&workspace_root);
    let mut command_args = vec![
        "run".to_string(),
        "-p".to_string(),
        "vyre-bench".to_string(),
        "--quiet".to_string(),
        "--".to_string(),
        "release-matrix".to_string(),
        "--format".to_string(),
        "json".to_string(),
        "--output".to_string(),
        output.display().to_string(),
    ];
    if config.enforce {
        command_args.push("--enforce".to_string());
    }
    let status = Command::new(&runner)
        .args(&command_args)
        .current_dir(&workspace_root)
        .status();
    match status {
        Ok(status) if status.success() => {
            println!("release-workload-matrix: wrote {}", output.display());
        }
        Ok(status) => {
            eprintln!(
                "Fix: `{}` exited with {status}. Workload matrix blockers must be resolved before release.",
                display_command(&runner, &command_args)
            );
            std::process::exit(1);
        }
        Err(error) => {
            eprintln!(
                "Fix: failed to run `{}`: {error}. Set VYRE_CARGO_RUNNER to the bounded workspace cargo wrapper if it is not named `cargo_full`.",
                display_command(&runner, &command_args)
            );
            std::process::exit(1);
        }
    }
}

struct Config {
    output: Option<PathBuf>,
    enforce: bool,
}

fn parse_args(args: &[String]) -> Result<Config, String> {
    let mut output = None;
    let mut enforce = false;
    let mut index = 2;
    while index < args.len() {
        match args[index].as_str() {
            "--output" => {
                let Some(path) = args.get(index + 1) else {
                    return Err("Fix: --output requires a path.".to_string());
                };
                output = Some(PathBuf::from(path));
                index += 2;
            }
            "--enforce" => {
                enforce = true;
                index += 1;
            }
            "--help" | "-h" => {
                println!(
                    "USAGE:\n  cargo_full run --bin xtask -- release-workload-matrix [--output PATH] [--enforce]\n\n\
                     Writes the release workload family matrix without running benchmark cases."
                );
                std::process::exit(0);
            }
            other => {
                return Err(format!(
                    "Fix: unknown release-workload-matrix option `{other}`."
                ));
            }
        }
    }
    Ok(Config { output, enforce })
}

fn cargo_runner(workspace_root: &Path) -> PathBuf {
    if let Some(runner) = std::env::var_os("VYRE_CARGO_RUNNER") {
        return PathBuf::from(runner);
    }
    let local = workspace_root.join("cargo_full");
    if local.is_file() {
        return local;
    }
    PathBuf::from("cargo_full")
}

fn display_command(runner: &Path, args: &[String]) -> String {
    format!("{} {}", runner.display(), args.join(" "))
}
