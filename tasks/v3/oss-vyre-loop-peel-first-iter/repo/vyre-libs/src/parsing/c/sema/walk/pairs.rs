use crate::parsing::c::lex::tokens::TOK_LBRACE;
use vyre::ir::{Expr, Node};

/// Resolve brace scopes using the precomputed `c11_dual_bracket_match` brace-pair table.
///
/// `brace_pairs[open]` is the matching close-brace index or `u32::MAX` for an
/// unmatched open. This keeps semantic scope recovery from redoing the full
/// close-brace depth walk for every token in the release pipeline.
pub fn emit_brace_scope_resolution_from_pairs(
    tok_types: &str,
    brace_pairs: &str,
    node_idx: Expr,
) -> Vec<Node> {
    vec![
        Node::loop_for(
            "scope_pair_scan",
            Expr::u32(0),
            node_idx.clone(),
            vec![
                Node::let_bind(
                    "scope_pair_idx",
                    Expr::sub(
                        Expr::sub(node_idx.clone(), Expr::u32(1)),
                        Expr::var("scope_pair_scan"),
                    ),
                ),
                Node::let_bind(
                    "scope_pair_tok",
                    Expr::load(tok_types, Expr::var("scope_pair_idx")),
                ),
                Node::if_then(
                    Expr::and(
                        Expr::eq(Expr::var("scope_open"), Expr::u32(u32::MAX)),
                        Expr::eq(Expr::var("scope_pair_tok"), Expr::u32(TOK_LBRACE)),
                    ),
                    vec![
                        Node::let_bind(
                            "scope_pair_close",
                            Expr::load(brace_pairs, Expr::var("scope_pair_idx")),
                        ),
                        Node::if_then(
                            Expr::or(
                                Expr::eq(Expr::var("scope_pair_close"), Expr::u32(u32::MAX)),
                                Expr::ge(Expr::var("scope_pair_close"), node_idx.clone()),
                            ),
                            vec![Node::assign("scope_open", Expr::var("scope_pair_idx"))],
                        ),
                    ],
                ),
            ],
        ),
        Node::if_then(
            Expr::ne(Expr::var("scope_open"), Expr::u32(u32::MAX)),
            vec![
                Node::assign("scope_id", Expr::add(Expr::var("scope_open"), Expr::u32(1))),
                Node::let_bind("scope_parent_open", Expr::u32(u32::MAX)),
                Node::if_then(
                    Expr::gt(Expr::var("scope_open"), Expr::u32(0)),
                    vec![Node::loop_for(
                        "scope_pair_parent_scan",
                        Expr::u32(0),
                        Expr::var("scope_open"),
                        vec![
                            Node::let_bind(
                                "scope_pair_parent_idx",
                                Expr::sub(
                                    Expr::sub(Expr::var("scope_open"), Expr::u32(1)),
                                    Expr::var("scope_pair_parent_scan"),
                                ),
                            ),
                            Node::let_bind(
                                "scope_pair_parent_tok",
                                Expr::load(tok_types, Expr::var("scope_pair_parent_idx")),
                            ),
                            Node::if_then(
                                Expr::and(
                                    Expr::eq(
                                        Expr::var("scope_parent_open"),
                                        Expr::u32(u32::MAX),
                                    ),
                                    Expr::eq(
                                        Expr::var("scope_pair_parent_tok"),
                                        Expr::u32(TOK_LBRACE),
                                    ),
                                ),
                                vec![
                                    Node::let_bind(
                                        "scope_pair_parent_close",
                                        Expr::load(brace_pairs, Expr::var("scope_pair_parent_idx")),
                                    ),
                                    Node::if_then(
                                        Expr::or(
                                            Expr::eq(
                                                Expr::var("scope_pair_parent_close"),
                                                Expr::u32(u32::MAX),
                                            ),
                                            Expr::ge(
                                                Expr::var("scope_pair_parent_close"),
                                                Expr::var("scope_open"),
                                            ),
                                        ),
                                        vec![Node::assign(
                                            "scope_parent_open",
                                            Expr::var("scope_pair_parent_idx"),
                                        )],
                                    ),
                                ],
                            ),
                        ],
                    )],
                ),
                Node::if_then(
                    Expr::ne(Expr::var("scope_parent_open"), Expr::u32(u32::MAX)),
                    vec![Node::assign(
                        "scope_parent_id",
                        Expr::add(Expr::var("scope_parent_open"), Expr::u32(1)),
                    )],
                ),
            ],
        ),
    ]
}
