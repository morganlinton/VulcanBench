use super::*;

#[test]
fn scope_tree_struct_field_not_classified_as_declaration() {
    let fix = fixture(
        "struct_field",
        &[
            tok(TOK_STRUCT),
            ident("S"),
            tok(TOK_LBRACE),
            tok(TOK_INT),
            ident("x"),
            tok(TOK_SEMICOLON),
            tok(TOK_CHAR_KW),
            ident("y"),
            tok(TOK_SEMICOLON),
            tok(TOK_RBRACE),
            tok(TOK_SEMICOLON),
            tok(TOK_VOID),
            ident("f"),
            tok(TOK_LPAREN),
            tok(TOK_VOID),
            tok(TOK_RPAREN),
            tok(TOK_LBRACE),
            tok(TOK_INT),
            ident("x"),
            tok(TOK_SEMICOLON),
            tok(TOK_RBRACE),
        ],
    );
    let st = scope_tree_for(&fix);

    // struct field x at token 4
    assert_eq!(
        scope_tree_word_at(&st, 4, 2),
        { DECL_KIND_NONE },
        "struct field x must not be classified as declaration"
    );
    // struct field y at token 7
    assert_eq!(
        scope_tree_word_at(&st, 7, 2),
        { DECL_KIND_NONE },
        "struct field y must not be classified as declaration"
    );
    // variable x in function at token 18
    assert_eq!(
        scope_tree_word_at(&st, 18, 2),
        { DECL_KIND_VARIABLE },
        "function variable x must be VARIABLE"
    );
}

#[test]
fn annotation_struct_field_does_not_affect_typedef_visibility() {
    let fix = fixture(
        "field_td",
        &[
            tok(TOK_TYPEDEF),
            tok(TOK_INT),
            ident("T"),
            tok(TOK_SEMICOLON),
            tok(TOK_STRUCT),
            ident("S"),
            tok(TOK_LBRACE),
            tok(TOK_INT),
            ident("T"),
            tok(TOK_SEMICOLON),
            tok(TOK_RBRACE),
            tok(TOK_SEMICOLON),
            tok(TOK_VOID),
            ident("f"),
            tok(TOK_LPAREN),
            tok(TOK_VOID),
            tok(TOK_RPAREN),
            tok(TOK_LBRACE),
            tok(TOK_LPAREN),
            ident("T"),
            tok(TOK_RPAREN),
            tok(TOK_STAR),
            ident("p"),
            tok(TOK_SEMICOLON),
            tok(TOK_RBRACE),
        ],
    );
    let ann = annotate_cpu(&fix);
    let typed = classify_cpu_annotated(&fix);

    // struct field T at token 6 should not shadow typedef
    assert_ne!(
        flags_at(&ann, 19) & TYPEDEF_FLAG_VISIBLE,
        0,
        "typedef T must remain visible despite struct field"
    );
    assert_eq!(
        kind_at(&typed, 18),
        C_AST_KIND_CAST_EXPR,
        "(T) must be cast despite struct field"
    );
}

#[test]
fn scope_tree_nested_struct_fields_not_classified() {
    let fix = fixture(
        "nested_struct_field",
        &[
            tok(TOK_STRUCT),
            ident("Outer"),
            tok(TOK_LBRACE),
            tok(TOK_STRUCT),
            ident("Inner"),
            tok(TOK_LBRACE),
            tok(TOK_INT),
            ident("z"),
            tok(TOK_SEMICOLON),
            tok(TOK_RBRACE),
            tok(TOK_SEMICOLON),
            tok(TOK_RBRACE),
            tok(TOK_SEMICOLON),
        ],
    );
    let st = scope_tree_for(&fix);

    assert_eq!(
        scope_tree_word_at(&st, 7, 2),
        { DECL_KIND_NONE },
        "nested struct field z must not be declaration"
    );
}

// ---------------------------------------------------------------------------
// Edge: typedef in sizeof / _Alignof
// ---------------------------------------------------------------------------

#[test]
fn annotation_typedef_visible_in_sizeof_context() {
    let fix = fixture(
        "sizeof_td",
        &[
            tok(TOK_TYPEDEF),
            tok(TOK_INT),
            ident("T"),
            tok(TOK_SEMICOLON),
            tok(TOK_VOID),
            ident("f"),
            tok(TOK_LPAREN),
            tok(TOK_VOID),
            tok(TOK_RPAREN),
            tok(TOK_LBRACE),
            tok(TOK_SIZEOF),
            tok(TOK_LPAREN),
            ident("T"),
            tok(TOK_RPAREN),
            tok(TOK_SEMICOLON),
            tok(TOK_RBRACE),
        ],
    );
    let ann = annotate_cpu(&fix);

    assert_ne!(
        flags_at(&ann, 12) & TYPEDEF_FLAG_VISIBLE,
        0,
        "typedef T must be visible inside sizeof"
    );
}

// ---------------------------------------------------------------------------
// GPU parity
// ---------------------------------------------------------------------------

