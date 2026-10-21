use super::*;

#[test]
fn attribute_before_declarator_gpu_cpu_parity() {
    let fix = fixture_attribute_before_declarator();

    // Lexer sanity-check: the raw source must produce the expected raw kinds.
    let lexed = lex_c11_max_munch_kinds(fix.source.as_bytes()).expect("fixture must lex");
    let lexed_non_ws = lexed
        .into_iter()
        .filter(|k| *k != TOK_WHITESPACE && *k != TOK_COMMENT)
        .collect::<Vec<_>>();
    assert_eq!(
        lexed_non_ws, fix.raw_kinds,
        "lexer raw kinds must match hand-built fixture"
    );

    assert_full_pipeline_parity(&fix, "attribute_before_declarator");

    // Semantic spot-checks on the CPU reference (GPU already proven equal).
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);

    assert!(
        !row_indices(&typed, C_AST_KIND_GNU_ATTRIBUTE).is_empty(),
        "__attribute__ must classify as GNU_ATTRIBUTE"
    );
    assert!(
        row_indices(&typed, C_AST_KIND_FUNCTION_DEFINITION).contains(&7),
        "foo declarator with a body must classify as FUNCTION_DEFINITION"
    );
}

#[test]
fn attribute_after_declarator_gpu_cpu_parity() {
    let fix = fixture_attribute_after_declarator();

    let lexed = lex_c11_max_munch_kinds(fix.source.as_bytes()).expect("fixture must lex");
    let lexed_non_ws = lexed
        .into_iter()
        .filter(|k| *k != TOK_WHITESPACE && *k != TOK_COMMENT)
        .collect::<Vec<_>>();
    assert_eq!(lexed_non_ws, fix.raw_kinds);

    assert_full_pipeline_parity(&fix, "attribute_after_declarator");

    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);

    let attr_rows = row_indices(&typed, C_AST_KIND_GNU_ATTRIBUTE);
    assert_eq!(
        attr_rows,
        vec![5],
        "__attribute__ after declarator must classify at correct index"
    );
}

#[test]
fn statement_expression_gpu_cpu_parity() {
    let fix = fixture_statement_expression();

    let lexed = lex_c11_max_munch_kinds(fix.source.as_bytes()).expect("fixture must lex");
    let lexed_non_ws = lexed
        .into_iter()
        .filter(|k| *k != TOK_WHITESPACE && *k != TOK_COMMENT)
        .collect::<Vec<_>>();
    assert_eq!(lexed_non_ws, fix.raw_kinds);

    assert_full_pipeline_parity(&fix, "statement_expression");

    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);

    // Statement expressions are not a distinct VAST kind today; the parity
    // assertion above already proves CPU and GPU agree on the current shape.
    // Spot-check that we got a non-empty typed VAST.
    assert!(
        !typed.is_empty(),
        "statement expression fixture must produce a non-empty typed VAST"
    );
}

#[test]
fn typeof_promotes_to_gnu_typeof_keyword_gpu_cpu_parity() {
    let fix = fixture_typeof();

    let lexed = lex_c11_max_munch_kinds(fix.source.as_bytes()).expect("fixture must lex");
    let lexed_non_ws = lexed
        .into_iter()
        .filter(|k| *k != TOK_WHITESPACE && *k != TOK_COMMENT)
        .collect::<Vec<_>>();
    assert_eq!(lexed_non_ws, fix.raw_kinds);

    assert_eq!(
        fix.tok_types[0], TOK_GNU_TYPEOF,
        "typeof must promote to the GNU typeof token"
    );
    assert_eq!(fix.tok_types[2], TOK_INT, "typeof(int) must promote int");
    assert_eq!(
        fix.tok_types[5], TOK_GNU_TYPEOF,
        "__typeof__ must promote to the GNU typeof token"
    );
    assert_eq!(
        fix.tok_types[7], TOK_INT,
        "__typeof__(int) must promote int"
    );

    assert_full_pipeline_parity(&fix, "typeof");
}

#[test]
fn inline_asm_gpu_cpu_parity() {
    let fix = fixture_inline_asm();

    let lexed = lex_c11_max_munch_kinds(fix.source.as_bytes()).expect("fixture must lex");
    let lexed_non_ws = lexed
        .into_iter()
        .filter(|k| *k != TOK_WHITESPACE && *k != TOK_COMMENT)
        .collect::<Vec<_>>();
    assert_eq!(lexed_non_ws, fix.raw_kinds);

    assert_full_pipeline_parity(&fix, "inline_asm");

    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);

    let asm_rows = row_indices(&typed, C_AST_KIND_INLINE_ASM);
    assert_eq!(asm_rows, vec![0], "asm keyword must classify as INLINE_ASM");
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ASM_TEMPLATE),
        vec![3],
        "asm template string must classify distinctly"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ASM_CLOBBERS_LIST),
        vec![7],
        "asm clobber string must classify distinctly"
    );
}

