//! Contract tests for named C preprocessor macro expansion.

#![cfg(feature = "c-parser")]
#![allow(deprecated)]

mod common;
use common::{decode_u32_words, u32_bytes};
use std::panic::{catch_unwind, AssertUnwindSafe};

use vyre::ir::Expr;
use vyre_libs::parsing::c::lex::tokens::{
    TOK_COMMA, TOK_GT, TOK_HASHHASH, TOK_IDENTIFIER, TOK_INTEGER, TOK_LPAREN, TOK_PLUS, TOK_RPAREN,
};
use vyre_libs::parsing::c::preprocess::expansion::{
    opt_named_macro_expansion, C_MACRO_KIND_FUNCTION_LIKE, C_MACRO_KIND_OBJECT_LIKE,
    C_MACRO_REPLACEMENT_LITERAL,
};
use vyre_libs::parsing::c::preprocess::synthesis::C_TOKEN_PASTE_RULES;
use vyre_reference::value::Value;

const EMPTY_SLOT: u32 = u32::MAX;
const TABLE_SLOTS: usize = 4096;
const TABLE_MASK: u32 = 4095;
const FNV1A32_OFFSET: u32 = 0x811c_9dc5;
const FNV1A32_PRIME: u32 = 0x0100_0193;

fn source_words(source: &[u8]) -> Vec<u32> {
    source.iter().map(|byte| u32::from(*byte)).collect()
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

#[derive(Clone)]
struct NamedMacroFixture {
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

impl NamedMacroFixture {
    fn empty() -> Self {
        Self {
            name_hashes: vec![EMPTY_SLOT; TABLE_SLOTS],
            name_starts: vec![0; TABLE_SLOTS],
            name_lens: vec![0; TABLE_SLOTS],
            name_words: vec![0; 16_384],
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
        if end > self.name_words.len() {
            self.name_words.resize(end, 0);
        }
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
            slot = (slot + 1) & TABLE_MASK as usize;
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

fn run_named_macro_expansion(
    stream: &TokenStream<'_>,
    fixture: &NamedMacroFixture,
    max_out_tokens: u32,
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
        max_out_tokens,
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
        Value::from(vec![0u8; max_out_tokens.max(1) as usize * 4]),
        Value::from(vec![0u8; 4]),
    ];
    vyre_reference::reference_eval(&program, &values)
}

#[test]
fn hash_collision_requires_byte_exact_macro_name_match() {
    let stream = TokenStream {
        source: b"FOO",
        types: vec![TOK_IDENTIFIER],
        starts: vec![0],
        lens: vec![3],
    };
    let foo_hash = fnv1a32(b"FOO");
    let first_slot = macro_slot(foo_hash);
    let second_slot = (first_slot + 1) & TABLE_MASK as usize;
    let mut fixture = NamedMacroFixture::empty();
    fixture.insert_at_slot_with_hash(
        first_slot,
        foo_hash,
        b"BAR",
        512,
        C_MACRO_KIND_OBJECT_LIKE,
        &[(TOK_PLUS, C_MACRO_REPLACEMENT_LITERAL)],
    );
    fixture.insert_at_slot_with_hash(
        second_slot,
        foo_hash,
        b"FOO",
        520,
        C_MACRO_KIND_OBJECT_LIKE,
        &[(TOK_INTEGER, C_MACRO_REPLACEMENT_LITERAL)],
    );

    let outputs = run_named_macro_expansion(&stream, &fixture, 4)
        .expect("collision-safe named macro expansion must probe past nonmatching names");
    let out_tokens = decode_u32_words(&outputs[0].to_bytes());
    let out_count = decode_u32_words(&outputs[1].to_bytes());
    assert_eq!(out_count, vec![1]);
    assert_eq!(out_tokens[0], TOK_INTEGER);
}

#[test]
fn named_macro_expansion_matches_name_beyond_legacy_16k_pool() {
    let name = vec![b'A'; 16_384 + 257];
    let stream = TokenStream {
        source: &name,
        types: vec![TOK_IDENTIFIER],
        starts: vec![0],
        lens: vec![name.len() as u32],
    };
    let mut fixture = NamedMacroFixture::empty();
    fixture.insert(
        &name,
        512,
        C_MACRO_KIND_OBJECT_LIKE,
        0,
        &[(TOK_INTEGER, C_MACRO_REPLACEMENT_LITERAL)],
    );

    let outputs = run_named_macro_expansion(&stream, &fixture, 4)
        .expect("runtime-sized macro-name pool must expand names beyond the legacy 16 KiB cap");
    let out_tokens = decode_u32_words(&outputs[0].to_bytes());
    let out_count = decode_u32_words(&outputs[1].to_bytes());

    assert_eq!(out_count, vec![1]);
    assert_eq!(out_tokens[0], TOK_INTEGER);
}

#[test]
fn object_like_macro_matches_identifier_name_hash_not_identifier_token_kind() {
    let stream = TokenStream {
        source: b"FOO + BAR FOO",
        types: vec![TOK_IDENTIFIER, TOK_PLUS, TOK_IDENTIFIER, TOK_IDENTIFIER],
        starts: vec![0, 4, 6, 10],
        lens: vec![3, 1, 3, 3],
    };
    let mut fixture = NamedMacroFixture::empty();
    fixture.insert(
        b"FOO",
        512,
        C_MACRO_KIND_OBJECT_LIKE,
        0,
        &[(TOK_INTEGER, C_MACRO_REPLACEMENT_LITERAL)],
    );

    let outputs = run_named_macro_expansion(&stream, &fixture, 8)
        .expect("named object-like macro expansion must succeed");
    let out_tokens = decode_u32_words(&outputs[0].to_bytes());
    let out_count = decode_u32_words(&outputs[1].to_bytes());
    assert_eq!(
        &out_tokens[..4],
        &[TOK_INTEGER, TOK_PLUS, TOK_IDENTIFIER, TOK_INTEGER]
    );
    assert_eq!(out_count, vec![4]);
}

#[test]
fn object_like_token_paste_generated_rules_synthesize_exact_tokens() {
    for (idx, (left, right, out)) in C_TOKEN_PASTE_RULES.iter().enumerate() {
        let name = format!("M{idx}");
        let stream = TokenStream {
            source: name.as_bytes(),
            types: vec![TOK_IDENTIFIER],
            starts: vec![0],
            lens: vec![name.len() as u32],
        };
        let mut fixture = NamedMacroFixture::empty();
        fixture.insert(
            name.as_bytes(),
            512,
            C_MACRO_KIND_OBJECT_LIKE,
            0,
            &[
                (*left, C_MACRO_REPLACEMENT_LITERAL),
                (TOK_HASHHASH, C_MACRO_REPLACEMENT_LITERAL),
                (*right, C_MACRO_REPLACEMENT_LITERAL),
            ],
        );

        let outputs = run_named_macro_expansion(&stream, &fixture, 4)
            .expect("object-like token paste rule must synthesize deterministically");
        let out_tokens = decode_u32_words(&outputs[0].to_bytes());
        let out_count = decode_u32_words(&outputs[1].to_bytes());
        assert_eq!(out_count, vec![1], "paste rule {idx} must emit one token");
        assert_eq!(
            out_tokens[0], *out,
            "paste rule {idx} must synthesize exact token"
        );
    }
}

#[test]
fn function_like_macro_expands_only_invocation_shape_and_substitutes_arguments() {
    let stream = TokenStream {
        source: b"MAX + MAX(a,b)",
        types: vec![
            TOK_IDENTIFIER,
            TOK_PLUS,
            TOK_IDENTIFIER,
            TOK_LPAREN,
            TOK_IDENTIFIER,
            TOK_COMMA,
            TOK_IDENTIFIER,
            TOK_RPAREN,
        ],
        starts: vec![0, 4, 6, 9, 10, 11, 12, 13],
        lens: vec![3, 1, 3, 1, 1, 1, 1, 1],
    };
    let mut fixture = NamedMacroFixture::empty();
    fixture.insert(
        b"MAX",
        512,
        C_MACRO_KIND_FUNCTION_LIKE,
        2,
        &[(0, 0), (TOK_GT, C_MACRO_REPLACEMENT_LITERAL), (0, 1)],
    );

    let outputs = run_named_macro_expansion(&stream, &fixture, 8)
        .expect("named function-like macro expansion must succeed");
    let out_tokens = decode_u32_words(&outputs[0].to_bytes());
    let out_count = decode_u32_words(&outputs[1].to_bytes());
    assert_eq!(out_count, vec![5]);
    assert_eq!(
        &out_tokens[..5],
        &[
            TOK_IDENTIFIER,
            TOK_PLUS,
            TOK_IDENTIFIER,
            TOK_GT,
            TOK_IDENTIFIER
        ]
    );
}

#[test]
fn function_like_macro_argument_count_mismatch_fails_loudly() {
    let stream = TokenStream {
        source: b"MAX(a)",
        types: vec![TOK_IDENTIFIER, TOK_LPAREN, TOK_IDENTIFIER, TOK_RPAREN],
        starts: vec![0, 3, 4, 5],
        lens: vec![3, 1, 1, 1],
    };
    let mut fixture = NamedMacroFixture::empty();
    fixture.insert(b"MAX", 512, C_MACRO_KIND_FUNCTION_LIKE, 2, &[(0, 0)]);

    let result = catch_unwind(AssertUnwindSafe(|| {
        run_named_macro_expansion(&stream, &fixture, 8)
    }))
    .expect("argument mismatch must return an error, not panic");
    let err = result.expect_err("MAX(a) with arity 2 must fail");
    let msg = err.to_string();
    assert!(
        msg.contains("argument") || msg.contains("Fix:") || msg.contains("MAX"),
        "argument-count mismatch error: {msg}"
    );
}

#[test]
fn function_like_macro_preserves_nested_parenthesized_argument_ranges() {
    let stream = TokenStream {
        source: b"MAX((a),b)",
        types: vec![
            TOK_IDENTIFIER,
            TOK_LPAREN,
            TOK_LPAREN,
            TOK_IDENTIFIER,
            TOK_RPAREN,
            TOK_COMMA,
            TOK_IDENTIFIER,
            TOK_RPAREN,
        ],
        starts: vec![0, 3, 4, 5, 6, 7, 8, 9],
        lens: vec![3, 1, 1, 1, 1, 1, 1, 1],
    };
    let mut fixture = NamedMacroFixture::empty();
    fixture.insert(
        b"MAX",
        512,
        C_MACRO_KIND_FUNCTION_LIKE,
        2,
        &[(0, 0), (TOK_PLUS, C_MACRO_REPLACEMENT_LITERAL), (0, 1)],
    );

    let outputs = run_named_macro_expansion(&stream, &fixture, 8)
        .expect("nested argument macro expansion must succeed");
    let out_tokens = decode_u32_words(&outputs[0].to_bytes());
    let out_count = decode_u32_words(&outputs[1].to_bytes());
    assert_eq!(out_count, vec![5]);
    assert_eq!(
        &out_tokens[..5],
        &[
            TOK_LPAREN,
            TOK_IDENTIFIER,
            TOK_RPAREN,
            TOK_PLUS,
            TOK_IDENTIFIER
        ]
    );
}

#[test]
fn named_macro_expansion_reports_capacity_overflow_deterministically() {
    let stream = TokenStream {
        source: b"FOO",
        types: vec![TOK_IDENTIFIER],
        starts: vec![0],
        lens: vec![3],
    };
    let mut fixture = NamedMacroFixture::empty();
    fixture.insert(
        b"FOO",
        512,
        C_MACRO_KIND_OBJECT_LIKE,
        0,
        &[
            (TOK_INTEGER, C_MACRO_REPLACEMENT_LITERAL),
            (TOK_PLUS, C_MACRO_REPLACEMENT_LITERAL),
            (TOK_INTEGER, C_MACRO_REPLACEMENT_LITERAL),
        ],
    );

    let result = catch_unwind(AssertUnwindSafe(|| {
        run_named_macro_expansion(&stream, &fixture, 2)
    }))
    .expect("output overflow must return an error, not panic");
    let err = result.expect_err("FOO expansion into two slots must fail");
    let msg = err.to_string();
    assert!(
        msg.contains("capacity") || msg.contains("overflow") || msg.contains("Fix:"),
        "named macro capacity overflow error: {msg}"
    );
}
