//! Span monotonicity, large token counts near stage bounds, and
//! out-of-bounds row reference contracts.

#![cfg(feature = "c-parser")]
#![allow(clippy::same_item_push)]
#![allow(deprecated)]
#[path = "c_ast_gpu_parity_support/mod.rs"]
mod c_ast_gpu_parity_support;

use c_ast_gpu_parity_support::{run_gpu_vast_builder_from_parts, starts_for_lens, word_at};
use proptest::prelude::*;
use vyre_foundation::vast::{VastNode, NODE_STRIDE_U32, SENTINEL};
use vyre_libs::parsing::c::lex::tokens::*;
use vyre_libs::parsing::c::parse::vast::reference_c11_build_vast_nodes;

const VAST_STRIDE_BYTES: usize = NODE_STRIDE_U32 * 4;

fn assert_span_monotonicity(node_bytes: &[u8]) {
    let node_count = node_bytes.len() / VAST_STRIDE_BYTES;
    if node_count < 2 {
        return;
    }
    for i in 1..node_count {
        let prev_off = word_at(node_bytes, (i - 1) * NODE_STRIDE_U32 + 5);
        let cur_off = word_at(node_bytes, i * NODE_STRIDE_U32 + 5);
        assert!(
            cur_off >= prev_off,
            "span offset decreased at node {i}: {cur_off} < {prev_off}"
        );
    }
}

fn assert_spans_match_tokens(node_bytes: &[u8], tok_starts: &[u32], tok_lens: &[u32]) {
    let node_count = node_bytes.len() / VAST_STRIDE_BYTES;
    assert_eq!(node_count, tok_starts.len());
    assert_eq!(node_count, tok_lens.len());
    for i in 0..node_count {
        let off = word_at(node_bytes, i * NODE_STRIDE_U32 + 5);
        let len = word_at(node_bytes, i * NODE_STRIDE_U32 + 6);
        assert_eq!(
            off, tok_starts[i],
            "node {i} span offset must match token start"
        );
        assert_eq!(
            len, tok_lens[i],
            "node {i} span length must match token length"
        );
    }
}

fn assert_no_oob_edges(node_bytes: &[u8]) {
    let node_count = node_bytes.len() / VAST_STRIDE_BYTES;
    if node_count == 0 {
        return;
    }
    for i in 0..node_count {
        let node = VastNode::read_row_bytes(node_bytes, i as u32).unwrap();
        let check = |field: &str, val: u32| {
            if val != SENTINEL {
                assert!(
                    (val as usize) < node_count,
                    "node {i} {field}={val} out of bounds (count={node_count})"
                );
            }
        };
        check("parent_idx", node.parent_idx);
        check("first_child", node.first_child);
        check("next_sibling", node.next_sibling);
    }
}

// ---------------------------------------------------------------------------
// Deterministic: large token counts near stage bounds
// ---------------------------------------------------------------------------

fn flat_tokens(count: usize) -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let tok_types: Vec<u32> = (0..count)
        .map(|i| {
            if i % 2 == 0 {
                TOK_IDENTIFIER
            } else {
                TOK_SEMICOLON
            }
        })
        .collect();
    let tok_lens = vec![1; count];
    let tok_starts = starts_for_lens(&tok_lens);
    (tok_types, tok_starts, tok_lens)
}

#[test]
fn span_monotonicity_flat_64() {
    let (tok_types, tok_starts, tok_lens) = flat_tokens(64);
    let cpu = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    assert_span_monotonicity(&cpu);
    assert_spans_match_tokens(&cpu, &tok_starts, &tok_lens);
    assert_no_oob_edges(&cpu);
}

#[test]
fn span_monotonicity_flat_65() {
    let (tok_types, tok_starts, tok_lens) = flat_tokens(65);
    let cpu = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    assert_span_monotonicity(&cpu);
    assert_spans_match_tokens(&cpu, &tok_starts, &tok_lens);
    assert_no_oob_edges(&cpu);
}

#[test]
fn span_monotonicity_flat_128() {
    let (tok_types, tok_starts, tok_lens) = flat_tokens(128);
    let cpu = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    assert_span_monotonicity(&cpu);
    assert_spans_match_tokens(&cpu, &tok_starts, &tok_lens);
    let gpu = run_gpu_vast_builder_from_parts(&tok_types, &tok_starts, &tok_lens);
    assert_eq!(gpu, cpu, "GPU parity at 128 tokens");
}

#[test]
fn span_monotonicity_flat_256() {
    let (tok_types, tok_starts, tok_lens) = flat_tokens(256);
    let cpu = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    assert_span_monotonicity(&cpu);
    assert_spans_match_tokens(&cpu, &tok_starts, &tok_lens);
    let gpu = run_gpu_vast_builder_from_parts(&tok_types, &tok_starts, &tok_lens);
    assert_eq!(gpu, cpu, "GPU parity at 256 tokens");
}

#[test]
fn span_monotonicity_flat_512() {
    let (tok_types, tok_starts, tok_lens) = flat_tokens(512);
    let cpu = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    assert_span_monotonicity(&cpu);
    assert_spans_match_tokens(&cpu, &tok_starts, &tok_lens);
    let gpu = run_gpu_vast_builder_from_parts(&tok_types, &tok_starts, &tok_lens);
    assert_eq!(gpu, cpu, "GPU parity at 512 tokens");
}

