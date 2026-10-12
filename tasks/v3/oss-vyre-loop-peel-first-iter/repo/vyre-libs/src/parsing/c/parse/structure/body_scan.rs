use super::*;

pub(super) fn emit_body_open_scan(
    tok_types: &str,
    start_idx: Expr,
    num_tokens: Expr,
    out_var: &str,
) -> Vec<Node> {
    vec![
        Node::let_bind(out_var, Expr::u32(u32::MAX)),
        Node::let_bind("body_open_start", start_idx),
        Node::let_bind("body_open_scan_active", Expr::u32(1)),
        Node::let_bind("body_open_paren_depth", Expr::u32(0)),
        Node::let_bind("body_open_bracket_depth", Expr::u32(0)),
        Node::if_then(
            Expr::lt(Expr::var("body_open_start"), num_tokens.clone()),
            vec![Node::loop_for(
                "body_open_scan",
                Expr::var("body_open_start"),
                num_tokens,
                vec![
                    Node::let_bind(
                        "body_open_tok",
                        Expr::load(tok_types, Expr::var("body_open_scan")),
                    ),
                    Node::if_then(
                        Expr::and(
                            Expr::eq(Expr::var("body_open_scan_active"), Expr::u32(1)),
                            Expr::and(
                                Expr::eq(Expr::var("body_open_paren_depth"), Expr::u32(0)),
                                Expr::and(
                                    Expr::eq(Expr::var("body_open_bracket_depth"), Expr::u32(0)),
                                    Expr::eq(Expr::var("body_open_tok"), Expr::u32(TOK_LBRACE)),
                                ),
                            ),
                        ),
                        vec![
                            Node::assign(out_var, Expr::var("body_open_scan")),
                            Node::assign("body_open_scan_active", Expr::u32(0)),
                        ],
                    ),
                    Node::if_then(
                        Expr::and(
                            Expr::eq(Expr::var("body_open_scan_active"), Expr::u32(1)),
                            Expr::and(
                                Expr::eq(Expr::var("body_open_paren_depth"), Expr::u32(0)),
                                Expr::and(
                                    Expr::eq(Expr::var("body_open_bracket_depth"), Expr::u32(0)),
                                    Expr::eq(Expr::var("body_open_tok"), Expr::u32(TOK_SEMICOLON)),
                                ),
                            ),
                        ),
                        vec![Node::assign("body_open_scan_active", Expr::u32(0))],
                    ),
                    Node::if_then(
                        Expr::and(
                            Expr::eq(Expr::var("body_open_scan_active"), Expr::u32(1)),
                            Expr::eq(Expr::var("body_open_tok"), Expr::u32(TOK_LPAREN)),
                        ),
                        vec![Node::assign(
                            "body_open_paren_depth",
                            Expr::add(Expr::var("body_open_paren_depth"), Expr::u32(1)),
                        )],
                    ),
                    Node::if_then(
                        Expr::and(
                            Expr::eq(Expr::var("body_open_scan_active"), Expr::u32(1)),
                            Expr::and(
                                Expr::gt(Expr::var("body_open_paren_depth"), Expr::u32(0)),
                                Expr::eq(Expr::var("body_open_tok"), Expr::u32(TOK_RPAREN)),
                            ),
                        ),
                        vec![Node::assign(
                            "body_open_paren_depth",
                            Expr::sub(Expr::var("body_open_paren_depth"), Expr::u32(1)),
                        )],
                    ),
                    Node::if_then(
                        Expr::and(
                            Expr::eq(Expr::var("body_open_scan_active"), Expr::u32(1)),
                            Expr::eq(Expr::var("body_open_tok"), Expr::u32(TOK_LBRACKET)),
                        ),
                        vec![Node::assign(
                            "body_open_bracket_depth",
                            Expr::add(Expr::var("body_open_bracket_depth"), Expr::u32(1)),
                        )],
                    ),
                    Node::if_then(
                        Expr::and(
                            Expr::eq(Expr::var("body_open_scan_active"), Expr::u32(1)),
                            Expr::and(
                                Expr::gt(Expr::var("body_open_bracket_depth"), Expr::u32(0)),
                                Expr::eq(Expr::var("body_open_tok"), Expr::u32(TOK_RBRACKET)),
                            ),
                        ),
                        vec![Node::assign(
                            "body_open_bracket_depth",
                            Expr::sub(Expr::var("body_open_bracket_depth"), Expr::u32(1)),
                        )],
                    ),
                ],
            )],
        ),
    ]
}
