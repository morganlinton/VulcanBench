#[test]
fn nested_compound_statements_preserve_blocks_and_return() {
    let fix = fixture_nested_compound_return();
    assert_full_pipeline_parity(&fix, "nested_compound_return");

    let typed = classify(&fix);
    assert_eq!(
        row_indices(&typed, node_kind::BASIC_BLOCK),
        vec![5, 6, 7],
        "function body and nested compound braces must classify as BASIC_BLOCK"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_RETURN_STMT),
        vec![8],
        "return inside nested compound must classify as RETURN_STMT"
    );
}

#[test]
fn pg_lower_preserves_statement_control_flow_rows() {
    let fix = fixture_default_do_break_continue_return();
    let typed = classify(&fix);
    let expected = reference_ast_to_pg_nodes(&typed);
    let gpu = run_gpu_pg_lower(&typed);
    assert_eq!(
        gpu, expected,
        "GPU PG lowerer must match CPU byte oracle for statement control flow"
    );

    for (idx, kind) in [
        (7usize, C_AST_KIND_SWITCH_STMT),
        (12, C_AST_KIND_DEFAULT_STMT),
        (14, C_AST_KIND_DO_STMT),
        (16, C_AST_KIND_CONTINUE_STMT),
        (19, C_AST_KIND_WHILE_STMT),
        (24, C_AST_KIND_CASE_STMT),
        (27, C_AST_KIND_BREAK_STMT),
        (30, C_AST_KIND_RETURN_STMT),
    ] {
        assert_pg_preserves_row(&typed, &expected, &fix, idx, kind);
    }
}

#[test]
fn pg_lower_preserves_label_and_goto_rows() {
    let fix = fixture_goto_across_switch_case();
    let typed = classify(&fix);
    let expected = reference_ast_to_pg_nodes(&typed);
    let gpu = run_gpu_pg_lower(&typed);
    assert_eq!(
        gpu, expected,
        "GPU PG lowerer must match CPU byte oracle for label/goto control flow"
    );

    for (idx, kind) in [
        (7usize, C_AST_KIND_SWITCH_STMT),
        (12, C_AST_KIND_CASE_STMT),
        (15, C_AST_KIND_IF_STMT),
        (20, C_AST_KIND_GOTO_STMT),
        (24, C_AST_KIND_LABEL_STMT),
    ] {
        assert_pg_preserves_row(&typed, &expected, &fix, idx, kind);
    }
}
