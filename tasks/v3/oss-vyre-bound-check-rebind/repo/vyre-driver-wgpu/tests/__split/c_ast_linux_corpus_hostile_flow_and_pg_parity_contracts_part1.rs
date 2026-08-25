use super::*;

#[test]
fn dense_switch_with_fallthrough_classifies_all_stmt_kinds() {
    let fix = fixture_dense_switch_with_fallthrough();
    assert_full_pipeline_parity(&fix, "dense_switch_with_fallthrough");

    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);

    assert!(
        !row_indices(&typed, C_AST_KIND_SWITCH_STMT).is_empty(),
        "switch must produce SWITCH_STMT"
    );
    assert!(
        !row_indices(&typed, C_AST_KIND_CASE_STMT).is_empty(),
        "case must produce CASE_STMT"
    );
    assert!(
        !row_indices(&typed, C_AST_KIND_DEFAULT_STMT).is_empty(),
        "default must produce DEFAULT_STMT"
    );
    assert!(
        !row_indices(&typed, C_AST_KIND_BREAK_STMT).is_empty(),
        "break must produce BREAK_STMT"
    );
    assert!(
        !row_indices(&typed, C_AST_KIND_GNU_ATTRIBUTE).is_empty(),
        "__attribute__ must produce GNU_ATTRIBUTE"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ATTRIBUTE_FALLTHROUGH),
        vec![25],
        "fallthrough attribute name must produce ATTRIBUTE_FALLTHROUGH"
    );

    // PG lowerer parity
    assert_gpu_pg_parity(&fix, &typed, "dense_switch_with_fallthrough");
}

#[test]
fn dense_switch_pg_preserves_switch_and_case_kinds() {
    let fix = fixture_dense_switch_with_fallthrough();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let pg = reference_ast_to_pg_nodes(&typed);

    for idx in row_indices(&typed, C_AST_KIND_SWITCH_STMT) {
        assert_pg_preserves_row(
            &typed,
            &pg,
            &fix.tok_starts,
            &fix.tok_lens,
            idx,
            C_AST_KIND_SWITCH_STMT,
        );
    }
    for idx in row_indices(&typed, C_AST_KIND_CASE_STMT) {
        assert_pg_preserves_row(
            &typed,
            &pg,
            &fix.tok_starts,
            &fix.tok_lens,
            idx,
            C_AST_KIND_CASE_STMT,
        );
    }
    for idx in row_indices(&typed, C_AST_KIND_BREAK_STMT) {
        assert_pg_preserves_row(
            &typed,
            &pg,
            &fix.tok_starts,
            &fix.tok_lens,
            idx,
            C_AST_KIND_BREAK_STMT,
        );
    }
}

// ---------------------------------------------------------------------------
// Tests  -  statement expression in if condition
// ---------------------------------------------------------------------------

#[test]
fn stmt_expr_in_if_condition_classifies_basic_block_and_if() {
    let fix = fixture_stmt_expr_in_if_condition();
    assert_full_pipeline_parity(&fix, "stmt_expr_in_if_condition");

    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);

    assert!(
        !row_indices(&typed, C_AST_KIND_IF_STMT).is_empty(),
        "if must produce IF_STMT"
    );
    assert!(
        !row_indices(&typed, node_kind::BASIC_BLOCK).is_empty(),
        "statement expression body must produce BASIC_BLOCK"
    );

    // check() call inside the stmt expr
    let check = lexeme_indices(&fix, "check");
    assert_eq!(check.len(), 1);
    assert_eq!(
        kind_at(&typed, check[0]),
        node_kind::CALL,
        "check() inside stmt expr must be CALL"
    );

    assert_gpu_pg_parity(&fix, &typed, "stmt_expr_in_if_condition");
}

// ---------------------------------------------------------------------------
// Tests  -  statement expression in while condition
// ---------------------------------------------------------------------------

#[test]
fn stmt_expr_in_while_condition_classifies_basic_block_and_while() {
    let fix = fixture_stmt_expr_in_while_condition();
    assert_full_pipeline_parity(&fix, "stmt_expr_in_while_condition");

    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);

    assert!(
        !row_indices(&typed, C_AST_KIND_WHILE_STMT).is_empty(),
        "while must produce WHILE_STMT"
    );
    assert!(
        !row_indices(&typed, node_kind::BASIC_BLOCK).is_empty(),
        "statement expression body must produce BASIC_BLOCK"
    );

    let poll = lexeme_indices(&fix, "poll");
    assert_eq!(poll.len(), 1);
    assert_eq!(
        kind_at(&typed, poll[0]),
        node_kind::CALL,
        "poll() inside stmt expr must be CALL"
    );

    assert_gpu_pg_parity(&fix, &typed, "stmt_expr_in_while_condition");
}

// ---------------------------------------------------------------------------
// Tests  -  nested statement expressions
// ---------------------------------------------------------------------------

#[test]
fn nested_stmt_expr_classifies_multiple_basic_blocks() {
    let fix = fixture_nested_stmt_expr();
    assert_full_pipeline_parity(&fix, "nested_stmt_expr");

    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);

    let bbs = row_indices(&typed, node_kind::BASIC_BLOCK);
    assert_eq!(
        bbs.len(),
        2,
        "nested statement expressions must produce exactly 2 BASIC_BLOCK rows, got {:?}",
        bbs
    );

    // Inner assignment z = 1 and outer assignment y = (...)
    let assigns = row_indices(&typed, C_AST_KIND_ASSIGN_EXPR);
    // The classifier emits ASSIGN_EXPR for '=' inside statement expressions.
    assert!(
        !assigns.is_empty(),
        "nested stmt expr must contain at least one ASSIGN_EXPR row"
    );

    assert_gpu_pg_parity(&fix, &typed, "nested_stmt_expr");
}

