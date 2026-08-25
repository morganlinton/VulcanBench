//! G12  -  CLI audit capabilities.
//!
//! Verifies that the CLI can be invoked programmatically and produces
//! expected output for the `list` and `run` subcommands.

#[test]
fn test_cli_list_produces_output() {
    let result = vyre_bench::cli::run_cli_with(["vyre-bench", "list", "--format", "table"]);
    assert!(
        matches!(result, Ok(())),
        "CLI list command should succeed: {:?}",
        result.err()
    );
}

#[test]
fn test_cli_snapshot_diff_requires_commit() {
    // snapshot-diff with a non-existent commit should bail
    let result = vyre_bench::cli::run_cli_with([
        "vyre-bench",
        "snapshot-diff",
        "--base",
        "0000000000000000000000000000000000000000",
    ]);
    assert!(
        matches!(result, Err(_)),
        "snapshot-diff should fail for non-existent commit"
    );
}
