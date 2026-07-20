use super::*;

#[test]
fn leading_hash_becomes_preproc_row_gpu_lexer() {
    let source = b"#define X 1\na # b";
    let (tok_types, _, _, count) = run_c11_lexer(source, source.len() as u32);
    assert_eq!(count, 4, "preproc row + a + # + b = 4 tokens");
    assert_eq!(
        tok_types[0], TOK_PREPROC,
        "leading # must start preproc row"
    );
    assert_eq!(tok_types[1], TOK_IDENTIFIER, "a");
    assert_eq!(tok_types[2], TOK_HASH, "mid-line # must be TOK_HASH");
    assert_eq!(tok_types[3], TOK_IDENTIFIER, "b");
}

#[test]
fn sparse_lexer_emits_single_preproc_row_for_directive_line() {
    let source = b"#include <x.h>\nint x;";
    let (types, starts, lens, flags) = run_sparse_c11_lexer_positions(source);

    assert_eq!(types[0], TOK_PREPROC, "line-start # must become TOK_PREPROC");
    assert_eq!(starts[0], 0, "preproc row starts at the directive hash");
    assert_eq!(lens[0], 14, "preproc row spans the directive payload without newline");
    assert_eq!(flags[0], 1, "preproc row must be visible to sparse compaction");

    assert!(
        types[1..14].iter().all(|kind| *kind == 0),
        "sparse lexer must suppress token starts inside the directive row"
    );
}

#[test]
fn leading_hash_becomes_preproc_row_host_lexer() {
    let kinds = lex_c11_max_munch_kinds(b"#define X 1\na # b").expect("host lexer must accept");
    let non_ws: Vec<u32> = kinds
        .into_iter()
        .filter(|k| *k != TOK_WHITESPACE && *k != TOK_COMMENT)
        .collect();
    assert_eq!(
        non_ws,
        vec![TOK_PREPROC, TOK_IDENTIFIER, TOK_HASH, TOK_IDENTIFIER],
        "host lexer: mid-line # must be TOK_HASH operator"
    );
}

#[test]
fn hash_after_whitespace_on_fresh_line_is_preproc_not_operator() {
    let source = b"\n  # define Y 2\nz";
    let (tok_types, _, _, count) = run_c11_lexer(source, source.len() as u32);
    assert_eq!(count, 2, "whitespace-leading preproc + z = 2 tokens");
    assert_eq!(
        tok_types[0], TOK_PREPROC,
        "# after newline+spaces must be preproc"
    );
    assert_eq!(tok_types[1], TOK_IDENTIFIER, "z");
}

// ---------------------------------------------------------------------------
// 7. Include guards
// ---------------------------------------------------------------------------

#[test]
fn include_guard_triplet_lexes_as_three_preproc_rows() {
    let source = b"#ifndef FOO\n#define FOO 1\n#endif\nint x;";
    let (tok_types, _, _, count) = run_c11_lexer(source, source.len() as u32);
    // #ifndef, #define, #endif are each preproc rows.
    // The preprocessor lexer boundary is pre-keyword-promotion: int and x are identifiers.
    assert_eq!(count, 6, "3 preproc rows + int + x + ; = 6 tokens");
    assert_eq!(tok_types[0], TOK_PREPROC, "#ifndef row");
    assert_eq!(tok_types[1], TOK_PREPROC, "#define row");
    assert_eq!(tok_types[2], TOK_PREPROC, "#endif row");
    assert_eq!(tok_types[3], TOK_IDENTIFIER, "int before keyword promotion");
    assert_eq!(tok_types[4], TOK_IDENTIFIER, "x");
    assert_eq!(tok_types[5], TOK_SEMICOLON, ";");
}

#[test]
fn pragma_once_lexes_as_single_preproc_row() {
    let source = b"#pragma once\n";
    let (tok_types, _, _, count) = run_c11_lexer(source, source.len() as u32);
    assert_eq!(count, 1);
    assert_eq!(
        tok_types[0], TOK_PREPROC,
        "#pragma once must be single preproc row"
    );
}