// ---------------------------------------------------------------------------
// Tests  -  computed goto with statement expression
// ---------------------------------------------------------------------------

#[test]
fn computed_goto_stmt_expr_classifies_goto_and_label_address() {
    let fix = fixture_computed_goto_stmt_expr();
    assert_full_pipeline_parity(&fix, "computed_goto_stmt_expr");

    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);

    assert!(
        !row_indices(&typed, C_AST_KIND_GOTO_STMT).is_empty(),
        "goto *t must produce GOTO_STMT"
    );
    assert!(
        !row_indices(&typed, C_AST_KIND_RETURN_STMT).is_empty(),
        "return must produce RETURN_STMT"
    );
    assert!(
        !row_indices(&typed, node_kind::BASIC_BLOCK).is_empty(),
        "statement expression body must produce BASIC_BLOCK"
    );

    // The label definition is classified as LABEL_STMT.
    let label = lexeme_indices(&fix, "label");
    assert_eq!(label.len(), 2, "label appears in &&label and label:");
    assert_eq!(
        kind_at(&typed, label[1]),
        C_AST_KIND_LABEL_STMT,
        "label definition must classify as LABEL_STMT"
    );

    assert_gpu_pg_parity(&fix, &typed, "computed_goto_stmt_expr");
}

// ---------------------------------------------------------------------------
// Tests  -  empty switch body
// ---------------------------------------------------------------------------

#[test]
fn empty_switch_does_not_panic_and_produces_switch_stmt() {
    let fix = fixture_empty_switch();
    assert_full_pipeline_parity(&fix, "empty_switch");

    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    assert!(
        !row_indices(&typed, C_AST_KIND_SWITCH_STMT).is_empty(),
        "empty switch must still produce SWITCH_STMT"
    );
    assert!(
        row_indices(&typed, C_AST_KIND_CASE_STMT).is_empty(),
        "empty switch must not manufacture phantom CASE_STMT rows"
    );
    assert!(
        row_indices(&typed, C_AST_KIND_DEFAULT_STMT).is_empty(),
        "empty switch must not manufacture phantom DEFAULT_STMT rows"
    );

    assert_gpu_pg_parity(&fix, &typed, "empty_switch");
}

// ---------------------------------------------------------------------------
// Tests  -  goto inside statement expression
// ---------------------------------------------------------------------------

#[test]
fn goto_inside_stmt_expr_classifies_basic_block_and_goto() {
    let fix = fixture_goto_inside_stmt_expr();
    assert_full_pipeline_parity(&fix, "goto_inside_stmt_expr");

    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);

    assert!(
        !row_indices(&typed, C_AST_KIND_GOTO_STMT).is_empty(),
        "goto inner must produce GOTO_STMT"
    );
    assert!(
        !row_indices(&typed, node_kind::BASIC_BLOCK).is_empty(),
        "statement expression body must produce BASIC_BLOCK"
    );

    // The inner label definition is classified as LABEL_STMT.
    let inner = lexeme_indices(&fix, "inner");
    assert_eq!(
        inner.len(),
        2,
        "inner appears as goto target and label definition"
    );
    assert_eq!(
        kind_at(&typed, inner[1]),
        C_AST_KIND_LABEL_STMT,
        "label definition inner: must classify as LABEL_STMT"
    );

    assert_gpu_pg_parity(&fix, &typed, "goto_inside_stmt_expr");
}

// ---------------------------------------------------------------------------
// Hostile combined fixture  -  everything mixed
// ---------------------------------------------------------------------------

/// ```c
/// void hostile(int v) {
///     switch (v) {
///     case 0:
///         if (({ int t = v; t; }))
///             goto out;
///         __attribute__((fallthrough));
///     case 1:
///         while (({ int u = 0; u; })) { break; }
///         break;
///     default:
///         return;
///     }
/// out:
///     return;
/// }
/// ```
pub(super) fn fixture_hostile_mixed_flow() -> Fixture {
    build_fixture(&[
        FixtureToken::new("void", TOK_IDENTIFIER),
        FixtureToken::new("hostile", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new("v", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("switch", TOK_SWITCH),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("v", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("case", TOK_CASE),
        FixtureToken::new("0", TOK_INTEGER),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new("if", TOK_IF),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new("t", TOK_IDENTIFIER),
        FixtureToken::new("=", TOK_ASSIGN),
        FixtureToken::new("v", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("t", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("goto", TOK_GOTO),
        FixtureToken::new("out", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("__attribute__", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("fallthrough", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("case", TOK_CASE),
        FixtureToken::new("1", TOK_INTEGER),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new("while", TOK_WHILE),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new("u", TOK_IDENTIFIER),
        FixtureToken::new("=", TOK_ASSIGN),
        FixtureToken::new("0", TOK_INTEGER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("u", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("break", TOK_BREAK),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
        FixtureToken::new("break", TOK_BREAK),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("default", TOK_DEFAULT),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new("return", TOK_RETURN),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
        FixtureToken::new("out", TOK_IDENTIFIER),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new("return", TOK_RETURN),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
    ])
}

