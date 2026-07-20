//! Packed-haystack parity tests for C semantic GPU programs.

#![cfg(feature = "c-parser")]
#![allow(deprecated)]
mod common;

use common::decode_u32_words as words_from_bytes;
use common::u32_bytes as bytes;
use vyre::ir::Expr;
use vyre_libs::parsing::c::lex::tokens::*;
use vyre_libs::parsing::c::parse::vast::{
    c11_annotate_typedef_names, c11_annotate_typedef_names_packed_haystack,
    c11_prehash_vast_identifiers, c11_prehash_vast_identifiers_packed_haystack,
};
use vyre_libs::parsing::c::sema::registry::{c_sema_scope, c_sema_scope_packed_haystack};
use vyre_reference::value::Value;

const VAST_NODE_STRIDE_U32: usize = 10;

fn expanded_haystack(source: &[u8]) -> Vec<u8> {
    bytes(
        &source
            .iter()
            .map(|byte| u32::from(*byte))
            .collect::<Vec<_>>(),
    )
}

fn packed_haystack(source: &[u8]) -> Vec<u8> {
    let mut packed = vec![0u8; source.len().max(1).div_ceil(4) * 4];
    packed[..source.len()].copy_from_slice(source);
    packed
}

fn eval_words(program: &vyre::ir::Program, inputs: Vec<Vec<u8>>) -> Vec<u32> {
    let values = inputs.into_iter().map(Value::from).collect::<Vec<_>>();
    let outputs = vyre_reference::reference_eval(program, &values)
        .expect("semantic packed-haystack parity program must execute");
    words_from_bytes(&outputs[0].to_bytes())
}

#[test]
fn sema_scope_packed_haystack_matches_expanded_identifier_interning() {
    let source = b"int alpha;";
    let tok_types = [TOK_INT, TOK_IDENTIFIER, TOK_SEMICOLON];
    let tok_starts = [0u32, 4, 9];
    let tok_lens = [3u32, 5, 1];
    let out_init = vec![0u8; tok_types.len() * 4 * 4];

    let expanded_program = c_sema_scope(
        "tok_types",
        "tok_starts",
        "tok_lens",
        "haystack",
        Expr::u32(source.len() as u32),
        Expr::u32(tok_types.len() as u32),
        "out_scope_tree",
    );
    let packed_program = c_sema_scope_packed_haystack(
        "tok_types",
        "tok_starts",
        "tok_lens",
        "haystack",
        Expr::u32(source.len() as u32),
        Expr::u32(tok_types.len() as u32),
        "out_scope_tree",
    );

    let expanded = eval_words(
        &expanded_program,
        vec![
            bytes(&tok_types),
            bytes(&tok_starts),
            bytes(&tok_lens),
            expanded_haystack(source),
            out_init.clone(),
        ],
    );
    let packed = eval_words(
        &packed_program,
        vec![
            bytes(&tok_types),
            bytes(&tok_starts),
            bytes(&tok_lens),
            packed_haystack(source),
            out_init,
        ],
    );

    assert_eq!(
        packed, expanded,
        "packed semantic scope/interning must match expanded haystack semantics"
    );
    assert_ne!(
        packed[1 * 4 + 3],
        0,
        "identifier interning should hash the identifier source bytes"
    );
}

