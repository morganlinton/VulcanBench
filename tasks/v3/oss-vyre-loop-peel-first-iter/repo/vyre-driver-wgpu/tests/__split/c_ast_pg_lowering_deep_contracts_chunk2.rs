#[test]
fn function_definition_has_declaration_category() {
    let typed = classify(&fixture_function_definition());
    let semantic = reference_ast_to_pg_semantic_graph(&typed);
    let fn_idx = row_indices(&typed, C_AST_KIND_FUNCTION_DEFINITION)[0];

    assert_semantic_node(
        &semantic.nodes,
        fn_idx,
        C_AST_KIND_FUNCTION_DEFINITION,
        C_AST_PG_CATEGORY_DECLARATION,
        C_AST_PG_ROLE_FUNCTION_DEFINITION,
    );
}

#[test]
fn control_flow_statements_have_stable_semantic_roles() {
    let typed = classify(&fixture_control_flow_roles());
    let semantic = reference_ast_to_pg_semantic_graph(&typed);

    for (kind, role) in [
        (C_AST_KIND_IF_STMT, C_AST_PG_ROLE_SELECTION),
        (C_AST_KIND_FOR_STMT, C_AST_PG_ROLE_LOOP),
        (C_AST_KIND_WHILE_STMT, C_AST_PG_ROLE_LOOP),
        (C_AST_KIND_DO_STMT, C_AST_PG_ROLE_LOOP),
        (C_AST_KIND_SWITCH_STMT, C_AST_PG_ROLE_SWITCH),
        (C_AST_KIND_GOTO_STMT, C_AST_PG_ROLE_GOTO),
        (C_AST_KIND_RETURN_STMT, C_AST_PG_ROLE_RETURN),
        (C_AST_KIND_BREAK_STMT, C_AST_PG_ROLE_BREAK),
        (C_AST_KIND_CONTINUE_STMT, C_AST_PG_ROLE_CONTINUE),
    ] {
        let idx = row_indices(&typed, kind)
            .into_iter()
            .next()
            .unwrap_or_else(|| panic!("fixture must classify kind {kind:#x}"));
        assert_semantic_node(&semantic.nodes, idx, kind, C_AST_PG_CATEGORY_CONTROL, role);
    }
}

#[test]
fn control_flow_semantic_edges_resolve_goto_and_switch_relationships() {
    let typed = classify(&fixture_control_flow_roles());
    let semantic = reference_ast_to_pg_semantic_graph(&typed);

    let switch_idx = row_indices(&typed, C_AST_KIND_SWITCH_STMT)[0];
    let case_idx = row_indices(&typed, C_AST_KIND_CASE_STMT)[0];
    let default_idx = row_indices(&typed, C_AST_KIND_DEFAULT_STMT)[0];
    let goto_idx = row_indices(&typed, C_AST_KIND_GOTO_STMT)[0];
    let label_idx = row_indices(&typed, C_AST_KIND_LABEL_STMT)[0];

    let switch_condition_group_idx = vast_word(&typed, switch_idx, 3) as usize;
    let switch_selector_idx = vast_word(&typed, switch_condition_group_idx, 2);
    let case_value_idx = vast_word(&typed, case_idx, 3);

    assert_semantic_edge(
        &semantic.edges,
        goto_idx,
        3,
        C_AST_PG_EDGE_GOTO_TARGET,
        goto_idx as u32,
        label_idx as u32,
    );
    assert_semantic_edge(
        &semantic.edges,
        switch_idx,
        3,
        C_AST_PG_EDGE_SWITCH_SELECTOR,
        switch_idx as u32,
        switch_selector_idx,
    );
    assert_semantic_edge(
        &semantic.edges,
        case_idx,
        3,
        C_AST_PG_EDGE_CASE_VALUE,
        case_idx as u32,
        case_value_idx,
    );
    assert_semantic_edge(
        &semantic.edges,
        case_idx,
        4,
        C_AST_PG_EDGE_SWITCH_CASE,
        switch_idx as u32,
        case_idx as u32,
    );
    assert_semantic_edge(
        &semantic.edges,
        default_idx,
        3,
        C_AST_PG_EDGE_SWITCH_DEFAULT,
        switch_idx as u32,
        default_idx as u32,
    );
}

#[test]
fn alignof_expression_has_specific_semantic_role() {
    let typed = classify(&fixture_alignof_expression());
    let semantic = reference_ast_to_pg_semantic_graph(&typed);
    let idx = row_indices(&typed, C_AST_KIND_ALIGNOF_EXPR)[0];

    assert_semantic_node(
        &semantic.nodes,
        idx,
        C_AST_KIND_ALIGNOF_EXPR,
        C_AST_PG_CATEGORY_EXPRESSION,
        C_AST_PG_ROLE_ALIGNOF,
    );
}

#[test]
fn function_pointer_declarator_marks_pointer_context() {
    let typed = classify(&fixture_function_pointer_declarator());
    let semantic = reference_ast_to_pg_semantic_graph(&typed);

    assert!(
        !row_indices(&typed, C_AST_KIND_FUNCTION_DECLARATOR).is_empty(),
        "fixture must emit a function declarator"
    );
    let function_pointer_ptrs: Vec<usize> = row_indices(&typed, C_AST_KIND_POINTER_DECL)
        .into_iter()
        .filter(|idx| {
            semantic_node_word(&semantic.nodes, *idx, 7) == C_AST_PG_ROLE_FUNCTION_POINTER_DECL
        })
        .collect();
    assert_ne!(function_pointer_ptrs.len(), 0,
        "function-pointer declarator must mark at least one POINTER_DECL with the function-pointer role"
    );
}

#[test]
fn gpu_semantic_lowering_matches_cpu_oracle_for_deep_fixture() {
    let typed = classify(&fixture_label_case_default());
    let semantic = reference_ast_to_pg_semantic_graph(&typed);
    let (gpu_nodes, gpu_edges) = run_gpu_semantic_lower(&typed);

    assert_eq!(gpu_nodes, semantic.nodes, "semantic node GPU/CPU parity");
    assert_eq!(gpu_edges, semantic.edges, "semantic edge GPU/CPU parity");
}
