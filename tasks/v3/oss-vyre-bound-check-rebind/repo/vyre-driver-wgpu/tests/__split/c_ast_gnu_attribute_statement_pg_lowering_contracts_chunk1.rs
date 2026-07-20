// C parser contract tests for GNU `__attribute__` on statements, labels,
// and declarations inside statement expressions  -  contexts likely to break
// VAST/PG lowering.
//
// Constructs under test:
//   - `__attribute__((fallthrough))` as a statement in switch bodies
//   - `__attribute__((unused))` on a declaration inside a statement expression
//   - `__attribute__((aligned))` on a label (GNU extension)
//   - multiple attributes on a declaration inside a compound statement
//   - PG lowering preservation and GPU/CPU parity
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
    reference_c11_classify_vast_node_kinds, C_AST_KIND_ATTRIBUTE_ALIGNED,
    C_AST_KIND_ATTRIBUTE_FALLTHROUGH, C_AST_KIND_ATTRIBUTE_UNUSED, C_AST_KIND_GNU_ATTRIBUTE,
    C_AST_KIND_GNU_STATEMENT_EXPR, C_AST_KIND_IF_STMT, C_AST_KIND_LABEL_STMT,
    C_AST_KIND_SWITCH_STMT,
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

/// ```c
/// void f(int x) {
///   switch (x) {
///     case 1:
///       __attribute__((fallthrough));
///     case 2:
///       break;
///   }
/// }
/// ```
fn fixture_attribute_fallthrough_statement() -> Fixture {
    build_fixture(&[
        FixtureToken::new("void", TOK_IDENTIFIER),
        FixtureToken::new("f", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new("x", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("switch", TOK_SWITCH),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("x", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("case", TOK_CASE),
        FixtureToken::new("1", TOK_INTEGER),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new("__attribute__", TOK_GNU_ATTRIBUTE),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("fallthrough", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("case", TOK_CASE),
        FixtureToken::new("2", TOK_INTEGER),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new("break", TOK_BREAK),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
        FixtureToken::new("}", TOK_RBRACE),
    ])
}

/// ```c
/// int v = ({ __attribute__((unused)) int tmp = 1; tmp; });
/// ```
fn fixture_attribute_unused_in_statement_expr() -> Fixture {
    build_fixture(&[
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new("v", TOK_IDENTIFIER),
        FixtureToken::new("=", TOK_ASSIGN),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("__attribute__", TOK_GNU_ATTRIBUTE),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("unused", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new("tmp", TOK_IDENTIFIER),
        FixtureToken::new("=", TOK_ASSIGN),
        FixtureToken::new("1", TOK_INTEGER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("tmp", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

/// ```c
/// void g() {
///   __attribute__((aligned(16))) label:
///     return;
/// }
/// ```
fn fixture_attribute_aligned_on_label() -> Fixture {
    build_fixture(&[
        FixtureToken::new("void", TOK_IDENTIFIER),
        FixtureToken::new("g", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("__attribute__", TOK_GNU_ATTRIBUTE),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("aligned", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("16", TOK_INTEGER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("label", TOK_IDENTIFIER),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new("return", TOK_RETURN),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
    ])
}

/// ```c
/// void h() {
///   __attribute__((section(".data"))) __attribute__((used)) int sym = 0;
/// }
/// ```
fn fixture_multiple_attributes_in_compound() -> Fixture {
    build_fixture(&[
        FixtureToken::new("void", TOK_IDENTIFIER),
        FixtureToken::new("h", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("__attribute__", TOK_GNU_ATTRIBUTE),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("section", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("\".data\"", TOK_STRING),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("__attribute__", TOK_GNU_ATTRIBUTE),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("used", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new("sym", TOK_IDENTIFIER),
        FixtureToken::new("=", TOK_ASSIGN),
        FixtureToken::new("0", TOK_INTEGER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
    ])
}

/// ```c
/// void k() {
///   if (1)
///     __attribute__((cold)) return;
/// }
/// ```
fn fixture_attribute_on_if_arm_statement() -> Fixture {
    build_fixture(&[
        FixtureToken::new("void", TOK_IDENTIFIER),
        FixtureToken::new("k", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("if", TOK_IF),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("1", TOK_INTEGER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("__attribute__", TOK_GNU_ATTRIBUTE),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("cold", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("return", TOK_RETURN),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
    ])
}

// ---------------------------------------------------------------------------
// CPU reference contracts
// ---------------------------------------------------------------------------

#[test]
fn cpu_attribute_fallthrough_in_switch_classifies() {
    let fix = fixture_attribute_fallthrough_statement();
    let typed = classify(&fix);
    assert_eq!(
        row_indices(&typed, C_AST_KIND_SWITCH_STMT),
        vec![7],
        "switch must classify as SWITCH_STMT"
    );
    assert!(
        !row_indices(&typed, C_AST_KIND_GNU_ATTRIBUTE).is_empty(),
        "__attribute__ must classify as GNU_ATTRIBUTE"
    );
    // The fallthrough attribute detail should be recognized if the parser
    // supports statement-level attributes.
    let fallthrough_rows = row_indices(&typed, C_AST_KIND_ATTRIBUTE_FALLTHROUGH);
    assert!(
        !fallthrough_rows.is_empty(),
        "fallthrough inside switch must classify as ATTRIBUTE_FALLTHROUGH"
    );
}

#[test]
fn cpu_attribute_unused_in_statement_expr_classifies() {
    let fix = fixture_attribute_unused_in_statement_expr();
    let typed = classify(&fix);
    assert_eq!(
        row_indices(&typed, C_AST_KIND_GNU_STATEMENT_EXPR),
        vec![3],
        "statement-expression introducer must classify"
    );
    assert!(
        !row_indices(&typed, C_AST_KIND_GNU_ATTRIBUTE).is_empty(),
        "__attribute__ inside statement expression must classify"
    );
    assert!(
        !row_indices(&typed, C_AST_KIND_ATTRIBUTE_UNUSED).is_empty(),
        "unused attribute detail must classify"
    );
    let vars = row_indices(&typed, node_kind::VARIABLE);
    assert!(
        !vars.is_empty(),
        "tmp must classify as VARIABLE; got {vars:?}"
    );
}

#[test]
fn cpu_attribute_aligned_on_label_classifies() {
    let fix = fixture_attribute_aligned_on_label();
    let typed = classify(&fix);
    let labels = row_indices(&typed, C_AST_KIND_LABEL_STMT);
    assert_ne!(labels.len(), 0,
        "label must classify as LABEL_STMT; got {labels:?}"
    );
    assert!(
        !row_indices(&typed, C_AST_KIND_GNU_ATTRIBUTE).is_empty(),
        "__attribute__ before label must classify"
    );
    assert!(
        !row_indices(&typed, C_AST_KIND_ATTRIBUTE_ALIGNED).is_empty(),
        "aligned attribute detail must classify"
    );
}

#[test]
fn cpu_multiple_attributes_in_compound_classifies() {
    let fix = fixture_multiple_attributes_in_compound();
    let typed = classify(&fix);
    let attrs = row_indices(&typed, C_AST_KIND_GNU_ATTRIBUTE);
    assert_eq!(
        attrs.len(),
        2,
        "both __attribute__ lists must classify as GNU_ATTRIBUTE"
    );
    let vars = row_indices(&typed, node_kind::VARIABLE);
    assert!(
        !vars.is_empty(),
        "sym must classify as VARIABLE; got {vars:?}"
    );
}

#[test]
fn cpu_attribute_on_if_arm_statement_classifies() {
    let fix = fixture_attribute_on_if_arm_statement();
    let typed = classify(&fix);
    assert_eq!(
        row_indices(&typed, C_AST_KIND_IF_STMT),
        vec![5],
        "if must classify as IF_STMT"
    );
    assert!(
        !row_indices(&typed, C_AST_KIND_GNU_ATTRIBUTE).is_empty(),
        "__attribute__ on if-arm statement must classify"
    );
}

// ---------------------------------------------------------------------------
// PG lowering preservation contracts
// ---------------------------------------------------------------------------

#[test]
fn pg_lower_preserves_attribute_fallthrough_in_switch() {
    let fix = fixture_attribute_fallthrough_statement();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let pg = reference_ast_to_pg_nodes(&typed);

    assert_pg_preserves_row(&typed, &pg, &fix, 7, C_AST_KIND_SWITCH_STMT);
    for idx in row_indices(&typed, C_AST_KIND_GNU_ATTRIBUTE) {
        assert_pg_preserves_row(&typed, &pg, &fix, idx, C_AST_KIND_GNU_ATTRIBUTE);
    }
}

#[test]
fn pg_lower_preserves_attribute_unused_in_statement_expr() {
    let fix = fixture_attribute_unused_in_statement_expr();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let pg = reference_ast_to_pg_nodes(&typed);

    assert_pg_preserves_row(&typed, &pg, &fix, 3, C_AST_KIND_GNU_STATEMENT_EXPR);
    for idx in row_indices(&typed, C_AST_KIND_ATTRIBUTE_UNUSED) {
        assert_pg_preserves_row(&typed, &pg, &fix, idx, C_AST_KIND_ATTRIBUTE_UNUSED);
    }
}
