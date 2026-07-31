//! Hostile Linux/kernel-grade C AST contracts for control flow and PG parity.
//!
//! Constructs under test:
//!   * dense switch/case/default with fallthrough attributes
//!   * statement expressions inside if/while conditions
//!   * nested statement expressions
//!   * computed goto combined with statement expressions
//!   * empty switch body (edge case  -  must not panic)
//!   * forward goto inside a statement expression
//!
//! Every fixture asserts full GPU/CPU parity for VAST build, annotate, classify,
//! AND PG lowerer.  A missing GPU adapter is a configuration failure.

#![cfg(feature = "c-parser")]
#![allow(deprecated)]
mod c_ast_gpu_parity_support;

use c_ast_gpu_parity_support::{
    assert_full_pipeline_parity, assert_gpu_pg_parity, assert_pg_preserves_row, build_fixture,
    c_fixture, kind_at, row_indices, Fixture, FixtureToken,
};
use vyre_libs::parsing::c::lex::tokens::*;
use vyre_libs::parsing::c::lower::reference_ast_to_pg_nodes;
use vyre_libs::parsing::c::parse::vast::{
    reference_c11_annotate_typedef_names, reference_c11_build_vast_nodes,
    reference_c11_classify_vast_node_kinds, C_AST_KIND_ASSIGN_EXPR,
    C_AST_KIND_ATTRIBUTE_FALLTHROUGH, C_AST_KIND_BREAK_STMT, C_AST_KIND_CASE_STMT,
    C_AST_KIND_DEFAULT_STMT, C_AST_KIND_GNU_ATTRIBUTE, C_AST_KIND_GOTO_STMT, C_AST_KIND_IF_STMT,
    C_AST_KIND_LABEL_STMT, C_AST_KIND_RETURN_STMT, C_AST_KIND_SWITCH_STMT, C_AST_KIND_WHILE_STMT,
};
use vyre_primitives::predicate::node_kind;

