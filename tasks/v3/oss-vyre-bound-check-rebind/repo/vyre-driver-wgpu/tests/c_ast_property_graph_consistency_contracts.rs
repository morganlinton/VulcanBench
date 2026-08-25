//! Property and adversarial tests for VAST graph consistency:
//! parent/child/sibling relationships, malformed delimiters,
//! deeply nested delimiters, and out-of-bounds row references.

#![cfg(feature = "c-parser")]
#![allow(clippy::same_item_push)]
#![allow(deprecated)]
#[path = "c_ast_gpu_parity_support/mod.rs"]
mod c_ast_gpu_parity_support;

use c_ast_gpu_parity_support::{
    run_gpu_vast_builder_from_parts as run_gpu_vast_builder, starts_for_lens,
};
use proptest::prelude::*;
use vyre_foundation::vast::{
    validate_vast, VastNode, HEADER_LEN, NODE_STRIDE_U32, SENTINEL, VAST_MAGIC, VAST_VERSION,
};
use vyre_libs::parsing::c::lex::tokens::*;
use vyre_libs::parsing::c::parse::vast::reference_c11_build_vast_nodes;

const VAST_STRIDE_BYTES: usize = NODE_STRIDE_U32 * 4;

/// Wrap raw node bytes (output of builder) into a minimal valid VAST buffer.
/// Adds one dummy file with size=u32::MAX so source spans are always valid.
fn wrap_raw_vast(node_bytes: &[u8]) -> Vec<u8> {
    let node_count = (node_bytes.len() / VAST_STRIDE_BYTES) as u32;
    let file_count = 1u32;
    let file_table_len = (file_count as usize) * 12;
    let mut buf = Vec::with_capacity(HEADER_LEN + node_bytes.len() + file_table_len);
    buf.extend_from_slice(&VAST_MAGIC);
    buf.extend_from_slice(&VAST_VERSION.to_le_bytes());
    buf.extend_from_slice(&0u16.to_le_bytes()); // source_lang
    buf.extend_from_slice(&node_count.to_le_bytes());
    buf.extend_from_slice(&file_count.to_le_bytes());
    buf.extend_from_slice(&0u32.to_le_bytes()); // string_blob_len
    buf.extend_from_slice(&0u32.to_le_bytes()); // attr_blob_len
    buf.extend_from_slice(node_bytes);
    // One dummy file entry: path_off=0, path_len=0, size=u32::MAX
    buf.extend_from_slice(&0u32.to_le_bytes());
    buf.extend_from_slice(&0u32.to_le_bytes());
    buf.extend_from_slice(&u32::MAX.to_le_bytes());
    buf
}

/// Read a node row from raw node bytes (no header).
fn read_node(node_bytes: &[u8], idx: u32) -> Option<VastNode> {
    VastNode::read_row_bytes(node_bytes, idx)
}

