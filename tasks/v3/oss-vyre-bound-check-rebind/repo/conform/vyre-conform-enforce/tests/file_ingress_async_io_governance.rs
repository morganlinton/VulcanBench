//! File ingress async io governance test suite.

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
fn file_ingress_async_io_sources_are_registered() {
    let ledger = read_repo_file("docs/optimization/RESEARCH_SOURCE_LEDGER.toml");

    for key in [
        "LINUX_IO_URING_SETUP",
        "LINUX_IO_URING_ENTER",
        "LINUX_IO_URING_REGISTER",
        "LINUX_OPENAT2",
        "LINUX_STATX",
        "LINUX_GETDENTS64",
        "LINUX_READV_PREADV2",
        "LINUX_OPEN_DIRECT_IO",
    ] {
        assert_contains(&ledger, key);
    }
}

#[test]
fn file_ingress_async_io_artifacts_define_clear_boundaries() {
    for relative in [
        "docs/optimization/FILE_INGRESS_ASYNC_IO_PIPELINE.toml",
        "docs/optimization/DIRECTORY_ENUMERATION_METADATA_POLICY.toml",
        "docs/optimization/SAFE_OPEN_HANDLE_READ_POLICY.toml",
        "docs/optimization/IO_PIPELINE_BACKPRESSURE_EVIDENCE.toml",
        "docs/optimization/END_TO_END_FILE_INGRESS_ASYNC_IO_TRANCHE_COVERAGE.toml",
    ] {
        let artifact = read_repo_file(relative);
        assert_contains(&artifact, "VX-1521..VX-1540");
        assert_contains(&artifact, "research_sources");
        assert_contains(&artifact, "owns");
        assert_contains(&artifact, "does_not_own");
    }
}

#[test]
fn safe_open_policy_links_handles_metadata_reads_and_direct_io() {
    let policy = read_repo_file("docs/optimization/SAFE_OPEN_HANDLE_READ_POLICY.toml");

    for token in [
        "dirfd-relative open admission",
        "resolve_constraints",
        "identity_recheck",
        "positioned reads",
        "direct I/O",
        "completion_drain",
    ] {
        assert_contains(&policy, token);
    }
}

#[test]
fn async_io_backpressure_connects_ingress_to_release_evidence() {
    let evidence = read_repo_file("docs/optimization/IO_PIPELINE_BACKPRESSURE_EVIDENCE.toml");

    for token in [
        "submission_ring_fill_ratio",
        "completion_ring_lag",
        "read_amplification_ratio",
        "buffered_downgrades",
        "BACKPRESSURE_QUEUE_QUOTA_POLICY",
        "STATISTICAL_PERFORMANCE_GATES",
    ] {
        assert_contains(&evidence, token);
    }
}

#[test]
fn acceleration_plan_contains_complete_file_ingress_tranche() {
    let plan = read_repo_file("docs/optimization/ALL_AXES_ACCELERATION_PLAN.md");

    for id in 1521..=1540 {
        assert_contains(&plan, &format!("VX-{id}"));
    }

    for token in [
        "FILE_INGRESS_ASYNC_IO_PIPELINE",
        "DIRECTORY_ENUMERATION_METADATA_POLICY",
        "SAFE_OPEN_HANDLE_READ_POLICY",
        "IO_PIPELINE_BACKPRESSURE_EVIDENCE",
        "END_TO_END_FILE_INGRESS_ASYNC_IO_TRANCHE_COVERAGE",
    ] {
        assert_contains(&plan, token);
    }
}
