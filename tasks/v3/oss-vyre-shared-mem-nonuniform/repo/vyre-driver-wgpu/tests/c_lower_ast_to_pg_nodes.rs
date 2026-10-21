//! CPU, WGSL, and GPU parity tests for C VAST-to-PG lowering.
#![cfg(feature = "c-parser")]
#![allow(deprecated)]
#![allow(
    clippy::useless_conversion,
    clippy::if_same_then_else,
    clippy::unnecessary_cast
)]
use c_grammar_gen::lex_c11_max_munch_kinds;
use proptest::prelude::*;
use std::sync::OnceLock;
use vyre::ir::{Expr, Program};
use vyre::{DispatchConfig, VyreBackend};
use vyre_driver_wgpu::WgpuBackend;
use vyre_emit_naga::program as naga_emit;
use vyre_foundation::optimizer::pre_lowering::optimize;
use vyre_libs::harness::{all_entries, OpEntry};
use vyre_libs::parsing::c::lex::keyword::reference_c_keyword_types;
use vyre_libs::parsing::c::lex::tokens::*;
use vyre_libs::parsing::c::lower::{c_lower_ast_to_pg_nodes, reference_ast_to_pg_nodes};
use vyre_libs::parsing::c::parse::vast::{
    reference_c11_build_vast_nodes, reference_c11_classify_vast_node_kinds, C_AST_KIND_ARRAY_DECL,
    C_AST_KIND_ARRAY_SUBSCRIPT_EXPR, C_AST_KIND_ASM_CLOBBERS_LIST, C_AST_KIND_ASM_TEMPLATE,
    C_AST_KIND_ASSIGN_EXPR, C_AST_KIND_CAST_EXPR, C_AST_KIND_COMPOUND_LITERAL_EXPR,
    C_AST_KIND_CONDITIONAL_EXPR, C_AST_KIND_ENUMERATOR_DECL, C_AST_KIND_FIELD_DECL,
    C_AST_KIND_FUNCTION_DECLARATOR, C_AST_KIND_FUNCTION_DEFINITION, C_AST_KIND_GNU_ATTRIBUTE,
    C_AST_KIND_IF_STMT, C_AST_KIND_INITIALIZER_LIST, C_AST_KIND_INLINE_ASM,
    C_AST_KIND_MEMBER_ACCESS_EXPR, C_AST_KIND_POINTER_DECL, C_AST_KIND_RETURN_STMT,
    C_AST_KIND_SIZEOF_EXPR, C_AST_KIND_UNARY_EXPR,
};
use vyre_primitives::predicate::node_kind;
use vyre_reference::value::Value;

mod c_ast_expression_support;

use c_ast_expression_support::{bytes, row_indices, starts_for_lens, word_at};

