//! Incremental corpus index governance test suite.

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
fn incremental_corpus_index_sources_are_registered() {
    let ledger = read_repo_file("docs/optimization/RESEARCH_SOURCE_LEDGER.toml");

    for key in ["LINUX_INOTIFY", "LINUX_FANOTIFY", "SQLITE_WAL", "BLAKE3_SPEC"] {
        assert_contains(&ledger, key);
    }
}

#[test]
fn incremental_corpus_index_artifacts_define_boundaries() {
    for relative in [
        "docs/optimization/INCREMENTAL_CORPUS_CHANGE_JOURNAL.toml",
        "docs/optimization/FILESYSTEM_WATCHER_EVENT_POLICY.toml",
        "docs/optimization/CORPUS_CONTENT_IDENTITY_INDEX.toml",
        "docs/optimization/INCREMENTAL_RESCAN_INVALIDATION_EVIDENCE.toml",
        "docs/optimization/END_TO_END_INCREMENTAL_CORPUS_INDEX_TRANCHE_COVERAGE.toml",
    ] {
        let artifact = read_repo_file(relative);
        assert_contains(&artifact, "VX-1541..VX-1560");
        assert_contains(&artifact, "research_sources");
        assert_contains(&artifact, "owns");
        assert_contains(&artifact, "does_not_own");
    }
}

#[test]
fn watcher_policy_covers_backend_events_overflow_and_pressure() {
    let policy = read_repo_file("docs/optimization/FILESYSTEM_WATCHER_EVENT_POLICY.toml");

    for token in [
        "inotify",
        "fanotify",
        "rename_pairing",
        "overflow",
        "watch_limits",
        "BACKPRESSURE_QUEUE_QUOTA_POLICY",
    ] {
        assert_contains(&policy, token);
    }
}

#[test]
fn content_identity_index_separates_reuse_from_authentication_and_parser_caches() {
    let index = read_repo_file("docs/optimization/CORPUS_CONTENT_IDENTITY_INDEX.toml");

    for token in [
        "blake3_digest",
        "chunk_digest_tree",
        "rule_set_digest",
        "secret_boundary",
        "libs_parsing roadmap substrate",
        "foundation_optimizer roadmap substrate",
    ] {
        assert_contains(&index, token);
    }
}

#[test]
fn invalidation_evidence_requires_truth_gates_and_repair_metrics() {
    let evidence = read_repo_file("docs/optimization/INCREMENTAL_RESCAN_INVALIDATION_EVIDENCE.toml");

    for token in [
        "positive",
        "negative",
        "adversarial",
        "cross_file",
        "false_reuse_count",
        "overflow_repairs",
        "bytes_avoided",
    ] {
        assert_contains(&evidence, token);
    }
}

#[test]
fn acceleration_plan_contains_complete_incremental_corpus_index_tranche() {
    let plan = read_repo_file("docs/optimization/ALL_AXES_ACCELERATION_PLAN.md");

    for id in 1541..=1560 {
        assert_contains(&plan, &format!("VX-{id}"));
    }

    for token in [
        "INCREMENTAL_CORPUS_CHANGE_JOURNAL",
        "FILESYSTEM_WATCHER_EVENT_POLICY",
        "CORPUS_CONTENT_IDENTITY_INDEX",
        "INCREMENTAL_RESCAN_INVALIDATION_EVIDENCE",
        "END_TO_END_INCREMENTAL_CORPUS_INDEX_TRANCHE_COVERAGE",
    ] {
        assert_contains(&plan, token);
    }
}
