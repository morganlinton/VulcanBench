//! Command execution boundary test suite.

const COMMANDS: &str =
    include_str!("../../docs/optimization/COMMAND_EXECUTION_BOUNDARY.toml");

#[test]
fn command_execution_boundary_records_shell_argv_env_cwd_stdin_and_diagnostics() {
    for required in [
        "boundary_id",
        "operation_class",
        "shell_policy",
        "argv_policy",
        "environment_policy",
        "cwd_policy",
        "stdin_policy",
        "diagnostic",
        "never-invoke-through-shell",
        "predeclared-argument-schema-no-string-joining",
    ] {
        assert!(
            COMMANDS.contains(required),
            "command execution boundary must include {required}"
        );
    }
}
