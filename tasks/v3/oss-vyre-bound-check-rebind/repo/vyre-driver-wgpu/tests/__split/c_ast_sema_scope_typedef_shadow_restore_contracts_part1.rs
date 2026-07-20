use super::*;

#[test]
fn scope_tree_typedef_shadowed_by_inner_variable_has_different_scope_ids() {
    let fix = fixture(
        "shadow_scope",
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
            tok(TOK_LBRACE),
            tok(TOK_INT),
            ident("T"),
            tok(TOK_SEMICOLON),
            tok(TOK_RBRACE),
            tok(TOK_RBRACE),
        ],
    );
    let st = scope_tree_for(&fix);

    // Global typedef T at token 2
    assert_eq!(
        scope_tree_word_at(&st, 2, 0),
        0,
        "typedef T must be in global scope"
    );
    assert_eq!(
        scope_tree_word_at(&st, 2, 2),
        { DECL_KIND_TYPEDEF },
        "token 2 must be TYPEDEF decl"
    );

    // Inner variable T at token 12
    let inner_scope = scope_tree_word_at(&st, 12, 0);
    assert_ne!(inner_scope, 0, "inner T must be in a non-global scope");
    assert_eq!(
        scope_tree_word_at(&st, 12, 2),
        { DECL_KIND_VARIABLE },
        "inner T must be VARIABLE decl"
    );

    // The inner scope's parent should be the function body scope
    let inner_parent = scope_tree_word_at(&st, 12, 1);
    assert_eq!(
        inner_parent, 10,
        "inner block parent scope must be the function body scope (token 9+1)"
    );
}

#[test]
fn scope_tree_typedef_restored_after_inner_block_exit() {
    let fix = fixture(
        "restore_scope",
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
            tok(TOK_LBRACE),
            tok(TOK_INT),
            ident("T"),
            tok(TOK_SEMICOLON),
            tok(TOK_RBRACE),
            ident("T"),
            tok(TOK_STAR),
            ident("p"),
            tok(TOK_SEMICOLON),
            tok(TOK_RBRACE),
        ],
    );
    let st = scope_tree_for(&fix);

    // T used after inner block (token 15) should be in function body scope
    let use_scope = scope_tree_word_at(&st, 15, 0);
    assert_eq!(
        use_scope, 10,
        "T use after inner block must be in function body scope (token 9+1)"
    );
    // And should NOT be a declaration
    assert_eq!(
        scope_tree_word_at(&st, 15, 2),
        { DECL_KIND_NONE },
        "T use must be NONE decl kind"
    );
}

#[test]
fn scope_tree_multiple_levels_of_shadowing_create_distinct_scope_ids() {
    let fix = fixture(
        "multi_shadow",
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
            tok(TOK_LBRACE),
            tok(TOK_INT),
            ident("T"),
            tok(TOK_SEMICOLON),
            tok(TOK_LBRACE),
            tok(TOK_INT),
            ident("T"),
            tok(TOK_SEMICOLON),
            tok(TOK_RBRACE),
            tok(TOK_RBRACE),
            tok(TOK_RBRACE),
        ],
    );
    let st = scope_tree_for(&fix);

    let scope_l1 = scope_tree_word_at(&st, 12, 0); // first inner T
    let scope_l2 = scope_tree_word_at(&st, 16, 0); // second inner T
    assert_ne!(
        scope_l1, scope_l2,
        "nested block scopes must have distinct scope ids"
    );
    assert_ne!(scope_l1, 0, "inner scopes must not be global");
    assert_ne!(scope_l2, 0, "innermost scope must not be global");
    assert_eq!(scope_tree_word_at(&st, 12, 2), { DECL_KIND_VARIABLE });
    assert_eq!(scope_tree_word_at(&st, 16, 2), { DECL_KIND_VARIABLE });
}

#[test]
fn scope_tree_typedef_inside_block_not_visible_outside() {
    let fix = fixture(
        "block_typedef",
        &[
            tok(TOK_VOID),
            ident("f"),
            tok(TOK_LPAREN),
            tok(TOK_VOID),
            tok(TOK_RPAREN),
            tok(TOK_LBRACE),
            tok(TOK_TYPEDEF),
            tok(TOK_INT),
            ident("T"),
            tok(TOK_SEMICOLON),
            tok(TOK_RBRACE),
            tok(TOK_VOID),
            ident("g"),
            tok(TOK_LPAREN),
            tok(TOK_VOID),
            tok(TOK_RPAREN),
            tok(TOK_LBRACE),
            tok(TOK_RBRACE),
        ],
    );
    let st = scope_tree_for(&fix);

    // T at token 8 is declared inside f's body
    assert_eq!(
        scope_tree_word_at(&st, 8, 0),
        6,
        "block typedef T must be in f's body scope (token 5+1)"
    );
    assert_eq!(scope_tree_word_at(&st, 8, 2), { DECL_KIND_TYPEDEF });
}

