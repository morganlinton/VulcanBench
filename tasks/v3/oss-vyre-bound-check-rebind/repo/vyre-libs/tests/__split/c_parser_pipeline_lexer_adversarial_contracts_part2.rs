use super::*;

#[test]
fn mid_line_hash_is_operator_not_directive() {
    let source = b"a # b";
    let (types, _, _, count) = run_gpu_lexer(source, source.len() as u32);
    assert_eq!(count, 3);
    assert_eq!(types[0], TOK_IDENTIFIER);
    assert_eq!(types[1], TOK_HASH, "mid-line # must be TOK_HASH");
    assert_eq!(types[2], TOK_IDENTIFIER);
}

#[test]
fn multiple_consecutive_directives_each_emits_separate_preproc() {
    let source = b"#ifndef FOO\n#define FOO 1\n#endif\n#pragma once\n";
    let (types, _, _, count) = run_gpu_lexer(source, source.len() as u32);
    assert_eq!(count, 4, "four directives = four preproc tokens");
    assert!(
        types.iter().all(|&t| t == TOK_PREPROC),
        "all must be TOK_PREPROC"
    );
}

#[test]
fn directive_with_string_inside_is_single_preproc() {
    let source = b"#define MSG \"hello\"\nint z;";
    let (types, _, _, count) = run_gpu_lexer(source, source.len() as u32);
    assert!(count >= 2, "preproc + int + z + ;");
    assert_eq!(
        types[0], TOK_PREPROC,
        "directive with string must stay one preproc row"
    );
}

#[test]
fn directive_with_comment_inside_is_single_preproc() {
    let source = b"#define FOO /*comment*/ 1\nint w;";
    let (types, _, _, count) = run_gpu_lexer(source, source.len() as u32);
    assert!(count >= 2, "preproc + int + w + ;");
    assert_eq!(
        types[0], TOK_PREPROC,
        "directive with inline block comment must stay one preproc row"
    );
}

#[test]
fn hash_hash_outside_directive_is_one_preprocessing_operator() {
    let source = b"a ## b";
    let (types, _, _, count) = run_gpu_lexer(source, source.len() as u32);
    assert_eq!(count, 3);
    assert_eq!(types[0], TOK_IDENTIFIER);
    assert_eq!(types[1], TOK_HASHHASH);
    assert_eq!(types[2], TOK_IDENTIFIER);
}

// ---------------------------------------------------------------------------
// 6. Source-order and span integrity contracts
// ---------------------------------------------------------------------------

#[test]
fn token_starts_are_strictly_monotonic_for_mixed_adversarial_source() {
    let source = b"int /*c*/ \"s\" 1 #define X 1\n + ;";
    let (_, starts, _, count) = run_gpu_lexer(source, source.len() as u32);
    assert!(count > 0);
    let starts = &starts[..count as usize];
    assert!(
        starts.windows(2).all(|w| w[0] < w[1]),
        "tok_starts must be strictly increasing even with comments/strings/directives: {:?}",
        starts
    );
}

#[test]
fn token_spans_cover_exact_source_bytes_for_punctuation() {
    let source = b"+++=--";
    let (types, starts, lens, count) = run_gpu_lexer(source, source.len() as u32);
    // max-munch: ++, +=, --
    assert_eq!(count, 3);
    assert_eq!(types[0], TOK_INC);
    assert_eq!(starts[0], 0);
    assert_eq!(lens[0], 2);
    assert_eq!(types[1], TOK_PLUS_EQ);
    assert_eq!(starts[1], 2);
    assert_eq!(lens[1], 2);
    assert_eq!(types[2], TOK_DEC);
    assert_eq!(starts[2], 4);
    assert_eq!(lens[2], 2);
}

#[test]
fn token_lens_sum_bounded_by_source_len() {
    let source = b"int main(void) { return 42; }";
    let (_, _, lens, _count) = run_gpu_lexer(source, source.len() as u32);
    let total_len: u32 = lens.iter().sum();
    assert!(
        total_len <= source.len() as u32,
        "sum of token lengths {} must not exceed source length {}",
        total_len,
        source.len()
    );
}

