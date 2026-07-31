use super::*;

#[test]
fn scope_tree_kr_function_definition_classified_correctly() {
    let fix = fixture(
        "kr_def",
        &[
            tok(TOK_INT),
            ident("f"),
            tok(TOK_LPAREN),
            ident("a"),
            tok(TOK_RPAREN),
            tok(TOK_INT),
            ident("a"),
            tok(TOK_SEMICOLON),
            tok(TOK_LBRACE),
            tok(TOK_RETURN),
            ident("a"),
            tok(TOK_SEMICOLON),
            tok(TOK_RBRACE),
        ],
    );
    let st = scope_tree_for(&fix);

    // f at token 1 is a function definition because next is ( and then eventually {
    assert_eq!(
        scope_tree_word_at(&st, 1, 2),
        { DECL_KIND_FUNCTION },
        "f must be FUNCTION definition"
    );
}

// ---------------------------------------------------------------------------
// Parameter typedef restoration across functions
// ---------------------------------------------------------------------------

#[test]
fn annotation_typedef_restored_after_function_definition() {
    let fix = fixture(
        "td_restore_after_def",
        &[
            tok(TOK_TYPEDEF),
            tok(TOK_INT),
            ident("T"),
            tok(TOK_SEMICOLON),
            tok(TOK_VOID),
            ident("f"),
            tok(TOK_LPAREN),
            tok(TOK_INT),
            ident("T"),
            tok(TOK_RPAREN),
            tok(TOK_LBRACE),
            tok(TOK_RBRACE),
            tok(TOK_VOID),
            ident("g"),
            tok(TOK_LPAREN),
            ident("T"),
            tok(TOK_STAR),
            ident("p"),
            tok(TOK_RPAREN),
            tok(TOK_LBRACE),
            tok(TOK_RBRACE),
        ],
    );
    let ann = annotate_cpu(&fix);
    let typed = classify_cpu_annotated(&fix);

    // g parameter list: T is restored
    assert_ne!(
        flags_at(&ann, 15) & TYPEDEF_FLAG_VISIBLE,
        0,
        "typedef T must be visible in g params"
    );
    assert_eq!(
        kind_at(&typed, 16),
        C_AST_KIND_POINTER_DECL,
        "T * p in g must be pointer decl"
    );
}

#[test]
fn annotation_typedef_restored_after_function_prototype() {
    let fix = fixture(
        "td_restore_after_proto",
        &[
            tok(TOK_TYPEDEF),
            tok(TOK_INT),
            ident("T"),
            tok(TOK_SEMICOLON),
            tok(TOK_VOID),
            ident("f"),
            tok(TOK_LPAREN),
            tok(TOK_INT),
            ident("T"),
            tok(TOK_RPAREN),
            tok(TOK_SEMICOLON),
            tok(TOK_VOID),
            ident("g"),
            tok(TOK_LPAREN),
            ident("T"),
            tok(TOK_STAR),
            ident("p"),
            tok(TOK_RPAREN),
            tok(TOK_LBRACE),
            tok(TOK_RBRACE),
        ],
    );
    let ann = annotate_cpu(&fix);
    let typed = classify_cpu_annotated(&fix);

    // g parameter list: T restored
    assert_ne!(
        flags_at(&ann, 14) & TYPEDEF_FLAG_VISIBLE,
        0,
        "typedef T must be visible after prototype"
    );
    assert_eq!(
        kind_at(&typed, 15),
        C_AST_KIND_POINTER_DECL,
        "T * p after prototype must be pointer decl"
    );
}

// ---------------------------------------------------------------------------
// Edge cases: parameter lists with typedef names as types
// ---------------------------------------------------------------------------

