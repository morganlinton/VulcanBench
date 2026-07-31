//! End-to-end GPU/CPU parity contracts for VAST classification and PG lowering.
//!
//! Covers: full-pipeline span preservation, GPU backend loud-failure, expression
//! shape parity, PG link-field parity, and adversarial fixtures (directives,
//! strings, comments, nested blocks). No stage may silently emit empty or
//! default output for non-empty input.

#![cfg(feature = "c-parser")]
#![allow(deprecated)]
use std::sync::OnceLock;

use vyre::ir::Expr;
use vyre::VyreBackend;
use vyre_driver_wgpu::WgpuBackend;
use vyre_libs::parsing::c::lex::keyword::reference_c_keyword_types;
use vyre_libs::parsing::c::lex::tokens::*;
use vyre_libs::parsing::c::lower::{c_lower_ast_to_pg_nodes, reference_ast_to_pg_nodes};
use vyre_libs::parsing::c::parse::vast::{
    c11_build_expression_shape_nodes, c11_build_vast_nodes, c11_classify_vast_node_kinds,
    reference_c11_build_expression_shape_nodes, reference_c11_build_vast_nodes,
    reference_c11_classify_vast_node_kinds,
};

const VAST_STRIDE_U32: usize = 10;
const PG_STRIDE_U32: usize = 6;
const SENTINEL: u32 = u32::MAX;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn bytes(words: &[u32]) -> Vec<u8> {
    vyre_primitives::wire::pack_u32_slice(words)
}

fn word_at(buf: &[u8], word: usize) -> u32 {
    let off = word * 4;
    u32::from_le_bytes(buf[off..off + 4].try_into().unwrap())
}

fn node_count_from_vast(buf: &[u8]) -> u32 {
    u32::try_from(buf.len() / (VAST_STRIDE_U32 * 4)).unwrap_or_default()
}

struct Fixture {
    _source: String,
    tok_types: Vec<u32>,
    tok_starts: Vec<u32>,
    tok_lens: Vec<u32>,
}

fn build_fixture(lexemes: &[(&str, u32)]) -> Fixture {
    let mut source = String::new();
    let mut tok_starts = Vec::new();
    let mut tok_lens = Vec::new();
    let mut raw_kinds = Vec::new();

    for (lex, kind) in lexemes {
        if *kind == TOK_WHITESPACE || *kind == TOK_COMMENT {
            source.push_str(lex);
            continue;
        }
        if !source.is_empty() && !source.ends_with('\n') {
            source.push(' ');
        }
        tok_starts.push(source.len() as u32);
        source.push_str(lex);
        tok_lens.push(lex.len() as u32);
        raw_kinds.push(*kind);
    }

    let tok_types =
        reference_c_keyword_types(&raw_kinds, &tok_starts, &tok_lens, source.as_bytes());
    Fixture {
        _source: source,
        tok_types,
        tok_starts,
        tok_lens,
    }
}

fn gpu_backend() -> &'static WgpuBackend {
    static BACKEND: OnceLock<WgpuBackend> = OnceLock::new();
    BACKEND.get_or_init(|| {
        WgpuBackend::acquire().expect(
            "WgpuBackend::acquire failed on a machine that must have a GPU. \
             This is a configuration bug, not a graceful skip.",
        )
    })
}

fn run_cpu_vast_builder(fix: &Fixture) -> Vec<u8> {
    reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens)
}

fn run_cpu_classifier(vast: &[u8]) -> Vec<u8> {
    reference_c11_classify_vast_node_kinds(vast)
}

fn run_cpu_expr_shape(raw_vast: &[u8], typed_vast: &[u8]) -> Vec<u8> {
    reference_c11_build_expression_shape_nodes(raw_vast, typed_vast)
}

fn run_cpu_pg_lower(typed_vast: &[u8]) -> Vec<u8> {
    reference_ast_to_pg_nodes(typed_vast)
}

fn run_gpu_vast_builder(fix: &Fixture) -> Vec<u8> {
    let program = c11_build_vast_nodes(
        "tok_types",
        "tok_starts",
        "tok_lens",
        Expr::u32(fix.tok_types.len() as u32),
        "out_vast_nodes",
        "out_count",
    );
    let tok_type_bytes = bytes(&fix.tok_types);
    let tok_start_bytes = bytes(&fix.tok_starts);
    let tok_len_bytes = bytes(&fix.tok_lens);
    let inputs: Vec<&[u8]> = vec![&tok_type_bytes, &tok_start_bytes, &tok_len_bytes];
    let outputs = gpu_backend()
        .dispatch_borrowed(&program, &inputs, &Default::default())
        .expect("GPU VAST builder dispatch must succeed");
    assert_eq!(outputs.len(), 2, "expected [vast_nodes, count]");
    outputs[0].clone()
}

