//! Integration tests for GNU/C11 builtin forms not covered by other suites:
//!   - `__builtin_offsetof(type, member)`
//!   - `__builtin_object_size(ptr, type)`
//!   - `__builtin_prefetch(addr, rw, locality)`
//!   - `__builtin_unreachable()`
//!
//! Every test asserts distinct VAST kinds, no collapse into CALL/BINARY,
//! PG lowering preservation, and GPU/CPU parity.

#![cfg(feature = "c-parser")]
#![allow(deprecated)]
mod c_ast_gpu_parity_support;

use c_ast_gpu_parity_support::{
    assert_full_pipeline_parity, build_fixture, row_indices, run_gpu_pg_lower, word_at, Fixture,
    FixtureToken, VAST_STRIDE_U32,
};
use vyre_libs::parsing::c::lex::tokens::*;
use vyre_libs::parsing::c::lower::reference_ast_to_pg_nodes;
use vyre_libs::parsing::c::parse::vast::{
    reference_c11_annotate_typedef_names, reference_c11_build_vast_nodes,
    reference_c11_classify_vast_node_kinds, C_AST_KIND_BUILTIN_OBJECT_SIZE_EXPR,
    C_AST_KIND_BUILTIN_OFFSETOF_EXPR, C_AST_KIND_BUILTIN_PREFETCH_EXPR,
    C_AST_KIND_BUILTIN_UNREACHABLE_STMT,
};
use vyre_primitives::predicate::node_kind;