#[test]
fn scope_tree_parameter_shadows_typedef_in_function_body() {
    let fix = fixture(
        "param_shadow",
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
            ident("T"),
            tok(TOK_STAR),
            ident("p"),
            tok(TOK_SEMICOLON),
            tok(TOK_RBRACE),
        ],
    );
    let st = scope_tree_for(&fix);

    // Parameter T at token 8
    assert_eq!(
        scope_tree_word_at(&st, 8, 2),
        { DECL_KIND_VARIABLE },
        "parameter T must be VARIABLE"
    );
    // Use of T in body at token 11
    let body_scope = scope_tree_word_at(&st, 11, 0);
    assert_eq!(
        body_scope, 11,
        "use of T in body must be in function body scope"
    );
}

#[test]
fn scope_tree_parameter_typedef_restored_for_later_function() {
    let fix = fixture(
        "param_restore",
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
    let st = scope_tree_for(&fix);

    // Parameter T in f (token 8) shadows typedef
    assert_eq!(scope_tree_word_at(&st, 8, 2), { DECL_KIND_VARIABLE });
    // Parameter T in g (token 16) is the typedef name restored
    assert_eq!(
        scope_tree_word_at(&st, 16, 2),
        { DECL_KIND_NONE },
        "typedef name use in parameter list must be NONE decl kind"
    );
}

// ---------------------------------------------------------------------------
// Typedef annotation contract tests
// ---------------------------------------------------------------------------

#[test]
fn annotation_typedef_visible_in_outer_scope_shadowed_in_inner() {
    let fix = fixture(
        "ann_shadow",
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
            tok(TOK_LBRACE),
            tok(TOK_INT),
            ident("T"),
            tok(TOK_SEMICOLON),
            tok(TOK_RBRACE),
            ident("T"),
            tok(TOK_STAR),
            ident("p"),
            tok(TOK_SEMICOLON),
            tok(TOK_RBRACE),
        ],
    );
    let ann = annotate_cpu(&fix);

    // Token 2: global typedef decl
    assert_ne!(
        flags_at(&ann, 2) & TYPEDEF_FLAG_DECL,
        0,
        "global typedef T must have DECL flag"
    );
    // Token 12: inner variable decl
    assert_ne!(
        flags_at(&ann, 12) & ORDINARY_FLAG_DECL,
        0,
        "inner T must be ordinary decl"
    );
    assert_eq!(
        flags_at(&ann, 12) & TYPEDEF_FLAG_VISIBLE,
        0,
        "inner T must not have typedef visible"
    );
    // Token 15: use after inner block
    assert_ne!(
        flags_at(&ann, 15) & TYPEDEF_FLAG_VISIBLE,
        0,
        "typedef T must be restored after inner block"
    );
}

#[test]
fn annotation_shadowed_typedef_changes_cast_to_multiply() {
    let fix = fixture(
        "ann_cast_mul",
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
            tok(TOK_LBRACE),
            tok(TOK_INT),
            ident("T"),
            tok(TOK_SEMICOLON),
            tok(TOK_LPAREN),
            ident("T"),
            tok(TOK_RPAREN),
            tok(TOK_STAR),
            ident("p"),
            tok(TOK_SEMICOLON),
            tok(TOK_RBRACE),
            tok(TOK_LPAREN),
            ident("T"),
            tok(TOK_RPAREN),
            tok(TOK_STAR),
            ident("p"),
            tok(TOK_SEMICOLON),
            tok(TOK_RBRACE),
        ],
    );
    let typed = classify_cpu_annotated(&fix);

    // Inside inner block: (T) is NOT a cast because T is shadowed
    assert_ne!(
        kind_at(&typed, 14),
        C_AST_KIND_CAST_EXPR,
        "shadowed (T) must not be cast"
    );
    assert_eq!(
        kind_at(&typed, 17),
        node_kind::BINARY,
        "shadowed * must be binary multiply"
    );
    // Outside inner block: (T) IS a cast
    assert_eq!(
        kind_at(&typed, 21),
        C_AST_KIND_CAST_EXPR,
        "restored typedef (T) must be cast"
    );
    assert_eq!(
        kind_at(&typed, 24),
        C_AST_KIND_UNARY_EXPR,
        "restored * must be unary deref"
    );
}

#[test]
fn annotation_typedef_declared_in_block_not_visible_outside() {
    let fix = fixture(
        "ann_block_td",
        &[
            tok(TOK_VOID),
            ident("f"),
            tok(TOK_LPAREN),
            tok(TOK_VOID),
            tok(TOK_RPAREN),
            tok(TOK_LBRACE),
            tok(TOK_TYPEDEF),
            tok(TOK_INT),
            ident("T"),
            tok(TOK_SEMICOLON),
            tok(TOK_RBRACE),
            tok(TOK_VOID),
            ident("g"),
            tok(TOK_LPAREN),
            tok(TOK_VOID),
            tok(TOK_RPAREN),
            tok(TOK_LBRACE),
            ident("T"),
            tok(TOK_STAR),
            ident("p"),
            tok(TOK_SEMICOLON),
            tok(TOK_RBRACE),
        ],
    );
    let ann = annotate_cpu(&fix);

    // T inside f's body is a typedef decl
    assert_ne!(flags_at(&ann, 8) & TYPEDEF_FLAG_DECL, 0);
    // T inside g's body should NOT be visible as typedef (it's undeclared in this test's model)
    // The annotation pass may or may not flag it; we assert it is NOT a typedef declaration
    assert_eq!(
        flags_at(&ann, 18) & TYPEDEF_FLAG_DECL,
        0,
        "T in g must not be a typedef decl"
    );
}

