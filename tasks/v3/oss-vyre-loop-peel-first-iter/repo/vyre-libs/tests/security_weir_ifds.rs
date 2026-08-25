//! Security-to-Weir IFDS routing contract tests.
//!
//! Gated by the same `weir_ifds_external_engine` feature as the code it
//! exercises (`vyre_libs::security::weir_ifds`, see security/mod.rs). That
//! module `use`s the external `external_dataflow_engine` crate, which is not
//! wired into this workspace, so both the module and these tests stay disabled
//! until that integration lands. Gating on the broader `security` feature was
//! wrong: `security` is enabled (e.g. by xtask) while the weir_ifds external
//! engine is absent, so the test failed to resolve the missing crate.

#![cfg(feature = "weir_ifds_external_engine")]

use std::collections::BTreeMap;

use external_dataflow_engine::ifds_gpu::{ifds_gpu_step, IfdsShape, OP_ID as WEIR_IFDS_GPU_OP_ID};
use external_dataflow_engine::reachability_witness::{ExtractedPath, ExtractedStatement};
use vyre_primitives::predicate::edge_kind;
use vyre_libs::{
    dataflow::{DynamicPrimitiveSoundness, PrecisionContract, Soundness},
    security::{
        route_security_taint_through_weir_ifds, security_witness_path_from_weir, AnalysisFact, AnalysisFactTable,
        AnalysisSourceSpan, FactId, FactKind, WeirIfdsSecurityBuffers, WEIR_IFDS_SECURITY_BACKEND_ID,
    },
};
use vyre_libs::security::facts::{
    FindingProofBundle, FindingProofStep, SourceToSinkFindingRequest,
};

#[test]
fn source_to_sink_query_dispatches_through_weir_ifds_and_returns_witness_seed() {
    let table = AnalysisFactTable::new(vec![fact(1, FactKind::Source, 1), fact(2, FactKind::Sink, 3)]);
    let shape = IfdsShape::new(1, 4, 1, 3);
    let buffers = WeirIfdsSecurityBuffers::new(
        "pg_edge_offsets",
        "pg_edge_targets",
        "pg_edge_kind_mask",
        "pg_node_tags",
        "fact_ids",
        "fact_kinds",
        "fact_subjects",
        "fact_objects",
        "ifds_frontier_in",
        "ifds_frontier_out",
    );
    let request = SourceToSinkFindingRequest {
        finding_id: "finding.security.weir-ifds".to_string(),
        query_id: "security.source_to_sink".to_string(),
        backend_id: "planner".to_string(),
        evidence_digest: "evidence:weir-ifds".to_string(),
        precision_contract: vyre_libs::dataflow::PrecisionContract::ZeroFalsePositive,
        source_fact_id: FactId(1),
        sink_fact_id: FactId(2),
        path_fact_ids: Vec::new(),
        sanitizer_fact_ids: Vec::new(),
        query_hit: 1,
        confidence_bps: 10_000,
        reason: "route through Weir IFDS".to_string(),
    };

    let dispatch = route_security_taint_through_weir_ifds(&table, &request, shape, buffers)
        .expect("Fix: fact-backed source-to-sink query should route through Weir IFDS");

    assert_eq!(dispatch.query_id, WEIR_IFDS_GPU_OP_ID);
    assert_eq!(dispatch.backend_id, WEIR_IFDS_SECURITY_BACKEND_ID);
    assert_eq!(dispatch.node_count, 4);
    assert_eq!(dispatch.source_fact_id, FactId(1));
    assert_eq!(dispatch.sink_fact_id, FactId(2));
    assert_eq!(dispatch.witness_seeds.len(), 1);
    assert_eq!(dispatch.witness_seeds[0].source_file, "src/security.c");
    assert_eq!(dispatch.witness_seeds[0].source_node, 1);
    assert_eq!(dispatch.witness_seeds[0].sink_file, "src/security.c");
    assert_eq!(dispatch.witness_seeds[0].sink_node, 3);
    assert_eq!(dispatch.primitive_soundness.len(), 1);
    assert_eq!(dispatch.primitive_soundness[0].op_id, WEIR_IFDS_GPU_OP_ID);
    assert_eq!(dispatch.primitive_soundness[0].soundness, Soundness::Exact);

    let routed = dispatch
        .step_program()
        .expect("Fix: routed Weir IFDS dispatch should build a Program")
        .fingerprint();
    let direct = ifds_gpu_step(shape, "ifds_frontier_in", "ifds_frontier_out")
        .expect("Fix: direct Weir IFDS dispatch should build a Program")
        .fingerprint();
    assert_eq!(
        routed, direct,
        "Fix: security routing must call the same Weir IFDS step builder"
    );
}

