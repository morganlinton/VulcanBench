use super::*;

pub(super) fn expr_is_kind(kind: Expr, expected: u32) -> Expr {
    Expr::eq(kind, Expr::u32(expected))
}

pub(super) fn valid_node_idx(idx: Expr, num_nodes: &Expr) -> Expr {
    Expr::and(
        Expr::ne(idx.clone(), Expr::u32(u32::MAX)),
        Expr::lt(idx, num_nodes.clone()),
    )
}

pub(super) fn common_parent_walk_bound(num_nodes: &Expr) -> Expr {
    Expr::select(
        Expr::lt(num_nodes.clone(), Expr::u32(COMMON_PARENT_WALK_LIMIT)),
        num_nodes.clone(),
        Expr::u32(COMMON_PARENT_WALK_LIMIT),
    )
}

pub(super) fn vast_field(vast_nodes: &str, idx: Expr, field: usize) -> Expr {
    Expr::load(
        vast_nodes,
        Expr::add(
            Expr::mul(idx, Expr::u32(VAST_NODE_STRIDE_U32)),
            Expr::u32(field as u32),
        ),
    )
}

fn emit_root_node_walk(
    vast_nodes: &str,
    num_nodes: &Expr,
    start_idx: Expr,
    root_var: &str,
    parent_var: &str,
    loop_var: &str,
    bind_initial_vars: bool,
) -> Vec<Node> {
    let root_init = if bind_initial_vars {
        Node::let_bind(root_var, start_idx.clone())
    } else {
        Node::assign(root_var, start_idx.clone())
    };
    let parent_init = if bind_initial_vars {
        Node::let_bind(parent_var, Expr::u32(u32::MAX))
    } else {
        Node::assign(parent_var, Expr::u32(u32::MAX))
    };
    vec![
        root_init,
        parent_init,
        Node::if_then(
            valid_node_idx(start_idx.clone(), num_nodes),
            vec![Node::assign(
                parent_var,
                vast_field(vast_nodes, start_idx, IDX_PARENT),
            )],
        ),
        Node::loop_for(
            loop_var,
            Expr::u32(0),
            num_nodes.clone(),
            vec![Node::if_then(
                valid_node_idx(Expr::var(parent_var), num_nodes),
                vec![
                    Node::assign(root_var, Expr::var(parent_var)),
                    Node::assign(
                        parent_var,
                        vast_field(vast_nodes, Expr::var(parent_var), IDX_PARENT),
                    ),
                ],
            )],
        ),
    ]
}

pub(super) fn resolve_root_nodes(
    vast_nodes: &str,
    num_nodes: &Expr,
    start_idx: Expr,
    root_var: &str,
    parent_var: &str,
    loop_var: &str,
) -> Vec<Node> {
    emit_root_node_walk(
        vast_nodes, num_nodes, start_idx, root_var, parent_var, loop_var, true,
    )
}

pub(super) fn assign_root_nodes(
    vast_nodes: &str,
    num_nodes: &Expr,
    start_idx: Expr,
    root_var: &str,
    parent_var: &str,
    loop_var: &str,
) -> Vec<Node> {
    emit_root_node_walk(
        vast_nodes, num_nodes, start_idx, root_var, parent_var, loop_var, false,
    )
}
