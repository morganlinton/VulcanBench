//! `csr_backward_traverse`  -  reverse BFS frontier step.
//!
//! Mirrors `super::csr_forward_traverse` but propagates along the
//! reverse edge direction: a destination in `frontier_in` lights up
//! every source that points at it. Used by dominator-tree
//! intersection and path_reconstruct frontier inversion.

use vyre_foundation::ir::Program;

use crate::graph::csr_frontier_step::{csr_frontier_step_program, CsrFrontierStepKind};
use crate::graph::program_graph::ProgramGraphShape;

/// Canonical op id.
pub const OP_ID: &str = "vyre-primitives::graph::csr_backward_traverse";

pub use crate::graph::csr_frontier_step::{
    csr_frontier_step_dispatch_grid as csr_backward_traverse_dispatch_grid, BINDING_FRONTIER_IN,
    BINDING_FRONTIER_OUT,
};

/// Build the IR `Program`. Each invocation owns one `src` and, if
/// any of its outgoing edges' destinations are set in `frontier_in`
/// AND the edge mask intersects `allow_mask`, sets `src`'s bit in
/// `frontier_out`.
#[must_use]
pub fn csr_backward_traverse(
    shape: ProgramGraphShape,
    frontier_in: &str,
    frontier_out: &str,
    allow_mask: u32,
) -> Program {
    csr_backward_traverse_with_op_id(OP_ID, shape, frontier_in, frontier_out, allow_mask)
}

/// Build the same reverse traversal kernel under a caller-owned op id.
///
/// Predicate wrappers use this to preserve their semantic operation identity
/// without forking the reverse CSR traversal body.
#[must_use]
pub(crate) fn csr_backward_traverse_with_op_id(
    op_id: &'static str,
    shape: ProgramGraphShape,
    frontier_in: &str,
    frontier_out: &str,
    allow_mask: u32,
) -> Program {
    csr_frontier_step_program(
        op_id,
        CsrFrontierStepKind::Backward,
        shape,
        frontier_in,
        frontier_out,
        allow_mask,
    )
}

/// CPU reference: one reverse step. Returns a bitset where bit `u`
/// is set iff there exists an edge `u → v` with `allow_mask`-matching
/// kind AND `v` is set in `frontier_in`.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn cpu_ref(
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    frontier_in: &[u32],
    allow_mask: u32,
) -> Vec<u32> {
    let mut out = Vec::new();
    cpu_ref_into(
        node_count,
        edge_offsets,
        edge_targets,
        edge_kind_mask,
        frontier_in,
        allow_mask,
        &mut out,
    );
    out
}

