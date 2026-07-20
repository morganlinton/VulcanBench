use super::*;

#[test]
fn gpu_parity_classifies_gnu_attribute_and_inline_asm_nodes() {
    let backend = match WgpuBackend::new() {
        Ok(backend) => backend,
        Err(error) => panic!(
            "WgpuBackend::new failed on a machine that must have a GPU: {error}. \
             This is a configuration bug, not a graceful skip."
        ),
    };

    let tok_types = [
        TOK_STATIC,
        TOK_INLINE,
        TOK_INT,
        TOK_IDENTIFIER,
        TOK_LPAREN,
        TOK_RPAREN,
        TOK_GNU_ATTRIBUTE,
        TOK_LPAREN,
        TOK_LPAREN,
        TOK_IDENTIFIER,
        TOK_RPAREN,
        TOK_RPAREN,
        TOK_LBRACE,
        TOK_GNU_ASM,
        TOK_VOLATILE,
        TOK_LPAREN,
        TOK_STRING,
        TOK_COLON,
        TOK_COLON,
        TOK_COLON,
        TOK_STRING,
        TOK_RPAREN,
        TOK_SEMICOLON,
        TOK_RETURN,
        TOK_INTEGER,
        TOK_SEMICOLON,
        TOK_RBRACE,
    ];
    let tok_lens = [
        6, 6, 3, 6, 1, 1, 13, 1, 1, 13, 1, 1, 1, 3, 8, 1, 5, 1, 1, 1, 8, 1, 1, 6, 1, 1, 1,
    ];
    let tok_starts = starts_for_lens(&tok_lens);
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let expected = reference_c11_classify_vast_node_kinds(&raw);
    let program = c11_classify_vast_node_kinds(
        "vast_nodes",
        Expr::u32(tok_types.len() as u32),
        "typed_vast_nodes",
    );
    let inputs: Vec<&[u8]> = vec![&raw];
    let outputs = backend
        .dispatch_borrowed(&program, &inputs, &Default::default())
        .expect("GPU VAST classifier dispatch must succeed");

    assert_eq!(outputs.len(), 1, "expected one typed VAST output");
    assert_eq!(outputs[0], expected, "GPU classifier must match CPU oracle");
    assert_kind(&outputs[0], 3, C_AST_KIND_FUNCTION_DEFINITION);
    assert_kind(&outputs[0], 6, C_AST_KIND_GNU_ATTRIBUTE);
    assert_kind(&outputs[0], 13, C_AST_KIND_INLINE_ASM);
    assert_kind(&outputs[0], 16, C_AST_KIND_ASM_TEMPLATE);
    assert_kind(&outputs[0], 20, C_AST_KIND_ASM_CLOBBERS_LIST);
}

#[test]
fn gpu_parity_classifies_c_statement_nodes() {
    let backend = match WgpuBackend::new() {
        Ok(backend) => backend,
        Err(error) => panic!(
            "WgpuBackend::new failed on a machine that must have a GPU: {error}. \
             This is a configuration bug, not a graceful skip."
        ),
    };

    let tok_types = [
        TOK_IF,
        TOK_LPAREN,
        TOK_IDENTIFIER,
        TOK_RPAREN,
        TOK_RETURN,
        TOK_INTEGER,
        TOK_SEMICOLON,
        TOK_ELSE,
        TOK_FOR,
        TOK_LPAREN,
        TOK_SEMICOLON,
        TOK_SEMICOLON,
        TOK_RPAREN,
        TOK_WHILE,
        TOK_LPAREN,
        TOK_IDENTIFIER,
        TOK_RPAREN,
        TOK_DO,
        TOK_CONTINUE,
        TOK_SEMICOLON,
        TOK_WHILE,
        TOK_LPAREN,
        TOK_IDENTIFIER,
        TOK_RPAREN,
        TOK_SEMICOLON,
        TOK_SWITCH,
        TOK_LPAREN,
        TOK_IDENTIFIER,
        TOK_RPAREN,
        TOK_LBRACE,
        TOK_CASE,
        TOK_INTEGER,
        TOK_COLON,
        TOK_BREAK,
        TOK_SEMICOLON,
        TOK_DEFAULT,
        TOK_COLON,
        TOK_GOTO,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
        TOK_RBRACE,
    ];
    let tok_lens = [
        2, 1, 1, 1, 6, 1, 1, 4, 3, 1, 1, 1, 1, 5, 1, 1, 1, 2, 8, 1, 5, 1, 1, 1, 1, 6, 1, 1, 1, 1,
        4, 1, 1, 5, 1, 7, 1, 4, 3, 1, 1,
    ];
    let tok_starts = starts_for_lens(&tok_lens);
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let expected = reference_c11_classify_vast_node_kinds(&raw);
    let program = c11_classify_vast_node_kinds(
        "vast_nodes",
        Expr::u32(tok_types.len() as u32),
        "typed_vast_nodes",
    );
    let inputs: Vec<&[u8]> = vec![&raw];
    let outputs = backend
        .dispatch_borrowed(&program, &inputs, &Default::default())
        .expect("GPU VAST classifier dispatch must succeed");

    assert_eq!(outputs.len(), 1, "expected one typed VAST output");
    assert_eq!(outputs[0], expected, "GPU classifier must match CPU oracle");
    assert_kind(&outputs[0], 0, C_AST_KIND_IF_STMT);
    assert_kind(&outputs[0], 4, C_AST_KIND_RETURN_STMT);
    assert_kind(&outputs[0], 8, C_AST_KIND_FOR_STMT);
    assert_kind(&outputs[0], 17, C_AST_KIND_DO_STMT);
    assert_kind(&outputs[0], 25, C_AST_KIND_SWITCH_STMT);
    assert_kind(&outputs[0], 37, C_AST_KIND_GOTO_STMT);
}