#[test]
fn annotation_typedef_used_as_parameter_type_is_visible() {
    let fix = fixture(
        "td_as_param_type",
        &[
            tok(TOK_TYPEDEF),
            tok(TOK_INT),
            ident("T"),
            tok(TOK_SEMICOLON),
            tok(TOK_VOID),
            ident("f"),
            tok(TOK_LPAREN),
            ident("T"),
            tok(TOK_STAR),
            ident("p"),
            tok(TOK_COMMA),
            ident("T"),
            ident("x"),
            tok(TOK_RPAREN),
            tok(TOK_LBRACE),
            tok(TOK_RBRACE),
        ],
    );
    let ann = annotate_cpu(&fix);
    let typed = classify_cpu_annotated(&fix);

    // T in parameter list at token 7
    assert_ne!(
        flags_at(&ann, 7) & TYPEDEF_FLAG_VISIBLE,
        0,
        "typedef T must be visible as param type"
    );
    assert_eq!(
        kind_at(&typed, 8),
        C_AST_KIND_POINTER_DECL,
        "T * p must be pointer decl"
    );
}

// ---------------------------------------------------------------------------
// GPU parity
// ---------------------------------------------------------------------------

#[test]
fn gpu_parity_scope_tree_parameter_scope() {
    let fix = fixture(
        "gpu_param_scope",
        &[
            tok(TOK_VOID),
            ident("f"),
            tok(TOK_LPAREN),
            tok(TOK_INT),
            ident("x"),
            tok(TOK_COMMA),
            tok(TOK_INT),
            ident("y"),
            tok(TOK_RPAREN),
            tok(TOK_LBRACE),
            tok(TOK_RBRACE),
        ],
    );
    let expected = scope_tree_for(&fix);
    let gpu = run_gpu_scope_tree(&fix);
    assert_eq!(
        gpu, expected,
        "GPU scope tree must match CPU for parameter scope"
    );
}

#[test]
fn gpu_parity_scope_tree_kr_parameters() {
    let fix = fixture(
        "gpu_kr",
        &[
            tok(TOK_INT),
            ident("f"),
            tok(TOK_LPAREN),
            ident("a"),
            tok(TOK_RPAREN),
            tok(TOK_INT),
            ident("a"),
            tok(TOK_SEMICOLON),
            tok(TOK_LBRACE),
            tok(TOK_RBRACE),
        ],
    );
    let expected = scope_tree_for(&fix);
    let gpu = run_gpu_scope_tree(&fix);
    assert_eq!(
        gpu, expected,
        "GPU scope tree must match CPU for K&R params"
    );
}

#[test]
fn gpu_parity_annotation_parameter_shadows_typedef() {
    let fix = fixture(
        "gpu_ann_param",
        &[
            tok(TOK_TYPEDEF),
            tok(TOK_INT),
            ident("T"),
            tok(TOK_SEMICOLON),
            tok(TOK_VOID),
            ident("f"),
            tok(TOK_LPAREN),
            tok(TOK_INT),
            ident("T"),
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
        "GPU annotation must match CPU for param shadow"
    );
}

#[test]
fn gpu_parity_classifier_parameter_shadows_typedef() {
    let fix = fixture(
        "gpu_cls_param",
        &[
            tok(TOK_TYPEDEF),
            tok(TOK_INT),
            ident("T"),
            tok(TOK_SEMICOLON),
            tok(TOK_VOID),
            ident("f"),
            tok(TOK_LPAREN),
            tok(TOK_INT),
            ident("T"),
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
    assert_eq!(gpu_ann, expected_ann);

    let expected_typed = reference_c11_classify_vast_node_kinds(&expected_ann);
    let gpu_typed = run_gpu_classify(&gpu_ann, fix.tok_types.len());
    assert_eq!(
        gpu_typed, expected_typed,
        "GPU classifier must match CPU for param shadow"
    );
}

#[test]
fn gpu_parity_scope_tree_prototype_vs_definition() {
    let fix = fixture(
        "gpu_proto_def",
        &[
            tok(TOK_VOID),
            ident("f"),
            tok(TOK_LPAREN),
            tok(TOK_INT),
            ident("x"),
            tok(TOK_RPAREN),
            tok(TOK_SEMICOLON),
            tok(TOK_VOID),
            ident("f"),
            tok(TOK_LPAREN),
            tok(TOK_INT),
            ident("y"),
            tok(TOK_RPAREN),
            tok(TOK_LBRACE),
            tok(TOK_RBRACE),
        ],
    );
    let expected = scope_tree_for(&fix);
    let gpu = run_gpu_scope_tree(&fix);
    assert_eq!(
        gpu, expected,
        "GPU scope tree must match CPU for prototype vs definition"
    );
}
