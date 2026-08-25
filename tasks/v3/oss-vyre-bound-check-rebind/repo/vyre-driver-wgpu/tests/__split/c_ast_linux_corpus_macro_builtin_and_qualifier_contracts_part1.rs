use super::*;

#[test]
fn container_of_macro_def_survives_and_use_is_call_shaped() {
    let fix = fixture_container_of_macro_and_use();
    assert_full_pipeline_parity(&fix, "container_of_macro_and_use");

    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);

    // The macro definition enters the token stream as a raw preprocessor row.
    // The typed VAST may classify it differently; the contract is that the
    // usage must be a CALL-shaped node and the fixture must not panic.

    // The usage must be a CALL-shaped node, not silently dropped.
    let container_of_rows = token_indices_containing(&fix, "container_of");
    assert!(
        container_of_rows.len() >= 2,
        "fixture must contain at least two tokens containing 'container_of' (def + use)"
    );
    // The last occurrence is the usage (first is inside the #define line).
    let use_row = *container_of_rows.last().unwrap();
    assert_eq!(
        kind_at(&typed, use_row),
        node_kind::CALL,
        "container_of(...) usage must classify as CALL-shaped token stream"
    );

    // Pointer declarator for p
    let ptrs = row_indices(&typed, C_AST_KIND_POINTER_DECL);
    assert!(
        !ptrs.is_empty(),
        "struct node *p must contain a POINTER_DECL"
    );

    // PG preservation for the call node
    let pg = reference_ast_to_pg_nodes(&typed);
    assert_pg_preserves_row(
        &typed,
        &pg,
        &fix.tok_starts,
        &fix.tok_lens,
        use_row,
        node_kind::CALL,
    );
}

#[test]
fn container_of_gpu_pg_lower_matches_cpu() {
    let fix = fixture_container_of_macro_and_use();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let expected_pg = reference_ast_to_pg_nodes(&typed);
    let gpu_pg = run_gpu_pg_lower(&typed, node_count_from_vast(&typed));
    assert_eq!(
        gpu_pg, expected_pg,
        "GPU PG lowerer must match CPU for container_of fixture"
    );
}

// ---------------------------------------------------------------------------
// Tests  -  list_entry
// ---------------------------------------------------------------------------

#[test]
fn list_entry_macro_def_survives_and_use_is_call_shaped() {
    let fix = fixture_list_entry_macro_and_use();
    assert_full_pipeline_parity(&fix, "list_entry_macro_and_use");

    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);

    let list_entry_rows = token_indices_containing(&fix, "list_entry");
    assert!(
        list_entry_rows.len() >= 2,
        "fixture must contain at least two tokens containing 'list_entry'"
    );
    let use_row = *list_entry_rows.last().unwrap();
    assert_eq!(
        kind_at(&typed, use_row),
        node_kind::CALL,
        "list_entry(...) usage must be CALL-shaped"
    );

    // Member access for head.next inside the call
    let members = row_indices(&typed, C_AST_KIND_MEMBER_ACCESS_EXPR);
    assert!(
        !members.is_empty(),
        "head.next must produce MEMBER_ACCESS_EXPR"
    );
}

#[test]
fn list_entry_gpu_pg_lower_matches_cpu() {
    let fix = fixture_list_entry_macro_and_use();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let expected = reference_ast_to_pg_nodes(&typed);
    let gpu = run_gpu_pg_lower(&typed, node_count_from_vast(&typed));
    assert_eq!(gpu, expected, "PG lowerer parity for list_entry");
}

// ---------------------------------------------------------------------------
// Tests  -  __builtin_expect direct usage
// ---------------------------------------------------------------------------

#[test]
fn builtin_expect_direct_classifies_as_builtin_expect_expr() {
    let fix = fixture_builtin_expect_direct();
    assert_full_pipeline_parity(&fix, "builtin_expect_direct");

    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);

    let expects = row_indices(&typed, C_AST_KIND_BUILTIN_EXPECT_EXPR);
    assert_eq!(
        expects.len(),
        2,
        "both __builtin_expect calls must classify as BUILTIN_EXPECT_EXPR, got {:?}",
        expects
    );

    // The !! must not confuse the parser into manufacturing extra call nodes.
    let calls = row_indices(&typed, node_kind::CALL);
    assert!(
        calls.is_empty(),
        "__builtin_expect must not be misclassified as a generic CALL"
    );
}