#[test]
fn gpu_parity_classifies_c_expression_operator_nodes() {
    let backend = match WgpuBackend::new() {
        Ok(backend) => backend,
        Err(error) => panic!(
            "WgpuBackend::new failed on a machine that must have a GPU: {error}. \
             This is a configuration bug, not a graceful skip."
        ),
    };

    let (tok_types, tok_starts, tok_lens) = expression_operator_fixture();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let expected = reference_c11_classify_vast_node_kinds(&raw);
    let program = c11_classify_vast_node_kinds(
        "vast_nodes",
        Expr::u32(tok_types.len() as u32),
        "typed_vast_nodes",
    );
    let inputs: Vec<&[u8]> = vec![&raw];
    let outputs = backend
        .dispatch_borrowed(&program, &inputs, &Default::default())
        .expect("GPU VAST classifier dispatch must succeed");

    assert_eq!(outputs.len(), 1, "expected one typed VAST output");
    assert_eq!(outputs[0], expected, "GPU classifier must match CPU oracle");
    assert_kind(&outputs[0], 1, C_AST_KIND_MEMBER_ACCESS_EXPR);
    assert_kind(&outputs[0], 3, C_AST_KIND_ASSIGN_EXPR);
    assert_kind(&outputs[0], 4, C_AST_KIND_SIZEOF_EXPR);
    assert_kind(&outputs[0], 6, C_AST_KIND_UNARY_EXPR);
    assert_kind(&outputs[0], 13, node_kind::BINARY);
    assert_kind(&outputs[0], 19, C_AST_KIND_CONDITIONAL_EXPR);
    assert_kind(&outputs[0], 28, C_AST_KIND_ASSIGN_EXPR);
    assert_kind(&outputs[0], 32, node_kind::BINARY);
    assert_kind(&outputs[0], 36, node_kind::BINARY);
    assert_kind(&outputs[0], 40, C_AST_KIND_ARRAY_SUBSCRIPT_EXPR);
    assert_kind(&outputs[0], 46, C_AST_KIND_UNARY_EXPR);
    assert_kind(&outputs[0], 48, node_kind::BINARY);
    assert_kind(&outputs[0], 49, C_AST_KIND_UNARY_EXPR);
    assert_kind(&outputs[0], 52, C_AST_KIND_UNARY_EXPR);
    assert_kind(&outputs[0], 56, node_kind::BINARY);
}

#[test]
fn gpu_parity_builds_c11_expression_semantic_shape_rows() {
    let backend = match WgpuBackend::new() {
        Ok(backend) => backend,
        Err(error) => panic!(
            "WgpuBackend::new failed on a machine that must have a GPU: {error}. \
             This is a configuration bug, not a graceful skip."
        ),
    };

    let (tok_types, tok_starts, tok_lens) = expression_shape_fixture();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let expected = reference_c11_build_expression_shape_nodes(&raw, &typed);
    let program = c11_build_expression_shape_nodes(
        "raw_vast_nodes",
        "typed_vast_nodes",
        Expr::u32(tok_types.len() as u32),
        "out_expr_shape_nodes",
    );
    let inputs: Vec<&[u8]> = vec![&raw, &typed];
    let outputs = backend
        .dispatch_borrowed(&program, &inputs, &Default::default())
        .expect("GPU expression-shape dispatch must succeed");

    assert_eq!(outputs.len(), 1, "expected one expression-shape output");
    assert_eq!(
        outputs[0], expected,
        "GPU expression-shape rows must match CPU oracle"
    );
    assert_expr_shape_row(
        &outputs[0],
        5,
        C_EXPR_SHAPE_CONDITIONAL,
        TOK_QUESTION,
        3,
        C_EXPR_ASSOC_RIGHT,
        1,
        7,
        11,
    );
}

