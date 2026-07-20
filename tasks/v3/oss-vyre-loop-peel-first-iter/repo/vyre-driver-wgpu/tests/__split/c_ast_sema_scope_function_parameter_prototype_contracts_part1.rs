use super::*;

#[test]
fn scope_tree_function_parameter_is_variable_decl() {
    let fix = fixture(
        "param_decl",
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
    let st = scope_tree_for(&fix);

    assert_eq!(
        scope_tree_word_at(&st, 4, 2),
        { DECL_KIND_VARIABLE },
        "parameter x must be VARIABLE"
    );
    assert_eq!(
        scope_tree_word_at(&st, 7, 2),
        { DECL_KIND_VARIABLE },
        "parameter y must be VARIABLE"
    );
}

#[test]
fn scope_tree_parameter_scope_is_function_body_scope() {
    let fix = fixture(
        "param_scope",
        &[
            tok(TOK_VOID),
            ident("f"),
            tok(TOK_LPAREN),
            tok(TOK_INT),
            ident("x"),
            tok(TOK_RPAREN),
            tok(TOK_LBRACE),
            ident("x"),
            tok(TOK_ASSIGN),
            tok(TOK_INTEGER),
            tok(TOK_SEMICOLON),
            tok(TOK_RBRACE),
        ],
    );
    let st = scope_tree_for(&fix);

    let param_scope = scope_tree_word_at(&st, 4, 0);
    let use_scope = scope_tree_word_at(&st, 7, 0);
    assert_eq!(
        param_scope, use_scope,
        "parameter and its use must share the same scope id"
    );
}

#[test]
fn scope_tree_parameter_shadows_outer_variable() {
    let fix = fixture(
        "param_shadow_outer",
        &[
            tok(TOK_INT),
            ident("x"),
            tok(TOK_SEMICOLON),
            tok(TOK_VOID),
            ident("f"),
            tok(TOK_LPAREN),
            tok(TOK_INT),
            ident("x"),
            tok(TOK_RPAREN),
            tok(TOK_LBRACE),
            tok(TOK_RBRACE),
        ],
    );
    let st = scope_tree_for(&fix);

    let outer_scope = scope_tree_word_at(&st, 1, 0);
    let param_scope = scope_tree_word_at(&st, 7, 0);
    assert_ne!(
        outer_scope, param_scope,
        "parameter scope must differ from outer variable scope"
    );
    assert_eq!(
        scope_tree_word_at(&st, 1, 2),
        { DECL_KIND_VARIABLE },
        "outer x must be VARIABLE"
    );
    assert_eq!(
        scope_tree_word_at(&st, 7, 2),
        { DECL_KIND_VARIABLE },
        "parameter x must be VARIABLE"
    );
}

#[test]
fn annotation_parameter_shadows_typedef_in_body() {
    let fix = fixture(
        "ann_param_td",
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
    let ann = annotate_cpu(&fix);
    let typed = classify_cpu_annotated(&fix);

    // Parameter T at token 8
    assert_ne!(
        flags_at(&ann, 8) & ORDINARY_FLAG_DECL,
        0,
        "parameter T must be ordinary decl"
    );
    // (T) in body is shadowed
    assert_eq!(
        flags_at(&ann, 12) & TYPEDEF_FLAG_VISIBLE,
        0,
        "typedef must be shadowed in body"
    );
    assert_ne!(
        kind_at(&typed, 11),
        C_AST_KIND_CAST_EXPR,
        "shadowed (T) must not be cast"
    );
}

#[test]
fn annotation_multiple_parameters_shadow_typedef_sequentially() {
    let fix = fixture(
        "ann_multi_param",
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
            tok(TOK_COMMA),
            tok(TOK_INT),
            ident("U"),
            tok(TOK_RPAREN),
            tok(TOK_LBRACE),
            ident("T"),
            tok(TOK_STAR),
            ident("a"),
            tok(TOK_SEMICOLON),
            ident("U"),
            tok(TOK_STAR),
            ident("b"),
            tok(TOK_SEMICOLON),
            tok(TOK_RBRACE),
        ],
    );
    let ann = annotate_cpu(&fix);
    let typed = classify_cpu_annotated(&fix);

    // T shadowed
    assert_eq!(
        flags_at(&ann, 14) & TYPEDEF_FLAG_VISIBLE,
        0,
        "T must be shadowed by parameter"
    );
    assert_eq!(
        kind_at(&typed, 15),
        node_kind::BINARY,
        "T * a must be multiply"
    );
    // U not a typedef, so ordinary
    assert_eq!(
        flags_at(&ann, 18) & TYPEDEF_FLAG_VISIBLE,
        0,
        "U must not be visible typedef"
    );
}

// ---------------------------------------------------------------------------
// Prototype scope
// ---------------------------------------------------------------------------

#[test]
fn scope_tree_prototype_parameter_not_visible_in_body() {
    // Prototype then definition: the prototype params should not leak
    let fix = fixture(
        "proto_scope",
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
            ident("y"),
            tok(TOK_ASSIGN),
            tok(TOK_INTEGER),
            tok(TOK_SEMICOLON),
            tok(TOK_RBRACE),
        ],
    );
    let st = scope_tree_for(&fix);

    // Prototype parameter x at token 4
    let proto_scope = scope_tree_word_at(&st, 4, 0);
    // Definition parameter y at token 11
    let def_param_scope = scope_tree_word_at(&st, 11, 0);
    // Use of y in body at token 14
    let use_scope = scope_tree_word_at(&st, 14, 0);
    assert_eq!(
        def_param_scope, use_scope,
        "definition parameter and body use must share scope"
    );
    assert_ne!(
        proto_scope, use_scope,
        "prototype scope must not leak into body"
    );
}