#[test]
fn weir_witness_path_attaches_rule_spans_edge_kinds_soundness_and_source_bytes() {
    let source = b"let a = recv();\nlet b = a;\nsink(b);\n".to_vec();
    let mut source_files = BTreeMap::new();
    source_files.insert("src/security.c".to_string(), source);
    let bundle = FindingProofBundle {
        finding_id: "finding.security.weir-ifds".to_string(),
        query_id: WEIR_IFDS_GPU_OP_ID.to_string(),
        backend_id: WEIR_IFDS_SECURITY_BACKEND_ID.to_string(),
        evidence_digest: "evidence:weir-path".to_string(),
        precision_contract: PrecisionContract::ZeroFalsePositive,
        soundness: Soundness::Exact,
        primitive_soundness: vec![DynamicPrimitiveSoundness::new(
            WEIR_IFDS_GPU_OP_ID,
            Soundness::Exact,
        )],
        fact_ids: vec![FactId(1), FactId(2)],
        proof_path: vec![
            FindingProofStep::new(FactId(1), AnalysisSourceSpan::byte_range(7, 0, 15), "source"),
            FindingProofStep::new(FactId(2), AnalysisSourceSpan::byte_range(7, 27, 35), "sink"),
        ],
        confidence_bps: 10_000,
        reason: "Weir path proves source reaches sink".to_string(),
    };
    let path = ExtractedPath {
        statements: vec![
            statement("call recv", 1, 0, 15),
            statement("assign b", 2, 16, 26),
            statement("call sink", 3, 27, 35),
        ],
    };
    let edge_kinds = [edge_kind::ASSIGNMENT, edge_kind::CALL_ARG];

    let witness = security_witness_path_from_weir(
        &bundle,
        "c.weir.source-to-sink",
        &path,
        &edge_kinds,
        &source_files,
    )
    .expect("Fix: Weir extracted path should attach to fact-backed finding");

    assert_eq!(witness.finding_id, "finding.security.weir-ifds");
    assert_eq!(witness.rule_id, "c.weir.source-to-sink");
    assert_eq!(witness.query_id, WEIR_IFDS_GPU_OP_ID);
    assert_eq!(witness.backend_id, WEIR_IFDS_SECURITY_BACKEND_ID);
    assert_eq!(witness.soundness, Soundness::Exact);
    assert_eq!(witness.source_span, AnalysisSourceSpan::byte_range(7, 0, 15));
    assert_eq!(witness.sink_span, AnalysisSourceSpan::byte_range(7, 27, 35));
    assert_eq!(witness.edge_kinds, edge_kinds);
    assert_eq!(witness.statements.len(), 3);
    assert_eq!(witness.statements[0].incoming_edge_kind, None);
    assert_eq!(
        witness.statements[1].incoming_edge_kind,
        Some(edge_kind::ASSIGNMENT)
    );
    assert_eq!(
        witness.statements[2].incoming_edge_kind,
        Some(edge_kind::CALL_ARG)
    );
    assert_eq!(witness.statements[0].source_bytes, b"let a = recv();");
    assert_eq!(witness.statements[1].source_bytes, b"let b = a;");
    assert_eq!(witness.statements[2].source_bytes, b"sink(b);");
}

fn fact(id: u64, kind: FactKind, subject: u64) -> AnalysisFact {
    let mut fact = AnalysisFact::exact(
        FactId(id),
        kind,
        AnalysisSourceSpan::byte_range(7, subject as u32 * 10, subject as u32 * 10 + 3),
        subject,
    );
    fact.payload
        .insert("file".to_string(), "src/security.c".to_string());
    fact
}

fn statement(description: &str, node_id: u32, byte_start: u32, byte_end: u32) -> ExtractedStatement {
    ExtractedStatement {
        adapter: "c-c11".to_string(),
        description: description.to_string(),
        file: "src/security.c".to_string(),
        node_id,
        byte_start,
        byte_end,
    }
}
