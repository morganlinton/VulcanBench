//! GPU parity tests for token-stream to VAST row construction.
#![cfg(feature = "c-parser")]
#![allow(clippy::too_many_arguments)]
#![allow(deprecated)]
use vyre::ir::Expr;
use vyre::VyreBackend;
use vyre_driver_wgpu::WgpuBackend;
use vyre_libs::parsing::c::lex::tokens::*;
use vyre_libs::parsing::c::parse::vast::{
    c11_build_expression_shape_nodes, c11_build_vast_nodes, c11_classify_vast_node_kinds,
    reference_c11_build_expression_shape_nodes, reference_c11_build_vast_nodes,
    reference_c11_classify_vast_node_kinds, C_AST_KIND_ARRAY_DECL, C_AST_KIND_ARRAY_SUBSCRIPT_EXPR,
    C_AST_KIND_ASM_CLOBBERS_LIST, C_AST_KIND_ASM_TEMPLATE, C_AST_KIND_ASSIGN_EXPR,
    C_AST_KIND_BREAK_STMT, C_AST_KIND_CASE_STMT, C_AST_KIND_CAST_EXPR,
    C_AST_KIND_COMPOUND_LITERAL_EXPR, C_AST_KIND_CONDITIONAL_EXPR, C_AST_KIND_CONTINUE_STMT,
    C_AST_KIND_DEFAULT_STMT, C_AST_KIND_DO_STMT, C_AST_KIND_ELSE_STMT, C_AST_KIND_ENUMERATOR_DECL,
    C_AST_KIND_FIELD_DECL, C_AST_KIND_FOR_STMT, C_AST_KIND_FUNCTION_DECLARATOR,
    C_AST_KIND_FUNCTION_DEFINITION, C_AST_KIND_GNU_ATTRIBUTE, C_AST_KIND_GOTO_STMT,
    C_AST_KIND_IF_STMT, C_AST_KIND_INITIALIZER_LIST, C_AST_KIND_INLINE_ASM,
    C_AST_KIND_MEMBER_ACCESS_EXPR, C_AST_KIND_POINTER_DECL, C_AST_KIND_RETURN_STMT,
    C_AST_KIND_SIZEOF_EXPR, C_AST_KIND_SWITCH_STMT, C_AST_KIND_UNARY_EXPR, C_AST_KIND_WHILE_STMT,
    C_EXPR_ASSOC_LEFT, C_EXPR_ASSOC_RIGHT, C_EXPR_SHAPE_BINARY, C_EXPR_SHAPE_CONDITIONAL,
    C_EXPR_SHAPE_STRIDE_U32,
};
use vyre_primitives::predicate::node_kind;

mod c_ast_expression_support;

use c_ast_expression_support::{bytes, starts_for_lens, word_at};

const VAST_STRIDE_U32: usize = 10;

