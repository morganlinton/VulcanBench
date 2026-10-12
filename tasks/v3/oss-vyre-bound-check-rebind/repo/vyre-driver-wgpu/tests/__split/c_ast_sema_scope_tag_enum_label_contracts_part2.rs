use super::*;

#[test]
fn annotation_label_does_not_affect_typedef_visibility() {
    let fix = fixture(
        "label_typedef",
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
            ident("T"),
            tok(TOK_COLON),
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

    // Label T at token 10 should not shadow typedef T
    assert_ne!(
        flags_at(&ann, 10) & TYPEDEF_FLAG_VISIBLE,
        0,
        "typedef T must remain visible despite label with same name"
    );
    assert_eq!(
        kind_at(&typed, 12),
        C_AST_KIND_CAST_EXPR,
        "(T) must remain cast despite label"
    );
}

// ---------------------------------------------------------------------------
// Typedef + tag same name
// ---------------------------------------------------------------------------

#[test]
fn annotation_struct_tag_and_typedef_same_name_both_visible() {
    let fix = fixture(
        "tag_typedef_same",
        &[
            tok(TOK_STRUCT),
            ident("S"),
            tok(TOK_LBRACE),
            tok(TOK_INT),
            ident("x"),
            tok(TOK_SEMICOLON),
            tok(TOK_RBRACE),
            tok(TOK_SEMICOLON),
            tok(TOK_TYPEDEF),
            tok(TOK_STRUCT),
            ident("S"),
            ident("S"),
            tok(TOK_SEMICOLON),
            tok(TOK_VOID),
            ident("f"),
            tok(TOK_LPAREN),
            tok(TOK_VOID),
            tok(TOK_RPAREN),
            tok(TOK_LBRACE),
            tok(TOK_STRUCT),
            ident("S"),
            tok(TOK_STAR),
            ident("a"),
            tok(TOK_SEMICOLON),
            ident("S"),
            tok(TOK_STAR),
            ident("b"),
            tok(TOK_SEMICOLON),
            tok(TOK_RBRACE),
        ],
    );
    let ann = annotate_cpu(&fix);
    let typed = classify_cpu_annotated(&fix);

    // typedef S at token 11
    assert_ne!(
        flags_at(&ann, 11) & TYPEDEF_FLAG_DECL,
        0,
        "typedef S must be decl"
    );
    // struct S * a: star must be POINTER_DECL
    assert_eq!(
        kind_at(&typed, 21),
        C_AST_KIND_POINTER_DECL,
        "struct S * a must be pointer decl"
    );
    // S * b: star must be POINTER_DECL
    assert_eq!(
        kind_at(&typed, 25),
        C_AST_KIND_POINTER_DECL,
        "typedef S * b must be pointer decl"
    );
}

// ---------------------------------------------------------------------------
// GPU parity
// ---------------------------------------------------------------------------

#[test]
fn gpu_parity_scope_tree_struct_tag_vs_ordinary() {
    let fix = fixture(
        "gpu_struct_tag",
        &[
            tok(TOK_STRUCT),
            ident("S"),
            tok(TOK_LBRACE),
            tok(TOK_INT),
            ident("x"),
            tok(TOK_SEMICOLON),
            tok(TOK_RBRACE),
            tok(TOK_SEMICOLON),
            tok(TOK_INT),
            ident("S"),
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
        "GPU scope tree must match CPU for struct tag vs ordinary"
    );
}

#[test]
fn gpu_parity_scope_tree_label_namespace() {
    let fix = fixture(
        "gpu_label",
        &[
            tok(TOK_VOID),
            ident("f"),
            tok(TOK_LPAREN),
            tok(TOK_VOID),
            tok(TOK_RPAREN),
            tok(TOK_LBRACE),
            ident("L"),
            tok(TOK_COLON),
            tok(TOK_GOTO),
            ident("L"),
            tok(TOK_SEMICOLON),
            tok(TOK_RBRACE),
        ],
    );
    let expected = scope_tree_for(&fix);
    let gpu = run_gpu_scope_tree(&fix);
    assert_eq!(
        gpu, expected,
        "GPU scope tree must match CPU for label namespace"
    );
}

#[test]
fn gpu_parity_annotation_tag_typedef_same_name() {
    let fix = fixture(
        "gpu_tag_td",
        &[
            tok(TOK_STRUCT),
            ident("S"),
            tok(TOK_LBRACE),
            tok(TOK_INT),
            ident("x"),
            tok(TOK_SEMICOLON),
            tok(TOK_RBRACE),
            tok(TOK_SEMICOLON),
            tok(TOK_TYPEDEF),
            tok(TOK_STRUCT),
            ident("S"),
            ident("S"),
            tok(TOK_SEMICOLON),
            tok(TOK_VOID),
            ident("f"),
            tok(TOK_LPAREN),
            tok(TOK_VOID),
            tok(TOK_RPAREN),
            tok(TOK_LBRACE),
            tok(TOK_STRUCT),
            ident("S"),
            tok(TOK_STAR),
            ident("a"),
            tok(TOK_SEMICOLON),
            tok(TOK_RBRACE),
        ],
    );
    let expected_ann = annotate_cpu(&fix);
    let gpu_ann = run_gpu_annotate(&fix);
    assert_eq!(
        gpu_ann, expected_ann,
        "GPU annotation must match CPU for tag+typedef same name"
    );
}

#[test]
fn gpu_parity_classifier_enum_constant_context() {
    let fix = fixture(
        "gpu_enum",
        &[
            tok(TOK_ENUM),
            ident("E"),
            tok(TOK_LBRACE),
            ident("A"),
            tok(TOK_COMMA),
            ident("B"),
            tok(TOK_RBRACE),
            tok(TOK_SEMICOLON),
            tok(TOK_VOID),
            ident("f"),
            tok(TOK_LPAREN),
            tok(TOK_VOID),
            tok(TOK_RPAREN),
            tok(TOK_LBRACE),
            tok(TOK_RETURN),
            ident("A"),
            tok(TOK_SEMICOLON),
            tok(TOK_RBRACE),
        ],
    );
    let expected_ann = annotate_cpu(&fix);
    let gpu_ann = run_gpu_annotate(&fix);
    assert_eq!(
        gpu_ann, expected_ann,
        "GPU annotation must match CPU for enum context"
    );

    let expected_typed = reference_c11_classify_vast_node_kinds(&expected_ann);
    let gpu_typed = run_gpu_classify(&gpu_ann, fix.tok_types.len());
    assert_eq!(
        gpu_typed, expected_typed,
        "GPU classifier must match CPU for enum context"
    );
}

#[test]
fn gpu_parity_scope_tree_enum_tag_vs_variable() {
    let fix = fixture(
        "gpu_enum_var",
        &[
            tok(TOK_ENUM),
            ident("E"),
            tok(TOK_LBRACE),
            ident("A"),
            tok(TOK_RBRACE),
            tok(TOK_SEMICOLON),
            tok(TOK_INT),
            ident("E"),
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
        "GPU scope tree must match CPU for enum tag vs variable"
    );
}

#[test]
fn gpu_parity_annotation_label_does_not_shadow_typedef() {
    let fix = fixture(
        "gpu_label_td",
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
            ident("T"),
            tok(TOK_COLON),
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
        "GPU annotation must match CPU for label+typedef"
    );
}
