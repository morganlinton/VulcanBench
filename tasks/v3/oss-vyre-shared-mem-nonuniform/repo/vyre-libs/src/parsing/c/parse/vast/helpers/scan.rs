use super::super::*;
use super::*;
use crate::parsing::c::lex::tokens::*;
use vyre::ir::{Expr, Node};

pub(crate) fn emit_prior_ternary_boundary_flag(
    raw_vast_nodes: &str,
    _parent_expr: Expr,
    target: Expr,
    active: Expr,
    prefix: &str,
) -> Vec<Node> {
    let flag = format!("{prefix}_use_ternary_boundaries");
    let stop = format!("{prefix}_ternary_boundary_stop");
    let scan = format!("{prefix}_ternary_boundary_scan");
    let cursor = format!("{prefix}_ternary_boundary_cursor");
    let cursor_valid = format!("{prefix}_ternary_boundary_cursor_valid");
    let safe_cursor = format!("{prefix}_ternary_boundary_safe_cursor");
    let base = format!("{prefix}_ternary_boundary_base");
    let raw = format!("{prefix}_ternary_boundary_raw");
    let is_prior_ternary = format!("{prefix}_is_prior_ternary_boundary");

    vec![
        Node::let_bind(&flag, Expr::u32(0)),
        Node::let_bind(&stop, Expr::u32(0)),
        Node::let_bind(
            &cursor,
            Expr::load(
                raw_vast_nodes,
                Expr::add(
                    Expr::mul(target.clone(), Expr::u32(VAST_NODE_STRIDE_U32)),
                    Expr::u32(VAST_PREVIOUS_SIBLING_FIELD),
                ),
            ),
        ),
        Node::if_then(
            active,
            vec![Node::loop_for(
                &scan,
                Expr::u32(0),
                target.clone(),
                vec![Node::if_then(
                    Expr::eq(Expr::var(&stop), Expr::u32(0)),
                    vec![
                        Node::let_bind(&cursor_valid, Expr::lt(Expr::var(&cursor), target.clone())),
                        Node::let_bind(
                            &safe_cursor,
                            Expr::select(
                                Expr::var(&cursor_valid),
                                Expr::var(&cursor),
                                Expr::u32(0),
                            ),
                        ),
                        Node::let_bind(
                            &base,
                            Expr::mul(Expr::var(&safe_cursor), Expr::u32(VAST_NODE_STRIDE_U32)),
                        ),
                        Node::let_bind(&raw, Expr::load(raw_vast_nodes, Expr::var(&base))),
                        Node::if_then(
                            Expr::and(
                                Expr::var(&cursor_valid),
                                is_expr_shape_boundary(Expr::var(&raw), true),
                            ),
                            vec![
                                Node::let_bind(
                                    &is_prior_ternary,
                                    Expr::or(
                                        Expr::eq(Expr::var(&raw), Expr::u32(TOK_QUESTION)),
                                        Expr::eq(Expr::var(&raw), Expr::u32(TOK_COLON)),
                                    ),
                                ),
                                Node::if_then(
                                    Expr::var(&is_prior_ternary),
                                    vec![Node::assign(&flag, Expr::u32(1))],
                                ),
                                Node::assign(&stop, Expr::u32(1)),
                            ],
                        ),
                        Node::if_then_else(
                            Expr::var(&cursor_valid),
                            vec![Node::assign(
                                &cursor,
                                Expr::load(
                                    raw_vast_nodes,
                                    Expr::add(
                                        Expr::var(&base),
                                        Expr::u32(VAST_PREVIOUS_SIBLING_FIELD),
                                    ),
                                ),
                            )],
                            vec![Node::assign(&stop, Expr::u32(1))],
                        ),
                    ],
                )],
            )],
        ),
    ]
}

