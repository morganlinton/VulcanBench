use super::*;

#[test]
fn gpu_parity_pg_lower_typedef_ambiguity() {
    let (tok_types, tok_starts, tok_lens) = fixture_typedef_expr_ambiguity();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let expected = reference_ast_to_pg_nodes(&typed);
    let gpu = run_gpu_pg_lower(&typed, tok_types.len() as u32);

    assert_eq!(
        gpu, expected,
        "GPU PG lowerer must match CPU for typedef ambiguity"
    );
}

#[test]
fn gpu_parity_pg_lower_nested_declarator() {
    let (tok_types, tok_starts, tok_lens) = fixture_deeply_nested_declarator();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let expected = reference_ast_to_pg_nodes(&typed);
    let gpu = run_gpu_pg_lower(&typed, tok_types.len() as u32);

    assert_eq!(
        gpu, expected,
        "GPU PG lowerer must match CPU for nested declarator"
    );
}

#[test]
fn gpu_parity_pg_lower_gnu_attribute_asm() {
    let (tok_types, tok_starts, tok_lens) = fixture_gnu_attribute_inline_asm();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let expected = reference_ast_to_pg_nodes(&typed);
    let gpu = run_gpu_pg_lower(&typed, tok_types.len() as u32);

    assert_eq!(
        gpu, expected,
        "GPU PG lowerer must match CPU for GNU attribute + inline asm"
    );
}

#[test]
fn gpu_parity_pg_lower_compound_literal() {
    let (tok_types, tok_starts, tok_lens) = fixture_compound_literal_stress();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let expected = reference_ast_to_pg_nodes(&typed);
    let gpu = run_gpu_pg_lower(&typed, tok_types.len() as u32);

    assert_eq!(
        gpu, expected,
        "GPU PG lowerer must match CPU for compound literal stress"
    );
}

#[test]
fn gpu_parity_pg_lower_nested_struct_enum() {
    let (tok_types, tok_starts, tok_lens) = fixture_nested_struct_enum();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let expected = reference_ast_to_pg_nodes(&typed);
    let gpu = run_gpu_pg_lower(&typed, tok_types.len() as u32);

    assert_eq!(
        gpu, expected,
        "GPU PG lowerer must match CPU for nested struct/enum"
    );
}

// ---------------------------------------------------------------------------
// Failure-oriented edge cases: malformed / boundary token streams
// ---------------------------------------------------------------------------

/// A parenthesised type-name followed immediately by a brace without an
/// intervening star or identifier: `(int){0}`.  The classifier must treat
/// the LPAREN as a compound literal, not a cast expression.
#[test]
fn cpu_reference_type_name_brace_is_compound_literal() {
    let tok_types = vec![
        TOK_VOID,
        TOK_IDENTIFIER,
        TOK_LPAREN,
        TOK_VOID,
        TOK_RPAREN,
        TOK_LBRACE,
        TOK_LPAREN, // (int)
        TOK_INT,
        TOK_RPAREN,
        TOK_LBRACE,
        TOK_INTEGER,
        TOK_RBRACE,
        TOK_SEMICOLON,
        TOK_RBRACE,
    ];
    let tok_lens = vec![1; tok_types.len()];
    let tok_starts = starts_for_lens(&tok_lens);
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    assert_eq!(
        word_at(&typed, 6 * VAST_STRIDE_U32),
        C_AST_KIND_COMPOUND_LITERAL_EXPR,
        "(int){{...}} must be a compound literal, never a cast"
    );
}

/// `int (*p)[3];`  -  the inner star is a pointer declarator because it sits
/// inside parentheses that themselves sit inside a declarator context.
#[test]
fn cpu_reference_parenthesised_pointer_declarator() {
    let tok_types = vec![
        TOK_INT,
        TOK_LPAREN,
        TOK_STAR,
        TOK_IDENTIFIER, // p
        TOK_RPAREN,
        TOK_LBRACKET,
        TOK_INTEGER, // 3
        TOK_RBRACKET,
        TOK_SEMICOLON,
    ];
    let tok_lens = vec![1; tok_types.len()];
    let tok_starts = starts_for_lens(&tok_lens);
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    assert_eq!(
        typed_indices(&typed, C_AST_KIND_POINTER_DECL),
        vec![2],
        "parenthesised star must still be a pointer declarator"
    );
    assert!(
        typed_indices(&typed, C_AST_KIND_ARRAY_DECL).contains(&5),
        "array suffix after parenthesised declarator must classify as array declarator"
    );
}

/// Empty struct `struct S {};` must not crash the classifier and the lone
/// brace must type as BASIC_BLOCK (struct body).
#[test]
fn cpu_reference_empty_struct_body() {
    let tok_types = vec![
        TOK_STRUCT,
        TOK_IDENTIFIER,
        TOK_LBRACE,
        TOK_RBRACE,
        TOK_SEMICOLON,
    ];
    let tok_lens = vec![1; tok_types.len()];
    let tok_starts = starts_for_lens(&tok_lens);
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    assert_eq!(
        word_at(&typed, 2 * VAST_STRIDE_U32),
        node_kind::BASIC_BLOCK,
        "empty struct body brace must type as BASIC_BLOCK"
    );
}