#[test]
fn gpu_parity_scope_tree_struct_fields() {
    let fix = fixture(
        "gpu_struct_field",
        &[
            tok(TOK_STRUCT),
            ident("S"),
            tok(TOK_LBRACE),
            tok(TOK_INT),
            ident("x"),
            tok(TOK_SEMICOLON),
            tok(TOK_RBRACE),
            tok(TOK_SEMICOLON),
            tok(TOK_VOID),
            ident("f"),
            tok(TOK_LPAREN),
            tok(TOK_VOID),
            tok(TOK_RPAREN),
            tok(TOK_LBRACE),
            tok(TOK_RBRACE),
        ],
    );
    let expected = scope_tree_for(&fix);
    let gpu = run_gpu_scope_tree(&fix);
    assert_eq!(
        gpu, expected,
        "GPU scope tree must match CPU for struct fields"
    );
}

#[test]
fn gpu_parity_classifier_cast_vs_declaration() {
    let fix = fixture(
        "gpu_cast_decl",
        &[
            tok(TOK_TYPEDEF),
            tok(TOK_INT),
            ident("T"),
            tok(TOK_SEMICOLON),
            tok(TOK_VOID),
            ident("f"),
            tok(TOK_LPAREN),
            tok(TOK_VOID),
            tok(TOK_RPAREN),
            tok(TOK_LBRACE),
            tok(TOK_LPAREN),
            ident("T"),
            tok(TOK_RPAREN),
            ident("x"),
            tok(TOK_SEMICOLON),
            ident("T"),
            tok(TOK_STAR),
            ident("p"),
            tok(TOK_SEMICOLON),
            tok(TOK_RBRACE),
        ],
    );
    let expected_ann = annotate_cpu(&fix);
    let gpu_ann = run_gpu_annotate(&fix);
    assert_eq!(
        gpu_ann, expected_ann,
        "GPU annotation must match CPU for cast vs decl"
    );

    let expected_typed = reference_c11_classify_vast_node_kinds(&expected_ann);
    let gpu_typed = run_gpu_classify(&gpu_ann, fix.tok_types.len());
    assert_eq!(
        gpu_typed, expected_typed,
        "GPU classifier must match CPU for cast vs decl"
    );
}

#[test]
fn gpu_parity_scope_tree_redeclaration() {
    let fix = fixture(
        "gpu_redecl",
        &[
            tok(TOK_INT),
            ident("x"),
            tok(TOK_SEMICOLON),
            tok(TOK_INT),
            ident("x"),
            tok(TOK_SEMICOLON),
        ],
    );
    let expected = scope_tree_for(&fix);
    let gpu = run_gpu_scope_tree(&fix);
    assert_eq!(
        gpu, expected,
        "GPU scope tree must match CPU for redeclaration"
    );
}

#[test]
fn gpu_parity_annotation_struct_field_typedef() {
    let fix = fixture(
        "gpu_field_td",
        &[
            tok(TOK_TYPEDEF),
            tok(TOK_INT),
            ident("T"),
            tok(TOK_SEMICOLON),
            tok(TOK_STRUCT),
            ident("S"),
            tok(TOK_LBRACE),
            tok(TOK_INT),
            ident("T"),
            tok(TOK_SEMICOLON),
            tok(TOK_RBRACE),
            tok(TOK_SEMICOLON),
            tok(TOK_VOID),
            ident("f"),
            tok(TOK_LPAREN),
            tok(TOK_VOID),
            tok(TOK_RPAREN),
            tok(TOK_LBRACE),
            tok(TOK_LPAREN),
            ident("T"),
            tok(TOK_RPAREN),
            tok(TOK_STAR),
            ident("p"),
            tok(TOK_SEMICOLON),
            tok(TOK_RBRACE),
        ],
    );
    let expected_ann = annotate_cpu(&fix);
    let gpu_ann = run_gpu_annotate(&fix);
    assert_eq!(
        gpu_ann, expected_ann,
        "GPU annotation must match CPU for struct field+typedef"
    );
}

#[test]
fn gpu_parity_classifier_shadowed_typedef_declaration() {
    let fix = fixture(
        "gpu_shadow_decl",
        &[
            tok(TOK_TYPEDEF),
            tok(TOK_INT),
            ident("T"),
            tok(TOK_SEMICOLON),
            tok(TOK_VOID),
            ident("f"),
            tok(TOK_LPAREN),
            tok(TOK_VOID),
            tok(TOK_RPAREN),
            tok(TOK_LBRACE),
            tok(TOK_INT),
            ident("T"),
            tok(TOK_SEMICOLON),
            ident("T"),
            tok(TOK_STAR),
            ident("p"),
            tok(TOK_SEMICOLON),
            tok(TOK_RBRACE),
        ],
    );
    let expected_ann = annotate_cpu(&fix);
    let gpu_ann = run_gpu_annotate(&fix);
    assert_eq!(gpu_ann, expected_ann);

    let expected_typed = reference_c11_classify_vast_node_kinds(&expected_ann);
    let gpu_typed = run_gpu_classify(&gpu_ann, fix.tok_types.len());
    assert_eq!(
        gpu_typed, expected_typed,
        "GPU classifier must match CPU for shadowed typedef in decl"
    );
}
