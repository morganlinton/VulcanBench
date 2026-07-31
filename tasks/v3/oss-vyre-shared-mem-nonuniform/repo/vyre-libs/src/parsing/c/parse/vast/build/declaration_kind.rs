use super::*;

pub(crate) fn emit_declaration_kind_for_index(
    vast_nodes: &str,
    haystack: &str,
    haystack_len: &Expr,
    idx: Expr,
    out_name: &str,
    prefix: &str,
    packed_haystack: bool,
    decl_contexts: Option<&str>,
) -> Vec<Node> {
    emit_declaration_kind_for_index_inner(
        vast_nodes,
        decl_contexts,
        idx,
        out_name,
        prefix,
        Some((haystack, haystack_len, packed_haystack)),
    )
}

pub(crate) fn emit_builtin_declaration_kind_for_index(
    vast_nodes: &str,
    idx: Expr,
    out_name: &str,
    prefix: &str,
    decl_contexts: Option<&str>,
) -> Vec<Node> {
    emit_declaration_kind_for_index_inner(vast_nodes, decl_contexts, idx, out_name, prefix, None)
}
