// Integration tests for C expression ambiguity contracts:
//   - member access (`a.b`) and pointer-member access (`a->b`)
//   - cast vs parenthesized expression (`(int)*p` vs `(x)*y`)
//   - nested conditional and comma expressions
//   - compound literals in array contexts
//   - sizeof / _Alignof type-name followed by `*` ambiguity
//
// Every fixture asserts semantic VAST/AST invariants: kind classification,
// parent/child tree links, span preservation, and PG lowering preservation.
// GPU/CPU parity is asserted for the full pipeline.

// cfg(feature = "c-parser")  -  moved to parent

#[path = "../c_ast_gpu_parity_support/mod.rs"]
mod c_ast_gpu_parity_support;

use c_ast_gpu_parity_support::{
    assert_full_pipeline_parity, build_fixture, row_indices, run_gpu_pg_lower, word_at, Fixture,
    FixtureToken, VAST_STRIDE_U32,
};
use vyre_libs::parsing::c::lex::tokens::*;
use vyre_libs::parsing::c::lower::reference_ast_to_pg_nodes;
use vyre_libs::parsing::c::parse::vast::{
    reference_c11_annotate_typedef_names, reference_c11_build_expression_shape_nodes,
    reference_c11_build_vast_nodes, reference_c11_classify_vast_node_kinds, C_AST_KIND_CAST_EXPR,
    C_AST_KIND_COMPOUND_LITERAL_EXPR, C_AST_KIND_CONDITIONAL_EXPR, C_AST_KIND_MEMBER_ACCESS_EXPR,
    C_EXPR_SHAPE_NONE, C_EXPR_SHAPE_STRIDE_U32,
};
use vyre_primitives::predicate::node_kind;

const PG_STRIDE_U32: usize = 6;
const SENTINEL: u32 = u32::MAX;

fn classify(fix: &Fixture) -> Vec<u8> {
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    reference_c11_classify_vast_node_kinds(&annotated)
}

fn pg_word_at(buf: &[u8], idx: usize, field: usize) -> u32 {
    word_at(buf, idx * PG_STRIDE_U32 + field)
}

fn assert_pg_preserves_row(
    typed_vast: &[u8],
    pg: &[u8],
    fix: &Fixture,
    idx: usize,
    expected_kind: u32,
) {
    assert_eq!(
        pg_word_at(pg, idx, 0),
        expected_kind,
        "PG kind mismatch at row {idx}"
    );
    assert_eq!(
        pg_word_at(pg, idx, 1),
        fix.tok_starts[idx],
        "PG span_start mismatch at row {idx}"
    );
    assert_eq!(
        pg_word_at(pg, idx, 2),
        fix.tok_starts[idx] + fix.tok_lens[idx],
        "PG span_end mismatch at row {idx}"
    );
    assert_eq!(
        pg_word_at(pg, idx, 3),
        word_at(typed_vast, idx * VAST_STRIDE_U32 + 1),
        "PG parent mismatch at row {idx}"
    );
    assert_eq!(
        pg_word_at(pg, idx, 4),
        word_at(typed_vast, idx * VAST_STRIDE_U32 + 2),
        "PG first_child mismatch at row {idx}"
    );
    assert_eq!(
        pg_word_at(pg, idx, 5),
        word_at(typed_vast, idx * VAST_STRIDE_U32 + 3),
        "PG next_sibling mismatch at row {idx}"
    );
}

fn assert_shape_none(rows: &[u8], idx: usize, raw_operator: u32) {
    let row = idx * C_EXPR_SHAPE_STRIDE_U32 as usize;
    assert_eq!(word_at(rows, row), C_EXPR_SHAPE_NONE, "shape_kind[{idx}]");
    assert_eq!(word_at(rows, row + 1), SENTINEL, "source_idx[{idx}]");
    assert_eq!(word_at(rows, row + 2), raw_operator, "raw_operator[{idx}]");
    assert_eq!(word_at(rows, row + 3), 0, "precedence[{idx}]");
    assert_eq!(word_at(rows, row + 4), 0, "associativity[{idx}]");
    assert_eq!(word_at(rows, row + 5), SENTINEL, "first[{idx}]");
    assert_eq!(word_at(rows, row + 6), SENTINEL, "second[{idx}]");
    assert_eq!(word_at(rows, row + 7), SENTINEL, "third[{idx}]");
}

