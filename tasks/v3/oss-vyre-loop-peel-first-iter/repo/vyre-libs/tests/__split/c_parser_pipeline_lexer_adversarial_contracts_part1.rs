use super::*;

#[test]
fn empty_string_literal_emits_one_string_token_with_len_two() {
    let source = b"\"\"";
    let (types, starts, lens, count) = run_gpu_lexer(source, source.len() as u32);
    assert_eq!(count, 1, "empty string must produce exactly one token");
    assert_eq!(types[0], TOK_STRING, "empty string must type as TOK_STRING");
    assert_eq!(starts[0], 0, "span start");
    assert_eq!(lens[0], 2, "span length");
}

#[test]
fn string_with_escaped_quote_preserves_correct_span() {
    let source = b"\"a\\\"b\"";
    let (types, starts, lens, count) = run_gpu_lexer(source, source.len() as u32);
    assert_eq!(count, 1);
    assert_eq!(types[0], TOK_STRING);
    assert_eq!(starts[0], 0);
    assert_eq!(lens[0], source.len() as u32);
}

#[test]
fn string_with_directive_like_content_is_not_preproc() {
    let source = b"\"#define FOO 1\"";
    let (types, _, _, count) = run_gpu_lexer(source, source.len() as u32);
    assert_eq!(count, 1);
    assert_eq!(
        types[0], TOK_STRING,
        "directive-like text inside string must stay TOK_STRING"
    );
}

#[test]
fn string_with_comment_like_content_is_not_comment() {
    let source = b"\"/* not a comment */\"";
    let (types, _, _, count) = run_gpu_lexer(source, source.len() as u32);
    assert_eq!(count, 1);
    assert_eq!(
        types[0], TOK_STRING,
        "comment-like text inside string must stay TOK_STRING"
    );
}

#[test]
fn long_string_literal_has_accurate_span() {
    let payload = "x".repeat(200);
    let source = format!("\"{}\"", payload);
    let bytes = source.as_bytes();
    let (types, starts, lens, count) = run_gpu_lexer(bytes, bytes.len() as u32);
    assert_eq!(count, 1);
    assert_eq!(types[0], TOK_STRING);
    assert_eq!(starts[0], 0);
    assert_eq!(lens[0], bytes.len() as u32);
}

#[test]
fn adjacent_strings_are_separate_tokens() {
    let source = b"\"a\"\"b\"";
    let (types, starts, lens, count) = run_gpu_lexer(source, source.len() as u32);
    assert_eq!(count, 2, "adjacent string literals must be two tokens");
    assert_eq!(types[0], TOK_STRING);
    assert_eq!(types[1], TOK_STRING);
    assert_eq!(starts[0], 0);
    assert_eq!(lens[0], 3);
    assert_eq!(starts[1], 3);
    assert_eq!(lens[1], 3);
}

#[test]
fn string_followed_by_semicolon_span_boundary() {
    let source = b"\"hello\";";
    let (types, starts, lens, count) = run_gpu_lexer(source, source.len() as u32);
    assert_eq!(count, 2);
    assert_eq!(types[0], TOK_STRING);
    assert_eq!(types[1], TOK_SEMICOLON);
    assert_eq!(
        starts[0] + lens[0],
        starts[1],
        "string must end exactly where semicolon begins"
    );
}

#[test]
fn unterminated_string_at_eof_emits_diagnostic_not_string_token() {
    let source = b"int x = \"unterminated";
    let (types, _starts, _lens, count) =
        assert_first_diagnostic(source, C11LexerDiagnosticKind::UnterminatedString);
    assert_eq!(types[count as usize - 1], TOK_ERR_UNTERMINATED_STRING);
    assert!(
        !types.contains(&TOK_STRING),
        "unterminated string must not silently become TOK_STRING"
    );
}

#[test]
fn string_newline_terminates_error_token_without_eating_following_code() {
    let source = b"\"bad\nint x;";
    let (types, starts, lens, count) =
        assert_first_diagnostic(source, C11LexerDiagnosticKind::UnterminatedString);
    assert_eq!(types[0], TOK_ERR_UNTERMINATED_STRING);
    assert_eq!(starts[0], 0);
    assert_eq!(lens[0], 5, "diagnostic spans through the physical newline");
    assert_eq!(
        &types[1..count as usize],
        &[TOK_INT, TOK_IDENTIFIER, TOK_SEMICOLON],
        "unterminated string must not consume the next source line"
    );
}

