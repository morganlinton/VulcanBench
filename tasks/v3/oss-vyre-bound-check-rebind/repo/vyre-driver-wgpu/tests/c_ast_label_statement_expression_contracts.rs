//! Integration tests for Linux-grade C AST parser gaps around labels,
//! goto target labels, nested labels in loops/switch/if bodies, GNU
//! statement expressions, statement-expression initializer contexts,
//! and CPU/GPU parity.
//!
//! Constructs under test:
//!   - multiple consecutive labels on the same statement
//!   - labels inside if/else, switch/case, for/while/do bodies
//!   - forward/backward goto across nested block boundaries
//!   - GNU statement expressions in assignment and initializer contexts
//!   - nested GNU statement expressions
//!   - labels and goto inside GNU statement expressions
//!
//! A missing GPU adapter is a configuration failure; tests do not skip.

#![cfg(feature = "c-parser")]
#![allow(deprecated)]
mod c_ast_gpu_parity_support;

use c_ast_gpu_parity_support::{
    assert_full_pipeline_parity, assert_pg_preserves_row, c_fixture, classify,
    node_count_from_vast, row_indices, run_gpu_pg_lower_with_count as run_gpu_pg_lower, word_at,
    Fixture, VAST_STRIDE_U32,
};
use vyre_libs::parsing::c::lex::tokens::*;
use vyre_libs::parsing::c::lower::reference_ast_to_pg_nodes;
use vyre_libs::parsing::c::parse::vast::{
    reference_c11_annotate_typedef_names, reference_c11_build_vast_nodes,
    reference_c11_classify_vast_node_kinds, C_AST_KIND_ASSIGN_EXPR, C_AST_KIND_BREAK_STMT,
    C_AST_KIND_CASE_STMT, C_AST_KIND_DEFAULT_STMT, C_AST_KIND_DO_STMT, C_AST_KIND_FOR_STMT,
    C_AST_KIND_GNU_STATEMENT_EXPR, C_AST_KIND_GOTO_STMT, C_AST_KIND_IF_STMT,
    C_AST_KIND_INITIALIZER_LIST, C_AST_KIND_LABEL_STMT, C_AST_KIND_MEMBER_ACCESS_EXPR,
    C_AST_KIND_RETURN_STMT, C_AST_KIND_SWITCH_STMT, C_AST_KIND_WHILE_STMT,
};
use vyre_primitives::predicate::node_kind;

// ---------------------------------------------------------------------------
// Fixtures – labels and goto
// ---------------------------------------------------------------------------

