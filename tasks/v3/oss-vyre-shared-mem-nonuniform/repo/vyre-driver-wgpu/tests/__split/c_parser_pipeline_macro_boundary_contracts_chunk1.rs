// Adversarial contract tests for C preprocessor macro boundaries.
//
// Covers: expansion at stream edges, zero-length replacements, capacity
// boundaries, hash-table collisions, conditional-mask invariants, and GPU/CPU
// parity. Every overflow or malformed boundary must fail loudly; no silent
// default outputs are permitted.

// cfg(feature = "c-parser")  -  moved to parent

use std::panic::{catch_unwind, AssertUnwindSafe};

#[path = "../c_ast_gpu_parity_support/mod.rs"]
mod c_ast_gpu_parity_support;

use c_ast_gpu_parity_support::dispatch_gpu_program;
use vyre::ir::Expr;
use vyre_libs::parsing::c::lex::tokens::*;
use vyre_libs::parsing::c::preprocess::expansion::{
    opt_conditional_mask, opt_dynamic_macro_expansion,
};
use vyre_reference::value::Value;

// ---------------------------------------------------------------------------
// Helpers
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
    let values = [
        Value::from(u32_bytes(tok_types)),
        Value::from(vec![0u8; tok_types.len() * 4]),
    ];
    vyre_reference::reference_eval(&program, &values)
}

fn run_gpu_macro_expansion(
    input: &[u32],
    fixture: &MacroFixture,
    max_out_tokens: u32,
) -> Vec<Vec<u8>> {
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
    let input_bytes = u32_bytes(input);
    let keys_bytes = u32_bytes(&fixture.keys);
    let vals_bytes = u32_bytes(&fixture.vals);
    let sizes_bytes = u32_bytes(&fixture.sizes);
    let out_tok_types = vec![0u8; max_out_tokens as usize * 4];
    let out_tok_counts = vec![0u8; 4];
    dispatch_gpu_program(
        "GPU macro expansion",
        program,
        vec![
            input_bytes,
            keys_bytes,
            vals_bytes,
            sizes_bytes,
            out_tok_types,
            out_tok_counts,
        ],
    )
}

// ---------------------------------------------------------------------------
// 1. Macro expansion at stream boundaries
// ---------------------------------------------------------------------------

#[test]
fn macro_expansion_at_stream_start_replaces_first_token() {
    let mut fixture = MacroFixture::empty();
    fixture.insert(TOK_IDENTIFIER, 512, &[TOK_INTEGER, TOK_PLUS, TOK_INTEGER]);

    let outputs = run_dynamic_macro_expansion(&[TOK_IDENTIFIER, TOK_SEMICOLON], &fixture, 8)
        .expect("expansion at start must succeed");
    let out_tokens = decode_u32_words(&outputs[0].to_bytes());
    let out_count = decode_u32_words(&outputs[1].to_bytes());
    assert_eq!(
        &out_tokens[..4],
        &[TOK_INTEGER, TOK_PLUS, TOK_INTEGER, TOK_SEMICOLON]
    );
    assert_eq!(out_count, vec![4]);
}

#[test]
fn macro_expansion_at_stream_end_replaces_last_token() {
    let mut fixture = MacroFixture::empty();
    fixture.insert(TOK_IDENTIFIER, 512, &[TOK_STAR, TOK_INTEGER]);

    let outputs = run_dynamic_macro_expansion(&[TOK_INT, TOK_IDENTIFIER], &fixture, 8)
        .expect("expansion at end must succeed");
    let out_tokens = decode_u32_words(&outputs[0].to_bytes());
    let out_count = decode_u32_words(&outputs[1].to_bytes());
    assert_eq!(&out_tokens[..3], &[TOK_INT, TOK_STAR, TOK_INTEGER]);
    assert_eq!(out_count, vec![3]);
}

#[test]
fn macro_expansion_of_every_token_in_stream() {
    let mut fixture = MacroFixture::empty();
    fixture.insert(TOK_IDENTIFIER, 512, &[TOK_INTEGER]);
    fixture.insert(TOK_INT, 513, &[TOK_VOID]);

    let outputs =
        run_dynamic_macro_expansion(&[TOK_INT, TOK_IDENTIFIER, TOK_IDENTIFIER], &fixture, 8)
            .expect("all-token expansion must succeed");
    let out_tokens = decode_u32_words(&outputs[0].to_bytes());
    let out_count = decode_u32_words(&outputs[1].to_bytes());
    assert_eq!(&out_tokens[..3], &[TOK_VOID, TOK_INTEGER, TOK_INTEGER]);
    assert_eq!(out_count, vec![3]);
}

// ---------------------------------------------------------------------------
// 2. Zero-length and exact-capacity replacements
// ---------------------------------------------------------------------------

