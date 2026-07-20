//! End-to-end GPU/CPU parity tests for C expression precedence and associativity.
//!
//! Covers: comma boundaries, assignment chains, nested ternary, logical/bitwise
//! precedence ladders, cast vs parenthesized expression typing, postfix
//! call/index/member, and unary chains.  Every fixture asserts both expression
//! shape rows and PG lowering (kind, span, and tree-link preservation).

#![cfg(feature = "c-parser")]
#![allow(clippy::too_many_arguments, clippy::erasing_op)]
#![allow(deprecated)]

use vyre_libs::parsing::c::lex::tokens::*;
use vyre_libs::parsing::c::parse::vast::{
    reference_c11_build_expression_shape_nodes, reference_c11_build_vast_nodes,
    reference_c11_classify_vast_node_kinds, C_AST_KIND_ARRAY_SUBSCRIPT_EXPR,
    C_AST_KIND_ASSIGN_EXPR, C_AST_KIND_CAST_EXPR, C_AST_KIND_CONDITIONAL_EXPR,
    C_AST_KIND_MEMBER_ACCESS_EXPR, C_AST_KIND_UNARY_EXPR, C_EXPR_ASSOC_LEFT, C_EXPR_ASSOC_NONE,
    C_EXPR_ASSOC_RIGHT, C_EXPR_SHAPE_BINARY, C_EXPR_SHAPE_CONDITIONAL, C_EXPR_SHAPE_NONE,
};
use vyre_primitives::predicate::node_kind;

mod c_ast_expression_support;
mod c_ast_gpu_parity_support;
use c_ast_expression_support::{
    assert_pg_links_match_vast, assert_pg_preserves_row, assert_shape_row, bytes, row_indices,
    run_pipeline, run_reference_pg_lower, starts_for_lens, unit_lens_fixture, word_at, SENTINEL,
    VAST_STRIDE_U32,
};
use c_ast_gpu_parity_support::{run_gpu_expr_shape, run_gpu_pg_lower};

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

fn comma_fixture() -> (Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_IDENTIFIER,
        TOK_ASSIGN,
        TOK_IDENTIFIER,
        TOK_COMMA,
        TOK_IDENTIFIER,
        TOK_ASSIGN,
        TOK_IDENTIFIER,
        TOK_COMMA,
        TOK_IDENTIFIER,
        TOK_ASSIGN,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
    ];
    unit_lens_fixture(tok_types)
}

fn assignment_chain_fixture() -> (Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_IDENTIFIER,
        TOK_ASSIGN,
        TOK_IDENTIFIER,
        TOK_ASSIGN,
        TOK_IDENTIFIER,
        TOK_ASSIGN,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
    ];
    unit_lens_fixture(tok_types)
}

fn ternary_nesting_fixture() -> (Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_IDENTIFIER,
        TOK_QUESTION,
        TOK_IDENTIFIER,
        TOK_QUESTION,
        TOK_IDENTIFIER,
        TOK_COLON,
        TOK_IDENTIFIER,
        TOK_COLON,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
    ];
    unit_lens_fixture(tok_types)
}

fn logical_bitwise_fixture() -> (Vec<u32>, Vec<u32>) {
    // a || b && c | d ^ e & f == g < h + i * j;
    let tok_types = vec![
        TOK_IDENTIFIER,
        TOK_OR,
        TOK_IDENTIFIER,
        TOK_AND,
        TOK_IDENTIFIER,
        TOK_PIPE,
        TOK_IDENTIFIER,
        TOK_CARET,
        TOK_IDENTIFIER,
        TOK_AMP,
        TOK_IDENTIFIER,
        TOK_EQ,
        TOK_IDENTIFIER,
        TOK_LT,
        TOK_IDENTIFIER,
        TOK_PLUS,
        TOK_IDENTIFIER,
        TOK_STAR,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
    ];
    unit_lens_fixture(tok_types)
}

fn cast_vs_paren_fixture() -> (Vec<u32>, Vec<u32>) {
    // (int)a; (b + c);
    let tok_types = vec![
        TOK_LPAREN,
        TOK_INT,
        TOK_RPAREN,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
        TOK_LPAREN,
        TOK_IDENTIFIER,
        TOK_PLUS,
        TOK_IDENTIFIER,
        TOK_RPAREN,
        TOK_SEMICOLON,
    ];
    unit_lens_fixture(tok_types)
}

fn postfix_fixture() -> (Vec<u32>, Vec<u32>) {
    // a(b); a[b]; a.c; a->d;
    let tok_types = vec![
        TOK_IDENTIFIER,
        TOK_LPAREN,
        TOK_IDENTIFIER,
        TOK_RPAREN,
        TOK_SEMICOLON,
        TOK_IDENTIFIER,
        TOK_LBRACKET,
        TOK_IDENTIFIER,
        TOK_RBRACKET,
        TOK_SEMICOLON,
        TOK_IDENTIFIER,
        TOK_DOT,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
        TOK_IDENTIFIER,
        TOK_ARROW,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
    ];
    unit_lens_fixture(tok_types)
}

fn unary_chain_fixture() -> (Vec<u32>, Vec<u32>) {
    // !~-*&++a;
    let tok_types = vec![
        TOK_BANG,
        TOK_TILDE,
        TOK_MINUS,
        TOK_STAR,
        TOK_AMP,
        TOK_INC,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
    ];
    unit_lens_fixture(tok_types)
}

// ---------------------------------------------------------------------------
// CPU shape + PG lowering tests
// ---------------------------------------------------------------------------

mod c_ast_expression_precedence_e2e_part1 {

    include!("__split/c_ast_expression_precedence_e2e_part1.rs");
}
mod c_ast_expression_precedence_e2e_part2 {
    include!("__split/c_ast_expression_precedence_e2e_part2.rs");
}
