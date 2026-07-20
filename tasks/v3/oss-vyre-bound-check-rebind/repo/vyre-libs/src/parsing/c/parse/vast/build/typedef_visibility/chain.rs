use super::super::super::decl_context_common;
use super::super::super::{
    SENTINEL, VAST_DECL_CONTEXT_PREV_DECL_CHAIN_LEN_FIELD, VAST_DECL_CONTEXT_PREV_DECL_LINK_FIELD,
    VAST_TYPEDEF_FLAGS_FIELD, VAST_TYPEDEF_SCOPE_FIELD, VAST_TYPEDEF_SYMBOL_FIELD,
};
use vyre::ir::Expr;

pub(super) fn vast_field_from_base(vast_nodes: &str, base_var: &str, field: u32) -> Expr {
    decl_context_common::load_vast_node_field(vast_nodes, Expr::var(base_var), field)
}

pub(super) fn vast_kind_from_base(vast_nodes: &str, base_var: &str) -> Expr {
    decl_context_common::load_vast_node_kind(vast_nodes, Expr::var(base_var))
}

pub(super) fn vast_len_from_base(vast_nodes: &str, base_var: &str) -> Expr {
    vast_field_from_base(vast_nodes, base_var, 6)
}

pub(super) fn vast_scope_from_base(vast_nodes: &str, base_var: &str) -> Expr {
    vast_field_from_base(vast_nodes, base_var, VAST_TYPEDEF_SCOPE_FIELD)
}

pub(super) fn vast_typedef_hash_from_base(vast_nodes: &str, base_var: &str) -> Expr {
    vast_field_from_base(vast_nodes, base_var, VAST_TYPEDEF_SYMBOL_FIELD)
}

pub(super) fn vast_typedef_flags_from_base(vast_nodes: &str, base_var: &str) -> Expr {
    vast_field_from_base(vast_nodes, base_var, VAST_TYPEDEF_FLAGS_FIELD)
}

pub(super) fn decl_context_base_for_index(idx: Expr) -> Expr {
    decl_context_common::decl_context_base(idx)
}

pub(super) fn prev_decl_link_from_base(decl_contexts: &str, base_var: &str) -> Expr {
    decl_context_common::load_decl_context_field(
        decl_contexts,
        Expr::var(base_var),
        VAST_DECL_CONTEXT_PREV_DECL_LINK_FIELD,
    )
}

pub(super) fn prev_decl_chain_len_from_base(decl_contexts: &str, base_var: &str) -> Expr {
    decl_context_common::load_decl_context_field(
        decl_contexts,
        Expr::var(base_var),
        VAST_DECL_CONTEXT_PREV_DECL_CHAIN_LEN_FIELD,
    )
}

pub(super) fn prev_decl_link_for_index(decl_contexts: &str, idx: Expr) -> Expr {
    decl_context_common::load_decl_context_field(
        decl_contexts,
        decl_context_base_for_index(idx),
        VAST_DECL_CONTEXT_PREV_DECL_LINK_FIELD,
    )
}

pub(super) fn prev_decl_chain_len_for_index(decl_contexts: &str, idx: Expr) -> Expr {
    decl_context_common::load_decl_context_field(
        decl_contexts,
        decl_context_base_for_index(idx),
        VAST_DECL_CONTEXT_PREV_DECL_CHAIN_LEN_FIELD,
    )
}

pub(super) fn decode_prev_decl_link(raw: Expr) -> Expr {
    Expr::select(
        Expr::or(
            Expr::eq(raw.clone(), Expr::u32(0)),
            Expr::eq(raw.clone(), Expr::u32(SENTINEL)),
        ),
        Expr::u32(SENTINEL),
        Expr::sub(raw, Expr::u32(1)),
    )
}

pub(super) fn decode_prepared_prev_decl_link(raw: Expr, prepared: Expr) -> Expr {
    Expr::select(
        Expr::and(prepared, Expr::ne(raw.clone(), Expr::u32(SENTINEL))),
        Expr::sub(raw, Expr::u32(1)),
        Expr::u32(SENTINEL),
    )
}
