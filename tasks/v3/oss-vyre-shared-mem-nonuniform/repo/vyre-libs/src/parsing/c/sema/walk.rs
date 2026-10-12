use super::predicates::function_name_prefix;
use super::scan::{
    emit_forward_matching_paren_scan, emit_post_paren_boundary_scan,
    emit_reverse_unmatched_lbrace_scan,
};
use crate::parsing::c::lex::tokens::*;
use vyre::ir::{Expr, Node};

/// Resolve the enclosing scope id and parent scope id for each token slot.
///
/// The logic is conservative and deterministic:
/// - `scope_id` is the nearest unmatched `{` before the node, plus one.
/// - `scope_parent_id` is the parent scope of that brace block, plus one.
/// - Global scope is `0`.
pub fn emit_scope_resolution(tok_types: &str, node_idx: Expr, _num_tokens: &Expr) -> Vec<Node> {
    let mut nodes = vec![
        Node::let_bind("scope_id", Expr::u32(0)),
        Node::let_bind("scope_parent_id", Expr::u32(0)),
        Node::let_bind("scope_open", Expr::u32(u32::MAX)),
        Node::let_bind("scope_depth", Expr::u32(0)),
    ];

    nodes.extend(emit_brace_scope_resolution(
        tok_types,
        node_idx.clone(),
        _num_tokens,
    ));
    nodes.extend(emit_function_parameter_scope(
        tok_types,
        node_idx,
        _num_tokens,
    ));

    nodes
}

/// Emits IR nodes for resolving scope boundaries at braces.
pub fn emit_brace_scope_resolution(
    tok_types: &str,
    node_idx: Expr,
    _num_tokens: &Expr,
) -> Vec<Node> {
    let mut nodes = emit_reverse_unmatched_lbrace_scan(
        tok_types,
        "scope_scan",
        "scan_idx",
        "scan_tok",
        "scope_depth",
        "scope_open",
        node_idx.clone(),
        None,
    );

    nodes.push(Node::if_then(
        Expr::ne(Expr::var("scope_open"), Expr::u32(u32::MAX)),
        vec![
            Node::assign("scope_id", Expr::add(Expr::var("scope_open"), Expr::u32(1))),
            Node::let_bind("scope_parent_open", Expr::u32(u32::MAX)),
            Node::let_bind("scope_parent_depth", Expr::u32(0)),
            Node::if_then(
                Expr::gt(Expr::var("scope_open"), Expr::u32(0)),
                emit_reverse_unmatched_lbrace_scan(
                    tok_types,
                    "scope_parent_scan",
                    "scope_parent_idx",
                    "scope_parent_tok",
                    "scope_parent_depth",
                    "scope_parent_open",
                    Expr::var("scope_open"),
                    None,
                ),
            ),
            Node::if_then(
                Expr::ne(Expr::var("scope_parent_open"), Expr::u32(u32::MAX)),
                vec![Node::assign(
                    "scope_parent_id",
                    Expr::add(Expr::var("scope_parent_open"), Expr::u32(1)),
                )],
            ),
        ],
    ));

    nodes.push(Node::if_then(
        Expr::eq(Expr::var("scope_open"), Expr::u32(u32::MAX)),
        vec![
            Node::assign("scope_id", Expr::u32(0)),
            Node::assign("scope_parent_id", Expr::u32(0)),
        ],
    ));

    nodes
}

/// Emits IR nodes for resolving scope boundaries of function parameters.
pub fn emit_function_parameter_scope(
    tok_types: &str,
    node_idx: Expr,
    num_tokens: &Expr,
) -> Vec<Node> {
    let mut nodes = vec![
        Node::let_bind("fn_param_lparen", Expr::u32(u32::MAX)),
        Node::let_bind("fn_param_depth", Expr::u32(0)),
    ];

    nodes.push(Node::loop_for(
        "fn_param_left_scan",
        Expr::u32(0),
        node_idx.clone(),
        vec![
            Node::let_bind(
                "fn_param_left_idx",
                Expr::sub(
                    Expr::sub(node_idx.clone(), Expr::u32(1)),
                    Expr::var("fn_param_left_scan"),
                ),
            ),
            Node::let_bind(
                "fn_param_left_tok",
                Expr::load(tok_types, Expr::var("fn_param_left_idx")),
            ),
            Node::if_then(
                Expr::eq(Expr::var("fn_param_left_tok"), Expr::u32(TOK_RPAREN)),
                vec![Node::assign(
                    "fn_param_depth",
                    Expr::add(Expr::var("fn_param_depth"), Expr::u32(1)),
                )],
            ),
            Node::if_then(
                Expr::eq(Expr::var("fn_param_lparen"), Expr::u32(u32::MAX)),
                vec![Node::if_then(
                    Expr::eq(Expr::var("fn_param_left_tok"), Expr::u32(TOK_LPAREN)),
                    vec![Node::if_then_else(
                        Expr::eq(Expr::var("fn_param_depth"), Expr::u32(0)),
                        vec![Node::assign(
                            "fn_param_lparen",
                            Expr::var("fn_param_left_idx"),
                        )],
                        vec![Node::assign(
                            "fn_param_depth",
                            Expr::sub(Expr::var("fn_param_depth"), Expr::u32(1)),
                        )],
                    )],
                )],
            ),
        ],
    ));

    nodes.push(Node::if_then(
        Expr::gt(Expr::var("fn_param_lparen"), Expr::u32(0)),
        vec![
            Node::let_bind(
                "fn_param_name_idx",
                Expr::sub(Expr::var("fn_param_lparen"), Expr::u32(1)),
            ),
            Node::let_bind(
                "fn_param_name_tok",
                Expr::load(tok_types, Expr::var("fn_param_name_idx")),
            ),
            Node::let_bind("fn_param_prefix_tok", Expr::u32(0)),
            Node::if_then(
                Expr::gt(Expr::var("fn_param_name_idx"), Expr::u32(0)),
                vec![Node::assign(
                    "fn_param_prefix_tok",
                    Expr::load(
                        tok_types,
                        Expr::sub(Expr::var("fn_param_name_idx"), Expr::u32(1)),
                    ),
                )],
            ),
            Node::if_then(
                Expr::and(
                    Expr::eq(Expr::var("fn_param_name_tok"), Expr::u32(TOK_IDENTIFIER)),
                    function_name_prefix(Expr::var("fn_param_prefix_tok")),
                ),
                emit_parameter_scope_from_lparen(tok_types, node_idx, num_tokens),
            ),
        ],
    ));

    nodes
}

