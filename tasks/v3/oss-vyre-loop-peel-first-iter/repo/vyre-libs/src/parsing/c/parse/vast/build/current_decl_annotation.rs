use super::*;

pub(crate) fn emit_current_declaration_annotation(
    vast_nodes: &str,
    haystack: &str,
    haystack_len: &Expr,
    t: Expr,
    _num_nodes: &Expr,
    packed_haystack: bool,
    decl_contexts: Option<&str>,
) -> Vec<Node> {
    let mut nodes = Vec::new();
    nodes.extend(emit_declaration_kind_for_index(
        vast_nodes,
        haystack,
        haystack_len,
        t,
        "current_decl_result_kind",
        "current_decl",
        packed_haystack,
        decl_contexts,
    ));
    nodes.push(Node::assign(
        "current_decl_flags",
        Expr::select(
            Expr::eq(Expr::var("current_decl_result_kind"), Expr::u32(1)),
            Expr::u32(C_TYPEDEF_FLAG_TYPEDEF_DECLARATOR),
            Expr::select(
                Expr::eq(Expr::var("current_decl_result_kind"), Expr::u32(2)),
                Expr::u32(C_TYPEDEF_FLAG_ORDINARY_DECLARATOR),
                Expr::u32(0),
            ),
        ),
    ));
    nodes
}
