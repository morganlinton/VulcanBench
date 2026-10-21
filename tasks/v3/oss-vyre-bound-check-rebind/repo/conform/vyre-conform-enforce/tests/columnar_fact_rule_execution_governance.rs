//! Columnar fact rule execution governance test suite.

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
fn columnar_fact_rule_execution_sources_are_registered() {
    let ledger = read_repo_file("docs/optimization/RESEARCH_SOURCE_LEDGER.toml");

    for key in [
        "APACHE_ARROW_COLUMNAR_FORMAT",
        "APACHE_DATAFUSION_QUERY_OPTIMIZER",
        "DUCKDB_VECTOR_EXECUTION_FORMAT",
        "SUBSTRAIT_SPECIFICATION",
    ] {
        assert_contains(&ledger, key);
    }
}

#[test]
fn columnar_fact_rule_execution_artifacts_define_boundaries() {
    for relative in [
        "docs/optimization/COLUMNAR_FACT_STORE_LAYOUT.toml",
        "docs/optimization/VECTORIZED_RULE_EXECUTION_PLAN.toml",
        "docs/optimization/FACT_QUERY_COST_MODEL_POLICY.toml",
        "docs/optimization/FACT_STORE_REUSE_EVIDENCE.toml",
        "docs/optimization/END_TO_END_COLUMNAR_FACT_RULE_EXECUTION_TRANCHE_COVERAGE.toml",
    ] {
        let artifact = read_repo_file(relative);
        assert_contains(&artifact, "VX-1601..VX-1620");
        assert_contains(&artifact, "research_sources");
        assert_contains(&artifact, "owns");
        assert_contains(&artifact, "does_not_own");
    }
}

#[test]
fn columnar_layout_covers_batches_buffers_dictionaries_and_domains() {
    let layout = read_repo_file("docs/optimization/COLUMNAR_FACT_STORE_LAYOUT.toml");

    for token in [
        "relation_batch",
        "column_buffers",
        "dictionary_columns",
        "selection_vectors",
        "join_keys",
        "compressed_domains",
        "ROARING",
    ] {
        assert_contains(&layout, token);
    }
}

#[test]
fn vectorized_plan_covers_operator_set_routes_and_fusion() {
    let plan = read_repo_file("docs/optimization/VECTORIZED_RULE_EXECUTION_PLAN.toml");

    for token in [
        "filter",
        "projection",
        "hash_join",
        "sort_merge_join",
        "semi_join",
        "anti_join",
        "aggregation",
        "cpu_vector",
        "gpu_batch",
        "portable_iterator",
        "fusion_policy",
    ] {
        assert_contains(&plan, token);
    }
}

#[test]
fn fact_query_cost_model_covers_optimizer_decisions_and_plan_digests() {
    let policy = read_repo_file("docs/optimization/FACT_QUERY_COST_MODEL_POLICY.toml");

    for token in [
        "relation_row_count",
        "selection_density",
        "join_key_cardinality",
        "predicate_pushdown",
        "projection_pruning",
        "join_reordering",
        "bitmap_domain",
        "gpu_route",
        "plan_digest",
    ] {
        assert_contains(&policy, token);
    }
}

#[test]
fn fact_store_evidence_records_operator_truth_and_publication_boundary() {
    let evidence = read_repo_file("docs/optimization/FACT_STORE_REUSE_EVIDENCE.toml");

    for token in [
        "required_per_run",
        "required_per_operator",
        "truth_gates",
        "cpu_vector_ns",
        "gpu_batch_ns",
        "portable_iterator_ns",
        "output_equivalence_digest",
        "public_fields",
        "private_fields",
    ] {
        assert_contains(&evidence, token);
    }
}

#[test]
fn acceleration_plan_contains_complete_columnar_fact_rule_execution_tranche() {
    let plan = read_repo_file("docs/optimization/ALL_AXES_ACCELERATION_PLAN.md");

    for id in 1601..=1620 {
        assert_contains(&plan, &format!("VX-{id}"));
    }

    for token in [
        "COLUMNAR_FACT_STORE_LAYOUT",
        "VECTORIZED_RULE_EXECUTION_PLAN",
        "FACT_QUERY_COST_MODEL_POLICY",
        "FACT_STORE_REUSE_EVIDENCE",
        "END_TO_END_COLUMNAR_FACT_RULE_EXECUTION_TRANCHE_COVERAGE",
    ] {
        assert_contains(&plan, token);
    }
}
