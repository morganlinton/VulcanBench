use super::*;

#[test]
fn object_like_macro_replaces_identifier_with_token_sequence() {
    let mut fixture = MacroFixture::empty();
    fixture.insert(TOK_IDENTIFIER, 512, &[TOK_INTEGER, TOK_PLUS, TOK_INTEGER]);

    let outputs = run_dynamic_macro_expansion(&[TOK_IDENTIFIER], &fixture, 8)
        .expect("object-like macro expansion must succeed");
    let out_tokens = decode_u32_words(&outputs[0].to_bytes());
    let out_count = decode_u32_words(&outputs[1].to_bytes());
    assert_eq!(&out_tokens[..3], &[TOK_INTEGER, TOK_PLUS, TOK_INTEGER]);
    assert_eq!(out_count, vec![3]);
}

#[test]
fn multiple_object_like_macros_expand_in_same_stream() {
    let mut fixture = MacroFixture::empty();
    fixture.insert(TOK_IDENTIFIER, 512, &[TOK_INTEGER, TOK_STAR]);
    fixture.insert(TOK_STAR, 514, &[TOK_PLUS, TOK_PLUS]);

    let outputs =
        run_dynamic_macro_expansion(&[TOK_IDENTIFIER, TOK_STAR, TOK_INTEGER], &fixture, 8)
            .expect("multi-macro expansion must succeed");
    let out_tokens = decode_u32_words(&outputs[0].to_bytes());
    let out_count = decode_u32_words(&outputs[1].to_bytes());
    assert_eq!(
        &out_tokens[..5],
        &[TOK_INTEGER, TOK_STAR, TOK_PLUS, TOK_PLUS, TOK_INTEGER]
    );
    assert_eq!(out_count, vec![5]);
}

// ---------------------------------------------------------------------------
// 2. Nested / function-like macro shapes
// ---------------------------------------------------------------------------

#[test]
fn function_like_macro_shape_emits_replacement_then_argument_tokens() {
    // This primitive runs after macro-definition parsing has already converted
    // each macro body to replacement-token rows; argument substitution belongs
    // to the host macro parser contracts in `vyre-frontend-c/tests/tu_host_preprocessor.rs`.
    // Here we assert the lower-level replacement splice stays source-ordered.
    let mut fixture = MacroFixture::empty();
    fixture.insert(
        TOK_IDENTIFIER,
        512,
        &[
            TOK_IDENTIFIER, // a
            TOK_GT,
            TOK_IDENTIFIER, // b
            TOK_QUESTION,
            TOK_IDENTIFIER, // a
            TOK_COLON,
            TOK_IDENTIFIER, // b
        ],
    );

    // Input: MAX ( 1 , 2 )
    let input = [
        TOK_IDENTIFIER,
        TOK_LPAREN,
        TOK_INTEGER,
        TOK_COMMA,
        TOK_INTEGER,
        TOK_RPAREN,
    ];
    let outputs = run_dynamic_macro_expansion(&input, &fixture, 16)
        .expect("function-like shape expansion must succeed");
    let out_tokens = decode_u32_words(&outputs[0].to_bytes());
    let out_count = decode_u32_words(&outputs[1].to_bytes());

    // MAX expands to 7 tokens; the surrounding () , tokens stay.
    assert_eq!(out_count, vec![12]);
    assert_eq!(
        &out_tokens[..7],
        &[
            TOK_IDENTIFIER,
            TOK_GT,
            TOK_IDENTIFIER,
            TOK_QUESTION,
            TOK_IDENTIFIER,
            TOK_COLON,
            TOK_IDENTIFIER,
        ]
    );
    // The original argument tokens follow unchanged.
    assert_eq!(
        &out_tokens[7..12],
        &[TOK_LPAREN, TOK_INTEGER, TOK_COMMA, TOK_INTEGER, TOK_RPAREN,]
    );
}

