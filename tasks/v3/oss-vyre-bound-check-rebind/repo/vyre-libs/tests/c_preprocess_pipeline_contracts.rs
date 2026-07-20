//! Contract tests for the C preprocessor pipeline.
//!
//! Covers: object-like macros, nested function-like macro shapes, token paste,
//! stringize, escaped newlines, directive-position hash versus operator hash,
//! include guards, and overflow/determinism contracts.
//!
//! GPU and host lexing must agree on directive-position `#`: only `#` at the
//! start of a logical line after whitespace starts a preprocessor row; mid-line
//! `#` remains a normal hash token.

#![cfg(feature = "c-parser")]
#![allow(deprecated)]

mod common;
use common::{decode_u32_words, u32_bytes};
use std::panic::{catch_unwind, AssertUnwindSafe};

use c_grammar_gen::lex_c11_max_munch_kinds;
use vyre::ir::Expr;
use vyre_libs::parsing::c::lex::lexer::{
    c11_lexer, c11_lexer_regular_sparse_packed_haystack_with_flags,
};
use vyre_libs::parsing::c::lex::tokens::*;
use vyre_libs::parsing::c::preprocess::expansion::{
    opt_conditional_mask, opt_dynamic_macro_expansion,
};
use vyre_libs::parsing::c::preprocess::{
    c_translation_phase_line_splice, reference_c_preprocessor_directive_metadata,
};
use vyre_reference::value::Value;

// ---------------------------------------------------------------------------
// Byte / word helpers
// ---------------------------------------------------------------------------

fn haystack_words(source: &[u8]) -> Vec<u32> {
    source.iter().map(|b| u32::from(*b)).collect()
}

// ---------------------------------------------------------------------------
// GPU lexer helper (mirrors c_lexer_parallelization_contract.rs)
// ---------------------------------------------------------------------------

fn run_c11_lexer(source: &[u8], haystack_len: u32) -> (Vec<u32>, Vec<u32>, Vec<u32>, u32) {
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
    (
        tok_types[..tok_count as usize].to_vec(),
        tok_starts[..tok_count as usize].to_vec(),
        tok_lens[..tok_count as usize].to_vec(),
        tok_count,
    )
}

fn run_sparse_c11_lexer_positions(source: &[u8]) -> (Vec<u32>, Vec<u32>, Vec<u32>, Vec<u32>) {
    let haystack_len = source.len() as u32;
    let program = c11_lexer_regular_sparse_packed_haystack_with_flags(
        "haystack",
        "sparse_types",
        "sparse_starts",
        "sparse_lens",
        "sparse_flags",
        haystack_len,
    );
    let padded_len = source.len().div_ceil(4).max(1) * 4;
    let mut haystack = Vec::with_capacity(padded_len);
    haystack.extend_from_slice(source);
    haystack.resize(padded_len, 0);
    let zero_buf = vec![0u8; source.len() * 4];
    let outputs = vyre_reference::reference_eval(
        &program,
        &[
            Value::from(haystack),
            Value::from(zero_buf.clone()),
            Value::from(zero_buf.clone()),
            Value::from(zero_buf.clone()),
            Value::from(zero_buf),
        ],
    )
    .expect("sparse c11 lexer must execute under the reference oracle");
    assert_eq!(
        outputs.len(),
        4,
        "expected [sparse_types, sparse_starts, sparse_lens, sparse_flags]"
    );
    (
        decode_u32_words(&outputs[0].to_bytes()),
        decode_u32_words(&outputs[1].to_bytes()),
        decode_u32_words(&outputs[2].to_bytes()),
        decode_u32_words(&outputs[3].to_bytes()),
    )
}

// ---------------------------------------------------------------------------
// Dynamic macro-expansion helpers (mirrors c_preprocess_dynamic_macro_expansion_contracts.rs)
// ---------------------------------------------------------------------------

const EMPTY_SLOT: u32 = u32::MAX;
const TABLE_SLOTS: usize = 4096;

fn hash_token(tok: u32) -> usize {
    (tok.wrapping_mul(2_654_435_769) & 4095) as usize
}

struct MacroFixture {
    keys: Vec<u32>,
    vals: Vec<u32>,
    sizes: Vec<u32>,
}

impl MacroFixture {
    fn empty() -> Self {
        Self {
            keys: vec![EMPTY_SLOT; TABLE_SLOTS],
            vals: vec![0; TABLE_SLOTS],
            sizes: vec![0; TABLE_SLOTS],
        }
    }

    fn insert(&mut self, token: u32, replacement_offset: usize, replacement: &[u32]) {
        let slot = hash_token(token);
        self.keys[slot] = token;
        self.vals[slot] = replacement_offset as u32;
        self.sizes[replacement_offset] = replacement.len() as u32;
        for (idx, value) in replacement.iter().enumerate() {
            self.vals[replacement_offset + idx] = *value;
        }
    }
}

fn run_dynamic_macro_expansion(
    input: &[u32],
    fixture: &MacroFixture,
    max_out_tokens: u32,
) -> Result<Vec<Value>, vyre::Error> {
    let program = opt_dynamic_macro_expansion(
        "in_tok_types",
        "macro_keys",
        "macro_vals",
        "macro_sizes",
        "out_tok_types",
        "out_tok_counts",
        Expr::u32(input.len() as u32),
        max_out_tokens,
    );
    let values = [
        Value::from(u32_bytes(input)),
        Value::from(u32_bytes(&fixture.keys)),
        Value::from(u32_bytes(&fixture.vals)),
        Value::from(u32_bytes(&fixture.sizes)),
        Value::from(vec![0u8; max_out_tokens as usize * 4]),
        Value::from(vec![0u8; 4]),
    ];
    vyre_reference::reference_eval(&program, &values)
}

fn run_conditional_mask(tok_types: &[u32]) -> Result<Vec<Value>, vyre::Error> {
    let program = opt_conditional_mask("tok_types", "out_mask", Expr::u32(tok_types.len() as u32));
    let input_bytes = if tok_types.is_empty() {
        vec![0u8; 4]
    } else {
        u32_bytes(tok_types)
    };
    let out_bytes = vec![0u8; tok_types.len().max(1) * 4];
    let values = [Value::from(input_bytes), Value::from(out_bytes)];
    vyre_reference::reference_eval(&program, &values)
}

// ---------------------------------------------------------------------------
// 1. Object-like macros
// ---------------------------------------------------------------------------

mod c_preprocess_pipeline_contracts_part1 {

    include!("__split/c_preprocess_pipeline_contracts_part1.rs");
}
mod c_preprocess_pipeline_contracts_part2 {
    include!("__split/c_preprocess_pipeline_contracts_part2.rs");
}
