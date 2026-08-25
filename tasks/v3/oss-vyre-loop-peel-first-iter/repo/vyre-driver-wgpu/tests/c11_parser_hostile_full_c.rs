//! Failure-oriented hostile-parser tests for full-C constructs.
//!
//! Targets the VYRE C AST parser (VAST builder + classifier + PG lowerer)
//! with table-driven edge cases that historically break C parsers:
//!
//!   * typedef/expression ambiguity (the "most vexing parse" family)
//!   * nested declarators (function-pointer arrays, parenthesised names)
//!   * compound literals vs casts
//!   * designated initialisers (nested, mixed dot/array)
//!   * GNU attributes and inline asm
//!   * nested structs / enums / unions
//!
//! Every case asserts concrete VAST/PG node kinds.  Where a GPU program
//! exists for the stage under test we also assert CPU/GPU parity.

#![cfg(feature = "c-parser")]
#![allow(clippy::erasing_op)]
#![allow(deprecated)]
use std::sync::OnceLock;

use vyre::ir::Expr;
use vyre::VyreBackend;
use vyre_driver_wgpu::WgpuBackend;
use vyre_libs::parsing::c::lex::tokens::*;
use vyre_libs::parsing::c::lower::{c_lower_ast_to_pg_nodes, reference_ast_to_pg_nodes};
use vyre_libs::parsing::c::parse::vast::{
    c11_build_vast_nodes, c11_classify_vast_node_kinds, reference_c11_build_vast_nodes,
    reference_c11_classify_vast_node_kinds, C_AST_KIND_ARRAY_DECL, C_AST_KIND_ARRAY_SUBSCRIPT_EXPR,
    C_AST_KIND_CAST_EXPR, C_AST_KIND_COMPOUND_LITERAL_EXPR, C_AST_KIND_ENUMERATOR_DECL,
    C_AST_KIND_FIELD_DECL, C_AST_KIND_FUNCTION_DEFINITION, C_AST_KIND_GNU_ATTRIBUTE,
    C_AST_KIND_INITIALIZER_LIST, C_AST_KIND_INLINE_ASM, C_AST_KIND_POINTER_DECL,
    C_AST_KIND_SIZEOF_EXPR,
};
use vyre_primitives::predicate::node_kind;

const VAST_STRIDE_U32: usize = 10;
const PG_STRIDE_U32: usize = 6;

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

fn starts_for_lens(lens: &[u32]) -> Vec<u32> {
    let mut cursor = 0u32;
    lens.iter()
        .map(|len| {
            let start = cursor;
            cursor = cursor.saturating_add(*len).saturating_add(1);
            start
        })
        .collect()
}

fn assert_vast_row(
    rows: &[u8],
    idx: usize,
    kind: u32,
    parent: u32,
    first_child: u32,
    next_sibling: u32,
) {
    let row = idx * VAST_STRIDE_U32;
    assert_eq!(word_at(rows, row), kind, "kind[{idx}]");
    assert_eq!(word_at(rows, row + 1), parent, "parent[{idx}]");
    assert_eq!(word_at(rows, row + 2), first_child, "first_child[{idx}]");
    assert_eq!(word_at(rows, row + 3), next_sibling, "next_sibling[{idx}]");
}

fn assert_kind(rows: &[u8], idx: usize, kind: u32) {
    assert_eq!(word_at(rows, idx * VAST_STRIDE_U32), kind, "kind[{idx}]");
}

fn typed_indices(rows: &[u8], kind: u32) -> Vec<usize> {
    rows.chunks_exact(VAST_STRIDE_U32 * 4)
        .enumerate()
        .filter_map(|(idx, row)| {
            let row_kind = u32::from_le_bytes(row[0..4].try_into().unwrap());
            (row_kind == kind).then_some(idx)
        })
        .collect()
}

fn pg_word_at(buf: &[u8], idx: usize, field: usize) -> u32 {
    word_at(buf, idx * PG_STRIDE_U32 + field)
}

fn gpu_backend() -> &'static WgpuBackend {
    static BACKEND: OnceLock<WgpuBackend> = OnceLock::new();
    BACKEND.get_or_init(|| {
        WgpuBackend::acquire().expect(
            "WgpuBackend::acquire failed on a machine that must have a GPU. \
             Per project GPU rule, this is a configuration bug, not a graceful skip.",
        )
    })
}

fn run_gpu_vast_builder(tok_types: &[u32], tok_starts: &[u32], tok_lens: &[u32]) -> Vec<u8> {
    let program = c11_build_vast_nodes(
        "tok_types",
        "tok_starts",
        "tok_lens",
        Expr::u32(tok_types.len() as u32),
        "out_vast_nodes",
        "out_count",
    );
    let tok_type_bytes = bytes(tok_types);
    let tok_start_bytes = bytes(tok_starts);
    let tok_len_bytes = bytes(tok_lens);
    let inputs: Vec<&[u8]> = vec![&tok_type_bytes, &tok_start_bytes, &tok_len_bytes];
    let outputs = gpu_backend()
        .dispatch_borrowed(&program, &inputs, &Default::default())
        .expect("GPU VAST builder dispatch must succeed");
    assert_eq!(outputs.len(), 2);
    outputs[0].clone()
}

