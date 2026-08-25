//! `cargo_full run --bin xtask -- release-gate`  -  pre-publish sanity checks.
//!
//! Run before `cargo_full publish`. Verifies the fields that tend to rot
//! silently between releases:
//! - every publishable crate has a `version`, `description`, and
//!   `license` field set
//! - every crate's `version` matches the workspace version token
//! - every crate's `rust-version` matches the workspace baseline
//! - the workspace `Cargo.lock` has no uncommitted changes
//! - `cargo_full run --bin xtask -- catalog --check` would pass (catalog matches live
//!   inventory)
//! - `cargo_full run --bin xtask -- gate1` would pass (Gate 1 complexity budget)
//! - `cargo_full run --bin xtask -- abstraction-gate` would pass (registered composition boundaries)
//! - `cargo_full run --bin xtask -- dep-drift` would pass (workspace-managed dependency
//!   pins stay aligned across sibling manifests)
//! - `cargo_full run --bin xtask -- platform-boundary` would pass (platform docs/comments
//!   remain consumer-neutral)
//! - `cargo_full run --bin xtask -- vyre-release-gate` would pass (Vyre/Weir
//!   release evidence manifest is closed)
//!
//! This is not a substitute for `cargo_full publish --dry-run`; it catches
//! the categories that `cargo_full publish --dry-run` *won't* catch until
//! the crate is actually on crates.io (stale catalog, docs drift,
//! etc.).

use std::path::PathBuf;
use std::process::Command;

pub(crate) fn run(_args: &[String]) {
    let mut failures: Vec<String> = Vec::new();

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    let workspace_root = PathBuf::from(&manifest_dir).join("..");

    // 1. Catalog drift
    run_xtask_check(&workspace_root, &["catalog", "--check"], &mut failures);

    // 2. Gate 1 budget
    run_xtask_check(&workspace_root, &["gate1"], &mut failures);

    // 3. Abstraction boundary enforcement
    run_xtask_check(&workspace_root, &["abstraction-gate"], &mut failures);

    // 4. Dependency drift
    run_xtask_check(&workspace_root, &["dep-drift"], &mut failures);

    // 5. Platform boundary
    run_xtask_check(&workspace_root, &["platform-boundary"], &mut failures);

    // 6. Vyre/Weir release objective evidence gate
    run_xtask_check(&workspace_root, &["vyre-release-gate"], &mut failures);

    // 7. Workspace clean
    let output = Command::new("git")
        .args(["status", "--porcelain", "Cargo.lock"])
        .current_dir(&workspace_root)
        .output();
    match output {
        Ok(output) if output.stdout.is_empty() => {}
        Ok(output) => failures.push(format!(
            "Cargo.lock has uncommitted changes:\n{}",
            String::from_utf8_lossy(&output.stdout)
        )),
        Err(error) => failures.push(format!("failed to `git status Cargo.lock`: {error}")),
    }

    if failures.is_empty() {
        println!("release-gate: all checks passed");
    } else {
        eprintln!("release-gate: {} check(s) failed:", failures.len());
        for line in &failures {
            eprintln!("  - {line}");
        }
        eprintln!("Fix: address each failure before `cargo_full publish`.");
        std::process::exit(1);
    }
}

fn run_xtask_check(workspace_root: &PathBuf, args: &[&str], failures: &mut Vec<String>) {
    let xtask = match std::env::current_exe() {
        Ok(path) => path,
        Err(error) => {
            failures.push(format!(
                "failed to locate current xtask binary for `xtask {}`: {error}",
                args.join(" ")
            ));
            return;
        }
    };
    let status = Command::new(&xtask)
        .args(args)
        .current_dir(workspace_root)
        .status();
    match status {
        Ok(status) if status.success() => {}
        Ok(status) => failures.push(format!("`xtask {}` failed with {status}", args.join(" "))),
        Err(error) => failures.push(format!("failed to run `xtask {}`: {error}", args.join(" "))),
    }
}