#[test]
fn extended_asm_operands_classify_gpu_cpu_parity() {
    let fix = fixture_extended_asm_operands();

    let lexed = lex_c11_max_munch_kinds(fix.source.as_bytes()).expect("fixture must lex");
    let lexed_non_ws = lexed
        .into_iter()
        .filter(|k| *k != TOK_WHITESPACE && *k != TOK_COMMENT)
        .collect::<Vec<_>>();
    assert_eq!(lexed_non_ws, fix.raw_kinds);

    assert_full_pipeline_parity(&fix, "extended_asm_operands");

    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    assert_eq!(row_indices(&typed, C_AST_KIND_ASM_TEMPLATE), vec![3]);
    assert_eq!(row_indices(&typed, C_AST_KIND_ASM_OUTPUT_OPERAND), vec![6]);
    assert_eq!(row_indices(&typed, C_AST_KIND_ASM_INPUT_OPERAND), vec![11]);
    assert_eq!(row_indices(&typed, C_AST_KIND_ASM_CLOBBERS_LIST), vec![15]);
}

#[test]
fn asm_goto_label_classifies_gpu_cpu_parity() {
    let fix = fixture_asm_goto();

    let lexed = lex_c11_max_munch_kinds(fix.source.as_bytes()).expect("fixture must lex");
    let lexed_non_ws = lexed
        .into_iter()
        .filter(|k| *k != TOK_WHITESPACE && *k != TOK_COMMENT)
        .collect::<Vec<_>>();
    assert_eq!(lexed_non_ws, fix.raw_kinds);

    assert_full_pipeline_parity(&fix, "asm_goto");

    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    assert_eq!(row_indices(&typed, C_AST_KIND_ASM_TEMPLATE), vec![3]);
    assert_eq!(row_indices(&typed, C_AST_KIND_ASM_GOTO_LABELS), vec![8]);
    assert_ne!(
        word_at(&typed, VAST_STRIDE_U32),
        C_AST_KIND_GOTO_STMT,
        "`goto` after `asm` is a qualifier, not a standalone goto statement"
    );
}

#[test]
fn labels_as_values_classifies_label_address_gpu_cpu_parity() {
    let fix = fixture_labels_as_values();

    let lexed = lex_c11_max_munch_kinds(fix.source.as_bytes()).expect("fixture must lex");
    let lexed_non_ws = lexed
        .into_iter()
        .filter(|k| *k != TOK_WHITESPACE && *k != TOK_COMMENT)
        .collect::<Vec<_>>();
    assert_eq!(lexed_non_ws, fix.raw_kinds);

    assert_eq!(fix.tok_types[4], TOK_AND, "&& must lex as TOK_AND");
    assert_eq!(
        fix.tok_types[5], TOK_IDENTIFIER,
        "label must remain identifier"
    );

    assert_full_pipeline_parity(&fix, "labels_as_values");

    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    assert_eq!(
        row_indices(&typed, C_AST_KIND_GNU_LABEL_ADDRESS_EXPR),
        vec![4],
        "GNU labels-as-values must classify &&label as a label-address expression"
    );
}

#[test]
fn extension_statement_expression_gpu_cpu_parity() {
    let fix = fixture_extension_statement_expression();

    let lexed = lex_c11_max_munch_kinds(fix.source.as_bytes()).expect("fixture must lex");
    let lexed_non_ws = lexed
        .into_iter()
        .filter(|k| *k != TOK_WHITESPACE && *k != TOK_COMMENT)
        .collect::<Vec<_>>();
    assert_eq!(lexed_non_ws, fix.raw_kinds);
    assert_eq!(
        fix.tok_types[0], TOK_GNU_EXTENSION,
        "__extension__ must promote before parsing"
    );

    assert_full_pipeline_parity(&fix, "extension_statement_expression");

    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    assert_eq!(
        word_at(&typed, 0),
        0,
        "__extension__ is a declaration/expression prefix, not a standalone AST node"
    );
    assert_eq!(
        word_at(&typed, 2 * VAST_STRIDE_U32),
        node_kind::BASIC_BLOCK,
        "statement-expression body must classify as a basic block"
    );
    assert_eq!(
        word_at(&typed, 5 * VAST_STRIDE_U32),
        C_AST_KIND_ASSIGN_EXPR,
        "assignment inside statement expression must classify"
    );
}

