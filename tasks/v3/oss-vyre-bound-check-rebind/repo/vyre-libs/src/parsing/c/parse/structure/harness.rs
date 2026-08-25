use super::*;
use vyre_primitives::hash::fnv1a::fnv1a32;

inventory::submit! {
    crate::harness::OpEntry {
        id: "vyre-libs::parsing::c11_extract_functions",
        build: || c11_extract_functions(
            "tok_types", "paren_pairs", "brace_pairs", Expr::u32(6), "out_functions", "out_counts"
        ),
        test_inputs: Some(function_extract_inputs),
        expected_output: Some(function_extract_expected),
        category: Some("parsing"),
    }
}

inventory::submit! {
    crate::harness::OpEntry {
        id: "vyre-libs::parsing::c11_extract_calls",
        build: || c11_extract_calls(
            "tok_types", "paren_pairs", "functions", Expr::u32(9), Expr::u32(1), "out_calls", "out_counts"
        ),
        test_inputs: Some(call_extract_inputs),
        expected_output: Some(call_extract_expected),
        category: Some("parsing"),
    }
}

inventory::submit! {
    crate::harness::OpEntry {
        id: "vyre-libs::parsing::c11_build_call_graph",
        build: || c11_build_call_graph("calls", "fn_hashes", "tok_starts", "tok_lens", "haystack", Expr::u32(1), Expr::u32(1), Expr::u32(6), "out_edges", "out_counts"),
        test_inputs: Some(call_graph_inputs),
        expected_output: Some(call_graph_expected),
        category: Some("parsing"),
    }
}

use crate::scan::dispatch_io::pack_u32_slice as pack_u32;

fn function_extract_inputs() -> Vec<Vec<Vec<u8>>> {
    vec![vec![
        pack_u32(&[
            TOK_INT,
            TOK_IDENTIFIER,
            TOK_LPAREN,
            TOK_RPAREN,
            TOK_LBRACE,
            TOK_RBRACE,
        ]),
        pack_u32(&[u32::MAX, u32::MAX, 3, 2, u32::MAX, u32::MAX]),
        pack_u32(&[u32::MAX, u32::MAX, u32::MAX, u32::MAX, 5, 4]),
        vec![0u8; 6 * 3 * 4],
        pack_u32(&[0]),
    ]]
}

fn function_extract_expected() -> Vec<Vec<Vec<u8>>> {
    let mut functions = vec![0u32; 18];
    functions[3..6].copy_from_slice(&[1, 4, 5]);
    vec![vec![pack_u32(&functions), pack_u32(&[18])]]
}

fn call_extract_inputs() -> Vec<Vec<Vec<u8>>> {
    vec![vec![
        pack_u32(&[
            TOK_INT,
            TOK_IDENTIFIER,
            TOK_LPAREN,
            TOK_RPAREN,
            TOK_LBRACE,
            TOK_IDENTIFIER,
            TOK_LPAREN,
            TOK_RPAREN,
            TOK_SEMICOLON,
        ]),
        pack_u32(&[u32::MAX, u32::MAX, 3, 2, u32::MAX, u32::MAX, 7, 6, u32::MAX]),
        pack_u32(&[1, 4, 8]),
        vec![0u8; 9 * 4 * 4],
        pack_u32(&[0]),
    ]]
}

fn call_extract_expected() -> Vec<Vec<Vec<u8>>> {
    let mut calls = vec![0u32; 9 * 4];
    calls[21..24].copy_from_slice(&[5, 6, 7]);
    vec![vec![pack_u32(&calls), pack_u32(&[36])]]
}

fn call_graph_inputs() -> Vec<Vec<Vec<u8>>> {
    vec![vec![
        pack_u32(&[0, 5, 6, 7]),
        pack_u32(&[fnv1a32(b"foo")]),
        pack_u32(&[0, 0, 0, 0, 0, 0]),
        pack_u32(&[0, 0, 0, 0, 0, 3]),
        pack_u32(&[
            u32::from(b'f'),
            u32::from(b'o'),
            u32::from(b'o'),
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        ]),
        vec![0u8; 4 * 4],
        pack_u32(&[0]),
    ]]
}

fn call_graph_expected() -> Vec<Vec<Vec<u8>>> {
    vec![vec![pack_u32(&[0, 0, 0, 0]), pack_u32(&[2])]]
}
