//! Integration contracts for C declarator shape extraction in structural parser stages.

#![cfg(feature = "c-parser")]

mod common;
use common::words_to_bytes;

#[path = "c_ast_gpu_parity_support/mod.rs"]
mod c_ast_gpu_parity_support;

use c_ast_gpu_parity_support::dispatch_gpu_program;
use vyre::ir::Expr;
use vyre_libs::parsing::c::lex::tokens::*;
use vyre_libs::parsing::c::parse::structure::{c11_extract_calls, c11_extract_functions};

const SENTINEL: u32 = u32::MAX;

fn bytes_to_words(bytes: &[u8]) -> Vec<u32> {
    bytes
        .chunks_exact(4)
        .map(|chunk| u32::from_le_bytes(chunk.try_into().unwrap()))
        .collect()
}

fn paired(len: usize, pairs: &[(usize, usize)]) -> Vec<u32> {
    let mut out = vec![SENTINEL; len];
    for &(left, right) in pairs {
        out[left] = right as u32;
        out[right] = left as u32;
    }
    out
}

fn run_extract_functions(tok_types: &[u32], paren_pairs: &[u32], brace_pairs: &[u32]) -> Vec<u32> {
    let program = c11_extract_functions(
        "tok_types",
        "paren_pairs",
        "brace_pairs",
        Expr::u32(tok_types.len() as u32),
        "out_functions",
        "out_counts",
    );
    let tok_bytes = words_to_bytes(tok_types);
    let paren_bytes = words_to_bytes(paren_pairs);
    let brace_bytes = words_to_bytes(brace_pairs);
    let count_bytes = words_to_bytes(&[0]);
    let outputs = dispatch_gpu_program(
        "GPU C function extraction",
        program,
        vec![tok_bytes, paren_bytes, brace_bytes, count_bytes],
    );
    assert_eq!(outputs.len(), 2);
    let mut words = bytes_to_words(&outputs[0]);
    words.push(bytes_to_words(&outputs[1])[0]);
    words
}

fn run_extract_calls(tok_types: &[u32], paren_pairs: &[u32], function_records: &[u32]) -> Vec<u32> {
    let program = c11_extract_calls(
        "tok_types",
        "paren_pairs",
        "functions",
        Expr::u32(tok_types.len() as u32),
        Expr::u32((function_records.len() / 3) as u32),
        "out_calls",
        "out_counts",
    );
    let tok_bytes = words_to_bytes(tok_types);
    let paren_bytes = words_to_bytes(paren_pairs);
    let function_bytes = words_to_bytes(function_records);
    let count_bytes = words_to_bytes(&[0]);
    let outputs = dispatch_gpu_program(
        "GPU C call extraction",
        program,
        vec![tok_bytes, paren_bytes, function_bytes, count_bytes],
    );
    assert_eq!(outputs.len(), 2);
    let mut words = bytes_to_words(&outputs[0]);
    words.push(bytes_to_words(&outputs[1])[0]);
    words
}

#[test]
fn typedef_return_function_definition_is_a_function_record() {
    let tok_types = [
        TOK_IDENTIFIER,
        TOK_IDENTIFIER,
        TOK_LPAREN,
        TOK_VOID,
        TOK_RPAREN,
        TOK_LBRACE,
        TOK_RBRACE,
    ];
    let paren_pairs = paired(tok_types.len(), &[(2, 4)]);
    let brace_pairs = paired(tok_types.len(), &[(5, 6)]);

    let out = run_extract_functions(&tok_types, &paren_pairs, &brace_pairs);

    assert_eq!(out.last().copied(), Some(3), "one 3-word function record");
    assert_eq!(
        &out[..3],
        &[1, 5, 6],
        "`typedef_name f(void) {{}}` must record f and its body span"
    );
}

#[test]
fn tagged_return_function_definition_is_a_function_record() {
    let tok_types = [
        TOK_STRUCT,
        TOK_IDENTIFIER,
        TOK_IDENTIFIER,
        TOK_LPAREN,
        TOK_VOID,
        TOK_RPAREN,
        TOK_LBRACE,
        TOK_RBRACE,
    ];
    let paren_pairs = paired(tok_types.len(), &[(3, 5)]);
    let brace_pairs = paired(tok_types.len(), &[(6, 7)]);

    let out = run_extract_functions(&tok_types, &paren_pairs, &brace_pairs);

    assert_eq!(out.last().copied(), Some(3), "one 3-word function record");
    assert_eq!(
        &out[..3],
        &[2, 6, 7],
        "`struct tag f(void) {{}}` must record f and its body span"
    );
}

