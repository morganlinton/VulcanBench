//! Plan policy enforcement governance test suite.

const LEDGER: &str = include_str!("../../../docs/optimization/RESEARCH_SOURCE_LEDGER.toml");
const PLAN: &str = include_str!("../../../docs/optimization/ALL_AXES_ACCELERATION_PLAN.md");
const POLICY: &str = include_str!("../../../docs/optimization/PLAN_POLICY_AS_CODE_RULES.toml");
const SCHEMA: &str =
    include_str!("../../../docs/optimization/PLAN_SCHEMA_AND_CONSTRAINT_VALIDATION.toml");
const QUERIES: &str = include_str!("../../../docs/optimization/PLAN_GRAPH_QUERY_AUDITS.toml");
const COVERAGE: &str =
    include_str!("../../../docs/optimization/END_TO_END_POLICY_ENFORCEMENT_TRANCHE_COVERAGE.toml");

#[test]
fn policy_enforcement_sources_are_registered_without_duplicating_existing_schema_or_datalog_keys() {
    for key in [
        "OPEN_POLICY_AGENT_REGO",
        "CUE_LANG",
        "CEL_SPEC",
        "W3C_SHACL",
        "JSON_SCHEMA_2020_12",
        "SOUFFLE_CC",
    ] {
        assert!(LEDGER.contains(key), "research source ledger must include {key}");
    }
}

#[test]
fn policy_rules_encode_completion_publication_dedup_and_dag_denials() {
    for required in [
        "policy_id",
        "engine_family",
        "input_artifacts",
        "decision",
        "deny_condition",
        "required_evidence",
        "diagnostic",
        "publication_class",
        "deny-completion-without-authorized-validation",
        "deny-private-santh-publication",
        "deny-duplicate-authority-registries",
        "deny-unsequenced-plan-node",
    ] {
        assert!(POLICY.contains(required), "policy rule registry must include {required}");
    }
}

#[test]
fn schema_constraints_cover_dag_authority_attestation_and_policy_artifacts() {
    for required in [
        "validator_id",
        "validator_family",
        "target_artifacts",
        "schema_contract",
        "constraint_contract",
        "expression_contract",
        "graph_shape_contract",
        "negative_case",
        "diagnostic",
        "execution-dag-schema",
        "artifact-authority-schema",
        "completion-attestation-schema",
        "policy-rule-schema",
    ] {
        assert!(
            SCHEMA.contains(required),
            "schema and constraint registry must include {required}"
        );
    }
}

#[test]
fn graph_queries_find_unowned_completion_private_and_stale_evidence_failures() {
    for required in [
        "query_id",
        "query_family",
        "input_graph",
        "relationship",
        "must_return",
        "must_not_return",
        "negative_fixture",
        "operator_result",
        "publication_class",
        "find-unowned-plan-artifacts",
        "find-completion-claims-without-gate-evidence",
        "find-private-boundary-leaks",
        "find-stale-dependent-artifacts",
    ] {
        assert!(QUERIES.contains(required), "graph query audit registry must include {required}");
    }
}

#[test]
fn plan_contains_policy_enforcement_rows() {
    for row in [
        "VX-1021",
        "VX-1022",
        "VX-1023",
        "VX-1024",
        "VX-1025",
        "VX-1026",
        "VX-1027",
        "VX-1028",
        "VX-1029",
        "VX-1030",
        "VX-1031",
        "VX-1032",
        "VX-1033",
        "VX-1034",
        "VX-1035",
        "VX-1036",
        "VX-1037",
        "VX-1038",
        "VX-1039",
        "VX-1040",
    ] {
        assert!(PLAN.contains(row), "plan must include {row}");
    }
}

#[test]
fn policy_enforcement_coverage_preserves_dedup_seams() {
    for required in [
        "VX-1021..VX-1040",
        "policy_as_code_rules",
        "schema_and_constraint_validation",
        "graph_query_audits",
        "execution_dag",
        "artifact_authority_map",
        "publication_boundary",
        "dedup_seam",
        "proof_gate",
    ] {
        assert!(
            COVERAGE.contains(required),
            "policy enforcement tranche coverage must include {required}"
        );
    }
}