fn run_gpu_classifier(vast: &[u8]) -> Vec<u8> {
    let n = node_count_from_vast(vast);
    let program = c11_classify_vast_node_kinds("vast_nodes", Expr::u32(n), "typed_vast_nodes");
    let inputs: Vec<&[u8]> = vec![vast];
    let outputs = gpu_backend()
        .dispatch_borrowed(&program, &inputs, &Default::default())
        .expect("GPU classify dispatch must succeed");
    assert_eq!(outputs.len(), 1);
    outputs[0].clone()
}

fn run_gpu_expr_shape(raw_vast: &[u8], typed_vast: &[u8]) -> Vec<u8> {
    let program = c11_build_expression_shape_nodes(
        "raw_vast_nodes",
        "typed_vast_nodes",
        Expr::u32(node_count_from_vast(raw_vast)),
        "expr_shape_nodes",
    );
    let inputs: Vec<&[u8]> = vec![raw_vast, typed_vast];
    let outputs = gpu_backend()
        .dispatch_borrowed(&program, &inputs, &Default::default())
        .expect("GPU expression-shape dispatch must succeed");
    assert_eq!(outputs.len(), 1);
    outputs[0].clone()
}

fn run_gpu_pg_lower(typed_vast: &[u8]) -> Vec<u8> {
    let program = c_lower_ast_to_pg_nodes(
        "vast_nodes",
        Expr::u32(node_count_from_vast(typed_vast)),
        "pg_nodes",
    );
    let inputs: Vec<&[u8]> = vec![typed_vast];
    let outputs = gpu_backend()
        .dispatch_borrowed(&program, &inputs, &Default::default())
        .expect("GPU PG lowerer dispatch must succeed");
    assert_eq!(outputs.len(), 1);
    outputs[0].clone()
}

fn assert_words_eq(actual: &[u8], expected: &[u8], context: &str) {
    if actual == expected {
        return;
    }
    let limit = (actual.len() / 4).min(expected.len() / 4);
    for w in 0..limit {
        let a = word_at(actual, w);
        let e = word_at(expected, w);
        if a != e {
            panic!(
                "{context}: word {w} differs (row={}, field={}): actual={a}, expected={e}",
                w / VAST_STRIDE_U32,
                w % VAST_STRIDE_U32
            );
        }
    }
    panic!(
        "{context}: byte lengths differ: actual={}, expected={}",
        actual.len(),
        expected.len()
    );
}

fn assert_full_pipeline_parity(fix: &Fixture, label: &str) {
    let raw_cpu = run_cpu_vast_builder(fix);
    let raw_gpu = run_gpu_vast_builder(fix);
    assert_words_eq(
        &raw_gpu,
        &raw_cpu,
        &format!("{label}: raw VAST GPU/CPU parity"),
    );

    let typed_cpu = run_cpu_classifier(&raw_cpu);
    let typed_gpu = run_gpu_classifier(&raw_cpu);
    assert_words_eq(
        &typed_gpu,
        &typed_cpu,
        &format!("{label}: typed VAST GPU/CPU parity"),
    );

    let shape_cpu = run_cpu_expr_shape(&raw_cpu, &typed_cpu);
    let shape_gpu = run_gpu_expr_shape(&raw_cpu, &typed_cpu);
    assert_words_eq(
        &shape_gpu,
        &shape_cpu,
        &format!("{label}: expression-shape GPU/CPU parity"),
    );

    let pg_cpu = run_cpu_pg_lower(&typed_cpu);
    let pg_gpu = run_gpu_pg_lower(&typed_cpu);
    assert_words_eq(
        &pg_gpu,
        &pg_cpu,
        &format!("{label}: PG lowerer GPU/CPU parity"),
    );
}

// ---------------------------------------------------------------------------
// 1. GPU acquisition loud-failure contract
// ---------------------------------------------------------------------------

mod c_parser_pipeline_vast_pg_parity_contracts_part1 {

    include!("__split/c_parser_pipeline_vast_pg_parity_contracts_part1.rs");
}
mod c_parser_pipeline_vast_pg_parity_contracts_part2 {
    include!("__split/c_parser_pipeline_vast_pg_parity_contracts_part2.rs");
}