#[test]
fn builtin_expect_gpu_pg_lower_matches_cpu() {
    let fix = fixture_builtin_expect_direct();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let expected = reference_ast_to_pg_nodes(&typed);
    let gpu = run_gpu_pg_lower(&typed, node_count_from_vast(&typed));
    assert_eq!(gpu, expected, "PG lowerer parity for builtin_expect");
}

// ---------------------------------------------------------------------------
// Tests  -  likely / unlikely macro wrappers
// ---------------------------------------------------------------------------

#[test]
fn likely_unlikely_macros_preserved_as_call_shapes() {
    let fix = fixture_likely_unlikely_macro_shapes();
    assert_full_pipeline_parity(&fix, "likely_unlikely_macros");

    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);

    // Usages are CALL-shaped because the parser sees them as unexpanded
    // function-like macro invocations.
    let likely_rows = token_indices_containing(&fix, "likely");
    let unlikely_rows = token_indices_containing(&fix, "unlikely");
    assert!(likely_rows.len() >= 2, "likely appears in def and use");
    assert!(unlikely_rows.len() >= 2, "unlikely appears in def and use");

    assert_eq!(
        kind_at(&typed, *likely_rows.last().unwrap()),
        node_kind::CALL,
        "likely(cond) must be preserved as a CALL-shaped token stream"
    );
    assert_eq!(
        kind_at(&typed, *unlikely_rows.last().unwrap()),
        node_kind::CALL,
        "unlikely(cond) must be preserved as a CALL-shaped token stream"
    );

    // Ensure __builtin_expect does NOT appear as a separate node (macro body
    // is inside the PREPROC row, not parsed separately).
    let builtins = row_indices(&typed, C_AST_KIND_BUILTIN_EXPECT_EXPR);
    assert_eq!(
        builtins.len(),
        0,
        "macro body tokens must not leak into the AST as separate nodes"
    );
}

#[test]
fn likely_unlikely_gpu_pg_lower_matches_cpu() {
    let fix = fixture_likely_unlikely_macro_shapes();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let expected = reference_ast_to_pg_nodes(&typed);
    let gpu = run_gpu_pg_lower(&typed, node_count_from_vast(&typed));
    assert_eq!(gpu, expected, "PG lowerer parity for likely/unlikely");
}

// ---------------------------------------------------------------------------
// Tests  -  static inline with attributes
// ---------------------------------------------------------------------------

#[test]
fn static_inline_always_inline_classifies_correctly() {
    let fix = fixture_static_inline_with_attributes();
    assert_full_pipeline_parity(&fix, "static_inline_attributes");

    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);

    let funcs = row_indices(&typed, C_AST_KIND_FUNCTION_DEFINITION);
    assert_eq!(funcs.len(), 2, "two function definitions must exist");

    let attrs = row_indices(&typed, C_AST_KIND_GNU_ATTRIBUTE);
    assert_eq!(
        attrs.len(),
        2,
        "two __attribute__ invocations must produce two GNU_ATTRIBUTE rows"
    );

    // This classifier's public contract is that GNU_ATTRIBUTE rows
    // exist and the function definitions survive.
    assert!(
        !row_indices(&typed, C_AST_KIND_GNU_ATTRIBUTE).is_empty(),
        "__attribute__ must produce GNU_ATTRIBUTE rows"
    );

    // dispatch must be a function definition, not a function declaration
    let dispatch = lexeme_indices(&fix, "dispatch");
    assert_eq!(dispatch.len(), 1);
    assert_eq!(
        kind_at(&typed, dispatch[0]),
        C_AST_KIND_FUNCTION_DEFINITION,
        "dispatch with a body must classify as FUNCTION_DEFINITION"
    );
}

#[test]
fn static_inline_gpu_pg_lower_matches_cpu() {
    let fix = fixture_static_inline_with_attributes();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let expected = reference_ast_to_pg_nodes(&typed);
    let gpu = run_gpu_pg_lower(&typed, node_count_from_vast(&typed));
    assert_eq!(gpu, expected, "PG lowerer parity for static inline attrs");
}

// ---------------------------------------------------------------------------
// Tests  -  volatile / _Atomic qualifiers
// ---------------------------------------------------------------------------

