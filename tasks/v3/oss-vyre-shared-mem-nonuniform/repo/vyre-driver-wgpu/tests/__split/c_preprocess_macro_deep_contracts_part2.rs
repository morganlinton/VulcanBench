use super::*;

#[test]
fn macro_call_with_trailing_comma_survives_as_call_in_vast() {
    let a = assemble(&[
        ("int", TOK_INT),
        ("z", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
        ("z", TOK_IDENTIFIER),
        ("=", TOK_ASSIGN),
        ("FOO", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("a", TOK_IDENTIFIER),
        (",", TOK_COMMA),
        (")", TOK_RPAREN),
        (";", TOK_SEMICOLON),
    ]);
    let out = run_cpu_pipeline(&a);
    let foo_idx = find_row_for_lexeme(&a, "FOO");
    assert_eq!(
        row_typed_kind(&out.typed_vast, foo_idx),
        node_kind::CALL,
        "macro call with trailing comma must stay CALL"
    );
}

// ---------------------------------------------------------------------------
// 8. Conditional directives as token streams
// ---------------------------------------------------------------------------

#[test]
fn conditional_directives_survive_cpu_pipeline_unexpanded() {
    let a = assemble(&[
        ("#if defined(X)", TOK_PREPROC),
        ("\n", TOK_WHITESPACE),
        ("#elif 1", TOK_PREPROC),
        ("\n", TOK_WHITESPACE),
        ("#else", TOK_PREPROC),
        ("\n", TOK_WHITESPACE),
        ("#endif", TOK_PREPROC),
        ("\n", TOK_WHITESPACE),
        ("int", TOK_INT),
        ("w", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
    ]);
    let out = run_cpu_pipeline(&a);
    for idx in 0..4 {
        assert_eq!(
            word_at(&out.raw_vast, idx * VAST_STRIDE_U32),
            TOK_PREPROC,
            "raw VAST conditional {idx}"
        );
        assert_eq!(
            row_typed_kind(&out.typed_vast, idx),
            0,
            "typed VAST conditional {idx}"
        );
        assert_pg_row(&a, &out.pg, &out.typed_vast, idx, 0);
        assert_shape_none(&out.expr_shape, idx);
    }
}

#[test]
fn conditional_directives_survive_gpu_pipeline_parity() {
    let a = assemble(&[
        ("#ifdef FOO", TOK_PREPROC),
        ("\n", TOK_WHITESPACE),
        ("#else", TOK_PREPROC),
        ("\n", TOK_WHITESPACE),
        ("#endif", TOK_PREPROC),
        ("\n", TOK_WHITESPACE),
        ("float", TOK_FLOAT_KW),
        ("f", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
    ]);
    let cpu = run_cpu_pipeline(&a);
    let gpu_typed = run_gpu_classify(&cpu.raw_vast);
    assert_eq!(
        gpu_typed, cpu.typed_vast,
        "GPU classify must match CPU for conditional block"
    );

    assert_eq!(
        run_gpu_expr_shape(&cpu.raw_vast, &cpu.typed_vast),
        cpu.expr_shape,
        "GPU expr-shape must match CPU"
    );
    assert_eq!(
        run_gpu_pg_lower(&cpu.typed_vast),
        cpu.pg,
        "GPU PG lower must match CPU"
    );
}

#[test]
fn conditional_mask_hides_dead_tokens_contract() {
    let source = b"#if 0\nx\n#endif\n";
    let (tok_types, tok_starts, tok_lens, _) = run_c11_lexer(source, source.len() as u32);
    let (directive_kinds, directive_values) = reference_c_preprocessor_directive_metadata(
        &tok_types,
        &tok_starts,
        &tok_lens,
        source,
        &[],
    )
    .expect("directive metadata must classify #if/#endif rows");
    assert_eq!(directive_kinds, vec![TOK_PP_IF, 0, TOK_PP_ENDIF]);
    assert_eq!(directive_values, vec![0, 0, 0]);

    let outputs =
        run_conditional_mask_with_directives(&tok_types, &directive_kinds, &directive_values)
            .expect("conditional mask must succeed");
    let mask = decode_u32_words(&outputs[0].to_bytes());
    assert_eq!(
        &mask[..tok_types.len()],
        &[1, 0, 1],
        "token inside false #if branch must be masked out"
    );
}

#[test]
fn conditional_mask_selects_elif_else_branch_from_directive_payload_values() {
    let source = b"#if defined(MISSING)\na\n#elif defined(HIT)\nb\n#else\nc\n#endif\n";
    let (tok_types, tok_starts, tok_lens, _) = run_c11_lexer(source, source.len() as u32);
    let (directive_kinds, directive_values) = reference_c_preprocessor_directive_metadata(
        &tok_types,
        &tok_starts,
        &tok_lens,
        source,
        &[b"HIT".as_slice()],
    )
    .expect("directive metadata must evaluate defined() payloads");
    assert_eq!(
        directive_kinds,
        vec![TOK_PP_IF, 0, TOK_PP_ELIF, 0, TOK_PP_ELSE, 0, TOK_PP_ENDIF]
    );
    assert_eq!(directive_values, vec![0, 0, 1, 0, 0, 0, 0]);

    let outputs =
        run_conditional_mask_with_directives(&tok_types, &directive_kinds, &directive_values)
            .expect("conditional mask must succeed");
    let mask = decode_u32_words(&outputs[0].to_bytes());
    assert_eq!(
        &mask[..tok_types.len()],
        &[1, 0, 1, 1, 1, 0, 1],
        "#elif true branch must stay live while #if false and #else bodies are masked"
    );
}

// ---------------------------------------------------------------------------
// 9. Malformed directives fail-loud behavior
// ---------------------------------------------------------------------------

#[test]
fn dynamic_macro_expansion_accepts_zero_length_input_as_noop() {
    let fixture = MacroFixture::empty();
    let outputs = run_dynamic_macro_expansion(&[], &fixture, 8)
        .expect("zero-length macro expansion must construct a valid no-op program");
    assert_eq!(
        decode_u32_words(&outputs[1].to_bytes())[0],
        0,
        "zero-length macro expansion must emit zero output tokens"
    );
}

#[test]
fn malformed_directive_lines_lex_as_preproc_rows_host() {
    let source = b"#\n#123\n# foo bar\n";
    let kinds = lex_c11_max_munch_kinds(source).expect("host lexer must accept");
    let non_ws: Vec<u32> = kinds
        .into_iter()
        .filter(|k| *k != TOK_WHITESPACE && *k != TOK_COMMENT)
        .collect();
    assert_eq!(
        non_ws,
        vec![TOK_PREPROC, TOK_PREPROC, TOK_PREPROC],
        "malformed directive lines must still lex as preproc rows"
    );
}

#[test]
fn malformed_directive_lines_lex_as_preproc_rows_gpu() {
    let source = b"#\n#123\n# foo bar\n";
    let (tok_types, _, _, count) = run_c11_lexer(source, source.len() as u32);
    assert_eq!(count, 3, "three malformed directives = 3 tokens");
    assert!(
        tok_types.iter().all(|&t| t == TOK_PREPROC),
        "all malformed directives must be TOK_PREPROC"
    );
}

// ---------------------------------------------------------------------------
// 10. Span preservation
// ---------------------------------------------------------------------------

#[test]
fn directive_span_preserved_through_gpu_lexer() {
    let source = b"#define SPAN 42\nint a;";
    let (tok_types, tok_starts, tok_lens, count) = run_c11_lexer(source, source.len() as u32);
    assert_eq!(count, 4);
    assert_eq!(tok_types[0], TOK_PREPROC);
    assert_eq!(tok_starts[0], 0);
    // "#define SPAN 42" is 15 characters.
    assert_eq!(
        tok_lens[0], 15,
        "TOK_PREPROC length must match directive text"
    );
    assert_eq!(tok_starts[1], 16, "int starts after newline");
    assert_eq!(tok_lens[1], 3);
    assert_eq!(tok_starts[2], 20);
    assert_eq!(tok_lens[2], 1);
    assert_eq!(tok_starts[3], 21);
    assert_eq!(tok_lens[3], 1);
}

#[test]
fn directive_span_preserved_through_pg_lowering() {
    let a = assemble(&[
        ("#pragma once", TOK_PREPROC),
        ("\n", TOK_WHITESPACE),
        ("double", TOK_DOUBLE),
        ("d", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
    ]);
    let out = run_cpu_pipeline(&a);
    assert_pg_row(&a, &out.pg, &out.typed_vast, 0, 0);
    let pg_start = word_at(&out.pg, 0 * PG_STRIDE_U32 + 1);
    let pg_end = word_at(&out.pg, 0 * PG_STRIDE_U32 + 2);
    assert_eq!(
        pg_start, a.tok_starts[0],
        "PG span start must match lexer start"
    );
    assert_eq!(
        pg_end,
        a.tok_starts[0] + a.tok_lens[0],
        "PG span end must match lexer end"
    );
}