pub(crate) fn emit_expr_segment_bounds(
    raw_vast_nodes: &str,
    _parent_expr: Expr,
    target: Expr,
    num_nodes: Expr,
    prefix: &str,
    include_ternary_parts: bool,
    active: Expr,
) -> Vec<Node> {
    let start = format!("{prefix}_seg_start");
    let end = format!("{prefix}_seg_end");
    let scan = format!("{prefix}_seg_scan");
    let cursor = format!("{prefix}_seg_cursor");
    let cursor_valid = format!("{prefix}_seg_cursor_valid");
    let safe_cursor = format!("{prefix}_seg_safe_cursor");
    let base = format!("{prefix}_seg_base");
    let raw = format!("{prefix}_seg_raw");
    let seen_left_boundary = format!("{prefix}_seen_left_boundary");
    let seen_right_boundary = format!("{prefix}_seen_right_boundary");

    vec![
        Node::let_bind(&start, Expr::u32(0)),
        Node::let_bind(&end, num_nodes.clone()),
        Node::let_bind(&seen_left_boundary, Expr::u32(0)),
        Node::let_bind(
            &cursor,
            Expr::load(
                raw_vast_nodes,
                Expr::add(
                    Expr::mul(target.clone(), Expr::u32(VAST_NODE_STRIDE_U32)),
                    Expr::u32(VAST_PREVIOUS_SIBLING_FIELD),
                ),
            ),
        ),
        Node::if_then(
            active.clone(),
            vec![Node::loop_for(
                &scan,
                Expr::u32(0),
                target.clone(),
                vec![Node::if_then(
                    Expr::eq(Expr::var(&seen_left_boundary), Expr::u32(0)),
                    vec![
                        Node::let_bind(&cursor_valid, Expr::lt(Expr::var(&cursor), target.clone())),
                        Node::let_bind(
                            &safe_cursor,
                            Expr::select(
                                Expr::var(&cursor_valid),
                                Expr::var(&cursor),
                                Expr::u32(0),
                            ),
                        ),
                        Node::let_bind(
                            &base,
                            Expr::mul(Expr::var(&safe_cursor), Expr::u32(VAST_NODE_STRIDE_U32)),
                        ),
                        Node::let_bind(&raw, Expr::load(raw_vast_nodes, Expr::var(&base))),
                        Node::if_then(
                            Expr::and(
                                Expr::var(&cursor_valid),
                                is_expr_shape_boundary(Expr::var(&raw), include_ternary_parts),
                            ),
                            vec![
                                Node::assign(&start, Expr::add(Expr::var(&cursor), Expr::u32(1))),
                                Node::assign(&seen_left_boundary, Expr::u32(1)),
                            ],
                        ),
                        Node::if_then_else(
                            Expr::var(&cursor_valid),
                            vec![Node::assign(
                                &cursor,
                                Expr::load(
                                    raw_vast_nodes,
                                    Expr::add(
                                        Expr::var(&base),
                                        Expr::u32(VAST_PREVIOUS_SIBLING_FIELD),
                                    ),
                                ),
                            )],
                            vec![Node::assign(&seen_left_boundary, Expr::u32(1))],
                        ),
                    ],
                )],
            )],
        ),
        Node::let_bind(&seen_right_boundary, Expr::u32(0)),
        Node::assign(
            &cursor,
            Expr::load(
                raw_vast_nodes,
                Expr::add(
                    Expr::mul(target.clone(), Expr::u32(VAST_NODE_STRIDE_U32)),
                    Expr::u32(3),
                ),
            ),
        ),
        Node::if_then(
            active,
            vec![Node::loop_for(
                &scan,
                Expr::add(target, Expr::u32(1)),
                num_nodes,
                vec![Node::if_then(
                    Expr::eq(Expr::var(&seen_right_boundary), Expr::u32(0)),
                    vec![
                        Node::let_bind(
                            &cursor_valid,
                            Expr::lt(Expr::var(&cursor), Expr::var(&end)),
                        ),
                        Node::let_bind(
                            &safe_cursor,
                            Expr::select(
                                Expr::var(&cursor_valid),
                                Expr::var(&cursor),
                                Expr::u32(0),
                            ),
                        ),
                        Node::let_bind(
                            &base,
                            Expr::mul(Expr::var(&safe_cursor), Expr::u32(VAST_NODE_STRIDE_U32)),
                        ),
                        Node::let_bind(&raw, Expr::load(raw_vast_nodes, Expr::var(&base))),
                        Node::if_then(
                            Expr::and(
                                Expr::var(&cursor_valid),
                                is_expr_shape_boundary(Expr::var(&raw), include_ternary_parts),
                            ),
                            vec![
                                Node::assign(&end, Expr::var(&cursor)),
                                Node::assign(&seen_right_boundary, Expr::u32(1)),
                            ],
                        ),
                        Node::if_then_else(
                            Expr::var(&cursor_valid),
                            vec![Node::assign(
                                &cursor,
                                Expr::load(
                                    raw_vast_nodes,
                                    Expr::add(Expr::var(&base), Expr::u32(3)),
                                ),
                            )],
                            vec![Node::assign(&seen_right_boundary, Expr::u32(1))],
                        ),
                    ],
                )],
            )],
        ),
    ]
}

