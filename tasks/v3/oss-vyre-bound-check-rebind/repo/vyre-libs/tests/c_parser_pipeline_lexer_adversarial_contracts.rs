//! Adversarial contract tests for the C11 GPU lexer.
//!
//! Covers string literals, character literals, comments, line continuations,
//! preprocessor directives, and source-span integrity under hostile inputs.
//! Every test asserts either exact token-kind sequences, exact byte spans,
//! or host-vs-GPU parity  -  never silent acceptance of empty or default output.

#![cfg(feature = "c-parser")]
#![allow(deprecated)]

mod common;
use c_grammar_gen::lex_c11_max_munch_kinds;
use common::{decode_u32_words, u32_bytes};
use vyre_libs::parsing::c::lex::diagnostics::{first_c11_lexer_diagnostic, C11LexerDiagnosticKind};
use vyre_libs::parsing::c::lex::keyword::reference_c_keyword_types;
use vyre_libs::parsing::c::lex::lexer::c11_lexer;
use vyre_libs::parsing::c::lex::tokens::*;
use vyre_libs::parsing::c::pipeline::stages::C11_AST_MAX_TOK_SCAN;
use vyre_reference::value::Value;

// ---------------------------------------------------------------------------
// Byte / word helpers
// ---------------------------------------------------------------------------

fn haystack_words(source: &[u8]) -> Vec<u32> {
    source.iter().map(|b| u32::from(*b)).collect()
}

/// Run the GPU lexer `c11_lexer` through the Reference oracle oracle and return
/// the compact, keyword-promoted token stream plus the emitted token count.
fn run_gpu_lexer(source: &[u8], haystack_len: u32) -> (Vec<u32>, Vec<u32>, Vec<u32>, u32) {
    let program = c11_lexer(
        "haystack",
        "out_tok_types",
        "out_tok_starts",
        "out_tok_lens",
        "out_counts",
        haystack_len,
    );
    let haystack_buf = u32_bytes(&haystack_words(source));
    let zero_buf = vec![0u8; haystack_len as usize * 4];
    let count_zero = vec![0u8; 4];
    let inputs = [
        Value::from(haystack_buf),
        Value::from(zero_buf.clone()),
        Value::from(zero_buf.clone()),
        Value::from(zero_buf),
        Value::from(count_zero),
    ];
    let outputs = vyre_reference::reference_eval(&program, &inputs)
        .expect("c11_lexer must execute under the reference oracle");
    assert_eq!(
        outputs.len(),
        4,
        "expected [tok_types, tok_starts, tok_lens, counts]"
    );
    let tok_types = decode_u32_words(&outputs[0].to_bytes());
    let tok_starts = decode_u32_words(&outputs[1].to_bytes());
    let tok_lens = decode_u32_words(&outputs[2].to_bytes());
    let counts = decode_u32_words(&outputs[3].to_bytes());
    let tok_count = counts.first().copied().unwrap_or(0);
    let promoted_tok_types = reference_c_keyword_types(
        &tok_types[..tok_count as usize],
        &tok_starts[..tok_count as usize],
        &tok_lens[..tok_count as usize],
        source,
    );
    (
        promoted_tok_types,
        tok_starts[..tok_count as usize].to_vec(),
        tok_lens[..tok_count as usize].to_vec(),
        tok_count,
    )
}

/// Assert that the GPU lexer and the host max-munch lexer agree on the
/// non-whitespace, non-comment token sequence for `source`.
fn assert_host_gpu_agree(source: &[u8]) {
    let host_kinds = lex_c11_max_munch_kinds(source).expect("host lexer must accept source");
    let host_non_ws: Vec<u32> = host_kinds
        .into_iter()
        .filter(|k| *k != TOK_WHITESPACE && *k != TOK_COMMENT)
        .collect();
    let (gpu_types, _, _, gpu_count) = run_gpu_lexer(source, source.len() as u32);
    assert_eq!(
        gpu_count as usize,
        gpu_types.len(),
        "GPU count must match trimmed length"
    );
    assert_eq!(
        gpu_types,
        host_non_ws,
        "GPU lexer disagrees with host lexer for source: {:?}",
        std::str::from_utf8(source).unwrap_or("<binary>")
    );
}

fn assert_first_diagnostic(
    source: &[u8],
    expected_kind: C11LexerDiagnosticKind,
) -> (Vec<u32>, Vec<u32>, Vec<u32>, u32) {
    let (types, starts, lens, count) = run_gpu_lexer(source, source.len() as u32);
    let diag = first_c11_lexer_diagnostic(&types, &starts, &lens)
        .expect("malformed fixture must emit a lexer diagnostic token");
    assert_eq!(diag.kind, expected_kind);
    assert!(
        diag.byte_start + diag.byte_len <= source.len() as u32,
        "diagnostic span must stay inside the source"
    );
    assert!(
        is_c_lexer_error_token(types[diag.token_index as usize]),
        "diagnostic token must be encoded as a lexer error token"
    );
    (types, starts, lens, count)
}

// ---------------------------------------------------------------------------
// 1. String literal adversarial contracts
// ---------------------------------------------------------------------------

mod c_parser_pipeline_lexer_adversarial_contracts_part1 {

    include!("__split/c_parser_pipeline_lexer_adversarial_contracts_part1.rs");
}
mod c_parser_pipeline_lexer_adversarial_contracts_part2 {
    include!("__split/c_parser_pipeline_lexer_adversarial_contracts_part2.rs");
}
