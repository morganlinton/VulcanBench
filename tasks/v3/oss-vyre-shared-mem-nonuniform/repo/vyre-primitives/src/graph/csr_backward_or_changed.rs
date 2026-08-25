//! Reverse CSR frontier expansion over an in-place accumulator bitset.

use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

use crate::graph::csr_forward_traverse::bitset_words;
use crate::graph::program_graph::{
    ProgramGraphShape, BINDING_PRIMITIVE_START, NAME_EDGE_KIND_MASK, NAME_EDGE_OFFSETS,
    NAME_EDGE_TARGETS,
};

/// Canonical op id.
pub const OP_ID: &str = "vyre-primitives::graph::csr_backward_or_changed";
/// Source-lane workgroup for reverse in-place CSR expansion.
pub const CSR_BACKWARD_OR_CHANGED_WORKGROUP_SIZE: [u32; 3] = [256, 1, 1];

/// Dispatch grid for a node-parallel reverse in-place CSR expansion pass.
#[must_use]
pub const fn csr_backward_or_changed_parallel_grid(node_count: u32) -> [u32; 3] {
    [
        ceil_div_u32(
            at_least_one(node_count),
            CSR_BACKWARD_OR_CHANGED_WORKGROUP_SIZE[0],
        ),
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

/// Parallel in-place reverse expansion program for resident fixed-point drivers.
#[must_use]
pub fn csr_backward_or_changed_parallel(
    shape: ProgramGraphShape,
    frontier_out: &str,
    changed: &str,
    edge_kind_mask: u32,
) -> Program {
    let src = Expr::InvocationId { axis: 0 };
    let words = bitset_words(shape.node_count);
    let body = vec![
        Node::let_bind("edge_start", Expr::load(NAME_EDGE_OFFSETS, src.clone())),
        Node::let_bind(
            "edge_end",
            Expr::load(NAME_EDGE_OFFSETS, Expr::add(src.clone(), Expr::u32(1))),
        ),
        Node::let_bind("hit", Expr::u32(0)),
        Node::loop_for(
            "e",
            Expr::var("edge_start"),
            Expr::var("edge_end"),
            vec![Node::if_then(
                Expr::eq(Expr::var("hit"), Expr::u32(0)),
                vec![
                    Node::let_bind("kind_mask", Expr::load(NAME_EDGE_KIND_MASK, Expr::var("e"))),
                    Node::if_then(
                        Expr::ne(
                            Expr::bitand(Expr::var("kind_mask"), Expr::u32(edge_kind_mask)),
                            Expr::u32(0),
                        ),
                        vec![
                            Node::let_bind("dst", Expr::load(NAME_EDGE_TARGETS, Expr::var("e"))),
                            Node::if_then(
                                Expr::lt(Expr::var("dst"), Expr::u32(shape.node_count)),
                                vec![
                                    Node::let_bind(
                                        "dst_word",
                                        Expr::load(
                                            frontier_out,
                                            Expr::shr(Expr::var("dst"), Expr::u32(5)),
                                        ),
                                    ),
                                    Node::let_bind(
                                        "dst_bit",
                                        Expr::shl(
                                            Expr::u32(1),
                                            Expr::bitand(Expr::var("dst"), Expr::u32(31)),
                                        ),
                                    ),
                                    Node::if_then(
                                        Expr::ne(
                                            Expr::bitand(
                                                Expr::var("dst_word"),
                                                Expr::var("dst_bit"),
                                            ),
                                            Expr::u32(0),
                                        ),
                                        vec![Node::assign("hit", Expr::u32(1))],
                                    ),
                                ],
                            ),
                        ],
                    ),
                ],
            )],
        ),
        Node::if_then(
            Expr::eq(Expr::var("hit"), Expr::u32(1)),
            vec![
                Node::let_bind("src_word_idx", Expr::shr(src.clone(), Expr::u32(5))),
                Node::let_bind(
                    "src_bit",
                    Expr::shl(Expr::u32(1), Expr::bitand(src.clone(), Expr::u32(31))),
                ),
                Node::let_bind(
                    "old",
                    Expr::atomic_or(
                        frontier_out,
                        Expr::var("src_word_idx"),
                        Expr::var("src_bit"),
                    ),
                ),
                Node::if_then(
                    Expr::eq(
                        Expr::bitand(Expr::var("old"), Expr::var("src_bit")),
                        Expr::u32(0),
                    ),
                    vec![Node::let_bind(
                        "_changed",
                        Expr::atomic_or(changed, Expr::u32(0), Expr::u32(1)),
                    )],
                ),
            ],
        ),
    ];
    let mut buffers = shape.read_only_buffers();
    buffers.push(
        BufferDecl::storage(
            frontier_out,
            BINDING_PRIMITIVE_START,
            BufferAccess::ReadWrite,
            DataType::U32,
        )
        .with_count(words),
    );
    buffers.push(
        BufferDecl::storage(
            changed,
            BINDING_PRIMITIVE_START + 1,
            BufferAccess::ReadWrite,
            DataType::U32,
        )
        .with_count(1),
    );
    Program::wrapped(
        buffers,
        CSR_BACKWARD_OR_CHANGED_WORKGROUP_SIZE,
        vec![Node::Region {
            generator: Ident::from(OP_ID),
            source_region: None,
            body: Arc::new(vec![Node::if_then(
                Expr::lt(src.clone(), Expr::u32(shape.node_count)),
                body,
            )]),
        }],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_frontier_and_changed_bindings() {
        let program = csr_backward_or_changed_parallel(
            ProgramGraphShape::new(4, 3),
            "frontier",
            "changed",
            u32::MAX,
        );
        let names = program
            .buffers()
            .iter()
            .map(|buffer| buffer.name())
            .collect::<Vec<_>>();

        assert!(names.contains(&"frontier"));
        assert!(names.contains(&"changed"));
        assert_eq!(
            program.workgroup_size(),
            CSR_BACKWARD_OR_CHANGED_WORKGROUP_SIZE
        );
    }

    #[test]
    fn parallel_grid_packs_source_lanes_into_blocks() {
        assert_eq!(csr_backward_or_changed_parallel_grid(0), [1, 1, 1]);
        assert_eq!(csr_backward_or_changed_parallel_grid(1), [1, 1, 1]);
        assert_eq!(csr_backward_or_changed_parallel_grid(256), [1, 1, 1]);
        assert_eq!(csr_backward_or_changed_parallel_grid(257), [2, 1, 1]);
        assert_eq!(csr_backward_or_changed_parallel_grid(513), [3, 1, 1]);
    }
}
