use super::*;

#[test]
fn host_lexer_preserves_complex_directive_block_as_preproc_rows() {
    let source = b"#ifndef FOO\n#define FOO 1\n#elif defined(BAR)\n#define BAR 2\n#else\n#define BAZ 3\n#endif\nint x;";
    let kinds = lex_c11_max_munch_kinds(source).expect("host lexer must accept");
    let non_ws: Vec<u32> = kinds
        .into_iter()
        .filter(|k| *k != TOK_WHITESPACE && *k != TOK_COMMENT)
        .collect();
    assert_eq!(
        non_ws,
        vec![
            TOK_PREPROC, // #ifndef FOO
            TOK_PREPROC, // #define FOO 1
            TOK_PREPROC, // #elif defined(BAR)
            TOK_PREPROC, // #define BAR 2
            TOK_PREPROC, // #else
            TOK_PREPROC, // #define BAZ 3
            TOK_PREPROC, // #endif
            TOK_INT,
            TOK_IDENTIFIER,
            TOK_SEMICOLON,
        ],
        "every directive line must be a discrete TOK_PREPROC row"
    );
}

#[test]
fn gpu_lexer_preserves_complex_directive_block_as_preproc_rows() {
    let source = b"#ifndef FOO\n#define FOO 1\n#elif defined(BAR)\n#define BAR 2\n#else\n#define BAZ 3\n#endif\nint x;";
    let (tok_types, _, _, count) = run_c11_lexer(source, source.len() as u32);
    assert_eq!(count, 10, "7 directives + int + x + ; = 10 tokens");
    assert_eq!(
        tok_types[0..7],
        [TOK_PREPROC; 7],
        "all directives must be TOK_PREPROC"
    );
    // The raw GPU lexer preserves keyword text as identifier-shaped tokens;
    // keyword promotion is a later parser/semantic pass.
    assert_eq!(tok_types[7], TOK_IDENTIFIER);
    assert_eq!(tok_types[8], TOK_IDENTIFIER);
    assert_eq!(tok_types[9], TOK_SEMICOLON);
}