#[test]
fn typedef_annotation_packed_haystack_matches_expanded_source_hashes() {
    let source = b"typedef int foo; foo bar;";
    let tokens = [
        (TOK_TYPEDEF, 0u32, 7u32),
        (TOK_INT, 8, 3),
        (TOK_IDENTIFIER, 12, 3),
        (TOK_SEMICOLON, 15, 1),
        (TOK_IDENTIFIER, 17, 3),
        (TOK_IDENTIFIER, 21, 3),
        (TOK_SEMICOLON, 24, 1),
    ];
    let mut vast = vec![0u32; tokens.len() * VAST_NODE_STRIDE_U32];
    for (idx, (kind, start, len)) in tokens.iter().copied().enumerate() {
        let base = idx * VAST_NODE_STRIDE_U32;
        vast[base] = kind;
        vast[base + 1] = u32::MAX;
        vast[base + 2] = u32::MAX;
        vast[base + 3] = u32::MAX;
        vast[base + 4] = idx.saturating_sub(1) as u32;
        vast[base + 5] = start;
        vast[base + 6] = len;
    }
    let out_init = vec![0u8; vast.len() * 4];

    let expanded_program = c11_annotate_typedef_names(
        "vast_nodes",
        "haystack",
        Expr::u32(source.len() as u32),
        Expr::u32(tokens.len() as u32),
        "annotated_vast",
    );
    let packed_program = c11_annotate_typedef_names_packed_haystack(
        "vast_nodes",
        "haystack",
        Expr::u32(source.len() as u32),
        Expr::u32(tokens.len() as u32),
        "annotated_vast",
    );

    let expanded = eval_words(
        &expanded_program,
        vec![bytes(&vast), expanded_haystack(source), out_init.clone()],
    );
    let packed = eval_words(
        &packed_program,
        vec![bytes(&vast), packed_haystack(source), out_init],
    );

    assert_eq!(
        packed, expanded,
        "packed typedef annotation must match expanded haystack semantics"
    );
}

#[test]
fn typedef_prehash_matches_annotation_source_hashes() {
    let source = b"typedef int foo; foo bar;";
    let tokens = [
        (TOK_TYPEDEF, 0u32, 7u32),
        (TOK_INT, 8, 3),
        (TOK_IDENTIFIER, 12, 3),
        (TOK_SEMICOLON, 15, 1),
        (TOK_IDENTIFIER, 17, 3),
        (TOK_IDENTIFIER, 21, 3),
        (TOK_SEMICOLON, 24, 1),
    ];
    let mut vast = vec![0u32; tokens.len() * VAST_NODE_STRIDE_U32];
    for (idx, (kind, start, len)) in tokens.iter().copied().enumerate() {
        let base = idx * VAST_NODE_STRIDE_U32;
        vast[base] = kind;
        vast[base + 1] = u32::MAX;
        vast[base + 2] = u32::MAX;
        vast[base + 3] = u32::MAX;
        vast[base + 4] = idx.saturating_sub(1) as u32;
        vast[base + 5] = start;
        vast[base + 6] = len;
    }
    let out_init = vec![0u8; vast.len() * 4];

    let expanded_prehash_program = c11_prehash_vast_identifiers(
        "vast_nodes",
        "haystack",
        Expr::u32(source.len() as u32),
        Expr::u32(tokens.len() as u32),
        "hashed_vast",
    );
    let packed_prehash_program = c11_prehash_vast_identifiers_packed_haystack(
        "vast_nodes",
        "haystack",
        Expr::u32(source.len() as u32),
        Expr::u32(tokens.len() as u32),
        "hashed_vast",
    );

    let expanded_hashes = eval_words(
        &expanded_prehash_program,
        vec![bytes(&vast), expanded_haystack(source), out_init.clone()],
    );
    let packed_hashes = eval_words(
        &packed_prehash_program,
        vec![bytes(&vast), packed_haystack(source), out_init.clone()],
    );
    assert_eq!(
        packed_hashes, expanded_hashes,
        "packed VAST identifier prehashing must match expanded haystack hashing"
    );
    assert_ne!(
        expanded_hashes[2 * VAST_NODE_STRIDE_U32 + 9],
        0,
        "identifier rows must receive cached source hashes"
    );
    assert_eq!(
        expanded_hashes[VAST_NODE_STRIDE_U32 + 9],
        0,
        "non-identifier rows must not synthesize identifier hashes"
    );

    let annotate_program = c11_annotate_typedef_names(
        "vast_nodes",
        "haystack",
        Expr::u32(source.len() as u32),
        Expr::u32(tokens.len() as u32),
        "annotated_vast",
    );
    let raw_annotated = eval_words(
        &annotate_program,
        vec![bytes(&vast), expanded_haystack(source), out_init.clone()],
    );
    let prehashed_annotated = eval_words(
        &annotate_program,
        vec![
            bytes(&expanded_hashes),
            expanded_haystack(source),
            out_init.clone(),
        ],
    );
    assert_eq!(
        prehashed_annotated, raw_annotated,
        "annotation must preserve semantics when identifier hashes are precomputed"
    );
}
