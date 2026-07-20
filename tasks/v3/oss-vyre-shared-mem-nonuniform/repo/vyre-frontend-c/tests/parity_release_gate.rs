//! Contract tests for clang/vyrec parity release-gate decisions.

use vyre_frontend_c::api::{
    ParityConstructStatus, ParityFactCategory, ParityFinding, ParityFindingKind,
    ParityGpuResidencyProof, ParityPerformanceProof, ParityPerformanceProofError,
    ParityReleaseReport, ParityUnsupportedConstruct,
};

#[test]
fn release_gate_rejects_empty_report() {
    let report = report();

    assert!(
        !report.is_release_ready(),
        "an empty report is not proof of clang parity"
    );
    assert!(report.blocking_findings().is_empty());
}

#[test]
fn release_gate_accepts_matches_and_approved_target_differences() {
    let mut report = report();
    report.push_match(
        ParityFactCategory::Lexer,
        "token:lib/math/gcd.c:1",
        "token kind, spelling, and span match clang",
    );
    report.push_explained_difference(
        ParityFactCategory::ObjectEvidence,
        "object:metadata:producer",
        "vyrec object producer string is expected to differ from clang",
    );

    assert!(report.is_release_ready());
    assert!(report.blocking_findings().is_empty());
}

#[test]
fn release_gate_rejects_every_unexplained_mismatch_class() {
    for kind in [
        ParityFindingKind::VyrecMissing,
        ParityFindingKind::VyrecExtra,
        ParityFindingKind::SpanMismatch,
        ParityFindingKind::SemanticMismatch,
        ParityFindingKind::DiagnosticMismatch,
        ParityFindingKind::PerformanceFailure,
        ParityFindingKind::GpuResidencyFailure,
    ] {
        let mut report = report();
        report.push_match(
            ParityFactCategory::Preprocessor,
            "macro:baseline",
            "baseline fact matches",
        );
        report.push_finding(ParityFinding::new(
            category_for(kind),
            kind,
            "release:blocker",
            "release-blocking parity failure",
        ));

        let blocking = report.blocking_findings();
        assert!(!report.is_release_ready(), "{kind:?} must block release");
        assert_eq!(blocking.len(), 1);
        assert_eq!(blocking[0].kind, kind);
    }
}

#[test]
fn performance_proof_must_meet_required_speedup() {
    let proof = ParityPerformanceProof::new(2_000_000, 10_000, 100_000)
        .expect("nonzero timings define a valid speedup proof");

    assert_eq!(proof.measured_speedup_x1000(), 200_000);
    assert!(proof.passes_contract());

    let mut report = report();
    report.push_match(
        ParityFactCategory::SemanticAnalysis,
        "semantic:baseline",
        "semantic facts match",
    );
    report.push_performance_proof("perf:linux-lib-math-v6.8", proof);

    assert!(report.is_release_ready());
    assert!(report.blocking_findings().is_empty());
}

#[test]
fn performance_proof_failure_blocks_release() {
    let proof = ParityPerformanceProof::new(500_000, 10_000, 100_000)
        .expect("nonzero timings define a valid speedup proof");

    assert_eq!(proof.measured_speedup_x1000(), 50_000);
    assert!(!proof.passes_contract());

    let mut report = report();
    report.push_match(
        ParityFactCategory::SemanticAnalysis,
        "semantic:baseline",
        "semantic facts match",
    );
    report.push_performance_proof("perf:linux-lib-math-v6.8", proof);

    let blocking = report.blocking_findings();
    assert!(!report.is_release_ready());
    assert_eq!(blocking.len(), 1);
    assert_eq!(blocking[0].kind, ParityFindingKind::PerformanceFailure);
}

#[test]
fn performance_proof_rejects_zero_inputs() {
    assert_eq!(
        ParityPerformanceProof::new(0, 1, 1),
        Err(ParityPerformanceProofError::ZeroClangWallTime)
    );
    assert_eq!(
        ParityPerformanceProof::new(1, 0, 1),
        Err(ParityPerformanceProofError::ZeroVyrecWallTime)
    );
    assert_eq!(
        ParityPerformanceProof::new(1, 1, 0),
        Err(ParityPerformanceProofError::ZeroRequiredSpeedup)
    );
}