#[test]
fn macro_zero_length_replacement_removes_token() {
    let mut fixture = MacroFixture::empty();
    fixture.insert(TOK_IDENTIFIER, 512, &[]);

    let outputs =
        run_dynamic_macro_expansion(&[TOK_INT, TOK_IDENTIFIER, TOK_SEMICOLON], &fixture, 8)
            .expect("zero-length replacement must succeed");
    let out_tokens = decode_u32_words(&outputs[0].to_bytes());
    let out_count = decode_u32_words(&outputs[1].to_bytes());
    assert_eq!(&out_tokens[..2], &[TOK_INT, TOK_SEMICOLON]);
    assert_eq!(out_count, vec![2]);
}

#[test]
fn macro_expansion_exactly_at_max_out_tokens_boundary() {
    let mut fixture = MacroFixture::empty();
    fixture.insert(TOK_IDENTIFIER, 512, &[TOK_INTEGER, TOK_PLUS, TOK_INTEGER]);

    // Two expansions × 3 tokens = 6, exactly at max_out_tokens.
    let outputs = run_dynamic_macro_expansion(&[TOK_IDENTIFIER, TOK_IDENTIFIER], &fixture, 6)
        .expect("exactly-at-capacity expansion must succeed");
    let out_tokens = decode_u32_words(&outputs[0].to_bytes());
    let out_count = decode_u32_words(&outputs[1].to_bytes());
    assert_eq!(
        &out_tokens[..6],
        &[
            TOK_INTEGER,
            TOK_PLUS,
            TOK_INTEGER,
            TOK_INTEGER,
            TOK_PLUS,
            TOK_INTEGER
        ]
    );
    assert_eq!(out_count, vec![6]);
}

#[test]
fn macro_expansion_one_past_max_out_tokens_fails() {
    let mut fixture = MacroFixture::empty();
    fixture.insert(TOK_IDENTIFIER, 512, &[TOK_INTEGER, TOK_PLUS, TOK_INTEGER]);

    let result = catch_unwind(AssertUnwindSafe(|| {
        run_dynamic_macro_expansion(&[TOK_IDENTIFIER, TOK_IDENTIFIER], &fixture, 5)
    }));
    let eval_result = result.expect("overflow must return error, not panic");
    assert!(
        matches!(eval_result, Err(_)),
        "two 3-token expansions into 5 slots must fail"
    );
}

// ---------------------------------------------------------------------------
// 3. Hash-table collision handling
// ---------------------------------------------------------------------------

#[test]
fn macro_table_hash_collision_last_insert_wins() {
    // Find two distinct tokens that hash to the same slot.
    let a = TOK_IDENTIFIER;
    let mut b = TOK_IDENTIFIER + 1;
    while hash_token(a) != hash_token(b) && b < u32::MAX {
        b += 1;
    }
    assert_ne!(a, b, "must find a colliding distinct token");

    let mut fixture = MacroFixture::empty();
    fixture.insert(a, 512, &[TOK_INTEGER]);
    // b hashes to the same slot and overwrites a.
    fixture.insert(b, 513, &[TOK_STAR]);

    // Only b's mapping survives because the table has one slot per hash bucket.
    let outputs =
        run_dynamic_macro_expansion(&[a, b], &fixture, 8).expect("collision handling must succeed");
    let out_tokens = decode_u32_words(&outputs[0].to_bytes());
    let out_count = decode_u32_words(&outputs[1].to_bytes());
    // a passes through (unmapped), b expands to TOK_STAR.
    assert_eq!(&out_tokens[..2], &[a, TOK_STAR]);
    assert_eq!(out_count, vec![2]);
}

#[test]
fn macro_table_overwrite_same_slot_replaces_mapping() {
    let mut fixture = MacroFixture::empty();
    fixture.insert(TOK_IDENTIFIER, 512, &[TOK_INTEGER]);
    // Overwrite with a different replacement at a different offset.
    fixture.insert(TOK_IDENTIFIER, 514, &[TOK_STAR, TOK_STAR]);

    let outputs = run_dynamic_macro_expansion(&[TOK_IDENTIFIER], &fixture, 8)
        .expect("overwrite must succeed");
    let out_tokens = decode_u32_words(&outputs[0].to_bytes());
    let out_count = decode_u32_words(&outputs[1].to_bytes());
    // The second insert wins (same slot, same key).
    assert_eq!(&out_tokens[..2], &[TOK_STAR, TOK_STAR]);
    assert_eq!(out_count, vec![2]);
}

// ---------------------------------------------------------------------------
// 4. Conditional mask contracts
// ---------------------------------------------------------------------------

#[test]
fn conditional_mask_on_empty_stream_fails_validation() {
    let result = catch_unwind(AssertUnwindSafe(|| run_conditional_mask(&[])));
    let eval_result = result.expect("empty conditional mask must return an error, not panic");
    assert!(
        matches!(eval_result, Err(_)),
        "zero-length token stream must fail validation through the engine, not silently succeed"
    );
}

#[test]
fn conditional_mask_on_single_token_emits_one() {
    let outputs = run_conditional_mask(&[TOK_IDENTIFIER]).expect("single-token mask must succeed");
    let mask = decode_u32_words(&outputs[0].to_bytes());
    assert_eq!(mask, vec![1], "single token must get mask value 1");
}