/// void f() { a: b: c: return; }
fn fixture_multiple_consecutive_labels() -> Fixture {
    c_fixture![
        ("void", TOK_IDENTIFIER),
        ("f", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        (")", TOK_RPAREN),
        ("{", TOK_LBRACE),
        ("a", TOK_IDENTIFIER),
        (":", TOK_COLON),
        ("b", TOK_IDENTIFIER),
        (":", TOK_COLON),
        ("c", TOK_IDENTIFIER),
        (":", TOK_COLON),
        ("return", TOK_RETURN),
        (";", TOK_SEMICOLON),
        ("}", TOK_RBRACE),
    ]
}

/// void f(int x) {
///   if (x) { label1: return; }
///   else { label2: return; }
/// }
fn fixture_label_inside_if_else() -> Fixture {
    c_fixture![
        ("void", TOK_IDENTIFIER),
        ("f", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("int", TOK_IDENTIFIER),
        ("x", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        ("{", TOK_LBRACE),
        ("if", TOK_IF),
        ("(", TOK_LPAREN),
        ("x", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        ("{", TOK_LBRACE),
        ("label1", TOK_IDENTIFIER),
        (":", TOK_COLON),
        ("return", TOK_RETURN),
        (";", TOK_SEMICOLON),
        ("}", TOK_RBRACE),
        ("else", TOK_ELSE),
        ("{", TOK_LBRACE),
        ("label2", TOK_IDENTIFIER),
        (":", TOK_COLON),
        ("return", TOK_RETURN),
        (";", TOK_SEMICOLON),
        ("}", TOK_RBRACE),
        ("}", TOK_RBRACE),
    ]
}

/// void f(int x) {
///   switch (x) {
///     case 0: inner: return;
///     default: return;
///   }
/// }
fn fixture_label_inside_switch_case() -> Fixture {
    c_fixture![
        ("void", TOK_IDENTIFIER),
        ("f", TOK_IDENTIFIER),
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
        ("case", TOK_CASE),
        ("0", TOK_INTEGER),
        (":", TOK_COLON),
        ("inner", TOK_IDENTIFIER),
        (":", TOK_COLON),
        ("return", TOK_RETURN),
        (";", TOK_SEMICOLON),
        ("default", TOK_DEFAULT),
        (":", TOK_COLON),
        ("return", TOK_RETURN),
        (";", TOK_SEMICOLON),
        ("}", TOK_RBRACE),
        ("}", TOK_RBRACE),
    ]
}

/// void f() {
///   for (;;) { loop_for: break; }
///   while (1) { loop_while: break; }
///   do { loop_do: break; } while (0);
/// }
fn fixture_label_inside_loops() -> Fixture {
    c_fixture![
        ("void", TOK_IDENTIFIER),
        ("f", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        (")", TOK_RPAREN),
        ("{", TOK_LBRACE),
        ("for", TOK_FOR),
        ("(", TOK_LPAREN),
        (";", TOK_SEMICOLON),
        (";", TOK_SEMICOLON),
        (")", TOK_RPAREN),
        ("{", TOK_LBRACE),
        ("loop_for", TOK_IDENTIFIER),
        (":", TOK_COLON),
        ("break", TOK_BREAK),
        (";", TOK_SEMICOLON),
        ("}", TOK_RBRACE),
        ("while", TOK_WHILE),
        ("(", TOK_LPAREN),
        ("1", TOK_INTEGER),
        (")", TOK_RPAREN),
        ("{", TOK_LBRACE),
        ("loop_while", TOK_IDENTIFIER),
        (":", TOK_COLON),
        ("break", TOK_BREAK),
        (";", TOK_SEMICOLON),
        ("}", TOK_RBRACE),
        ("do", TOK_DO),
        ("{", TOK_LBRACE),
        ("loop_do", TOK_IDENTIFIER),
        (":", TOK_COLON),
        ("break", TOK_BREAK),
        (";", TOK_SEMICOLON),
        ("}", TOK_RBRACE),
        ("while", TOK_WHILE),
        ("(", TOK_LPAREN),
        ("0", TOK_INTEGER),
        (")", TOK_RPAREN),
        (";", TOK_SEMICOLON),
        ("}", TOK_RBRACE),
    ]
}

/// void f() {
///   goto target;
///   if (1) { target: return; }
/// }
fn fixture_forward_goto_into_nested_if() -> Fixture {
    c_fixture![
        ("void", TOK_IDENTIFIER),
        ("f", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        (")", TOK_RPAREN),
        ("{", TOK_LBRACE),
        ("goto", TOK_GOTO),
        ("target", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
        ("if", TOK_IF),
        ("(", TOK_LPAREN),
        ("1", TOK_INTEGER),
        (")", TOK_RPAREN),
        ("{", TOK_LBRACE),
        ("target", TOK_IDENTIFIER),
        (":", TOK_COLON),
        ("return", TOK_RETURN),
        (";", TOK_SEMICOLON),
        ("}", TOK_RBRACE),
        ("}", TOK_RBRACE),
    ]
}

/// void f() {
///   if (1) { goto outer; }
///   outer: return;
/// }
fn fixture_backward_goto_from_nested_block() -> Fixture {
    c_fixture![
        ("void", TOK_IDENTIFIER),
        ("f", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        (")", TOK_RPAREN),
        ("{", TOK_LBRACE),
        ("if", TOK_IF),
        ("(", TOK_LPAREN),
        ("1", TOK_INTEGER),
        (")", TOK_RPAREN),
        ("{", TOK_LBRACE),
        ("goto", TOK_GOTO),
        ("outer", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
        ("}", TOK_RBRACE),
        ("outer", TOK_IDENTIFIER),
        (":", TOK_COLON),
        ("return", TOK_RETURN),
        (";", TOK_SEMICOLON),
        ("}", TOK_RBRACE),
    ]
}

// ---------------------------------------------------------------------------
// Fixtures – statement expressions
// ---------------------------------------------------------------------------

/// int x = ({ int y = 1; y + 2; });
fn fixture_statement_expression_simple() -> Fixture {
    c_fixture![
        ("int", TOK_IDENTIFIER),
        ("x", TOK_IDENTIFIER),
        ("=", TOK_ASSIGN),
        ("(", TOK_LPAREN),
        ("{", TOK_LBRACE),
        ("int", TOK_IDENTIFIER),
        ("y", TOK_IDENTIFIER),
        ("=", TOK_ASSIGN),
        ("1", TOK_INTEGER),
        (";", TOK_SEMICOLON),
        ("y", TOK_IDENTIFIER),
        ("+", TOK_PLUS),
        ("2", TOK_INTEGER),
        (";", TOK_SEMICOLON),
        ("}", TOK_RBRACE),
        (")", TOK_RPAREN),
        (";", TOK_SEMICOLON),
    ]
}

/// int arr[2] = { ({ 1; }), ({ 2; }) };
fn fixture_statement_expression_in_array_init() -> Fixture {
    c_fixture![
        ("int", TOK_IDENTIFIER),
        ("arr", TOK_IDENTIFIER),
        ("[", TOK_LBRACKET),
        ("2", TOK_INTEGER),
        ("]", TOK_RBRACKET),
        ("=", TOK_ASSIGN),
        ("{", TOK_LBRACE),
        ("(", TOK_LPAREN),
        ("{", TOK_LBRACE),
        ("1", TOK_INTEGER),
        (";", TOK_SEMICOLON),
        ("}", TOK_RBRACE),
        (")", TOK_RPAREN),
        (",", TOK_COMMA),
        ("(", TOK_LPAREN),
        ("{", TOK_LBRACE),
        ("2", TOK_INTEGER),
        (";", TOK_SEMICOLON),
        ("}", TOK_RBRACE),
        (")", TOK_RPAREN),
        ("}", TOK_RBRACE),
        (";", TOK_SEMICOLON),
    ]
}

/// struct S { int a; };
/// struct S s = { .a = ({ int t = 1; t; }) };
fn fixture_statement_expression_in_struct_designated_init() -> Fixture {
    c_fixture![
        ("struct", TOK_IDENTIFIER),
        ("S", TOK_IDENTIFIER),
        ("{", TOK_LBRACE),
        ("int", TOK_IDENTIFIER),
        ("a", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
        ("}", TOK_RBRACE),
        (";", TOK_SEMICOLON),
        ("struct", TOK_IDENTIFIER),
        ("S", TOK_IDENTIFIER),
        ("s", TOK_IDENTIFIER),
        ("=", TOK_ASSIGN),
        ("{", TOK_LBRACE),
        (".", TOK_DOT),
        ("a", TOK_IDENTIFIER),
        ("=", TOK_ASSIGN),
        ("(", TOK_LPAREN),
        ("{", TOK_LBRACE),
        ("int", TOK_IDENTIFIER),
        ("t", TOK_IDENTIFIER),
        ("=", TOK_ASSIGN),
        ("1", TOK_INTEGER),
        (";", TOK_SEMICOLON),
        ("t", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
        ("}", TOK_RBRACE),
        (")", TOK_RPAREN),
        ("}", TOK_RBRACE),
        (";", TOK_SEMICOLON),
    ]
}

/// int x = ({ int y = ({ 1; }); y; });
fn fixture_nested_statement_expression() -> Fixture {
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
        ("1", TOK_INTEGER),
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

/// int x = ({ goto end; end: 42; });
fn fixture_statement_expression_with_label_and_goto() -> Fixture {
    c_fixture![
        ("int", TOK_IDENTIFIER),
        ("x", TOK_IDENTIFIER),
        ("=", TOK_ASSIGN),
        ("(", TOK_LPAREN),
        ("{", TOK_LBRACE),
        ("goto", TOK_GOTO),
        ("end", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
        ("end", TOK_IDENTIFIER),
        (":", TOK_COLON),
        ("42", TOK_INTEGER),
        (";", TOK_SEMICOLON),
        ("}", TOK_RBRACE),
        (")", TOK_RPAREN),
        (";", TOK_SEMICOLON),
    ]
}

// ---------------------------------------------------------------------------
// Tests – CPU reference contracts (labels)
// ---------------------------------------------------------------------------

mod c_ast_label_statement_expression_contracts_part1 {

    include!("__split/c_ast_label_statement_expression_contracts_part1.rs");
}
mod c_ast_label_statement_expression_contracts_part2 {
    include!("__split/c_ast_label_statement_expression_contracts_part2.rs");
}
