// GPU/CPU parity end-to-end tests for statement construct gaps and
// label/goto interactions that appear in Linux-grade C but lack
// dedicated coverage.
//
// Constructs under test:
//   - empty statement (`;`)
//   - for-loop with a declaration in the init clause (C99)
//   - labels inside nested loops, switch, and if bodies
//   - goto jumping across nested block boundaries
//
// A missing GPU adapter is a configuration failure.

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
    reference_c11_classify_vast_node_kinds, C_AST_KIND_BREAK_STMT, C_AST_KIND_CASE_STMT,
    C_AST_KIND_CONTINUE_STMT, C_AST_KIND_DEFAULT_STMT, C_AST_KIND_DO_STMT, C_AST_KIND_FOR_STMT,
    C_AST_KIND_GOTO_STMT, C_AST_KIND_IF_STMT, C_AST_KIND_LABEL_STMT, C_AST_KIND_RETURN_STMT,
    C_AST_KIND_SWITCH_STMT, C_AST_KIND_WHILE_STMT,
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
// Fixture builders
// ---------------------------------------------------------------------------

/// void f() { int x; ; }
fn fixture_empty_statement() -> Fixture {
    build_fixture(&[
        FixtureToken::new("void", TOK_IDENTIFIER),
        FixtureToken::new("f", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("void", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new("x", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
    ])
}

/// void g() { for (int i = 0; i < 10; i++) { } }
fn fixture_for_with_declaration() -> Fixture {
    build_fixture(&[
        FixtureToken::new("void", TOK_IDENTIFIER),
        FixtureToken::new("g", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("void", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("for", TOK_FOR),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new("i", TOK_IDENTIFIER),
        FixtureToken::new("=", TOK_ASSIGN),
        FixtureToken::new("0", TOK_INTEGER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("i", TOK_IDENTIFIER),
        FixtureToken::new("<", TOK_LT),
        FixtureToken::new("10", TOK_INTEGER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("i", TOK_IDENTIFIER),
        FixtureToken::new("++", TOK_INC),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("}", TOK_RBRACE),
        FixtureToken::new("}", TOK_RBRACE),
    ])
}

/// void h() { while (1) { label: goto label; } }
fn fixture_label_goto_inside_while() -> Fixture {
    build_fixture(&[
        FixtureToken::new("void", TOK_IDENTIFIER),
        FixtureToken::new("h", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("void", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("while", TOK_WHILE),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("1", TOK_INTEGER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("label", TOK_IDENTIFIER),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new("goto", TOK_GOTO),
        FixtureToken::new("label", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
        FixtureToken::new("}", TOK_RBRACE),
    ])
}

/// void k(int x) { switch (x) { case 1: if (1) { goto end; } end: ; } }
fn fixture_goto_across_switch_case() -> Fixture {
    build_fixture(&[
        FixtureToken::new("void", TOK_IDENTIFIER),
        FixtureToken::new("k", TOK_IDENTIFIER),
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
        FixtureToken::new("if", TOK_IF),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("1", TOK_INTEGER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("goto", TOK_GOTO),
        FixtureToken::new("end", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
        FixtureToken::new("end", TOK_IDENTIFIER),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
        FixtureToken::new("}", TOK_RBRACE),
    ])
}

/// void m(int x) { switch (x) { default: do { continue; } while (0); case 1: break; } return; }
fn fixture_default_do_break_continue_return() -> Fixture {
    build_fixture(&[
        FixtureToken::new("void", TOK_IDENTIFIER),
        FixtureToken::new("m", TOK_IDENTIFIER),
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
        FixtureToken::new("default", TOK_DEFAULT),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new("do", TOK_DO),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("continue", TOK_CONTINUE),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
        FixtureToken::new("while", TOK_WHILE),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("0", TOK_INTEGER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("case", TOK_CASE),
        FixtureToken::new("1", TOK_INTEGER),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new("break", TOK_BREAK),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
        FixtureToken::new("return", TOK_RETURN),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
    ])
}

/// void n(void) { { { return; } } }
fn fixture_nested_compound_return() -> Fixture {
    build_fixture(&[
        FixtureToken::new("void", TOK_IDENTIFIER),
        FixtureToken::new("n", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("void", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("return", TOK_RETURN),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
        FixtureToken::new("}", TOK_RBRACE),
        FixtureToken::new("}", TOK_RBRACE),
    ])
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn empty_statement_inside_function_gpu_cpu_parity() {
    let fix = fixture_empty_statement();
    assert_full_pipeline_parity(&fix, "empty_statement");

    let typed = classify(&fix);

    assert!(
        row_indices(&typed, node_kind::VARIABLE).contains(&7),
        "x must classify as VARIABLE"
    );
    // The second semicolon is an empty statement; it should not crash and
    // should simply classify as 0 (unknown / raw token).
    assert_eq!(
        word_at(&typed, 9 * VAST_STRIDE_U32),
        0,
        "empty statement semicolon must classify as 0"
    );
}

#[test]
fn for_loop_with_declaration_gpu_cpu_parity() {
    let fix = fixture_for_with_declaration();
    assert_full_pipeline_parity(&fix, "for_with_declaration");

    let typed = classify(&fix);

    assert_eq!(
        row_indices(&typed, C_AST_KIND_FOR_STMT),
        vec![6],
        "for must classify as FOR_STMT"
    );
    assert!(
        row_indices(&typed, node_kind::VARIABLE).contains(&9),
        "i must classify as VARIABLE"
    );
    assert_eq!(
        word_at(&typed, 11 * VAST_STRIDE_U32),
        node_kind::LITERAL,
        "declaration initializer literal must classify as LITERAL"
    );
    assert!(
        row_indices(&typed, node_kind::BASIC_BLOCK).contains(&20),
        "for body brace must classify as BASIC_BLOCK"
    );
}

#[test]
fn label_goto_inside_while_loop_gpu_cpu_parity() {
    let fix = fixture_label_goto_inside_while();
    assert_full_pipeline_parity(&fix, "label_goto_inside_while");

    let typed = classify(&fix);

    assert_eq!(
        row_indices(&typed, C_AST_KIND_WHILE_STMT),
        vec![6],
        "while must classify as WHILE_STMT"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_GOTO_STMT),
        vec![13],
        "goto must classify as GOTO_STMT"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_LABEL_STMT),
        vec![11],
        "label definition must classify as LABEL_STMT"
    );
}

#[test]
fn goto_across_switch_case_blocks_gpu_cpu_parity() {
    let fix = fixture_goto_across_switch_case();
    assert_full_pipeline_parity(&fix, "goto_across_switch_case");

    let typed = classify(&fix);

    assert_eq!(
        row_indices(&typed, C_AST_KIND_SWITCH_STMT),
        vec![7],
        "switch must classify as SWITCH_STMT"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_CASE_STMT),
        vec![12],
        "case must classify as CASE_STMT"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_IF_STMT),
        vec![15],
        "if must classify as IF_STMT"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_GOTO_STMT),
        vec![20],
        "goto must classify as GOTO_STMT"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_LABEL_STMT),
        vec![24],
        "end label after the if-block must classify as LABEL_STMT"
    );
}

#[test]
fn default_do_break_continue_return_gpu_cpu_parity() {
    let fix = fixture_default_do_break_continue_return();
    assert_full_pipeline_parity(&fix, "default_do_break_continue_return");

    let typed = classify(&fix);
    assert_eq!(
        row_indices(&typed, C_AST_KIND_SWITCH_STMT),
        vec![7],
        "switch must classify as SWITCH_STMT"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_DEFAULT_STMT),
        vec![12],
        "default must classify as DEFAULT_STMT"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_DO_STMT),
        vec![14],
        "do must classify as DO_STMT"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_CONTINUE_STMT),
        vec![16],
        "continue must classify as CONTINUE_STMT"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_WHILE_STMT),
        vec![19],
        "do/while trailer must classify while as WHILE_STMT"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_CASE_STMT),
        vec![24],
        "case must classify as CASE_STMT"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_BREAK_STMT),
        vec![27],
        "break must classify as BREAK_STMT"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_RETURN_STMT),
        vec![30],
        "return after switch must classify as RETURN_STMT"
    );
}