#[test]
fn extension_declaration_prefix_gpu_cpu_parity() {
    let fix = fixture_extension_declaration();

    let lexed = lex_c11_max_munch_kinds(fix.source.as_bytes()).expect("fixture must lex");
    let lexed_non_ws = lexed
        .into_iter()
        .filter(|k| *k != TOK_WHITESPACE && *k != TOK_COMMENT)
        .collect::<Vec<_>>();
    assert_eq!(lexed_non_ws, fix.raw_kinds);
    assert_eq!(fix.tok_types[0], TOK_GNU_EXTENSION);

    assert_full_pipeline_parity(&fix, "extension_declaration");

    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    assert_eq!(
        word_at(&typed, 0),
        0,
        "__extension__ prefix must not become a fake AST node"
    );
    assert_eq!(
        word_at(&typed, 2 * VAST_STRIDE_U32),
        node_kind::VARIABLE,
        "identifier after __extension__ declaration prefix must classify as variable"
    );
}

#[test]
fn builtins_and_generic_classify_as_distinct_exprs_gpu_cpu_parity() {
    let fix = fixture_builtin_and_generic_expressions();

    let lexed = lex_c11_max_munch_kinds(fix.source.as_bytes()).expect("fixture must lex");
    let lexed_non_ws = lexed
        .into_iter()
        .filter(|k| *k != TOK_WHITESPACE && *k != TOK_COMMENT)
        .collect::<Vec<_>>();
    assert_eq!(lexed_non_ws, fix.raw_kinds);
    assert_eq!(fix.tok_types[3], TOK_BUILTIN_CONSTANT_P);
    assert_eq!(fix.tok_types[15], TOK_BUILTIN_CHOOSE_EXPR);
    assert_eq!(fix.tok_types[27], TOK_BUILTIN_TYPES_COMPATIBLE_P);
    assert_eq!(fix.tok_types[37], TOK_GENERIC);

    assert_full_pipeline_parity(&fix, "builtin_and_generic_expressions");

    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    assert_eq!(
        row_indices(&typed, C_AST_KIND_BUILTIN_CONSTANT_P_EXPR),
        vec![3],
        "__builtin_constant_p must be a distinct expression kind"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_BUILTIN_CHOOSE_EXPR),
        vec![15],
        "__builtin_choose_expr must be a distinct expression kind"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_BUILTIN_TYPES_COMPATIBLE_P_EXPR),
        vec![27],
        "__builtin_types_compatible_p must be a distinct expression kind"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_GENERIC_SELECTION_EXPR),
        vec![37],
        "_Generic must be a distinct selection-expression kind"
    );
    for idx in [3, 15, 27, 37] {
        assert_ne!(
            word_at(&typed, idx * VAST_STRIDE_U32),
            node_kind::CALL,
            "builtin/generic row {idx} must not collapse into generic CALL"
        );
    }
}

#[test]
fn range_designator_ellipsis_classifies_gpu_cpu_parity() {
    let fix = fixture_range_designator();

    let lexed = lex_c11_max_munch_kinds(fix.source.as_bytes()).expect("fixture must lex");
    let lexed_non_ws = lexed
        .into_iter()
        .filter(|k| *k != TOK_WHITESPACE && *k != TOK_COMMENT)
        .collect::<Vec<_>>();
    assert_eq!(lexed_non_ws, fix.raw_kinds);
    assert_eq!(
        fix.tok_types[8], TOK_ELLIPSIS,
        "`...` must lex as one ellipsis token, not three dots"
    );

    assert_full_pipeline_parity(&fix, "range_designator");

    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    assert_eq!(
        row_indices(&typed, C_AST_KIND_RANGE_DESIGNATOR_EXPR),
        vec![8],
        "GNU range designator ellipsis must be a distinct AST marker"
    );
}

#[test]
fn packed_struct_attribute_gpu_cpu_parity() {
    let fix = fixture_packed_struct_attribute();

    let lexed = lex_c11_max_munch_kinds(fix.source.as_bytes()).expect("fixture must lex");
    let lexed_non_ws = lexed
        .into_iter()
        .filter(|k| *k != TOK_WHITESPACE && *k != TOK_COMMENT)
        .collect::<Vec<_>>();
    assert_eq!(lexed_non_ws, fix.raw_kinds);
    assert_eq!(fix.tok_types[1], TOK_GNU_ATTRIBUTE);

    assert_full_pipeline_parity(&fix, "packed_struct_attribute");

    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    assert_eq!(
        row_indices(&typed, C_AST_KIND_GNU_ATTRIBUTE),
        vec![1],
        "packed attribute must classify at the attribute token"
    );
    assert_eq!(
        word_at(&typed, 7 * VAST_STRIDE_U32),
        node_kind::VARIABLE,
        "struct tag name must not classify as a field declaration"
    );
    assert_eq!(
        word_at(&typed, 8 * VAST_STRIDE_U32),
        node_kind::BASIC_BLOCK,
        "struct body brace must classify as a basic block"
    );
    assert_eq!(
        word_at(&typed, 10 * VAST_STRIDE_U32),
        C_AST_KIND_FIELD_DECL,
        "field inside packed struct must classify as a field declaration"
    );
}