pub(crate) fn emit_expr_root_scan(
    raw_vast_nodes: &str,
    typed_vast_nodes: &str,
    lo: Expr,
    hi: Expr,
    parent_expr: Expr,
    active: Expr,
    prefix: &str,
) -> Vec<Node> {
    let root = format!("{prefix}_root");
    let root_prec = format!("{prefix}_root_prec");
    let operand = format!("{prefix}_operand");
    let scan = format!("{prefix}_scan");
    let base = format!("{prefix}_base");
    let raw = format!("{prefix}_raw");
    let typed = format!("{prefix}_typed");
    let parent = format!("{prefix}_parent");
    let shape = format!("{prefix}_shape");
    let prec = format!("{prefix}_prec");
    let assoc = format!("{prefix}_assoc");
    let is_operator = format!("{prefix}_is_operator");
    let replace_root = format!("{prefix}_replace_root");

    vec![
        Node::let_bind(&root, Expr::u32(SENTINEL)),
        Node::let_bind(&root_prec, Expr::u32(u32::MAX)),
        Node::let_bind(&operand, Expr::u32(SENTINEL)),
        Node::if_then(
            active,
            vec![Node::loop_for(
                &scan,
                lo,
                hi,
                vec![
                    Node::let_bind(
                        &base,
                        Expr::mul(Expr::var(&scan), Expr::u32(VAST_NODE_STRIDE_U32)),
                    ),
                    Node::let_bind(&raw, Expr::load(raw_vast_nodes, Expr::var(&base))),
                    Node::let_bind(&typed, Expr::load(typed_vast_nodes, Expr::var(&base))),
                    Node::let_bind(
                        &parent,
                        Expr::load(raw_vast_nodes, Expr::add(Expr::var(&base), Expr::u32(1))),
                    ),
                    Node::let_bind(
                        &shape,
                        c_expr_shape_kind(Expr::var(&raw), Expr::var(&typed)),
                    ),
                    Node::let_bind(
                        &prec,
                        c_expr_operator_precedence(Expr::var(&raw), Expr::var(&typed)),
                    ),
                    Node::let_bind(&assoc, c_expr_operator_associativity(Expr::var(&typed))),
                    Node::let_bind(
                        &is_operator,
                        Expr::ne(Expr::var(&shape), Expr::u32(C_EXPR_SHAPE_NONE)),
                    ),
                    Node::if_then(
                        Expr::or(
                            Expr::eq(Expr::var(&parent), parent_expr.clone()),
                            Expr::var(&is_operator),
                        ),
                        vec![
                            Node::if_then(
                                Expr::and(
                                    Expr::eq(Expr::var(&operand), Expr::u32(SENTINEL)),
                                    Expr::and(
                                        Expr::eq(Expr::var(&parent), parent_expr.clone()),
                                        Expr::and(
                                            Expr::not(Expr::var(&is_operator)),
                                            Expr::not(is_expr_shape_boundary(
                                                Expr::var(&raw),
                                                true,
                                            )),
                                        ),
                                    ),
                                ),
                                vec![Node::assign(&operand, Expr::var(&scan))],
                            ),
                            Node::let_bind(
                                &replace_root,
                                Expr::or(
                                    Expr::eq(Expr::var(&root), Expr::u32(SENTINEL)),
                                    Expr::or(
                                        Expr::lt(Expr::var(&prec), Expr::var(&root_prec)),
                                        Expr::and(
                                            Expr::eq(Expr::var(&prec), Expr::var(&root_prec)),
                                            Expr::eq(
                                                Expr::var(&assoc),
                                                Expr::u32(C_EXPR_ASSOC_LEFT),
                                            ),
                                        ),
                                    ),
                                ),
                            ),
                            Node::if_then(
                                Expr::and(Expr::var(&is_operator), Expr::var(&replace_root)),
                                vec![
                                    Node::assign(&root, Expr::var(&scan)),
                                    Node::assign(&root_prec, Expr::var(&prec)),
                                ],
                            ),
                        ],
                    ),
                ],
            )],
        ),
        Node::if_then(
            Expr::eq(Expr::var(&root), Expr::u32(SENTINEL)),
            vec![Node::assign(&root, Expr::var(&operand))],
        ),
    ]
}