#[test]
fn unterminated_string_newline_preserves_directive_boundary() {
    let source = b"\"bad\n#define X 1\nint x;";
    let (types, _starts, _lens, count) =
        assert_first_diagnostic(source, C11LexerDiagnosticKind::UnterminatedString);
    assert_eq!(types[0], TOK_ERR_UNTERMINATED_STRING);
    assert_eq!(types[1], TOK_PREPROC);
    assert_eq!(
        &types[count as usize - 3..count as usize],
        &[TOK_INT, TOK_IDENTIFIER, TOK_SEMICOLON]
    );
}

#[test]
fn invalid_string_escape_emits_diagnostic_token_with_full_literal_span() {
    let source = b"\"bad\\q\"";
    let (types, starts, lens, count) =
        assert_first_diagnostic(source, C11LexerDiagnosticKind::InvalidEscape);
    assert_eq!(count, 1);
    assert_eq!(types[0], TOK_ERR_INVALID_ESCAPE);
    assert_eq!(starts[0], 0);
    assert_eq!(lens[0], source.len() as u32);
}

#[test]
fn valid_hex_and_universal_string_escapes_remain_string_tokens() {
    let source = b"\"\\x41\\u0042\\U00000043\"";
    let (types, starts, lens, count) = run_gpu_lexer(source, source.len() as u32);
    assert_eq!(count, 1);
    assert_eq!(types[0], TOK_STRING);
    assert_eq!(starts[0], 0);
    assert_eq!(lens[0], source.len() as u32);
    assert!(
        first_c11_lexer_diagnostic(&types, &starts, &lens).is_none(),
        "valid C escape families must not produce diagnostics"
    );
}

// ---------------------------------------------------------------------------
// 2. Character literal adversarial contracts
// ---------------------------------------------------------------------------

#[test]
fn empty_char_literal_emits_one_char_token_with_len_two() {
    let source = b"\'\'";
    let (types, starts, lens, count) = run_gpu_lexer(source, source.len() as u32);
    assert_eq!(count, 1, "empty char must produce one token");
    assert_eq!(types[0], TOK_CHAR);
    assert_eq!(starts[0], 0);
    assert_eq!(lens[0], 2);
}

#[test]
fn char_with_escaped_quote_has_accurate_span() {
    let source = b"\'\\\'\'";
    let (types, starts, lens, count) = run_gpu_lexer(source, source.len() as u32);
    assert_eq!(count, 1);
    assert_eq!(types[0], TOK_CHAR);
    assert_eq!(starts[0], 0);
    assert_eq!(lens[0], source.len() as u32);
}

#[test]
fn char_with_escaped_backslash_has_accurate_span() {
    let source = b"\'\\\\\'";
    let (types, _starts, lens, count) = run_gpu_lexer(source, source.len() as u32);
    assert_eq!(count, 1);
    assert_eq!(types[0], TOK_CHAR);
    assert_eq!(lens[0], source.len() as u32);
}

#[test]
fn char_with_newline_escape_has_accurate_span() {
    let source = b"\'\\n\'";
    let (types, _starts, lens, count) = run_gpu_lexer(source, source.len() as u32);
    assert_eq!(count, 1);
    assert_eq!(types[0], TOK_CHAR);
    assert_eq!(lens[0], 4);
}

#[test]
fn multi_byte_char_literal_is_single_token() {
    // C implementation-defined, but lexer must emit exactly one TOK_CHAR.
    let source = b"\'ab\'";
    let (types, _, _, count) = run_gpu_lexer(source, source.len() as u32);
    assert_eq!(count, 1);
    assert_eq!(types[0], TOK_CHAR);
}

#[test]
fn char_literal_adjacent_to_identifier_span_boundary() {
    let source = b"\'x\'y";
    let (types, starts, lens, count) = run_gpu_lexer(source, source.len() as u32);
    assert_eq!(count, 2);
    assert_eq!(types[0], TOK_CHAR);
    assert_eq!(types[1], TOK_IDENTIFIER);
    assert_eq!(starts[0] + lens[0], starts[1]);
}

#[test]
fn unterminated_char_at_eof_emits_diagnostic_not_char_token() {
    let source = b"char c = 'x";
    let (types, _starts, _lens, count) =
        assert_first_diagnostic(source, C11LexerDiagnosticKind::UnterminatedChar);
    assert_eq!(types[count as usize - 1], TOK_ERR_UNTERMINATED_CHAR);
    assert!(
        !types.contains(&TOK_CHAR),
        "unterminated char must not silently become TOK_CHAR"
    );
}