/// CPU reference using caller-owned output storage.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn cpu_ref_into(
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    frontier_in: &[u32],
    allow_mask: u32,
    out: &mut Vec<u32>,
) {
    let words = crate::graph::csr_forward_traverse::bitset_words(node_count) as usize;
    out.clear();
    out.resize(words, 0);
    crate::graph::csr_forward_traverse::validate_csr_frontier_step_cpu_inputs(
        "csr_backward_traverse",
        node_count,
        edge_offsets,
        edge_targets,
        edge_kind_mask,
    );
    for src in 0..node_count {
        let edge_start = edge_offsets[src as usize] as usize;
        let edge_end = edge_offsets[src as usize + 1] as usize;
        let mut hit = false;
        for e in edge_start..edge_end {
            let kind = edge_kind_mask[e];
            if (kind & allow_mask) == 0 {
                continue;
            }
            let dst = edge_targets[e];
            let dst_word = (dst / 32) as usize;
            let dst_bit = 1u32 << (dst % 32);
            if dst_word < frontier_in.len() && (frontier_in[dst_word] & dst_bit) != 0 {
                hit = true;
                break;
            }
        }
        if hit {
            let src_word = (src / 32) as usize;
            let src_bit = 1u32 << (src % 32);
            if src_word < out.len() {
                out[src_word] |= src_bit;
            }
        }
    }
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        OP_ID,
        || csr_backward_traverse(ProgramGraphShape::new(4, 4), "fin", "fout", 0xFFFF_FFFF),
        Some(|| {
            // Same graph as forward test. frontier_in = {3}; after
            // one reverse step, frontier_out = {1, 2} (both point at
            // 3).
            let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
            vec![vec![
                to_bytes(&[0, 0, 0, 0]),
                to_bytes(&[0, 2, 3, 4, 4]),
                to_bytes(&[1, 2, 3, 3]),
                to_bytes(&[1, 1, 1, 1]),
                to_bytes(&[0, 0, 0, 0]),
                to_bytes(&[0b1000]),
                to_bytes(&[0]),
            ]]
        }),
        Some(|| {
            let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
            vec![vec![to_bytes(&[0b0110])]]
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reverse_step_reaches_predecessors() {
        let got = cpu_ref(
            4,
            &[0, 2, 3, 4, 4],
            &[1, 2, 3, 3],
            &[1, 1, 1, 1],
            &[0b1000],
            0xFFFF_FFFF,
        );
        assert_eq!(got, vec![0b0110]);
    }

    #[test]
    fn cpu_ref_into_reuses_output_buffer_and_truncates_stale_words() {
        let mut out = Vec::with_capacity(4);
        out.extend_from_slice(&[u32::MAX, u32::MAX, u32::MAX]);
        let capacity = out.capacity();

        cpu_ref_into(
            4,
            &[0, 2, 3, 4, 4],
            &[1, 2, 3, 3],
            &[1, 1, 1, 1],
            &[0b1000],
            0xFFFF_FFFF,
            &mut out,
        );

        assert_eq!(out, vec![0b0110]);
        assert_eq!(out.capacity(), capacity);

        cpu_ref_into(0, &[0], &[], &[], &[], 0xFFFF_FFFF, &mut out);
        assert!(out.is_empty());
        assert_eq!(out.capacity(), capacity);
    }

    // ------------------------------------------------------------------
    // Adversarial fixtures  -  backward direction is undertested.
    // ------------------------------------------------------------------

    #[test]
    fn empty_graph_returns_empty() {
        let got = cpu_ref(0, &[0], &[], &[], &[], 0xFFFF_FFFF);
        assert!(got.is_empty());
    }

    #[test]
    fn single_node_no_edges_returns_empty() {
        let got = cpu_ref(1, &[0, 0], &[0], &[0], &[0b0001], 0xFFFF_FFFF);
        assert_eq!(got, vec![0]);
    }

    #[test]
    fn self_loops_only_predecessor_is_self() {
        // 2 nodes, each has self-loop. frontier {0} → out {0}
        let got = cpu_ref(2, &[0, 1, 2], &[0, 1], &[1, 1], &[0b0001], 0xFFFF_FFFF);
        assert_eq!(got, vec![0b0001], "self-loop: predecessor of 0 is 0");
    }

    #[test]
    fn disconnected_components_reverse_only_reach_within() {
        // Component A: 0→1. Component B: 2→3. frontier {3} → out {2}
        let got = cpu_ref(
            4,
            &[0, 1, 1, 2, 2],
            &[1, 3],
            &[1, 1],
            &[0b1000],
            0xFFFF_FFFF,
        );
        assert_eq!(got, vec![0b0100]);
    }

    #[test]
    fn max_node_count_cross_word_boundary_backward() {
        // 65 nodes (3 words), one edge from node 0 to node 64.
        let mut offsets = vec![0u32; 66];
        offsets[1..].fill(1);
        let mut frontier = vec![0u32; 3];
        frontier[2] = 1; // node 64
        let got = cpu_ref(65, &offsets, &[64], &[1], &frontier, 0xFFFF_FFFF);
        assert_eq!(got.len(), 3);
        assert_eq!(got[0], 1, "node 0 is predecessor of 64");
        assert_eq!(got[1], 0);
        assert_eq!(got[2], 0);
    }

    #[test]
    fn edge_mask_zero_blocks_all_backward() {
        let got = cpu_ref(
            4,
            &[0, 2, 3, 4, 4],
            &[1, 2, 3, 3],
            &[1, 1, 1, 1],
            &[0b1000],
            0,
        );
        assert_eq!(got, vec![0], "zero allow_mask must block every edge");
    }

    #[test]
    fn edge_mask_universal_backward() {
        let got = cpu_ref(
            4,
            &[0, 2, 3, 4, 4],
            &[1, 2, 3, 3],
            &[1, 1, 1, 1],
            &[0b1000],
            0xFFFF_FFFF,
        );
        assert_eq!(got, vec![0b0110]);
    }

    #[test]
    fn edge_kind_diversity_backward_ignoring_mask_would_fail() {
        // M8 regression: graph with two edge kinds.
        // 0→1 (DOMINANCE=0x01), 0→2 (ASSIGNMENT=0x02).
        // Backward from frontier {1} with mask DOMINANCE → out {0}.
        // Broken impl ignoring mask would see 0→2 (ASSIGNMENT) and not set 0,
        // producing an empty frontier.
        let got = cpu_ref(
            4,
            &[0, 2, 2, 2, 2],
            &[1, 2],
            &[0x01, 0x02],
            &[0b0010], // frontier = {1}
            0x01,      // only DOMINANCE
        );
        assert_eq!(
            got,
            vec![0b0001],
            "only node 0 reaches 1 via DOMINANCE; broken impl ignoring mask would produce 0"
        );
    }

    #[test]
    #[should_panic(expected = "node_count + 1 CSR offsets")]
    fn malformed_csr_short_offsets_fail_loudly() {
        let got = cpu_ref(4, &[0, 1], &[1], &[1], &[0b0001], 0xFFFF_FFFF);
        assert_eq!(got, vec![0]);
    }

    #[test]
    #[should_panic(expected = "complete CSR edge buffers")]
    fn malformed_csr_short_edge_buffers_fail_loudly() {
        let _ = cpu_ref(2, &[0, 2, 2], &[1], &[1], &[0b0010], 0xFFFF_FFFF);
    }

    #[test]
    #[should_panic(expected = "non-monotonic CSR offsets")]
    fn malformed_csr_non_monotonic_offsets_fail_loudly() {
        let _ = cpu_ref(2, &[0, 2, 1], &[1, 0], &[1, 1], &[0b0010], 0xFFFF_FFFF);
    }
}