#[test]
fn parenthesized_function_declarator_definition_is_a_function_record() {
    let tok_types = [
        TOK_INT,
        TOK_LPAREN,
        TOK_IDENTIFIER,
        TOK_RPAREN,
        TOK_LPAREN,
        TOK_VOID,
        TOK_RPAREN,
        TOK_LBRACE,
        TOK_RBRACE,
    ];
    let paren_pairs = paired(tok_types.len(), &[(1, 3), (4, 6)]);
    let brace_pairs = paired(tok_types.len(), &[(7, 8)]);

    let out = run_extract_functions(&tok_types, &paren_pairs, &brace_pairs);

    assert_eq!(out.last().copied(), Some(3), "one 3-word function record");
    assert_eq!(
        &out[..3],
        &[2, 7, 8],
        "`int (f)(void) {{}}` must record f and its body span"
    );
}

#[test]
fn function_pointer_declarator_is_not_a_pointer_call() {
    let tok_types = [
        TOK_INT,
        TOK_LPAREN,
        TOK_STAR,
        TOK_IDENTIFIER,
        TOK_RPAREN,
        TOK_LPAREN,
        TOK_INT,
        TOK_RPAREN,
        TOK_SEMICOLON,
    ];
    let paren_pairs = paired(tok_types.len(), &[(1, 4), (5, 7)]);
    let out = run_extract_calls(&tok_types, &paren_pairs, &[0, 0, 0]);

    assert_eq!(
        out.last().copied(),
        Some(0),
        "`int (*fp)(int);` must not emit a pointer-call record"
    );
}

#[test]
fn abstract_function_pointer_parameter_is_not_a_pointer_call() {
    let tok_types = [
        TOK_VOID,
        TOK_IDENTIFIER,
        TOK_LPAREN,
        TOK_INT,
        TOK_LPAREN,
        TOK_STAR,
        TOK_RPAREN,
        TOK_LPAREN,
        TOK_INT,
        TOK_RPAREN,
        TOK_RPAREN,
        TOK_SEMICOLON,
    ];
    let paren_pairs = paired(tok_types.len(), &[(2, 10), (4, 6), (7, 9)]);
    let out = run_extract_calls(&tok_types, &paren_pairs, &[0, 0, 0]);

    assert_eq!(
        out.last().copied(),
        Some(0),
        "`void f(int (*)(int));` must not emit a pointer-call record"
    );
}

#[test]
fn parenthesized_pointer_call_still_emits_call_record() {
    let tok_types = [
        TOK_LPAREN,
        TOK_STAR,
        TOK_IDENTIFIER,
        TOK_RPAREN,
        TOK_LPAREN,
        TOK_IDENTIFIER,
        TOK_RPAREN,
        TOK_SEMICOLON,
    ];
    let paren_pairs = paired(tok_types.len(), &[(0, 3), (4, 6)]);
    let out = run_extract_calls(&tok_types, &paren_pairs, &[0, 0, 0]);

    assert_eq!(out.last().copied(), Some(4), "one 4-word call record");
    assert_eq!(
        &out[..4],
        &[SENTINEL, 2, 4, 6],
        "`(*fp)(arg);` must keep emitting a pointer-call record"
    );
}

#[test]
fn consecutive_direct_calls_are_compacted_without_sparse_zero_clobber() {
    let tok_types = [
        TOK_IDENTIFIER,
        TOK_LPAREN,
        TOK_RPAREN,
        TOK_SEMICOLON,
        TOK_IDENTIFIER,
        TOK_LPAREN,
        TOK_RPAREN,
        TOK_SEMICOLON,
    ];
    let paren_pairs = paired(tok_types.len(), &[(1, 2), (5, 6)]);
    let out = run_extract_calls(&tok_types, &paren_pairs, &[SENTINEL, 0, 0]);

    assert_eq!(out.last().copied(), Some(8), "two compact 4-word records");
    let mut records = out[..8]
        .chunks_exact(4)
        .map(|record| [record[0], record[1], record[2], record[3]])
        .collect::<Vec<_>>();
    records.sort_by_key(|record| record[1]);
    assert_eq!(
        records
            .iter()
            .map(|record| &record[1..])
            .collect::<Vec<_>>(),
        vec![&[0, 1, 2][..], &[4, 5, 6][..]],
        "compact call records must not be overwritten by obsolete sparse row clearing"
    );
}
