//! `scc_decompose`  -  Forward-Backward strongly-connected-component
//! decomposition over `super::program_graph::ProgramGraph`.
//!
//! For each pivot node `v`, the set of nodes simultaneously forward-
//! reachable from `v` AND backward-reachable from `v` is exactly the
//! SCC containing `v`. The primitive runs one pass given a pre-
//! computed forward-reach bitset and backward-reach bitset and
//! emits `component[v] = pivot` for every `v` in the pivot's SCC.
//!
//! Driver composition: iterate until every node carries a component
//! id. The CPU reference below shows the composition; the Program
//! ships one pass.

use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

use crate::graph::csr_forward_traverse::bitset_words;

/// Canonical op id.
pub const OP_ID: &str = "vyre-primitives::graph::scc_decompose";
/// Source-lane workgroup for SCC component stamping.
pub const SCC_DECOMPOSE_WORKGROUP_SIZE: [u32; 3] = [256, 1, 1];

/// Dispatch grid for one SCC decomposition pass over `node_count` lanes.
#[must_use]
pub const fn scc_decompose_dispatch_grid(node_count: u32) -> [u32; 3] {
    [
        ceil_div_u32(at_least_one(node_count), SCC_DECOMPOSE_WORKGROUP_SIZE[0]),
        1,
        1,
    ]
}

const fn at_least_one(value: u32) -> u32 {
    if value == 0 {
        1
    } else {
        value
    }
}

const fn ceil_div_u32(value: u32, divisor: u32) -> u32 {
    ((value - 1) / divisor) + 1
}

