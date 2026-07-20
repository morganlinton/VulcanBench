use super::*;

#[test]
fn cpu_multiple_consecutive_labels_classify() {
    let fix = fixture_multiple_consecutive_labels();
    let typed = classify(&fix);
    let labels = row_indices(&typed, C_AST_KIND_LABEL_STMT);
    assert_eq!(
        labels,
        vec![5, 7, 9],
        "consecutive labels a, b, c must classify as LABEL_STMT at rows 5, 7, 9"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_RETURN_STMT),
        vec![11],
        "return must classify as RETURN_STMT"
    );
}

#[test]
fn cpu_label_inside_if_else_classify() {
    let fix = fixture_label_inside_if_else();
    let typed = classify(&fix);
    assert_eq!(
        row_indices(&typed, C_AST_KIND_IF_STMT),
        vec![7],
        "if must classify as IF_STMT"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_LABEL_STMT),
        vec![12, 19],
        "label1 and label2 must classify as LABEL_STMT"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_RETURN_STMT),
        vec![14, 21],
        "returns inside if/else must classify as RETURN_STMT"
    );
}

#[test]
fn cpu_label_inside_switch_case_classify() {
    let fix = fixture_label_inside_switch_case();
    let typed = classify(&fix);
    assert_eq!(
        row_indices(&typed, C_AST_KIND_SWITCH_STMT),
        vec![7],
        "switch must classify as SWITCH_STMT"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_CASE_STMT),
        vec![12],
        "case must classify as CASE_STMT"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_DEFAULT_STMT),
        vec![19],
        "default must classify as DEFAULT_STMT"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_LABEL_STMT),
        vec![15],
        "inner label inside switch case must classify as LABEL_STMT"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_RETURN_STMT),
        vec![17, 21],
        "returns must classify as RETURN_STMT"
    );
}

#[test]
fn cpu_label_inside_loop_bodies_classify() {
    let fix = fixture_label_inside_loops();
    let typed = classify(&fix);
    assert_eq!(
        row_indices(&typed, C_AST_KIND_FOR_STMT),
        vec![5],
        "for must classify as FOR_STMT"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_WHILE_STMT),
        vec![16, 33],
        "while statements must classify as WHILE_STMT"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_DO_STMT),
        vec![26],
        "do must classify as DO_STMT"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_LABEL_STMT),
        vec![11, 21, 28],
        "loop labels must classify as LABEL_STMT"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_BREAK_STMT),
        vec![13, 23, 30],
        "break statements must classify as BREAK_STMT"
    );
}

#[test]
fn cpu_forward_goto_into_nested_if_classify() {
    let fix = fixture_forward_goto_into_nested_if();
    let typed = classify(&fix);
    assert_eq!(
        row_indices(&typed, C_AST_KIND_GOTO_STMT),
        vec![5],
        "goto must classify as GOTO_STMT"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_LABEL_STMT),
        vec![13],
        "forward target label inside if-body must classify as LABEL_STMT"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_IF_STMT),
        vec![8],
        "if must classify as IF_STMT"
    );
}

#[test]
fn cpu_backward_goto_from_nested_block_classify() {
    let fix = fixture_backward_goto_from_nested_block();
    let typed = classify(&fix);
    assert_eq!(
        row_indices(&typed, C_AST_KIND_GOTO_STMT),
        vec![10],
        "goto from nested block must classify as GOTO_STMT"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_LABEL_STMT),
        vec![14],
        "backward target label must classify as LABEL_STMT"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_IF_STMT),
        vec![5],
        "if must classify as IF_STMT"
    );
}

// ---------------------------------------------------------------------------
// Tests – CPU reference contracts (statement expressions)
// ---------------------------------------------------------------------------

#[test]
fn cpu_statement_expression_simple_classify() {
    let fix = fixture_statement_expression_simple();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);

    // Structural gap check: the `(` must have `{` as its first child for the
    // classifier to recognise a statement expression.
    assert_eq!(
        word_at(&raw, 3 * VAST_STRIDE_U32 + 2),
        4,
        "statement-expression `(` must have `{{` as first child in raw VAST"
    );

    assert_eq!(
        row_indices(&typed, C_AST_KIND_GNU_STATEMENT_EXPR),
        vec![3],
        "statement-expression introducer `(` must classify as GNU_STATEMENT_EXPR"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ASSIGN_EXPR),
        vec![7],
        "assignment inside statement expression must classify as ASSIGN_EXPR"
    );
    assert!(
        !row_indices(&typed, node_kind::BASIC_BLOCK).is_empty(),
        "statement expression must contain a BASIC_BLOCK for the brace body"
    );
}

#[test]
fn cpu_statement_expression_in_array_init_classify() {
    let fix = fixture_statement_expression_in_array_init();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    // Verify the builder still links statement-expr `(` to `{` inside an
    // initializer list  -  a common parser gap.
    assert_eq!(
        word_at(&raw, 7 * VAST_STRIDE_U32 + 2),
        8,
        "array-init statement-expr `(` must have `{{` as first child"
    );
    assert_eq!(
        word_at(&raw, 14 * VAST_STRIDE_U32 + 2),
        15,
        "second array-init statement-expr `(` must have `{{` as first child"
    );

    assert_eq!(
        row_indices(&typed, C_AST_KIND_GNU_STATEMENT_EXPR),
        vec![7, 14],
        "statement expressions in array initializer must classify as GNU_STATEMENT_EXPR"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_INITIALIZER_LIST),
        vec![6],
        "outer brace must classify as INITIALIZER_LIST"
    );
}

