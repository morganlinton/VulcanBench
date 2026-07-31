use super::*;

#[test]
fn kernel_style_fixture_lexes_and_promotes_expected_tokens() {
    let fix = fixture_token_stream();
    let lexed_raw = lex_c11_max_munch_kinds(fix.source.as_bytes())
        .expect("kernel-style parser corpus fixture must lex");
    let lexed_non_ws = lexed_raw
        .into_iter()
        .filter(|kind| *kind != TOK_WHITESPACE && *kind != TOK_COMMENT)
        .collect::<Vec<_>>();

    assert_eq!(
        lexed_non_ws, fix.tok_types,
        "fixture source must produce the expected promoted preprocessing-token stream"
    );
    assert_eq!(
        fix.raw_kinds
            .iter()
            .filter(|kind| **kind == TOK_PREPROC)
            .count(),
        2,
        "macro definitions must enter the AST lane as token-stream shaped preprocessor rows"
    );
    assert!(
        fix.tok_types.iter().any(|kind| *kind == TOK_GNU_ATTRIBUTE),
        "GNU attribute spelling must be promoted before VAST classification"
    );
    assert!(
        fix.tok_types.iter().any(|kind| *kind == TOK_VOLATILE),
        "volatile qualifier spellings must be promoted before VAST classification"
    );
    assert!(
        fix.tok_types.iter().any(|kind| *kind == TOK_ATOMIC),
        "_Atomic qualifier spelling must be promoted before VAST classification"
    );
}

