//! Function-like macro argument scan builders.

use crate::parsing::c::lex::tokens::*;
use vyre::ir::{Expr, Node};

use super::helpers::*;

pub(super) fn emit_function_like_argument_scan(
    in_tok_types: &str,
    macro_arg_starts: &str,
    macro_arg_ends: &str,
    num_tokens: Expr,
) -> Vec<Node> {
    let mut nodes = vec![
        Node::if_then(
            Expr::gt(Expr::var("named_param_count"), num_tokens.clone()),
            vec![Node::trap(
                Expr::var("named_param_count"),
                "function-like-macro-parameter-count-exceeds-token-capacity",
            )],
        ),
        Node::let_bind(
            "macro_scan_base",
            Expr::add(Expr::var("named_i"), Expr::u32(2)),
        ),
        Node::let_bind("macro_depth", Expr::u32(0)),
        Node::let_bind("macro_arg_index", Expr::u32(0)),
        Node::let_bind("macro_current_arg_start", Expr::var("macro_scan_base")),
        Node::let_bind("macro_found_close", Expr::u32(0)),
        Node::let_bind("macro_close_idx", num_tokens.clone()),
        Node::store(macro_arg_starts, Expr::u32(0), Expr::var("macro_scan_base")),
        Node::store(macro_arg_ends, Expr::u32(0), Expr::var("macro_scan_base")),
    ];
    let scan_body = vec![
        Node::let_bind(
            "macro_scan_idx",
            Expr::add(Expr::var("macro_scan_base"), Expr::var("macro_scan_rel")),
        ),
        Node::if_then(
            Expr::and(
                Expr::eq(Expr::var("macro_found_close"), Expr::u32(0)),
                Expr::ge(Expr::var("macro_scan_idx"), num_tokens.clone()),
            ),
            vec![Node::trap(
                Expr::var("macro_scan_idx"),
                "function-like-macro-invocation-missing-rparen",
            )],
        ),
        Node::if_then(
            Expr::and(
                Expr::eq(Expr::var("macro_found_close"), Expr::u32(0)),
                Expr::lt(Expr::var("macro_scan_idx"), num_tokens.clone()),
            ),
            vec![
                Node::let_bind(
                    "macro_scan_tok",
                    Expr::load(in_tok_types, Expr::var("macro_scan_idx")),
                ),
                Node::if_then(
                    Expr::eq(Expr::var("macro_scan_tok"), Expr::u32(TOK_LPAREN)),
                    vec![Node::assign(
                        "macro_depth",
                        Expr::add(Expr::var("macro_depth"), Expr::u32(1)),
                    )],
                ),
                Node::if_then(
                    Expr::and(
                        Expr::eq(Expr::var("macro_scan_tok"), Expr::u32(TOK_COMMA)),
                        Expr::eq(Expr::var("macro_depth"), Expr::u32(0)),
                    ),
                    {
                        let mut comma = assign_arg_bound(
                            macro_arg_ends,
                            Expr::var("macro_arg_index"),
                            Expr::var("macro_scan_idx"),
                            num_tokens.clone(),
                            "function-like-macro-argument-count-overflow",
                        );
                        comma.extend([
                            Node::let_bind(
                                "macro_next_arg_index",
                                Expr::add(Expr::var("macro_arg_index"), Expr::u32(1)),
                            ),
                            Node::if_then(
                                Expr::ge(Expr::var("macro_next_arg_index"), num_tokens.clone()),
                                vec![Node::trap(
                                    Expr::var("macro_next_arg_index"),
                                    "function-like-macro-argument-count-overflow",
                                )],
                            ),
                            Node::assign(
                                "macro_current_arg_start",
                                Expr::add(Expr::var("macro_scan_idx"), Expr::u32(1)),
                            ),
                            Node::assign("macro_arg_index", Expr::var("macro_next_arg_index")),
                        ]);
                        comma.extend(assign_arg_bound(
                            macro_arg_starts,
                            Expr::var("macro_next_arg_index"),
                            Expr::var("macro_current_arg_start"),
                            num_tokens.clone(),
                            "function-like-macro-argument-count-overflow",
                        ));
                        comma
                    },
                ),
                Node::if_then(
                    Expr::eq(Expr::var("macro_scan_tok"), Expr::u32(TOK_RPAREN)),
                    vec![Node::if_then_else(
                        Expr::eq(Expr::var("macro_depth"), Expr::u32(0)),
                        {
                            let mut close = assign_arg_bound(
                                macro_arg_ends,
                                Expr::var("macro_arg_index"),
                                Expr::var("macro_scan_idx"),
                                num_tokens.clone(),
                                "function-like-macro-argument-count-overflow",
                            );
                            close.extend([
                                Node::assign("macro_found_close", Expr::u32(1)),
                                Node::assign("macro_close_idx", Expr::var("macro_scan_idx")),
                            ]);
                            close
                        },
                        vec![Node::assign(
                            "macro_depth",
                            Expr::sub(Expr::var("macro_depth"), Expr::u32(1)),
                        )],
                    )],
                ),
            ],
        ),
    ];
    nodes.push(Node::loop_for(
        "macro_scan_rel",
        Expr::u32(0),
        num_tokens.clone(),
        scan_body,
    ));
    nodes.extend([
        Node::if_then(
            Expr::eq(Expr::var("macro_found_close"), Expr::u32(0)),
            vec![Node::trap(
                Expr::var("named_i"),
                "function-like-macro-invocation-missing-rparen",
            )],
        ),
        Node::let_bind(
            "macro_seen_arg_count",
            Expr::add(Expr::var("macro_arg_index"), Expr::u32(1)),
        ),
        Node::if_then(
            Expr::and(
                Expr::and(
                    Expr::eq(Expr::var("macro_close_idx"), Expr::var("macro_scan_base")),
                    Expr::eq(Expr::var("named_required_param_count"), Expr::u32(0)),
                ),
                Expr::eq(Expr::var("macro_arg_index"), Expr::u32(0)),
            ),
            vec![Node::assign("macro_seen_arg_count", Expr::u32(0))],
        ),
        Node::if_then(
            Expr::and(
                Expr::ne(
                    Expr::var("macro_seen_arg_count"),
                    Expr::var("named_param_count"),
                ),
                Expr::not(Expr::and(
                    Expr::eq(Expr::var("named_is_variadic"), Expr::u32(1)),
                    Expr::eq(
                        Expr::var("macro_seen_arg_count"),
                        Expr::var("named_required_param_count"),
                    ),
                )),
            ),
            vec![Node::trap(
                Expr::var("macro_seen_arg_count"),
                "function-like-macro-argument-count-mismatch",
            )],
        ),
        Node::if_then(
            Expr::and(
                Expr::eq(Expr::var("named_is_variadic"), Expr::u32(1)),
                Expr::eq(
                    Expr::var("macro_seen_arg_count"),
                    Expr::var("named_required_param_count"),
                ),
            ),
            vec![
                Node::store(
                    macro_arg_starts,
                    Expr::var("named_required_param_count"),
                    Expr::var("macro_close_idx"),
                ),
                Node::store(
                    macro_arg_ends,
                    Expr::var("named_required_param_count"),
                    Expr::var("macro_close_idx"),
                ),
            ],
        ),
    ]);
    nodes
}
