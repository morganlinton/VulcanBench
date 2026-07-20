// Integration tests for GNU computed goto (`&&label`) and C11 atomic type
// qualifiers (`_Atomic`), plus C99 for-loop declarations.
//
// Constructs under test:
//   - computed goto label-address expressions
//   - `_Atomic` as type specifier and type qualifier
//   - `for (int i = 0; …)` declaration-in-init
//   - GPU/CPU parity for VAST build, annotation, classify, and PG lower
//   - PG lowering preservation (kind, span, parent, first_child, next_sibling)
//
// A missing GPU adapter is a configuration failure; tests do not skip.

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
    reference_c11_annotate_typedef_names, reference_c11_build_vast_nodes,
    reference_c11_classify_vast_node_kinds, C_AST_KIND_ASSIGN_EXPR, C_AST_KIND_CAST_EXPR,
    C_AST_KIND_FOR_STMT, C_AST_KIND_GNU_LABEL_ADDRESS_EXPR, C_AST_KIND_GOTO_STMT,
    C_AST_KIND_LABEL_STMT,
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
// Fixtures – computed goto
// ---------------------------------------------------------------------------

/// void f() { void *p = &&label; label: return; }
fn fixture_computed_goto_simple() -> Fixture {
    build_fixture(&[
        FixtureToken::new("void", TOK_VOID),
        FixtureToken::new("f", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("void", TOK_VOID),
        FixtureToken::new("*", TOK_STAR),
        FixtureToken::new("p", TOK_IDENTIFIER),
        FixtureToken::new("=", TOK_ASSIGN),
        FixtureToken::new("&&", TOK_AND),
        FixtureToken::new("label", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("label", TOK_IDENTIFIER),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new("return", TOK_RETURN),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
    ])
}

/// void f() { goto *&&end; end: return; }
fn fixture_computed_goto_in_goto() -> Fixture {
    build_fixture(&[
        FixtureToken::new("void", TOK_VOID),
        FixtureToken::new("f", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("goto", TOK_GOTO),
        FixtureToken::new("*", TOK_STAR),
        FixtureToken::new("&&", TOK_AND),
        FixtureToken::new("end", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("end", TOK_IDENTIFIER),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new("return", TOK_RETURN),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
    ])
}

/// void f() { return &&a, &&b; a: b: ; }
fn fixture_computed_goto_comma() -> Fixture {
    build_fixture(&[
        FixtureToken::new("void", TOK_VOID),
        FixtureToken::new("f", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("return", TOK_RETURN),
        FixtureToken::new("&&", TOK_AND),
        FixtureToken::new("a", TOK_IDENTIFIER),
        FixtureToken::new(",", TOK_COMMA),
        FixtureToken::new("&&", TOK_AND),
        FixtureToken::new("b", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("a", TOK_IDENTIFIER),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new("b", TOK_IDENTIFIER),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
    ])
}

// ---------------------------------------------------------------------------
// Fixtures – C99 for-loop with declaration
// ---------------------------------------------------------------------------

/// void f(int n) { for (int i = 0; i < n; i++) { } }
fn fixture_for_with_declaration() -> Fixture {
    build_fixture(&[
        FixtureToken::new("void", TOK_VOID),
        FixtureToken::new("f", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("int", TOK_INT),
        FixtureToken::new("n", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("for", TOK_FOR),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("int", TOK_INT),
        FixtureToken::new("i", TOK_IDENTIFIER),
        FixtureToken::new("=", TOK_ASSIGN),
        FixtureToken::new("0", TOK_INTEGER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("i", TOK_IDENTIFIER),
        FixtureToken::new("<", TOK_LT),
        FixtureToken::new("n", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("i", TOK_IDENTIFIER),
        FixtureToken::new("++", TOK_INC),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("}", TOK_RBRACE),
        FixtureToken::new("}", TOK_RBRACE),
    ])
}

/// void f() { for (int i = 0, j = 1; ; ) { } }
fn fixture_for_with_multiple_declarators() -> Fixture {
    build_fixture(&[
        FixtureToken::new("void", TOK_VOID),
        FixtureToken::new("f", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("for", TOK_FOR),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("int", TOK_INT),
        FixtureToken::new("i", TOK_IDENTIFIER),
        FixtureToken::new("=", TOK_ASSIGN),
        FixtureToken::new("0", TOK_INTEGER),
        FixtureToken::new(",", TOK_COMMA),
        FixtureToken::new("j", TOK_IDENTIFIER),
        FixtureToken::new("=", TOK_ASSIGN),
        FixtureToken::new("1", TOK_INTEGER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("}", TOK_RBRACE),
        FixtureToken::new("}", TOK_RBRACE),
    ])
}

// ---------------------------------------------------------------------------
// Fixtures – C11 atomics
// ---------------------------------------------------------------------------

/// void f() { _Atomic int x; }
fn fixture_atomic_qualifier() -> Fixture {
    build_fixture(&[
        FixtureToken::new("void", TOK_VOID),
        FixtureToken::new("f", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("_Atomic", TOK_ATOMIC),
        FixtureToken::new("int", TOK_INT),
        FixtureToken::new("x", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
    ])
}

/// void f() { _Atomic(int) y; }
fn fixture_atomic_type_specifier() -> Fixture {
    build_fixture(&[
        FixtureToken::new("void", TOK_VOID),
        FixtureToken::new("f", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("_Atomic", TOK_ATOMIC),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("int", TOK_INT),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("y", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
    ])
}

// ---------------------------------------------------------------------------
// Tests – computed goto classification
// ---------------------------------------------------------------------------

#[test]
fn computed_goto_simple_classifies() {
    let fix = fixture_computed_goto_simple();
    assert_full_pipeline_parity(&fix, "computed_goto_simple");

    let typed = classify(&fix);
    assert_eq!(
        row_indices(&typed, C_AST_KIND_GNU_LABEL_ADDRESS_EXPR),
        vec![9],
        "`&&` in computed goto must classify as GNU_LABEL_ADDRESS_EXPR"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_LABEL_STMT),
        vec![12],
        "target label must classify as LABEL_STMT"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_GOTO_STMT).len(),
        0,
        "computed goto is not a GOTO_STMT"
    );
}

#[test]
fn computed_goto_in_goto_classifies() {
    let fix = fixture_computed_goto_in_goto();
    assert_full_pipeline_parity(&fix, "computed_goto_in_goto");

    let typed = classify(&fix);
    assert_eq!(
        row_indices(&typed, C_AST_KIND_GNU_LABEL_ADDRESS_EXPR),
        vec![7],
        "`&&` after `goto *` must classify as GNU_LABEL_ADDRESS_EXPR"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_GOTO_STMT),
        vec![5],
        "`goto` must still classify as GOTO_STMT"
    );
}

#[test]
fn computed_goto_comma_classifies() {
    let fix = fixture_computed_goto_comma();
    assert_full_pipeline_parity(&fix, "computed_goto_comma");

    let typed = classify(&fix);
    let label_addrs = row_indices(&typed, C_AST_KIND_GNU_LABEL_ADDRESS_EXPR);
    assert_eq!(
        label_addrs,
        vec![6, 9],
        "both `&&` in comma expression must classify as GNU_LABEL_ADDRESS_EXPR"
    );
}

// ---------------------------------------------------------------------------
// Tests – C99 for-loop with declaration
// ---------------------------------------------------------------------------

#[test]
fn for_with_declaration_classifies() {
    let fix = fixture_for_with_declaration();
    assert_full_pipeline_parity(&fix, "for_with_declaration");

    let typed = classify(&fix);
    assert_eq!(
        row_indices(&typed, C_AST_KIND_FOR_STMT),
        vec![7],
        "for must classify as FOR_STMT"
    );
    // The `=` inside the for-init should NOT be classified as ASSIGN_EXPR because
    // it is part of a declaration-with-initializer in the C99 for-init scope.
    let assigns = row_indices(&typed, C_AST_KIND_ASSIGN_EXPR);
    assert!(
        assigns.is_empty(),
        "assignment inside C99 for-init must not be a top-level ASSIGN_EXPR"
    );
}

#[test]
fn for_with_multiple_declarators_classifies() {
    let fix = fixture_for_with_multiple_declarators();
    assert_full_pipeline_parity(&fix, "for_with_multiple_declarators");

    let typed = classify(&fix);
    assert_eq!(
        row_indices(&typed, C_AST_KIND_FOR_STMT),
        vec![5],
        "for with multiple declarators must classify as FOR_STMT"
    );
}

// ---------------------------------------------------------------------------
// Tests – C11 atomics
// ---------------------------------------------------------------------------

#[test]
fn atomic_qualifier_classifies() {
    let fix = fixture_atomic_qualifier();
    assert_full_pipeline_parity(&fix, "atomic_qualifier");

    let typed = classify(&fix);
    // `_Atomic` should be treated as a declaration prefix token, not as an identifier.
    assert!(
        row_indices(&typed, node_kind::CALL).is_empty(),
        "_Atomic must not be confused with a function call"
    );
    assert!(
        row_indices(&typed, node_kind::BINARY).is_empty(),
        "_Atomic must not be confused with a binary operator"
    );
}

#[test]
fn atomic_type_specifier_classifies() {
    let fix = fixture_atomic_type_specifier();
    assert_full_pipeline_parity(&fix, "atomic_type_specifier");

    let typed = classify(&fix);
    assert!(
        row_indices(&typed, node_kind::CALL).is_empty(),
        "_Atomic(type) must not be confused with a function call"
    );
    // The `(` after `_Atomic` must NOT be classified as CAST_EXPR because it is
    // part of the _Atomic type-specifier syntax.
    assert!(
        row_indices(&typed, C_AST_KIND_CAST_EXPR).is_empty(),
        "paren after _Atomic must not be classified as CAST_EXPR"
    );
}

// ---------------------------------------------------------------------------
// Tests – PG lowering preservation
// ---------------------------------------------------------------------------
