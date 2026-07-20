//! `cargo_full run --bin xtask -- check-cat-a`  -  one-shot gate that runs every
//! pre-merge CI step a Cat-A author cares about:
//!
//! 1. `cargo_full check --workspace --all-features --all-targets`
//! 2. `cargo_full clippy --workspace --all-features --all-targets -- -D warnings`
//! 3. `cargo_full test -p vyre-libs --all-features`
//! 4. `cargo_full test -p vyre-foundation --all-features` (region-inline + wire)
//! 5. `cargo_full test -p vyre-reference --all-features` (assign + lifetime)
//! 6. `scripts/check_parity_testing_not_leaked.sh`
//! 7. `scripts/check_op_names.sh`
//! 8. `cargo_full run -p xtask --bin xtask -- platform-boundary`
//! 9. `cargo_full doc --workspace --all-features --no-deps`
//!
//! Exits non-zero on the first failure; prints a pass summary on
//! success. Designed to be the single command a Cat-A author runs
//! before opening a PR.

use std::path::PathBuf;
use std::process::{Command, ExitStatus};

fn repo_root() -> PathBuf {
    // xtask binary is invoked via `cargo_full run --bin xtask --`, whose CWD is the
    // workspace root  -  use that directly.
    std::env::current_dir()
        .expect("Fix: xtask must run in a cwd; restore this invariant before continuing.")
}

fn run_step(label: &str, mut cmd: Command) -> ExitStatus {
    println!("\n==> {label}");
    println!("    $ {cmd:?}");
    let status = match cmd.status() {
        Ok(status) => status,
        Err(error) => {
            eprintln!("Fix: `{label}` could not launch: {error}");
            std::process::exit(1);
        }
    };
    if !status.success() {
        eprintln!("==> FAIL: {label} (exit {})", status.code().unwrap_or(-1));
    }
    status
}

pub(crate) fn run(_args: &[String]) {
    let root = repo_root();
    let mut failed = Vec::<&str>::new();

    let cargo = std::env::var("VYRE_CARGO_RUNNER").unwrap_or_else(|_| "cargo_full".to_string());

    let mut check = Command::new(&cargo);
    check
        .args(["check", "--workspace", "--all-features", "--all-targets"])
        .current_dir(&root);
    if !run_step("check (all-features, all-targets)", check).success() {
        failed.push("cargo_full check");
    }

    let mut clippy = Command::new(&cargo);
    clippy
        .args([
            "clippy",
            "--workspace",
            "--all-features",
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ])
        .current_dir(&root);
    if !run_step("clippy -D warnings", clippy).success() {
        failed.push("cargo_full clippy");
    }

    for crate_name in &["vyre-libs", "vyre-foundation", "vyre-reference"] {
        let mut test = Command::new(&cargo);
        test.args(["test", "-p", crate_name, "--all-features"])
            .current_dir(&root);
        if !run_step(&format!("test -p {crate_name}"), test).success() {
            failed.push("cargo_full test");
        }
    }

    for script in &[
        "scripts/check_parity_testing_not_leaked.sh",
        "scripts/check_op_names.sh",
    ] {
        let mut sh = Command::new("bash");
        sh.arg(script).current_dir(&root);
        if !run_step(script, sh).success() {
            failed.push(script);
        }
    }

    let mut platform_boundary = Command::new(&cargo);
    platform_boundary
        .args([
            "run",
            "-p",
            "xtask",
            "--bin",
            "xtask",
            "--",
            "platform-boundary",
        ])
        .current_dir(&root);
    if !run_step("platform-boundary", platform_boundary).success() {
        failed.push("xtask platform-boundary");
    }

    let mut doc = Command::new(&cargo);
    doc.args(["doc", "--workspace", "--all-features", "--no-deps"])
        .current_dir(&root);
    if !run_step("doc (no-deps)", doc).success() {
        failed.push("cargo_full doc");
    }

    if !failed.is_empty() {
        eprintln!("\n==> check-cat-a FAILED on: {failed:?}");
        std::process::exit(1);
    }
    println!("\n==> check-cat-a: all gates passed.");
}
