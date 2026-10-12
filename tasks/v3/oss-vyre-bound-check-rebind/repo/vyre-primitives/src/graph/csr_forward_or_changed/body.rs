use std::sync::Arc;

use vyre_foundation::ir::model::expr::{GeneratorRef, Ident};
use vyre_foundation::ir::{Expr, Node};

use super::layout::OP_ID;
use crate::graph::program_graph::{
    ProgramGraphShape, NAME_EDGE_KIND_MASK, NAME_EDGE_OFFSETS, NAME_EDGE_TARGETS,
};

/// Build one in-place forward expansion pass over an accumulating frontier.
#[must_use]
pub fn csr_forward_or_changed_body(
    shape: ProgramGraphShape,
    frontier_out: &str,
    changed_var: &str,
    edge_kind_mask: u32,
) -> Vec<Node> {
    csr_forward_or_changed_body_prefixed(shape, frontier_out, changed_var, edge_kind_mask, "")
}

fn local(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        name.to_string()
    } else {
        format!("{prefix}_{name}")
    }
}

/// Build one traversal pass with caller-provided local-name prefixing for
/// repeated inlining under validators that disallow shadowing.
#[must_use]
pub fn csr_forward_or_changed_body_prefixed(
    shape: ProgramGraphShape,
    frontier_out: &str,
    changed_var: &str,
    edge_kind_mask: u32,
    prefix: &str,
) -> Vec<Node> {
    let src = local(prefix, "src");
    let word_idx = local(prefix, "word_idx");
    let bit_mask = local(prefix, "bit_mask");
    let src_word = local(prefix, "src_word");
    let edge_start = local(prefix, "edge_start");
    let edge_end = local(prefix, "edge_end");
    let edge_iter = local(prefix, "e");
    let kind_mask = local(prefix, "kind_mask");
    let dst = local(prefix, "dst");
    let dst_word_idx = local(prefix, "dst_word_idx");
    let dst_bit = local(prefix, "dst_bit");
    let old = local(prefix, "old");

    let per_source = vec![
        Node::let_bind(
            word_idx.as_str(),
            Expr::shr(Expr::var(src.as_str()), Expr::u32(5)),
        ),
        Node::let_bind(
            bit_mask.as_str(),
            Expr::shl(
                Expr::u32(1),
                Expr::bitand(Expr::var(src.as_str()), Expr::u32(31)),
            ),
        ),
        Node::let_bind(
            src_word.as_str(),
            Expr::load(frontier_out, Expr::var(word_idx.as_str())),
        ),
        Node::if_then(
            Expr::ne(
                Expr::bitand(Expr::var(src_word.as_str()), Expr::var(bit_mask.as_str())),
                Expr::u32(0),
            ),
            vec![
                Node::let_bind(
                    edge_start.as_str(),
                    Expr::load(NAME_EDGE_OFFSETS, Expr::var(src.as_str())),
                ),
                Node::let_bind(
                    edge_end.as_str(),
                    Expr::load(
                        NAME_EDGE_OFFSETS,
                        Expr::add(Expr::var(src.as_str()), Expr::u32(1)),
                    ),
                ),
                Node::loop_for(
                    edge_iter.as_str(),
                    Expr::var(edge_start.as_str()),
                    Expr::var(edge_end.as_str()),
                    vec![
                        Node::let_bind(
                            kind_mask.as_str(),
                            Expr::load(NAME_EDGE_KIND_MASK, Expr::var(edge_iter.as_str())),
                        ),
                        Node::if_then(
                            Expr::ne(
                                Expr::bitand(
                                    Expr::var(kind_mask.as_str()),
                                    Expr::u32(edge_kind_mask),
                                ),
                                Expr::u32(0),
                            ),
                            vec![
                                Node::let_bind(
                                    dst.as_str(),
                                    Expr::load(NAME_EDGE_TARGETS, Expr::var(edge_iter.as_str())),
                                ),
                                Node::if_then(
                                    Expr::lt(Expr::var(dst.as_str()), Expr::u32(shape.node_count)),
                                    vec![
                                        Node::let_bind(
                                            dst_word_idx.as_str(),
                                            Expr::shr(Expr::var(dst.as_str()), Expr::u32(5)),
                                        ),
                                        Node::let_bind(
                                            dst_bit.as_str(),
                                            Expr::shl(
                                                Expr::u32(1),
                                                Expr::bitand(
                                                    Expr::var(dst.as_str()),
                                                    Expr::u32(31),
                                                ),
                                            ),
                                        ),
                                        Node::let_bind(
                                            old.as_str(),
                                            Expr::atomic_or(
                                                frontier_out,
                                                Expr::var(dst_word_idx.as_str()),
                                                Expr::var(dst_bit.as_str()),
                                            ),
                                        ),
                                        Node::if_then(
                                            Expr::eq(
                                                Expr::bitand(
                                                    Expr::var(old.as_str()),
                                                    Expr::var(dst_bit.as_str()),
                                                ),
                                                Expr::u32(0),
                                            ),
                                            vec![Node::assign(changed_var, Expr::u32(1))],
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

    vec![Node::if_then(
        Expr::eq(Expr::local_x(), Expr::u32(0)),
        vec![Node::loop_for(
            src.as_str(),
            Expr::u32(0),
            Expr::u32(shape.node_count),
            per_source,
        )],
    )]
}

/// Wrap one traversal pass as a child Region of `parent_op_id`.
#[must_use]
pub fn csr_forward_or_changed_child(
    parent_op_id: &str,
    shape: ProgramGraphShape,
    frontier_out: &str,
    changed_var: &str,
    edge_kind_mask: u32,
) -> Node {
    csr_forward_or_changed_child_prefixed(
        parent_op_id,
        shape,
        frontier_out,
        changed_var,
        edge_kind_mask,
        "",
    )
}

/// Wrap a traversal pass with a local-name prefix for repeated inlining.
#[must_use]
pub fn csr_forward_or_changed_child_prefixed(
    parent_op_id: &str,
    shape: ProgramGraphShape,
    frontier_out: &str,
    changed_var: &str,
    edge_kind_mask: u32,
    local_prefix: &str,
) -> Node {
    Node::Region {
        generator: Ident::from(OP_ID),
        source_region: Some(GeneratorRef {
            name: parent_op_id.to_string(),
        }),
        body: Arc::new(csr_forward_or_changed_body_prefixed(
            shape,
            frontier_out,
            changed_var,
            edge_kind_mask,
            local_prefix,
        )),
    }
}
