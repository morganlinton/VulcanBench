//! Failure-oriented adversarial tests for graph primitives.
//!
//! Focus: hostile boundaries, overflow, invalid offsets, property invariants.
#![cfg(feature = "graph")]

use vyre_primitives::graph::program_graph::*;

fn cpu_ref(
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kinds: &[u32],
    frontier: &[u32],
    edge_kind_mask: u32,
) -> Vec<u32> {
    assert_eq!(
        edge_offsets.len(),
        node_count as usize + 1,
        "node_count + 1 CSR offsets required"
    );
    let edge_count = edge_offsets.last().copied().unwrap_or(0) as usize;
    assert!(
        edge_targets.len() >= edge_count && edge_kinds.len() >= edge_count,
        "complete CSR edge buffers required"
    );
    let mut out = vec![0u32; node_count.div_ceil(32) as usize];
    for src in 0..node_count as usize {
        if frontier
            .get(src / 32)
            .is_none_or(|word| (word & (1u32 << (src % 32))) == 0)
        {
            continue;
        }
        let start = edge_offsets[src] as usize;
        let end = edge_offsets[src + 1] as usize;
        assert!(start <= end, "CSR offsets must be monotonic");
        for edge in start..end {
            if edge_kinds[edge] & edge_kind_mask == 0 {
                continue;
            }
            let dst = edge_targets[edge] as usize;
            if let Some(word) = out.get_mut(dst / 32) {
                *word |= 1u32 << (dst % 32);
            }
        }
    }
    out
}

#[test]
fn validate_program_graph_empty() {
    let shape = ProgramGraphShape::new(0, 0);
    let result = validate_program_graph(shape, &[], &[0], &[0], &[0], &[]);
    assert_eq!(
        result,
        Ok(()),
        "empty graph with sentinel edges should validate"
    );
}

#[test]
fn validate_program_graph_mismatched_nodes_len() {
    let shape = ProgramGraphShape::new(3, 2);
    let err = validate_program_graph(shape, &[0, 0], &[0, 1, 2, 2], &[1, 2], &[1, 1], &[0, 0, 0])
        .unwrap_err();
    assert!(matches!(err, GraphValidationError::NodesLen { .. }));
}

#[test]
fn validate_program_graph_non_monotonic_offsets() {
    let shape = ProgramGraphShape::new(2, 1);
    let err = validate_program_graph(shape, &[0, 0], &[0, 2, 1], &[0], &[0], &[0, 0]).unwrap_err();
    assert!(matches!(
        err,
        GraphValidationError::NonMonotonicOffsets { .. }
    ));
}

#[test]
fn validate_program_graph_oob_edge_target() {
    let shape = ProgramGraphShape::new(2, 1);
    let err = validate_program_graph(shape, &[0, 0], &[0, 1, 1], &[5], &[0], &[0, 0]).unwrap_err();
    assert!(matches!(
        err,
        GraphValidationError::EdgeOutOfRange { target: 5, .. }
    ));
}

#[test]
fn validate_program_graph_first_offset_nonzero() {
    let shape = ProgramGraphShape::new(2, 1);
    let err = validate_program_graph(shape, &[0, 0], &[1, 1, 1], &[0], &[0], &[0, 0]).unwrap_err();
    assert!(matches!(
        err,
        GraphValidationError::NonMonotonicOffsets { index: 0 }
    ));
}

#[test]
fn validate_program_graph_u32_max_node_count() {
    let shape = ProgramGraphShape::new(u32::MAX, 0);
    let nodes: &[u32] = &[];
    let edge_offsets = &[0u32];
    let err = validate_program_graph(shape, nodes, edge_offsets, &[0], &[0], nodes).unwrap_err();
    assert!(
        matches!(
            err,
            GraphValidationError::NodesLen { .. } | GraphValidationError::EdgeOffsetsLen { .. }
        ),
        "got unexpected error variant: {err:?}"
    );
}

#[test]
fn csr_forward_traverse_empty_frontier() {
    let got = cpu_ref(
        4,
        &[0, 2, 3, 4, 4],
        &[1, 2, 3, 3],
        &[1, 1, 1, 1],
        &[0],
        0xFFFF_FFFF,
    );
    assert_eq!(got, vec![0]);
}

#[test]
fn csr_forward_traverse_zero_nodes() {
    let got = cpu_ref(0, &[0], &[], &[], &[], 0xFFFF_FFFF);
    assert_eq!(got, vec![]);
}

#[test]
#[should_panic(expected = "node_count + 1 CSR offsets")]
fn csr_forward_traverse_malformed_csr_fails_loudly() {
    let _ = cpu_ref(2, &[0], &[1], &[1], &[0b11], 0xFFFF_FFFF);
}

#[test]
fn csr_forward_traverse_edge_mask_filters() {
    let got = cpu_ref(
        4,
        &[0, 2, 3, 4, 4],
        &[1, 2, 3, 3],
        &[0b10, 0b01, 0b01, 0b01],
        &[0b0001],
        0b01,
    );
    assert_eq!(got, vec![0b0100]);
}

#[test]
fn csr_forward_traverse_oob_target_silently_dropped() {
    // cpu_ref does not panic on OOB targets; it bounds-checks via dst_word < out.len()
    let got = cpu_ref(2, &[0, 2, 2], &[1, 100], &[1, 1], &[0b0001], 0xFFFF_FFFF);
    // Only node 1 is valid; node 100 is dropped
    assert_eq!(got, vec![0b0010]);
}
