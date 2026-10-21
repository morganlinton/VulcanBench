//! Contract tests for dynamic C preprocessor macro expansion bounds.

#![cfg(feature = "c-parser")]
#![allow(deprecated)]

mod common;
use common::{decode_u32_words, u32_bytes};
use std::panic::{catch_unwind, AssertUnwindSafe};

use vyre::ir::Expr;
use vyre_libs::parsing::c::lex::tokens::{TOK_IDENTIFIER, TOK_INTEGER, TOK_PLUS, TOK_STAR};
use vyre_libs::parsing::c::preprocess::expansion::opt_dynamic_macro_expansion;
use vyre_reference::value::Value;

const EMPTY_SLOT: u32 = u32::MAX;
const TABLE_SLOTS: usize = 4096;
const TABLE_MASK: u32 = 4095;

fn hash_token(tok: u32) -> usize {
    (tok.wrapping_mul(2_654_435_769) & TABLE_MASK) as usize
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

#[test]
fn dynamic_macro_expansion_emits_replacement_tokens_and_count() {
    let mut fixture = MacroFixture::empty();
    fixture.insert(TOK_IDENTIFIER, 512, &[TOK_INTEGER, TOK_PLUS, TOK_INTEGER]);

    let outputs = run_dynamic_macro_expansion(&[TOK_IDENTIFIER, TOK_STAR], &fixture, 8)
        .expect("bounded macro expansion must succeed");
    assert_eq!(outputs.len(), 2);

    let out_tokens = decode_u32_words(&outputs[0].to_bytes());
    let out_count = decode_u32_words(&outputs[1].to_bytes());
    assert_eq!(
        &out_tokens[..4],
        &[TOK_INTEGER, TOK_PLUS, TOK_INTEGER, TOK_STAR]
    );
    assert_eq!(out_count, vec![4]);
}

#[test]
fn dynamic_macro_expansion_passthrough_counts_unmapped_tokens() {
    let fixture = MacroFixture::empty();
    let input = [TOK_IDENTIFIER, TOK_PLUS, TOK_INTEGER];

    let outputs = run_dynamic_macro_expansion(&input, &fixture, 8)
        .expect("unmapped macro tokens must pass through");
    let out_tokens = decode_u32_words(&outputs[0].to_bytes());
    let out_count = decode_u32_words(&outputs[1].to_bytes());
    assert_eq!(&out_tokens[..input.len()], &input);
    assert_eq!(out_count, vec![input.len() as u32]);
}

#[test]
fn dynamic_macro_expansion_rejects_output_capacity_overflow_without_panic() {
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
