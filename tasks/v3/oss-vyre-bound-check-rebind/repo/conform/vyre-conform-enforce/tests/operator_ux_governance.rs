//! Operator ux governance test suite.

const LEDGER: &str = include_str!("../../../docs/optimization/RESEARCH_SOURCE_LEDGER.toml");
const CLI: &str = include_str!("../../../docs/optimization/CLI_SURFACE_CONTRACTS.toml");
const ERRORS: &str =
    include_str!("../../../docs/optimization/STRUCTURED_ERROR_PROBLEM_DETAILS.toml");
const EXIT_STREAM: &str =
    include_str!("../../../docs/optimization/EXIT_CODE_AND_STREAM_POLICY.toml");
const COHERENCE: &str =
    include_str!("../../../docs/optimization/DOCS_HELP_EXAMPLE_COHERENCE.toml");
const COVERAGE: &str =
    include_str!("../../../docs/optimization/OPERATOR_UX_GOVERNANCE_TRANCHE_COVERAGE.toml");

#[test]
fn operator_ux_primary_sources_are_registered() {
    for key in [
        "RFC_9457_PROBLEM_DETAILS",
        "CLAP_RS",
        "GNU_CLI_STANDARDS",
        "POSIX_UTILITY_CONVENTIONS",
    ] {
        assert!(LEDGER.contains(key), "research source ledger must include {key}");
    }
}

#[test]
fn cli_surface_contract_records_command_options_help_config_env_and_machine_modes() {
    for required in [
        "command_id",
        "subcommand",
        "option_contract",
        "help_contract",
        "config_binding",
        "env_binding",
        "completion_contract",
        "tty_policy",
        "machine_output_policy",
    ] {
        assert!(CLI.contains(required), "CLI surface contract must include {required}");
    }
}

#[test]
fn structured_error_problem_details_require_fix_redaction_exit_and_report_link() {
    for required in [
        "error_id",
        "type_uri",
        "title",
        "status_class",
        "detail",
        "instance",
        "exit_code",
        "remediation",
        "redaction_state",
        "report_link",
    ] {
        assert!(
            ERRORS.contains(required),
            "structured error problem details must include {required}"
        );
    }
}

#[test]
fn exit_code_and_stream_policy_separates_stdout_stderr_tty_and_machine_modes() {
    for required in [
        "case_id",
        "exit_code",
        "stdout_contract",
        "stderr_contract",
        "tty_behavior",
        "machine_mode_behavior",
        "diagnostic_contract",
        "usage-error",
        "release-boundary-error",
    ] {
        assert!(
            EXIT_STREAM.contains(required),
            "exit code and stream policy must include {required}"
        );
    }
}

#[test]
fn docs_help_example_coherence_links_readme_help_examples_fields_exit_and_config() {
    for required in [
        "coherence_id",
        "readme_claim",
        "help_claim",
        "example_path",
        "json_field_contract",
        "sarif_field_contract",
        "exit_code_contract",
        "config_contract",
        "publication_class",
    ] {
        assert!(
            COHERENCE.contains(required),
            "docs/help/example coherence must include {required}"
        );
    }
}

#[test]
fn operator_ux_tranche_coverage_preserves_shared_dedup_seams() {
    for required in [
        "VX-821..VX-840",
        "cli_surface_contract",
        "structured_error_contract",
        "exit_stream_policy",
        "docs_help_example_coherence",
        "source_ledger_coverage",
        "publication_boundary",
        "dedup_seam",
        "proof_gate",
    ] {
        assert!(
            COVERAGE.contains(required),
            "operator UX tranche coverage must include {required}"
        );
    }
}
