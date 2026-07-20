// C parser contract tests for GNU builtins (`__builtin_expect`,
// `__builtin_choose_expr`) in control-flow contexts that stress VAST/PG
// lowering.
//
// Constructs under test:
//   - `__builtin_expect` as an if-condition
//   - `__builtin_expect` as a switch-selector
//   - `__builtin_choose_expr` inside a statement expression
//   - `__builtin_choose_expr` inside a designated initializer value
//   - nested builtins (`__builtin_expect` around `__builtin_choose_expr`)
//   - PG lowering preservation (kind, span, parent, first_child, next_sibling)
//   - GPU/CPU parity for the full pipeline
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
    reference_c11_classify_vast_node_kinds, C_AST_KIND_BUILTIN_CHOOSE_EXPR,
    C_AST_KIND_BUILTIN_EXPECT_EXPR, C_AST_KIND_IF_STMT, C_AST_KIND_SWITCH_STMT,
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
/// if (__builtin_expect(x, 1)) { }
/// ```
fn fixture_builtin_expect_if_condition() -> Fixture {
    build_fixture(&[
        FixtureToken::new("if", TOK_IF),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("__builtin_expect", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("x", TOK_IDENTIFIER),
        FixtureToken::new(",", TOK_COMMA),
        FixtureToken::new("1", TOK_INTEGER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("}", TOK_RBRACE),
    ])
}

/// ```c
/// switch (__builtin_expect(x, 0)) { case 1: break; }
/// ```
fn fixture_builtin_expect_switch_selector() -> Fixture {
    build_fixture(&[
        FixtureToken::new("switch", TOK_SWITCH),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("__builtin_expect", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("x", TOK_IDENTIFIER),
        FixtureToken::new(",", TOK_COMMA),
        FixtureToken::new("0", TOK_INTEGER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("case", TOK_CASE),
        FixtureToken::new("1", TOK_INTEGER),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new("break", TOK_BREAK),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
    ])
}

/// ```c
/// int y = ({ __builtin_choose_expr(1, 2, 3); });
/// ```
fn fixture_builtin_choose_expr_in_statement_expr() -> Fixture {
    build_fixture(&[
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new("y", TOK_IDENTIFIER),
        FixtureToken::new("=", TOK_ASSIGN),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("__builtin_choose_expr", TOK_BUILTIN_CHOOSE_EXPR),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("1", TOK_INTEGER),
        FixtureToken::new(",", TOK_COMMA),
        FixtureToken::new("2", TOK_INTEGER),
        FixtureToken::new(",", TOK_COMMA),
        FixtureToken::new("3", TOK_INTEGER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

/// ```c
/// struct S { int a; };
/// struct S s = { .a = __builtin_choose_expr(1, 10, 20) };
/// ```
fn fixture_builtin_choose_expr_in_designated_init() -> Fixture {
    build_fixture(&[
        FixtureToken::new("struct", TOK_STRUCT),
        FixtureToken::new("S", TOK_IDENTIFIER),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new("a", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("struct", TOK_STRUCT),
        FixtureToken::new("S", TOK_IDENTIFIER),
        FixtureToken::new("s", TOK_IDENTIFIER),
        FixtureToken::new("=", TOK_ASSIGN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new(".", TOK_DOT),
        FixtureToken::new("a", TOK_IDENTIFIER),
        FixtureToken::new("=", TOK_ASSIGN),
        FixtureToken::new("__builtin_choose_expr", TOK_BUILTIN_CHOOSE_EXPR),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("1", TOK_INTEGER),
        FixtureToken::new(",", TOK_COMMA),
        FixtureToken::new("10", TOK_INTEGER),
        FixtureToken::new(",", TOK_COMMA),
        FixtureToken::new("20", TOK_INTEGER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("}", TOK_RBRACE),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

/// ```c
/// int z = __builtin_expect(__builtin_choose_expr(1, 2, 3), 1);
/// ```
fn fixture_nested_builtin_expect_choose_expr() -> Fixture {
    build_fixture(&[
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new("z", TOK_IDENTIFIER),
        FixtureToken::new("=", TOK_ASSIGN),
        FixtureToken::new("__builtin_expect", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("__builtin_choose_expr", TOK_BUILTIN_CHOOSE_EXPR),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("1", TOK_INTEGER),
        FixtureToken::new(",", TOK_COMMA),
        FixtureToken::new("2", TOK_INTEGER),
        FixtureToken::new(",", TOK_COMMA),
        FixtureToken::new("3", TOK_INTEGER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(",", TOK_COMMA),
        FixtureToken::new("1", TOK_INTEGER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

/// ```c
/// int w = __builtin_expect(!!(x), 1) ? 1 : 0;
/// ```
fn fixture_builtin_expect_in_ternary() -> Fixture {
    build_fixture(&[
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new("w", TOK_IDENTIFIER),
        FixtureToken::new("=", TOK_ASSIGN),
        FixtureToken::new("__builtin_expect", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("!", TOK_BANG),
        FixtureToken::new("!", TOK_BANG),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("x", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(",", TOK_COMMA),
        FixtureToken::new("1", TOK_INTEGER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("?", TOK_QUESTION),
        FixtureToken::new("1", TOK_INTEGER),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new("0", TOK_INTEGER),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

// ---------------------------------------------------------------------------
// CPU reference contracts
// ---------------------------------------------------------------------------

#[test]
fn cpu_builtin_expect_if_condition_classifies() {
    let fix = fixture_builtin_expect_if_condition();
    let typed = classify(&fix);
    assert_eq!(
        row_indices(&typed, C_AST_KIND_IF_STMT),
        vec![0],
        "if must classify as IF_STMT"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_BUILTIN_EXPECT_EXPR),
        vec![2],
        "__builtin_expect in if condition must classify as BUILTIN_EXPECT_EXPR"
    );
    assert!(
        row_indices(&typed, node_kind::CALL).is_empty(),
        "__builtin_expect must not collapse into CALL"
    );
}

#[test]
fn cpu_builtin_expect_switch_selector_classifies() {
    let fix = fixture_builtin_expect_switch_selector();
    let typed = classify(&fix);
    assert_eq!(
        row_indices(&typed, C_AST_KIND_SWITCH_STMT),
        vec![0],
        "switch must classify as SWITCH_STMT"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_BUILTIN_EXPECT_EXPR),
        vec![2],
        "__builtin_expect in switch selector must classify as BUILTIN_EXPECT_EXPR"
    );
}

#[test]
fn cpu_builtin_choose_expr_in_statement_expr_classifies() {
    let fix = fixture_builtin_choose_expr_in_statement_expr();
    let typed = classify(&fix);
    assert_eq!(
        row_indices(&typed, C_AST_KIND_BUILTIN_CHOOSE_EXPR),
        vec![5],
        "__builtin_choose_expr inside statement expression must classify"
    );
    assert!(
        !row_indices(&typed, node_kind::BASIC_BLOCK).is_empty(),
        "statement expression must contain a BASIC_BLOCK"
    );
}

#[test]
fn cpu_builtin_choose_expr_in_designated_init_classifies() {
    let fix = fixture_builtin_choose_expr_in_designated_init();
    let typed = classify(&fix);
    assert_eq!(
        row_indices(&typed, C_AST_KIND_BUILTIN_CHOOSE_EXPR),
        vec![16],
        "__builtin_choose_expr in designated initializer value must classify"
    );
}

#[test]
fn cpu_nested_builtin_expect_choose_expr_classifies() {
    let fix = fixture_nested_builtin_expect_choose_expr();
    let typed = classify(&fix);
    assert_eq!(
        row_indices(&typed, C_AST_KIND_BUILTIN_EXPECT_EXPR),
        vec![3],
        "outer __builtin_expect must classify"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_BUILTIN_CHOOSE_EXPR),
        vec![5],
        "inner __builtin_choose_expr must classify"
    );
}

#[test]
fn cpu_builtin_expect_in_ternary_classifies() {
    let fix = fixture_builtin_expect_in_ternary();
    let typed = classify(&fix);
    assert_eq!(
        row_indices(&typed, C_AST_KIND_BUILTIN_EXPECT_EXPR),
        vec![3],
        "__builtin_expect in ternary condition must classify"
    );
}

// ---------------------------------------------------------------------------
// PG lowering preservation contracts
// ---------------------------------------------------------------------------

#[test]
fn pg_lower_preserves_builtin_expect_if_condition() {
    let fix = fixture_builtin_expect_if_condition();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let pg = reference_ast_to_pg_nodes(&typed);

    assert_pg_preserves_row(&typed, &pg, &fix, 0, C_AST_KIND_IF_STMT);
    assert_pg_preserves_row(&typed, &pg, &fix, 2, C_AST_KIND_BUILTIN_EXPECT_EXPR);
}

#[test]
fn pg_lower_preserves_builtin_expect_switch_selector() {
    let fix = fixture_builtin_expect_switch_selector();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let pg = reference_ast_to_pg_nodes(&typed);

    assert_pg_preserves_row(&typed, &pg, &fix, 0, C_AST_KIND_SWITCH_STMT);
    assert_pg_preserves_row(&typed, &pg, &fix, 2, C_AST_KIND_BUILTIN_EXPECT_EXPR);
}

#[test]
fn pg_lower_preserves_builtin_choose_expr_in_statement_expr() {
    let fix = fixture_builtin_choose_expr_in_statement_expr();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let pg = reference_ast_to_pg_nodes(&typed);

    assert_pg_preserves_row(&typed, &pg, &fix, 5, C_AST_KIND_BUILTIN_CHOOSE_EXPR);
}

#[test]
fn pg_lower_preserves_builtin_choose_expr_in_designated_init() {
    let fix = fixture_builtin_choose_expr_in_designated_init();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let pg = reference_ast_to_pg_nodes(&typed);

    assert_pg_preserves_row(&typed, &pg, &fix, 16, C_AST_KIND_BUILTIN_CHOOSE_EXPR);
}

#[test]
fn pg_lower_preserves_nested_builtin_expect_choose_expr() {
    let fix = fixture_nested_builtin_expect_choose_expr();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let pg = reference_ast_to_pg_nodes(&typed);

    assert_pg_preserves_row(&typed, &pg, &fix, 3, C_AST_KIND_BUILTIN_EXPECT_EXPR);
    assert_pg_preserves_row(&typed, &pg, &fix, 5, C_AST_KIND_BUILTIN_CHOOSE_EXPR);
}