#[test]
fn cpu_statement_expression_in_struct_designated_init_classify() {
    let fix = fixture_statement_expression_in_struct_designated_init();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    assert_eq!(
        word_at(&raw, 17 * VAST_STRIDE_U32 + 2),
        18,
        "designated-init statement-expr `(` must have `{{` as first child"
    );

    assert_eq!(
        row_indices(&typed, C_AST_KIND_GNU_STATEMENT_EXPR),
        vec![16],
        "statement expression in designated initializer must classify as GNU_STATEMENT_EXPR"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_INITIALIZER_LIST),
        vec![12],
        "struct initializer brace must classify as INITIALIZER_LIST"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_MEMBER_ACCESS_EXPR),
        vec![13],
        "dot designator must classify as MEMBER_ACCESS_EXPR"
    );
}

#[test]
fn cpu_nested_statement_expression_classify() {
    let fix = fixture_nested_statement_expression();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    assert_eq!(
        word_at(&raw, 3 * VAST_STRIDE_U32 + 2),
        4,
        "outer statement-expr `(` must have `{{` as first child"
    );
    assert_eq!(
        word_at(&raw, 8 * VAST_STRIDE_U32 + 2),
        9,
        "inner statement-expr `(` must have `{{` as first child"
    );

    let stmt_exprs = row_indices(&typed, C_AST_KIND_GNU_STATEMENT_EXPR);
    assert_eq!(
        stmt_exprs,
        vec![3, 8],
        "outer and inner statement expression `(` tokens must classify as GNU_STATEMENT_EXPR"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ASSIGN_EXPR),
        vec![7],
        "assignment inside outer statement expression must classify as ASSIGN_EXPR"
    );
}

#[test]
fn cpu_statement_expression_with_label_and_goto_classify() {
    let fix = fixture_statement_expression_with_label_and_goto();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    assert_eq!(
        word_at(&raw, 3 * VAST_STRIDE_U32 + 2),
        4,
        "statement-expr `(` must have `{{` as first child"
    );

    assert_eq!(
        row_indices(&typed, C_AST_KIND_GNU_STATEMENT_EXPR),
        vec![3],
        "statement expression with label/goto must classify as GNU_STATEMENT_EXPR"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_GOTO_STMT),
        vec![5],
        "goto inside statement expression must classify as GOTO_STMT"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_LABEL_STMT),
        vec![8],
        "label inside statement expression must classify as LABEL_STMT"
    );
}

// ---------------------------------------------------------------------------
// Tests – GPU parity contracts (full pipeline)
// ---------------------------------------------------------------------------

#[test]
fn gpu_parity_multiple_consecutive_labels() {
    let fix = fixture_multiple_consecutive_labels();
    assert_full_pipeline_parity(&fix, "multiple_consecutive_labels");
}

#[test]
fn gpu_parity_label_inside_if_else() {
    let fix = fixture_label_inside_if_else();
    assert_full_pipeline_parity(&fix, "label_inside_if_else");
}

#[test]
fn gpu_parity_label_inside_switch_case() {
    let fix = fixture_label_inside_switch_case();
    assert_full_pipeline_parity(&fix, "label_inside_switch_case");
}

#[test]
fn gpu_parity_label_inside_loops() {
    let fix = fixture_label_inside_loops();
    assert_full_pipeline_parity(&fix, "label_inside_loops");
}

#[test]
fn gpu_parity_forward_goto_into_nested_if() {
    let fix = fixture_forward_goto_into_nested_if();
    assert_full_pipeline_parity(&fix, "forward_goto_into_nested_if");
}

#[test]
fn gpu_parity_backward_goto_from_nested_block() {
    let fix = fixture_backward_goto_from_nested_block();
    assert_full_pipeline_parity(&fix, "backward_goto_from_nested_block");
}

#[test]
fn gpu_parity_statement_expression_simple() {
    let fix = fixture_statement_expression_simple();
    assert_full_pipeline_parity(&fix, "statement_expression_simple");
}

#[test]
fn gpu_parity_statement_expression_in_array_init() {
    let fix = fixture_statement_expression_in_array_init();
    assert_full_pipeline_parity(&fix, "statement_expression_in_array_init");
}

#[test]
fn gpu_parity_statement_expression_in_struct_designated_init() {
    let fix = fixture_statement_expression_in_struct_designated_init();
    assert_full_pipeline_parity(&fix, "statement_expression_in_struct_designated_init");
}

#[test]
fn gpu_parity_nested_statement_expression() {
    let fix = fixture_nested_statement_expression();
    assert_full_pipeline_parity(&fix, "nested_statement_expression");
}

#[test]
fn gpu_parity_statement_expression_with_label_and_goto() {
    let fix = fixture_statement_expression_with_label_and_goto();
    assert_full_pipeline_parity(&fix, "statement_expression_with_label_and_goto");
}

// ---------------------------------------------------------------------------
// Tests – PG lowering preservation contracts
// ---------------------------------------------------------------------------

#[test]
fn pg_lower_preserves_label_stmt_kinds() {
    let fix = fixture_multiple_consecutive_labels();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let pg = reference_ast_to_pg_nodes(&typed);

    for idx in [5usize, 7, 9] {
        assert_pg_preserves_row(
            &typed,
            &pg,
            &fix.tok_starts,
            &fix.tok_lens,
            idx,
            C_AST_KIND_LABEL_STMT,
        );
    }
    assert_pg_preserves_row(
        &typed,
        &pg,
        &fix.tok_starts,
        &fix.tok_lens,
        11,
        C_AST_KIND_RETURN_STMT,
    );
}

