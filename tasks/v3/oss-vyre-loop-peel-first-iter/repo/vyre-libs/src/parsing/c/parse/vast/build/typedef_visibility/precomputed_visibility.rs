use super::chain;
use super::*;

pub(crate) fn emit_typedef_visibility_scan_precomputed_context(
    vast_nodes: &str,
    decl_contexts: &str,
    t: Expr,
) -> Vec<Node> {
    let prefix = "current_visible_typedef_precomputed";
    let target_base = format!("{prefix}_target_base");
    let target_len = format!("{prefix}_target_len");
    let target_hash = format!("{prefix}_target_hash");
    let target_scope = format!("{prefix}_target_scope");
    let target_context_base = format!("{prefix}_target_context_base");
    let target_link_raw = format!("{prefix}_target_link_raw");
    let target_chain_len = format!("{prefix}_target_chain_len");
    let last_decl_kind = format!("{prefix}_last_decl_kind");
    let chain_cursor = format!("{prefix}_chain_cursor");
    let scan_offset = format!("{prefix}_scan_offset");
    let scan_valid = format!("{prefix}_scan_valid");
    let scan_base = format!("{prefix}_scan_base");
    let scan_context_base = format!("{prefix}_scan_context_base");
    let scan_hash = format!("{prefix}_scan_hash");
    let scan_len = format!("{prefix}_scan_len");
    let scan_decl_kind = format!("{prefix}_scan_decl_result_kind");
    let scan_scope = format!("{prefix}_scan_scope");
    let visible_scope = format!("{prefix}_visible_scope");
    let visible_function = format!("{prefix}_visible_function");
    let target_function = format!("{prefix}_target_function");
    let scan_function = format!("{prefix}_scan_function");
    let scope_walk = format!("{prefix}_scope_walk");
    let scope_walk_depth = format!("{prefix}_scope_walk_depth");
    let next_link_raw = format!("{prefix}_next_link_raw");

    let mut nodes = vec![
        Node::let_bind("current_visible_typedef_name", Expr::u32(0)),
        Node::let_bind(&target_base, vast_row_base_expr(t.clone())),
        Node::let_bind(
            &target_len,
            chain::vast_len_from_base(vast_nodes, &target_base),
        ),
        Node::let_bind(
            &target_hash,
            chain::vast_typedef_hash_from_base(vast_nodes, &target_base),
        ),
        Node::let_bind(
            &target_scope,
            chain::vast_scope_from_base(vast_nodes, &target_base),
        ),
        Node::let_bind(
            &target_context_base,
            chain::decl_context_base_for_index(t.clone()),
        ),
        Node::let_bind(
            &target_link_raw,
            chain::prev_decl_link_from_base(decl_contexts, &target_context_base),
        ),
        Node::let_bind(
            &target_chain_len,
            chain::prev_decl_chain_len_from_base(decl_contexts, &target_context_base),
        ),
        Node::let_bind(&last_decl_kind, Expr::u32(0)),
        Node::let_bind(
            &chain_cursor,
            chain::decode_prev_decl_link(Expr::var(&target_link_raw)),
        ),
    ];

    let mut same_candidate_body = Vec::new();
    same_candidate_body.extend(emit_precomputed_declaration_kind_for_index(
        vast_nodes,
        decl_contexts,
        Expr::var(&chain_cursor),
        &scan_decl_kind,
        &format!("{prefix}_scan_decl"),
    ));
    same_candidate_body.push(Node::let_bind(
        &scan_scope,
        chain::vast_scope_from_base(vast_nodes, &scan_base),
    ));
    same_candidate_body.push(Node::let_bind(
        &visible_scope,
        Expr::or(
            Expr::eq(Expr::var(&scan_scope), Expr::u32(SENTINEL)),
            Expr::eq(Expr::var(&scan_scope), Expr::var(&target_scope)),
        ),
    ));
    same_candidate_body.push(Node::let_bind(&visible_function, Expr::bool(true)));
    same_candidate_body.push(super::visibility_match::emit_function_visibility_gate(
        vast_nodes,
        t.clone(),
        Expr::var(&chain_cursor),
        &scan_decl_kind,
        &visible_function,
        &target_function,
        &scan_function,
        &format!("{prefix}_target_function"),
        &format!("{prefix}_scan_function"),
    ));
    same_candidate_body.extend(super::visibility_match::emit_scope_visibility_update(
        vast_nodes,
        &target_scope,
        &scan_scope,
        &visible_scope,
        &visible_function,
        &scan_decl_kind,
        &last_decl_kind,
        &scope_walk,
        &scope_walk_depth,
    ));

    let loop_body = vec![
        Node::let_bind(
            &scan_valid,
            Expr::and(
                Expr::eq(Expr::var(&last_decl_kind), Expr::u32(0)),
                Expr::ne(Expr::var(&chain_cursor), Expr::u32(SENTINEL)),
            ),
        ),
        Node::let_bind(&scan_base, vast_row_base_expr(Expr::var(&chain_cursor))),
        Node::let_bind(
            &scan_hash,
            chain::vast_typedef_hash_from_base(vast_nodes, &scan_base),
        ),
        Node::let_bind(&scan_len, chain::vast_len_from_base(vast_nodes, &scan_base)),
        Node::if_then(
            Expr::and(
                Expr::var(&scan_valid),
                Expr::and(
                    Expr::eq(Expr::var(&scan_hash), Expr::var(&target_hash)),
                    Expr::eq(Expr::var(&scan_len), Expr::var(&target_len)),
                ),
            ),
            same_candidate_body,
        ),
        Node::let_bind(
            &scan_context_base,
            chain::decl_context_base_for_index(Expr::var(&chain_cursor)),
        ),
        Node::let_bind(
            &next_link_raw,
            chain::prev_decl_link_from_base(decl_contexts, &scan_context_base),
        ),
        Node::assign(
            &chain_cursor,
            chain::decode_prev_decl_link(Expr::var(&next_link_raw)),
        ),
    ];
    nodes.push(Node::loop_for(
        &scan_offset,
        Expr::u32(0),
        Expr::var(&target_chain_len),
        loop_body,
    ));
    nodes.push(Node::assign(
        "current_visible_typedef_name",
        Expr::select(
            Expr::eq(Expr::var(&last_decl_kind), Expr::u32(1)),
            Expr::u32(1),
            Expr::u32(0),
        ),
    ));
    nodes.push(Node::assign(
        "last_decl_kind",
        Expr::select(
            Expr::eq(Expr::var("current_visible_typedef_name"), Expr::u32(1)),
            Expr::u32(1),
            Expr::u32(0),
        ),
    ));
    nodes
}
