//! GPU expression-shape graph construction builders.

#![allow(missing_docs)] // Internal VAST-builder helpers are documented at the owning module boundary.
use crate::parsing::c::lex::tokens::*;
use crate::parsing::composition::child_phase;
use crate::region::wrap_anonymous;
use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

use super::helpers::*;
use super::*;

pub fn c11_build_expression_shape_nodes(
    raw_vast_nodes: &str,
    typed_vast_nodes: &str,
    num_nodes: Expr,
    out_expr_shape_nodes: &str,
) -> Program {
    c11_build_expression_shape_nodes_impl(
        raw_vast_nodes,
        typed_vast_nodes,
        num_nodes,
        out_expr_shape_nodes,
        true,
    )
}

#[must_use]
pub fn c11_build_expression_shape_nodes_no_conditional(
    raw_vast_nodes: &str,
    typed_vast_nodes: &str,
    num_nodes: Expr,
    out_expr_shape_nodes: &str,
) -> Program {
    c11_build_expression_shape_nodes_impl(
        raw_vast_nodes,
        typed_vast_nodes,
        num_nodes,
        out_expr_shape_nodes,
        false,
    )
}

fn c11_build_expression_shape_nodes_impl(
    raw_vast_nodes: &str,
    typed_vast_nodes: &str,
    num_nodes: Expr,
    out_expr_shape_nodes: &str,
    include_conditional_shapes: bool,
) -> Program {
    let t = Expr::InvocationId { axis: 0 };
    let vast_base = Expr::mul(t.clone(), Expr::u32(VAST_NODE_STRIDE_U32));
    let out_base = Expr::mul(t.clone(), Expr::u32(C_EXPR_SHAPE_STRIDE_U32));

    let mut loop_body = vec![
        Node::let_bind("raw_kind", Expr::load(raw_vast_nodes, vast_base.clone())),
        Node::let_bind(
            "typed_kind",
            Expr::load(typed_vast_nodes, vast_base.clone()),
        ),
        Node::let_bind(
            "cur_parent",
            Expr::load(raw_vast_nodes, Expr::add(vast_base.clone(), Expr::u32(1))),
        ),
        Node::let_bind(
            "shape_kind",
            c_expr_shape_kind(Expr::var("raw_kind"), Expr::var("typed_kind")),
        ),
        Node::let_bind(
            "precedence",
            c_expr_operator_precedence(Expr::var("raw_kind"), Expr::var("typed_kind")),
        ),
        Node::let_bind(
            "associativity",
            c_expr_operator_associativity(Expr::var("typed_kind")),
        ),
        Node::let_bind(
            "shape_is_expr",
            Expr::ne(Expr::var("shape_kind"), Expr::u32(C_EXPR_SHAPE_NONE)),
        ),
        Node::let_bind(
            "shape_is_conditional",
            Expr::eq(Expr::var("shape_kind"), Expr::u32(C_EXPR_SHAPE_CONDITIONAL)),
        ),
    ];

    if include_conditional_shapes {
        loop_body.extend(emit_prior_ternary_boundary_flag(
            raw_vast_nodes,
            Expr::var("cur_parent"),
            t.clone(),
            Expr::var("shape_is_expr"),
            "bin",
        ));
        loop_body.extend(emit_expr_segment_bounds(
            raw_vast_nodes,
            Expr::var("cur_parent"),
            t.clone(),
            num_nodes.clone(),
            "bin_plain",
            false,
            Expr::var("shape_is_expr"),
        ));
        loop_body.extend(emit_expr_segment_bounds(
            raw_vast_nodes,
            Expr::var("cur_parent"),
            t.clone(),
            num_nodes.clone(),
            "bin_ternary",
            true,
            Expr::var("shape_is_expr"),
        ));
    } else {
        loop_body.extend(emit_expr_segment_bounds(
            raw_vast_nodes,
            Expr::var("cur_parent"),
            t.clone(),
            num_nodes.clone(),
            "bin_plain",
            false,
            Expr::var("shape_is_expr"),
        ));
        loop_body.extend([
            Node::let_bind("bin_use_ternary_boundaries", Expr::u32(0)),
            Node::let_bind("bin_ternary_seg_start", Expr::var("bin_plain_seg_start")),
            Node::let_bind("bin_ternary_seg_end", Expr::var("bin_plain_seg_end")),
        ]);
    }
    loop_body.extend(vec![
        Node::let_bind(
            "bin_seg_start",
            Expr::select(
                Expr::eq(Expr::var("bin_use_ternary_boundaries"), Expr::u32(1)),
                Expr::var("bin_ternary_seg_start"),
                Expr::var("bin_plain_seg_start"),
            ),
        ),
        Node::let_bind(
            "bin_seg_end",
            Expr::select(
                Expr::eq(Expr::var("bin_use_ternary_boundaries"), Expr::u32(1)),
                Expr::var("bin_ternary_seg_end"),
                Expr::var("bin_plain_seg_end"),
            ),
        ),
        Node::let_bind("bin_left_bound", Expr::var("bin_seg_start")),
        Node::let_bind("bin_right_bound", Expr::var("bin_seg_end")),
        Node::let_bind("bin_left_parent_op", Expr::u32(SENTINEL)),
        Node::let_bind("bin_right_parent_op", Expr::u32(SENTINEL)),
        Node::if_then(
            Expr::var("shape_is_expr"),
            vec![Node::loop_for(
                "bin_parent_scan",
                Expr::var("bin_seg_start"),
                Expr::var("bin_seg_end"),
                vec![
                    Node::let_bind(
                        "bin_parent_base",
                        Expr::mul(
                            Expr::var("bin_parent_scan"),
                            Expr::u32(VAST_NODE_STRIDE_U32),
                        ),
                    ),
                    Node::let_bind(
                        "bin_parent_raw",
                        Expr::load(raw_vast_nodes, Expr::var("bin_parent_base")),
                    ),
                    Node::let_bind(
                        "bin_parent_typed",
                        Expr::load(typed_vast_nodes, Expr::var("bin_parent_base")),
                    ),
                    Node::let_bind(
                        "bin_parent_parent",
                        Expr::load(
                            raw_vast_nodes,
                            Expr::add(Expr::var("bin_parent_base"), Expr::u32(1)),
                        ),
                    ),
                    Node::let_bind(
                        "bin_parent_shape",
                        c_expr_shape_kind(
                            Expr::var("bin_parent_raw"),
                            Expr::var("bin_parent_typed"),
                        ),
                    ),
                    Node::let_bind(
                        "bin_parent_prec",
                        c_expr_operator_precedence(
                            Expr::var("bin_parent_raw"),
                            Expr::var("bin_parent_typed"),
                        ),
                    ),
                    Node::let_bind(
                        "bin_parent_is_operator",
                        Expr::and(
                            Expr::ne(Expr::var("bin_parent_shape"), Expr::u32(C_EXPR_SHAPE_NONE)),
                            Expr::ne(Expr::var("bin_parent_scan"), t.clone()),
                        ),
                    ),
                    Node::let_bind(
                        "bin_parent_equal_assoc",
                        Expr::and(
                            Expr::eq(Expr::var("bin_parent_prec"), Expr::var("precedence")),
                            Expr::or(
                                Expr::and(
                                    Expr::eq(
                                        Expr::var("associativity"),
                                        Expr::u32(C_EXPR_ASSOC_LEFT),
                                    ),
                                    Expr::lt(t.clone(), Expr::var("bin_parent_scan")),
                                ),
                                Expr::and(
                                    Expr::eq(
                                        Expr::var("associativity"),
                                        Expr::u32(C_EXPR_ASSOC_RIGHT),
                                    ),
                                    Expr::lt(Expr::var("bin_parent_scan"), t.clone()),
                                ),
                            ),
                        ),
                    ),
                    Node::let_bind(
                        "bin_parent_is_ancestor",
                        Expr::and(
                            Expr::and(
                                Expr::eq(Expr::var("bin_parent_parent"), Expr::var("cur_parent")),
                                Expr::var("bin_parent_is_operator"),
                            ),
                            Expr::or(
                                Expr::lt(Expr::var("bin_parent_prec"), Expr::var("precedence")),
                                Expr::var("bin_parent_equal_assoc"),
                            ),
                        ),
                    ),
                    Node::if_then(
                        Expr::and(
                            Expr::var("bin_parent_is_ancestor"),
                            Expr::lt(Expr::var("bin_parent_scan"), t.clone()),
                        ),
                        vec![Node::assign(
                            "bin_left_parent_op",
                            Expr::var("bin_parent_scan"),
                        )],
                    ),
                    Node::if_then(
                        Expr::and(
                            Expr::var("bin_parent_is_ancestor"),
                            Expr::and(
                                Expr::lt(t.clone(), Expr::var("bin_parent_scan")),
                                Expr::or(
                                    Expr::eq(Expr::var("bin_right_parent_op"), Expr::u32(SENTINEL)),
                                    Expr::lt(
                                        Expr::var("bin_parent_scan"),
                                        Expr::var("bin_right_parent_op"),
                                    ),
                                ),
                            ),
                        ),
                        vec![Node::assign(
                            "bin_right_parent_op",
                            Expr::var("bin_parent_scan"),
                        )],
                    ),
                ],
            )],
        ),
        Node::if_then(
            Expr::ne(Expr::var("bin_left_parent_op"), Expr::u32(SENTINEL)),
            vec![Node::assign(
                "bin_left_bound",
                Expr::add(Expr::var("bin_left_parent_op"), Expr::u32(1)),
            )],
        ),
        Node::if_then(
            Expr::ne(Expr::var("bin_right_parent_op"), Expr::u32(SENTINEL)),
            vec![Node::assign(
                "bin_right_bound",
                Expr::var("bin_right_parent_op"),
            )],
        ),
    ]);
    loop_body.extend(emit_expr_root_scan(
        raw_vast_nodes,
        typed_vast_nodes,
        Expr::var("bin_left_bound"),
        t.clone(),
        Expr::var("cur_parent"),
        Expr::var("shape_is_expr"),
        "bin_lhs",
    ));
    loop_body.extend(emit_expr_root_scan(
        raw_vast_nodes,
        typed_vast_nodes,
        Expr::add(t.clone(), Expr::u32(1)),
        Expr::var("bin_right_bound"),
        Expr::var("cur_parent"),
        Expr::var("shape_is_expr"),
        "bin_rhs",
    ));

    if include_conditional_shapes {
        loop_body.extend(emit_expr_segment_bounds(
            raw_vast_nodes,
            Expr::var("cur_parent"),
            t.clone(),
            num_nodes.clone(),
            "cond",
            false,
            Expr::var("shape_is_conditional"),
        ));
        loop_body.extend(emit_expr_segment_bounds(
            raw_vast_nodes,
            Expr::var("cur_parent"),
            t.clone(),
            num_nodes.clone(),
            "cond_condition",
            true,
            Expr::var("shape_is_conditional"),
        ));
        loop_body.extend(vec![
            Node::let_bind("cond_colon", Expr::u32(SENTINEL)),
            Node::let_bind("cond_depth", Expr::u32(0)),
            Node::if_then(
                Expr::var("shape_is_conditional"),
                vec![Node::loop_for(
                    "cond_colon_scan",
                    Expr::add(t.clone(), Expr::u32(1)),
                    Expr::var("cond_seg_end"),
                    vec![Node::if_then(
                        Expr::eq(Expr::var("cond_colon"), Expr::u32(SENTINEL)),
                        vec![
                            Node::let_bind(
                                "cond_colon_base",
                                Expr::mul(
                                    Expr::var("cond_colon_scan"),
                                    Expr::u32(VAST_NODE_STRIDE_U32),
                                ),
                            ),
                            Node::let_bind(
                                "cond_colon_raw",
                                Expr::load(raw_vast_nodes, Expr::var("cond_colon_base")),
                            ),
                            Node::let_bind(
                                "cond_colon_parent",
                                Expr::load(
                                    raw_vast_nodes,
                                    Expr::add(Expr::var("cond_colon_base"), Expr::u32(1)),
                                ),
                            ),
                            Node::if_then(
                                Expr::eq(Expr::var("cond_colon_parent"), Expr::var("cur_parent")),
                                vec![
                                    Node::if_then(
                                        Expr::eq(
                                            Expr::var("cond_colon_raw"),
                                            Expr::u32(TOK_QUESTION),
                                        ),
                                        vec![Node::assign(
                                            "cond_depth",
                                            Expr::add(Expr::var("cond_depth"), Expr::u32(1)),
                                        )],
                                    ),
                                    Node::if_then(
                                        Expr::eq(Expr::var("cond_colon_raw"), Expr::u32(TOK_COLON)),
                                        vec![
                                            Node::if_then(
                                                Expr::eq(Expr::var("cond_depth"), Expr::u32(0)),
                                                vec![Node::assign(
                                                    "cond_colon",
                                                    Expr::var("cond_colon_scan"),
                                                )],
                                            ),
                                            Node::if_then(
                                                Expr::gt(Expr::var("cond_depth"), Expr::u32(0)),
                                                vec![Node::assign(
                                                    "cond_depth",
                                                    Expr::sub(
                                                        Expr::var("cond_depth"),
                                                        Expr::u32(1),
                                                    ),
                                                )],
                                            ),
                                        ],
                                    ),
                                ],
                            ),
                        ],
                    )],
                )],
            ),
            Node::let_bind(
                "cond_has_colon",
                Expr::ne(Expr::var("cond_colon"), Expr::u32(SENTINEL)),
            ),
            Node::let_bind(
                "cond_then_end",
                Expr::select(
                    Expr::var("cond_has_colon"),
                    Expr::var("cond_colon"),
                    Expr::add(t.clone(), Expr::u32(1)),
                ),
            ),
            Node::let_bind(
                "cond_else_start",
                Expr::select(
                    Expr::var("cond_has_colon"),
                    Expr::add(Expr::var("cond_colon"), Expr::u32(1)),
                    Expr::var("cond_seg_end"),
                ),
            ),
            Node::let_bind(
                "cond_condition_start",
                Expr::var("cond_condition_seg_start"),
            ),
            Node::let_bind("cond_parent_op", Expr::u32(SENTINEL)),
            Node::if_then(
                Expr::var("shape_is_conditional"),
                vec![Node::loop_for(
                    "cond_parent_scan",
                    Expr::var("cond_seg_start"),
                    t.clone(),
                    vec![
                        Node::let_bind(
                            "cond_parent_base",
                            Expr::mul(
                                Expr::var("cond_parent_scan"),
                                Expr::u32(VAST_NODE_STRIDE_U32),
                            ),
                        ),
                        Node::let_bind(
                            "cond_parent_raw",
                            Expr::load(raw_vast_nodes, Expr::var("cond_parent_base")),
                        ),
                        Node::let_bind(
                            "cond_parent_typed",
                            Expr::load(typed_vast_nodes, Expr::var("cond_parent_base")),
                        ),
                        Node::let_bind(
                            "cond_parent_parent",
                            Expr::load(
                                raw_vast_nodes,
                                Expr::add(Expr::var("cond_parent_base"), Expr::u32(1)),
                            ),
                        ),
                        Node::let_bind(
                            "cond_parent_shape",
                            c_expr_shape_kind(
                                Expr::var("cond_parent_raw"),
                                Expr::var("cond_parent_typed"),
                            ),
                        ),
                        Node::let_bind(
                            "cond_parent_prec",
                            c_expr_operator_precedence(
                                Expr::var("cond_parent_raw"),
                                Expr::var("cond_parent_typed"),
                            ),
                        ),
                        Node::if_then(
                            Expr::and(
                                Expr::eq(Expr::var("cond_parent_parent"), Expr::var("cur_parent")),
                                Expr::and(
                                    Expr::ne(
                                        Expr::var("cond_parent_shape"),
                                        Expr::u32(C_EXPR_SHAPE_NONE),
                                    ),
                                    Expr::lt(
                                        Expr::var("cond_parent_prec"),
                                        Expr::var("precedence"),
                                    ),
                                ),
                            ),
                            vec![Node::assign(
                                "cond_parent_op",
                                Expr::var("cond_parent_scan"),
                            )],
                        ),
                    ],
                )],
            ),
            Node::if_then(
                Expr::ne(Expr::var("cond_parent_op"), Expr::u32(SENTINEL)),
                vec![Node::assign(
                    "cond_condition_start",
                    Expr::add(Expr::var("cond_parent_op"), Expr::u32(1)),
                )],
            ),
        ]);
        loop_body.extend(emit_expr_root_scan(
            raw_vast_nodes,
            typed_vast_nodes,
            Expr::var("cond_condition_start"),
            t.clone(),
            Expr::var("cur_parent"),
            Expr::var("shape_is_conditional"),
            "cond_condition",
        ));
        loop_body.extend(emit_expr_root_scan(
            raw_vast_nodes,
            typed_vast_nodes,
            Expr::add(t.clone(), Expr::u32(1)),
            Expr::var("cond_then_end"),
            Expr::var("cur_parent"),
            Expr::var("shape_is_conditional"),
            "cond_then",
        ));
        loop_body.extend(emit_expr_root_scan(
            raw_vast_nodes,
            typed_vast_nodes,
            Expr::var("cond_else_start"),
            Expr::var("cond_seg_end"),
            Expr::var("cur_parent"),
            Expr::var("shape_is_conditional"),
            "cond_else",
        ));
    } else {
        loop_body.extend(vec![
            Node::let_bind("cond_condition_root", Expr::u32(SENTINEL)),
            Node::let_bind("cond_then_root", Expr::u32(SENTINEL)),
            Node::let_bind("cond_else_root", Expr::u32(SENTINEL)),
        ]);
    }

    loop_body.extend(vec![
        Node::let_bind(
            "field5",
            Expr::select(
                Expr::eq(Expr::var("shape_kind"), Expr::u32(C_EXPR_SHAPE_CONDITIONAL)),
                Expr::var("cond_condition_root"),
                Expr::var("bin_lhs_root"),
            ),
        ),
        Node::let_bind(
            "field6",
            Expr::select(
                Expr::eq(Expr::var("shape_kind"), Expr::u32(C_EXPR_SHAPE_CONDITIONAL)),
                Expr::var("cond_then_root"),
                Expr::var("bin_rhs_root"),
            ),
        ),
        Node::let_bind(
            "field7",
            Expr::select(
                Expr::eq(Expr::var("shape_kind"), Expr::u32(C_EXPR_SHAPE_CONDITIONAL)),
                Expr::var("cond_else_root"),
                Expr::u32(SENTINEL),
            ),
        ),
        Node::store(
            out_expr_shape_nodes,
            out_base.clone(),
            Expr::var("shape_kind"),
        ),
        Node::store(
            out_expr_shape_nodes,
            Expr::add(out_base.clone(), Expr::u32(1)),
            Expr::select(
                Expr::eq(Expr::var("shape_kind"), Expr::u32(C_EXPR_SHAPE_NONE)),
                Expr::u32(SENTINEL),
                t.clone(),
            ),
        ),
        Node::store(
            out_expr_shape_nodes,
            Expr::add(out_base.clone(), Expr::u32(2)),
            Expr::var("raw_kind"),
        ),
        Node::store(
            out_expr_shape_nodes,
            Expr::add(out_base.clone(), Expr::u32(3)),
            Expr::var("precedence"),
        ),
        Node::store(
            out_expr_shape_nodes,
            Expr::add(out_base.clone(), Expr::u32(4)),
            Expr::var("associativity"),
        ),
        Node::store(
            out_expr_shape_nodes,
            Expr::add(out_base.clone(), Expr::u32(5)),
            Expr::select(
                Expr::eq(Expr::var("shape_kind"), Expr::u32(C_EXPR_SHAPE_NONE)),
                Expr::u32(SENTINEL),
                Expr::var("field5"),
            ),
        ),
        Node::store(
            out_expr_shape_nodes,
            Expr::add(out_base.clone(), Expr::u32(6)),
            Expr::select(
                Expr::eq(Expr::var("shape_kind"), Expr::u32(C_EXPR_SHAPE_NONE)),
                Expr::u32(SENTINEL),
                Expr::var("field6"),
            ),
        ),
        Node::store(
            out_expr_shape_nodes,
            Expr::add(out_base, Expr::u32(7)),
            Expr::select(
                Expr::eq(Expr::var("shape_kind"), Expr::u32(C_EXPR_SHAPE_NONE)),
                Expr::u32(SENTINEL),
                Expr::var("field7"),
            ),
        ),
    ]);

    let n = node_count(&num_nodes).max(1);
    Program::wrapped(
        vec![
            BufferDecl::storage(raw_vast_nodes, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(n.saturating_mul(VAST_NODE_STRIDE_U32)),
            BufferDecl::storage(typed_vast_nodes, 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(n.saturating_mul(VAST_NODE_STRIDE_U32)),
            BufferDecl::output(out_expr_shape_nodes, 2, DataType::U32)
                .with_count(n.saturating_mul(C_EXPR_SHAPE_STRIDE_U32)),
        ],
        [256, 1, 1],
        vec![wrap_anonymous(
            EXPR_SHAPE_OP_ID,
            vec![Node::if_then(
                Expr::lt(t.clone(), num_nodes),
                vec![child_phase(
                    EXPR_SHAPE_OP_ID,
                    "vyre-libs::parsing::c11_build_expression_shape_nodes::node_shape_pass",
                    loop_body,
                )],
            )],
        )],
    )
    .with_entry_op_id(EXPR_SHAPE_OP_ID)
}

pub(super) fn u32_words_to_bytes(words: &[u32]) -> Vec<u8> {
    vyre_primitives::wire::pack_u32_slice(words)
}
