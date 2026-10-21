use super::*;

#[test]
fn gpu_parity_classifies_c_declarators_initializers_and_fields() {
    let backend = match WgpuBackend::new() {
        Ok(backend) => backend,
        Err(error) => panic!(
            "WgpuBackend::new failed on a machine that must have a GPU: {error}. \
             This is a configuration bug, not a graceful skip."
        ),
    };

    let (tok_types, tok_starts, tok_lens) = declarator_initializer_fixture();
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
    assert_kind(&outputs[0], 4, C_AST_KIND_FIELD_DECL);
    assert_kind(&outputs[0], 8, C_AST_KIND_POINTER_DECL);
    assert_kind(&outputs[0], 13, C_AST_KIND_ARRAY_DECL);
    assert_kind(&outputs[0], 22, C_AST_KIND_ENUMERATOR_DECL);
    assert_kind(&outputs[0], 35, C_AST_KIND_FUNCTION_DECLARATOR);
    assert_kind(&outputs[0], 46, C_AST_KIND_CAST_EXPR);
    assert_kind(&outputs[0], 56, C_AST_KIND_COMPOUND_LITERAL_EXPR);
    assert_kind(&outputs[0], 60, C_AST_KIND_INITIALIZER_LIST);
}

#[test]
fn gpu_parity_classifies_nested_function_pointer_array_prototype() {
    let backend = match WgpuBackend::new() {
        Ok(backend) => backend,
        Err(error) => panic!(
            "WgpuBackend::new failed on a machine that must have a GPU: {error}. \
             This is a configuration bug, not a graceful skip."
        ),
    };

    let (tok_types, tok_starts, tok_lens) = function_pointer_array_prototype_fixture();
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
    assert_kind(&outputs[0], 2, C_AST_KIND_POINTER_DECL);
    assert_kind(&outputs[0], 4, C_AST_KIND_ARRAY_DECL);
    assert_kind(&outputs[0], 9, C_AST_KIND_FUNCTION_DECLARATOR);
    assert_kind(&outputs[0], 12, C_AST_KIND_POINTER_DECL);
    assert_kind(&outputs[0], 18, C_AST_KIND_ARRAY_DECL);
}

#[test]
fn gpu_parity_classifies_anonymous_aggregate_declarators() {
    let backend = match WgpuBackend::new() {
        Ok(backend) => backend,
        Err(error) => panic!(
            "WgpuBackend::new failed on a machine that must have a GPU: {error}. \
             This is a configuration bug, not a graceful skip."
        ),
    };

    let (tok_types, tok_starts, tok_lens) = anonymous_aggregate_declarator_fixture();
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
    assert_kind(&outputs[0], 10, C_AST_KIND_POINTER_DECL);
    assert_kind(&outputs[0], 12, C_AST_KIND_ARRAY_DECL);
    assert_kind(&outputs[0], 16, C_AST_KIND_FUNCTION_DECLARATOR);
    assert_kind(&outputs[0], 28, C_AST_KIND_FIELD_DECL);
    assert_kind(&outputs[0], 39, C_AST_KIND_FIELD_DECL);
}

#[test]
fn gpu_parity_int_main_return_zero_vast_rows() {
    let backend = match WgpuBackend::new() {
        Ok(backend) => backend,
        Err(error) => panic!(
            "WgpuBackend::new failed on a machine that must have a GPU: {error}. \
             This is a configuration bug, not a graceful skip."
        ),
    };

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
    let expected = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let program = c11_build_vast_nodes(
        "tok_types",
        "tok_starts",
        "tok_lens",
        Expr::u32(tok_types.len() as u32),
        "out_vast_nodes",
        "out_count",
    );

    let tok_type_bytes = bytes(&tok_types);
    let tok_start_bytes = bytes(&tok_starts);
    let tok_len_bytes = bytes(&tok_lens);
    let inputs: Vec<&[u8]> = vec![&tok_type_bytes, &tok_start_bytes, &tok_len_bytes];
    let outputs = backend
        .dispatch_borrowed(&program, &inputs, &Default::default())
        .expect("GPU VAST builder dispatch must succeed");

    assert_eq!(outputs.len(), 2, "expected VAST rows and count outputs");
    assert_eq!(outputs[0], expected, "GPU VAST rows must match CPU oracle");
    assert_eq!(
        word_at(&outputs[1], 0),
        tok_types.len() as u32,
        "VAST builder must write exact node count"
    );

    for i in 0..tok_types.len() {
        let row = i * VAST_STRIDE_U32;
        assert_eq!(word_at(&outputs[0], row), tok_types[i], "kind[{i}]");
        assert_eq!(word_at(&outputs[0], row + 5), tok_starts[i], "start[{i}]");
        assert_eq!(word_at(&outputs[0], row + 6), tok_lens[i], "len[{i}]");
    }

    assert_vast_row(&outputs[0], 2, TOK_LPAREN, u32::MAX, 3, 4);
    assert_vast_row(&outputs[0], 4, TOK_LBRACE, u32::MAX, 5, u32::MAX);
    assert_vast_row(&outputs[0], 8, TOK_RBRACE, 4, u32::MAX, u32::MAX);
}
