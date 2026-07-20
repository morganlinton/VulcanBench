//! `csr_forward_traverse`  -  one BFS frontier step over a
//! `super::program_graph::ProgramGraph`.
//!
//! Given an input frontier bitset (`frontier_in`) and a per-edge
//! allow-mask, the primitive emits the next frontier: every node
//! that has at least one predecessor in `frontier_in` reached via
//! an edge whose `edge_kind_mask` intersects the allowed mask.
//!
//! One dispatch is one step. Transitive closure is driven by
//! composing with `super::super::bitset` primitives and
//! `super::super::fixpoint::bitset_fixpoint`.
//!
//! CPU reference + witness ship alongside so the conform harness
//! can exercise the primitive end-to-end without GPU hardware.

use vyre_foundation::ir::Program;

use crate::graph::csr_frontier_step::{csr_frontier_step_program, CsrFrontierStepKind};
use crate::graph::program_graph::ProgramGraphShape;

/// Canonical op id.
pub const OP_ID: &str = "vyre-primitives::graph::csr_forward_traverse";

pub use crate::graph::csr_frontier_step::{
    csr_frontier_step_dispatch_grid as csr_forward_traverse_dispatch_grid, BINDING_FRONTIER_IN,
    BINDING_FRONTIER_OUT,
};

/// Number of u32 words needed to hold a bitset over `node_count`
/// nodes (one bit per node, packed 32-per-word, rounded up).
///
/// Delegates to `crate::bitset::bitset_words` so CSR traversal and
/// bitset primitives share one overflow-safe sizing rule.
#[must_use]
pub const fn bitset_words(node_count: u32) -> u32 {
    crate::bitset::bitset_words(node_count)
}

/// Build the IR `Program` for one BFS forward step.
///
/// Each invocation owns one source node `src`. For each outgoing edge
/// whose `edge_kind_mask` intersects `allow_mask`, the program computes
/// `dst = edge_targets[e]` and atomically ORs the destination bit into
/// `frontier_out`. Transitive closure is driven by composing this step
/// with `bitset_fixpoint`.
///
/// Backward-edge iteration would be cheap given a CSC side-car; for
/// forward-only CSR, the atomic-OR path keeps the primitive
/// substrate-neutral without requiring two index layouts.
///
/// `dst` is bounds-checked against `shape.node_count` before
/// `atomic_or` so malformed edge lists cannot write outside the
/// node-indexed `frontier_out` bitset.
#[must_use]
pub fn csr_forward_traverse(
    shape: ProgramGraphShape,
    frontier_in: &str,
    frontier_out: &str,
    allow_mask: u32,
) -> Program {
    csr_forward_traverse_with_op_id(OP_ID, shape, frontier_in, frontier_out, allow_mask)
}

/// Build a CSR forward step under a caller-owned op id.
#[must_use]
pub(crate) fn csr_forward_traverse_with_op_id(
    op_id: &'static str,
    shape: ProgramGraphShape,
    frontier_in: &str,
    frontier_out: &str,
    allow_mask: u32,
) -> Program {
    csr_frontier_step_program(
        op_id,
        CsrFrontierStepKind::Forward,
        shape,
        frontier_in,
        frontier_out,
        allow_mask,
    )
}

/// CPU reference: one forward step. Returns a fresh bitset where bit
/// `v` is set iff any predecessor `u` with `frontier_in` bit set has
/// an edge `u → v` whose `edge_kind_mask[e] & allow_mask != 0`.
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

/// Validate CSR buffers shared by forward and backward CPU references.
///
/// The CPU oracle is used as GPU parity evidence, so malformed graph layouts
/// must fail loudly instead of producing an empty frontier that can mask
/// upstream object corruption.
#[cfg(any(test, feature = "cpu-parity"))]
pub(crate) fn validate_csr_frontier_step_cpu_inputs(
    label: &str,
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
) -> usize {
    let expected_offsets = node_count as usize + 1;
    assert_eq!(
        edge_offsets.len(),
        expected_offsets,
        "{label} CPU oracle received {} row offsets for node_count={node_count}; Fix: pass exactly node_count + 1 CSR offsets.",
        edge_offsets.len()
    );
    let edge_count = edge_offsets[expected_offsets - 1] as usize;
    assert!(
        edge_targets.len() >= edge_count && edge_kind_mask.len() >= edge_count,
        "{label} CPU oracle received edge_count={edge_count} but targets_len={} kind_mask_len={}. Fix: pass complete CSR edge buffers.",
        edge_targets.len(),
        edge_kind_mask.len()
    );
    for (index, pair) in edge_offsets.windows(2).enumerate() {
        assert!(
            pair[0] <= pair[1],
            "{label} CPU oracle received non-monotonic CSR offsets at row {index}: {} > {}. Fix: rebuild CSR row pointers before parity comparison.",
            pair[0],
            pair[1]
        );
    }
    edge_count
}