#[test]
fn char_newline_terminates_error_token_without_eating_following_code() {
    let source = b"'x\nint y;";
    let (types, starts, lens, count) =
        assert_first_diagnostic(source, C11LexerDiagnosticKind::UnterminatedChar);
    assert_eq!(types[0], TOK_ERR_UNTERMINATED_CHAR);
    assert_eq!(starts[0], 0);
    assert_eq!(lens[0], 3);
    assert_eq!(
        &types[1..count as usize],
        &[TOK_INT, TOK_IDENTIFIER, TOK_SEMICOLON],
        "unterminated char must not consume the next source line"
    );
}

#[test]
fn invalid_char_escape_emits_diagnostic_token_with_full_literal_span() {
    let source = b"'\\q'";
    let (types, starts, lens, count) =
        assert_first_diagnostic(source, C11LexerDiagnosticKind::InvalidEscape);
    assert_eq!(count, 1);
    assert_eq!(types[0], TOK_ERR_INVALID_ESCAPE);
    assert_eq!(starts[0], 0);
    assert_eq!(lens[0], source.len() as u32);
}

// ---------------------------------------------------------------------------
// 3. Comment adversarial contracts
// ---------------------------------------------------------------------------

#[test]
fn line_comment_eats_rest_of_line_including_directive_like_text() {
    let source = b"// #define FOO\nint x;";
    let (types, _, _, count) = run_gpu_lexer(source, source.len() as u32);
    // Whitespace/comment stripped; remaining: int, x, ;
    assert_eq!(
        count, 3,
        "line comment must swallow directive-like remainder"
    );
    assert_eq!(types, vec![TOK_INT, TOK_IDENTIFIER, TOK_SEMICOLON]);
}

#[test]
fn block_comment_with_fake_nesting_ends_at_first_close() {
    // C does not nest block comments.
    let source = b"/* outer /* inner */ still comment */ int x;";
    let (types, _, _, count) = run_gpu_lexer(source, source.len() as u32);
    // After first */ we have " still comment */ int x;"
    // The GPU lexer (and host) will treat " still comment */" as tokens, then int x ;
    // This is adversarial: it tests that the lexer doesn't over-consume.
    assert!(
        count >= 3,
        "block comment must end at first */; remaining tokens must survive"
    );
    // The last three tokens should be int, x, ;
    assert_eq!(types[count as usize - 3], TOK_INT);
    assert_eq!(types[count as usize - 2], TOK_IDENTIFIER);
    assert_eq!(types[count as usize - 1], TOK_SEMICOLON);
}

#[test]
fn comment_adjacent_tokens_have_correct_spans() {
    let source = b"int/*comment*/x;";
    let (types, starts, lens, count) = run_gpu_lexer(source, source.len() as u32);
    assert_eq!(count, 3);
    assert_eq!(types, vec![TOK_INT, TOK_IDENTIFIER, TOK_SEMICOLON]);
    // int starts at 0, len 3
    assert_eq!(starts[0], 0);
    assert_eq!(lens[0], 3);
    // x starts right after comment ends: 3 + 11 = 14
    assert_eq!(starts[1], 14);
    assert_eq!(lens[1], 1);
    // ; starts at 15
    assert_eq!(starts[2], 15);
    assert_eq!(lens[2], 1);
}

#[test]
fn block_comment_spanning_lines_does_not_corrupt_following_spans() {
    let source = b"int /* multi\nline comment */ x ;";
    let (types, starts, lens, count) = run_gpu_lexer(source, source.len() as u32);
    assert_eq!(count, 3);
    assert_eq!(types, vec![TOK_INT, TOK_IDENTIFIER, TOK_SEMICOLON]);
    // x should start at index after comment: "int " = 4, comment = 25 chars total, so x at 29
    let expected_x_start = source.iter().position(|&b| b == b'x').unwrap() as u32;
    assert_eq!(starts[1], expected_x_start);
    assert_eq!(lens[1], 1);
    assert_eq!(starts[2], expected_x_start + 2); // space between x and ;
}