fn lexeme_indices(fix: &Fixture, lexeme: &str) -> Vec<usize> {
    fix.tok_starts
        .iter()
        .zip(&fix.tok_lens)
        .enumerate()
        .filter_map(|(idx, (start, len))| {
            let s = *start as usize;
            let e = s.saturating_add(*len as usize);
            (fix.source.as_bytes().get(s..e) == Some(lexeme.as_bytes())).then_some(idx)
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// ```c
/// void f(int v) {
///     int a;
///     switch (v) {
///     case 0: a = 0; __attribute__((fallthrough));
///     case 1: a = 1; break;
///     default: break;
///     }
/// }
/// ```
fn fixture_dense_switch_with_fallthrough() -> Fixture {
    c_fixture![
        ("void", TOK_IDENTIFIER),
        ("f", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("int", TOK_IDENTIFIER),
        ("v", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        ("{", TOK_LBRACE),
        ("int", TOK_IDENTIFIER),
        ("a", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
        ("switch", TOK_SWITCH),
        ("(", TOK_LPAREN),
        ("v", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        ("{", TOK_LBRACE),
        ("case", TOK_CASE),
        ("0", TOK_INTEGER),
        (":", TOK_COLON),
        ("a", TOK_IDENTIFIER),
        ("=", TOK_ASSIGN),
        ("0", TOK_INTEGER),
        (";", TOK_SEMICOLON),
        ("__attribute__", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("(", TOK_LPAREN),
        ("fallthrough", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        (")", TOK_RPAREN),
        (";", TOK_SEMICOLON),
        ("case", TOK_CASE),
        ("1", TOK_INTEGER),
        (":", TOK_COLON),
        ("a", TOK_IDENTIFIER),
        ("=", TOK_ASSIGN),
        ("1", TOK_INTEGER),
        (";", TOK_SEMICOLON),
        ("break", TOK_BREAK),
        (";", TOK_SEMICOLON),
        ("default", TOK_DEFAULT),
        (":", TOK_COLON),
        ("break", TOK_BREAK),
        (";", TOK_SEMICOLON),
        ("}", TOK_RBRACE),
        ("}", TOK_RBRACE),
    ]
}

/// ```c
/// void g(void) {
///     if (({ int r = check(); r; })) { ok(); } else { fail(); }
/// }
/// ```
fn fixture_stmt_expr_in_if_condition() -> Fixture {
    c_fixture![
        ("void", TOK_IDENTIFIER),
        ("g", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("void", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        ("{", TOK_LBRACE),
        ("if", TOK_IF),
        ("(", TOK_LPAREN),
        ("(", TOK_LPAREN),
        ("{", TOK_LBRACE),
        ("int", TOK_IDENTIFIER),
        ("r", TOK_IDENTIFIER),
        ("=", TOK_ASSIGN),
        ("check", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        (")", TOK_RPAREN),
        (";", TOK_SEMICOLON),
        ("r", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
        ("}", TOK_RBRACE),
        (")", TOK_RPAREN),
        (")", TOK_RPAREN),
        ("{", TOK_LBRACE),
        ("ok", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        (")", TOK_RPAREN),
        (";", TOK_SEMICOLON),
        ("}", TOK_RBRACE),
        ("else", TOK_ELSE),
        ("{", TOK_LBRACE),
        ("fail", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        (")", TOK_RPAREN),
        (";", TOK_SEMICOLON),
        ("}", TOK_RBRACE),
        ("}", TOK_RBRACE),
    ]
}

/// ```c
/// void h(void) {
///     while (({ int r = poll(); r >= 0; })) { body(); }
/// }
/// ```
fn fixture_stmt_expr_in_while_condition() -> Fixture {
    c_fixture![
        ("void", TOK_IDENTIFIER),
        ("h", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("void", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        ("{", TOK_LBRACE),
        ("while", TOK_WHILE),
        ("(", TOK_LPAREN),
        ("(", TOK_LPAREN),
        ("{", TOK_LBRACE),
        ("int", TOK_IDENTIFIER),
        ("r", TOK_IDENTIFIER),
        ("=", TOK_ASSIGN),
        ("poll", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        (")", TOK_RPAREN),
        (";", TOK_SEMICOLON),
        ("r", TOK_IDENTIFIER),
        (">=", TOK_GE),
        ("0", TOK_INTEGER),
        (";", TOK_SEMICOLON),
        ("}", TOK_RBRACE),
        (")", TOK_RPAREN),
        (")", TOK_RPAREN),
        ("{", TOK_LBRACE),
        ("body", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        (")", TOK_RPAREN),
        (";", TOK_SEMICOLON),
        ("}", TOK_RBRACE),
        ("}", TOK_RBRACE),
    ]
}

/// ```c
/// int x = ({ int y = ({ int z = 1; z; }); y; });
/// ```
fn fixture_nested_stmt_expr() -> Fixture {
    c_fixture![
        ("int", TOK_IDENTIFIER),
        ("x", TOK_IDENTIFIER),
        ("=", TOK_ASSIGN),
        ("(", TOK_LPAREN),
        ("{", TOK_LBRACE),
        ("int", TOK_IDENTIFIER),
        ("y", TOK_IDENTIFIER),
        ("=", TOK_ASSIGN),
        ("(", TOK_LPAREN),
        ("{", TOK_LBRACE),
        ("int", TOK_IDENTIFIER),
        ("z", TOK_IDENTIFIER),
        ("=", TOK_ASSIGN),
        ("1", TOK_INTEGER),
        (";", TOK_SEMICOLON),
        ("z", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
        ("}", TOK_RBRACE),
        (")", TOK_RPAREN),
        (";", TOK_SEMICOLON),
        ("y", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
        ("}", TOK_RBRACE),
        (")", TOK_RPAREN),
        (";", TOK_SEMICOLON),
    ]
}

/// ```c
/// void *t = ({ void *p = &&label; p; });
/// goto *t;
/// label: return;
/// ```
fn fixture_computed_goto_stmt_expr() -> Fixture {
    c_fixture![
        ("void", TOK_IDENTIFIER),
        ("*", TOK_STAR),
        ("t", TOK_IDENTIFIER),
        ("=", TOK_ASSIGN),
        ("(", TOK_LPAREN),
        ("{", TOK_LBRACE),
        ("void", TOK_IDENTIFIER),
        ("*", TOK_STAR),
        ("p", TOK_IDENTIFIER),
        ("=", TOK_ASSIGN),
        ("&&", TOK_AND),
        ("label", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
        ("p", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
        ("}", TOK_RBRACE),
        (")", TOK_RPAREN),
        (";", TOK_SEMICOLON),
        ("goto", TOK_GOTO),
        ("*", TOK_STAR),
        ("t", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
        ("label", TOK_IDENTIFIER),
        (":", TOK_COLON),
        ("return", TOK_RETURN),
        (";", TOK_SEMICOLON),
    ]
}

/// ```c
/// void k(int x) { switch (x) {} }
/// ```
fn fixture_empty_switch() -> Fixture {
    c_fixture![
        ("void", TOK_IDENTIFIER),
        ("k", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("int", TOK_IDENTIFIER),
        ("x", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        ("{", TOK_LBRACE),
        ("switch", TOK_SWITCH),
        ("(", TOK_LPAREN),
        ("x", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        ("{", TOK_LBRACE),
        ("}", TOK_RBRACE),
        ("}", TOK_RBRACE),
    ]
}

/// ```c
/// int w = ({ goto inner; inner: 42; });
/// ```
fn fixture_goto_inside_stmt_expr() -> Fixture {
    c_fixture![
        ("int", TOK_IDENTIFIER),
        ("w", TOK_IDENTIFIER),
        ("=", TOK_ASSIGN),
        ("(", TOK_LPAREN),
        ("{", TOK_LBRACE),
        ("goto", TOK_GOTO),
        ("inner", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
        ("inner", TOK_IDENTIFIER),
        (":", TOK_COLON),
        ("42", TOK_INTEGER),
        (";", TOK_SEMICOLON),
        ("}", TOK_RBRACE),
        (")", TOK_RPAREN),
        (";", TOK_SEMICOLON),
    ]
}

// ---------------------------------------------------------------------------
// Tests  -  dense switch with fallthrough
// ---------------------------------------------------------------------------

mod c_ast_linux_corpus_hostile_flow_and_pg_parity_contracts_part1 {

    include!("__split/c_ast_linux_corpus_hostile_flow_and_pg_parity_contracts_part1.rs");
}
mod c_ast_linux_corpus_hostile_flow_and_pg_parity_contracts_part2 {
    include!("__split/c_ast_linux_corpus_hostile_flow_and_pg_parity_contracts_part2.rs");
}