#[test]
fn gpu_residency_proof_records_launches_transfers_and_syncs() {
    let proof = ParityGpuResidencyProof::new("NVIDIA GeForce RTX 5090", "570.211.01")
        .with_kernel_launch_count(42)
        .with_host_write_bytes(1_024)
        .with_host_readback_bytes(2_048)
        .with_host_sync_points(3)
        .with_device_allocation_bytes(4_096)
        .with_gpu_occupancy_evidence("occupancy-sample-present")
        .with_memory_pressure_bytes(8_192);

    assert!(proof.passes_contract());

    let mut report = report();
    report.push_match(
        ParityFactCategory::SemanticAnalysis,
        "semantic:baseline",
        "semantic facts match",
    );
    report.push_gpu_residency_proof("gpu:linux-lib-math-v6.8", proof);

    assert!(report.is_release_ready());
    assert!(report.blocking_findings().is_empty());
    assert!(
        report.findings()[1].detail.contains("launches=42"),
        "GPU proof must carry launch count evidence"
    );
    assert!(
        report.findings()[1]
            .detail
            .contains("host_readback_bytes=2048"),
        "GPU proof must carry readback evidence"
    );
    assert!(
        report.findings()[1].detail.contains("host_sync_points=3"),
        "GPU proof must carry synchronization evidence"
    );
    assert!(
        report.findings()[1]
            .detail
            .contains("occupancy=occupancy-sample-present"),
        "GPU proof must carry occupancy evidence"
    );
    assert!(
        report.findings()[1]
            .detail
            .contains("memory_pressure_bytes=8192"),
        "GPU proof must carry memory-pressure evidence"
    );
}

#[test]
fn gpu_residency_proof_rejects_host_escape_events_and_false_no_gpu_skips() {
    for (proof, expected_failure) in [
        (
            ParityGpuResidencyProof::new("NVIDIA GeForce RTX 5090", "570.211.01")
                .with_gpu_occupancy_evidence("occupancy-sample-present")
                .with_production_host_escape_events(1),
            "production host-reference escape",
        ),
        (
            ParityGpuResidencyProof::new("NVIDIA GeForce RTX 5090", "570.211.01")
                .with_gpu_occupancy_evidence("occupancy-sample-present")
                .with_false_no_gpu_skips(1),
            "false no-GPU skip",
        ),
        (
            ParityGpuResidencyProof::new("", "570.211.01")
                .with_gpu_occupancy_evidence("occupancy-sample-present"),
            "missing GPU name",
        ),
        (
            ParityGpuResidencyProof::new("NVIDIA GeForce RTX 5090", "")
                .with_gpu_occupancy_evidence("occupancy-sample-present"),
            "missing GPU driver",
        ),
        (
            ParityGpuResidencyProof::new("NVIDIA GeForce RTX 5090", "570.211.01"),
            "missing GPU occupancy evidence",
        ),
    ] {
        let failures = proof.contract_failures();
        assert!(
            failures
                .iter()
                .any(|failure| failure.contains(expected_failure)),
            "GPU residency proof should explain blocker `{expected_failure}`, got {failures:?}"
        );
        let mut report = report();
        report.push_match(
            ParityFactCategory::SemanticAnalysis,
            "semantic:baseline",
            "semantic facts match",
        );
        report.push_gpu_residency_proof("gpu:linux-lib-math-v6.8", proof);

        let blocking = report.blocking_findings();
        assert!(!report.is_release_ready());
        assert_eq!(blocking.len(), 1);
        assert_eq!(blocking[0].kind, ParityFindingKind::GpuResidencyFailure);
        assert!(
            blocking[0].detail.contains(expected_failure),
            "GPU residency finding must include actionable failure detail"
        );
    }
}