#[test]
fn line_comment_at_end_of_source_produces_no_trailing_tokens() {
    let source = b"int x; // trailing";
    let (types, _, _, count) = run_gpu_lexer(source, source.len() as u32);
    assert_eq!(count, 3);
    assert_eq!(types, vec![TOK_INT, TOK_IDENTIFIER, TOK_SEMICOLON]);
}

#[test]
fn unterminated_block_comment_emits_diagnostic_instead_of_disappearing() {
    let source = b"int x; /* unterminated";
    let (types, starts, lens, count) =
        assert_first_diagnostic(source, C11LexerDiagnosticKind::UnterminatedBlockComment);
    assert_eq!(
        &types[..count as usize],
        &[
            TOK_INT,
            TOK_IDENTIFIER,
            TOK_SEMICOLON,
            TOK_ERR_UNTERMINATED_COMMENT
        ]
    );
    assert_eq!(starts[count as usize - 1], 7);
    assert_eq!(lens[count as usize - 1], (source.len() - 7) as u32);
}

// ---------------------------------------------------------------------------
// 4. Line continuation / backslash-newline contracts
// ---------------------------------------------------------------------------

#[test]
fn backslash_newline_inside_string_does_not_terminate_string() {
    let source = b"\"a\\\nb\"";
    let (types, _starts, lens, count) = run_gpu_lexer(source, source.len() as u32);
    assert_eq!(
        count, 1,
        "backslash-newline inside string must not split token"
    );
    assert_eq!(types[0], TOK_STRING);
    assert_eq!(lens[0], source.len() as u32);
}

#[test]
fn backslash_newline_inside_char_does_not_terminate_char() {
    let source = b"\'a\\\nb\'";
    let (types, _starts, lens, count) = run_gpu_lexer(source, source.len() as u32);
    assert_eq!(
        count, 1,
        "backslash-newline inside char must not split token"
    );
    assert_eq!(types[0], TOK_CHAR);
    assert_eq!(lens[0], source.len() as u32);
}

#[test]
fn backslash_newline_in_code_splits_at_newline() {
    // The GPU lexer treats backslash-before-newline as ordinary characters in code.
    let source = b"in\\\nt x;";
    let (_types, _starts, _lens, count) = run_gpu_lexer(source, source.len() as u32);
    // This is implementation-specific; the contract is that it does not crash
    // and emits some tokens. We assert non-empty, ordered, bounded output.
    assert!(count > 0, "backslash-newline in code must not crash lexer");
    assert!(
        count <= source.len() as u32,
        "token count must not exceed source length"
    );
}

#[test]
fn backslash_newline_inside_line_comment_does_not_extend_comment() {
    let source = b"// comment \\\nstill comment\nint x;";
    let (types, _, _, count) = run_gpu_lexer(source, source.len() as u32);
    // The GPU lexer terminates the line comment at the newline; backslash is
    // part of the comment. 'still' and 'comment' become identifiers.
    assert!(
        count >= 3,
        "line comment must end at newline; remaining tokens must survive"
    );
    // The last three non-comment tokens must be int, x, ;
    assert_eq!(types[count as usize - 3], TOK_INT);
    assert_eq!(types[count as usize - 2], TOK_IDENTIFIER);
    assert_eq!(types[count as usize - 1], TOK_SEMICOLON);
}

// ---------------------------------------------------------------------------
// 5. Preprocessor directive adversarial contracts
// ---------------------------------------------------------------------------

#[test]
fn directive_after_leading_whitespace_is_preproc() {
    let source = b"   #define X 1\nint y;";
    let (types, _, _, count) = run_gpu_lexer(source, source.len() as u32);
    assert!(count >= 3, "must emit preproc + int + y + ;");
    assert_eq!(types[0], TOK_PREPROC, "# after spaces must be preproc");
    assert_eq!(types[count as usize - 3], TOK_INT);
    assert_eq!(types[count as usize - 2], TOK_IDENTIFIER);
    assert_eq!(types[count as usize - 1], TOK_SEMICOLON);
}

#[test]
fn directive_at_column_zero_after_newline_is_preproc_not_operator() {
    let source = b"a\n#b";
    let (types, _, _, count) = run_gpu_lexer(source, source.len() as u32);
    // #b on its own line is consumed as one TOK_PREPROC row.
    assert_eq!(count, 2, "a + preproc_row");
    assert_eq!(types[0], TOK_IDENTIFIER, "a");
    assert_eq!(types[1], TOK_PREPROC, "# at start of line must be preproc");
}