fn expression_operator_fixture() -> (Vec<u32>, Vec<u32>, Vec<u32>) {
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
fn expression_shape_fixture() -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_IDENTIFIER,
        TOK_PLUS,
        TOK_IDENTIFIER,
        TOK_STAR,
        TOK_IDENTIFIER,
        TOK_QUESTION,
        TOK_IDENTIFIER,
        TOK_PLUS,
        TOK_IDENTIFIER,
        TOK_COLON,
        TOK_IDENTIFIER,
        TOK_STAR,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
    ];
    let tok_lens = vec![1; tok_types.len()];
    let tok_starts = starts_for_lens(&tok_lens);
    (tok_types, tok_starts, tok_lens)
}
fn declarator_initializer_fixture() -> (Vec<u32>, Vec<u32>, Vec<u32>) {
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
fn function_pointer_array_prototype_fixture() -> (Vec<u32>, Vec<u32>, Vec<u32>) {
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
fn cast_vs_declaration_fixture() -> (Vec<u32>, Vec<u32>, Vec<u32>) {
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
        TOK_LPAREN,
        TOK_IDENTIFIER,
        TOK_RPAREN,
        TOK_STAR,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
        TOK_IDENTIFIER,
        TOK_STAR,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
        TOK_IDENTIFIER,
        TOK_ASSIGN,
        TOK_LPAREN,
        TOK_IDENTIFIER,
        TOK_RPAREN,
        TOK_MINUS,
        TOK_INTEGER,
        TOK_SEMICOLON,
        TOK_IDENTIFIER,
        TOK_ASSIGN,
        TOK_LPAREN,
        TOK_IDENTIFIER,
        TOK_RPAREN,
        TOK_LBRACE,
        TOK_DOT,
        TOK_IDENTIFIER,
        TOK_ASSIGN,
        TOK_INTEGER,
        TOK_COMMA,
        TOK_LBRACKET,
        TOK_INTEGER,
        TOK_RBRACKET,
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
fn nested_designated_initializer_fixture() -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_STRUCT,
        TOK_IDENTIFIER,
        TOK_LBRACE,
        TOK_INT,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
        TOK_INT,
        TOK_IDENTIFIER,
        TOK_LBRACKET,
        TOK_INTEGER,
        TOK_RBRACKET,
        TOK_SEMICOLON,
        TOK_RBRACE,
        TOK_IDENTIFIER,
        TOK_ASSIGN,
        TOK_LBRACE,
        TOK_DOT,
        TOK_IDENTIFIER,
        TOK_ASSIGN,
        TOK_INTEGER,
        TOK_COMMA,
        TOK_DOT,
        TOK_IDENTIFIER,
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
fn qualified_function_pointer_array_fixture() -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_EXTERN,
        TOK_THREAD_LOCAL,
        TOK_INT,
        TOK_LPAREN,
        TOK_STAR,
        TOK_IDENTIFIER,
        TOK_LBRACKET,
        TOK_INTEGER,
        TOK_RBRACKET,
        TOK_RPAREN,
        TOK_LPAREN,
        TOK_INT,
        TOK_RPAREN,
        TOK_SEMICOLON,
        TOK_STATIC,
        TOK_GNU_ATTRIBUTE,
        TOK_LPAREN,
        TOK_LPAREN,
        TOK_IDENTIFIER,
        TOK_RPAREN,
        TOK_RPAREN,
        TOK_INT,
        TOK_LPAREN,
        TOK_STAR,
        TOK_IDENTIFIER,
        TOK_LBRACKET,
        TOK_INTEGER,
        TOK_RBRACKET,
        TOK_RPAREN,
        TOK_LPAREN,
        TOK_INT,
        TOK_COMMA,
        TOK_INT,
        TOK_RPAREN,
        TOK_SEMICOLON,
    ];
    let tok_lens = vec![1; tok_types.len()];
    let tok_starts = starts_for_lens(&tok_lens);
    (tok_types, tok_starts, tok_lens)
}
fn anonymous_aggregate_declarator_fixture() -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_STRUCT,
        TOK_IDENTIFIER,
        TOK_LBRACE,
        TOK_STRUCT,
        TOK_LBRACE,
        TOK_INT,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
        TOK_RBRACE,
        TOK_LPAREN,
        TOK_STAR,
        TOK_IDENTIFIER,
        TOK_LBRACKET,
        TOK_INTEGER,
        TOK_RBRACKET,
        TOK_RPAREN,
        TOK_LPAREN,
        TOK_INT,
        TOK_RPAREN,
        TOK_SEMICOLON,
        TOK_ENUM,
        TOK_LBRACE,
        TOK_IDENTIFIER,
        TOK_COMMA,
        TOK_IDENTIFIER,
        TOK_ASSIGN,
        TOK_INTEGER,
        TOK_RBRACE,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
        TOK_UNION,
        TOK_LBRACE,
        TOK_INT,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
        TOK_FLOAT_KW,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
        TOK_RBRACE,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
        TOK_RBRACE,
        TOK_SEMICOLON,
    ];
    let tok_lens = vec![1; tok_types.len()];
    let tok_starts = starts_for_lens(&tok_lens);
    (tok_types, tok_starts, tok_lens)
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
fn assert_expr_shape_row(
    rows: &[u8],
    idx: usize,
    shape_kind: u32,
    raw_operator: u32,
    precedence: u32,
    associativity: u32,
    first: u32,
    second: u32,
    third: u32,
) {
    let row = idx * C_EXPR_SHAPE_STRIDE_U32 as usize;
    assert_eq!(word_at(rows, row), shape_kind, "shape_kind[{idx}]");
    assert_eq!(word_at(rows, row + 1), idx as u32, "source_idx[{idx}]");
    assert_eq!(word_at(rows, row + 2), raw_operator, "raw_operator[{idx}]");
    assert_eq!(word_at(rows, row + 3), precedence, "precedence[{idx}]");
    assert_eq!(
        word_at(rows, row + 4),
        associativity,
        "associativity[{idx}]"
    );
    assert_eq!(word_at(rows, row + 5), first, "first[{idx}]");
    assert_eq!(word_at(rows, row + 6), second, "second[{idx}]");
    assert_eq!(word_at(rows, row + 7), third, "third[{idx}]");
}
mod c11_build_vast_nodes_part1 {
    include!("__split/c11_build_vast_nodes_part1.rs");
}
mod c11_build_vast_nodes_part2 {
    include!("__split/c11_build_vast_nodes_part2.rs");
}
mod c11_build_vast_nodes_part3 {
    include!("__split/c11_build_vast_nodes_part3.rs");
}
mod c11_build_vast_nodes_part4 {
    include!("__split/c11_build_vast_nodes_part4.rs");
}
mod c11_build_vast_nodes_part5 {
    include!("__split/c11_build_vast_nodes_part5.rs");
}
