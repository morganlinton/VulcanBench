// (use super::* removed  -  parts now share crate-root scope via flat include)

#[test]
fn atomic_qualifier_does_not_misclassify() {
    let fix = fixture_atomic_qualifier();
    assert_eq!(fix.tok_types[5], TOK_ATOMIC);
    assert_full_pipeline_parity(&fix, "atomic_qualifier");

    let typed = classify(&fix);
    assert!(
        row_indices(&typed, node_kind::CALL).is_empty(),
        "_Atomic must not be confused with a function call"
    );
    assert!(
        row_indices(&typed, node_kind::BINARY).is_empty(),
        "_Atomic must not be confused with a binary operator"
    );
}

#[test]
fn atomic_type_specifier_does_not_misclassify() {
    let fix = fixture_atomic_type_specifier();
    assert_full_pipeline_parity(&fix, "atomic_type_specifier");

    let typed = classify(&fix);
    assert!(
        row_indices(&typed, node_kind::CALL).is_empty(),
        "_Atomic(type) must not be confused with a function call"
    );
    assert!(
        row_indices(&typed, C_AST_KIND_CAST_EXPR).is_empty(),
        "paren after _Atomic must not be classified as CAST_EXPR"
    );
}

// ---------------------------------------------------------------------------
// 6. typeof_unqual
// ---------------------------------------------------------------------------

