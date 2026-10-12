use super::*;

pub(super) fn assign_related_kind_if_valid(
    nodes: &mut Vec<Node>,
    related_var: &str,
    related_kind_var: &str,
    vast_nodes: &str,
    num_nodes: &Expr,
) {
    nodes.push(Node::if_then(
        Expr::and(
            Expr::ne(Expr::var(related_var), Expr::u32(u32::MAX)),
            Expr::lt(Expr::var(related_var), num_nodes.clone()),
        ),
        vec![Node::assign(
            related_kind_var,
            Expr::load(
                vast_nodes,
                Expr::mul(Expr::var(related_var), Expr::u32(VAST_NODE_STRIDE_U32)),
            ),
        )],
    ));
}

pub(super) fn valid_node_ref_expr(idx: Expr, num_nodes: &Expr) -> Expr {
    Expr::and(
        Expr::ne(idx.clone(), Expr::u32(u32::MAX)),
        Expr::lt(idx, num_nodes.clone()),
    )
}

pub(super) fn semantic_context_bind_nodes() -> Vec<Node> {
    vec![
        Node::let_bind("parent_kind", Expr::u32(0)),
        Node::let_bind("first_child_kind", Expr::u32(0)),
        Node::let_bind("next_sibling_kind", Expr::u32(0)),
    ]
}

pub(super) fn semantic_context_assign_nodes(vast_nodes: &str, num_nodes: &Expr) -> Vec<Node> {
    let mut nodes = Vec::new();
    assign_related_kind_if_valid(
        &mut nodes,
        "parent_idx",
        "parent_kind",
        vast_nodes,
        num_nodes,
    );
    assign_related_kind_if_valid(
        &mut nodes,
        "first_child_idx",
        "first_child_kind",
        vast_nodes,
        num_nodes,
    );
    assign_related_kind_if_valid(
        &mut nodes,
        "next_sibling_idx",
        "next_sibling_kind",
        vast_nodes,
        num_nodes,
    );
    nodes
}
