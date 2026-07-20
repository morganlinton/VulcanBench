use super::*;

#[test]
fn loops_break_continue_classify() {
    let fix = fixture_loops_break_continue();
    assert_full_pipeline_parity(&fix, "loops_break_continue");

    let typed = classify(&fix);
    assert_eq!(
        row_indices(&typed, C_AST_KIND_FOR_STMT),
        vec![5],
        "for must classify as FOR_STMT"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_WHILE_STMT),
        vec![12, 21],
        "while statements must classify as WHILE_STMT"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_DO_STMT),
        vec![18],
        "do must classify as DO_STMT"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_BREAK_STMT),
        vec![10],
        "break must classify as BREAK_STMT"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_CONTINUE_STMT),
        vec![16],
        "continue must classify as CONTINUE_STMT"
    );
}

#[test]
fn pg_lower_preserves_loop_and_jump_rows() {
    let fix = fixture_loops_break_continue();
    let typed = classify(&fix);
    let pg = reference_ast_to_pg_nodes(&typed);
    assert_pg_preserves_row(&typed, &pg, &fix, 5, C_AST_KIND_FOR_STMT);
    assert_pg_preserves_row(&typed, &pg, &fix, 10, C_AST_KIND_BREAK_STMT);
    assert_pg_preserves_row(&typed, &pg, &fix, 12, C_AST_KIND_WHILE_STMT);
    assert_pg_preserves_row(&typed, &pg, &fix, 16, C_AST_KIND_CONTINUE_STMT);
    assert_pg_preserves_row(&typed, &pg, &fix, 18, C_AST_KIND_DO_STMT);
}

// ---------------------------------------------------------------------------
// Tests – return
// ---------------------------------------------------------------------------

#[test]
fn return_variants_classify() {
    let fix = fixture_return_variants();
    assert_full_pipeline_parity(&fix, "return_variants");

    let typed = classify(&fix);
    assert_eq!(
        row_indices(&typed, C_AST_KIND_RETURN_STMT),
        vec![5, 7],
        "both returns must classify as RETURN_STMT"
    );
}

#[test]
fn pg_lower_preserves_return_rows() {
    let fix = fixture_return_variants();
    let typed = classify(&fix);
    let pg = reference_ast_to_pg_nodes(&typed);
    assert_pg_preserves_row(&typed, &pg, &fix, 5, C_AST_KIND_RETURN_STMT);
    assert_pg_preserves_row(&typed, &pg, &fix, 7, C_AST_KIND_RETURN_STMT);
}

// ---------------------------------------------------------------------------
// Tests – GNU statement expressions
// ---------------------------------------------------------------------------

#[test]
fn statement_expr_simple_classifies_and_links() {
    let fix = fixture_statement_expr_simple();
    assert_full_pipeline_parity(&fix, "statement_expr_simple");

    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    assert_eq!(
        row_indices(&typed, C_AST_KIND_GNU_STATEMENT_EXPR),
        vec![8],
        "statement-expression introducer `(` must be GNU_STATEMENT_EXPR"
    );
    assert_first_child(&raw, 8, 9);
    assert_eq!(
        word_at(&typed, 9 * VAST_STRIDE_U32),
        node_kind::BASIC_BLOCK,
        "statement-expression body brace must classify as BASIC_BLOCK"
    );
}

#[test]
fn statement_expr_with_goto_and_label_classifies() {
    let fix = fixture_statement_expr_with_goto_label();
    assert_full_pipeline_parity(&fix, "statement_expr_with_goto_label");

    let typed = classify(&fix);
    assert_eq!(
        row_indices(&typed, C_AST_KIND_GNU_STATEMENT_EXPR),
        vec![8],
        "statement expr with goto/label must classify as GNU_STATEMENT_EXPR"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_GOTO_STMT),
        vec![10],
        "goto inside statement expr must classify as GOTO_STMT"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_LABEL_STMT),
        vec![13],
        "label inside statement expr must classify as LABEL_STMT"
    );
}

#[test]
fn pg_lower_preserves_gnu_statement_expr_rows() {
    let fix = fixture_statement_expr_simple();
    let typed = classify(&fix);
    let pg = reference_ast_to_pg_nodes(&typed);
    assert_pg_preserves_row(&typed, &pg, &fix, 8, C_AST_KIND_GNU_STATEMENT_EXPR);
}

// ---------------------------------------------------------------------------
// Tests – GPU PG lowering parity
// ---------------------------------------------------------------------------

#[test]
fn gpu_pg_lower_matches_cpu_for_all_fixtures() {
    let fixtures: Vec<(&str, Fixture)> = vec![
        ("cast_simple", fixture_cast_simple()),
        ("cast_complex", fixture_cast_complex()),
        ("compound_literal_simple", fixture_compound_literal_simple()),
        ("cast_not_compound", fixture_cast_not_compound()),
        ("generic_selection", fixture_generic_selection()),
        ("sizeof_typename", fixture_sizeof_typename()),
        ("sizeof_expr", fixture_sizeof_expr()),
        ("alignof_typename", fixture_alignof_typename()),
        ("conditional_simple", fixture_conditional_simple()),
        ("conditional_nested", fixture_conditional_nested()),
        ("comma_in_return", fixture_comma_in_return()),
        ("multiple_labels", fixture_multiple_labels()),
        ("goto_forward", fixture_goto_forward()),
        ("goto_backward", fixture_goto_backward()),
        ("switch_case_fallthrough", fixture_switch_case_fallthrough()),
        ("switch_case_range", fixture_switch_case_range()),
        ("loops_break_continue", fixture_loops_break_continue()),
        ("return_variants", fixture_return_variants()),
        ("statement_expr_simple", fixture_statement_expr_simple()),
        (
            "statement_expr_with_goto_label",
            fixture_statement_expr_with_goto_label(),
        ),
    ];

    for (label, fix) in fixtures {
        let typed = classify(&fix);
        let expected = reference_ast_to_pg_nodes(&typed);
        let gpu = run_gpu_pg_lower(&typed);
        assert_eq!(
            gpu, expected,
            "GPU PG lowerer must match CPU for fixture `{label}`"
        );
    }
}