/// `__attribute__((aligned(16))) int x;`  -  attribute before declaration.
/// The identifier inside the attribute is seen as a CALL because it is
/// followed by LPAREN; this is the expected token-level behaviour.
#[test]
fn cpu_reference_gnu_attribute_before_declaration() {
    let tok_types = vec![
        TOK_GNU_ATTRIBUTE,
        TOK_LPAREN,
        TOK_LPAREN,
        TOK_IDENTIFIER, // aligned
        TOK_LPAREN,
        TOK_INTEGER, // 16
        TOK_RPAREN,
        TOK_RPAREN,
        TOK_RPAREN,
        TOK_INT,
        TOK_IDENTIFIER, // x
        TOK_SEMICOLON,
    ];
    let tok_lens = vec![1; tok_types.len()];
    let tok_starts = starts_for_lens(&tok_lens);
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    assert_eq!(
        word_at(&typed, 0 * VAST_STRIDE_U32),
        C_AST_KIND_GNU_ATTRIBUTE,
        "leading GNU attribute must be a first-class node"
    );
    assert_eq!(
        word_at(&typed, 3 * VAST_STRIDE_U32),
        node_kind::CALL,
        "attribute argument identifier(...) must type as CALL at token level"
    );
    assert_eq!(
        word_at(&typed, 10 * VAST_STRIDE_U32),
        node_kind::VARIABLE,
        "identifier after attribute must type as VARIABLE"
    );
}

/// `sizeof(int)`  -  the LPAREN after sizeof is a type operand, not a cast.
#[test]
fn cpu_reference_sizeof_paren_is_not_cast() {
    let tok_types = vec![
        TOK_INT,
        TOK_IDENTIFIER,
        TOK_ASSIGN,
        TOK_SIZEOF,
        TOK_LPAREN,
        TOK_INT,
        TOK_RPAREN,
        TOK_SEMICOLON,
    ];
    let tok_lens = vec![1; tok_types.len()];
    let tok_starts = starts_for_lens(&tok_lens);
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    assert_eq!(
        word_at(&typed, 3 * VAST_STRIDE_U32),
        C_AST_KIND_SIZEOF_EXPR,
        "sizeof token must be SIZEOF_EXPR"
    );
    assert_eq!(
        word_at(&typed, 4 * VAST_STRIDE_U32),
        0,
        "paren containing sizeof type operand must not be classified as a cast"
    );
}

// ---------------------------------------------------------------------------
// GPU parity for the edge-case fixtures above
// ---------------------------------------------------------------------------

#[test]
fn gpu_parity_parenthesised_pointer_declarator() {
    let tok_types = vec![
        TOK_INT,
        TOK_LPAREN,
        TOK_STAR,
        TOK_IDENTIFIER,
        TOK_RPAREN,
        TOK_LBRACKET,
        TOK_INTEGER,
        TOK_RBRACKET,
        TOK_SEMICOLON,
    ];
    let tok_lens = vec![1; tok_types.len()];
    let tok_starts = starts_for_lens(&tok_lens);
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let expected = reference_c11_classify_vast_node_kinds(&raw);
    let gpu = run_gpu_classifier(&raw, tok_types.len() as u32);

    assert_eq!(
        gpu, expected,
        "GPU classifier must match CPU for parenthesised pointer declarator"
    );
    assert_eq!(typed_indices(&gpu, C_AST_KIND_POINTER_DECL), vec![2]);
    assert!(typed_indices(&gpu, C_AST_KIND_ARRAY_DECL).contains(&5));
}

#[test]
fn gpu_parity_empty_struct_body() {
    let tok_types = vec![
        TOK_STRUCT,
        TOK_IDENTIFIER,
        TOK_LBRACE,
        TOK_RBRACE,
        TOK_SEMICOLON,
    ];
    let tok_lens = vec![1; tok_types.len()];
    let tok_starts = starts_for_lens(&tok_lens);
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let expected = reference_c11_classify_vast_node_kinds(&raw);
    let gpu = run_gpu_classifier(&raw, tok_types.len() as u32);

    assert_eq!(
        gpu, expected,
        "GPU classifier must match CPU for empty struct body"
    );
    assert_kind(&gpu, 2, node_kind::BASIC_BLOCK);
}

#[test]
fn gpu_parity_gnu_attribute_before_declaration() {
    let tok_types = vec![
        TOK_GNU_ATTRIBUTE,
        TOK_LPAREN,
        TOK_LPAREN,
        TOK_IDENTIFIER,
        TOK_LPAREN,
        TOK_INTEGER,
        TOK_RPAREN,
        TOK_RPAREN,
        TOK_RPAREN,
        TOK_INT,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
    ];
    let tok_lens = vec![1; tok_types.len()];
    let tok_starts = starts_for_lens(&tok_lens);
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let expected = reference_c11_classify_vast_node_kinds(&raw);
    let gpu = run_gpu_classifier(&raw, tok_types.len() as u32);

    assert_eq!(
        gpu, expected,
        "GPU classifier must match CPU for leading GNU attribute"
    );
    assert_kind(&gpu, 0, C_AST_KIND_GNU_ATTRIBUTE);
    assert_kind(&gpu, 10, node_kind::VARIABLE);
}

#[test]
fn gpu_parity_sizeof_paren() {
    let tok_types = vec![
        TOK_INT,
        TOK_IDENTIFIER,
        TOK_ASSIGN,
        TOK_SIZEOF,
        TOK_LPAREN,
        TOK_INT,
        TOK_RPAREN,
        TOK_SEMICOLON,
    ];
    let tok_lens = vec![1; tok_types.len()];
    let tok_starts = starts_for_lens(&tok_lens);
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let expected = reference_c11_classify_vast_node_kinds(&raw);
    let gpu = run_gpu_classifier(&raw, tok_types.len() as u32);

    assert_eq!(
        gpu, expected,
        "GPU classifier must match CPU for sizeof paren"
    );
    assert_kind(&gpu, 3, C_AST_KIND_SIZEOF_EXPR);
    assert_kind(&gpu, 4, 0);
}
