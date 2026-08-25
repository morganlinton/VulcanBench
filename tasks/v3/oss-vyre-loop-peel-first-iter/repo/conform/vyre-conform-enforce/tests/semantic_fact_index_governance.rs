//! Semantic fact index governance test suite.

use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("conform crate must live under the vyre repository")
}

fn read_repo_file(relative: &str) -> String {
    fs::read_to_string(repo_root().join(relative))
        .unwrap_or_else(|error| panic!("failed to read {relative}: {error}"))
}

fn assert_contains(haystack: &str, needle: &str) {
    assert!(haystack.contains(needle), "missing required governance token: {needle}");
}

#[test]
fn semantic_fact_index_sources_are_registered() {
    let ledger = read_repo_file("docs/optimization/RESEARCH_SOURCE_LEDGER.toml");

    for key in [
        "SCIP_CODE_INTELLIGENCE_PROTOCOL",
        "LSP_SEMANTIC_TOKENS_3_17",
        "SOUFFLE_RELATIONS",
        "CODEQL_DATA_FLOW_ANALYSIS",
    ] {
        assert_contains(&ledger, key);
    }
}

#[test]
fn semantic_fact_index_artifacts_define_boundaries() {
    for relative in [
        "docs/optimization/SEMANTIC_FACT_INDEX_SCHEMA.toml",
        "docs/optimization/SYMBOL_REFERENCE_STABILITY_POLICY.toml",
        "docs/optimization/INCREMENTAL_DATAFLOW_FACT_REUSE_POLICY.toml",
        "docs/optimization/SEMANTIC_FACT_INDEX_EVIDENCE.toml",
        "docs/optimization/END_TO_END_SEMANTIC_FACT_INDEX_TRANCHE_COVERAGE.toml",
    ] {
        let artifact = read_repo_file(relative);
        assert_contains(&artifact, "VX-1581..VX-1600");
        assert_contains(&artifact, "research_sources");
        assert_contains(&artifact, "owns");
        assert_contains(&artifact, "does_not_own");
    }
}

#[test]
fn fact_schema_covers_symbols_references_calls_types_scopes_and_flow() {
    let schema = read_repo_file("docs/optimization/SEMANTIC_FACT_INDEX_SCHEMA.toml");

    for token in [
        "symbol",
        "definition",
        "reference",
        "call",
        "type_fact",
        "scope",
        "flow_node",
        "flow_edge",
        "taint_source",
        "taint_sink",
    ] {
        assert_contains(&schema, token);
    }
}

#[test]
fn symbol_policy_covers_stable_unstable_unknown_and_delta_cases() {
    let policy = read_repo_file("docs/optimization/SYMBOL_REFERENCE_STABILITY_POLICY.toml");

    for token in [
        "stable_when",
        "unstable_when",
        "unknown",
        "reference_delta",
        "definition_delta",
        "callsite_delta",
        "reuse_blocked",
    ] {
        assert_contains(&policy, token);
    }
}

#[test]
fn dataflow_policy_covers_delta_fixpoint_and_truth_requirements() {
    let policy = read_repo_file("docs/optimization/INCREMENTAL_DATAFLOW_FACT_REUSE_POLICY.toml");

    for token in [
        "local_flow",
        "global_flow",
        "taint_flow",
        "control_flow",
        "base_delta",
        "derived_delta",
        "join_policy",
        "positive",
        "negative",
        "adversarial",
        "cross_file",
    ] {
        assert_contains(&policy, token);
    }
}

#[test]
fn fact_index_evidence_records_reuse_truth_and_publication_boundary() {
    let evidence = read_repo_file("docs/optimization/SEMANTIC_FACT_INDEX_EVIDENCE.toml");

    for token in [
        "base_fact_delta_count",
        "derived_fact_delta_count",
        "fixpoint_iterations",
        "reuse_rejected",
        "bytes_avoided",
        "false_reuse_count",
        "public_fields",
        "private_fields",
    ] {
        assert_contains(&evidence, token);
    }
}

#[test]
fn acceleration_plan_contains_complete_semantic_fact_index_tranche() {
    let plan = read_repo_file("docs/optimization/ALL_AXES_ACCELERATION_PLAN.md");

    for id in 1581..=1600 {
        assert_contains(&plan, &format!("VX-{id}"));
    }

    for token in [
        "SEMANTIC_FACT_INDEX_SCHEMA",
        "SYMBOL_REFERENCE_STABILITY_POLICY",
        "INCREMENTAL_DATAFLOW_FACT_REUSE_POLICY",
        "SEMANTIC_FACT_INDEX_EVIDENCE",
        "END_TO_END_SEMANTIC_FACT_INDEX_TRANCHE_COVERAGE",
    ] {
        assert_contains(&plan, token);
    }
}
