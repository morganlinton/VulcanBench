//! Tests for include-graph GPU residency evidence.

use vyre_frontend_c::api::{IncludeGraphEdge, IncludeGraphProof, IncludeGraphResidency};
use vyre_libs::parsing::c::preprocess::gpu_pipeline::{
    IncludeEvent, IncludeEventResidency, PreprocessedSource,
};

#[test]
fn include_graph_proof_allows_host_metadata_but_requires_gpu_production_edges() {
    let mut proof = IncludeGraphProof::new("lib/math/gcd.c");
    proof.push_edge(IncludeGraphEdge::new(
        "lib/math/gcd.c",
        "linux/gcd.h",
        0,
        true,
        IncludeGraphResidency::GpuResident,
    ));
    proof.push_edge(IncludeGraphEdge::new(
        "linux/gcd.h",
        "/linux/include/linux/gcd.h",
        0,
        true,
        IncludeGraphResidency::HostFilesystemMetadata,
    ));

    assert!(proof.production_edges_are_gpu_resident());
    assert!(proof.unresolved_edges().is_empty());
}

#[test]
fn include_graph_proof_rejects_cpu_oracle_edges_as_production_evidence() {
    let mut proof = IncludeGraphProof::new("lib/math/gcd.c");
    proof.push_edge(IncludeGraphEdge::new(
        "lib/math/gcd.c",
        "linux/gcd.h",
        0,
        true,
        IncludeGraphResidency::CpuOracle,
    ));

    assert!(!proof.production_edges_are_gpu_resident());
}

#[test]
fn include_graph_proof_reports_unresolved_edges() {
    let mut proof = IncludeGraphProof::new("lib/math/gcd.c");
    proof.push_edge(IncludeGraphEdge::new(
        "lib/math/gcd.c",
        "missing.h",
        17,
        false,
        IncludeGraphResidency::GpuResident,
    ));

    let unresolved = proof.unresolved_edges();
    assert_eq!(unresolved.len(), 1);
    assert_eq!(unresolved[0].includee, "missing.h");
    assert_eq!(unresolved[0].directive_byte_offset, 17);
}

#[test]
fn include_graph_proof_converts_gpu_preprocessor_events() {
    let source = PreprocessedSource {
        bytes: b"int from_header;\n".to_vec(),
        macros: Vec::new(),
        include_byte_cache_stats: Default::default(),
        include_events: vec![IncludeEvent {
            includer: "lib/math/gcd.c".into(),
            requested_path: b"linux/gcd.h".to_vec(),
            resolved_path: "/linux/include/linux/gcd.h".into(),
            directive_row: 0,
            directive_byte_offset: 12,
            is_system: true,
            is_next: false,
            request_residency: IncludeEventResidency::GpuResidentRequest,
            resolution_residency: IncludeEventResidency::HostFilesystemMetadata,
        }],
        conditional_events: Vec::new(),
        macro_events: Vec::new(),
        macro_expansion_events: Vec::new(),
        token_provenance_events: Vec::new(),
        include_acceleration_events: Vec::new(),
        header_reuse_events: Vec::new(),
    };

    let proof = IncludeGraphProof::from_gpu_preprocessed_source("lib/math/gcd.c", &source);

    assert_eq!(proof.edges.len(), 2);
    assert_eq!(proof.edges[0].includer, "lib/math/gcd.c");
    assert_eq!(proof.edges[0].includee, "/linux/include/linux/gcd.h");
    assert_eq!(proof.edges[0].directive_byte_offset, 12);
    assert_eq!(proof.edges[0].residency, IncludeGraphResidency::GpuResident);
    assert_eq!(
        proof.edges[1].residency,
        IncludeGraphResidency::HostFilesystemMetadata
    );
    assert!(proof.production_edges_are_gpu_resident());
}

#[test]
fn include_graph_proof_preserves_host_memory_cache_resolution() {
    let source = PreprocessedSource {
        bytes: b"int from_header;\n".to_vec(),
        macros: Vec::new(),
        include_byte_cache_stats: Default::default(),
        include_events: vec![IncludeEvent {
            includer: "lib/math/gcd.c".into(),
            requested_path: b"linux/gcd.h".to_vec(),
            resolved_path: "/linux/include/linux/gcd.h".into(),
            directive_row: 0,
            directive_byte_offset: 12,
            is_system: true,
            is_next: false,
            request_residency: IncludeEventResidency::GpuResidentRequest,
            resolution_residency: IncludeEventResidency::HostMemoryCache,
        }],
        conditional_events: Vec::new(),
        macro_events: Vec::new(),
        macro_expansion_events: Vec::new(),
        token_provenance_events: Vec::new(),
        include_acceleration_events: Vec::new(),
        header_reuse_events: Vec::new(),
    };

    let proof = IncludeGraphProof::from_gpu_preprocessed_source("lib/math/gcd.c", &source);

    assert_eq!(proof.edges.len(), 2);
    assert_eq!(proof.edges[0].residency, IncludeGraphResidency::GpuResident);
    assert_eq!(
        proof.edges[1].residency,
        IncludeGraphResidency::HostMemoryCache
    );
    assert!(proof.production_edges_are_gpu_resident());
}

#[test]
fn include_graph_proof_rejects_host_memory_cache_request_residency() {
    let source = PreprocessedSource {
        bytes: b"int from_header;\n".to_vec(),
        macros: Vec::new(),
        include_byte_cache_stats: Default::default(),
        include_events: vec![IncludeEvent {
            includer: "lib/math/gcd.c".into(),
            requested_path: b"linux/gcd.h".to_vec(),
            resolved_path: "/linux/include/linux/gcd.h".into(),
            directive_row: 0,
            directive_byte_offset: 12,
            is_system: true,
            is_next: false,
            request_residency: IncludeEventResidency::HostMemoryCache,
            resolution_residency: IncludeEventResidency::HostFilesystemMetadata,
        }],
        conditional_events: Vec::new(),
        macro_events: Vec::new(),
        macro_expansion_events: Vec::new(),
        token_provenance_events: Vec::new(),
        include_acceleration_events: Vec::new(),
        header_reuse_events: Vec::new(),
    };

    let proof = IncludeGraphProof::from_gpu_preprocessed_source("lib/math/gcd.c", &source);

    assert_eq!(proof.edges.len(), 2);
    assert_eq!(proof.edges[0].residency, IncludeGraphResidency::CpuOracle);
    assert!(!proof.production_edges_are_gpu_resident());
}