/// Build a Program that marks every node in the intersection of
/// `forward` ∩ `backward` with the pivot id.
///
/// AUDIT_2026-04-24 F-SCC-01: the IR consumes `component_out` as a
/// ReadWrite buffer and only *writes* to slots where both bitsets
/// are set  -  it never reads the prior value. Callers MUST pre-load
/// `component_out` with the initial component assignment before
/// dispatch (typically `vec![u32::MAX; node_count]` for "unassigned"
/// on the first pivot, or the running component vector on
/// subsequent passes). The `cpu_ref` below models this contract by
/// taking `component_in` and cloning it into `out`; the IR expects
/// the dispatcher to supply that seed state in-place. Not binding
/// `component_in` as a separate ReadOnly buffer keeps the primitive
/// composable in a multi-pivot loop without ping-pong copies.
#[must_use]
pub fn scc_decompose(
    node_count: u32,
    forward_bitset: &str,
    backward_bitset: &str,
    component_out: &str,
    pivot: u32,
) -> Program {
    let t = Expr::InvocationId { axis: 0 };
    let words = bitset_words(node_count);

    let body = vec![
        Node::let_bind("word_idx", Expr::shr(t.clone(), Expr::u32(5))),
        Node::let_bind(
            "bit",
            Expr::shl(Expr::u32(1), Expr::bitand(t.clone(), Expr::u32(31))),
        ),
        Node::let_bind(
            "fwd_word",
            Expr::load(forward_bitset, Expr::var("word_idx")),
        ),
        Node::let_bind(
            "bwd_word",
            Expr::load(backward_bitset, Expr::var("word_idx")),
        ),
        Node::let_bind(
            "fwd_set",
            Expr::ne(
                Expr::bitand(Expr::var("fwd_word"), Expr::var("bit")),
                Expr::u32(0),
            ),
        ),
        Node::let_bind(
            "bwd_set",
            Expr::ne(
                Expr::bitand(Expr::var("bwd_word"), Expr::var("bit")),
                Expr::u32(0),
            ),
        ),
        // PHASE7_GRAPH HIGH: previously this stored unconditionally,
        // overwriting any prior pivot's assignment for nodes that
        // happen to be in both. The component_out invariant is "first
        // pivot wins" (the caller iterates pivots in descending reach
        // order). Read first; only write if the slot is still
        // u32::MAX (unassigned). Eliminates the silent
        // pivot-ordering hazard the audit flagged.
        Node::if_then(
            Expr::and(Expr::var("fwd_set"), Expr::var("bwd_set")),
            vec![
                Node::let_bind("prior", Expr::load(component_out, t.clone())),
                Node::if_then(
                    Expr::eq(Expr::var("prior"), Expr::u32(u32::MAX)),
                    vec![Node::store(component_out, t.clone(), Expr::u32(pivot))],
                ),
            ],
        ),
    ];

    Program::wrapped(
        vec![
            BufferDecl::storage(forward_bitset, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(words),
            BufferDecl::storage(backward_bitset, 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(words),
            BufferDecl::storage(component_out, 2, BufferAccess::ReadWrite, DataType::U32)
                .with_count(node_count),
        ],
        SCC_DECOMPOSE_WORKGROUP_SIZE,
        vec![Node::Region {
            generator: Ident::from(OP_ID),
            source_region: None,
            body: Arc::new(vec![Node::if_then(
                Expr::lt(t.clone(), Expr::u32(node_count)),
                body,
            )]),
        }],
    )
}

/// CPU reference: intersect two bitsets, stamp `pivot` into
/// `component[v]` for each `v` in the intersection.
///
/// `component_in` is the running component vector carried across
/// pivots. For the first pivot pass callers typically pass
/// `&vec![u32::MAX; node_count]`; subsequent passes feed back the
/// previous return value so this pivot's hits only overwrite
/// unassigned slots (or any slot  -  the scc_decompose composition
/// walks pivots in descending reach order so re-stamping is safe).
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn cpu_ref(
    node_count: u32,
    forward: &[u32],
    backward: &[u32],
    component_in: &[u32],
    pivot: u32,
) -> Vec<u32> {
    let mut out = Vec::new();
    cpu_ref_into(node_count, forward, backward, component_in, pivot, &mut out);
    out
}

/// CPU reference writing into caller-owned storage.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn cpu_ref_into(
    node_count: u32,
    forward: &[u32],
    backward: &[u32],
    component_in: &[u32],
    pivot: u32,
    out: &mut Vec<u32>,
) {
    let expected_words = crate::bitset::bitset_words(node_count) as usize;
    assert!(
        forward.len() >= expected_words && backward.len() >= expected_words,
        "scc_decompose CPU oracle received forward_len={} backward_len={} for node_count={node_count} requiring {expected_words} words. Fix: pass complete reachability bitsets before parity comparison.",
        forward.len(),
        backward.len()
    );
    assert_eq!(
        component_in.len(),
        node_count as usize,
        "scc_decompose CPU oracle received component_len={} for node_count={node_count}. Fix: pass one component slot per node before parity comparison.",
        component_in.len()
    );
    out.clear();
    out.extend_from_slice(component_in);
    for v in 0..node_count {
        let word = (v / 32) as usize;
        let bit = 1u32 << (v % 32);
        let fwd = forward[word] & bit != 0;
        let bwd = backward[word] & bit != 0;
        if fwd && bwd && (v as usize) < out.len() && out[v as usize] == u32::MAX {
            // PHASE7_GRAPH HIGH: first pivot wins. Match the GPU
            // kernel's "only stamp if unassigned" semantics so cpu_ref
            // is bit-identical to the GPU output.
            out[v as usize] = pivot;
        }
    }
}

#[cfg(test)]
mod regression_tests {
    use super::*;

    /// PHASE7_GRAPH HIGH regression: two pivots stamping the same
    /// node  -  the first pivot's assignment must survive. Prior
    /// scc_decompose blindly overwrote, so the order of dispatch
    /// determined the outcome.
    #[test]
    fn cpu_ref_first_pivot_wins_when_two_pivots_share_a_node() {
        // Node 0 is in the forward+backward intersection of BOTH
        // pivots. First pivot (5) stamps; second pivot (9) must NOT
        // overwrite.
        let component_in = vec![u32::MAX; 4];
        let forward = vec![0b1111];
        let backward = vec![0b1111];

        let after_first = cpu_ref(4, &forward, &backward, &component_in, 5);
        assert_eq!(after_first, vec![5, 5, 5, 5]);

        let after_second = cpu_ref(4, &forward, &backward, &after_first, 9);
        assert_eq!(
            after_second,
            vec![5, 5, 5, 5],
            "second pivot must NOT overwrite first pivot's assignments"
        );
    }

    /// PHASE7_GRAPH HIGH regression: a node only assigned by the
    /// second pivot still gets stamped (no false-skip).
    #[test]
    fn cpu_ref_unassigned_node_picks_up_second_pivot() {
        // Pivot 5 only sees node 0; pivot 9 only sees node 2. Both
        // must end up stamped.
        let component_in = vec![u32::MAX; 4];

        let after_first = cpu_ref(4, &[0b0001], &[0b0001], &component_in, 5);
        assert_eq!(after_first[0], 5);
        assert_eq!(after_first[2], u32::MAX);

        let after_second = cpu_ref(4, &[0b0100], &[0b0100], &after_first, 9);
        assert_eq!(after_second[0], 5, "first pivot survives");
        assert_eq!(after_second[2], 9, "second pivot stamps unassigned node");
    }
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        OP_ID,
        // AUDIT_2026-04-24 F-SCC-02: fixture differentiates forward
        // from backward so the intersection actually filters. Nodes
        // 0..=2 are forward-reachable from pivot 0; nodes 0, 2, 3
        // reach pivot 0 backward. Intersection = {0, 2}  -  node 1 is
        // forward-only, node 3 is backward-only, neither gets
        // stamped. Prior fixture fed identical bitsets and therefore
        // never exercised the AND gate.
        || scc_decompose(4, "fwd", "bwd", "comp", 0),
        Some(|| {
            let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
            vec![vec![
                to_bytes(&[0b0111]),                           // forward = {0,1,2}
                to_bytes(&[0b1101]),                           // backward = {0,2,3}
                to_bytes(&[u32::MAX, u32::MAX, u32::MAX, u32::MAX]),
            ]]
        }),
        Some(|| {
            let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
            // forward ∩ backward = 0b0101 → nodes 0 and 2 stamped.
            vec![vec![to_bytes(&[0, u32::MAX, 0, u32::MAX])]]
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn program_uses_packed_source_lane_workgroup() {
        let program = scc_decompose(513, "fwd", "bwd", "comp", 23);
        assert_eq!(program.workgroup_size(), SCC_DECOMPOSE_WORKGROUP_SIZE);
    }

    #[test]
    fn dispatch_grid_packs_node_lanes_into_blocks() {
        assert_eq!(scc_decompose_dispatch_grid(0), [1, 1, 1]);
        assert_eq!(scc_decompose_dispatch_grid(1), [1, 1, 1]);
        assert_eq!(scc_decompose_dispatch_grid(256), [1, 1, 1]);
        assert_eq!(scc_decompose_dispatch_grid(257), [2, 1, 1]);
        assert_eq!(scc_decompose_dispatch_grid(513), [3, 1, 1]);
    }

    #[test]
    fn intersection_stamps_pivot() {
        let out = cpu_ref(4, &[0b0011], &[0b0011], &[u32::MAX; 4], 0);
        assert_eq!(&out[0..2], &[0, 0]);
        assert_eq!(&out[2..4], &[u32::MAX, u32::MAX]);
    }

    #[test]
    fn disjoint_forward_backward_yields_no_change() {
        let comp_in = vec![u32::MAX; 4];
        let out = cpu_ref(4, &[0b0001], &[0b1000], &comp_in, 0);
        assert_eq!(out, comp_in);
    }

    // ------------------------------------------------------------------
    // Adversarial fixtures  -  empty/single/self-loop/disconnected/multi-word.
    // ------------------------------------------------------------------

    #[test]
    fn empty_graph_returns_empty() {
        let out = cpu_ref(0, &[], &[], &[], 0);
        assert!(out.is_empty());
    }

    #[test]
    fn single_node_not_in_intersection_stays_unassigned() {
        let out = cpu_ref(1, &[0], &[0], &[u32::MAX; 1], 0);
        assert_eq!(out, vec![u32::MAX]);
    }

    #[test]
    fn single_node_in_intersection_gets_stamped() {
        let out = cpu_ref(1, &[0b0001], &[0b0001], &[u32::MAX; 1], 7);
        assert_eq!(out, vec![7]);
    }

    #[test]
    fn self_loop_scc() {
        // Node 0 can reach itself forward and backward.
        let out = cpu_ref(1, &[0b0001], &[0b0001], &[u32::MAX; 1], 0);
        assert_eq!(out, vec![0]);
    }

    #[test]
    fn disconnected_components_only_stamp_reachable() {
        // Nodes 0 and 2 are in their own SCCs; nodes 1 and 3 are isolated.
        let forward = vec![0b0101];
        let backward = vec![0b0101];
        let comp_in = vec![u32::MAX; 4];
        let out = cpu_ref(4, &forward, &backward, &comp_in, 0);
        assert_eq!(out[0], 0);
        assert_eq!(out[1], u32::MAX);
        assert_eq!(out[2], 0);
        assert_eq!(out[3], u32::MAX);
    }

    #[test]
    fn all_nodes_pre_assigned_skips_all() {
        let comp_in = vec![5, 5, 5, 5];
        let out = cpu_ref(4, &[0b1111], &[0b1111], &comp_in, 9);
        assert_eq!(
            out,
            vec![5, 5, 5, 5],
            "pre-assigned nodes must not be overwritten"
        );
    }

    #[test]
    fn multi_word_bitset_cross_boundary() {
        // 65 nodes: node 32 (word 1 bit 0) and node 64 (word 2 bit 0) in intersection.
        let mut forward = vec![0u32; 3];
        let mut backward = vec![0u32; 3];
        forward[1] = 1; // node 32
        forward[2] = 1; // node 64
        backward[1] = 1; // node 32
        backward[2] = 1; // node 64
        let comp_in = vec![u32::MAX; 65];
        let out = cpu_ref(65, &forward, &backward, &comp_in, 42);
        assert_eq!(out[32], 42);
        assert_eq!(out[64], 42);
        assert_eq!(out[0], u32::MAX);
        assert_eq!(out[31], u32::MAX);
        assert_eq!(out[33], u32::MAX);
        assert_eq!(out[63], u32::MAX);
    }
}