fn fixture_typeof_unqual() -> Fixture {
    build_fixture(&[
        FixtureToken::new("__typeof_unqual__", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("int", TOK_INT),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("z", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

#[test]
fn typeof_unqual_promotes_and_declares_variable() {
    let fix = fixture_typeof_unqual();
    assert_eq!(
        fix.tok_types[0], TOK_GNU_TYPEOF_UNQUAL,
        "__typeof_unqual__ must promote to TOK_GNU_TYPEOF_UNQUAL"
    );
    assert_full_pipeline_parity(&fix, "typeof_unqual");

    let typed = classify(&fix);
    assert!(
        row_indices(&typed, node_kind::VARIABLE).contains(&4),
        "z must classify as VARIABLE"
    );
    assert!(
        row_indices(&typed, node_kind::CALL).is_empty(),
        "typeof_unqual must not be confused with CALL"
    );
}

// ---------------------------------------------------------------------------
// 7. __auto_type
// ---------------------------------------------------------------------------

fn fixture_auto_type() -> Fixture {
    build_fixture(&[
        FixtureToken::new("__auto_type", TOK_IDENTIFIER),
        FixtureToken::new("x", TOK_IDENTIFIER),
        FixtureToken::new("=", TOK_ASSIGN),
        FixtureToken::new("42", TOK_INTEGER),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

#[test]
fn auto_type_promotes_and_declares_variable() {
    let fix = fixture_auto_type();
    assert_eq!(
        fix.tok_types[0], TOK_GNU_AUTO_TYPE,
        "__auto_type must promote to TOK_GNU_AUTO_TYPE"
    );
    assert_full_pipeline_parity(&fix, "auto_type");

    let typed = classify(&fix);
    assert!(
        row_indices(&typed, node_kind::VARIABLE).contains(&1),
        "x must classify as VARIABLE"
    );
}

// ---------------------------------------------------------------------------
// 8. __int128
// ---------------------------------------------------------------------------

fn fixture_int128() -> Fixture {
    build_fixture(&[
        FixtureToken::new("unsigned", TOK_UNSIGNED),
        FixtureToken::new("__int128", TOK_IDENTIFIER),
        FixtureToken::new("wide", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

#[test]
fn int128_promotes_and_declares_variable() {
    let fix = fixture_int128();
    assert_eq!(
        fix.tok_types[1], TOK_GNU_INT128,
        "__int128 must promote to TOK_GNU_INT128"
    );
    assert_full_pipeline_parity(&fix, "int128");

    let typed = classify(&fix);
    assert!(
        row_indices(&typed, node_kind::VARIABLE).contains(&2),
        "wide must classify as VARIABLE"
    );
}

// ---------------------------------------------------------------------------
// 9. Declarator ambiguity
// ---------------------------------------------------------------------------

fn fixture_declarator_pointer_vs_array() -> Fixture {
    build_fixture(&[
        FixtureToken::new("int", TOK_INT),
        FixtureToken::new("*", TOK_STAR),
        FixtureToken::new("a", TOK_IDENTIFIER),
        FixtureToken::new("[", TOK_LBRACKET),
        FixtureToken::new("4", TOK_INTEGER),
        FixtureToken::new("]", TOK_RBRACKET),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

fn fixture_declarator_array_vs_pointer() -> Fixture {
    build_fixture(&[
        FixtureToken::new("int", TOK_INT),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("*", TOK_STAR),
        FixtureToken::new("b", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("[", TOK_LBRACKET),
        FixtureToken::new("4", TOK_INTEGER),
        FixtureToken::new("]", TOK_RBRACKET),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

#[test]
fn declarator_pointer_array_precedence() {
    let fix = fixture_declarator_pointer_vs_array();
    assert_full_pipeline_parity(&fix, "declarator_pointer_vs_array");

    let typed = classify(&fix);
    let ptr_idx = row_indices(&typed, C_AST_KIND_POINTER_DECL)[0];
    let arr_idx = row_indices(&typed, C_AST_KIND_ARRAY_DECL)[0];
    let var_idx = row_indices(&typed, node_kind::VARIABLE)[0];
    // In `int *a[4];` there is no grouping paren, so POINTER_DECL is a direct
    // sibling of the variable and the array follows immediately.
    assert_eq!(
        parent_of(&typed, ptr_idx),
        u32::MAX,
        "POINTER_DECL in `int *a[4]` must not be inside a grouping paren"
    );
    assert_eq!(
        word_at(&typed, ptr_idx * VAST_STRIDE_U32 + 3),
        var_idx as u32,
        "POINTER_DECL next_sibling must be the variable"
    );
    assert_eq!(
        word_at(&typed, var_idx * VAST_STRIDE_U32 + 3),
        arr_idx as u32,
        "variable next_sibling must be the ARRAY_DECL"
    );
}

#[test]
fn declarator_array_pointer_precedence() {
    let fix = fixture_declarator_array_vs_pointer();
    assert_full_pipeline_parity(&fix, "declarator_array_vs_pointer");

    let typed = classify(&fix);
    let ptr_idx = row_indices(&typed, C_AST_KIND_POINTER_DECL)[0];
    let arr_idx = row_indices(&typed, C_AST_KIND_ARRAY_DECL)[0];
    let lparen_idx = ptr_idx - 1; // the `(` that groups the pointer
                                  // In `int (*b)[4];` the pointer is grouped inside parentheses, and that
                                  // group is a sibling preceding the array declarator.
    assert_eq!(
        parent_of(&typed, ptr_idx),
        lparen_idx as u32,
        "POINTER_DECL in `int (*b)[4]` must be parented by the grouping paren"
    );
    assert_eq!(
        word_at(&typed, lparen_idx * VAST_STRIDE_U32 + 2),
        ptr_idx as u32,
        "grouping paren first_child must be the POINTER_DECL"
    );
    assert_eq!(
        word_at(&typed, lparen_idx * VAST_STRIDE_U32 + 3),
        arr_idx as u32,
        "grouping paren next_sibling must be the ARRAY_DECL"
    );
}

// ---------------------------------------------------------------------------
// 10. C99 for declarations
// ---------------------------------------------------------------------------

fn fixture_c99_for_declaration() -> Fixture {
    build_fixture(&[
        FixtureToken::new("void", TOK_VOID),
        FixtureToken::new("f", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("int", TOK_INT),
        FixtureToken::new("n", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("for", TOK_FOR),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("int", TOK_INT),
        FixtureToken::new("i", TOK_IDENTIFIER),
        FixtureToken::new("=", TOK_ASSIGN),
        FixtureToken::new("0", TOK_INTEGER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("i", TOK_IDENTIFIER),
        FixtureToken::new("<", TOK_LT),
        FixtureToken::new("n", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("i", TOK_IDENTIFIER),
        FixtureToken::new("++", TOK_INC),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("}", TOK_RBRACE),
        FixtureToken::new("}", TOK_RBRACE),
    ])
}

#[test]
fn c99_for_declaration_classifies_correctly() {
    let fix = fixture_c99_for_declaration();
    assert_full_pipeline_parity(&fix, "c99_for_declaration");

    let typed = classify(&fix);
    assert_eq!(
        row_indices(&typed, C_AST_KIND_FOR_STMT),
        vec![7],
        "for must classify as FOR_STMT"
    );
    let assigns = row_indices(&typed, C_AST_KIND_ASSIGN_EXPR);
    assert!(
        assigns.is_empty(),
        "assignment inside C99 for-init must not be a top-level ASSIGN_EXPR"
    );
}

// ---------------------------------------------------------------------------
// 11. Abstract function pointers
// ---------------------------------------------------------------------------

fn fixture_abstract_function_pointer_param() -> Fixture {
    build_fixture(&[
        FixtureToken::new("void", TOK_VOID),
        FixtureToken::new("foo", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("void", TOK_VOID),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("*", TOK_STAR),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("int", TOK_INT),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

#[test]
fn abstract_function_pointer_param_classifies() {
    let fix = fixture_abstract_function_pointer_param();
    assert_full_pipeline_parity(&fix, "abstract_function_pointer_param");

    let typed = classify(&fix);
    let fn_decls = row_indices(&typed, C_AST_KIND_FUNCTION_DECLARATOR);
    assert_eq!(
        fn_decls.len(),
        2,
        "must contain two function declarators: outer param list and abstract function type"
    );
    assert!(
        fn_decls.contains(&2),
        "foo's parameter list must be FUNCTION_DECLARATOR"
    );
    assert!(
        fn_decls.contains(&7),
        "abstract function parameter list must be FUNCTION_DECLARATOR"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_POINTER_DECL),
        vec![5],
        "abstract pointer declarator must be at the `*` token"
    );
}

// ---------------------------------------------------------------------------
// 12. Linux-kernel-shaped declarations
// ---------------------------------------------------------------------------

fn fixture_kernel_function_with_attributes() -> Fixture {
    build_fixture(&[
        FixtureToken::new("static", TOK_STATIC),
        FixtureToken::new("__attribute__", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("section", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("\".init.text\"", TOK_STRING),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("int", TOK_INT),
        FixtureToken::new("foo", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("void", TOK_VOID),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("return", TOK_RETURN),
        FixtureToken::new("0", TOK_INTEGER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
    ])
}

fn fixture_kernel_typeof_fnptr_array() -> Fixture {
    build_fixture(&[
        FixtureToken::new("typeof", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("int", TOK_INT),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("*", TOK_STAR),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("*", TOK_STAR),
        FixtureToken::new("ops", TOK_IDENTIFIER),
        FixtureToken::new("[", TOK_LBRACKET),
        FixtureToken::new("4", TOK_INTEGER),
        FixtureToken::new("]", TOK_RBRACKET),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("void", TOK_VOID),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

#[test]
fn kernel_function_with_attributes_classifies() {
    let fix = fixture_kernel_function_with_attributes();
    assert_eq!(fix.tok_types[1], TOK_GNU_ATTRIBUTE);
    assert_full_pipeline_parity(&fix, "kernel_function_with_attributes");

    let typed = classify(&fix);
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ATTRIBUTE_SECTION),
        vec![4],
        "section attribute must classify distinctly"
    );
    assert!(
        row_indices(&typed, C_AST_KIND_FUNCTION_DEFINITION).contains(&11),
        "foo must be a FUNCTION_DEFINITION"
    );
}

#[test]
fn kernel_typeof_fnptr_array_classifies() {
    let fix = fixture_kernel_typeof_fnptr_array();
    assert_eq!(fix.tok_types[0], TOK_GNU_TYPEOF);
    assert_full_pipeline_parity(&fix, "kernel_typeof_fnptr_array");

    let typed = classify(&fix);
    let ptrs = row_indices(&typed, C_AST_KIND_POINTER_DECL);
    assert_eq!(ptrs.len(), 2, "must contain two pointer declarators");
    assert!(
        row_indices(&typed, C_AST_KIND_ARRAY_DECL).contains(&8),
        "ops[4] must classify as ARRAY_DECL"
    );
    assert!(
        !row_indices(&typed, C_AST_KIND_FUNCTION_DECLARATOR).is_empty(),
        "must contain a function declarator"
    );
    assert!(
        row_indices(&typed, node_kind::VARIABLE).contains(&7),
        "ops must classify as VARIABLE"
    );
}
