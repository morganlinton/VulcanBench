//! Kernel-style C AST corpus coverage for parser constructs that are common in
//! systems code but are not Linux-specific.

#![cfg(feature = "c-parser")]
#![allow(deprecated)]
#[path = "c_ast_gpu_parity_support/mod.rs"]
mod c_ast_gpu_parity_support;

use c_ast_gpu_parity_support::{
    run_gpu_classifier, run_gpu_scoped_typedef_annotation, run_gpu_vast_builder_from_parts,
};
use c_grammar_gen::lex_c11_max_munch_kinds;
use vyre_libs::parsing::c::lex::tokens::*;
use vyre_libs::parsing::c::parse::vast::{
    reference_c11_annotate_typedef_names, reference_c11_build_vast_nodes,
    reference_c11_classify_vast_node_kinds, C_AST_KIND_BREAK_STMT, C_AST_KIND_CASE_STMT,
    C_AST_KIND_CAST_EXPR, C_AST_KIND_DEFAULT_STMT, C_AST_KIND_FUNCTION_DECLARATOR,
    C_AST_KIND_GNU_ATTRIBUTE, C_AST_KIND_GOTO_STMT, C_AST_KIND_LABEL_STMT, C_AST_KIND_POINTER_DECL,
    C_AST_KIND_RETURN_STMT, C_AST_KIND_SIZEOF_EXPR, C_AST_KIND_SWITCH_STMT,
};
use vyre_primitives::predicate::node_kind;

const VAST_STRIDE_U32: usize = 10;
const TYPEDEF_FLAGS_FIELD: usize = 7;
const TYPEDEF_FLAG_VISIBLE: u32 = 1;
const TYPEDEF_FLAG_DECL: u32 = 1 << 1;
const ORDINARY_FLAG_DECL: u32 = 1 << 2;

mod common;
use common::c_fixture::*;