#[test]
fn directive_rows_survive_cpu_pipeline_without_expansion() {
    let a = assemble(&[
        ("#ifndef FOO", TOK_PREPROC),
        ("\n", TOK_WHITESPACE),
        ("#define FOO 1", TOK_PREPROC),
        ("\n", TOK_WHITESPACE),
        ("int", TOK_INT),
        ("x", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
    ]);
    let out = run_cpu_pipeline(&a);
    for idx in [0usize, 1] {
        assert_eq!(
            word_at(&out.raw_vast, idx * VAST_STRIDE_U32),
            TOK_PREPROC,
            "raw VAST must preserve TOK_PREPROC (no expansion)"
        );
        assert_eq!(row_typed_kind(&out.typed_vast, idx), 0);
        assert_pg_row(&a, &out.pg, &out.typed_vast, idx, 0);
        assert_shape_none(&out.expr_shape, idx);
    }
}

#[test]
fn directive_rows_survive_gpu_pipeline_parity() {
    let a = assemble(&[
        ("#define M(x) x", TOK_PREPROC),
        ("\n", TOK_WHITESPACE),
        ("int", TOK_INT),
        ("z", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
        ("z", TOK_IDENTIFIER),
        ("=", TOK_ASSIGN),
        ("M", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("42", TOK_INTEGER),
        (")", TOK_RPAREN),
        (";", TOK_SEMICOLON),
    ]);
    let cpu = run_cpu_pipeline(&a);
    let gpu_typed = run_gpu_classify(&cpu.raw_vast);
    assert_eq!(
        gpu_typed, cpu.typed_vast,
        "GPU classify must match CPU for directive stream"
    );

    assert_eq!(
        run_gpu_expr_shape(&cpu.raw_vast, &cpu.typed_vast),
        cpu.expr_shape,
        "GPU expression-shape must match CPU"
    );
    assert_eq!(
        run_gpu_pg_lower(&cpu.typed_vast),
        cpu.pg,
        "GPU PG lowering must match CPU"
    );

    let m_idx = find_row_for_lexeme(&a, "M");
    assert_eq!(row_typed_kind(&cpu.typed_vast, m_idx), node_kind::CALL);
}

// ---------------------------------------------------------------------------
// 2. Line continuations
// ---------------------------------------------------------------------------

#[test]
fn host_lexer_splices_backslash_newline_inside_directive() {
    let source = b"#define FOO \\\n42\nx";
    let spliced = c_translation_phase_line_splice(source);
    let kinds = lex_c11_max_munch_kinds(&spliced.bytes).expect("host lexer must accept");
    let non_ws: Vec<u32> = kinds
        .into_iter()
        .filter(|k| *k != TOK_WHITESPACE && *k != TOK_COMMENT)
        .collect();
    // Contract: translation phase 2 removes backslash-newline, yielding one logical line.
    assert_eq!(
        non_ws,
        vec![TOK_PREPROC, TOK_IDENTIFIER],
        "line continuation must splice into a single logical line"
    );
}

#[test]
fn gpu_lexer_splices_backslash_newline_inside_directive() {
    let source = b"#define FOO \\\n42\nx";
    let (tok_types, _, _, count) = run_c11_lexer(source, source.len() as u32);
    assert_eq!(count, 2, "spliced directive + x = 2 tokens");
    assert_eq!(tok_types[0], TOK_PREPROC);
    assert_eq!(tok_types[1], TOK_IDENTIFIER);
}

// ---------------------------------------------------------------------------
// 3. Function-like macros
// ---------------------------------------------------------------------------

#[test]
fn function_like_macro_shape_expansion_preserves_argument_tokens() {
    let mut fixture = MacroFixture::empty();
    // Simulate #define MAX(a,b) ((a) > (b) ? (a) : (b))
    let replacement = &[
        TOK_LPAREN,
        TOK_LPAREN,
        TOK_IDENTIFIER,
        TOK_RPAREN,
        TOK_GT,
        TOK_LPAREN,
        TOK_IDENTIFIER,
        TOK_RPAREN,
        TOK_QUESTION,
        TOK_LPAREN,
        TOK_IDENTIFIER,
        TOK_RPAREN,
        TOK_COLON,
        TOK_LPAREN,
        TOK_IDENTIFIER,
        TOK_RPAREN,
        TOK_RPAREN,
    ];
    fixture.insert(TOK_IDENTIFIER, 512, replacement);
    // Input: MAX ( 1 , 2 )
    let input = [
        TOK_IDENTIFIER,
        TOK_LPAREN,
        TOK_INTEGER,
        TOK_COMMA,
        TOK_INTEGER,
        TOK_RPAREN,
    ];
    let outputs = run_dynamic_macro_expansion(&input, &fixture, 32)
        .expect("function-like macro expansion must succeed");
    let out_tokens = decode_u32_words(&outputs[0].to_bytes());
    let out_count = decode_u32_words(&outputs[1].to_bytes());
    let expected_count = replacement.len() + input.len() - 1;
    assert_eq!(out_count[0], expected_count as u32);
    assert_eq!(&out_tokens[..replacement.len()], replacement);
    assert_eq!(&out_tokens[replacement.len()..expected_count], &input[1..]);
}

#[test]
fn function_like_macro_with_empty_args_expands_shape() {
    let mut fixture = MacroFixture::empty();
    // #define LOCK() acquire()
    fixture.insert(
        TOK_IDENTIFIER,
        512,
        &[TOK_IDENTIFIER, TOK_LPAREN, TOK_RPAREN],
    );
    let input = [TOK_IDENTIFIER, TOK_LPAREN, TOK_RPAREN];
    let outputs = run_dynamic_macro_expansion(&input, &fixture, 8)
        .expect("empty-arg macro expansion must succeed");
    let out_tokens = decode_u32_words(&outputs[0].to_bytes());
    let out_count = decode_u32_words(&outputs[1].to_bytes());
    // The engine is a raw token substitutor; it does not consume the () tokens.
    assert_eq!(out_count, vec![5]);
    assert_eq!(
        &out_tokens[..5],
        &[
            TOK_IDENTIFIER,
            TOK_LPAREN,
            TOK_RPAREN,
            TOK_LPAREN,
            TOK_RPAREN
        ]
    );
}

// ---------------------------------------------------------------------------
// 4. Nested macro calls
// ---------------------------------------------------------------------------

#[test]
fn nested_macro_names_in_replacement_are_not_recursively_expanded_single_pass() {
    let mut fixture = MacroFixture::empty();
    // OUTER -> [INNER, TOK_INTEGER] where INNER is TOK_STAR
    fixture.insert(TOK_IDENTIFIER, 512, &[TOK_STAR, TOK_INTEGER]);
    // INNER (TOK_STAR) -> [TOK_PLUS, TOK_PLUS] (must NOT be invoked)
    fixture.insert(TOK_STAR, 514, &[TOK_PLUS, TOK_PLUS]);
    let outputs = run_dynamic_macro_expansion(&[TOK_IDENTIFIER], &fixture, 8)
        .expect("expansion must succeed");
    let out_tokens = decode_u32_words(&outputs[0].to_bytes());
    let out_count = decode_u32_words(&outputs[1].to_bytes());
    assert_eq!(&out_tokens[..2], &[TOK_STAR, TOK_INTEGER]);
    assert_eq!(out_count, vec![2]);
}

#[test]
fn macro_replacement_tokens_are_not_expanded_but_later_occurrences_are() {
    let mut fixture = MacroFixture::empty();
    // OUTER -> [INNER, TOK_INTEGER] where INNER is TOK_STAR
    fixture.insert(TOK_IDENTIFIER, 512, &[TOK_STAR, TOK_INTEGER]);
    // INNER (TOK_STAR) -> [TOK_PLUS, TOK_PLUS]
    fixture.insert(TOK_STAR, 514, &[TOK_PLUS, TOK_PLUS]);
    // Input: OUTER INNER
    let input = [TOK_IDENTIFIER, TOK_STAR];
    let outputs = run_dynamic_macro_expansion(&input, &fixture, 8).expect("expansion must succeed");
    let out_tokens = decode_u32_words(&outputs[0].to_bytes());
    let out_count = decode_u32_words(&outputs[1].to_bytes());
    // Expected: [TOK_STAR, TOK_INTEGER, TOK_PLUS, TOK_PLUS]
    assert_eq!(out_count, vec![4]);
    assert_eq!(
        &out_tokens[..4],
        &[TOK_STAR, TOK_INTEGER, TOK_PLUS, TOK_PLUS]
    );
}

#[test]
fn nested_macro_call_shapes_survive_as_calls_in_typed_vast() {
    let a = assemble(&[
        ("int", TOK_INT),
        ("y", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
        ("y", TOK_IDENTIFIER),
        ("=", TOK_ASSIGN),
        ("OUTER", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("INNER", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("1", TOK_INTEGER),
        (",", TOK_COMMA),
        ("2", TOK_INTEGER),
        (")", TOK_RPAREN),
        (")", TOK_RPAREN),
        (";", TOK_SEMICOLON),
    ]);
    let out = run_cpu_pipeline(&a);
    let outer_idx = find_row_for_lexeme(&a, "OUTER");
    let inner_idx = find_row_for_lexeme(&a, "INNER");
    assert_eq!(
        row_typed_kind(&out.typed_vast, outer_idx),
        node_kind::CALL,
        "OUTER must classify as CALL"
    );
    assert_eq!(
        row_typed_kind(&out.typed_vast, inner_idx),
        node_kind::CALL,
        "INNER must classify as CALL"
    );
}

// ---------------------------------------------------------------------------
// 5. Token pasting
// ---------------------------------------------------------------------------

#[test]
fn token_paste_outside_directive_is_hashhash_operator_host() {
    let kinds = lex_c11_max_munch_kinds(b"a ## b").expect("host lexer must accept");
    let non_ws: Vec<u32> = kinds
        .into_iter()
        .filter(|k| *k != TOK_WHITESPACE && *k != TOK_COMMENT)
        .collect();
    assert_eq!(
        non_ws,
        vec![TOK_IDENTIFIER, TOK_HASHHASH, TOK_IDENTIFIER],
        "## outside directive must be one TOK_HASHHASH operator"
    );
}

#[test]
fn token_paste_outside_directive_is_hashhash_operator_gpu() {
    let source = b"a ## b";
    let (tok_types, _, _, count) = run_c11_lexer(source, source.len() as u32);
    assert_eq!(count, 3);
    assert_eq!(
        tok_types,
        vec![TOK_IDENTIFIER, TOK_HASHHASH, TOK_IDENTIFIER]
    );
}

#[test]
fn double_hash_inside_define_directive_stays_inside_preproc_row() {
    let source = b"#define CAT(a,b) a ## b\nx";
    let (tok_types, _, _, count) = run_c11_lexer(source, source.len() as u32);
    assert_eq!(count, 2, "directive line + identifier = 2 tokens");
    assert_eq!(tok_types[0], TOK_PREPROC);
    assert_eq!(tok_types[1], TOK_IDENTIFIER);
}

#[test]
fn token_paste_in_macro_replacement_is_emitted_as_hashhash_token() {
    let mut fixture = MacroFixture::empty();
    // #define CAT(a,b) a ## b
    fixture.insert(
        TOK_IDENTIFIER,
        512,
        &[TOK_IDENTIFIER, TOK_HASHHASH, TOK_IDENTIFIER],
    );
    let input = [TOK_IDENTIFIER];
    let outputs = run_dynamic_macro_expansion(&input, &fixture, 8).expect("expansion must succeed");
    let out_tokens = decode_u32_words(&outputs[0].to_bytes());
    let out_count = decode_u32_words(&outputs[1].to_bytes());
    assert_eq!(out_count, vec![3]);
    assert_eq!(
        &out_tokens[..3],
        &[TOK_IDENTIFIER, TOK_HASHHASH, TOK_IDENTIFIER]
    );
}

// ---------------------------------------------------------------------------
// 6. Stringification
// ---------------------------------------------------------------------------

#[test]
fn stringize_outside_directive_is_hash_operator_host() {
    let kinds = lex_c11_max_munch_kinds(b"a # b").expect("host lexer must accept");
    let non_ws: Vec<u32> = kinds
        .into_iter()
        .filter(|k| *k != TOK_WHITESPACE && *k != TOK_COMMENT)
        .collect();
    assert_eq!(
        non_ws,
        vec![TOK_IDENTIFIER, TOK_HASH, TOK_IDENTIFIER],
        "mid-line # must be TOK_HASH operator"
    );
}

#[test]
fn stringize_outside_directive_is_hash_operator_gpu() {
    let source = b"a # b";
    let (tok_types, _, _, count) = run_c11_lexer(source, source.len() as u32);
    assert_eq!(count, 3);
    assert_eq!(tok_types, vec![TOK_IDENTIFIER, TOK_HASH, TOK_IDENTIFIER]);
}

#[test]
fn hash_inside_define_directive_stays_inside_preproc_row() {
    let source = b"#define STR(x) #x\ny";
    let (tok_types, _, _, count) = run_c11_lexer(source, source.len() as u32);
    assert_eq!(count, 2, "directive line + identifier = 2 tokens");
    assert_eq!(tok_types[0], TOK_PREPROC);
    assert_eq!(tok_types[1], TOK_IDENTIFIER);
}

#[test]
fn stringize_in_macro_replacement_is_emitted_as_hash_token() {
    let mut fixture = MacroFixture::empty();
    // #define STR(x) #x
    fixture.insert(TOK_IDENTIFIER, 512, &[TOK_HASH, TOK_IDENTIFIER]);
    let input = [TOK_IDENTIFIER];
    let outputs = run_dynamic_macro_expansion(&input, &fixture, 8).expect("expansion must succeed");
    let out_tokens = decode_u32_words(&outputs[0].to_bytes());
    let out_count = decode_u32_words(&outputs[1].to_bytes());
    assert_eq!(out_count, vec![2]);
    assert_eq!(&out_tokens[..2], &[TOK_HASH, TOK_IDENTIFIER]);
}

// ---------------------------------------------------------------------------
// 7. Variadic trailing comma behavior
// ---------------------------------------------------------------------------

#[test]
fn variadic_macro_definition_lexes_as_single_preproc_row() {
    let source = b"#define LOG(fmt, ...) fprintf(stderr, fmt, __VA_ARGS__)";
    let kinds = lex_c11_max_munch_kinds(source).expect("host lexer must accept");
    let non_ws: Vec<u32> = kinds
        .into_iter()
        .filter(|k| *k != TOK_WHITESPACE && *k != TOK_COMMENT)
        .collect();
    assert_eq!(
        non_ws,
        vec![TOK_PREPROC],
        "variadic macro definition must be one preproc row"
    );
}

#[test]
fn variadic_macro_with_trailing_comma_in_params_lexes_as_preproc() {
    let source = b"#define FOO(a, b, ...) a + b";
    let kinds = lex_c11_max_munch_kinds(source).expect("host lexer must accept");
    let non_ws: Vec<u32> = kinds
        .into_iter()
        .filter(|k| *k != TOK_WHITESPACE && *k != TOK_COMMENT)
        .collect();
    assert_eq!(non_ws, vec![TOK_PREPROC]);
}