#[test]
fn nested_macro_names_in_replacement_are_not_recursively_expanded() {
    // The engine is single-pass. If OUTER expands to a sequence containing
    // INNER, INNER is emitted as a raw token, not expanded again.
    let mut fixture = MacroFixture::empty();
    fixture.insert(TOK_IDENTIFIER, 512, &[TOK_STAR, TOK_INTEGER]);
    // Use TOK_STAR as the "inner" macro name in the replacement of OUTER.
    // We deliberately map TOK_STAR → [TOK_PLUS] so we can prove it is NOT
    // invoked when it appears inside OUTER's replacement.
    fixture.insert(TOK_STAR, 514, &[TOK_PLUS]);

    // Input: OUTER (represented by TOK_IDENTIFIER)
    let outputs = run_dynamic_macro_expansion(&[TOK_IDENTIFIER], &fixture, 8)
        .expect("nested macro expansion must succeed");
    let out_tokens = decode_u32_words(&outputs[0].to_bytes());
    let out_count = decode_u32_words(&outputs[1].to_bytes());

    // Should be [TOK_STAR, TOK_INTEGER], not [TOK_PLUS, TOK_INTEGER]
    assert_eq!(&out_tokens[..2], &[TOK_STAR, TOK_INTEGER]);
    assert_eq!(out_count, vec![2]);
}

// ---------------------------------------------------------------------------
// 3. Token paste (##)
// ---------------------------------------------------------------------------

#[test]
fn token_paste_outside_directive_lexes_as_hashhash_operator_host_lexer() {
    let kinds = lex_c11_max_munch_kinds(b"a ## b").expect("host lexer must accept");
    let non_ws: Vec<u32> = kinds
        .into_iter()
        .filter(|k| *k != TOK_WHITESPACE && *k != TOK_COMMENT)
        .collect();
    assert_eq!(
        non_ws,
        vec![TOK_IDENTIFIER, TOK_HASHHASH, TOK_IDENTIFIER],
        "host lexer: ## outside directive must be one TOK_HASHHASH operator"
    );
}

#[test]
fn token_paste_outside_directive_gpu_lexer_is_hash_hash_operator() {
    let source = b"a ## b";
    let (tok_types, _, _, count) = run_c11_lexer(source, source.len() as u32);
    assert_eq!(
        count, 3,
        "GPU lexer: a ## b emits three non-whitespace tokens"
    );
    assert_eq!(
        tok_types,
        vec![TOK_IDENTIFIER, TOK_HASHHASH, TOK_IDENTIFIER],
        "GPU lexer: ## outside directive must be one TOK_HASHHASH operator"
    );
}

#[test]
fn double_hash_inside_define_directive_stays_inside_preproc_row() {
    let source = b"#define CAT(a,b) a ## b\nx";
    let (tok_types, _, _, count) = run_c11_lexer(source, source.len() as u32);
    assert_eq!(count, 2, "directive line + identifier must be two tokens");
    assert_eq!(
        tok_types[0], TOK_PREPROC,
        "entire #define line must be TOK_PREPROC"
    );
    assert_eq!(
        tok_types[1], TOK_IDENTIFIER,
        "trailing x must be TOK_IDENTIFIER"
    );
}

// ---------------------------------------------------------------------------
// 4. Stringize (#)
// ---------------------------------------------------------------------------

#[test]
fn stringize_hash_outside_directive_host_lexer_is_hash_operator() {
    let kinds = lex_c11_max_munch_kinds(b"a # b").expect("host lexer must accept");
    let non_ws: Vec<u32> = kinds
        .into_iter()
        .filter(|k| *k != TOK_WHITESPACE && *k != TOK_COMMENT)
        .collect();
    assert_eq!(
        non_ws,
        vec![TOK_IDENTIFIER, TOK_HASH, TOK_IDENTIFIER],
        "host lexer: mid-line # must be TOK_HASH operator"
    );
}

#[test]
fn stringize_hash_outside_directive_gpu_lexer_is_hash_operator() {
    let source = b"a # b";
    let (tok_types, _, _, count) = run_c11_lexer(source, source.len() as u32);
    assert_eq!(
        count, 3,
        "GPU lexer: a # b emits three non-whitespace tokens"
    );
    assert_eq!(
        tok_types,
        vec![TOK_IDENTIFIER, TOK_HASH, TOK_IDENTIFIER],
        "GPU lexer: mid-line # must be TOK_HASH"
    );
}

