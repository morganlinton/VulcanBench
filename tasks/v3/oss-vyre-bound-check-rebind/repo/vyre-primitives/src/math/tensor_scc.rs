//! Bounded SCC-local bitset fixpoint primitive.
//!
//! The primitive operates on a row-major bit-matrix where each row is a
//! `u32` adjacency mask. Starting from `seed_mask`, it repeatedly expands
//! reachable bits through rows inside `group_mask` and writes the final
//! bounded closure to `out_mask[0]`.

use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Canonical op id.
pub const OP_ID: &str = "vyre-primitives::math::tensor_scc";

/// Build a bounded SCC-local bitset fixpoint program.
#[must_use]
pub fn tensor_scc_fixpoint(
    matrix_rows: &str,
    seed_mask: &str,
    group_mask: &str,
    out_mask: &str,
    row_count: u32,
    iteration_limit: u32,
) -> Program {
    let row_count = row_count.max(1);
    let iteration_limit = iteration_limit.max(1);
    let body = vec![
        Node::let_bind("active", Expr::load(seed_mask, Expr::u32(0))),
        Node::loop_for(
            "iter",
            Expr::u32(0),
            Expr::u32(iteration_limit),
            vec![
                Node::let_bind("next", Expr::var("active")),
                Node::loop_for(
                    "row",
                    Expr::u32(0),
                    Expr::u32(row_count),
                    vec![Node::if_then(
                        Expr::ne(
                            Expr::bitand(
                                Expr::var("active"),
                                Expr::shl(Expr::u32(1), Expr::var("row")),
                            ),
                            Expr::u32(0),
                        ),
                        vec![Node::assign(
                            "next",
                            Expr::bitor(
                                Expr::var("next"),
                                Expr::bitand(
                                    Expr::load(matrix_rows, Expr::var("row")),
                                    Expr::load(group_mask, Expr::u32(0)),
                                ),
                            ),
                        )],
                    )],
                ),
                Node::assign("active", Expr::var("next")),
            ],
        ),
        Node::store(
            out_mask,
            Expr::u32(0),
            Expr::bitand(Expr::var("active"), Expr::load(group_mask, Expr::u32(0))),
        ),
    ];

    Program::wrapped(
        vec![
            BufferDecl::storage(matrix_rows, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(row_count),
            BufferDecl::storage(seed_mask, 1, BufferAccess::ReadOnly, DataType::U32).with_count(1),
            BufferDecl::storage(group_mask, 2, BufferAccess::ReadOnly, DataType::U32).with_count(1),
            BufferDecl::storage(out_mask, 3, BufferAccess::ReadWrite, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![Node::Region {
            generator: Ident::from(OP_ID),
            source_region: None,
            body: Arc::new(body),
        }],
    )
}

/// CPU reference for the bounded SCC-local bitset fixpoint.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn cpu_ref(matrix_rows: &[u32], seed_mask: u32, group_mask: u32, iteration_limit: u32) -> u32 {
    let mut active = seed_mask & group_mask;
    for _ in 0..iteration_limit {
        let previous = active;
        let mut next = active;
        for (row, edges) in matrix_rows.iter().copied().enumerate().take(32) {
            if (active & (1u32 << row)) != 0 {
                next |= edges & group_mask;
            }
        }
        active = next & group_mask;
        if active == previous {
            break;
        }
    }
    active
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_ref_closes_cycle_inside_group() {
        let rows = [0b0010, 0b0100, 0b0001, 0b1000];
        assert_eq!(cpu_ref(&rows, 0b0001, 0b0111, 8), 0b0111);
    }

    #[test]
    fn cpu_ref_masks_edges_outside_group() {
        let rows = [0b1010, 0b0100, 0b0000, 0b0001];
        assert_eq!(cpu_ref(&rows, 0b0001, 0b0011, 8), 0b0011);
    }

    #[test]
    fn program_declares_bounded_matrix_buffers() {
        let program = tensor_scc_fixpoint("rows", "seed", "group", "out", 4, 8);
        assert_eq!(program.workgroup_size(), [1, 1, 1]);
        assert_eq!(program.buffers()[0].count(), 4);
        assert_eq!(program.buffers()[3].count(), 1);
    }
}