#[test]
fn span_monotonicity_flat_1024() {
    let (tok_types, tok_starts, tok_lens) = flat_tokens(1024);
    let cpu = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    assert_span_monotonicity(&cpu);
    assert_spans_match_tokens(&cpu, &tok_starts, &tok_lens);
    let gpu = run_gpu_vast_builder_from_parts(&tok_types, &tok_starts, &tok_lens);
    assert_eq!(gpu, cpu, "GPU parity at 1024 tokens");
}

#[test]
fn span_monotonicity_flat_2048() {
    let (tok_types, tok_starts, tok_lens) = flat_tokens(2048);
    let cpu = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    assert_span_monotonicity(&cpu);
    assert_spans_match_tokens(&cpu, &tok_starts, &tok_lens);
    let gpu = run_gpu_vast_builder_from_parts(&tok_types, &tok_starts, &tok_lens);
    assert_eq!(gpu, cpu, "GPU parity at 2048 tokens");
}

#[test]
fn span_monotonicity_near_typedef_search_limit() {
    // 63, 64, 65 tokens to exercise C_TYPEDEF_SCOPE_SEARCH_LIMIT boundary
    for count in [63, 64, 65] {
        let mut tok_types = vec![TOK_LBRACE];
        for _ in 0..(count - 2) {
            tok_types.push(TOK_IDENTIFIER);
            tok_types.push(TOK_SEMICOLON);
        }
        tok_types.push(TOK_RBRACE);
        let tok_lens = vec![1; tok_types.len()];
        let tok_starts = starts_for_lens(&tok_lens);
        let cpu = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
        assert_span_monotonicity(&cpu);
        assert_spans_match_tokens(&cpu, &tok_starts, &tok_lens);
        assert_no_oob_edges(&cpu);
    }
}

#[test]
fn span_monotonicity_near_declarator_paren_limit() {
    // 7, 8, 9 nested parens to exercise C_DECLARATOR_PAREN_SEARCH_LIMIT boundary
    for depth in [7, 8, 9] {
        let mut tok_types = Vec::new();
        for _ in 0..depth {
            tok_types.push(TOK_LPAREN);
        }
        tok_types.push(TOK_IDENTIFIER);
        for _ in 0..depth {
            tok_types.push(TOK_RPAREN);
        }
        tok_types.push(TOK_SEMICOLON);
        let tok_lens = vec![1; tok_types.len()];
        let tok_starts = starts_for_lens(&tok_lens);
        let cpu = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
        assert_span_monotonicity(&cpu);
        assert_spans_match_tokens(&cpu, &tok_starts, &tok_lens);
        assert_no_oob_edges(&cpu);
    }
}

#[test]
fn span_monotonicity_deep_nesting_512() {
    let mut tok_types = Vec::new();
    for _ in 0..256 {
        tok_types.push(TOK_LBRACE);
        tok_types.push(TOK_LPAREN);
    }
    for _ in 0..256 {
        tok_types.push(TOK_RPAREN);
        tok_types.push(TOK_RBRACE);
    }
    let tok_lens = vec![1; tok_types.len()];
    let tok_starts = starts_for_lens(&tok_lens);
    let cpu = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    assert_span_monotonicity(&cpu);
    assert_spans_match_tokens(&cpu, &tok_starts, &tok_lens);
    let gpu = run_gpu_vast_builder_from_parts(&tok_types, &tok_starts, &tok_lens);
    assert_eq!(gpu, cpu, "GPU parity for 512-token deep nesting");
}

#[test]
fn span_monotonicity_large_varying_lengths() {
    let lens: Vec<u32> = (1..=256).map(|i| (i % 8 + 1) as u32).collect();
    let tok_types: Vec<u32> = lens.iter().map(|_| TOK_IDENTIFIER).collect();
    let tok_starts = starts_for_lens(&lens);
    let cpu = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &lens);
    assert_span_monotonicity(&cpu);
    assert_spans_match_tokens(&cpu, &tok_starts, &lens);
    let gpu = run_gpu_vast_builder_from_parts(&tok_types, &tok_starts, &lens);
    assert_eq!(gpu, cpu, "GPU parity for large varying lengths");
}

// ---------------------------------------------------------------------------
// Proptest
// ---------------------------------------------------------------------------

fn arb_lens() -> impl Strategy<Value = Vec<u32>> {
    prop::collection::vec(1u32..8, 1..256)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn proptest_spans_monotonic_and_match_tokens(lens in arb_lens()) {
        let tok_types: Vec<u32> = lens.iter().map(|_| TOK_IDENTIFIER).collect();
        let tok_starts = starts_for_lens(&lens);
        let cpu = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &lens);
        assert_span_monotonicity(&cpu);
        assert_spans_match_tokens(&cpu, &tok_starts, &lens);
        assert_no_oob_edges(&cpu);
    }

    #[test]
    fn proptest_large_random_lens_monotonic(lens in prop::collection::vec(1u32..4, 256..1025)) {
        let tok_types: Vec<u32> = lens.iter().map(|_| TOK_IDENTIFIER).collect();
        let tok_starts = starts_for_lens(&lens);
        let cpu = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &lens);
        assert_span_monotonicity(&cpu);
        assert_spans_match_tokens(&cpu, &tok_starts, &lens);
    }
}
