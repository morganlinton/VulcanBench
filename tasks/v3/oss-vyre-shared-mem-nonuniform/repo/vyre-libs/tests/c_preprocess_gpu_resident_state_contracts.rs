//! GPU-resident preprocessor state contracts for the C parser megakernel.
//!
//! Table-driven assertions covering:
//! - macro table arena (slot geometry, probe discipline, empty-sentinel)
//! - function-like macro arg arena (bound tracking, parameter substitution)
//! - conditional stack (depth mask, active/taken bits, nesting overflow)
//! - directive metadata (kind token IDs, payload evaluation, phase-2 splicing)
//! - expansion queue (warp-base accumulation, source-ordered emission)
//! - overflow diagnostics (every trap path must fail loudly, not panic)
//! - collision-safe macro names (FNV-1a + byte-exact verification)
#![cfg(feature = "c-parser")]
#![allow(deprecated)]

mod common;
use common::{decode_u32_words, u32_bytes};
use std::panic::{catch_unwind, AssertUnwindSafe};
use vyre::ir::Expr;
use vyre_libs::parsing::c::lex::tokens::*;
use vyre_libs::parsing::c::preprocess::expansion::{
    opt_conditional_mask, opt_conditional_mask_with_directives, opt_dynamic_macro_expansion,
    opt_named_macro_expansion, C_MACRO_KIND_FUNCTION_LIKE, C_MACRO_KIND_OBJECT_LIKE,
    C_MACRO_REPLACEMENT_LITERAL,
};
use vyre_libs::parsing::c::preprocess::{
    c_translation_phase_line_splice, reference_c_preprocessor_directive_metadata,
    CPreprocessorDirectiveKind,
};
use vyre_reference::value::Value;
// ---------------------------------------------------------------------------
// Constants mirroring the megakernel state layout
// ---------------------------------------------------------------------------
const EMPTY_SLOT: u32 = u32::MAX;
const TABLE_SLOTS: usize = 4096;
const TABLE_MASK: u32 = 4095;
#[allow(dead_code)]
const MAX_FN_ARGS: u32 = 16;
const NAME_POOL_BYTES: usize = 16_384;
const FNV1A32_OFFSET: u32 = 0x811c_9dc5;
const FNV1A32_PRIME: u32 = 0x0100_0193;
// ---------------------------------------------------------------------------
// Byte / word helpers
// ---------------------------------------------------------------------------
fn source_words(source: &[u8]) -> Vec<u32> {
    source.iter().map(|b| u32::from(*b)).collect()
}
fn fnv1a32(bytes: &[u8]) -> u32 {
    let mut hash = FNV1A32_OFFSET;
    for byte in bytes {
        hash ^= u32::from(*byte);
        hash = hash.wrapping_mul(FNV1A32_PRIME);
    }
    hash
}
fn macro_slot(hash: u32) -> usize {
    (hash.wrapping_mul(2_654_435_769) & TABLE_MASK) as usize
}
fn hash_token(tok: u32) -> usize {
    (tok.wrapping_mul(2_654_435_769) & TABLE_MASK) as usize
}
// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------
#[derive(Clone)]
struct DynamicFixture {
    keys: Vec<u32>,
    vals: Vec<u32>,
    sizes: Vec<u32>,
}
impl DynamicFixture {
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
#[derive(Clone)]
struct NamedFixture {
    name_hashes: Vec<u32>,
    name_starts: Vec<u32>,
    name_lens: Vec<u32>,
    name_words: Vec<u32>,
    vals: Vec<u32>,
    sizes: Vec<u32>,
    kinds: Vec<u32>,
    param_counts: Vec<u32>,
    replacement_params: Vec<u32>,
    next_name_offset: usize,
}
impl NamedFixture {
    fn empty() -> Self {
        Self {
            name_hashes: vec![EMPTY_SLOT; TABLE_SLOTS],
            name_starts: vec![0; TABLE_SLOTS],
            name_lens: vec![0; TABLE_SLOTS],
            name_words: vec![0; NAME_POOL_BYTES],
            vals: vec![0; TABLE_SLOTS],
            sizes: vec![0; TABLE_SLOTS],
            kinds: vec![C_MACRO_KIND_OBJECT_LIKE; TABLE_SLOTS],
            param_counts: vec![0; TABLE_SLOTS],
            replacement_params: vec![C_MACRO_REPLACEMENT_LITERAL; TABLE_SLOTS],
            next_name_offset: 0,
        }
    }
    fn install_name(&mut self, slot: usize, name: &[u8]) {
        let start = self.next_name_offset;
        let end = start + name.len();
        assert!(
            end <= self.name_words.len(),
            "test macro-name pool exceeded fixed fixture capacity"
        );
        for (idx, byte) in name.iter().enumerate() {
            self.name_words[start + idx] = u32::from(*byte);
        }
        self.name_starts[slot] = start as u32;
        self.name_lens[slot] = name.len() as u32;
        self.next_name_offset = end;
    }
    fn insert(
        &mut self,
        name: &[u8],
        replacement_offset: usize,
        kind: u32,
        param_count: u32,
        replacement: &[(u32, u32)],
    ) {
        let name_hash = fnv1a32(name);
        let mut slot = macro_slot(name_hash);
        while self.name_hashes[slot] != EMPTY_SLOT {
            slot = (slot + 1) & (TABLE_SLOTS - 1);
        }
        self.name_hashes[slot] = name_hash;
        self.install_name(slot, name);
        self.vals[slot] = replacement_offset as u32;
        self.kinds[slot] = kind;
        self.param_counts[slot] = param_count;
        self.sizes[replacement_offset] = replacement.len() as u32;
        for (idx, (tok, param)) in replacement.iter().enumerate() {
            self.vals[replacement_offset + idx] = *tok;
            self.replacement_params[replacement_offset + idx] = *param;
        }
    }
    fn insert_at_slot_with_hash(
        &mut self,
        slot: usize,
        hash: u32,
        name: &[u8],
        replacement_offset: usize,
        kind: u32,
        replacement: &[(u32, u32)],
    ) {
        assert_eq!(self.name_hashes[slot], EMPTY_SLOT);
        self.name_hashes[slot] = hash;
        self.install_name(slot, name);
        self.vals[slot] = replacement_offset as u32;
        self.kinds[slot] = kind;
        self.param_counts[slot] = 0;
        self.sizes[replacement_offset] = replacement.len() as u32;
        for (idx, (tok, param)) in replacement.iter().enumerate() {
            self.vals[replacement_offset + idx] = *tok;
            self.replacement_params[replacement_offset + idx] = *param;
        }
    }
}
struct TokenStream<'a> {
    source: &'a [u8],
    types: Vec<u32>,
    starts: Vec<u32>,
    lens: Vec<u32>,
}
// ---------------------------------------------------------------------------
// Runners
// ---------------------------------------------------------------------------
fn run_dynamic(
    input: &[u32],
    fixture: &DynamicFixture,
    max_out: u32,
) -> Result<Vec<Value>, vyre::Error> {
    let program = opt_dynamic_macro_expansion(
        "in_tok_types",
        "macro_keys",
        "macro_vals",
        "macro_sizes",
        "out_tok_types",
        "out_tok_counts",
        Expr::u32(input.len() as u32),
        max_out,
    );
    let input_bytes = if input.is_empty() {
        vec![0u8; 4]
    } else {
        u32_bytes(input)
    };
    let values = [
        Value::from(input_bytes),
        Value::from(u32_bytes(&fixture.keys)),
        Value::from(u32_bytes(&fixture.vals)),
        Value::from(u32_bytes(&fixture.sizes)),
        Value::from(vec![0u8; max_out.max(1) as usize * 4]),
        Value::from(vec![0u8; 4]),
    ];
    vyre_reference::reference_eval(&program, &values)
}
fn run_named(
    stream: &TokenStream<'_>,
    fixture: &NamedFixture,
    max_out: u32,
) -> Result<Vec<Value>, vyre::Error> {
    let program = opt_named_macro_expansion(
        "in_tok_types",
        "in_tok_starts",
        "in_tok_lens",
        "source_words",
        "macro_name_hashes",
        "macro_name_starts",
        "macro_name_lens",
        "macro_name_words",
        "macro_vals",
        "macro_sizes",
        "macro_kinds",
        "macro_param_counts",
        "macro_replacement_params",
        "out_tok_types",
        "out_tok_counts",
        Expr::u32(stream.types.len() as u32),
        Expr::u32(stream.source.len() as u32),
        max_out,
    );
    let values = [
        Value::from(u32_bytes(&stream.types)),
        Value::from(u32_bytes(&stream.starts)),
        Value::from(u32_bytes(&stream.lens)),
        Value::from(u32_bytes(&source_words(stream.source))),
        Value::from(u32_bytes(&fixture.name_hashes)),
        Value::from(u32_bytes(&fixture.name_starts)),
        Value::from(u32_bytes(&fixture.name_lens)),
        Value::from(u32_bytes(&fixture.name_words)),
        Value::from(u32_bytes(&fixture.vals)),
        Value::from(u32_bytes(&fixture.sizes)),
        Value::from(u32_bytes(&fixture.kinds)),
        Value::from(u32_bytes(&fixture.param_counts)),
        Value::from(u32_bytes(&fixture.replacement_params)),
        Value::from(vec![0u8; max_out.max(1) as usize * 4]),
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
    let values = [
        Value::from(input_bytes),
        Value::from(vec![0u8; tok_types.len().max(1) * 4]),
    ];
    vyre_reference::reference_eval(&program, &values)
}
fn run_conditional_mask_with_directives(
    tok_types: &[u32],
    directive_kinds: &[u32],
    directive_values: &[u32],
) -> Result<Vec<Value>, vyre::Error> {
    let program = opt_conditional_mask_with_directives(
        "tok_types",
        "directive_kinds",
        "directive_values",
        "out_mask",
        Expr::u32(tok_types.len() as u32),
    );
    let values = [
        Value::from(u32_bytes(tok_types)),
        Value::from(u32_bytes(directive_kinds)),
        Value::from(u32_bytes(directive_values)),
        Value::from(vec![0u8; tok_types.len() * 4]),
    ];
    vyre_reference::reference_eval(&program, &values)
}
// ---------------------------------------------------------------------------
// 1. Macro table arena contracts
// ---------------------------------------------------------------------------
mod c_preprocess_gpu_resident_state_contracts_part1 {
    include!("__split/c_preprocess_gpu_resident_state_contracts_part1.rs");
}
mod c_preprocess_gpu_resident_state_contracts_part2 {
    include!("__split/c_preprocess_gpu_resident_state_contracts_part2.rs");
}
mod c_preprocess_gpu_resident_state_contracts_part3 {
    include!("__split/c_preprocess_gpu_resident_state_contracts_part3.rs");
}
mod c_preprocess_gpu_resident_state_contracts_part4 {
    include!("__split/c_preprocess_gpu_resident_state_contracts_part4.rs");
}
mod c_preprocess_gpu_resident_state_contracts_part5 {
    include!("__split/c_preprocess_gpu_resident_state_contracts_part5.rs");
}
mod c_preprocess_gpu_resident_state_contracts_part6 {
    include!("__split/c_preprocess_gpu_resident_state_contracts_part6.rs");
}
mod c_preprocess_gpu_resident_state_contracts_part7 {
    include!("__split/c_preprocess_gpu_resident_state_contracts_part7.rs");
}
mod c_preprocess_gpu_resident_state_contracts_part8 {
    include!("__split/c_preprocess_gpu_resident_state_contracts_part8.rs");
}
mod c_preprocess_gpu_resident_state_contracts_part9 {
    include!("__split/c_preprocess_gpu_resident_state_contracts_part9.rs");
}
mod c_preprocess_gpu_resident_state_contracts_part10 {
    include!("__split/c_preprocess_gpu_resident_state_contracts_part10.rs");
}
mod c_preprocess_gpu_resident_state_contracts_part11 {
    include!("__split/c_preprocess_gpu_resident_state_contracts_part11.rs");
}
mod c_preprocess_gpu_resident_state_contracts_part12 {
    include!("__split/c_preprocess_gpu_resident_state_contracts_part12.rs");
}
mod c_preprocess_gpu_resident_state_contracts_part13 {
    include!("__split/c_preprocess_gpu_resident_state_contracts_part13.rs");
}
mod c_preprocess_gpu_resident_state_contracts_part14 {
    include!("__split/c_preprocess_gpu_resident_state_contracts_part14.rs");
}
mod c_preprocess_gpu_resident_state_contracts_part15 {
    include!("__split/c_preprocess_gpu_resident_state_contracts_part15.rs");
}
mod c_preprocess_gpu_resident_state_contracts_part16 {
    include!("__split/c_preprocess_gpu_resident_state_contracts_part16.rs");
}
mod c_preprocess_gpu_resident_state_contracts_part17 {
    include!("__split/c_preprocess_gpu_resident_state_contracts_part17.rs");
}
mod c_preprocess_gpu_resident_state_contracts_part18 {
    include!("__split/c_preprocess_gpu_resident_state_contracts_part18.rs");
}
