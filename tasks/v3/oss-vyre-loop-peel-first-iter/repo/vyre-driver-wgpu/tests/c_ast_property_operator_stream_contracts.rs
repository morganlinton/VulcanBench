//! CPU/GPU parity tests on generated C-like token streams with random
//! operators, keywords, and punctuation.

#![cfg(feature = "c-parser")]
#![allow(deprecated)]
#[path = "c_ast_gpu_parity_support/mod.rs"]
mod c_ast_gpu_parity_support;

use c_ast_gpu_parity_support::{
    run_gpu_classifier_with_count, run_gpu_vast_builder_from_parts, starts_for_lens,
};
use proptest::prelude::*;
use vyre_libs::parsing::c::lex::tokens::*;
use vyre_libs::parsing::c::parse::vast::{
    reference_c11_build_vast_nodes, reference_c11_classify_vast_node_kinds,
};

// ---------------------------------------------------------------------------
// Proptest strategies
// ---------------------------------------------------------------------------

fn arb_operator_token_stream() -> impl Strategy<Value = (Vec<u32>, Vec<u32>, Vec<u32>)> {
    let choices = vec![
        TOK_IDENTIFIER,
        TOK_INTEGER,
        TOK_LPAREN,
        TOK_RPAREN,
        TOK_LBRACE,
        TOK_RBRACE,
        TOK_SEMICOLON,
        TOK_COMMA,
        TOK_PLUS,
        TOK_MINUS,
        TOK_STAR,
        TOK_SLASH,
        TOK_PERCENT,
        TOK_AMP,
        TOK_PIPE,
        TOK_CARET,
        TOK_TILDE,
        TOK_BANG,
        TOK_ASSIGN,
        TOK_LT,
        TOK_GT,
        TOK_EQ,
        TOK_NE,
        TOK_LE,
        TOK_GE,
        TOK_AND,
        TOK_OR,
        TOK_LSHIFT,
        TOK_RSHIFT,
        TOK_INC,
        TOK_DEC,
        TOK_PLUS_EQ,
        TOK_MINUS_EQ,
        TOK_STAR_EQ,
        TOK_SLASH_EQ,
        TOK_QUESTION,
        TOK_COLON,
        TOK_DOT,
        TOK_ARROW,
        TOK_IF,
        TOK_ELSE,
        TOK_FOR,
        TOK_WHILE,
        TOK_RETURN,
        TOK_STRUCT,
        TOK_TYPEDEF,
        TOK_INT,
        TOK_VOID,
        TOK_CHAR_KW,
    ];
    prop::collection::vec(prop::sample::select(choices), 1..512).prop_map(|tok_types| {
        let tok_lens: Vec<u32> = tok_types.iter().map(|_| 1u32).collect();
        let tok_starts = starts_for_lens(&tok_lens);
        (tok_types, tok_starts, tok_lens)
    })
}

// ---------------------------------------------------------------------------
// Property tests
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn proptest_random_operator_stream_vast_builder_parity(
        (tok_types, tok_starts, tok_lens) in arb_operator_token_stream()
    ) {
        let cpu = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
        let gpu = run_gpu_vast_builder_from_parts(&tok_types, &tok_starts, &tok_lens);
        prop_assert_eq!(gpu, cpu, "GPU VAST builder must match CPU for random operator stream");
    }
}

// ---------------------------------------------------------------------------
// Deterministic classifier CPU/GPU parity
// ---------------------------------------------------------------------------

#[test]
fn classifier_parity_lparen_lbrace_lparen_assign() {
    let tok_types = vec![TOK_STRUCT, TOK_LPAREN, TOK_LBRACE, TOK_LPAREN, TOK_ASSIGN];
    let tok_lens = vec![1; tok_types.len()];
    let tok_starts = starts_for_lens(&tok_lens);
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let cpu_typed = reference_c11_classify_vast_node_kinds(&raw);
    let gpu_typed = run_gpu_classifier_with_count(&raw, tok_types.len() as u32);
    assert_eq!(
        gpu_typed, cpu_typed,
        "GPU classifier must not propagate declaration context through aggregate braces"
    );
}

// ---------------------------------------------------------------------------
// Deterministic adversarial operator tables
// ---------------------------------------------------------------------------