/// CPU reference using caller-owned output storage.
///
/// Malformed CSR inputs fail loudly. GPU parity evidence must not turn a
/// truncated row pointer or edge table into an all-zero frontier because that
/// would bless corrupted graph inputs as valid dataflow results.
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
    let words = bitset_words(node_count) as usize;
    out.clear();
    out.resize(words, 0);
    validate_csr_frontier_step_cpu_inputs(
        "csr_forward_traverse",
        node_count,
        edge_offsets,
        edge_targets,
        edge_kind_mask,
    );
    for src in 0..node_count {
        let word_idx = (src / 32) as usize;
        let bit_mask = 1u32 << (src % 32);
        if word_idx >= frontier_in.len() {
            continue;
        }
        if (frontier_in[word_idx] & bit_mask) == 0 {
            continue;
        }
        let edge_start = edge_offsets[src as usize] as usize;
        let edge_end = edge_offsets[src as usize + 1] as usize;
        for e in edge_start..edge_end {
            let kind = edge_kind_mask[e];
            if (kind & allow_mask) == 0 {
                continue;
            }
            let dst = edge_targets[e];
            if dst < node_count {
                let dst_word = (dst / 32) as usize;
                let dst_bit = 1u32 << (dst % 32);
                out[dst_word] |= dst_bit;
            }
        }
    }
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        OP_ID,
        || csr_forward_traverse(ProgramGraphShape::new(4, 4), "fin", "fout", 0xFFFF_FFFF),
        Some(|| {
            // Graph: 0→1, 0→2, 1→3, 2→3. Start frontier = {0}.
            // Expected out frontier = {1, 2}.
            let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
            vec![vec![
                to_bytes(&[0, 0, 0, 0]),          // pg_nodes
                to_bytes(&[0, 2, 3, 4, 4]),       // pg_edge_offsets
                to_bytes(&[1, 2, 3, 3]),          // pg_edge_targets
                to_bytes(&[1, 1, 1, 1]),          // pg_edge_kind_mask
                to_bytes(&[0, 0, 0, 0]),          // pg_node_tags
                to_bytes(&[0b0001]),              // frontier_in = {0}
                to_bytes(&[0]),                   // frontier_out
            ]]
        }),
        Some(|| {
            let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
            // After one forward step starting from {0}: frontier = {1, 2}.
            vec![vec![to_bytes(&[0b0110])]]
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_step_reaches_immediate_successors() {
        // 0→1, 0→2, 1→3, 2→3
        let got = cpu_ref(
            4,
            &[0, 2, 3, 4, 4],
            &[1, 2, 3, 3],
            &[1, 1, 1, 1],
            &[0b0001],
            0xFFFF_FFFF,
        );
        assert_eq!(got, vec![0b0110]);
    }

    #[test]
    fn edge_mask_filters_disallowed_edges() {
        // Same graph but one edge (0→1) has mask 0b10, others 0b01.
        // Allow only 0b01: out frontier should exclude node 1.
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
    fn empty_frontier_produces_empty_output() {
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
    #[should_panic(expected = "node_count + 1 CSR offsets")]
    fn malformed_csr_short_offsets_fail_loudly() {
        let _ = cpu_ref(4, &[0, 1], &[1], &[1], &[0b0001], 0xFFFF_FFFF);
    }

    #[test]
    fn cpu_ref_into_reuses_output_buffer() {
        let mut out = Vec::with_capacity(8);
        let ptr = out.as_ptr();
        cpu_ref_into(
            4,
            &[0, 2, 3, 4, 4],
            &[1, 2, 3, 3],
            &[1, 1, 1, 1],
            &[0b0001],
            0xFFFF_FFFF,
            &mut out,
        );
        assert_eq!(out.as_ptr(), ptr);
        assert_eq!(out, vec![0b0110]);
    }

    // ------------------------------------------------------------------
    // Adversarial fixtures  -  hostile boundaries, empty graphs, kind-mask
    // diversity (M8), malformed CSR, cross-word bitsets.
    // ------------------------------------------------------------------

    #[test]
    fn empty_graph_zero_nodes_zero_edges() {
        let got = cpu_ref(0, &[0], &[], &[], &[], 0xFFFF_FFFF);
        assert!(got.is_empty(), "0-node graph must produce empty bitset");
    }

    #[test]
    fn single_node_no_edges_returns_empty() {
        // 1 node, 0 edges, frontier {0} → no successors → empty output.
        let got = cpu_ref(1, &[0, 0], &[0], &[0], &[0b0001], 0xFFFF_FFFF);
        assert_eq!(got, vec![0]);
    }

    #[test]
    fn self_loops_only_preserve_frontier() {
        // 2 nodes, each has a self-loop. frontier {0,1} → out {0,1}
        let got = cpu_ref(2, &[0, 1, 2], &[0, 1], &[1, 1], &[0b0011], 0xFFFF_FFFF);
        assert_eq!(got, vec![0b0011]);
    }

    #[test]
    fn disconnected_components_only_reach_within_component() {
        // Component A: 0→1. Component B: 2→3. frontier {0} → out {1}
        let got = cpu_ref(
            4,
            &[0, 1, 1, 2, 2],
            &[1, 3],
            &[1, 1],
            &[0b0001],
            0xFFFF_FFFF,
        );
        assert_eq!(got, vec![0b0010]);
    }

    #[test]
    fn max_node_count_cross_word_boundary() {
        // 65 nodes (3 words), one edge from node 64 to node 0.
        let mut offsets = vec![0u32; 66];
        offsets[64] = 0;
        offsets[65] = 1;
        let mut frontier = vec![0u32; 3];
        frontier[2] = 1; // node 64 is set
        let got = cpu_ref(65, &offsets, &[0], &[1], &frontier, 0xFFFF_FFFF);
        assert_eq!(got.len(), 3);
        assert_eq!(got[0], 1, "node 0 reached from node 64");
        assert_eq!(got[1], 0);
        assert_eq!(got[2], 0, "node 64 is not its own successor");
    }

    #[test]
    fn edge_mask_filters_all_edges_yields_empty() {
        // All edges have mask 0b01; allow_mask is 0b10 → no overlap.
        let got = cpu_ref(
            4,
            &[0, 2, 3, 4, 4],
            &[1, 2, 3, 3],
            &[0b01, 0b01, 0b01, 0b01],
            &[0b0001],
            0b10,
        );
        assert_eq!(got, vec![0], "mask mismatch must block every edge");
    }

    #[test]
    fn edge_mask_universal_allow_mask_behaves_like_all_ones() {
        let got = cpu_ref(
            4,
            &[0, 2, 3, 4, 4],
            &[1, 2, 3, 3],
            &[1, 1, 1, 1],
            &[0b0001],
            0xFFFF_FFFF,
        );
        assert_eq!(got, vec![0b0110]);
    }

    #[test]
    fn edge_kind_diversity_ignoring_mask_would_fail() {
        // M8 regression: graph with two edge kinds.
        // 0→1 (DOMINANCE=0x01), 0→2 (ASSIGNMENT=0x02).
        // Mask only DOMINANCE → only node 1 reached.
        // A broken implementation that ignores kind_mask would reach BOTH.
        let got = cpu_ref(4, &[0, 2, 2, 2, 2], &[1, 2], &[0x01, 0x02], &[0b0001], 0x01);
        assert_eq!(
            got,
            vec![0b0010],
            "only DOMINANCE edge 0→1 must be traversed; broken impl ignoring mask would produce 0b0110"
        );
    }

    #[test]
    #[should_panic(expected = "non-monotonic CSR offsets")]
    fn malformed_csr_non_monotonic_offsets_fail_loudly() {
        let _ = cpu_ref(
            4,
            &[0, 2, 1, 1, 1],
            &[1, 2],
            &[1, 1],
            &[0b0001],
            0xFFFF_FFFF,
        );
    }

    #[test]
    fn frontier_word_oob_is_safely_skipped() {
        // 40 nodes need 2 words, but frontier only has 1 word.
        let offsets: Vec<u32> = (0..41).map(|_| 0).collect();
        let got = cpu_ref(40, &offsets, &[0], &[0], &[0], 0xFFFF_FFFF);
        assert_eq!(got, vec![0, 0], "short frontier must be handled safely");
    }
}