#[test]
fn hash_inside_define_directive_stays_inside_preproc_row() {
    let source = b"#define STR(x) #x\ny";
    let (tok_types, _, _, count) = run_c11_lexer(source, source.len() as u32);
    assert_eq!(count, 2, "directive line + identifier must be two tokens");
    assert_eq!(
        tok_types[0], TOK_PREPROC,
        "entire #define line must be TOK_PREPROC"
    );
    assert_eq!(
        tok_types[1], TOK_IDENTIFIER,
        "trailing y must be TOK_IDENTIFIER"
    );
}

// ---------------------------------------------------------------------------
// 5. Escaped newlines
// ---------------------------------------------------------------------------

#[test]
fn translation_phase_line_splice_deletes_lf_crlf_and_maps_offsets() {
    let source = b"in\\\nt x\\\r\n;\n";
    let spliced = c_translation_phase_line_splice(source);

    assert_eq!(spliced.bytes, b"int x;\n");
    assert_eq!(spliced.original_offsets.len(), spliced.bytes.len() + 1);
    assert_eq!(spliced.original_offset(0), 0, "i maps to original byte 0");
    assert_eq!(spliced.original_offset(2), 4, "t maps past the LF splice");
    assert_eq!(
        spliced.original_offset(5),
        10,
        "semicolon maps past the CRLF splice"
    );
    assert_eq!(
        spliced.original_offset(spliced.bytes.len()),
        source.len(),
        "final boundary maps to source length"
    );
}

#[test]
fn backslash_before_newline_splices_preproc_logical_row() {
    let source = b"#define FOO \\\n1\nx";
    let (tok_types, tok_starts, tok_lens, count) = run_c11_lexer(source, source.len() as u32);

    assert_eq!(count, 2, "spliced directive + trailing identifier");
    assert_eq!(tok_types[0], TOK_PREPROC);
    assert_eq!(tok_types[1], TOK_IDENTIFIER);

    let preproc_end = tok_starts[0] + tok_lens[0];
    let second_newline_pos = source
        .iter()
        .enumerate()
        .filter_map(|(idx, byte)| (*byte == b'\n').then_some(idx))
        .nth(1)
        .unwrap();
    assert_eq!(
        preproc_end as usize, second_newline_pos,
        "TOK_PREPROC must end at the first non-spliced newline"
    );
}

#[test]
fn directive_metadata_uses_phase2_spliced_keyword_and_ifdef_payload() {
    let source = b"#ifd\\\nef FO\\\nO\nx\n#endif\n";
    let (tok_types, tok_starts, tok_lens, count) = run_c11_lexer(source, source.len() as u32);
    assert_eq!(count, 3, "ifdef row + x + endif row");

    let (directive_kinds, directive_values) = reference_c_preprocessor_directive_metadata(
        &tok_types,
        &tok_starts,
        &tok_lens,
        source,
        &[b"FOO".as_slice()],
    )
    .expect("phase-2 spliced directive metadata must classify");

    assert_eq!(directive_kinds, vec![TOK_PP_IFDEF, 0, TOK_PP_ENDIF]);
    assert_eq!(directive_values, vec![1, 0, 0]);
}

#[test]
fn directive_metadata_treats_comments_as_preprocessor_whitespace() {
    let source = b"# /*lead*/ ifdef /*payload*/ FOO\nx\n#endif\n";
    let (tok_types, tok_starts, tok_lens, count) = run_c11_lexer(source, source.len() as u32);
    assert_eq!(count, 3, "ifdef row + x + endif row");

    let (directive_kinds, directive_values) = reference_c_preprocessor_directive_metadata(
        &tok_types,
        &tok_starts,
        &tok_lens,
        source,
        &[b"FOO".as_slice()],
    )
    .expect("comments in directive whitespace must classify");

    assert_eq!(directive_kinds, vec![TOK_PP_IFDEF, 0, TOK_PP_ENDIF]);
    assert_eq!(directive_values, vec![1, 0, 0]);
}

