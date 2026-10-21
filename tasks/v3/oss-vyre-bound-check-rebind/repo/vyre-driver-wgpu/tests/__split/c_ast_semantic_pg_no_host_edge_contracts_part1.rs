use super::*;

#[test]
fn gpu_typedef_node_has_declaration_category_and_typedef_role() {
    let fix = fixture_typedef_struct_enum_fnptr();
    let typed = classify(&fix);
    let (gpu_nodes, gpu_edges) = run_gpu_semantic_lower(&typed);

    let typedef_idxs = row_indices(&typed, C_AST_KIND_TYPEDEF_DECL);
    assert_eq!(
        typedef_idxs.len(),
        1,
        "fixture must classify exactly one typedef declaration"
    );

    for &idx in &typedef_idxs {
        assert_semantic_node(
            &gpu_nodes,
            idx,
            C_AST_KIND_TYPEDEF_DECL,
            C_AST_PG_CATEGORY_DECLARATION,
            C_AST_PG_ROLE_TYPEDEF_DECL,
        );
        let parent = vast_word(&typed, idx, 1);
        if parent != u32::MAX {
            assert_parent_edge(
                &gpu_edges,
                idx,
                parent,
                C_AST_PG_ROLE_TYPEDEF_DECL,
                C_AST_PG_CATEGORY_DECLARATION,
            );
        }
    }
}

#[test]
fn gpu_struct_tag_node_has_aggregate_decl_role() {
    let fix = fixture_typedef_struct_enum_fnptr();
    let typed = classify(&fix);
    let (gpu_nodes, _gpu_edges) = run_gpu_semantic_lower(&typed);

    let struct_idxs = row_indices(&typed, C_AST_KIND_STRUCT_DECL);
    assert_eq!(
        struct_idxs.len(),
        1,
        "fixture must classify exactly one struct declaration"
    );

    for &idx in &struct_idxs {
        assert_semantic_node(
            &gpu_nodes,
            idx,
            C_AST_KIND_STRUCT_DECL,
            C_AST_PG_CATEGORY_DECLARATION,
            C_AST_PG_ROLE_AGGREGATE_DECL,
        );
    }
}

#[test]
fn gpu_enum_tag_and_enumerator_nodes_have_correct_roles() {
    let fix = fixture_typedef_struct_enum_fnptr();
    let typed = classify(&fix);
    let (gpu_nodes, _gpu_edges) = run_gpu_semantic_lower(&typed);

    let enum_idxs = row_indices(&typed, C_AST_KIND_ENUM_DECL);
    assert_eq!(
        enum_idxs.len(),
        1,
        "fixture must classify exactly one enum declaration"
    );

    for &idx in &enum_idxs {
        assert_semantic_node(
            &gpu_nodes,
            idx,
            C_AST_KIND_ENUM_DECL,
            C_AST_PG_CATEGORY_DECLARATION,
            C_AST_PG_ROLE_AGGREGATE_DECL,
        );
    }

    let enumerator_idxs = row_indices(&typed, C_AST_KIND_ENUMERATOR_DECL);
    assert_eq!(
        enumerator_idxs.len(),
        2,
        "fixture must classify exactly two enumerators"
    );

    for &idx in &enumerator_idxs {
        assert_semantic_node(
            &gpu_nodes,
            idx,
            C_AST_KIND_ENUMERATOR_DECL,
            C_AST_PG_CATEGORY_DECLARATION,
            C_AST_PG_ROLE_ENUMERATOR_DECL,
        );
    }
}

#[test]
fn gpu_function_pointer_declarator_marks_function_pointer_role() {
    let fix = fixture_typedef_struct_enum_fnptr();
    let typed = classify(&fix);
    let (gpu_nodes, _gpu_edges) = run_gpu_semantic_lower(&typed);

    let fn_declarator_idxs = row_indices(&typed, C_AST_KIND_FUNCTION_DECLARATOR);
    assert_eq!(
        fn_declarator_idxs.len(),
        1,
        "fixture must classify exactly one function declarator"
    );

    let ptr_idxs: Vec<usize> = row_indices(&typed, C_AST_KIND_POINTER_DECL)
        .into_iter()
        .filter(|&idx| {
            semantic_node_word(&gpu_nodes, idx, 7) == C_AST_PG_ROLE_FUNCTION_POINTER_DECL
        })
        .collect();

    assert_eq!(
        ptr_idxs.len(),
        1,
        "function-pointer declarator must mark exactly one POINTER_DECL with FUNCTION_POINTER_DECL role"
    );
}