// ---------------------------------------------------------------------------
// Fixtures – member / pointer-member access
// ---------------------------------------------------------------------------

/// void f() { s.field; }
fn fixture_member_access_simple() -> Fixture {
    build_fixture(&[
        FixtureToken::new("void", TOK_VOID),
        FixtureToken::new("f", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("s", TOK_IDENTIFIER),
        FixtureToken::new(".", TOK_DOT),
        FixtureToken::new("field", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
    ])
}

/// void f() { p->field; }
fn fixture_ptr_member_access_simple() -> Fixture {
    build_fixture(&[
        FixtureToken::new("void", TOK_VOID),
        FixtureToken::new("f", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("p", TOK_IDENTIFIER),
        FixtureToken::new("->", TOK_ARROW),
        FixtureToken::new("field", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
    ])
}

/// void f() { a.b->c.d; }
fn fixture_chained_member_access() -> Fixture {
    build_fixture(&[
        FixtureToken::new("void", TOK_VOID),
        FixtureToken::new("f", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("a", TOK_IDENTIFIER),
        FixtureToken::new(".", TOK_DOT),
        FixtureToken::new("b", TOK_IDENTIFIER),
        FixtureToken::new("->", TOK_ARROW),
        FixtureToken::new("c", TOK_IDENTIFIER),
        FixtureToken::new(".", TOK_DOT),
        FixtureToken::new("d", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
    ])
}

// ---------------------------------------------------------------------------
// Fixtures – cast vs parenthesized expression
// ---------------------------------------------------------------------------

/// void f() { (int)*p; }
fn fixture_cast_then_deref() -> Fixture {
    build_fixture(&[
        FixtureToken::new("void", TOK_VOID),
        FixtureToken::new("f", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("int", TOK_INT),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("*", TOK_STAR),
        FixtureToken::new("p", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
    ])
}

/// void f() { (x)*y; }
fn fixture_paren_expr_then_mul() -> Fixture {
    build_fixture(&[
        FixtureToken::new("void", TOK_VOID),
        FixtureToken::new("f", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("x", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("*", TOK_STAR),
        FixtureToken::new("y", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
    ])
}

/// void f() { (int)(1); }
fn fixture_cast_not_compound_literal() -> Fixture {
    build_fixture(&[
        FixtureToken::new("void", TOK_VOID),
        FixtureToken::new("f", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("int", TOK_INT),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("1", TOK_INTEGER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
    ])
}

/// void f() { (x)(1); }
fn fixture_paren_expr_then_call_like() -> Fixture {
    build_fixture(&[
        FixtureToken::new("void", TOK_VOID),
        FixtureToken::new("f", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("x", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("1", TOK_INTEGER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
    ])
}

// ---------------------------------------------------------------------------
// Fixtures – nested conditional / comma
// ---------------------------------------------------------------------------

/// void f() { a ? b ? c : d : e, f; }
fn fixture_nested_conditional_comma() -> Fixture {
    build_fixture(&[
        FixtureToken::new("void", TOK_VOID),
        FixtureToken::new("f", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("a", TOK_IDENTIFIER),
        FixtureToken::new("?", TOK_QUESTION),
        FixtureToken::new("b", TOK_IDENTIFIER),
        FixtureToken::new("?", TOK_QUESTION),
        FixtureToken::new("c", TOK_IDENTIFIER),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new("d", TOK_IDENTIFIER),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new("e", TOK_IDENTIFIER),
        FixtureToken::new(",", TOK_COMMA),
        FixtureToken::new("f", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
    ])
}

// ---------------------------------------------------------------------------
// Fixtures – compound literal (array)
// ---------------------------------------------------------------------------

/// void f() { int *p = (int[]){1, 2, 3}; }
fn fixture_array_compound_literal() -> Fixture {
    build_fixture(&[
        FixtureToken::new("void", TOK_VOID),
        FixtureToken::new("f", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("int", TOK_INT),
        FixtureToken::new("*", TOK_STAR),
        FixtureToken::new("p", TOK_IDENTIFIER),
        FixtureToken::new("=", TOK_ASSIGN),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("int", TOK_INT),
        FixtureToken::new("[", TOK_LBRACKET),
        FixtureToken::new("]", TOK_RBRACKET),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("1", TOK_INTEGER),
        FixtureToken::new(",", TOK_COMMA),
        FixtureToken::new("2", TOK_INTEGER),
        FixtureToken::new(",", TOK_COMMA),
        FixtureToken::new("3", TOK_INTEGER),
        FixtureToken::new("}", TOK_RBRACE),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
    ])
}

// ---------------------------------------------------------------------------
// Fixtures – sizeof / _Alignof ambiguity
// ---------------------------------------------------------------------------

/// void f() { sizeof(int) * p; }
fn fixture_sizeof_typename_then_star() -> Fixture {
    build_fixture(&[
        FixtureToken::new("void", TOK_VOID),
        FixtureToken::new("f", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("sizeof", TOK_SIZEOF),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("int", TOK_INT),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("*", TOK_STAR),
        FixtureToken::new("p", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
    ])
}

/// void f() { _Alignof(int) * p; }
fn fixture_alignof_typename_then_star() -> Fixture {
    build_fixture(&[
        FixtureToken::new("void", TOK_VOID),
        FixtureToken::new("f", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("_Alignof", TOK_ALIGNOF),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("int", TOK_INT),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("*", TOK_STAR),
        FixtureToken::new("p", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
    ])
}

// ---------------------------------------------------------------------------
// Tests – member / pointer-member access
// ---------------------------------------------------------------------------

#[test]
fn member_access_simple_classifies() {
    let fix = fixture_member_access_simple();
    assert_full_pipeline_parity(&fix, "member_access_simple");

    let typed = classify(&fix);
    assert_eq!(
        row_indices(&typed, C_AST_KIND_MEMBER_ACCESS_EXPR),
        vec![6],
        "dot must classify as MEMBER_ACCESS_EXPR"
    );
}

#[test]
fn ptr_member_access_simple_classifies() {
    let fix = fixture_ptr_member_access_simple();
    assert_full_pipeline_parity(&fix, "ptr_member_access_simple");

    let typed = classify(&fix);
    assert_eq!(
        row_indices(&typed, C_AST_KIND_MEMBER_ACCESS_EXPR),
        vec![6],
        "arrow must classify as MEMBER_ACCESS_EXPR"
    );
}

#[test]
fn chained_member_access_classifies_all_operators() {
    let fix = fixture_chained_member_access();
    assert_full_pipeline_parity(&fix, "chained_member_access");

    let typed = classify(&fix);
    let members = row_indices(&typed, C_AST_KIND_MEMBER_ACCESS_EXPR);
    assert_eq!(
        members,
        vec![6, 8, 10],
        "all three `.` and `->` must classify as MEMBER_ACCESS_EXPR"
    );
}

// ---------------------------------------------------------------------------
// Tests – cast vs parenthesized expression
// ---------------------------------------------------------------------------

#[test]
fn cast_then_deref_is_cast_not_binary() {
    let fix = fixture_cast_then_deref();
    assert_full_pipeline_parity(&fix, "cast_then_deref");

    let typed = classify(&fix);
    assert_eq!(
        row_indices(&typed, C_AST_KIND_CAST_EXPR),
        vec![5],
        "`(int)` must classify as CAST_EXPR"
    );
    // The `*` after a cast is a dereference (unary), not a binary multiply.
    // It should NOT be classified as BINARY.
    assert!(
        !row_indices(&typed, node_kind::BINARY).contains(&8),
        "`*` after cast must not be BINARY"
    );
}
