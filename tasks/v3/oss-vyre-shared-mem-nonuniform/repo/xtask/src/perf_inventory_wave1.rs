//! `cargo_full xtask perf-inventory-wave1`  -  run Phase 1 wave 1.1 contract tests
//! for [`audits/WAVE_EXECUTION.md`](../audits/WAVE_EXECUTION.md).

use std::path::PathBuf;
use std::process::Command;

fn repo_root() -> PathBuf {
    std::env::current_dir().expect("Fix: xtask must run in a cwd; restore this invariant before continuing.")
}

pub fn run(_args: &[String]) {
    let root = repo_root();
    let cargo = cargo_runner(&root);

    let script = root.join("scripts/check_performance_inventory_wave1.sh");
    if script.is_file() {
        let status = Command::new("bash")
            .arg(&script)
            .current_dir(&root)
            .status()
            .unwrap_or_else(|e| {
                eprintln!("Fix: could not run {}: {e}", script.display());
                std::process::exit(1);
            });
        if !status.success() {
            std::process::exit(status.code().unwrap_or(1));
        }
        return;
    }

    // Fallback if script path differs (e.g. nested invocation).
    eprintln!("==> perf-inventory-wave1: running cargo_full tests directly");
    for args in [
        vec![
            "test",
            "-p",
            "vyre-foundation",
            "--test",
            "optimizer_reference_parity_smoke",
        ],
        vec![
            "test",
            "-p",
            "vyre-driver-wgpu",
            "--test",
            "dispatch_allocation_contract",
        ],
    ] {
        let mut cmd = Command::new(&cargo);
        cmd.args(&args).current_dir(&root);
        let status = cmd
            .status()
            .expect("Fix: cargo_full; restore this invariant before continuing.");
        if !status.success() {
            eprintln!(
                "Fix: perf-inventory-wave1 failed on: {} {}",
                cargo.display(),
                args.join(" ")
            );
            std::process::exit(status.code().unwrap_or(1));
        }
    }
}

fn cargo_runner(root: &std::path::Path) -> PathBuf {
    if let Ok(runner) = std::env::var("VYRE_CARGO_RUNNER") {
        return PathBuf::from(runner);
    }
    let local = root.join("cargo_full");
    if local.is_file() {
        return local;
    }
    PathBuf::from("cargo_full")
}
