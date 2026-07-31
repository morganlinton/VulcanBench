use super::*;

pub(super) fn emit_function_visibility_gate(
    vast_nodes: &str,
    target_idx: Expr,
    scan_idx: Expr,
    scan_decl_kind: &str,
    visible_function: &str,
    target_function: &str,
    scan_function: &str,
    target_prefix: &str,
    scan_prefix: &str,
) -> Node {
    let mut body = emit_enclosing_function_lparen_for_index(
        vast_nodes,
        target_idx,
        target_function,
        target_prefix,
    );
    body.extend(emit_enclosing_function_lparen_for_index(
        vast_nodes,
        scan_idx,
        scan_function,
        scan_prefix,
    ));
    body.push(Node::assign(
        visible_function,
        Expr::or(
            Expr::eq(Expr::var(scan_function), Expr::u32(SENTINEL)),
            Expr::eq(Expr::var(scan_function), Expr::var(target_function)),
        ),
    ));
    Node::if_then(Expr::eq(Expr::var(scan_decl_kind), Expr::u32(2)), body)
}

pub(super) fn emit_scope_visibility_update(
    vast_nodes: &str,
    target_scope: &str,
    scan_scope: &str,
    visible_scope: &str,
    visible_function: &str,
    scan_decl_kind: &str,
    last_decl_kind: &str,
    scope_walk: &str,
    scope_walk_depth: &str,
) -> Vec<Node> {
    vec![
        Node::if_then(
            Expr::and(
                Expr::not(Expr::var(visible_scope)),
                Expr::and(
                    Expr::var(visible_function),
                    Expr::ne(Expr::var(scan_decl_kind), Expr::u32(0)),
                ),
            ),
            vec![
                Node::let_bind(scope_walk, Expr::var(target_scope)),
                Node::loop_for(
                    scope_walk_depth,
                    Expr::u32(0),
                    Expr::var("annot_num_nodes"),
                    vec![
                        Node::if_then(
                            Expr::and(
                                Expr::not(Expr::var(visible_scope)),
                                Expr::eq(Expr::var(scope_walk), Expr::var(scan_scope)),
                            ),
                            vec![Node::assign(visible_scope, Expr::bool(true))],
                        ),
                        Node::if_then(
                            Expr::and(
                                Expr::not(Expr::var(visible_scope)),
                                Expr::ne(Expr::var(scope_walk), Expr::u32(SENTINEL)),
                            ),
                            vec![Node::assign(
                                scope_walk,
                                Expr::load(
                                    vast_nodes,
                                    Expr::add(
                                        vast_row_base_expr(Expr::var(scope_walk)),
                                        Expr::u32(1),
                                    ),
                                ),
                            )],
                        ),
                    ],
                ),
            ],
        ),
        emit_last_decl_kind_update(
            visible_scope,
            visible_function,
            scan_decl_kind,
            last_decl_kind,
        ),
    ]
}

pub(super) fn emit_last_decl_kind_update(
    visible_scope: &str,
    visible_function: &str,
    scan_decl_kind: &str,
    last_decl_kind: &str,
) -> Node {
    Node::if_then(
        Expr::and(
            Expr::var(visible_scope),
            Expr::and(
                Expr::var(visible_function),
                Expr::ne(Expr::var(scan_decl_kind), Expr::u32(0)),
            ),
        ),
        vec![Node::assign(last_decl_kind, Expr::var(scan_decl_kind))],
    )
}