#[test]
fn adversarial_all_operators_flat() {
    let tok_types = vec![
        TOK_IDENTIFIER,
        TOK_PLUS,
        TOK_IDENTIFIER,
        TOK_MINUS,
        TOK_IDENTIFIER,
        TOK_STAR,
        TOK_IDENTIFIER,
        TOK_SLASH,
        TOK_IDENTIFIER,
        TOK_PERCENT,
        TOK_AMP,
        TOK_IDENTIFIER,
        TOK_PIPE,
        TOK_IDENTIFIER,
        TOK_CARET,
        TOK_IDENTIFIER,
        TOK_LT,
        TOK_IDENTIFIER,
        TOK_GT,
        TOK_IDENTIFIER,
        TOK_EQ,
        TOK_IDENTIFIER,
        TOK_NE,
        TOK_IDENTIFIER,
        TOK_LE,
        TOK_IDENTIFIER,
        TOK_GE,
        TOK_IDENTIFIER,
        TOK_AND,
        TOK_IDENTIFIER,
        TOK_OR,
        TOK_IDENTIFIER,
        TOK_LSHIFT,
        TOK_IDENTIFIER,
        TOK_RSHIFT,
        TOK_INC,
        TOK_IDENTIFIER,
        TOK_DEC,
        TOK_IDENTIFIER,
        TOK_PLUS_EQ,
        TOK_IDENTIFIER,
        TOK_MINUS_EQ,
        TOK_IDENTIFIER,
        TOK_STAR_EQ,
        TOK_IDENTIFIER,
        TOK_SLASH_EQ,
        TOK_IDENTIFIER,
        TOK_QUESTION,
        TOK_IDENTIFIER,
        TOK_COLON,
        TOK_DOT,
        TOK_IDENTIFIER,
        TOK_ARROW,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
    ];
    let tok_lens = vec![1; tok_types.len()];
    let tok_starts = starts_for_lens(&tok_lens);
    let cpu = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let gpu = run_gpu_vast_builder_from_parts(&tok_types, &tok_starts, &tok_lens);
    assert_eq!(cpu, gpu, "GPU parity for all-operators flat stream");
}

#[test]
fn adversarial_random_operators_with_nesting() {
    let tok_types = vec![
        TOK_IF,
        TOK_LPAREN,
        TOK_IDENTIFIER,
        TOK_EQ,
        TOK_INTEGER,
        TOK_RPAREN,
        TOK_LBRACE,
        TOK_IDENTIFIER,
        TOK_ASSIGN,
        TOK_IDENTIFIER,
        TOK_PLUS,
        TOK_IDENTIFIER,
        TOK_STAR,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
        TOK_WHILE,
        TOK_LPAREN,
        TOK_IDENTIFIER,
        TOK_LT,
        TOK_INTEGER,
        TOK_RPAREN,
        TOK_LBRACE,
        TOK_IDENTIFIER,
        TOK_PLUS_EQ,
        TOK_INTEGER,
        TOK_SEMICOLON,
        TOK_RBRACE,
        TOK_RBRACE,
    ];
    let tok_lens = vec![1; tok_types.len()];
    let tok_starts = starts_for_lens(&tok_lens);
    let cpu = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let gpu = run_gpu_vast_builder_from_parts(&tok_types, &tok_starts, &tok_lens);
    assert_eq!(cpu, gpu, "GPU parity for nested operator stream");
}

#[test]
fn adversarial_ternary_and_compound_assignments() {
    let tok_types = vec![
        TOK_IDENTIFIER,
        TOK_ASSIGN,
        TOK_IDENTIFIER,
        TOK_QUESTION,
        TOK_IDENTIFIER,
        TOK_COLON,
        TOK_IDENTIFIER,
        TOK_STAR_EQ,
        TOK_IDENTIFIER,
        TOK_MINUS_EQ,
        TOK_IDENTIFIER,
        TOK_PLUS_EQ,
        TOK_IDENTIFIER,
        TOK_SLASH_EQ,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
    ];
    let tok_lens = vec![1; tok_types.len()];
    let tok_starts = starts_for_lens(&tok_lens);
    let cpu = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let gpu = run_gpu_vast_builder_from_parts(&tok_types, &tok_starts, &tok_lens);
    assert_eq!(cpu, gpu, "GPU parity for ternary and compound assignments");
}

#[test]
fn adversarial_bitwise_and_logical_operators() {
    let tok_types = vec![
        TOK_IDENTIFIER,
        TOK_AMP,
        TOK_IDENTIFIER,
        TOK_PIPE,
        TOK_IDENTIFIER,
        TOK_CARET,
        TOK_IDENTIFIER,
        TOK_TILDE,
        TOK_IDENTIFIER,
        TOK_BANG,
        TOK_IDENTIFIER,
        TOK_AND,
        TOK_IDENTIFIER,
        TOK_OR,
        TOK_IDENTIFIER,
        TOK_LSHIFT,
        TOK_IDENTIFIER,
        TOK_RSHIFT,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
    ];
    let tok_lens = vec![1; tok_types.len()];
    let tok_starts = starts_for_lens(&tok_lens);
    let cpu = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let gpu = run_gpu_vast_builder_from_parts(&tok_types, &tok_starts, &tok_lens);
    assert_eq!(cpu, gpu, "GPU parity for bitwise and logical operators");
}