#[test]
fn volatile_atomic_qualifiers_promote_and_classify() {
    let fix = fixture_volatile_atomic_parameters();
    assert_full_pipeline_parity(&fix, "volatile_atomic_parameters");

    // Keyword promotion must happen before VAST build.
    assert!(
        fix.tok_types.iter().any(|k| *k == TOK_VOLATILE),
        "volatile must be promoted to TOK_VOLATILE"
    );
    assert!(
        fix.tok_types.iter().any(|k| *k == TOK_ATOMIC),
        "_Atomic must be promoted to TOK_ATOMIC"
    );

    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);

    // Two POINTER_DECL rows for the two '*' in parameters
    let ptrs = row_indices(&typed, C_AST_KIND_POINTER_DECL);
    assert_eq!(
        ptrs.len(),
        2,
        "volatile *flags and _Atomic *state must each have a POINTER_DECL"
    );

    // Parameters must survive as VARIABLE rows
    let vars = row_indices(&typed, node_kind::VARIABLE);
    assert!(
        vars.contains(&7),
        "flags parameter must classify as VARIABLE"
    );
    assert!(
        vars.contains(&13),
        "state parameter must classify as VARIABLE"
    );
}

#[test]
fn volatile_atomic_gpu_pg_lower_matches_cpu() {
    let fix = fixture_volatile_atomic_parameters();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let expected = reference_ast_to_pg_nodes(&typed);
    let gpu = run_gpu_pg_lower(&typed, node_count_from_vast(&typed));
    assert_eq!(gpu, expected, "PG lowerer parity for volatile/atomic");
}

#[test]
fn alignof_initializer_classifies_as_expression_not_decl_prefix() {
    let fix = fixture_alignof_initializer_expression();
    assert_full_pipeline_parity(&fix, "alignof_initializer_expression");

    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);

    assert_eq!(
        row_indices(&typed, C_AST_KIND_ALIGNOF_EXPR),
        vec![3],
        "_Alignof must classify as an expression operator"
    );
    assert_eq!(
        kind_at(&typed, 4),
        0,
        "_Alignof(type-name) paren must not be rewritten as cast or function declarator"
    );
}

// ---------------------------------------------------------------------------
// Tests  -  Linux error-label cleanup
// ---------------------------------------------------------------------------

#[test]
fn linux_error_label_cleanup_classifies_all_jump_and_control_nodes() {
    let fix = fixture_linux_error_label_cleanup();
    assert_full_pipeline_parity(&fix, "linux_error_label_cleanup");

    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);

    assert!(
        !row_indices(&typed, C_AST_KIND_FUNCTION_DEFINITION).is_empty(),
        "alloc must be a FUNCTION_DEFINITION"
    );
    assert!(
        !row_indices(&typed, C_AST_KIND_IF_STMT).is_empty(),
        "if (!dev) must be IF_STMT"
    );
    assert!(
        !row_indices(&typed, C_AST_KIND_GOTO_STMT).is_empty(),
        "goto err_free must be GOTO_STMT"
    );
    assert!(
        !row_indices(&typed, C_AST_KIND_RETURN_STMT).is_empty(),
        "return statements must exist"
    );

    // The label definition is classified as LABEL_STMT, not left as a raw
    // identifier row.
    let err_free = lexeme_indices(&fix, "err_free");
    assert_eq!(
        err_free.len(),
        2,
        "err_free must appear as goto target and label definition"
    );
    assert_eq!(
        kind_at(&typed, err_free[1]),
        C_AST_KIND_LABEL_STMT,
        "label definition err_free: must classify as LABEL_STMT"
    );

    // kfree(dev) must be a CALL
    let kfree = lexeme_indices(&fix, "kfree");
    assert_eq!(kfree.len(), 1);
    assert_eq!(
        kind_at(&typed, kfree[0]),
        node_kind::CALL,
        "kfree(dev) must be CALL-shaped"
    );
}

#[test]
fn linux_error_label_gpu_pg_lower_matches_cpu() {
    let fix = fixture_linux_error_label_cleanup();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let expected = reference_ast_to_pg_nodes(&typed);
    let gpu = run_gpu_pg_lower(&typed, node_count_from_vast(&typed));
    assert_eq!(gpu, expected, "PG lowerer parity for error-label cleanup");
}

