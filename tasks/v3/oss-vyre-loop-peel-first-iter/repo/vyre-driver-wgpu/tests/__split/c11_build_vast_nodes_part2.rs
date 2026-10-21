use super::*;

#[test]
fn cpu_reference_classifies_c_expression_operators_as_first_class_vast_nodes() {
    let (tok_types, tok_starts, tok_lens) = expression_operator_fixture();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    let expected = [
        (1, C_AST_KIND_MEMBER_ACCESS_EXPR, "pointer member access"),
        (3, C_AST_KIND_ASSIGN_EXPR, "simple assignment"),
        (4, C_AST_KIND_SIZEOF_EXPR, "sizeof expression"),
        (6, C_AST_KIND_UNARY_EXPR, "sizeof dereference"),
        (13, node_kind::BINARY, "relational operator"),
        (15, node_kind::BINARY, "logical and operator"),
        (17, node_kind::BINARY, "inequality operator"),
        (19, C_AST_KIND_CONDITIONAL_EXPR, "ternary marker"),
        (26, C_AST_KIND_MEMBER_ACCESS_EXPR, "direct member access"),
        (28, C_AST_KIND_ASSIGN_EXPR, "compound assignment"),
        (32, node_kind::BINARY, "shift operator"),
        (36, node_kind::BINARY, "remainder operator"),
        (40, C_AST_KIND_ARRAY_SUBSCRIPT_EXPR, "array subscript"),
        (45, C_AST_KIND_ASSIGN_EXPR, "assignment before unary"),
        (46, C_AST_KIND_UNARY_EXPR, "prefix minus"),
        (48, node_kind::BINARY, "binary plus"),
        (49, C_AST_KIND_UNARY_EXPR, "pointer dereference"),
        (52, C_AST_KIND_UNARY_EXPR, "prefix increment"),
        (56, node_kind::BINARY, "bitwise and"),
    ];

    for (idx, kind, label) in expected {
        assert_kind(&typed, idx, kind);
        assert_eq!(
            word_at(&typed, idx * VAST_STRIDE_U32 + 5),
            tok_starts[idx],
            "{label} span start must survive classification"
        );
    }

    assert_vast_row(&raw, 5, TOK_LPAREN, u32::MAX, 6, 9);
    assert_vast_row(&raw, 11, TOK_LPAREN, u32::MAX, 12, 24);
    assert_vast_row(&typed, 24, node_kind::BASIC_BLOCK, u32::MAX, 25, u32::MAX);
}

#[test]
fn cpu_reference_builds_c11_expression_semantic_shape_rows() {
    let (tok_types, tok_starts, tok_lens) = expression_shape_fixture();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let shape = reference_c11_build_expression_shape_nodes(&raw, &typed);

    assert_expr_shape_row(
        &shape,
        1,
        C_EXPR_SHAPE_BINARY,
        TOK_PLUS,
        12,
        C_EXPR_ASSOC_LEFT,
        0,
        3,
        u32::MAX,
    );
    assert_expr_shape_row(
        &shape,
        3,
        C_EXPR_SHAPE_BINARY,
        TOK_STAR,
        13,
        C_EXPR_ASSOC_LEFT,
        2,
        4,
        u32::MAX,
    );
    assert_expr_shape_row(
        &shape,
        5,
        C_EXPR_SHAPE_CONDITIONAL,
        TOK_QUESTION,
        3,
        C_EXPR_ASSOC_RIGHT,
        1,
        7,
        11,
    );
    assert_expr_shape_row(
        &shape,
        7,
        C_EXPR_SHAPE_BINARY,
        TOK_PLUS,
        12,
        C_EXPR_ASSOC_LEFT,
        6,
        8,
        u32::MAX,
    );
    assert_expr_shape_row(
        &shape,
        11,
        C_EXPR_SHAPE_BINARY,
        TOK_STAR,
        13,
        C_EXPR_ASSOC_LEFT,
        10,
        12,
        u32::MAX,
    );
}

#[test]
fn cpu_reference_classifies_c_declarators_initializers_and_fields() {
    let (tok_types, tok_starts, tok_lens) = declarator_initializer_fixture();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    assert_eq!(typed_indices(&typed, C_AST_KIND_FIELD_DECL), vec![4, 9, 12]);
    assert_eq!(
        typed_indices(&typed, C_AST_KIND_ENUMERATOR_DECL),
        vec![22, 26, 28]
    );
    assert_eq!(typed_indices(&typed, C_AST_KIND_ARRAY_DECL), vec![13]);
    assert_eq!(
        typed_indices(&typed, C_AST_KIND_FUNCTION_DECLARATOR),
        vec![35]
    );
    assert_eq!(
        typed_indices(&typed, C_AST_KIND_POINTER_DECL),
        vec![8, 40, 48]
    );
    assert_eq!(typed_indices(&typed, C_AST_KIND_CAST_EXPR), vec![46]);
    assert_eq!(
        typed_indices(&typed, C_AST_KIND_COMPOUND_LITERAL_EXPR),
        vec![56]
    );
    assert_eq!(typed_indices(&typed, C_AST_KIND_INITIALIZER_LIST), vec![60]);

    for idx in [4usize, 9, 12, 13, 22, 26, 28, 35, 40, 46, 56, 60] {
        assert_eq!(
            word_at(&typed, idx * VAST_STRIDE_U32 + 5),
            tok_starts[idx],
            "semantic declarator/initializer row {idx} must preserve source start"
        );
    }
}

#[test]
fn cpu_reference_classifies_nested_function_pointer_array_prototype() {
    let (tok_types, tok_starts, tok_lens) = function_pointer_array_prototype_fixture();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    assert_eq!(
        typed_indices(&typed, C_AST_KIND_POINTER_DECL),
        vec![2, 12],
        "nested callback pointer and pointer parameter must both type as pointer declarators"
    );
    assert_eq!(
        typed_indices(&typed, C_AST_KIND_ARRAY_DECL),
        vec![4, 18],
        "callback array declarator and array parameter must both type as array declarators"
    );
    assert_eq!(
        typed_indices(&typed, C_AST_KIND_FUNCTION_DECLARATOR),
        vec![9],
        "function-pointer prototype parameter list must remain a function declarator"
    );

    for idx in [2usize, 4, 9, 12, 18] {
        assert_eq!(
            word_at(&typed, idx * VAST_STRIDE_U32 + 5),
            tok_starts[idx],
            "nested declarator row {idx} must preserve source start"
        );
    }
}