#[test]
fn every_token_kind_has_non_zero_length() {
    let source = b"int x = 1;";
    let (_, _, lens, count) = run_gpu_lexer(source, source.len() as u32);
    for i in 0..count {
        assert!(
            lens[i as usize] > 0,
            "token {i} has zero length  -  lexer emitted a ghost token"
        );
    }
}

// ---------------------------------------------------------------------------
// 7. Host / GPU parity on adversarial fixtures
// ---------------------------------------------------------------------------

#[test]
fn host_gpu_agree_on_pure_punctuation() {
    assert_host_gpu_agree(b"++ -- += -= *= /= %= &= |= ^= <<= >>=");
}

#[test]
fn host_gpu_agree_on_strings_and_comments() {
    assert_host_gpu_agree(b"\"hello\" /* world */ int x; // end\n");
}

#[test]
fn host_gpu_agree_on_directive_heavy_source() {
    assert_host_gpu_agree(b"#ifndef FOO\n#define FOO 1\n#endif\n#pragma once\n");
}

#[test]
fn host_gpu_agree_on_mixed_adversarial_fixture() {
    let source = b"int /*c*/ \"str\" 1 #define X 1\n + ; { } [ ] ( ) -> .";
    assert_host_gpu_agree(source);
}

#[test]
fn host_gpu_agree_on_ellipsis_and_dots() {
    assert_host_gpu_agree(b"... . .. ...");
}

// ---------------------------------------------------------------------------
// 8. Max-size and empty-boundary contracts
// ---------------------------------------------------------------------------

#[test]
fn empty_source_emits_zero_tokens() {
    let kinds = lex_c11_max_munch_kinds(b"").expect("empty source must lex");
    let non_ws: Vec<u32> = kinds
        .into_iter()
        .filter(|k| *k != TOK_WHITESPACE && *k != TOK_COMMENT)
        .collect();
    assert_eq!(non_ws.len(), 0, "empty source must emit zero tokens");
}

#[test]
fn whitespace_only_source_emits_zero_tokens() {
    let source = b" \t\n\r ";
    let (_, _, _, count) = run_gpu_lexer(source, source.len() as u32);
    assert_eq!(count, 0, "whitespace-only source must emit zero tokens");
}

#[test]
fn token_count_bounded_by_ast_max_tok_scan_for_dense_stream() {
    // 256 single-byte tokens (semicolon does not combine with itself)
    let source = vec![b';'; 256];
    let (types, _, _, count) = run_gpu_lexer(&source, source.len() as u32);
    assert_eq!(count, 256, "dense ; stream must emit 256 tokens");
    assert!(
        count <= C11_AST_MAX_TOK_SCAN,
        "count must not exceed C11_AST_MAX_TOK_SCAN"
    );
    assert!(
        types.iter().all(|&t| t == TOK_SEMICOLON),
        "all must be TOK_SEMICOLON"
    );
}

#[test]
fn lexer_does_not_emit_zero_length_tokens_for_nonempty_source() {
    let source = b"a+b";
    let (_, _, lens, count) = run_gpu_lexer(source, source.len() as u32);
    for i in 0..count {
        assert!(lens[i as usize] > 0, "token {i} must have non-zero length");
    }
}

#[test]
fn lexer_output_buffers_are_not_all_zeros_for_nonempty_input() {
    let source = b"int x;";
    let program = c11_lexer(
        "haystack",
        "out_tok_types",
        "out_tok_starts",
        "out_tok_lens",
        "out_counts",
        source.len() as u32,
    );
    let haystack_buf = u32_bytes(&haystack_words(source));
    let zero_buf = vec![0u8; source.len() * 4];
    let count_zero = vec![0u8; 4];
    let inputs = [
        Value::from(haystack_buf),
        Value::from(zero_buf.clone()),
        Value::from(zero_buf.clone()),
        Value::from(zero_buf.clone()),
        Value::from(count_zero),
    ];
    let outputs = vyre_reference::reference_eval(&program, &inputs).expect("lexer must run");
    // At least one output buffer must contain non-zero data (the count if nothing else)
    let any_nonzero = outputs.iter().any(|v| v.to_bytes().iter().any(|&b| b != 0));
    assert!(
        any_nonzero,
        "lexer must not silently produce all-zero outputs for non-empty source"
    );
}