fn fixture_token_stream() -> Fixture {
    let tokens = [
        FixtureToken::new(
            "#define READ_ONCE(x) (*(volatile typeof(x) *)&(x))\n",
            TOK_PREPROC,
        ),
        FixtureToken::new(
            "#define fallthrough __attribute__((fallthrough))\n",
            TOK_PREPROC,
        ),
        FixtureToken::new("typedef", TOK_IDENTIFIER),
        FixtureToken::new("unsigned", TOK_IDENTIFIER),
        FixtureToken::new("long", TOK_IDENTIFIER),
        FixtureToken::new("word_t", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("typedef", TOK_IDENTIFIER),
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("*", TOK_STAR),
        FixtureToken::new("probe_cb_t", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("volatile", TOK_IDENTIFIER),
        FixtureToken::new("struct", TOK_IDENTIFIER),
        FixtureToken::new("device", TOK_IDENTIFIER),
        FixtureToken::new("*", TOK_STAR),
        FixtureToken::new("dev", TOK_IDENTIFIER),
        FixtureToken::new(",", TOK_COMMA),
        FixtureToken::new("_Atomic", TOK_IDENTIFIER),
        FixtureToken::new("unsigned", TOK_IDENTIFIER),
        FixtureToken::new("long", TOK_IDENTIFIER),
        FixtureToken::new("*", TOK_STAR),
        FixtureToken::new("state", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("static", TOK_IDENTIFIER),
        FixtureToken::new("inline", TOK_IDENTIFIER),
        FixtureToken::new("__attribute__", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("always_inline", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new("dispatch", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("struct", TOK_IDENTIFIER),
        FixtureToken::new("device", TOK_IDENTIFIER),
        FixtureToken::new("*", TOK_STAR),
        FixtureToken::new("dev", TOK_IDENTIFIER),
        FixtureToken::new(",", TOK_COMMA),
        FixtureToken::new("volatile", TOK_IDENTIFIER),
        FixtureToken::new("unsigned", TOK_IDENTIFIER),
        FixtureToken::new("long", TOK_IDENTIFIER),
        FixtureToken::new("*", TOK_STAR),
        FixtureToken::new("flags", TOK_IDENTIFIER),
        FixtureToken::new(",", TOK_COMMA),
        FixtureToken::new("probe_cb_t", TOK_IDENTIFIER),
        FixtureToken::new("cb", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("word_t", TOK_IDENTIFIER),
        FixtureToken::new("saved", TOK_IDENTIFIER),
        FixtureToken::new("=", TOK_ASSIGN),
        FixtureToken::new("sizeof", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("word_t", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new("expr_sz", TOK_IDENTIFIER),
        FixtureToken::new("=", TOK_ASSIGN),
        FixtureToken::new("sizeof", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("*", TOK_STAR),
        FixtureToken::new("flags", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new("word_t", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("word_t", TOK_IDENTIFIER),
        FixtureToken::new("=", TOK_ASSIGN),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("volatile", TOK_IDENTIFIER),
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new("tmp", TOK_IDENTIFIER),
        FixtureToken::new("=", TOK_ASSIGN),
        FixtureToken::new("1", TOK_INTEGER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("tmp", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
        FixtureToken::new("word_t", TOK_IDENTIFIER),
        FixtureToken::new("restored", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("saved", TOK_IDENTIFIER),
        FixtureToken::new("=", TOK_ASSIGN),
        FixtureToken::new("sizeof", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("restored", TOK_IDENTIFIER),
        FixtureToken::new("+", TOK_PLUS),
        FixtureToken::new("expr_sz", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("again", TOK_IDENTIFIER),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new("switch", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("expr_sz", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("case", TOK_IDENTIFIER),
        FixtureToken::new("0", TOK_INTEGER),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new("expr_sz", TOK_IDENTIFIER),
        FixtureToken::new("=", TOK_ASSIGN),
        FixtureToken::new("cb", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("dev", TOK_IDENTIFIER),
        FixtureToken::new(",", TOK_COMMA),
        FixtureToken::new("flags", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("break", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("case", TOK_IDENTIFIER),
        FixtureToken::new("1", TOK_INTEGER),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new("goto", TOK_IDENTIFIER),
        FixtureToken::new("out", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("default", TOK_IDENTIFIER),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new("expr_sz", TOK_IDENTIFIER),
        FixtureToken::new("=", TOK_ASSIGN),
        FixtureToken::new("READ_ONCE", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("expr_sz", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("fallthrough", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
        FixtureToken::new("out", TOK_IDENTIFIER),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new("return", TOK_IDENTIFIER),
        FixtureToken::new("expr_sz", TOK_IDENTIFIER),
        FixtureToken::new("?", TOK_QUESTION),
        FixtureToken::new("expr_sz", TOK_IDENTIFIER),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new("saved", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
    ];

    build_fixture(&tokens)
}

fn bytes(words: &[u32]) -> Vec<u8> {
    vyre_primitives::wire::pack_u32_slice(words)
}

fn haystack_words(bytes: &[u8]) -> Vec<u8> {
    vyre_primitives::wire::pack_bytes_as_u32_slice(bytes)
}

fn word_at(buf: &[u8], word: usize) -> u32 {
    let offset = word * 4;
    u32::from_le_bytes(buf[offset..offset + 4].try_into().unwrap())
}

fn kind_at(rows: &[u8], idx: usize) -> u32 {
    word_at(rows, idx * VAST_STRIDE_U32)
}

fn flags_at(rows: &[u8], idx: usize) -> u32 {
    word_at(rows, idx * VAST_STRIDE_U32 + TYPEDEF_FLAGS_FIELD)
}

fn assert_words_eq(actual: &[u8], expected: &[u8], context: &str) {
    if actual == expected {
        return;
    }
    let actual_words = actual.len() / core::mem::size_of::<u32>();
    let expected_words = expected.len() / core::mem::size_of::<u32>();
    let limit = actual_words.min(expected_words);
    for word in 0..limit {
        let actual_word = word_at(actual, word);
        let expected_word = word_at(expected, word);
        if actual_word != expected_word {
            panic!(
                "{context}: word {word} differs: actual={actual_word}, expected={expected_word}, row={}, field={}",
                word / VAST_STRIDE_U32,
                word % VAST_STRIDE_U32
            );
        }
    }
    panic!(
        "{context}: byte lengths differ: actual={}, expected={}",
        actual.len(),
        expected.len()
    );
}

fn node_count_from_vast(rows: &[u8]) -> u32 {
    u32::try_from(rows.len() / (VAST_STRIDE_U32 * core::mem::size_of::<u32>()))
        .expect("VAST row count must fit in u32")
}

fn row_indices(rows: &[u8], kind: u32) -> Vec<usize> {
    rows.chunks_exact(VAST_STRIDE_U32 * core::mem::size_of::<u32>())
        .enumerate()
        .filter_map(|(idx, row)| {
            let row_kind = u32::from_le_bytes(row[0..4].try_into().unwrap());
            (row_kind == kind).then_some(idx)
        })
        .collect()
}

fn lexeme_indices(fix: &Fixture, lexeme: &str) -> Vec<usize> {
    fix.tok_starts
        .iter()
        .zip(&fix.tok_lens)
        .enumerate()
        .filter_map(|(idx, (start, len))| {
            let start = *start as usize;
            let end = start.saturating_add(*len as usize);
            (fix.source.as_bytes().get(start..end) == Some(lexeme.as_bytes())).then_some(idx)
        })
        .collect()
}

fn assert_flag(rows: &[u8], idx: usize, flag: u32, message: &str) {
    assert_ne!(flags_at(rows, idx) & flag, 0, "{message} at row {idx}");
}

fn assert_no_flag(rows: &[u8], idx: usize, flag: u32, message: &str) {
    assert_eq!(flags_at(rows, idx) & flag, 0, "{message} at row {idx}");
}

fn run_gpu_vast_builder(fix: &Fixture) -> Vec<u8> {
    run_gpu_vast_builder_from_parts(&fix.tok_types, &fix.tok_starts, &fix.tok_lens)
}

fn run_gpu_typedef_annotation(fix: &Fixture, raw_vast: &[u8]) -> Vec<u8> {
    run_gpu_scoped_typedef_annotation(fix.source.as_bytes(), raw_vast)
}

mod c_ast_kernel_style_corpus_part1 {

    include!("__split/c_ast_kernel_style_corpus_part1.rs");
}
