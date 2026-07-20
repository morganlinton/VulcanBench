//! Adversarial / property tests for `ProgramGraphShape` and
//! `csr_forward_traverse` CPU reference.
//!
//! Coverage:
//!   - zero-edge sentinel contract
//!   • malformed CSR buffer lengths
//!   • non-monotonic edge_offsets
//!   • out-of-bounds edge targets
//!   • edge mask filtering
//!   • high node counts / multi-word bitsets
//!   • edge_offsets last-entry vs edge_count mismatch

#![cfg(feature = "graph")]
#![allow(clippy::needless_range_loop)]

use vyre_primitives::graph::program_graph::{
    validate_program_graph, GraphValidationError, ProgramGraphShape,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn bitset_words(node_count: u32) -> usize {
    ((node_count + 31) / 32) as usize
}

fn zero_frontier(node_count: u32) -> Vec<u32> {
    vec![0u32; bitset_words(node_count)]
}

fn csr_cpu_ref(
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    frontier_in: &[u32],
    allow_mask: u32,
) -> Vec<u32> {
    let mut out = zero_frontier(node_count);
    let expected_offsets = node_count as usize + 1;
    assert_eq!(
        edge_offsets.len(),
        expected_offsets,
        "csr_forward_traverse CPU oracle received {} row offsets for node_count={node_count}; Fix: pass exactly node_count + 1 CSR offsets.",
        edge_offsets.len()
    );
    let edge_count = edge_offsets[expected_offsets - 1] as usize;
    assert!(
        edge_targets.len() >= edge_count && edge_kind_mask.len() >= edge_count,
        "csr_forward_traverse CPU oracle received edge_count={edge_count} but targets_len={} kind_mask_len={}. Fix: pass complete CSR edge buffers.",
        edge_targets.len(),
        edge_kind_mask.len()
    );
    for pair in edge_offsets.windows(2) {
        assert!(
            pair[0] <= pair[1],
            "csr_forward_traverse test oracle received non-monotonic CSR offsets"
        );
    }
    for src in 0..node_count {
        let src_word = (src / 32) as usize;
        let src_bit = 1u32 << (src % 32);
        if src_word >= frontier_in.len() || (frontier_in[src_word] & src_bit) == 0 {
            continue;
        }
        let edge_start = edge_offsets[src as usize];
        let edge_end = edge_offsets[src as usize + 1];
        for edge in edge_start as usize..edge_end as usize {
            let kind = edge_kind_mask[edge];
            if (kind & allow_mask) == 0 {
                continue;
            }
            let dst = edge_targets[edge];
            if dst >= node_count {
                continue;
            }
            let dst_word = (dst / 32) as usize;
            let dst_bit = 1u32 << (dst % 32);
            if let Some(slot) = out.get_mut(dst_word) {
                *slot |= dst_bit;
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Zero-edge sentinel contract
// ---------------------------------------------------------------------------

#[path = "adversarial_graph_csr_validation_contracts/zero_edge_contracts.rs"]
mod zero_edge_contracts;
#[path = "adversarial_graph_csr_validation_contracts/length_contracts.rs"]
mod length_contracts;
#[path = "adversarial_graph_csr_validation_contracts/offset_contracts.rs"]
mod offset_contracts;
#[path = "adversarial_graph_csr_validation_contracts/target_contracts.rs"]
mod target_contracts;
#[path = "adversarial_graph_csr_validation_contracts/mask_contracts.rs"]
mod mask_contracts;
#[path = "adversarial_graph_csr_validation_contracts/multiword_contracts.rs"]
mod multiword_contracts;
#[path = "adversarial_graph_csr_validation_contracts/edge_count_contracts.rs"]
mod edge_count_contracts;
#[path = "adversarial_graph_csr_validation_contracts/frontier_shape_contracts.rs"]
mod frontier_shape_contracts;