// ---------------------------------------------------------------------------
// 8. Overflow / determinism contracts
// ---------------------------------------------------------------------------

#[test]
fn dynamic_macro_expansion_traps_on_output_overflow() {
    let mut fixture = MacroFixture::empty();
    fixture.insert(TOK_IDENTIFIER, 512, &[TOK_INTEGER, TOK_PLUS, TOK_INTEGER]);

    let result = catch_unwind(AssertUnwindSafe(|| {
        run_dynamic_macro_expansion(&[TOK_IDENTIFIER, TOK_IDENTIFIER], &fixture, 5)
    }));
    let eval_result = result.expect("output-capacity overflow must return an error, not panic");
    let err = eval_result.expect_err("two 3-token expansions into five output slots must reject");
    let msg = err.to_string();
    assert!(
        msg.contains("capacity") || msg.contains("overflow") || msg.contains("Fix:"),
        "capacity overflow error: {msg}"
    );
}

#[test]
fn dynamic_macro_expansion_is_deterministic_across_identical_runs() {
    let mut fixture = MacroFixture::empty();
    fixture.insert(TOK_IDENTIFIER, 512, &[TOK_INTEGER, TOK_STAR, TOK_INTEGER]);
    let input = [TOK_IDENTIFIER, TOK_PLUS, TOK_IDENTIFIER];

    let out_a = run_dynamic_macro_expansion(&input, &fixture, 16).unwrap();
    let out_b = run_dynamic_macro_expansion(&input, &fixture, 16).unwrap();

    assert_eq!(
        decode_u32_words(&out_a[0].to_bytes()),
        decode_u32_words(&out_b[0].to_bytes()),
        "token output must be deterministic"
    );
    assert_eq!(
        decode_u32_words(&out_a[1].to_bytes()),
        decode_u32_words(&out_b[1].to_bytes()),
        "count output must be deterministic"
    );
}

#[test]
fn conditional_mask_produces_stable_all_ones_for_any_input() {
    let inputs: Vec<u32> = vec![TOK_PREPROC, TOK_IDENTIFIER, TOK_HASH, TOK_INTEGER];
    let outputs = run_conditional_mask(&inputs).expect("conditional mask must succeed");
    let mask = decode_u32_words(&outputs[0].to_bytes());
    assert_eq!(
        mask.len(),
        inputs.len(),
        "mask length must match input length"
    );
    assert!(
        mask.iter().all(|&v| v == 1),
        "conditional mask must emit all-ones for every token"
    );
}

#[test]
fn conditional_mask_is_deterministic_across_runs() {
    let inputs: Vec<u32> = vec![TOK_PREPROC, TOK_IDENTIFIER, TOK_HASH];
    let out_a = run_conditional_mask(&inputs).unwrap();
    let out_b = run_conditional_mask(&inputs).unwrap();
    assert_eq!(
        decode_u32_words(&out_a[0].to_bytes()),
        decode_u32_words(&out_b[0].to_bytes()),
        "conditional mask must be deterministic"
    );
}

#[test]
fn conditional_mask_rejects_zero_length_stream_loudly() {
    let err = run_conditional_mask(&[])
        .expect_err("zero-length conditional mask must fail loudly instead of hiding bad bounds");
    assert!(
        err.to_string()
            .contains("conditional-mask-empty-token-stream"),
        "empty conditional-mask failure must identify the parser pipeline boundary, got: {err}"
    );
}

#[test]
fn host_and_gpu_lexer_agree_on_pure_directive_lines() {
    let source = b"#ifndef FOO\n#define FOO 1\n#endif\n";
    let host_kinds = lex_c11_max_munch_kinds(source).expect("host lexer must accept");
    let gpu_kinds = run_c11_lexer(source, source.len() as u32).0;

    let host_non_ws: Vec<u32> = host_kinds
        .into_iter()
        .filter(|k| *k != TOK_WHITESPACE && *k != TOK_COMMENT)
        .collect();

    assert_eq!(
        gpu_kinds, host_non_ws,
        "GPU and host lexer must agree on pure directive-only source"
    );
}
