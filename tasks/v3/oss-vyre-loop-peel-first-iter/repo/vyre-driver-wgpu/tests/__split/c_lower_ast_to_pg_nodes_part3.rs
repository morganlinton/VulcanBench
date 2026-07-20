use super::*;

#[test]
fn typed_c_declarator_initializer_vast_lowers_to_program_graph_nodes() {
    let (tok_types, tok_starts, tok_lens) = c_declarator_initializer_fixture_tokens();
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
        "Fix: declarator and initializer semantic VAST rows must lower without drift"
    );

    let expected_rows = [
        (4usize, C_AST_KIND_FIELD_DECL),
        (8, C_AST_KIND_POINTER_DECL),
        (12, C_AST_KIND_FIELD_DECL),
        (13, C_AST_KIND_ARRAY_DECL),
        (22, C_AST_KIND_ENUMERATOR_DECL),
        (26, C_AST_KIND_ENUMERATOR_DECL),
        (28, C_AST_KIND_ENUMERATOR_DECL),
        (35, C_AST_KIND_FUNCTION_DECLARATOR),
        (40, C_AST_KIND_POINTER_DECL),
        (46, C_AST_KIND_CAST_EXPR),
        (48, C_AST_KIND_POINTER_DECL),
        (56, C_AST_KIND_COMPOUND_LITERAL_EXPR),
        (60, C_AST_KIND_INITIALIZER_LIST),
    ];

    for (idx, kind) in expected_rows {
        assert_eq!(
            word_at(&typed_vast, idx * VAST_STRIDE_U32 as usize),
            kind,
            "Fix: typed declarator/initializer kind at row {idx}"
        );
        assert_eq!(
            word_at(&expected_pg, idx * PG_STRIDE_U32 as usize),
            kind,
            "Fix: PG declarator/initializer kind at row {idx}"
        );
        assert_eq!(
            word_at(&expected_pg, idx * PG_STRIDE_U32 as usize + 1),
            tok_starts[idx],
            "Fix: PG declarator/initializer span start at row {idx}"
        );
    }
}

#[test]
fn nested_function_pointer_array_prototype_lowers_to_program_graph_nodes() {
    let (tok_types, tok_starts, tok_lens) = c_function_pointer_array_prototype_fixture_tokens();
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
        "Fix: nested callback declarator VAST rows must lower without drift"
    );

    let expected_rows = [
        (2usize, C_AST_KIND_POINTER_DECL),
        (4, C_AST_KIND_ARRAY_DECL),
        (9, C_AST_KIND_FUNCTION_DECLARATOR),
        (12, C_AST_KIND_POINTER_DECL),
        (18, C_AST_KIND_ARRAY_DECL),
    ];

    for (idx, kind) in expected_rows {
        assert_eq!(
            word_at(&typed_vast, idx * VAST_STRIDE_U32 as usize),
            kind,
            "Fix: typed nested declarator kind at row {idx}"
        );
        assert_eq!(
            word_at(&expected_pg, idx * PG_STRIDE_U32 as usize),
            kind,
            "Fix: PG nested declarator kind at row {idx}"
        );
        assert_eq!(
            word_at(&expected_pg, idx * PG_STRIDE_U32 as usize + 1),
            tok_starts[idx],
            "Fix: PG nested declarator span start at row {idx}"
        );
    }
}

#[test]
fn nested_function_pointer_array_prototype_gpu_lowers_like_cpu() {
    static BACKEND: OnceLock<WgpuBackend> = OnceLock::new();
    let backend =
        BACKEND.get_or_init(|| WgpuBackend::acquire().expect("Fix: GPU backend must be available"));

    let (tok_types, _, tok_lens) = c_function_pointer_array_prototype_fixture_tokens();
    let tok_starts = starts_for_lens(&tok_lens);
    let raw_vast = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed_vast = reference_c11_classify_vast_node_kinds(&raw_vast);
    let program = c_lower_ast_to_pg_nodes(
        "vast_nodes",
        Expr::u32(node_count_from_vast(&typed_vast)),
        "pg_nodes",
    );
    let optimized = optimize(program.clone());
    let expected = run_reference_eval(&program, std::slice::from_ref(&typed_vast));
    let actual = backend
        .dispatch(&optimized, &[typed_vast], &DispatchConfig::default())
        .expect("Fix: GPU PG lowering dispatch must succeed");

    assert_eq!(
        actual, expected,
        "Fix: GPU PG lowering must match CPU for nested callback declarators"
    );
}

