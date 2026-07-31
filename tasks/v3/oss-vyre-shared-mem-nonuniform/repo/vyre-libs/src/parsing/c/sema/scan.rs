use crate::parsing::c::lex::tokens::*;
use vyre::ir::{Expr, Node};

pub(super) fn emit_forward_matching_paren_scan(
    tok_types: &str,
    scan_index_name: &str,
    scan_start: Expr,
    scan_end: Expr,
    scan_token_name: &str,
    depth_name: &str,
    out_match_name: &str,
    guard: Option<Expr>,
) -> Vec<Node> {
    let loop_node = Node::loop_for(
        scan_index_name,
        scan_start,
        scan_end,
        vec![
            Node::let_bind(
                scan_token_name,
                Expr::load(tok_types, Expr::var(scan_index_name)),
            ),
            Node::if_then(
                Expr::eq(Expr::var(scan_token_name), Expr::u32(TOK_LPAREN)),
                vec![Node::assign(
                    depth_name,
                    Expr::add(Expr::var(depth_name), Expr::u32(1)),
                )],
            ),
            Node::if_then(
                Expr::and(
                    Expr::eq(Expr::var(out_match_name), Expr::u32(u32::MAX)),
                    Expr::eq(Expr::var(scan_token_name), Expr::u32(TOK_RPAREN)),
                ),
                vec![Node::if_then_else(
                    Expr::eq(Expr::var(depth_name), Expr::u32(1)),
                    vec![Node::assign(out_match_name, Expr::var(scan_index_name))],
                    vec![Node::assign(
                        depth_name,
                        Expr::sub(Expr::var(depth_name), Expr::u32(1)),
                    )],
                )],
            ),
        ],
    );

    let mut nodes = vec![
        Node::let_bind(out_match_name, Expr::u32(u32::MAX)),
        Node::let_bind(depth_name, Expr::u32(1)),
    ];
    if let Some(guard) = guard {
        nodes.push(Node::if_then(guard, vec![loop_node]));
    } else {
        nodes.push(loop_node);
    }
    nodes
}

pub(super) fn emit_reverse_unmatched_lbrace_scan(
    tok_types: &str,
    scan_name: &str,
    scan_index_name: &str,
    scan_token_name: &str,
    depth_name: &str,
    out_open_name: &str,
    scan_limit: Expr,
    guard: Option<Expr>,
) -> Vec<Node> {
    let loop_node = Node::loop_for(
        scan_name,
        Expr::u32(0),
        scan_limit.clone(),
        vec![
            Node::let_bind(
                scan_index_name,
                Expr::sub(Expr::sub(scan_limit, Expr::u32(1)), Expr::var(scan_name)),
            ),
            Node::let_bind(
                scan_token_name,
                Expr::load(tok_types, Expr::var(scan_index_name)),
            ),
            Node::if_then(
                Expr::eq(Expr::var(scan_token_name), Expr::u32(TOK_RBRACE)),
                vec![Node::assign(
                    depth_name,
                    Expr::add(Expr::var(depth_name), Expr::u32(1)),
                )],
            ),
            Node::if_then(
                Expr::eq(Expr::var(out_open_name), Expr::u32(u32::MAX)),
                vec![Node::if_then(
                    Expr::eq(Expr::var(scan_token_name), Expr::u32(TOK_LBRACE)),
                    vec![Node::if_then_else(
                        Expr::eq(Expr::var(depth_name), Expr::u32(0)),
                        vec![Node::assign(out_open_name, Expr::var(scan_index_name))],
                        vec![Node::assign(
                            depth_name,
                            Expr::sub(Expr::var(depth_name), Expr::u32(1)),
                        )],
                    )],
                )],
            ),
        ],
    );

    if let Some(guard) = guard {
        vec![Node::if_then(guard, vec![loop_node])]
    } else {
        vec![loop_node]
    }
}

pub(super) fn emit_post_paren_boundary_scan(
    tok_types: &str,
    scan_name: &str,
    scan_token_name: &str,
    active_name: &str,
    matched_rparen: Expr,
    scan_end: Expr,
    out_token_name: &str,
    out_index_name: &str,
) -> Vec<Node> {
    let scan_start = Expr::add(matched_rparen.clone(), Expr::u32(1));
    vec![
        Node::let_bind(out_token_name, Expr::u32(u32::MAX)),
        Node::let_bind(out_index_name, Expr::u32(u32::MAX)),
        Node::let_bind(active_name, Expr::u32(1)),
        Node::if_then(
            Expr::lt(scan_start.clone(), scan_end.clone()),
            vec![Node::loop_for(
                scan_name,
                scan_start,
                scan_end,
                vec![
                    Node::let_bind(scan_token_name, Expr::load(tok_types, Expr::var(scan_name))),
                    Node::if_then(
                        Expr::and(
                            Expr::eq(Expr::var(active_name), Expr::u32(1)),
                            Expr::or(
                                Expr::eq(Expr::var(scan_token_name), Expr::u32(TOK_LBRACE)),
                                Expr::and(
                                    Expr::eq(Expr::var(scan_token_name), Expr::u32(TOK_SEMICOLON)),
                                    Expr::eq(
                                        Expr::var(scan_name),
                                        Expr::add(matched_rparen.clone(), Expr::u32(1)),
                                    ),
                                ),
                            ),
                        ),
                        vec![
                            Node::assign(out_token_name, Expr::var(scan_token_name)),
                            Node::assign(out_index_name, Expr::var(scan_name)),
                            Node::assign(active_name, Expr::u32(0)),
                        ],
                    ),
                ],
            )],
        ),
    ]
}