fn emit_parameter_scope_from_lparen(
    tok_types: &str,
    node_idx: Expr,
    num_tokens: &Expr,
) -> Vec<Node> {
    let mut nodes = emit_forward_matching_paren_scan(
        tok_types,
        "fn_param_right_scan",
        Expr::add(Expr::var("fn_param_lparen"), Expr::u32(1)),
        num_tokens.clone(),
        "fn_param_right_tok",
        "fn_param_right_depth",
        "fn_param_rparen",
        None,
    );
    nodes.push(Node::if_then(
        Expr::and(
            Expr::ne(Expr::var("fn_param_rparen"), Expr::u32(u32::MAX)),
            Expr::lt(node_idx.clone(), Expr::var("fn_param_rparen")),
        ),
        emit_parameter_scope_boundary(tok_types, node_idx, num_tokens),
    ));
    nodes
}

fn emit_parameter_scope_boundary(tok_types: &str, node_idx: Expr, num_tokens: &Expr) -> Vec<Node> {
    let mut nodes = vec![Node::let_bind("fn_param_scope_open", Expr::u32(u32::MAX))];
    nodes.extend(emit_post_paren_boundary_scan(
        tok_types,
        "fn_param_boundary_scan",
        "fn_param_boundary_tok",
        "fn_param_boundary_active",
        Expr::var("fn_param_rparen"),
        num_tokens.clone(),
        "fn_param_boundary_found_tok",
        "fn_param_boundary_found_idx",
    ));
    nodes.extend([
        Node::if_then(
            Expr::ne(
                Expr::var("fn_param_boundary_found_tok"),
                Expr::u32(u32::MAX),
            ),
            vec![Node::if_then_else(
                Expr::eq(
                    Expr::var("fn_param_boundary_found_tok"),
                    Expr::u32(TOK_LBRACE),
                ),
                vec![Node::assign(
                    "fn_param_scope_open",
                    Expr::var("fn_param_boundary_found_idx"),
                )],
                vec![Node::assign(
                    "fn_param_scope_open",
                    Expr::var("fn_param_lparen"),
                )],
            )],
        ),
        Node::if_then(
            Expr::ne(Expr::var("fn_param_scope_open"), Expr::u32(u32::MAX)),
            vec![
                Node::let_bind("fn_param_parent_scope_id", Expr::var("scope_id")),
                Node::let_bind("fn_param_parent_pending_brace", Expr::u32(0)),
                Node::let_bind("fn_param_parent_pending_close", Expr::u32(0)),
                Node::let_bind(
                    "fn_param_parent_scan_start",
                    Expr::add(node_idx.clone(), Expr::u32(1)),
                ),
                Node::if_then(
                    Expr::lt(
                        Expr::var("fn_param_parent_scan_start"),
                        Expr::var("fn_param_scope_open"),
                    ),
                    vec![Node::loop_for(
                        "fn_param_parent_scan",
                        Expr::var("fn_param_parent_scan_start"),
                        Expr::var("fn_param_scope_open"),
                        vec![
                            Node::let_bind(
                                "fn_param_parent_scan_tok",
                                Expr::load(tok_types, Expr::var("fn_param_parent_scan")),
                            ),
                            Node::if_then(
                                Expr::eq(
                                    Expr::var("fn_param_parent_scan_tok"),
                                    Expr::u32(TOK_LBRACE),
                                ),
                                vec![Node::assign("fn_param_parent_pending_brace", Expr::u32(1))],
                            ),
                            Node::if_then(
                                Expr::eq(
                                    Expr::var("fn_param_parent_scan_tok"),
                                    Expr::u32(TOK_RBRACE),
                                ),
                                vec![Node::assign("fn_param_parent_pending_close", Expr::u32(1))],
                            ),
                        ],
                    )],
                ),
                Node::if_then(
                    Expr::or(
                        Expr::or(
                            Expr::eq(Expr::load(tok_types, node_idx), Expr::u32(TOK_LBRACE)),
                            Expr::eq(Expr::var("fn_param_parent_pending_brace"), Expr::u32(1)),
                        ),
                        Expr::eq(Expr::var("fn_param_parent_pending_close"), Expr::u32(1)),
                    ),
                    vec![Node::assign(
                        "fn_param_parent_scope_id",
                        Expr::var("scope_parent_id"),
                    )],
                ),
                Node::assign(
                    "scope_id",
                    Expr::add(Expr::var("fn_param_scope_open"), Expr::u32(1)),
                ),
                Node::assign("scope_parent_id", Expr::var("fn_param_parent_scope_id")),
            ],
        ),
    ]);
    nodes
}
