//! Shared CSR frontier-step Program builder.
//!
//! Forward and reverse traversals use the same ProgramGraph ABI,
//! frontier buffers, edge-kind mask filtering, and packed-NodeSet
//! output writes. The only semantic difference is whether the input
//! frontier is tested at `src` before walking outgoing edges or at
//! `dst` while scanning a source row.

use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

use crate::graph::program_graph::{
    ProgramGraphShape, BINDING_PRIMITIVE_START, NAME_EDGE_KIND_MASK, NAME_EDGE_OFFSETS,
    NAME_EDGE_TARGETS,
};

/// Canonical binding index for the input frontier bitset.
pub const BINDING_FRONTIER_IN: u32 = BINDING_PRIMITIVE_START;
/// Canonical binding index for the output frontier bitset.
pub const BINDING_FRONTIER_OUT: u32 = BINDING_PRIMITIVE_START + 1;
pub(crate) const CSR_FRONTIER_STEP_WORKGROUP_SIZE: [u32; 3] = [256, 1, 1];

/// Dispatch grid for one source-lane CSR frontier step.
#[must_use]
pub const fn csr_frontier_step_dispatch_grid(node_count: u32) -> [u32; 3] {
    let blocks = node_count.div_ceil(CSR_FRONTIER_STEP_WORKGROUP_SIZE[0]);
    if blocks == 0 {
        [1, 1, 1]
    } else {
        [blocks, 1, 1]
    }
}

/// Direction for a one-step CSR frontier traversal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CsrFrontierStepKind {
    /// If `src` is active, emit each allowed `dst`.
    Forward,
    /// If any allowed `dst` is active, emit `src`.
    Backward,
}

/// Build a one-step CSR frontier traversal under a caller-owned op id.
#[must_use]
pub(crate) fn csr_frontier_step_program(
    op_id: &'static str,
    kind: CsrFrontierStepKind,
    shape: ProgramGraphShape,
    frontier_in: &str,
    frontier_out: &str,
    allow_mask: u32,
) -> Program {
    let t = Expr::InvocationId { axis: 0 };
    let words = crate::bitset::bitset_words(shape.node_count);
    let mut buffers = shape.read_only_buffers();
    buffers.push(
        BufferDecl::storage(
            frontier_in,
            BINDING_FRONTIER_IN,
            BufferAccess::ReadOnly,
            DataType::U32,
        )
        .with_count(words),
    );
    buffers.push(
        BufferDecl::storage(
            frontier_out,
            BINDING_FRONTIER_OUT,
            BufferAccess::ReadWrite,
            DataType::U32,
        )
        .with_count(words),
    );

    let body = match kind {
        CsrFrontierStepKind::Forward => {
            forward_body(shape.node_count, frontier_in, frontier_out, allow_mask, t)
        }
        CsrFrontierStepKind::Backward => vec![Node::if_then(
            Expr::lt(t.clone(), Expr::u32(shape.node_count)),
            backward_body(shape.node_count, frontier_in, frontier_out, allow_mask, t),
        )],
    };

    Program::wrapped(
        buffers,
        CSR_FRONTIER_STEP_WORKGROUP_SIZE,
        vec![Node::Region {
            generator: Ident::from(op_id),
            source_region: None,
            body: Arc::new(body),
        }],
    )
}

pub(crate) fn active_frontier_source_lane(
    node_count: u32,
    frontier_in: &str,
    source: Expr,
    active_body: Vec<Node>,
) -> Node {
    Node::if_then(
        Expr::lt(source.clone(), Expr::u32(node_count)),
        vec![
            Node::let_bind("src", source),
            Node::let_bind("word_idx", Expr::shr(Expr::var("src"), Expr::u32(5))),
            Node::let_bind(
                "bit_mask",
                Expr::shl(Expr::u32(1), Expr::bitand(Expr::var("src"), Expr::u32(31))),
            ),
            Node::let_bind("src_word", Expr::load(frontier_in, Expr::var("word_idx"))),
            Node::if_then(
                Expr::ne(
                    Expr::bitand(Expr::var("src_word"), Expr::var("bit_mask")),
                    Expr::u32(0),
                ),
                active_body,
            ),
        ],
    )
}

fn forward_body(
    node_count: u32,
    frontier_in: &str,
    frontier_out: &str,
    allow_mask: u32,
    t: Expr,
) -> Vec<Node> {
    vec![active_frontier_source_lane(
        node_count,
        frontier_in,
        t,
        edge_scan_body(
            allow_mask,
            vec![Node::let_bind(
                "dst",
                Expr::load(NAME_EDGE_TARGETS, Expr::var("e")),
            )],
            vec![Node::if_then(
                Expr::lt(Expr::var("dst"), Expr::u32(node_count)),
                mark_node_bit(frontier_out, "dst", "dst_word_idx", "dst_bit"),
            )],
        ),
    )]
}

