//! xtask CLI surface contract: top subcommands must remain discoverable via `--help`.

use std::process::Command;

use super::workspace_root;

const REQUIRED_SUBCOMMANDS: &[&str] = &[
    "list-ops",
    "catalog",
    "conformance-matrix",
    "lint-shape-tests",
];

#[test]
fn xtask_help_lists_required_subcommands() {
    let workspace = workspace_root();
    let output = Command::new("cargo")
        .args(["xtask", "--help"])
        .current_dir(&workspace)
        .output()
        .expect("cargo xtask --help should execute");

    assert!(
        output.status.success(),
        "cargo xtask --help must succeed.\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let help = String::from_utf8_lossy(&output.stdout);
    for subcommand in REQUIRED_SUBCOMMANDS {
        assert!(
            help.contains(subcommand),
            "xtask --help must document subcommand `{subcommand}`.\nhelp excerpt:\n{}",
            help.lines().take(40).collect::<Vec<_>>().join("\n")
        );
    }
}
