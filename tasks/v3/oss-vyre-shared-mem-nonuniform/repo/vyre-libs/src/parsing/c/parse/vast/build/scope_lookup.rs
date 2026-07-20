use super::*;

pub(crate) fn emit_scope_open_for_index(
    vast_nodes: &str,
    idx: Expr,
    out_name: &str,
    prefix: &str,
) -> Vec<Node> {
    let mut nodes = vec![Node::let_bind(out_name, Expr::u32(SENTINEL))];
    nodes.extend(emit_scope_open_scan_assign_for_index(
        vast_nodes, idx, out_name, prefix,
    ));
    nodes
}

pub(crate) fn emit_scope_open_scan_assign_for_index(
    vast_nodes: &str,
    idx: Expr,
    out_name: &str,
    prefix: &str,
) -> Vec<Node> {
    let depth = format!("{prefix}_depth");
    let scan = format!("{prefix}_scan");
    let rev = format!("{prefix}_idx");
    let kind = format!("{prefix}_kind");

    vec![
        Node::let_bind(&depth, Expr::u32(0)),
        Node::loop_for(
            &scan,
            Expr::u32(0),
            idx.clone(),
            vec![
                Node::let_bind(
                    &rev,
                    Expr::sub(Expr::sub(idx, Expr::u32(1)), Expr::var(&scan)),
                ),
                Node::let_bind(
                    &kind,
                    Expr::load(
                        vast_nodes,
                        Expr::mul(Expr::var(&rev), Expr::u32(VAST_NODE_STRIDE_U32)),
                    ),
                ),
                Node::if_then(
                    Expr::eq(Expr::var(&kind), Expr::u32(TOK_RBRACE)),
                    vec![Node::assign(
                        &depth,
                        Expr::add(Expr::var(&depth), Expr::u32(1)),
                    )],
                ),
                Node::if_then(
                    Expr::eq(Expr::var(out_name), Expr::u32(SENTINEL)),
                    vec![Node::if_then(
                        Expr::eq(Expr::var(&kind), Expr::u32(TOK_LBRACE)),
                        vec![Node::if_then_else(
                            Expr::eq(Expr::var(&depth), Expr::u32(0)),
                            vec![Node::assign(out_name, Expr::var(&rev))],
                            vec![Node::assign(
                                &depth,
                                Expr::sub(Expr::var(&depth), Expr::u32(1)),
                            )],
                        )],
                    )],
                ),
            ],
        ),
    ]
}
