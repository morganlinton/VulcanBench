//! Contracts for C expression-operator precedence and associativity.
//!
//! Covers every precedence band that participates in expression-shape rows,
//! including shift, relational, equality, compound assignment, ternary
//! conditional, and comma boundaries.  Each fixture asserts the exact root
//! operator and operand links expected from a full precedence-climbing parser.
//! GPU/CPU parity and PG lowering preservation are required for all fixtures.

#![cfg(feature = "c-parser")]
#![allow(deprecated)]

use vyre_libs::parsing::c::lex::tokens::*;
use vyre_libs::parsing::c::parse::vast::{
    reference_c11_build_expression_shape_nodes, reference_c11_build_vast_nodes,
    reference_c11_classify_vast_node_kinds, C_AST_KIND_ASSIGN_EXPR, C_AST_KIND_CONDITIONAL_EXPR,
    C_EXPR_ASSOC_LEFT, C_EXPR_ASSOC_NONE, C_EXPR_ASSOC_RIGHT, C_EXPR_SHAPE_BINARY,
    C_EXPR_SHAPE_CONDITIONAL, C_EXPR_SHAPE_NONE,
};
use vyre_primitives::predicate::node_kind;

mod c_ast_expression_support;
mod c_ast_gpu_parity_support;
use c_ast_expression_support::{
    assert_pg_links_match_vast, assert_pg_preserves_row, assert_shape_row, bytes, row_indices,
    run_pipeline, run_reference_pg_lower, starts_for_lens, unit_lens_fixture, SENTINEL,
    VAST_STRIDE_U32,
};
use c_ast_gpu_parity_support::{run_gpu_expr_shape, run_gpu_pg_lower};

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

fn shift_precedence_fixture() -> (Vec<u32>, Vec<u32>) {
    // a << b + c;
    let tok_types = vec![
        TOK_IDENTIFIER,
        TOK_LSHIFT,
        TOK_IDENTIFIER,
        TOK_PLUS,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
    ];
    unit_lens_fixture(tok_types)
}

fn relational_precedence_fixture() -> (Vec<u32>, Vec<u32>) {
    // a < b << c;
    let tok_types = vec![
        TOK_IDENTIFIER,
        TOK_LT,
        TOK_IDENTIFIER,
        TOK_LSHIFT,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
    ];
    unit_lens_fixture(tok_types)
}

fn equality_precedence_fixture() -> (Vec<u32>, Vec<u32>) {
    // a == b < c;
    let tok_types = vec![
        TOK_IDENTIFIER,
        TOK_EQ,
        TOK_IDENTIFIER,
        TOK_LT,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
    ];
    unit_lens_fixture(tok_types)
}

fn equality_left_assoc_fixture() -> (Vec<u32>, Vec<u32>) {
    // a == b != c;
    let tok_types = vec![
        TOK_IDENTIFIER,
        TOK_EQ,
        TOK_IDENTIFIER,
        TOK_NE,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
    ];
    unit_lens_fixture(tok_types)
}

fn compound_assignment_fixture() -> (Vec<u32>, Vec<u32>) {
    // a += b -= c;
    let tok_types = vec![
        TOK_IDENTIFIER,
        TOK_PLUS_EQ,
        TOK_IDENTIFIER,
        TOK_MINUS_EQ,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
    ];
    unit_lens_fixture(tok_types)
}

fn ternary_looser_than_assignment_fixture() -> (Vec<u32>, Vec<u32>) {
    // a = b ? c : d;
    let tok_types = vec![
        TOK_IDENTIFIER,
        TOK_ASSIGN,
        TOK_IDENTIFIER,
        TOK_QUESTION,
        TOK_IDENTIFIER,
        TOK_COLON,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
    ];
    unit_lens_fixture(tok_types)
}

fn ternary_right_assoc_fixture() -> (Vec<u32>, Vec<u32>) {
    // a ? b : c ? d : e;
    let tok_types = vec![
        TOK_IDENTIFIER,
        TOK_QUESTION,
        TOK_IDENTIFIER,
        TOK_COLON,
        TOK_IDENTIFIER,
        TOK_QUESTION,
        TOK_IDENTIFIER,
        TOK_COLON,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
    ];
    unit_lens_fixture(tok_types)
}

fn comma_boundary_fixture() -> (Vec<u32>, Vec<u32>) {
    // a = b, c = d;
    let tok_types = vec![
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

fn full_precedence_ladder_fixture() -> (Vec<u32>, Vec<u32>) {
    // a || b && c | d ^ e & f == g < h + i << j * k;
    let tok_types = vec![
        TOK_IDENTIFIER, // 0  a
        TOK_OR,         // 1  ||
        TOK_IDENTIFIER, // 2  b
        TOK_AND,        // 3  &&
        TOK_IDENTIFIER, // 4  c
        TOK_PIPE,       // 5  |
        TOK_IDENTIFIER, // 6  d
        TOK_CARET,      // 7  ^
        TOK_IDENTIFIER, // 8  e
        TOK_AMP,        // 9  &
        TOK_IDENTIFIER, // 10 f
        TOK_EQ,         // 11 ==
        TOK_IDENTIFIER, // 12 g
        TOK_LT,         // 13 <
        TOK_IDENTIFIER, // 14 h
        TOK_PLUS,       // 15 +
        TOK_IDENTIFIER, // 16 i
        TOK_LSHIFT,     // 17 <<
        TOK_IDENTIFIER, // 18 j
        TOK_STAR,       // 19 *
        TOK_IDENTIFIER, // 20 k
        TOK_SEMICOLON,  // 21 ;
    ];
    unit_lens_fixture(tok_types)
}

// ---------------------------------------------------------------------------
// Precedence tests
// ---------------------------------------------------------------------------

mod c_ast_expression_operator_precedence_contracts_part1 {

    include!("__split/c_ast_expression_operator_precedence_contracts_part1.rs");
}
mod c_ast_expression_operator_precedence_contracts_part2 {
    include!("__split/c_ast_expression_operator_precedence_contracts_part2.rs");
}
