//! Postorder tree walk over [`vyre_foundation::vast::VastNode`] rows.
//!
//! The registered op consumes the node table and walks a general
//! first-child / next-sibling tree. The legacy two-argument helper is
//! retained for callers that explicitly need the old spine sequence.

use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};
use vyre_primitives::graph::vast_tree_walk;

use crate::region::{tag_program, wrap_anonymous};

const OP_ID: &str = "vyre-libs::graph::ast_walk_postorder";

/// Emit `node_count - 1 - i` into `out[i]` for a spine postorder sequence.
#[must_use]
pub fn ast_walk_postorder(out: &str, node_count: u32) -> Program {
    let out_words = node_count.max(1);
    let body = vec![Node::loop_for(
        "i",
        Expr::u32(0),
        Expr::u32(node_count),
        vec![Node::store(
            out,
            Expr::var("i"),
            Expr::sub(Expr::u32(node_count.saturating_sub(1)), Expr::var("i")),
        )],
    )];

    Program::wrapped(
        vec![
            BufferDecl::storage(out, 0, BufferAccess::ReadWrite, DataType::U32)
                .with_count(out_words),
        ],
        [1, 1, 1],
        vec![wrap_anonymous(
            "vyre-libs::graph::ast_walk_postorder_spine",
            body,
        )],
    )
}

/// Emit postorder node indices for a general VAST first-child /
/// next-sibling tree rooted at node `0`.
#[must_use]
pub fn ast_walk_postorder_nodes(nodes: &str, out: &str, node_count: u32, out_cap: u32) -> Program {
    tag_program(
        OP_ID,
        vast_tree_walk::ast_walk_postorder(nodes, out, node_count, out_cap),
    )
}

inventory::submit! {
    crate::harness::OpEntry {
        id: OP_ID,
        build: || ast_walk_postorder_nodes("nodes", "out", 6, 8),
        test_inputs: Some(|| {
            vec![vec![
                super::ast_walk_preorder::pack_branching_fixture(),
                vec![0u8; 32],
            ]]
        }),
        expected_output: Some(|| {
            let node_region = super::ast_walk_preorder::pack_branching_fixture();
            let Ok(order) = vyre_foundation::vast::walk_postorder_indices(&node_region, 6, 128)
            else {
                return Vec::new();
            };
            let mut out = vec![0u8; 32];
            for (i, v) in order.into_iter().enumerate() {
                out[i * 4..(i + 1) * 4].copy_from_slice(&v.to_le_bytes());
            }
            vec![vec![out]]
        }),
        category: Some("graph"),
    }
}

#[cfg(test)]
mod tests {
    use super::super::ast_walk_preorder::{pack_branching_fixture, pack_spine_fixture};
    use super::*;

    #[test]
    fn postorder_matches_host_reverse_of_preorder_spine() {
        let (_, node_region) = pack_spine_fixture(4);
        let pre = vyre_foundation::vast::walk_preorder_indices(&node_region, 4, 128).unwrap();
        let post = vyre_foundation::vast::walk_postorder_indices(&node_region, 4, 128).unwrap();
        let rev: Vec<u32> = pre.iter().rev().copied().collect();
        assert_eq!(post, rev);
        let p = ast_walk_postorder("out", 4);
        assert!(
            vyre::validate(&p).is_empty(),
            "postorder spine program must validate"
        );
    }

    #[test]
    fn postorder_handles_branching_siblings() {
        let node_region = pack_branching_fixture();
        let order = vyre_foundation::vast::walk_postorder_indices(&node_region, 6, 128).unwrap();
        assert_eq!(order, vec![4, 1, 2, 5, 3, 0]);
        let p = ast_walk_postorder_nodes("nodes", "out", 6, 8);
        assert!(vyre::validate(&p).is_empty());
    }
}