#[test]
fn conditional_mask_on_all_preproc_tokens_emits_all_ones() {
    let inputs = vec![TOK_PREPROC; 64];
    let outputs = run_conditional_mask(&inputs).expect("mask must succeed");
    let mask = decode_u32_words(&outputs[0].to_bytes());
    assert_eq!(mask.len(), 64);
    assert!(
        mask.iter().all(|&v| v == 1),
        "all preproc tokens must get mask 1"
    );
}

#[test]
fn conditional_mask_on_mixed_token_types_emits_all_ones() {
    let inputs = vec![
        TOK_IDENTIFIER,
        TOK_INTEGER,
        TOK_HASH,
        TOK_PREPROC,
        TOK_STRING,
        TOK_CHAR,
        TOK_PLUS,
        TOK_MINUS,
        TOK_STAR,
        TOK_ASSIGN,
    ];
    let outputs = run_conditional_mask(&inputs).expect("mask must succeed");
    let mask = decode_u32_words(&outputs[0].to_bytes());
    assert_eq!(mask.len(), inputs.len());
    assert!(
        mask.iter().all(|&v| v == 1),
        "conditional mask must emit all-ones for every token type"
    );
}

#[test]
fn conditional_mask_output_length_equals_input_length() {
    let inputs: Vec<u32> = (0..100).map(|i| TOK_IDENTIFIER + (i % 50)).collect();
    let outputs = run_conditional_mask(&inputs).expect("mask must succeed");
    let mask = decode_u32_words(&outputs[0].to_bytes());
    assert_eq!(
        mask.len(),
        inputs.len(),
        "mask length must equal input token count"
    );
}

// ---------------------------------------------------------------------------
// 5. Macro boundary with adjacent punctuation
// ---------------------------------------------------------------------------

#[test]
fn macro_adjacent_to_punctuation_preserves_order() {
    let mut fixture = MacroFixture::empty();
    fixture.insert(TOK_IDENTIFIER, 512, &[TOK_INTEGER, TOK_STAR]);

    // Input: IDENTIFIER + IDENTIFIER * IDENTIFIER
    let input = [
        TOK_IDENTIFIER,
        TOK_PLUS,
        TOK_IDENTIFIER,
        TOK_STAR,
        TOK_IDENTIFIER,
    ];
    let outputs = run_dynamic_macro_expansion(&input, &fixture, 16)
        .expect("adjacent punctuation must survive");
    let out_tokens = decode_u32_words(&outputs[0].to_bytes());
    let out_count = decode_u32_words(&outputs[1].to_bytes());
    assert_eq!(out_count, vec![8]);
    assert_eq!(
        &out_tokens[..8],
        &[
            TOK_INTEGER,
            TOK_STAR,
            TOK_PLUS,
            TOK_INTEGER,
            TOK_STAR,
            TOK_STAR,
            TOK_INTEGER,
            TOK_STAR
        ]
    );
}

#[test]
fn macro_replacement_produces_correct_count_for_mixed_expansion() {
    let mut fixture = MacroFixture::empty();
    fixture.insert(TOK_IDENTIFIER, 512, &[TOK_INTEGER, TOK_INTEGER]);
    fixture.insert(TOK_STAR, 514, &[TOK_PLUS]);

    let input = [TOK_IDENTIFIER, TOK_STAR, TOK_IDENTIFIER];
    let outputs =
        run_dynamic_macro_expansion(&input, &fixture, 16).expect("mixed expansion must succeed");
    let out_count = decode_u32_words(&outputs[1].to_bytes());
    // ID -> 2 tokens, STAR -> 1 token, ID -> 2 tokens = 5
    assert_eq!(out_count, vec![5]);
}

// ---------------------------------------------------------------------------
// 6. Determinism and idempotence
// ---------------------------------------------------------------------------

#[test]
fn dynamic_macro_expansion_is_deterministic_across_identical_runs() {
    let mut fixture = MacroFixture::empty();
    fixture.insert(TOK_IDENTIFIER, 512, &[TOK_INTEGER, TOK_STAR, TOK_INTEGER]);
    let input = [TOK_IDENTIFIER, TOK_PLUS, TOK_IDENTIFIER];

    let out_a = run_dynamic_macro_expansion(&input, &fixture, 16).unwrap();
    let out_b = run_dynamic_macro_expansion(&input, &fixture, 16).unwrap();
    let out_c = run_dynamic_macro_expansion(&input, &fixture, 16).unwrap();

    assert_eq!(
        decode_u32_words(&out_a[0].to_bytes()),
        decode_u32_words(&out_b[0].to_bytes())
    );
    assert_eq!(
        decode_u32_words(&out_b[0].to_bytes()),
        decode_u32_words(&out_c[0].to_bytes())
    );
    assert_eq!(
        decode_u32_words(&out_a[1].to_bytes()),
        decode_u32_words(&out_b[1].to_bytes())
    );
}