fn assert_no_oob_edges(node_bytes: &[u8]) {
    let node_count = node_bytes.len() / VAST_STRIDE_BYTES;
    if node_count == 0 {
        return;
    }
    for i in 0..node_count {
        let node = read_node(node_bytes, i as u32).unwrap();
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

fn assert_graph_consistency(node_bytes: &[u8]) {
    let node_count = node_bytes.len() / VAST_STRIDE_BYTES;
    if node_count == 0 {
        return;
    }
    for i in 0..node_count {
        let node = read_node(node_bytes, i as u32).unwrap();

        // Parent <-> child bidirectional check
        if node.parent_idx != SENTINEL {
            let parent = read_node(node_bytes, node.parent_idx).unwrap();
            let mut found = false;
            let mut c = parent.first_child;
            while c != SENTINEL {
                if c == i as u32 {
                    found = true;
                    break;
                }
                c = read_node(node_bytes, c).unwrap().next_sibling;
            }
            assert!(
                found,
                "node {i} parent {} does not list it as child",
                node.parent_idx
            );
        }

        // first_child parent check
        if node.first_child != SENTINEL {
            let child = read_node(node_bytes, node.first_child).unwrap();
            assert_eq!(
                child.parent_idx, i as u32,
                "node {i} first_child {} has wrong parent {}",
                node.first_child, child.parent_idx
            );
        }

        // next_sibling parent check
        if node.next_sibling != SENTINEL {
            let sib = read_node(node_bytes, node.next_sibling).unwrap();
            assert_eq!(
                sib.parent_idx, node.parent_idx,
                "node {i} next_sibling {} has parent {} != {}",
                node.next_sibling, sib.parent_idx, node.parent_idx
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Deterministic adversarial tables  -  malformed delimiters
// ---------------------------------------------------------------------------

#[test]
fn malformed_unmatched_open_paren() {
    let tok_types = vec![TOK_LPAREN, TOK_IDENTIFIER, TOK_SEMICOLON];
    let tok_lens = vec![1, 1, 1];
    let tok_starts = starts_for_lens(&tok_lens);
    let cpu = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    assert_no_oob_edges(&cpu);
    assert_graph_consistency(&cpu);
    let gpu = run_gpu_vast_builder(&tok_types, &tok_starts, &tok_lens);
    assert_eq!(gpu, cpu, "GPU/CPU parity for unmatched open paren");
}

#[test]
fn malformed_unmatched_close_paren() {
    let tok_types = vec![TOK_IDENTIFIER, TOK_RPAREN, TOK_SEMICOLON];
    let tok_lens = vec![1, 1, 1];
    let tok_starts = starts_for_lens(&tok_lens);
    let cpu = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    assert_no_oob_edges(&cpu);
    assert_graph_consistency(&cpu);
    let gpu = run_gpu_vast_builder(&tok_types, &tok_starts, &tok_lens);
    assert_eq!(gpu, cpu, "GPU/CPU parity for unmatched close paren");
}

#[test]
fn malformed_unmatched_open_brace() {
    let tok_types = vec![TOK_LBRACE, TOK_IDENTIFIER, TOK_SEMICOLON];
    let tok_lens = vec![1, 1, 1];
    let tok_starts = starts_for_lens(&tok_lens);
    let cpu = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    assert_no_oob_edges(&cpu);
    assert_graph_consistency(&cpu);
}

#[test]
fn malformed_unmatched_close_brace() {
    let tok_types = vec![TOK_IDENTIFIER, TOK_RBRACE, TOK_SEMICOLON];
    let tok_lens = vec![1, 1, 1];
    let tok_starts = starts_for_lens(&tok_lens);
    let cpu = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    assert_no_oob_edges(&cpu);
    assert_graph_consistency(&cpu);
}

#[test]
fn malformed_unmatched_open_bracket() {
    let tok_types = vec![TOK_LBRACKET, TOK_IDENTIFIER, TOK_SEMICOLON];
    let tok_lens = vec![1, 1, 1];
    let tok_starts = starts_for_lens(&tok_lens);
    let cpu = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    assert_no_oob_edges(&cpu);
    assert_graph_consistency(&cpu);
}

#[test]
fn malformed_unmatched_close_bracket() {
    let tok_types = vec![TOK_IDENTIFIER, TOK_RBRACKET, TOK_SEMICOLON];
    let tok_lens = vec![1, 1, 1];
    let tok_starts = starts_for_lens(&tok_lens);
    let cpu = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    assert_no_oob_edges(&cpu);
    assert_graph_consistency(&cpu);
}

#[test]
fn malformed_mismatched_delimiters() {
    // ({)]
    let tok_types = vec![TOK_LPAREN, TOK_LBRACE, TOK_RPAREN, TOK_RBRACKET];
    let tok_lens = vec![1; 4];
    let tok_starts = starts_for_lens(&tok_lens);
    let cpu = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    assert_no_oob_edges(&cpu);
    assert_graph_consistency(&cpu);
    let wrapped = wrap_raw_vast(&cpu);
    validate_vast(&wrapped).expect("mismatched delimiters must still produce valid VAST layout");
}

#[test]
fn malformed_surround_with_unbalanced() {
    // ( } { )
    let tok_types = vec![TOK_LPAREN, TOK_RBRACE, TOK_LBRACE, TOK_RPAREN];
    let tok_lens = vec![1; 4];
    let tok_starts = starts_for_lens(&tok_lens);
    let cpu = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    assert_no_oob_edges(&cpu);
    assert_graph_consistency(&cpu);
    let gpu = run_gpu_vast_builder(&tok_types, &tok_starts, &tok_lens);
    assert_eq!(gpu, cpu, "GPU/CPU parity for surround-with-unbalanced");
}

// ---------------------------------------------------------------------------
// Deterministic adversarial tables  -  deeply nested delimiters
// ---------------------------------------------------------------------------

#[test]
fn deeply_nested_braces_depth_64() {
    let mut tok_types = Vec::new();
    for _ in 0..64 {
        tok_types.push(TOK_LBRACE);
    }
    tok_types.push(TOK_IDENTIFIER);
    for _ in 0..64 {
        tok_types.push(TOK_RBRACE);
    }
    let tok_lens = vec![1; tok_types.len()];
    let tok_starts = starts_for_lens(&tok_lens);
    let cpu = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    assert_no_oob_edges(&cpu);
    assert_graph_consistency(&cpu);
    // The identifier at index 64 should have parent chain length 64.
    let mut depth = 0u32;
    let mut cur = 64u32;
    let node = read_node(&cpu, cur).unwrap();
    let mut p = node.parent_idx;
    while p != SENTINEL {
        depth += 1;
        cur = p;
        p = read_node(&cpu, cur).unwrap().parent_idx;
    }
    assert_eq!(depth, 64, "identifier must be nested 64 braces deep");
}

#[test]
fn deeply_nested_parens_depth_128() {
    let mut tok_types = Vec::new();
    for _ in 0..128 {
        tok_types.push(TOK_LPAREN);
    }
    tok_types.push(TOK_IDENTIFIER);
    for _ in 0..128 {
        tok_types.push(TOK_RPAREN);
    }
    let tok_lens = vec![1; tok_types.len()];
    let tok_starts = starts_for_lens(&tok_lens);
    let cpu = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    assert_no_oob_edges(&cpu);
    assert_graph_consistency(&cpu);
    let gpu = run_gpu_vast_builder(&tok_types, &tok_starts, &tok_lens);
    assert_eq!(gpu, cpu, "GPU/CPU parity for depth-128 parens");
}

#[test]
fn deeply_nested_brackets_depth_256() {
    let mut tok_types = Vec::new();
    for _ in 0..256 {
        tok_types.push(TOK_LBRACKET);
    }
    tok_types.push(TOK_IDENTIFIER);
    for _ in 0..256 {
        tok_types.push(TOK_RBRACKET);
    }
    let tok_lens = vec![1; tok_types.len()];
    let tok_starts = starts_for_lens(&tok_lens);
    let cpu = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    assert_no_oob_edges(&cpu);
    assert_graph_consistency(&cpu);
    let gpu = run_gpu_vast_builder(&tok_types, &tok_starts, &tok_lens);
    assert_eq!(gpu, cpu, "GPU/CPU parity for depth-256 brackets");
}

#[test]
fn deeply_nested_mixed_depth_64() {
    let mut tok_types = Vec::new();
    for _ in 0..64 {
        tok_types.push(TOK_LBRACE);
        tok_types.push(TOK_LPAREN);
        tok_types.push(TOK_LBRACKET);
    }
    tok_types.push(TOK_IDENTIFIER);
    for _ in 0..64 {
        tok_types.push(TOK_RBRACKET);
        tok_types.push(TOK_RPAREN);
        tok_types.push(TOK_RBRACE);
    }
    let tok_lens = vec![1; tok_types.len()];
    let tok_starts = starts_for_lens(&tok_lens);
    let cpu = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    assert_no_oob_edges(&cpu);
    assert_graph_consistency(&cpu);
    let gpu = run_gpu_vast_builder(&tok_types, &tok_starts, &tok_lens);
    assert_eq!(gpu, cpu, "GPU/CPU parity for depth-64 mixed nesting");
}

// ---------------------------------------------------------------------------
// Proptest: random token streams must produce consistent graphs
// ---------------------------------------------------------------------------

fn arb_token_stream() -> impl Strategy<Value = (Vec<u32>, Vec<u32>, Vec<u32>)> {
    let choices = vec![
        TOK_LPAREN,
        TOK_RPAREN,
        TOK_LBRACE,
        TOK_RBRACE,
        TOK_LBRACKET,
        TOK_RBRACKET,
        TOK_IDENTIFIER,
        TOK_INTEGER,
        TOK_SEMICOLON,
        TOK_PLUS,
        TOK_STAR,
        TOK_COMMA,
        TOK_ASSIGN,
        TOK_IF,
        TOK_WHILE,
        TOK_RETURN,
        TOK_STRUCT,
        TOK_INT,
    ];
    prop::collection::vec(prop::sample::select(choices), 1..512).prop_map(|tok_types| {
        let tok_lens: Vec<u32> = tok_types.iter().map(|_| 1u32).collect();
        let tok_starts = starts_for_lens(&tok_lens);
        (tok_types, tok_starts, tok_lens)
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn proptest_random_streams_graph_consistent((tok_types, tok_starts, tok_lens) in arb_token_stream()) {
        let cpu = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
        assert_no_oob_edges(&cpu);
        assert_graph_consistency(&cpu);
        let wrapped = wrap_raw_vast(&cpu);
        validate_vast(&wrapped).expect("random stream must produce valid VAST layout");
    }

    #[test]
    fn proptest_random_streams_gpu_parity((tok_types, tok_starts, tok_lens) in arb_token_stream()) {
        let cpu = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
        let gpu = run_gpu_vast_builder(&tok_types, &tok_starts, &tok_lens);
        prop_assert_eq!(gpu, cpu, "GPU must match CPU reference for random token stream");
    }
}