fn run_gpu_classifier(raw_vast: &[u8], num_nodes: u32) -> Vec<u8> {
    let program =
        c11_classify_vast_node_kinds("vast_nodes", Expr::u32(num_nodes), "typed_vast_nodes");
    let inputs: Vec<&[u8]> = vec![raw_vast];
    let outputs = gpu_backend()
        .dispatch_borrowed(&program, &inputs, &Default::default())
        .expect("GPU classifier dispatch must succeed");
    assert_eq!(outputs.len(), 1);
    outputs[0].clone()
}

fn run_gpu_pg_lower(typed_vast: &[u8], num_nodes: u32) -> Vec<u8> {
    let program = c_lower_ast_to_pg_nodes("vast_nodes", Expr::u32(num_nodes), "out_pg_nodes");
    let inputs: Vec<&[u8]> = vec![typed_vast];
    let outputs = gpu_backend()
        .dispatch_borrowed(&program, &inputs, &Default::default())
        .expect("GPU PG lowerer dispatch must succeed");
    assert_eq!(outputs.len(), 1);
    outputs[0].clone()
}

// ---------------------------------------------------------------------------
// Hostile fixtures
// ---------------------------------------------------------------------------

/// typedef int Foo;
/// void bar(void) {
///   Foo *a;          -- typedef-name declarator
///   (Foo)*b;         -- cast expression (type-name paren without decl context)
///   (Foo)-1;         -- cast expression
///   c = (Foo){ .x=1 }; -- compound literal + initializer list
/// }
fn fixture_typedef_expr_ambiguity() -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_TYPEDEF,
        TOK_INT,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
        TOK_VOID,
        TOK_IDENTIFIER,
        TOK_LPAREN,
        TOK_VOID,
        TOK_RPAREN,
        TOK_LBRACE,
        TOK_IDENTIFIER, // Foo
        TOK_STAR,
        TOK_IDENTIFIER, // a
        TOK_SEMICOLON,
        TOK_LPAREN, // (Foo)
        TOK_IDENTIFIER,
        TOK_RPAREN,
        TOK_STAR,
        TOK_IDENTIFIER, // b
        TOK_SEMICOLON,
        TOK_LPAREN, // (Foo)
        TOK_IDENTIFIER,
        TOK_RPAREN,
        TOK_MINUS,
        TOK_INTEGER,
        TOK_SEMICOLON,
        TOK_IDENTIFIER, // c
        TOK_ASSIGN,
        TOK_LPAREN, // (Foo)
        TOK_IDENTIFIER,
        TOK_RPAREN,
        TOK_LBRACE,
        TOK_DOT,
        TOK_IDENTIFIER,
        TOK_ASSIGN,
        TOK_INTEGER,
        TOK_RBRACE,
        TOK_SEMICOLON,
        TOK_RBRACE,
    ];
    let tok_lens = vec![1; tok_types.len()];
    let tok_starts = starts_for_lens(&tok_lens);
    (tok_types, tok_starts, tok_lens)
}

/// int (*(*f[4])(int))[2];
/// Deeply nested: the classifier loses the decl context after the first
/// parenthesis, so only the outermost star is a POINTER_DECL.
fn fixture_deeply_nested_declarator() -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_INT,
        TOK_LPAREN,
        TOK_STAR,
        TOK_LPAREN,
        TOK_STAR,
        TOK_IDENTIFIER, // f
        TOK_LBRACKET,
        TOK_INTEGER, // 4
        TOK_RBRACKET,
        TOK_RPAREN,
        TOK_LPAREN,
        TOK_INT,
        TOK_RPAREN,
        TOK_RPAREN,
        TOK_LBRACKET,
        TOK_INTEGER, // 2
        TOK_RBRACKET,
        TOK_SEMICOLON,
    ];
    let tok_lens = vec![1; tok_types.len()];
    let tok_starts = starts_for_lens(&tok_lens);
    (tok_types, tok_starts, tok_lens)
}

/// struct S { int a; struct { int b; } nested; };
/// enum E { A, B };
fn fixture_nested_struct_enum() -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_STRUCT,
        TOK_IDENTIFIER, // S
        TOK_LBRACE,
        TOK_INT,
        TOK_IDENTIFIER, // a
        TOK_SEMICOLON,
        TOK_STRUCT,
        TOK_LBRACE,
        TOK_INT,
        TOK_IDENTIFIER, // b
        TOK_SEMICOLON,
        TOK_RBRACE,
        TOK_IDENTIFIER, // nested
        TOK_SEMICOLON,
        TOK_RBRACE,
        TOK_SEMICOLON,
        TOK_ENUM,
        TOK_IDENTIFIER, // E
        TOK_LBRACE,
        TOK_IDENTIFIER, // A
        TOK_COMMA,
        TOK_IDENTIFIER, // B
        TOK_RBRACE,
        TOK_SEMICOLON,
    ];
    let tok_lens = vec![1; tok_types.len()];
    let tok_starts = starts_for_lens(&tok_lens);
    (tok_types, tok_starts, tok_lens)
}

