//! Formal correctness governance test suite.

const LEDGER: &str = include_str!("../../../docs/optimization/RESEARCH_SOURCE_LEDGER.toml");
const MODELS: &str = include_str!("../../../docs/optimization/FORMAL_MODEL_CONTRACTS.toml");
const COUNTEREXAMPLES: &str =
    include_str!("../../../docs/optimization/CORRECTNESS_COUNTEREXAMPLE_ARTIFACTS.toml");
const COVERAGE: &str =
    include_str!("../../../docs/optimization/CORRECTNESS_VALIDATION_TRANCHE_COVERAGE.toml");

#[test]
fn formal_correctness_primary_sources_are_registered() {
    for key in [
        "TLA_PLUS",
        "ALLOY_ANALYZER",
        "KANI_RUST_VERIFIER",
        "LOOM",
        "PROPTEST",
        "QUICKCHECK",
        "CSMITH",
        "C_REDUCE",
    ] {
        assert!(LEDGER.contains(key), "research source ledger must include {key}");
    }
}

#[test]
fn formal_model_contracts_require_invariants_assumptions_bounds_and_counterexamples() {
    for required in [
        "model_id",
        "model_kind",
        "owning_crate",
        "invariant",
        "assumptions",
        "bounded_scope",
        "counterexample_artifact",
        "source_impact_edge",
    ] {
        assert!(
            MODELS.contains(required),
            "formal model contract must include {required}"
        );
    }
}

#[test]
fn correctness_counterexample_artifacts_link_inputs_assumptions_reports_and_privacy() {
    for required in [
        "artifact_id",
        "source_contract",
        "counterexample_digest",
        "minimal_input",
        "assumption_set",
        "replay_command_class",
        "operator_report_link",
        "privacy_class",
    ] {
        assert!(
            COUNTEREXAMPLES.contains(required),
            "correctness counterexample artifact must include {required}"
        );
    }
}

#[test]
fn correctness_validation_tranche_coverage_preserves_shared_dedup_seams() {
    for required in [
        "VX-761..VX-780",
        "formal_model_contract",
        "property_differential_oracles",
        "concurrency_schedule_contracts",
        "counterexample_artifacts",
        "source_ledger_coverage",
        "dedup_seam",
        "proof_gate",
    ] {
        assert!(
            COVERAGE.contains(required),
            "correctness validation coverage must include {required}"
        );
    }
}
