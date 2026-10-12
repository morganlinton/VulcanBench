//! Preorder walk for a tree encoded like [`vyre_foundation::vast::VastNode`]
//! (`first_child` plus `next_sibling`, rooted at node `0`).
//!
//! The IR uses parent links to avoid an auxiliary stack: after visiting
//! a node it descends to `first_child` when present; otherwise it climbs
//! parents until it finds a valid `next_sibling`.

use vyre::ir::Program;
use vyre_foundation::vast::{VastNode, NODE_STRIDE_U32, SENTINEL};
use vyre_primitives::graph::vast_tree_walk;

use crate::region::tag_program;

const OP_ID: &str = "vyre-libs::graph::ast_walk_preorder";

/// Pack a spine fixture: full VAST bytes plus the node-table slice.
#[cfg(test)]
pub(crate) fn pack_spine_fixture(node_count: u32) -> (Vec<u8>, Vec<u8>) {
    let full = vyre_foundation::vast::pack_spine_vast(&vec![1u32; node_count as usize]);
    let node_len = (node_count as usize) * NODE_STRIDE_U32 * 4;
    let start = vyre_foundation::vast::HEADER_LEN;
    let region = full[start..start + node_len].to_vec();
    (full, region)
}

/// Pack a branching fixture:
///
/// ```text
/// 0
/// |- 1
/// |  `- 4
/// |- 2
/// `- 3
///    `- 5
/// ```
///
/// Preorder: `0, 1, 4, 2, 3, 5`; postorder: `4, 1, 2, 5, 3, 0`.
pub(crate) fn pack_branching_fixture() -> Vec<u8> {
    let nodes = [
        VastNode {
            kind: 1,
            parent_idx: SENTINEL,
            first_child: 1,
            next_sibling: SENTINEL,
            src_file: 0,
            src_byte_off: 0,
            src_byte_len: 1,
            attr_off: 0,
            attr_len: 0,
            reserved: 0,
        },
        VastNode {
            kind: 1,
            parent_idx: 0,
            first_child: 4,
            next_sibling: 2,
            src_file: 0,
            src_byte_off: 1,
            src_byte_len: 1,
            attr_off: 0,
            attr_len: 0,
            reserved: 0,
        },
        VastNode {
            kind: 1,
            parent_idx: 0,
            first_child: SENTINEL,
            next_sibling: 3,
            src_file: 0,
            src_byte_off: 2,
            src_byte_len: 1,
            attr_off: 0,
            attr_len: 0,
            reserved: 0,
        },
        VastNode {
            kind: 1,
            parent_idx: 0,
            first_child: 5,
            next_sibling: SENTINEL,
            src_file: 0,
            src_byte_off: 3,
            src_byte_len: 1,
            attr_off: 0,
            attr_len: 0,
            reserved: 0,
        },
        VastNode {
            kind: 1,
            parent_idx: 1,
            first_child: SENTINEL,
            next_sibling: SENTINEL,
            src_file: 0,
            src_byte_off: 4,
            src_byte_len: 1,
            attr_off: 0,
            attr_len: 0,
            reserved: 0,
        },
        VastNode {
            kind: 1,
            parent_idx: 3,
            first_child: SENTINEL,
            next_sibling: SENTINEL,
            src_file: 0,
            src_byte_off: 5,
            src_byte_len: 1,
            attr_off: 0,
            attr_len: 0,
            reserved: 0,
        },
    ];
    let mut out = Vec::with_capacity(nodes.len() * NODE_STRIDE_U32 * 4);
    for node in nodes {
        out.extend_from_slice(&node.to_bytes());
    }
    out
}

/// Emit preorder node indices for a VAST first-child / next-sibling tree.
#[must_use]
pub fn ast_walk_preorder(nodes: &str, out: &str, node_count: u32, out_cap: u32) -> Program {
    tag_program(
        OP_ID,
        vast_tree_walk::ast_walk_preorder(nodes, out, node_count, out_cap),
    )
}

fn preorder_harness_inputs() -> Vec<Vec<Vec<u8>>> {
    vec![vec![pack_branching_fixture(), vec![0u8; 32]]]
}

fn preorder_harness_expected() -> Vec<Vec<Vec<u8>>> {
    let node_region = pack_branching_fixture();
    let Ok(order) = vyre_foundation::vast::walk_preorder_indices(&node_region, 6, 128) else {
        return Vec::new();
    };
    let mut out = vec![0u8; 32];
    for (i, &v) in order.iter().enumerate() {
        out[i * 4..(i + 1) * 4].copy_from_slice(&v.to_le_bytes());
    }
    vec![vec![out]]
}

inventory::submit! {
    crate::harness::OpEntry {
        id: OP_ID,
        build: || ast_walk_preorder("nodes", "out", 6, 8),
        test_inputs: Some(preorder_harness_inputs),
        expected_output: Some(preorder_harness_expected),
        category: Some("graph"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression: the original test only asserted `validate(&p).is_empty()`, which
    /// passes even if the walk emits postorder or visits nodes in the wrong order
    /// (the IR validator checks program shape, not traversal semantics).  This
    /// replacement asserts the concrete preorder output for a 3-node spine (0→1→2)
    /// and retains the IR-validation check as a secondary guard.
    #[test]
    fn preorder_program_validates() {
        // 3-node spine: 0 → 1 → 2 (each node's first_child = next node).
        let (_full, region) = pack_spine_fixture(3);
        let expected_order =
            vyre_foundation::vast::walk_preorder_indices(&region, 3, 16).unwrap();
        // Spine preorder is always the identity permutation.
        assert_eq!(
            expected_order,
            vec![0u32, 1, 2],
            "spine preorder must be identity [0, 1, 2]"
        );
        // IR shape is also valid — kept as a supporting check.
        let p = ast_walk_preorder("nodes", "out", 4, 8);
        assert!(
            vyre::validate(&p).is_empty(),
            "ast_walk_preorder IR must pass the validator"
        );
    }

    #[test]
    fn spine_fixture_region_has_expected_node_words() {
        let (_full, region) = pack_spine_fixture(3);
        assert_eq!(region.len(), 3 * NODE_STRIDE_U32 * 4);
        assert_eq!(
            vyre_foundation::vast::walk_preorder_indices(&region, 3, 16).unwrap(),
            vec![0, 1, 2]
        );
    }

    #[test]
    fn preorder_handles_branching_siblings() {
        let node_region = pack_branching_fixture();
        let order = vyre_foundation::vast::walk_preorder_indices(&node_region, 6, 128).unwrap();
        assert_eq!(order, vec![0, 1, 4, 2, 3, 5]);
        let p = ast_walk_preorder("nodes", "out", 6, 8);
        assert!(vyre::validate(&p).is_empty());
    }
}
