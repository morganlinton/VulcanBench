use super::*;

pub(super) fn store_semantic_edge(
    out_pg_edges: &str,
    edge_base: Expr,
    row_offset: u32,
    has_edge: Expr,
    edge_kind: u32,
    src_idx: Expr,
    dst_idx: Expr,
) -> Vec<Node> {
    let base = Expr::add(
        edge_base,
        Expr::u32(row_offset.saturating_mul(C_AST_PG_EDGE_STRIDE_U32)),
    );
    vec![
        Node::store(
            out_pg_edges,
            base.clone(),
            Expr::select(
                has_edge.clone(),
                Expr::u32(edge_kind),
                Expr::u32(C_AST_PG_EDGE_NONE),
            ),
        ),
        Node::store(
            out_pg_edges,
            Expr::add(base.clone(), Expr::u32(1)),
            Expr::select(has_edge.clone(), src_idx, Expr::u32(u32::MAX)),
        ),
        Node::store(
            out_pg_edges,
            Expr::add(base.clone(), Expr::u32(2)),
            Expr::select(has_edge.clone(), dst_idx, Expr::u32(u32::MAX)),
        ),
        Node::store(
            out_pg_edges,
            Expr::add(base.clone(), Expr::u32(3)),
            Expr::var("kind"),
        ),
        Node::store(
            out_pg_edges,
            Expr::add(base.clone(), Expr::u32(4)),
            Expr::var("semantic_role"),
        ),
        Node::store(
            out_pg_edges,
            Expr::add(base, Expr::u32(5)),
            Expr::var("semantic_category"),
        ),
    ]
}

pub(super) fn store_semantic_edge_expr(
    out_pg_edges: &str,
    edge_base: Expr,
    row_offset: u32,
    has_edge: Expr,
    edge_kind: Expr,
    src_idx: Expr,
    dst_idx: Expr,
) -> Vec<Node> {
    let base = Expr::add(
        edge_base,
        Expr::u32(row_offset.saturating_mul(C_AST_PG_EDGE_STRIDE_U32)),
    );
    vec![
        Node::store(
            out_pg_edges,
            base.clone(),
            Expr::select(has_edge.clone(), edge_kind, Expr::u32(C_AST_PG_EDGE_NONE)),
        ),
        Node::store(
            out_pg_edges,
            Expr::add(base.clone(), Expr::u32(1)),
            Expr::select(has_edge.clone(), src_idx, Expr::u32(u32::MAX)),
        ),
        Node::store(
            out_pg_edges,
            Expr::add(base.clone(), Expr::u32(2)),
            Expr::select(has_edge.clone(), dst_idx, Expr::u32(u32::MAX)),
        ),
        Node::store(
            out_pg_edges,
            Expr::add(base.clone(), Expr::u32(3)),
            Expr::var("kind"),
        ),
        Node::store(
            out_pg_edges,
            Expr::add(base.clone(), Expr::u32(4)),
            Expr::var("semantic_role"),
        ),
        Node::store(
            out_pg_edges,
            Expr::add(base, Expr::u32(5)),
            Expr::var("semantic_category"),
        ),
    ]
}