const VAST_STRIDE_U32: u32 = 10;
const VAST_STRIDE_BYTES: usize = (VAST_STRIDE_U32 as usize) * core::mem::size_of::<u32>();
const PG_STRIDE_U32: u32 = 6;
const TEST_WORKGROUP_SIZE: [u32; 3] = [1, 1, 1];
const OP_ID: &str = "vyre-libs::parsing::c::lower::ast_to_pg_nodes";
mod common;
use common::c_fixture::*;
fn gnu_c_stress_fixture_source_and_tokens() -> (String, Vec<u32>, Vec<u32>, Vec<u32>) {
    let tokens = [
        FixtureToken::new(
            "#define likely(x) __builtin_expect(!!(x), 1)\n",
            TOK_PREPROC,
        ),
        FixtureToken::new("typedef", TOK_TYPEDEF),
        FixtureToken::new("long", TOK_LONG),
        FixtureToken::new("fault_cb_t", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("struct", TOK_STRUCT),
        FixtureToken::new("file", TOK_IDENTIFIER),
        FixtureToken::new("*", TOK_STAR),
        FixtureToken::new("f", TOK_IDENTIFIER),
        FixtureToken::new(",", TOK_COMMA),
        FixtureToken::new("int", TOK_INT),
        FixtureToken::new("flags", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("static", TOK_STATIC),
        FixtureToken::new("inline", TOK_INLINE),
        FixtureToken::new("long", TOK_LONG),
        FixtureToken::new("handle_fault", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("struct", TOK_STRUCT),
        FixtureToken::new("file", TOK_IDENTIFIER),
        FixtureToken::new("*", TOK_STAR),
        FixtureToken::new("f", TOK_IDENTIFIER),
        FixtureToken::new(",", TOK_COMMA),
        FixtureToken::new("const", TOK_CONST),
        FixtureToken::new("char", TOK_CHAR_KW),
        FixtureToken::new("*", TOK_STAR),
        FixtureToken::new("name", TOK_IDENTIFIER),
        FixtureToken::new(",", TOK_COMMA),
        FixtureToken::new("int", TOK_INT),
        FixtureToken::new("flags", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("__attribute__", TOK_GNU_ATTRIBUTE),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("always_inline", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("trace_fault", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("name", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
        FixtureToken::new("if", TOK_IF),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("likely", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("flags", TOK_IDENTIFIER),
        FixtureToken::new("&", TOK_AMP),
        FixtureToken::new("1", TOK_INTEGER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("asm", TOK_GNU_ASM),
        FixtureToken::new("volatile", TOK_VOLATILE),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("\"mfence\"", TOK_STRING),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new("\"memory\"", TOK_STRING),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("return", TOK_RETURN),
        FixtureToken::new("do_fault", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("f", TOK_IDENTIFIER),
        FixtureToken::new(",", TOK_COMMA),
        FixtureToken::new("flags", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
        FixtureToken::new("return", TOK_RETURN),
        FixtureToken::new("0", TOK_INTEGER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
    ];
    let mut source = String::new();
    let mut starts = Vec::with_capacity(tokens.len());
    let mut lens = Vec::with_capacity(tokens.len());
    let mut raw_kinds = Vec::with_capacity(tokens.len());
    for token in tokens {
        if !source.is_empty() && !source.ends_with('\n') {
            source.push(' ');
        }
        starts.push(source.len() as u32);
        source.push_str(token.lexeme);
        lens.push(token.lexeme.len() as u32);
        raw_kinds.push(token.raw_kind);
    }
    (source, raw_kinds, starts, lens)
}
fn c_expression_operator_fixture_tokens() -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_IDENTIFIER,
        TOK_ARROW,
        TOK_IDENTIFIER,
        TOK_ASSIGN,
        TOK_SIZEOF,
        TOK_LPAREN,
        TOK_STAR,
        TOK_IDENTIFIER,
        TOK_RPAREN,
        TOK_SEMICOLON,
        TOK_IF,
        TOK_LPAREN,
        TOK_IDENTIFIER,
        TOK_GE,
        TOK_IDENTIFIER,
        TOK_AND,
        TOK_IDENTIFIER,
        TOK_NE,
        TOK_INTEGER,
        TOK_QUESTION,
        TOK_INTEGER,
        TOK_COLON,
        TOK_INTEGER,
        TOK_RPAREN,
        TOK_LBRACE,
        TOK_IDENTIFIER,
        TOK_DOT,
        TOK_IDENTIFIER,
        TOK_SLASH_EQ,
        TOK_INTEGER,
        TOK_SEMICOLON,
        TOK_IDENTIFIER,
        TOK_LSHIFT,
        TOK_INTEGER,
        TOK_SEMICOLON,
        TOK_IDENTIFIER,
        TOK_PERCENT,
        TOK_INTEGER,
        TOK_SEMICOLON,
        TOK_IDENTIFIER,
        TOK_LBRACKET,
        TOK_IDENTIFIER,
        TOK_RBRACKET,
        TOK_SEMICOLON,
        TOK_IDENTIFIER,
        TOK_ASSIGN,
        TOK_MINUS,
        TOK_IDENTIFIER,
        TOK_PLUS,
        TOK_STAR,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
        TOK_INC,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
        TOK_IDENTIFIER,
        TOK_AMP,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
        TOK_RBRACE,
    ];
    let tok_lens = vec![
        2, 2, 5, 1, 6, 1, 1, 2, 1, 1, 2, 1, 5, 2, 3, 2, 3, 2, 1, 1, 1, 1, 1, 1, 1, 2, 1, 5, 2, 1,
        1, 3, 2, 1, 1, 3, 1, 1, 1, 3, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2, 1, 1, 1, 1, 4, 1, 1, 1,
    ];
    let tok_starts = starts_for_lens(&tok_lens);
    (tok_types, tok_starts, tok_lens)
}
fn c_declarator_initializer_fixture_tokens() -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_STRUCT,
        TOK_IDENTIFIER,
        TOK_LBRACE,
        TOK_INT,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
        TOK_CONST,
        TOK_CHAR_KW,
        TOK_STAR,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
        TOK_LONG,
        TOK_IDENTIFIER,
        TOK_LBRACKET,
        TOK_INTEGER,
        TOK_RBRACKET,
        TOK_SEMICOLON,
        TOK_RBRACE,
        TOK_SEMICOLON,
        TOK_ENUM,
        TOK_IDENTIFIER,
        TOK_LBRACE,
        TOK_IDENTIFIER,
        TOK_ASSIGN,
        TOK_INTEGER,
        TOK_COMMA,
        TOK_IDENTIFIER,
        TOK_COMMA,
        TOK_IDENTIFIER,
        TOK_ASSIGN,
        TOK_INTEGER,
        TOK_RBRACE,
        TOK_SEMICOLON,
        TOK_INT,
        TOK_IDENTIFIER,
        TOK_LPAREN,
        TOK_INT,
        TOK_RPAREN,
        TOK_SEMICOLON,
        TOK_INT,
        TOK_STAR,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
        TOK_INT,
        TOK_IDENTIFIER,
        TOK_ASSIGN,
        TOK_LPAREN,
        TOK_INT,
        TOK_STAR,
        TOK_RPAREN,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
        TOK_STRUCT,
        TOK_IDENTIFIER,
        TOK_IDENTIFIER,
        TOK_ASSIGN,
        TOK_LPAREN,
        TOK_STRUCT,
        TOK_IDENTIFIER,
        TOK_RPAREN,
        TOK_LBRACE,
        TOK_DOT,
        TOK_IDENTIFIER,
        TOK_ASSIGN,
        TOK_INTEGER,
        TOK_COMMA,
        TOK_DOT,
        TOK_IDENTIFIER,
        TOK_ASSIGN,
        TOK_STRING,
        TOK_RBRACE,
        TOK_SEMICOLON,
    ];
    let tok_lens = vec![
        6, 5, 1, 3, 1, 1, 5, 4, 1, 4, 1, 4, 6, 1, 1, 1, 1, 1, 1, 4, 5, 1, 3, 1, 1, 1, 5, 1, 4, 1,
        1, 1, 1, 3, 2, 1, 3, 1, 1, 3, 1, 1, 1, 3, 1, 1, 1, 3, 1, 1, 1, 1, 6, 5, 1, 1, 1, 6, 5, 1,
        1, 1, 1, 1, 1, 1, 1, 4, 1, 3, 1, 1,
    ];
    let tok_starts = starts_for_lens(&tok_lens);
    (tok_types, tok_starts, tok_lens)
}
fn c_function_pointer_array_prototype_fixture_tokens() -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_INT,
        TOK_LPAREN,
        TOK_STAR,
        TOK_IDENTIFIER,
        TOK_LBRACKET,
        TOK_STATIC,
        TOK_INTEGER,
        TOK_RBRACKET,
        TOK_RPAREN,
        TOK_LPAREN,
        TOK_STRUCT,
        TOK_IDENTIFIER,
        TOK_STAR,
        TOK_IDENTIFIER,
        TOK_COMMA,
        TOK_CONST,
        TOK_CHAR_KW,
        TOK_IDENTIFIER,
        TOK_LBRACKET,
        TOK_STATIC,
        TOK_INTEGER,
        TOK_RBRACKET,
        TOK_RPAREN,
        TOK_SEMICOLON,
    ];
    let tok_lens = vec![
        3, 1, 1, 8, 1, 6, 1, 1, 1, 1, 6, 4, 1, 4, 1, 5, 4, 4, 1, 6, 2, 1, 1, 1,
    ];
    let tok_starts = starts_for_lens(&tok_lens);
    (tok_types, tok_starts, tok_lens)
}
fn entry() -> &'static OpEntry {
    all_entries()
        .find(|entry| entry.id == OP_ID)
        .unwrap_or_else(|| panic!("Fix: missing OpEntry for {OP_ID}"))
}
fn assert_reference_witnesses(
    program: &Program,
    inputs: &[Vec<Vec<u8>>],
    expected: &[Vec<Vec<u8>>],
) {
    assert_eq!(
        inputs.len(),
        expected.len(),
        "Fix: every witness input case must have an expected output case"
    );
    for (case_idx, (case_inputs, case_expected)) in inputs.iter().zip(expected).enumerate() {
        let actual = run_reference_eval(program, case_inputs);
        assert_eq!(
            actual, *case_expected,
            "Fix: witness case {case_idx} must match CPU reference output"
        );
    }
}
fn run_reference_eval(program: &Program, inputs: &[Vec<u8>]) -> Vec<Vec<u8>> {
    let owned_inputs;
    let inputs = if inputs.len() == 1 {
        let output_len = node_count_from_vast(&inputs[0])
            .saturating_mul(PG_STRIDE_U32)
            .max(1) as usize
            * 4;
        owned_inputs = vec![inputs[0].clone(), vec![0; output_len]];
        owned_inputs.as_slice()
    } else {
        inputs
    };
    let values = inputs.iter().cloned().map(Value::from).collect::<Vec<_>>();
    vyre_reference::reference_eval(program, &values)
        .unwrap_or_else(|error| panic!("Fix: CPU reference must execute: {error}"))
        .into_iter()
        .map(|value| value.to_bytes())
        .collect()
}
fn node_count_from_vast(bytes: &[u8]) -> u32 {
    u32::try_from(bytes.len() / VAST_STRIDE_BYTES).unwrap_or_default()
}
fn emit_wgsl(program: &Program) -> String {
    let module = naga_emit::emit_module(program, &DispatchConfig::default(), TEST_WORKGROUP_SIZE)
        .expect("Fix: program must lower to a Naga module");
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&module)
    .expect("Fix: emitted Naga module must validate");
    naga::back::wgsl::write_string(&module, &info, naga::back::wgsl::WriterFlags::empty())
        .expect("Fix: Naga module must serialize to WGSL")
}
fn build_vast_node(
    kind: u32,
    parent_idx: u32,
    span_start: u32,
    span_len: u32,
    attr_off: u32,
    attr_len: u32,
) -> Vec<u32> {
    vec![
        kind,
        parent_idx,
        u32::MAX,
        u32::MAX,
        u32::MAX,
        span_start,
        span_len,
        attr_off,
        attr_len,
        u32::MAX,
    ]
}
fn build_vast(nodes: &[Vec<u32>]) -> Vec<u8> {
    nodes.iter().flat_map(|node| bytes(node)).collect()
}
fn assert_pg_row(
    rows: &[u8],
    idx: usize,
    kind: u32,
    parent: u32,
    first_child: u32,
    next_sibling: u32,
) {
    let row = idx * PG_STRIDE_U32 as usize;
    assert_eq!(word_at(rows, row), kind, "pg kind[{idx}]");
    assert_eq!(word_at(rows, row + 3), parent, "pg parent[{idx}]");
    assert_eq!(word_at(rows, row + 4), first_child, "pg first_child[{idx}]");
    assert_eq!(
        word_at(rows, row + 5),
        next_sibling,
        "pg next_sibling[{idx}]"
    );
}

fn adversarial_vast_cases() -> Vec<Vec<u8>> {
    let mut cases = Vec::with_capacity(64);
    for case_idx in 0..60 {
        let node_count = (case_idx % 16) + 1;
        let mut nodes = Vec::new();
        let seed = u32::try_from(case_idx).unwrap_or_default();
        for node_idx in 0..node_count {
            let kind = match (seed + node_idx) % 6 {
                0 => node_kind::VARIABLE,
                1 => node_kind::CALL,
                2 => node_kind::IMPORT,
                3 => node_kind::LITERAL,
                4 => node_kind::SSA,
                _ => node_kind::BASIC_BLOCK,
            };
            let parent = if node_idx == 0 {
                u32::MAX
            } else if node_idx % 3 == 0 {
                u32::MAX
            } else {
                seed.wrapping_mul(0x9E37_79B9)
                    .wrapping_add(u32::try_from(node_idx).unwrap_or_default())
            };
            let span_start = seed
                .rotate_left((node_idx % 32) as u32)
                .wrapping_add(u32::try_from(node_idx).unwrap_or_default().wrapping_mul(17));
            let span_len = if node_idx % 4 == 0 {
                u32::MAX
            } else {
                seed.wrapping_mul(97)
                    .wrapping_add(u32::try_from(node_idx).unwrap_or_default())
            };
            let attr_off = seed
                .wrapping_mul(31)
                .wrapping_add(u32::try_from(node_idx).unwrap_or_default() * 13);
            let attr_len = if node_idx % 2 == 0 {
                0
            } else {
                seed.wrapping_mul(9)
                    .wrapping_add(u32::try_from(node_idx).unwrap_or_default() * 7)
            };
            nodes.push(build_vast_node(
                kind, parent, span_start, span_len, attr_off, attr_len,
            ));
        }
        cases.push(build_vast(&nodes));
    }
    cases
}
mod c_lower_ast_to_pg_nodes_part1 {
    include!("__split/c_lower_ast_to_pg_nodes_part1.rs");
}
mod c_lower_ast_to_pg_nodes_part2 {
    include!("__split/c_lower_ast_to_pg_nodes_part2.rs");
}
mod c_lower_ast_to_pg_nodes_part3 {
    include!("__split/c_lower_ast_to_pg_nodes_part3.rs");
}
mod c_lower_ast_to_pg_nodes_part4 {
    include!("__split/c_lower_ast_to_pg_nodes_part4.rs");
}
mod c_lower_ast_to_pg_nodes_part5 {
    include!("__split/c_lower_ast_to_pg_nodes_part5.rs");
}