/// int x[] = { [0] = 1, [1] = { [2] = 3, [0] = 4 } };
fn fixture_nested_designated_init() -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_INT,
        TOK_IDENTIFIER, // x
        TOK_LBRACKET,
        TOK_RBRACKET,
        TOK_ASSIGN,
        TOK_LBRACE,
        TOK_LBRACKET,
        TOK_INTEGER,
        TOK_RBRACKET,
        TOK_ASSIGN,
        TOK_INTEGER,
        TOK_COMMA,
        TOK_LBRACKET,
        TOK_INTEGER,
        TOK_RBRACKET,
        TOK_ASSIGN,
        TOK_LBRACE,
        TOK_LBRACKET,
        TOK_INTEGER,
        TOK_RBRACKET,
        TOK_ASSIGN,
        TOK_INTEGER,
        TOK_COMMA,
        TOK_LBRACKET,
        TOK_INTEGER,
        TOK_RBRACKET,
        TOK_ASSIGN,
        TOK_INTEGER,
        TOK_RBRACE,
        TOK_RBRACE,
        TOK_SEMICOLON,
    ];
    let tok_lens = vec![1; tok_types.len()];
    let tok_starts = starts_for_lens(&tok_lens);
    (tok_types, tok_starts, tok_lens)
}

/// __attribute__((noreturn)) void die(int code) {
///   __asm__ volatile ("ud2" ::: "memory");
/// }
fn fixture_gnu_attribute_inline_asm() -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_GNU_ATTRIBUTE,
        TOK_LPAREN,
        TOK_LPAREN,
        TOK_IDENTIFIER, // noreturn
        TOK_RPAREN,
        TOK_RPAREN,
        TOK_VOID,
        TOK_IDENTIFIER, // die
        TOK_LPAREN,
        TOK_INT,
        TOK_IDENTIFIER, // code
        TOK_RPAREN,
        TOK_LBRACE,
        TOK_GNU_ASM,
        TOK_VOLATILE,
        TOK_LPAREN,
        TOK_STRING, // "ud2"
        TOK_COLON,
        TOK_COLON,
        TOK_COLON,
        TOK_STRING, // "memory"
        TOK_RPAREN,
        TOK_SEMICOLON,
        TOK_RBRACE,
    ];
    let tok_lens = vec![1; tok_types.len()];
    let tok_starts = starts_for_lens(&tok_lens);
    (tok_types, tok_starts, tok_lens)
}

/// void f(void) {
///   int *p = (int []){ 1, 2, 3 };
///   struct S *s = (struct S){ .a = 1 };
/// }
fn fixture_compound_literal_stress() -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_VOID,
        TOK_IDENTIFIER, // f
        TOK_LPAREN,
        TOK_VOID,
        TOK_RPAREN,
        TOK_LBRACE,
        TOK_INT,
        TOK_STAR,
        TOK_IDENTIFIER, // p
        TOK_ASSIGN,
        TOK_LPAREN, // (int [])
        TOK_INT,
        TOK_LBRACKET,
        TOK_RBRACKET,
        TOK_RPAREN,
        TOK_LBRACE,
        TOK_INTEGER,
        TOK_COMMA,
        TOK_INTEGER,
        TOK_COMMA,
        TOK_INTEGER,
        TOK_RBRACE,
        TOK_SEMICOLON,
        TOK_STRUCT,
        TOK_IDENTIFIER, // S
        TOK_STAR,
        TOK_IDENTIFIER, // s
        TOK_ASSIGN,
        TOK_LPAREN, // (struct S)
        TOK_STRUCT,
        TOK_IDENTIFIER,
        TOK_RPAREN,
        TOK_LBRACE,
        TOK_DOT,
        TOK_IDENTIFIER, // a
        TOK_ASSIGN,
        TOK_INTEGER,
        TOK_RBRACE,
        TOK_SEMICOLON,
        TOK_RBRACE,
    ];
    let tok_lens = vec![1; tok_types.len()];
    let tok_starts = starts_for_lens(&tok_lens);
    (tok_types, tok_starts, tok_lens)
}

// ---------------------------------------------------------------------------
// Table-driven CPU reference tests
// ---------------------------------------------------------------------------

mod c11_parser_hostile_full_c_part1 {

    include!("__split/c11_parser_hostile_full_c_part1.rs");
}
mod c11_parser_hostile_full_c_part2 {
    include!("__split/c11_parser_hostile_full_c_part2.rs");
}