#[test]
fn directive_metadata_evaluates_linux_grade_if_arithmetic() {
    let source = b"#if ((1u << 12) == 4096) && ('\\n' == 10) && ((0x10 + 010) == 24)\nx\n#endif\n";
    let (tok_types, tok_starts, tok_lens, count) = run_c11_lexer(source, source.len() as u32);
    assert_eq!(count, 3, "#if row + x + #endif row");

    let (directive_kinds, directive_values) = reference_c_preprocessor_directive_metadata(
        &tok_types,
        &tok_starts,
        &tok_lens,
        source,
        &[],
    )
    .expect("arithmetic #if directive metadata must classify");

    assert_eq!(directive_kinds, vec![TOK_PP_IF, 0, TOK_PP_ENDIF]);
    assert_eq!(
        directive_values,
        vec![1, 0, 0],
        "shift, equality, character constants, hex, and octal literals must evaluate"
    );
}

#[test]
fn directive_metadata_accepts_gnu_system_directives_and_ternary_if() {
    let source = b"#include_next <linux/compiler.h>\n#warning keep this diagnostic\n#ident \"kernel\"\n#sccs \"@(#)\"\n#if defined(ENABLED) ? (L'\\n' == 10) : 0\nx\n#endif\n";
    let (tok_types, tok_starts, tok_lens, count) = run_c11_lexer(source, source.len() as u32);
    assert_eq!(count, 7, "four GNU/system directives + #if + body + #endif");

    let (directive_kinds, directive_values) = reference_c_preprocessor_directive_metadata(
        &tok_types,
        &tok_starts,
        &tok_lens,
        source,
        &[b"ENABLED".as_slice()],
    )
    .expect("GNU/system directives and ternary #if metadata must classify");

    assert_eq!(
        directive_kinds,
        vec![
            TOK_PP_INCLUDE_NEXT,
            TOK_PP_WARNING,
            TOK_PP_IDENT,
            TOK_PP_SCCS,
            TOK_PP_IF,
            0,
            TOK_PP_ENDIF
        ]
    );
    assert_eq!(
        directive_values,
        vec![0, 0, 0, 0, 1, 0, 0],
        "defined() ternary branch and prefixed character constant must evaluate"
    );
}

#[test]
fn gpu_lexer_recognizes_linux_compound_assignment_and_float_pp_numbers() {
    let source = b"x >>= 1; y %= .5e+2; z ## w; q <<= 0x1p-2;";
    let (tok_types, _, _, count) = run_c11_lexer(source, source.len() as u32);
    assert_eq!(count, 16, "all non-whitespace tokens must be emitted");
    assert_eq!(tok_types[1], TOK_RSHIFT_EQ, "`>>=` must be one token");
    assert_eq!(tok_types[5], TOK_PERCENT_EQ, "`%=` must be one token");
    assert_eq!(tok_types[6], TOK_FLOAT, "`.5e+2` must lex as a float token");
    assert_eq!(tok_types[9], TOK_HASHHASH, "`##` must be one token");
    assert_eq!(tok_types[13], TOK_LSHIFT_EQ, "`<<=` must be one token");
    assert_eq!(
        tok_types[14], TOK_FLOAT,
        "`0x1p-2` must lex as a float token"
    );
}

#[test]
fn directive_metadata_rejects_divide_by_zero_if_expression() {
    let source = b"#if 4 / 0\nx\n#endif\n";
    let (tok_types, tok_starts, tok_lens, count) = run_c11_lexer(source, source.len() as u32);
    assert_eq!(count, 3, "#if row + x + #endif row");

    let err = reference_c_preprocessor_directive_metadata(
        &tok_types,
        &tok_starts,
        &tok_lens,
        source,
        &[],
    )
    .expect_err("divide-by-zero #if must fail loudly");

    assert_eq!(err.message, "Fix: #if expression divides by zero");
}

#[test]
fn directive_metadata_rejects_pre_splice_preproc_span() {
    let source = b"#define FOO \\\n1\nx";
    let first_newline = source.iter().position(|byte| *byte == b'\n').unwrap();
    let err = reference_c_preprocessor_directive_metadata(
        &[TOK_PREPROC],
        &[0],
        &[first_newline as u32],
        source,
        &[],
    )
    .expect_err("pre-splice token streams must fail loudly");

    assert_eq!(
        err.message,
        "Fix: TOK_PREPROC span must include the full phase-2 spliced directive row"
    );
}

// ---------------------------------------------------------------------------
// 6. Directive-position hash vs operator hash
// ---------------------------------------------------------------------------

