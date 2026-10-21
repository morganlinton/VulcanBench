use super::*;
use vyre::ir::{Expr, Node};

pub(super) fn vast_node_base(node_idx: Expr) -> Expr {
    Expr::mul(node_idx, Expr::u32(VAST_NODE_STRIDE_U32))
}

pub(super) fn decl_context_base(node_idx: Expr) -> Expr {
    Expr::mul(node_idx, Expr::u32(VAST_DECL_CONTEXT_STRIDE_U32))
}

pub(super) fn load_vast_node_field(vast_nodes: &str, base: Expr, field: u32) -> Expr {
    let offset = if field == 0 {
        base
    } else {
        Expr::add(base, Expr::u32(field))
    };
    Expr::load(vast_nodes, offset)
}

pub(super) fn load_vast_node_kind(vast_nodes: &str, base: Expr) -> Expr {
    load_vast_node_field(vast_nodes, base, 0)
}

pub(super) fn load_vast_node_parent(vast_nodes: &str, base: Expr) -> Expr {
    load_vast_node_field(vast_nodes, base, 1)
}

pub(super) fn load_decl_context_field(decl_contexts: &str, base: Expr, field: u32) -> Expr {
    Expr::load(decl_contexts, Expr::add(base, Expr::u32(field)))
}

pub(super) fn store_decl_context_field(
    decl_contexts: &str,
    base: Expr,
    field: u32,
    value: Expr,
) -> Node {
    Node::store(decl_contexts, Expr::add(base, Expr::u32(field)), value)
}

pub(super) fn bind_vast_node_base(name: &'static str, node_idx: Expr) -> Node {
    Node::let_bind(name, vast_node_base(node_idx))
}

pub(super) fn bind_vast_node_kind(name: &'static str, vast_nodes: &str, base_var: &str) -> Node {
    Node::let_bind(name, load_vast_node_kind(vast_nodes, Expr::var(base_var)))
}

pub(super) fn bind_vast_node_parent(name: &'static str, vast_nodes: &str, base_var: &str) -> Node {
    Node::let_bind(name, load_vast_node_parent(vast_nodes, Expr::var(base_var)))
}

pub(super) fn bind_vast_node_field(
    name: &'static str,
    vast_nodes: &str,
    base_var: &str,
    field: u32,
) -> Node {
    Node::let_bind(
        name,
        load_vast_node_field(vast_nodes, Expr::var(base_var), field),
    )
}