#[test]
fn annotation_prototype_does_not_restore_typedef_for_body() {
    let fix = fixture(
        "ann_proto",
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
    let ann = annotate_cpu(&fix);
    let typed = classify_cpu_annotated(&fix);

    // Body use of T at token 19
    assert_eq!(
        flags_at(&ann, 19) & TYPEDEF_FLAG_VISIBLE,
        0,
        "typedef T must be shadowed by definition parameter in body"
    );
    assert_ne!(
        kind_at(&typed, 18),
        C_AST_KIND_CAST_EXPR,
        "(T) in body must not be cast"
    );
}

// ---------------------------------------------------------------------------
// K&R style parameters
// ---------------------------------------------------------------------------

#[test]
fn scope_tree_kr_parameter_is_variable_in_body_scope() {
    let fix = fixture(
        "kr_param",
        &[
            tok(TOK_VOID),
            ident("f"),
            tok(TOK_LPAREN),
            ident("x"),
            tok(TOK_COMMA),
            ident("y"),
            tok(TOK_RPAREN),
            tok(TOK_INT),
            ident("x"),
            tok(TOK_SEMICOLON),
            tok(TOK_INT),
            ident("y"),
            tok(TOK_SEMICOLON),
            tok(TOK_LBRACE),
            ident("x"),
            tok(TOK_ASSIGN),
            ident("y"),
            tok(TOK_SEMICOLON),
            tok(TOK_RBRACE),
        ],
    );
    let st = scope_tree_for(&fix);

    // K&R declarations at token 8 and 11
    assert_eq!(
        scope_tree_word_at(&st, 8, 2),
        { DECL_KIND_VARIABLE },
        "K&R param x must be VARIABLE"
    );
    assert_eq!(
        scope_tree_word_at(&st, 11, 2),
        { DECL_KIND_VARIABLE },
        "K&R param y must be VARIABLE"
    );
    // Uses in body
    assert_eq!(
        scope_tree_word_at(&st, 14, 2),
        { DECL_KIND_NONE },
        "use of x must be NONE"
    );
    assert_eq!(
        scope_tree_word_at(&st, 16, 2),
        { DECL_KIND_NONE },
        "use of y must be NONE"
    );
}

#[test]
fn annotation_kr_parameter_shadows_typedef() {
    let fix = fixture(
        "ann_kr_td",
        &[
            tok(TOK_TYPEDEF),
            tok(TOK_INT),
            ident("T"),
            tok(TOK_SEMICOLON),
            tok(TOK_VOID),
            ident("f"),
            tok(TOK_LPAREN),
            ident("T"),
            tok(TOK_RPAREN),
            tok(TOK_INT),
            ident("T"),
            tok(TOK_SEMICOLON),
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

    // K&R declaration T at token 10
    assert_ne!(
        flags_at(&ann, 10) & ORDINARY_FLAG_DECL,
        0,
        "K&R param T must be ordinary decl"
    );
    // (T) in body
    assert_eq!(
        flags_at(&ann, 13) & TYPEDEF_FLAG_VISIBLE,
        0,
        "typedef must be shadowed by K&R param"
    );
    assert_ne!(
        kind_at(&typed, 12),
        C_AST_KIND_CAST_EXPR,
        "K&R shadowed (T) must not be cast"
    );
}