fn backward_body(
    node_count: u32,
    frontier_in: &str,
    frontier_out: &str,
    allow_mask: u32,
    t: Expr,
) -> Vec<Node> {
    let mut body = vec![
        Node::let_bind("src", t),
        Node::let_bind("hit", Expr::u32(0)),
    ];
    body.extend(edge_bounds_and_loop(vec![Node::if_then(
        Expr::eq(Expr::var("hit"), Expr::u32(0)),
        vec![
            Node::let_bind("kind_mask", Expr::load(NAME_EDGE_KIND_MASK, Expr::var("e"))),
            Node::if_then(
                Expr::ne(
                    Expr::bitand(Expr::var("kind_mask"), Expr::u32(allow_mask)),
                    Expr::u32(0),
                ),
                vec![
                    Node::let_bind("dst", Expr::load(NAME_EDGE_TARGETS, Expr::var("e"))),
                    Node::if_then(
                        Expr::lt(Expr::var("dst"), Expr::u32(node_count)),
                        vec![
                            Node::let_bind(
                                "dst_word",
                                Expr::load(frontier_in, Expr::shr(Expr::var("dst"), Expr::u32(5))),
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
                                    Expr::bitand(Expr::var("dst_word"), Expr::var("dst_bit")),
                                    Expr::u32(0),
                                ),
                                vec![Node::assign("hit", Expr::u32(1))],
                            ),
                        ],
                    ),
                ],
            ),
        ],
    )]));
    body.push(Node::if_then(
        Expr::eq(Expr::var("hit"), Expr::u32(1)),
        mark_node_bit(frontier_out, "src", "src_word_idx", "src_bit"),
    ));
    body
}

pub(crate) fn edge_scan_body(
    allow_mask: u32,
    before_kind_body: Vec<Node>,
    on_allowed_body: Vec<Node>,
) -> Vec<Node> {
    let mut loop_body = before_kind_body;
    loop_body.push(Node::let_bind(
        "kind_mask",
        Expr::load(NAME_EDGE_KIND_MASK, Expr::var("e")),
    ));
    loop_body.push(Node::if_then(
        Expr::ne(
            Expr::bitand(Expr::var("kind_mask"), Expr::u32(allow_mask)),
            Expr::u32(0),
        ),
        on_allowed_body,
    ));
    edge_bounds_and_loop(loop_body)
}

fn edge_bounds_and_loop(loop_body: Vec<Node>) -> Vec<Node> {
    vec![
        Node::let_bind(
            "edge_start",
            Expr::load(NAME_EDGE_OFFSETS, Expr::var("src")),
        ),
        Node::let_bind(
            "edge_end",
            Expr::load(NAME_EDGE_OFFSETS, Expr::add(Expr::var("src"), Expr::u32(1))),
        ),
        Node::loop_for(
            "e",
            Expr::var("edge_start"),
            Expr::var("edge_end"),
            loop_body,
        ),
    ]
}