#[test]
fn unresolved_construct_blocks_release_until_implemented_or_approved() {
    for status in [
        ParityConstructStatus::Implemented,
        ParityConstructStatus::ApprovedOutOfScope,
    ] {
        let mut report = report();
        report.push_unsupported_construct(ParityUnsupportedConstruct::new(
            ParityFactCategory::Parser,
            "gnu_statement_expression",
            "lib/math/example.c:10:5",
            status,
            "construct handled for this release status",
        ));

        assert!(
            report.is_release_ready(),
            "{status:?} construct status must not block release"
        );
        assert!(report.blocking_findings().is_empty());
    }

    let mut report = report();
    report.push_unsupported_construct(ParityUnsupportedConstruct::new(
        ParityFactCategory::Parser,
        "asm_goto",
        "lib/math/example.c:12:1",
        ParityConstructStatus::Unresolved,
        "no parser or approved out-of-scope record exists",
    ));

    let blocking = report.blocking_findings();
    assert!(!report.is_release_ready());
    assert_eq!(blocking.len(), 1);
    assert_eq!(blocking[0].kind, ParityFindingKind::VyrecMissing);
}

#[test]
fn dashboard_reports_parity_performance_and_gpu_residency_evidence() {
    let mut report = report();
    report.push_match(
        ParityFactCategory::Lexer,
        "token:baseline",
        "token facts match",
    );
    report.push_explained_difference(
        ParityFactCategory::ObjectEvidence,
        "object:producer",
        "producer metadata is allowed to differ",
    );
    report.push_performance_proof(
        "perf:linux-lib-math-v6.8",
        ParityPerformanceProof::new(2_000_000, 10_000, 100_000)
            .expect("nonzero timings define a valid speedup proof"),
    );
    report.push_gpu_residency_proof(
        "gpu:linux-lib-math-v6.8",
        ParityGpuResidencyProof::new("NVIDIA GeForce RTX 5090", "570.211.01")
            .with_kernel_launch_count(7)
            .with_host_write_bytes(11)
            .with_host_readback_bytes(13)
            .with_host_sync_points(17)
            .with_device_allocation_bytes(19)
            .with_gpu_occupancy_evidence("occupancy-sample-present")
            .with_memory_pressure_bytes(23),
    );

    let dashboard = report.dashboard();
    assert!(dashboard.release_ready);
    assert_eq!(dashboard.target_id, "linux-lib-math-v6.8");
    assert_eq!(
        dashboard.source_commit,
        "90d1f30371ae3337beb01666b226320728d35c70"
    );
    assert_eq!(dashboard.total_findings, 4);
    assert_eq!(dashboard.blocking_findings, 0);
    assert_eq!(dashboard.matching_findings, 3);
    assert_eq!(dashboard.explained_differences, 1);
    assert_eq!(dashboard.performance_proof_count, 1);
    assert_eq!(dashboard.best_measured_speedup_x1000, Some(200_000));
    assert_eq!(dashboard.gpu_residency_proof_count, 1);
    assert_eq!(dashboard.total_kernel_launch_count, 7);
    assert_eq!(dashboard.total_host_write_bytes, 11);
    assert_eq!(dashboard.total_host_readback_bytes, 13);
    assert_eq!(dashboard.total_host_sync_points, 17);
    assert_eq!(dashboard.total_device_allocation_bytes, 19);
    assert_eq!(dashboard.gpu_occupancy_evidence_count, 1);
    assert_eq!(dashboard.total_memory_pressure_bytes, 23);
}

fn report() -> ParityReleaseReport {
    ParityReleaseReport::new(
        "linux-lib-math-v6.8",
        "90d1f30371ae3337beb01666b226320728d35c70",
        "clang-oracle",
        "vyrec-under-test",
        "NVIDIA GeForce RTX 5090",
        "resident-graph",
    )
}

fn category_for(kind: ParityFindingKind) -> ParityFactCategory {
    match kind {
        ParityFindingKind::PerformanceFailure => ParityFactCategory::Performance,
        ParityFindingKind::GpuResidencyFailure => ParityFactCategory::GpuResidency,
        ParityFindingKind::DiagnosticMismatch => ParityFactCategory::SemanticAnalysis,
        ParityFindingKind::SemanticMismatch => ParityFactCategory::SemanticAnalysis,
        ParityFindingKind::SpanMismatch => ParityFactCategory::Parser,
        ParityFindingKind::VyrecExtra => ParityFactCategory::ObjectEvidence,
        ParityFindingKind::VyrecMissing => ParityFactCategory::AbiLayout,
        ParityFindingKind::Match | ParityFindingKind::ExplainedTargetDifference => {
            ParityFactCategory::Lexer
        }
    }
}