#[test]
fn kernel_style_fixture_preserves_typedef_scope_and_ast_kinds_on_gpu() {
    let fix = fixture_token_stream();
    let raw_cpu = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let raw_gpu = run_gpu_vast_builder(&fix);
    assert_words_eq(
        &raw_gpu,
        &raw_cpu,
        "GPU VAST builder must match CPU on kernel-style parser corpus",
    );

    let annotated_cpu = reference_c11_annotate_typedef_names(&raw_cpu, fix.source.as_bytes());
    let annotated_gpu = run_gpu_typedef_annotation(&fix, &raw_gpu);
    assert_words_eq(
        &annotated_gpu,
        &annotated_cpu,
        "GPU typedef annotation must match CPU on nested shadowing and restoration",
    );

    let typed_cpu = reference_c11_classify_vast_node_kinds(&annotated_cpu);
    let typed_gpu = run_gpu_classifier(&annotated_gpu);
    assert_words_eq(
        &typed_gpu,
        &typed_cpu,
        "GPU classifier must match CPU on kernel-style parser corpus",
    );

    let word_t = lexeme_indices(&fix, "word_t");
    assert_eq!(word_t.len(), 6, "fixture must keep six word_t occurrences");
    assert_flag(
        &annotated_gpu,
        word_t[0],
        TYPEDEF_FLAG_DECL,
        "top-level word_t must be recorded as a typedef declarator",
    );
    assert_flag(
        &annotated_gpu,
        word_t[1],
        TYPEDEF_FLAG_VISIBLE,
        "word_t type use before nested shadow must see the typedef",
    );
    assert_flag(
        &annotated_gpu,
        word_t[2],
        TYPEDEF_FLAG_VISIBLE,
        "sizeof(word_t) must see the typedef name",
    );
    assert_flag(
        &annotated_gpu,
        word_t[3],
        ORDINARY_FLAG_DECL,
        "inner int word_t must be recorded as an ordinary shadowing declarator",
    );
    assert_no_flag(
        &annotated_gpu,
        word_t[4],
        TYPEDEF_FLAG_VISIBLE,
        "word_t expression inside the shadowing block must not resolve to the typedef",
    );
    assert_flag(
        &annotated_gpu,
        word_t[5],
        TYPEDEF_FLAG_VISIBLE,
        "word_t type use after the block must restore the outer typedef binding",
    );

    let probe_cb_t = lexeme_indices(&fix, "probe_cb_t");
    assert_eq!(
        probe_cb_t.len(),
        2,
        "fixture must declare and consume a function pointer typedef"
    );
    assert_flag(
        &annotated_gpu,
        probe_cb_t[0],
        TYPEDEF_FLAG_DECL,
        "probe_cb_t must be recorded as a typedef declarator",
    );
    assert_flag(
        &annotated_gpu,
        probe_cb_t[1],
        TYPEDEF_FLAG_VISIBLE,
        "probe_cb_t parameter type must resolve through typedef annotation",
    );

    assert_eq!(
        row_indices(&typed_gpu, C_AST_KIND_GNU_ATTRIBUTE),
        lexeme_indices(&fix, "__attribute__"),
        "GNU attributes must be first-class AST rows, not calls"
    );
    assert!(
        row_indices(&typed_gpu, C_AST_KIND_POINTER_DECL).len() >= 5,
        "function pointer typedefs and qualified pointer parameters must classify pointer declarators"
    );
    assert!(
        !row_indices(&typed_gpu, C_AST_KIND_FUNCTION_DECLARATOR).is_empty(),
        "function pointer typedef must classify a function-declarator suffix"
    );

    for idx in fix
        .tok_types
        .iter()
        .enumerate()
        .filter_map(|(idx, kind)| (*kind == TOK_SIZEOF).then_some(idx))
    {
        assert_eq!(
            kind_at(&typed_gpu, idx),
            C_AST_KIND_SIZEOF_EXPR,
            "sizeof token at row {idx} must be a sizeof expression"
        );
        assert_ne!(
            kind_at(&typed_gpu, idx + 1),
            C_AST_KIND_CAST_EXPR,
            "sizeof(type) and sizeof(expr) must not classify the following paren as a cast"
        );
    }
    assert!(
        row_indices(&typed_gpu, node_kind::BINARY).contains(&lexeme_indices(&fix, "+")[0]),
        "sizeof(restored + expr_sz) must preserve expression-form binary operator shape"
    );

    assert_eq!(
        row_indices(&typed_gpu, C_AST_KIND_SWITCH_STMT),
        fix.tok_types
            .iter()
            .enumerate()
            .filter_map(|(idx, kind)| (*kind == TOK_SWITCH).then_some(idx))
            .collect::<Vec<_>>(),
        "switch must classify as a switch AST row"
    );
    assert_eq!(
        row_indices(&typed_gpu, C_AST_KIND_CASE_STMT),
        fix.tok_types
            .iter()
            .enumerate()
            .filter_map(|(idx, kind)| (*kind == TOK_CASE).then_some(idx))
            .collect::<Vec<_>>(),
        "case labels must classify as case AST rows"
    );
    assert_eq!(
        row_indices(&typed_gpu, C_AST_KIND_DEFAULT_STMT),
        fix.tok_types
            .iter()
            .enumerate()
            .filter_map(|(idx, kind)| (*kind == TOK_DEFAULT).then_some(idx))
            .collect::<Vec<_>>(),
        "default labels must classify as default AST rows"
    );
    assert_eq!(
        row_indices(&typed_gpu, C_AST_KIND_GOTO_STMT),
        fix.tok_types
            .iter()
            .enumerate()
            .filter_map(|(idx, kind)| (*kind == TOK_GOTO).then_some(idx))
            .collect::<Vec<_>>(),
        "goto must classify as a jump AST row"
    );
    assert_eq!(
        row_indices(&typed_gpu, C_AST_KIND_BREAK_STMT),
        fix.tok_types
            .iter()
            .enumerate()
            .filter_map(|(idx, kind)| (*kind == TOK_BREAK).then_some(idx))
            .collect::<Vec<_>>(),
        "break must classify as a jump AST row"
    );
    assert_eq!(
        row_indices(&typed_gpu, C_AST_KIND_RETURN_STMT),
        fix.tok_types
            .iter()
            .enumerate()
            .filter_map(|(idx, kind)| (*kind == TOK_RETURN).then_some(idx))
            .collect::<Vec<_>>(),
        "return must classify as a jump AST row"
    );

    for label in ["again", "out"] {
        let label_rows = lexeme_indices(&fix, label);
        let definition_row = label_rows
            .iter()
            .copied()
            .find(|idx| fix.tok_types.get(idx + 1) == Some(&TOK_COLON))
            .expect("fixture must contain a label definition");
        assert_eq!(
            kind_at(&typed_gpu, definition_row),
            C_AST_KIND_LABEL_STMT,
            "label definition {label} must classify as a first-class label statement row"
        );
        assert_no_flag(
            &annotated_gpu,
            definition_row,
            ORDINARY_FLAG_DECL,
            "label definition must not become an ordinary declarator",
        );
    }

    assert_eq!(
        kind_at(&typed_gpu, lexeme_indices(&fix, "cb")[1]),
        node_kind::CALL,
        "function pointer call through cb must classify as a call"
    );
    assert_eq!(
        kind_at(&typed_gpu, lexeme_indices(&fix, "READ_ONCE")[0]),
        node_kind::CALL,
        "macro-shaped READ_ONCE invocation must classify as a call-shaped token stream"
    );
    assert_eq!(
        kind_at(&typed_gpu, lexeme_indices(&fix, "fallthrough")[0]),
        TOK_IDENTIFIER,
        "statement-like macro token use without a paren must remain an identifier row"
    );
}