fn mark_node_bit(
    frontier_out: &str,
    node_var: &'static str,
    word_var: &'static str,
    bit_var: &'static str,
) -> Vec<Node> {
    vec![
        Node::let_bind(word_var, Expr::shr(Expr::var(node_var), Expr::u32(5))),
        Node::let_bind(
            bit_var,
            Expr::shl(
                Expr::u32(1),
                Expr::bitand(Expr::var(node_var), Expr::u32(31)),
            ),
        ),
        Node::let_bind(
            "_prev",
            Expr::atomic_or(frontier_out, Expr::var(word_var), Expr::var(bit_var)),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::{csr_frontier_step_dispatch_grid, CSR_FRONTIER_STEP_WORKGROUP_SIZE};

    fn scalar_forward(
        node_count: u32,
        edge_offsets: &[u32],
        edge_targets: &[u32],
        edge_kind_mask: &[u32],
        frontier_in: &[u32],
        allow_mask: u32,
    ) -> Vec<u32> {
        let mut out = vec![0_u32; crate::bitset::bitset_words(node_count) as usize];
        for src in 0..node_count {
            let src_word = (src / 32) as usize;
            if frontier_in
                .get(src_word)
                .copied()
                .is_none_or(|word| (word & (1_u32 << (src % 32))) == 0)
            {
                continue;
            }
            let start = edge_offsets[src as usize] as usize;
            let end = edge_offsets[src as usize + 1] as usize;
            for edge in start..end {
                if (edge_kind_mask[edge] & allow_mask) == 0 {
                    continue;
                }
                let dst = edge_targets[edge];
                if dst < node_count {
                    out[(dst / 32) as usize] |= 1_u32 << (dst % 32);
                }
            }
        }
        out
    }

    fn scalar_backward(
        node_count: u32,
        edge_offsets: &[u32],
        edge_targets: &[u32],
        edge_kind_mask: &[u32],
        frontier_in: &[u32],
        allow_mask: u32,
    ) -> Vec<u32> {
        let mut out = vec![0_u32; crate::bitset::bitset_words(node_count) as usize];
        for src in 0..node_count {
            let start = edge_offsets[src as usize] as usize;
            let end = edge_offsets[src as usize + 1] as usize;
            let mut hit = false;
            for edge in start..end {
                if (edge_kind_mask[edge] & allow_mask) == 0 {
                    continue;
                }
                let dst = edge_targets[edge];
                if dst < node_count {
                    let word = (dst / 32) as usize;
                    let bit = 1_u32 << (dst % 32);
                    if frontier_in
                        .get(word)
                        .copied()
                        .is_some_and(|w| (w & bit) != 0)
                    {
                        hit = true;
                        break;
                    }
                }
            }
            if hit {
                out[(src / 32) as usize] |= 1_u32 << (src % 32);
            }
        }
        out
    }

    #[test]
    fn generated_csr_frontier_step_uses_block_sized_workgroup() {
        let program = crate::graph::csr_forward_traverse::csr_forward_traverse(
            crate::graph::program_graph::ProgramGraphShape::new(1024, 1536),
            "frontier_in",
            "frontier_out",
            u32::MAX,
        );

        assert_eq!(program.workgroup_size(), CSR_FRONTIER_STEP_WORKGROUP_SIZE);
        assert!(
            program.workgroup_size()[0] > 1,
            "Fix: CSR frontier traversal must not launch one CUDA block per source node."
        );
    }

    #[test]
    fn dispatch_grid_packs_source_lanes_into_workgroups() {
        assert_eq!(csr_frontier_step_dispatch_grid(0), [1, 1, 1]);
        assert_eq!(csr_frontier_step_dispatch_grid(1), [1, 1, 1]);
        assert_eq!(csr_frontier_step_dispatch_grid(256), [1, 1, 1]);
        assert_eq!(csr_frontier_step_dispatch_grid(257), [2, 1, 1]);
        assert_eq!(csr_frontier_step_dispatch_grid(513), [3, 1, 1]);
    }

    #[test]
    fn generated_csr_frontier_steps_match_scalar_reference() {
        let mut state = 0xC5A1_F00D_u32;
        for case in 0..2048_u32 {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let node_count = (state % 97) + 1;
            let mut offsets = Vec::with_capacity(node_count as usize + 1);
            let mut targets = Vec::new();
            let mut masks = Vec::new();
            offsets.push(0);
            for src in 0..node_count {
                state = state.rotate_left(5) ^ src.wrapping_mul(0x9E37_79B9);
                let degree = state % 5;
                for edge in 0..degree {
                    state = state.rotate_left(7) ^ edge.wrapping_mul(0x85EB_CA6B);
                    let target = match edge % 5 {
                        0 => state % node_count,
                        1 => node_count,
                        2 => u32::MAX,
                        _ => state % (node_count + 3),
                    };
                    targets.push(target);
                    masks.push(1_u32 << (state & 7));
                }
                offsets.push(targets.len() as u32);
            }
            let words = crate::bitset::bitset_words(node_count) as usize;
            let mut frontier = vec![0_u32; words];
            for node in 0..node_count {
                state = state.rotate_left(3) ^ node.wrapping_mul(0x27D4_EB2D);
                if (state & 3) != 0 {
                    frontier[(node / 32) as usize] |= 1_u32 << (node % 32);
                }
            }
            let allow_mask = if case % 11 == 0 {
                0
            } else {
                (1_u32 << (case & 7)) | (1_u32 << ((case + 3) & 7))
            };

            assert_eq!(
                crate::graph::csr_forward_traverse::cpu_ref(
                    node_count, &offsets, &targets, &masks, &frontier, allow_mask,
                ),
                scalar_forward(node_count, &offsets, &targets, &masks, &frontier, allow_mask),
                "forward case {case}"
            );
            assert_eq!(
                crate::graph::csr_backward_traverse::cpu_ref(
                    node_count, &offsets, &targets, &masks, &frontier, allow_mask,
                ),
                scalar_backward(node_count, &offsets, &targets, &masks, &frontier, allow_mask),
                "backward case {case}"
            );
        }
    }
}
