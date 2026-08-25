use super::*;

#[test]
fn c11_vast_reference_feeds_program_graph_node_lowering() {
    let tok_types = [
        TOK_INT,
        TOK_IDENTIFIER,
        TOK_LPAREN,
        TOK_RPAREN,
        TOK_LBRACE,
        TOK_RETURN,
        TOK_INTEGER,
        TOK_SEMICOLON,
        TOK_RBRACE,
    ];
    let tok_starts = [0u32, 4, 8, 9, 10, 11, 18, 19, 20];
    let tok_lens = [3u32, 4, 1, 1, 1, 6, 1, 1, 1];
    let vast = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let expected = reference_ast_to_pg_nodes(&vast);
    let program = c_lower_ast_to_pg_nodes(
        "vast_nodes",
        Expr::u32(node_count_from_vast(&vast)),
        "pg_nodes",
    );
    let actual = run_reference_eval(&program, std::slice::from_ref(&vast));
    assert_eq!(actual, vec![expected.clone()]);

    assert_eq!(word_at(&expected, 2 * 6 + 4), 3, "paren first child");
    assert_eq!(word_at(&expected, 2 * 6 + 5), 4, "paren next sibling");
    assert_eq!(word_at(&expected, 4 * 6 + 4), 5, "body first child");
}

#[test]
fn typed_gnu_c_vast_lowers_to_program_graph_nodes() {
    let tok_types = [
        TOK_STATIC,
        TOK_INLINE,
        TOK_LONG,
        TOK_STAR,
        TOK_IDENTIFIER,
        TOK_LPAREN,
        TOK_STRUCT,
        TOK_IDENTIFIER,
        TOK_STAR,
        TOK_IDENTIFIER,
        TOK_COMMA,
        TOK_CONST,
        TOK_CHAR_KW,
        TOK_IDENTIFIER,
        TOK_STAR,
        TOK_IDENTIFIER,
        TOK_RPAREN,
        TOK_LBRACE,
        TOK_RETURN,
        TOK_IDENTIFIER,
        TOK_LPAREN,
        TOK_IDENTIFIER,
        TOK_RPAREN,
        TOK_QUESTION,
        TOK_IDENTIFIER,
        TOK_LPAREN,
        TOK_IDENTIFIER,
        TOK_RPAREN,
        TOK_COLON,
        TOK_INTEGER,
        TOK_SEMICOLON,
        TOK_RBRACE,
    ];
    let tok_lens = [
        6, 6, 4, 1, 9, 1, 6, 6, 1, 3, 1, 5, 4, 6, 1, 3, 1, 1, 6, 6, 1, 3, 1, 1, 7, 1, 3, 1, 1, 1,
        1, 1,
    ];
    let tok_starts = starts_for_lens(&tok_lens);
    let raw_vast = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed_vast = reference_c11_classify_vast_node_kinds(&raw_vast);
    let expected = reference_ast_to_pg_nodes(&typed_vast);
    let program = c_lower_ast_to_pg_nodes(
        "vast_nodes",
        Expr::u32(node_count_from_vast(&typed_vast)),
        "pg_nodes",
    );
    let actual = run_reference_eval(&program, std::slice::from_ref(&typed_vast));
    assert_eq!(actual, vec![expected.clone()]);

    assert_eq!(word_at(&expected, 4 * 6), C_AST_KIND_FUNCTION_DEFINITION);
    assert_eq!(word_at(&expected, 17 * 6), node_kind::BASIC_BLOCK);
    assert_eq!(word_at(&expected, 19 * 6), node_kind::CALL);
    assert_eq!(word_at(&expected, 24 * 6), node_kind::CALL);
    assert_eq!(word_at(&expected, 29 * 6), node_kind::LITERAL);
    assert_eq!(
        word_at(&expected, 4 * 6 + 1),
        tok_starts[4],
        "function span start must lower into PG"
    );
    assert_eq!(
        word_at(&expected, 4 * 6 + 2),
        tok_starts[4] + tok_lens[4],
        "function span end must lower into PG"
    );
}

#[test]
fn typed_c_expression_operator_vast_lowers_to_program_graph_nodes() {
    let (tok_types, tok_starts, tok_lens) = c_expression_operator_fixture_tokens();
    let raw_vast = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed_vast = reference_c11_classify_vast_node_kinds(&raw_vast);
    let expected_pg = reference_ast_to_pg_nodes(&typed_vast);
    let program = c_lower_ast_to_pg_nodes(
        "vast_nodes",
        Expr::u32(node_count_from_vast(&typed_vast)),
        "pg_nodes",
    );
    let actual = run_reference_eval(&program, std::slice::from_ref(&typed_vast));
    assert_eq!(
        actual,
        vec![expected_pg.clone()],
        "Fix: expression operator VAST rows must lower without kind or span drift"
    );

    let typed_assignments = row_indices(
        &typed_vast,
        VAST_STRIDE_U32 as usize,
        C_AST_KIND_ASSIGN_EXPR,
    );
    let typed_member_access = row_indices(
        &typed_vast,
        VAST_STRIDE_U32 as usize,
        C_AST_KIND_MEMBER_ACCESS_EXPR,
    );
    let typed_binary = row_indices(&typed_vast, VAST_STRIDE_U32 as usize, node_kind::BINARY);
    let typed_sizeof = row_indices(
        &typed_vast,
        VAST_STRIDE_U32 as usize,
        C_AST_KIND_SIZEOF_EXPR,
    );
    let typed_conditional = row_indices(
        &typed_vast,
        VAST_STRIDE_U32 as usize,
        C_AST_KIND_CONDITIONAL_EXPR,
    );
    let typed_unary = row_indices(&typed_vast, VAST_STRIDE_U32 as usize, C_AST_KIND_UNARY_EXPR);
    let typed_subscript = row_indices(
        &typed_vast,
        VAST_STRIDE_U32 as usize,
        C_AST_KIND_ARRAY_SUBSCRIPT_EXPR,
    );

    assert_eq!(typed_assignments, vec![3, 28, 45]);
    assert_eq!(typed_member_access, vec![1, 26]);
    assert_eq!(typed_binary, vec![13, 15, 17, 32, 36, 48, 56]);
    assert_eq!(typed_sizeof, vec![4]);
    assert_eq!(typed_conditional, vec![19]);
    assert_eq!(typed_unary, vec![6, 46, 49, 52]);
    assert_eq!(typed_subscript, vec![40]);

    for idx in [
        1usize, 3, 4, 6, 13, 15, 17, 19, 26, 28, 32, 36, 40, 45, 46, 48, 49, 52, 56,
    ] {
        assert_eq!(
            word_at(&expected_pg, idx * PG_STRIDE_U32 as usize),
            word_at(&typed_vast, idx * VAST_STRIDE_U32 as usize),
            "Fix: expression node kind at row {idx} must survive PG lowering"
        );
        assert_eq!(
            word_at(&expected_pg, idx * PG_STRIDE_U32 as usize + 1),
            tok_starts[idx],
            "Fix: expression node span start at row {idx} must survive PG lowering"
        );
    }
}

