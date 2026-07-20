use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

use super::batch_shared::checked_batched_frontier_words;
use super::layout::{CSR_FORWARD_OR_CHANGED_PARALLEL_WORKGROUP_SIZE, OP_ID};
use crate::graph::program_graph::{
    ProgramGraphShape, BINDING_PRIMITIVE_START, NAME_EDGE_KIND_MASK, NAME_EDGE_OFFSETS,
    NAME_EDGE_TARGETS,
};

/// Parallel in-place expansion for several frontier accumulators at once.
///
/// Invocation axis 0 is the source node and axis 1 is the query/frontier index.
/// `frontier_out` is laid out as `query_count` consecutive bitsets, each
/// containing `bitset_words(shape.node_count)` u32 words. `changed` contains
/// one u32 flag per query.
#[must_use]
pub fn csr_forward_or_changed_parallel_batch(
    shape: ProgramGraphShape,
    frontier_out: &str,
    changed: &str,
    edge_kind_mask: u32,
    query_count: u32,
) -> Program {
    // Fail fast on an invalid flat-frontier shape rather than silently degrading
    // to an inert empty kernel (silent recall loss). Use
    // `try_csr_forward_or_changed_parallel_batch` for structured handling.
    try_csr_forward_or_changed_parallel_batch(
        shape,
        frontier_out,
        changed,
        edge_kind_mask,
        query_count,
    )
    .unwrap_or_else(|error| panic!("{error}"))
}

/// Parallel in-place expansion for several frontier accumulators with checked
/// flat-frontier sizing.
pub fn try_csr_forward_or_changed_parallel_batch(
    shape: ProgramGraphShape,
    frontier_out: &str,
    changed: &str,
    edge_kind_mask: u32,
    query_count: u32,
) -> Result<Program, String> {
    if query_count == 0 {
        return Err(
            "Fix: csr_forward_or_changed_parallel_batch requires at least one query frontier."
                .to_string(),
        );
    }
    let src = Expr::InvocationId { axis: 0 };
    let query = Expr::InvocationId { axis: 1 };
    let words = crate::bitset::bitset_words(shape.node_count);
    let total_words = checked_batched_frontier_words(words, query_count)?;
    let query_word_base = Expr::mul(query.clone(), Expr::u32(words));
    let body = vec![
        Node::let_bind("query_word_base", query_word_base.clone()),
        Node::let_bind(
            "word_idx",
            Expr::add(
                Expr::var("query_word_base"),
                Expr::shr(src.clone(), Expr::u32(5)),
            ),
        ),
        Node::let_bind(
            "bit_mask",
            Expr::shl(Expr::u32(1), Expr::bitand(src.clone(), Expr::u32(31))),
        ),
        Node::let_bind("src_word", Expr::load(frontier_out, Expr::var("word_idx"))),
        Node::if_then(
            Expr::ne(
                Expr::bitand(Expr::var("src_word"), Expr::var("bit_mask")),
                Expr::u32(0),
            ),
            vec![
                Node::let_bind("edge_start", Expr::load(NAME_EDGE_OFFSETS, src.clone())),
                Node::let_bind(
                    "edge_end",
                    Expr::load(NAME_EDGE_OFFSETS, Expr::add(src.clone(), Expr::u32(1))),
                ),
                Node::loop_for(
                    "e",
                    Expr::var("edge_start"),
                    Expr::var("edge_end"),
                    vec![
                        Node::let_bind(
                            "kind_mask",
                            Expr::load(NAME_EDGE_KIND_MASK, Expr::var("e")),
                        ),
                        Node::if_then(
                            Expr::ne(
                                Expr::bitand(Expr::var("kind_mask"), Expr::u32(edge_kind_mask)),
                                Expr::u32(0),
                            ),
                            vec![
                                Node::let_bind(
                                    "dst",
                                    Expr::load(NAME_EDGE_TARGETS, Expr::var("e")),
                                ),
                                Node::if_then(
                                    Expr::lt(Expr::var("dst"), Expr::u32(shape.node_count)),
                                    vec![
                                        Node::let_bind(
                                            "dst_word_idx",
                                            Expr::add(
                                                Expr::var("query_word_base"),
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
                                        Node::let_bind(
                                            "old",
                                            Expr::atomic_or(
                                                frontier_out,
                                                Expr::var("dst_word_idx"),
                                                Expr::var("dst_bit"),
                                            ),
                                        ),
                                        Node::if_then(
                                            Expr::eq(
                                                Expr::bitand(
                                                    Expr::var("old"),
                                                    Expr::var("dst_bit"),
                                                ),
                                                Expr::u32(0),
                                            ),
                                            vec![Node::let_bind(
                                                "_changed",
                                                Expr::atomic_or(
                                                    changed,
                                                    query.clone(),
                                                    Expr::u32(1),
                                                ),
                                            )],
                                        ),
                                    ],
                                ),
                            ],
                        ),
                    ],
                ),
            ],
        ),
    ];
    let mut buffers = shape.try_read_only_buffers()?;
    buffers.push(
        BufferDecl::storage(
            frontier_out,
            BINDING_PRIMITIVE_START,
            BufferAccess::ReadWrite,
            DataType::U32,
        )
        .with_count(total_words.max(1)),
    );
    buffers.push(
        BufferDecl::storage(
            changed,
            BINDING_PRIMITIVE_START + 1,
            BufferAccess::ReadWrite,
            DataType::U32,
        )
        .with_count(query_count),
    );
    Ok(Program::wrapped(
        buffers,
        CSR_FORWARD_OR_CHANGED_PARALLEL_WORKGROUP_SIZE,
        vec![Node::Region {
            generator: Ident::from(OP_ID),
            source_region: None,
            body: Arc::new(vec![Node::if_then(
                Expr::lt(src.clone(), Expr::u32(shape.node_count)),
                body,
            )]),
        }],
    ))
}