#[test]
fn gpu_regular_pointer_declarator_has_pointer_decl_role() {
    // Build a standalone pointer declaration (not a function pointer) to verify
    // the non-function-pointer path.
    let fix = build_fixture(&[
        FixtureToken::new("int", TOK_INT),
        FixtureToken::new("*", TOK_STAR),
        FixtureToken::new("p", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
    ]);
    let typed = classify(&fix);
    let (gpu_nodes, _gpu_edges) = run_gpu_semantic_lower(&typed);

    let ptr_idxs = row_indices(&typed, C_AST_KIND_POINTER_DECL);
    assert_eq!(
        ptr_idxs.len(),
        1,
        "fixture must classify exactly one pointer declarator"
    );

    for &idx in &ptr_idxs {
        assert_eq!(
            semantic_node_word(&gpu_nodes, idx, 7),
            C_AST_PG_ROLE_POINTER_DECL,
            "regular pointer declarator must have POINTER_DECL role"
        );
    }
}

// ---------------------------------------------------------------------------
// Label / goto / switch / case / default edge contracts
// ---------------------------------------------------------------------------

#[test]
fn gpu_switch_case_default_goto_label_edges_resolve() {
    let fix = fixture_switch_case_default_goto_label();
    let typed = classify(&fix);
    let (gpu_nodes, gpu_edges) = run_gpu_semantic_lower(&typed);

    let switch_idx = row_indices(&typed, C_AST_KIND_SWITCH_STMT)
        .into_iter()
        .next()
        .expect("fixture must classify a switch statement");
    let case_idx = row_indices(&typed, C_AST_KIND_CASE_STMT)
        .into_iter()
        .next()
        .expect("fixture must classify a case statement");
    let default_idx = row_indices(&typed, C_AST_KIND_DEFAULT_STMT)
        .into_iter()
        .next()
        .expect("fixture must classify a default statement");
    let goto_idx = row_indices(&typed, C_AST_KIND_GOTO_STMT)
        .into_iter()
        .next()
        .expect("fixture must classify a goto statement");
    let label_idx = row_indices(&typed, C_AST_KIND_LABEL_STMT)
        .into_iter()
        .next()
        .expect("fixture must classify a label statement");

    // Semantic node roles
    assert_semantic_node(
        &gpu_nodes,
        switch_idx,
        C_AST_KIND_SWITCH_STMT,
        C_AST_PG_CATEGORY_CONTROL,
        C_AST_PG_ROLE_SWITCH,
    );
    assert_semantic_node(
        &gpu_nodes,
        case_idx,
        C_AST_KIND_CASE_STMT,
        C_AST_PG_CATEGORY_CONTROL,
        C_AST_PG_ROLE_CASE,
    );
    assert_semantic_node(
        &gpu_nodes,
        default_idx,
        C_AST_KIND_DEFAULT_STMT,
        C_AST_PG_CATEGORY_CONTROL,
        C_AST_PG_ROLE_DEFAULT,
    );
    assert_semantic_node(
        &gpu_nodes,
        goto_idx,
        C_AST_KIND_GOTO_STMT,
        C_AST_PG_CATEGORY_CONTROL,
        C_AST_PG_ROLE_GOTO,
    );
    assert_semantic_node(
        &gpu_nodes,
        label_idx,
        C_AST_KIND_LABEL_STMT,
        C_AST_PG_CATEGORY_CONTROL,
        C_AST_PG_ROLE_LABEL,
    );

    // Compute expected edge endpoints from VAST structure (not from CPU oracle).
    let switch_condition_group = vast_word(&typed, switch_idx, 3);
    assert_ne!(
        switch_condition_group,
        u32::MAX,
        "switch must have a condition-group sibling"
    );
    let switch_selector = vast_word(&typed, switch_condition_group as usize, 2);
    assert_ne!(
        switch_selector,
        u32::MAX,
        "switch condition group must have a first-child selector"
    );

    let case_value = vast_word(&typed, case_idx, 3);
    assert_ne!(
        case_value,
        u32::MAX,
        "case must have a value-expression sibling"
    );

    // Concrete semantic edge assertions
    assert_semantic_edge(
        &gpu_edges,
        switch_idx,
        3,
        C_AST_PG_EDGE_SWITCH_SELECTOR,
        switch_idx as u32,
        switch_selector,
    );
    assert_semantic_edge(
        &gpu_edges,
        case_idx,
        3,
        C_AST_PG_EDGE_CASE_VALUE,
        case_idx as u32,
        case_value,
    );
    assert_semantic_edge(
        &gpu_edges,
        case_idx,
        4,
        C_AST_PG_EDGE_SWITCH_CASE,
        switch_idx as u32,
        case_idx as u32,
    );
    assert_semantic_edge(
        &gpu_edges,
        default_idx,
        3,
        C_AST_PG_EDGE_SWITCH_DEFAULT,
        switch_idx as u32,
        default_idx as u32,
    );
    assert_semantic_edge(
        &gpu_edges,
        goto_idx,
        3,
        C_AST_PG_EDGE_GOTO_TARGET,
        goto_idx as u32,
        label_idx as u32,
    );
}

// ---------------------------------------------------------------------------
// Scope structural edge contracts
// ---------------------------------------------------------------------------

#[test]
fn gpu_scope_nesting_preserves_structural_parent_edges() {
    let fix = fixture_scope_nesting();
    let typed = classify(&fix);
    let (_gpu_nodes, gpu_edges) = run_gpu_semantic_lower(&typed);

    // Token layout (one node per token):
    // 0:void 1:f 2:( 3:) 4:{ 5:{ 6:int 7:a 8:; 9:} 10:{ 11:int 12:b 13:; 14:} 15:}
    let outer_brace = 4usize;
    let first_inner_brace = 5usize;
    let second_inner_brace = 10usize;

    // First inner compound brace is a child of the outer function-body brace.
    assert_eq!(
        semantic_edge_word(&gpu_edges, first_inner_brace, 0, 0),
        C_AST_PG_EDGE_PARENT,
        "first inner brace must have a parent edge"
    );
    assert_eq!(
        semantic_edge_word(&gpu_edges, first_inner_brace, 0, 1),
        outer_brace as u32,
        "first inner brace parent must be outer brace"
    );

    // Second inner compound brace is also a child of the outer brace.
    assert_eq!(
        semantic_edge_word(&gpu_edges, second_inner_brace, 0, 0),
        C_AST_PG_EDGE_PARENT,
        "second inner brace must have a parent edge"
    );
    assert_eq!(
        semantic_edge_word(&gpu_edges, second_inner_brace, 0, 1),
        outer_brace as u32,
        "second inner brace parent must be outer brace"
    );

    // `int` inside first compound has first_inner_brace as parent.
    assert_eq!(
        semantic_edge_word(&gpu_edges, 6, 0, 0),
        C_AST_PG_EDGE_PARENT,
        "first-compound int must have a parent edge"
    );
    assert_eq!(
        semantic_edge_word(&gpu_edges, 6, 0, 1),
        first_inner_brace as u32,
        "first-compound int parent must be first inner brace"
    );

    // `int` inside second compound has second_inner_brace as parent.
    assert_eq!(
        semantic_edge_word(&gpu_edges, 11, 0, 0),
        C_AST_PG_EDGE_PARENT,
        "second-compound int must have a parent edge"
    );
    assert_eq!(
        semantic_edge_word(&gpu_edges, 11, 0, 1),
        second_inner_brace as u32,
        "second-compound int parent must be second inner brace"
    );
}

#[test]
fn gpu_function_definition_has_declaration_category_and_role() {
    let fix = fixture_scope_nesting();
    let typed = classify(&fix);
    let (gpu_nodes, _gpu_edges) = run_gpu_semantic_lower(&typed);

    let fn_idxs = row_indices(&typed, C_AST_KIND_FUNCTION_DEFINITION);
    assert_eq!(
        fn_idxs.len(),
        1,
        "fixture must classify exactly one function definition"
    );

    for &idx in &fn_idxs {
        assert_semantic_node(
            &gpu_nodes,
            idx,
            C_AST_KIND_FUNCTION_DEFINITION,
            C_AST_PG_CATEGORY_DECLARATION,
            C_AST_PG_ROLE_FUNCTION_DEFINITION,
        );
    }
}

// ---------------------------------------------------------------------------
// No-host completion: GPU dispatch must succeed for every fixture
// ---------------------------------------------------------------------------

#[test]
fn gpu_semantic_lowering_completes_for_typedef_struct_enum_fnptr() {
    let fix = fixture_typedef_struct_enum_fnptr();
    let typed = classify(&fix);
    let (gpu_nodes, gpu_edges) = run_gpu_semantic_lower(&typed);

    let node_count = node_count_from_vast(&typed) as usize;
    assert_eq!(
        gpu_nodes.len(),
        node_count * C_AST_PG_SEMANTIC_NODE_STRIDE_U32 as usize * 4,
        "semantic node buffer size must match node count"
    );
    assert_eq!(
        gpu_edges.len(),
        node_count * C_AST_PG_EDGE_ROWS_PER_NODE as usize * C_AST_PG_EDGE_STRIDE_U32 as usize * 4,
        "semantic edge buffer size must match node count"
    );
}

#[test]
fn gpu_semantic_lowering_completes_for_switch_case_default_goto_label() {
    let fix = fixture_switch_case_default_goto_label();
    let typed = classify(&fix);
    let (gpu_nodes, gpu_edges) = run_gpu_semantic_lower(&typed);

    let node_count = node_count_from_vast(&typed) as usize;
    assert_eq!(
        gpu_nodes.len(),
        node_count * C_AST_PG_SEMANTIC_NODE_STRIDE_U32 as usize * 4,
        "semantic node buffer size must match node count"
    );
    assert_eq!(
        gpu_edges.len(),
        node_count * C_AST_PG_EDGE_ROWS_PER_NODE as usize * C_AST_PG_EDGE_STRIDE_U32 as usize * 4,
        "semantic edge buffer size must match node count"
    );
}

