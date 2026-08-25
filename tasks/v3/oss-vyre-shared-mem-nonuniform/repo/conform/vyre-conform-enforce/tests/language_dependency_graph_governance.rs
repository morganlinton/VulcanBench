//! Language dependency graph governance test suite.

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
fn language_dependency_graph_sources_are_registered() {
    let ledger = read_repo_file("docs/optimization/RESEARCH_SOURCE_LEDGER.toml");

    for key in [
        "GCC_PREPROCESSOR_DEPENDENCIES",
        "CLANG_DEPENDENCY_SCANNING",
        "PYTHON_IMPORT_SYSTEM",
        "RUST_MODULES_REFERENCE",
    ] {
        assert_contains(&ledger, key);
    }
}

#[test]
fn language_dependency_graph_artifacts_define_boundaries() {
    for relative in [
        "docs/optimization/LANGUAGE_DEPENDENCY_EDGE_MATRIX.toml",
        "docs/optimization/MODULE_IMPORT_RESOLUTION_CONTRACTS.toml",
        "docs/optimization/CROSS_FILE_INVALIDATION_PROPAGATION_POLICY.toml",
        "docs/optimization/DEPENDENCY_GRAPH_REUSE_EVIDENCE.toml",
        "docs/optimization/END_TO_END_LANGUAGE_DEPENDENCY_GRAPH_TRANCHE_COVERAGE.toml",
    ] {
        let artifact = read_repo_file(relative);
        assert_contains(&artifact, "VX-1561..VX-1580");
        assert_contains(&artifact, "research_sources");
        assert_contains(&artifact, "owns");
        assert_contains(&artifact, "does_not_own");
    }
}

#[test]
fn edge_matrix_covers_c_cpp_python_and_rust() {
    let matrix = read_repo_file("docs/optimization/LANGUAGE_DEPENDENCY_EDGE_MATRIX.toml");

    for token in [
        "language_rows.c_cpp",
        "language_rows.python",
        "language_rows.rust",
        "include directives",
        "module spec",
        "mod declarations",
    ] {
        assert_contains(&matrix, token);
    }
}

#[test]
fn resolution_contracts_cover_uncertain_dependency_states() {
    let contracts = read_repo_file("docs/optimization/MODULE_IMPORT_RESOLUTION_CONTRACTS.toml");

    for token in ["resolved", "unresolved", "ambiguous", "dynamic", "cycle", "operator_fix"] {
        assert_contains(&contracts, token);
    }
}

#[test]
fn propagation_policy_requires_cross_file_truth() {
    let policy = read_repo_file("docs/optimization/CROSS_FILE_INVALIDATION_PROPAGATION_POLICY.toml");

    for token in [
        "changed_file",
        "strongly connected component",
        "positive",
        "negative",
        "adversarial",
        "cross_file",
        "provider file",
        "consumer file",
    ] {
        assert_contains(&policy, token);
    }
}

#[test]
fn reuse_evidence_records_correctness_and_performance_fields() {
    let evidence = read_repo_file("docs/optimization/DEPENDENCY_GRAPH_REUSE_EVIDENCE.toml");

    for token in [
        "false_reuse_count",
        "unresolved_edges",
        "ambiguous_edges",
        "bytes_avoided",
        "affected_consumers",
        "OUTPUT_SLAB_SCHEMA_PROVENANCE",
    ] {
        assert_contains(&evidence, token);
    }
}

#[test]
fn acceleration_plan_contains_complete_language_dependency_graph_tranche() {
    let plan = read_repo_file("docs/optimization/ALL_AXES_ACCELERATION_PLAN.md");

    for id in 1561..=1580 {
        assert_contains(&plan, &format!("VX-{id}"));
    }

    for token in [
        "LANGUAGE_DEPENDENCY_EDGE_MATRIX",
        "MODULE_IMPORT_RESOLUTION_CONTRACTS",
        "CROSS_FILE_INVALIDATION_PROPAGATION_POLICY",
        "DEPENDENCY_GRAPH_REUSE_EVIDENCE",
        "END_TO_END_LANGUAGE_DEPENDENCY_GRAPH_TRANCHE_COVERAGE",
    ] {
        assert_contains(&plan, token);
    }
}