const PG_STRIDE_U32: usize = 6;

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

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// void f() { __builtin_offsetof(struct S, field); }
fn fixture_builtin_offsetof() -> Fixture {
    build_fixture(&[
        FixtureToken::new("void", TOK_VOID),
        FixtureToken::new("f", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("__builtin_offsetof", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("struct", TOK_STRUCT),
        FixtureToken::new("S", TOK_IDENTIFIER),
        FixtureToken::new(",", TOK_COMMA),
        FixtureToken::new("field", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
    ])
}

/// void f() { __builtin_object_size(ptr, 0); }
fn fixture_builtin_object_size() -> Fixture {
    build_fixture(&[
        FixtureToken::new("void", TOK_VOID),
        FixtureToken::new("f", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("__builtin_object_size", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("ptr", TOK_IDENTIFIER),
        FixtureToken::new(",", TOK_COMMA),
        FixtureToken::new("0", TOK_INTEGER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
    ])
}

/// void f() { __builtin_prefetch(addr, 0, 3); }
fn fixture_builtin_prefetch() -> Fixture {
    build_fixture(&[
        FixtureToken::new("void", TOK_VOID),
        FixtureToken::new("f", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("__builtin_prefetch", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("addr", TOK_IDENTIFIER),
        FixtureToken::new(",", TOK_COMMA),
        FixtureToken::new("0", TOK_INTEGER),
        FixtureToken::new(",", TOK_COMMA),
        FixtureToken::new("3", TOK_INTEGER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
    ])
}

/// void f() { __builtin_unreachable(); }
fn fixture_builtin_unreachable() -> Fixture {
    build_fixture(&[
        FixtureToken::new("void", TOK_VOID),
        FixtureToken::new("f", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("__builtin_unreachable", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
    ])
}

// ---------------------------------------------------------------------------
// Tests – classification
// ---------------------------------------------------------------------------

#[test]
fn builtin_offsetof_classifies_as_distinct_expr() {
    let fix = fixture_builtin_offsetof();
    assert_full_pipeline_parity(&fix, "builtin_offsetof");

    let typed = classify(&fix);
    assert_eq!(
        row_indices(&typed, C_AST_KIND_BUILTIN_OFFSETOF_EXPR),
        vec![5],
        "__builtin_offsetof must classify as BUILTIN_OFFSETOF_EXPR"
    );
    assert_ne!(
        word_at(&typed, 5 * VAST_STRIDE_U32),
        node_kind::CALL,
        "__builtin_offsetof must not collapse into CALL"
    );
}

#[test]
fn builtin_object_size_classifies_as_distinct_expr() {
    let fix = fixture_builtin_object_size();
    assert_full_pipeline_parity(&fix, "builtin_object_size");

    let typed = classify(&fix);
    assert_eq!(
        row_indices(&typed, C_AST_KIND_BUILTIN_OBJECT_SIZE_EXPR),
        vec![5],
        "__builtin_object_size must classify as BUILTIN_OBJECT_SIZE_EXPR"
    );
    assert_ne!(
        word_at(&typed, 5 * VAST_STRIDE_U32),
        node_kind::CALL,
        "__builtin_object_size must not collapse into CALL"
    );
}

#[test]
fn builtin_prefetch_classifies_as_distinct_expr() {
    let fix = fixture_builtin_prefetch();
    assert_full_pipeline_parity(&fix, "builtin_prefetch");

    let typed = classify(&fix);
    assert_eq!(
        row_indices(&typed, C_AST_KIND_BUILTIN_PREFETCH_EXPR),
        vec![5],
        "__builtin_prefetch must classify as BUILTIN_PREFETCH_EXPR"
    );
    assert_ne!(
        word_at(&typed, 5 * VAST_STRIDE_U32),
        node_kind::CALL,
        "__builtin_prefetch must not collapse into CALL"
    );
}

#[test]
fn builtin_unreachable_classifies_as_distinct_stmt() {
    let fix = fixture_builtin_unreachable();
    assert_full_pipeline_parity(&fix, "builtin_unreachable");

    let typed = classify(&fix);
    assert_eq!(
        row_indices(&typed, C_AST_KIND_BUILTIN_UNREACHABLE_STMT),
        vec![5],
        "__builtin_unreachable must classify as BUILTIN_UNREACHABLE_STMT"
    );
    assert_ne!(
        word_at(&typed, 5 * VAST_STRIDE_U32),
        node_kind::CALL,
        "__builtin_unreachable must not collapse into CALL"
    );
}

// ---------------------------------------------------------------------------
// Tests – PG lowering preservation
// ---------------------------------------------------------------------------

#[test]
fn pg_lower_preserves_builtin_offsetof() {
    let fix = fixture_builtin_offsetof();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let pg = reference_ast_to_pg_nodes(&typed);

    assert_pg_preserves_row(&typed, &pg, &fix, 5, C_AST_KIND_BUILTIN_OFFSETOF_EXPR);
}

#[test]
fn pg_lower_preserves_builtin_object_size() {
    let fix = fixture_builtin_object_size();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let pg = reference_ast_to_pg_nodes(&typed);

    assert_pg_preserves_row(&typed, &pg, &fix, 5, C_AST_KIND_BUILTIN_OBJECT_SIZE_EXPR);
}

#[test]
fn pg_lower_preserves_builtin_prefetch() {
    let fix = fixture_builtin_prefetch();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let pg = reference_ast_to_pg_nodes(&typed);

    assert_pg_preserves_row(&typed, &pg, &fix, 5, C_AST_KIND_BUILTIN_PREFETCH_EXPR);
}

#[test]
fn pg_lower_preserves_builtin_unreachable() {
    let fix = fixture_builtin_unreachable();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let pg = reference_ast_to_pg_nodes(&typed);

    assert_pg_preserves_row(&typed, &pg, &fix, 5, C_AST_KIND_BUILTIN_UNREACHABLE_STMT);
}

// ---------------------------------------------------------------------------
// Tests – GPU PG lowering parity
// ---------------------------------------------------------------------------

#[test]
fn gpu_pg_lower_matches_cpu_for_remaining_builtin_fixtures() {
    let fixtures: Vec<(&str, Fixture)> = vec![
        ("builtin_offsetof", fixture_builtin_offsetof()),
        ("builtin_object_size", fixture_builtin_object_size()),
        ("builtin_prefetch", fixture_builtin_prefetch()),
        ("builtin_unreachable", fixture_builtin_unreachable()),
    ];

    for (label, fix) in fixtures {
        let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
        let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
        let typed = reference_c11_classify_vast_node_kinds(&annotated);
        let expected = reference_ast_to_pg_nodes(&typed);
        let gpu = run_gpu_pg_lower(&typed);
        assert_eq!(
            gpu, expected,
            "GPU PG lowerer must match CPU for fixture `{label}`"
        );
    }
}
