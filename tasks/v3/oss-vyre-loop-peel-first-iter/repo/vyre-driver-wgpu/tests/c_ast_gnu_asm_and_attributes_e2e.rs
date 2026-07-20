//! GPU/CPU parity end-to-end tests for GNU asm, asm goto, __attribute__ around
//! declarations, and Linux-style statement expressions.
//!
//! A missing GPU adapter is a configuration failure.

#![cfg(feature = "c-parser")]
#![allow(clippy::erasing_op)]
#![allow(deprecated)]
mod c_ast_gpu_parity_support;

use c_ast_gpu_parity_support::{
    assert_full_pipeline_parity, build_fixture, row_indices, word_at, Fixture, FixtureToken,
    VAST_STRIDE_U32,
};
use vyre_libs::parsing::c::lex::tokens::*;
use vyre_libs::parsing::c::parse::vast::{
    reference_c11_annotate_typedef_names, reference_c11_build_vast_nodes,
    reference_c11_classify_vast_node_kinds, C_AST_KIND_ASM_CLOBBERS_LIST,
    C_AST_KIND_ASM_GOTO_LABELS, C_AST_KIND_ASM_INPUT_OPERAND, C_AST_KIND_ASM_OUTPUT_OPERAND,
    C_AST_KIND_ASM_TEMPLATE, C_AST_KIND_ASSIGN_EXPR, C_AST_KIND_ATTRIBUTE_ALIGNED,
    C_AST_KIND_ATTRIBUTE_SECTION, C_AST_KIND_ATTRIBUTE_UNUSED, C_AST_KIND_ATTRIBUTE_USED,
    C_AST_KIND_GNU_ATTRIBUTE, C_AST_KIND_GOTO_STMT, C_AST_KIND_IF_STMT, C_AST_KIND_INLINE_ASM,
};
use vyre_primitives::predicate::node_kind;

// ---------------------------------------------------------------------------
// Fixture builders
// ---------------------------------------------------------------------------

fn fixture_asm_goto_multiple_labels() -> Fixture {
    build_fixture(&[
        FixtureToken::new("__asm__", TOK_IDENTIFIER),
        FixtureToken::new("goto", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("\"jmp %l0\"", TOK_STRING),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new("fail", TOK_IDENTIFIER),
        FixtureToken::new(",", TOK_COMMA),
        FixtureToken::new("ok", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

fn fixture_asm_with_multiple_clobbers() -> Fixture {
    build_fixture(&[
        FixtureToken::new("__asm__", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("\"syscall\"", TOK_STRING),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new("\"memory\"", TOK_STRING),
        FixtureToken::new(",", TOK_COMMA),
        FixtureToken::new("\"cc\"", TOK_STRING),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

fn fixture_asm_extended_with_input_output() -> Fixture {
    build_fixture(&[
        FixtureToken::new("asm", TOK_IDENTIFIER),
        FixtureToken::new("volatile", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("\"mov %1, %0\"", TOK_STRING),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new("\"=a\"", TOK_STRING),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("out", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new("\"r\"", TOK_STRING),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("in", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new("\"rax\"", TOK_STRING),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

fn fixture_attribute_before_variable() -> Fixture {
    build_fixture(&[
        FixtureToken::new("__attribute__", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("aligned", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("64", TOK_INTEGER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("static", TOK_IDENTIFIER),
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new("buf", TOK_IDENTIFIER),
        FixtureToken::new("[", TOK_LBRACKET),
        FixtureToken::new("8", TOK_INTEGER),
        FixtureToken::new("]", TOK_RBRACKET),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

fn fixture_attribute_after_function_declarator() -> Fixture {
    build_fixture(&[
        FixtureToken::new("void", TOK_IDENTIFIER),
        FixtureToken::new("cleanup", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("void", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("__attribute__", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("unused", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

fn fixture_attribute_multiple_on_declaration() -> Fixture {
    build_fixture(&[
        FixtureToken::new("__attribute__", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("section", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("\".data\"", TOK_STRING),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("__attribute__", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("used", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new("sym", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

fn fixture_linux_statement_expression() -> Fixture {
    build_fixture(&[
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new("v", TOK_IDENTIFIER),
        FixtureToken::new("=", TOK_ASSIGN),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new("x", TOK_IDENTIFIER),
        FixtureToken::new("=", TOK_ASSIGN),
        FixtureToken::new("1", TOK_INTEGER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new("y", TOK_IDENTIFIER),
        FixtureToken::new("=", TOK_ASSIGN),
        FixtureToken::new("2", TOK_INTEGER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("x", TOK_IDENTIFIER),
        FixtureToken::new("+", TOK_PLUS),
        FixtureToken::new("y", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

fn fixture_statement_expression_in_condition() -> Fixture {
    build_fixture(&[
        FixtureToken::new("if", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new("t", TOK_IDENTIFIER),
        FixtureToken::new("=", TOK_ASSIGN),
        FixtureToken::new("3", TOK_INTEGER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("t", TOK_IDENTIFIER),
        FixtureToken::new(">", TOK_GT),
        FixtureToken::new("0", TOK_INTEGER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("}", TOK_RBRACE),
    ])
}

// ---------------------------------------------------------------------------
// Tests  -  GNU asm & asm goto
// ---------------------------------------------------------------------------

#[test]
fn asm_goto_multiple_labels_gpu_cpu_parity() {
    let fix = fixture_asm_goto_multiple_labels();
    assert_eq!(
        fix.tok_types[0], TOK_GNU_ASM,
        "__asm__ must promote to TOK_GNU_ASM"
    );
    assert_full_pipeline_parity(&fix, "asm_goto_multiple_labels");

    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);

    assert_eq!(
        row_indices(&typed, C_AST_KIND_INLINE_ASM),
        vec![0],
        "__asm__ must classify as INLINE_ASM"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ASM_TEMPLATE),
        vec![3],
        "template must classify as ASM_TEMPLATE"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ASM_GOTO_LABELS),
        vec![8, 10],
        "both labels must classify as ASM_GOTO_LABELS"
    );
    assert_ne!(
        word_at(&typed, VAST_STRIDE_U32),
        C_AST_KIND_GOTO_STMT,
        "goto after asm is a qualifier, not a standalone goto statement"
    );
}

#[test]
fn asm_with_multiple_clobbers_gpu_cpu_parity() {
    let fix = fixture_asm_with_multiple_clobbers();
    assert_full_pipeline_parity(&fix, "asm_with_multiple_clobbers");

    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);

    assert_eq!(row_indices(&typed, C_AST_KIND_INLINE_ASM), vec![0]);
    assert_eq!(row_indices(&typed, C_AST_KIND_ASM_TEMPLATE), vec![2]);
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ASM_CLOBBERS_LIST),
        vec![6, 8],
        "both clobber strings must classify as ASM_CLOBBERS_LIST"
    );
}

#[test]
fn asm_extended_with_input_output_gpu_cpu_parity() {
    let fix = fixture_asm_extended_with_input_output();
    assert_full_pipeline_parity(&fix, "asm_extended_with_input_output");

    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);

    assert_eq!(row_indices(&typed, C_AST_KIND_INLINE_ASM), vec![0]);
    assert_eq!(row_indices(&typed, C_AST_KIND_ASM_TEMPLATE), vec![3]);
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ASM_OUTPUT_OPERAND),
        vec![6],
        "output operand paren must classify"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ASM_INPUT_OPERAND),
        vec![11],
        "input operand paren must classify"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ASM_CLOBBERS_LIST),
        vec![15],
        "clobber string must classify"
    );
}

// ---------------------------------------------------------------------------
// Tests  -  __attribute__ around declarations
// ---------------------------------------------------------------------------

#[test]
fn attribute_before_variable_gpu_cpu_parity() {
    let fix = fixture_attribute_before_variable();
    assert_eq!(
        fix.tok_types[0], TOK_GNU_ATTRIBUTE,
        "__attribute__ must promote"
    );
    assert_eq!(
        fix.tok_types[9], TOK_STATIC,
        "static must promote to keyword"
    );
    assert_eq!(fix.tok_types[10], TOK_INT, "int must promote to keyword");
    assert_full_pipeline_parity(&fix, "attribute_before_variable");

    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);

    assert_eq!(
        row_indices(&typed, C_AST_KIND_GNU_ATTRIBUTE),
        vec![0],
        "__attribute__ must classify as GNU_ATTRIBUTE"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ATTRIBUTE_ALIGNED),
        vec![3],
        "aligned must classify as ATTRIBUTE_ALIGNED"
    );
    assert_eq!(
        word_at(&typed, 11 * VAST_STRIDE_U32),
        node_kind::VARIABLE,
        "buf must classify as VARIABLE"
    );
}

#[test]
fn attribute_after_function_declarator_gpu_cpu_parity() {
    let fix = fixture_attribute_after_function_declarator();
    assert_eq!(
        fix.tok_types[5], TOK_GNU_ATTRIBUTE,
        "__attribute__ must promote"
    );
    assert_full_pipeline_parity(&fix, "attribute_after_function_declarator");

    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);

    assert_eq!(
        word_at(&typed, VAST_STRIDE_U32),
        node_kind::FUNCTION_DECL,
        "cleanup must classify as FUNCTION_DECL"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_GNU_ATTRIBUTE),
        vec![5],
        "__attribute__ after declarator must classify"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ATTRIBUTE_UNUSED),
        vec![8],
        "unused must classify as ATTRIBUTE_UNUSED"
    );
}

#[test]
fn attribute_multiple_on_declaration_gpu_cpu_parity() {
    let fix = fixture_attribute_multiple_on_declaration();
    assert_eq!(
        fix.tok_types[0], TOK_GNU_ATTRIBUTE,
        "first __attribute__ must promote"
    );
    assert_eq!(
        fix.tok_types[9], TOK_GNU_ATTRIBUTE,
        "second __attribute__ must also promote"
    );
    assert_full_pipeline_parity(&fix, "attribute_multiple_on_declaration");

    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);

    assert_eq!(
        row_indices(&typed, C_AST_KIND_GNU_ATTRIBUTE),
        vec![0, 9],
        "both attribute lists must classify as GNU_ATTRIBUTE"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ATTRIBUTE_SECTION),
        vec![3],
        "section must classify as ATTRIBUTE_SECTION"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ATTRIBUTE_USED),
        vec![12],
        "used must classify as ATTRIBUTE_USED"
    );
    assert_eq!(
        word_at(&typed, 16 * VAST_STRIDE_U32),
        node_kind::VARIABLE,
        "sym must classify as VARIABLE"
    );
}

// ---------------------------------------------------------------------------
// Tests  -  Linux-style statement expressions
// ---------------------------------------------------------------------------

#[test]
fn linux_statement_expression_gpu_cpu_parity() {
    let fix = fixture_linux_statement_expression();
    assert_full_pipeline_parity(&fix, "linux_statement_expression");

    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);

    assert_eq!(
        word_at(&typed, 4 * VAST_STRIDE_U32),
        node_kind::BASIC_BLOCK,
        "statement-expression body brace must classify as BASIC_BLOCK"
    );
    assert_eq!(
        word_at(&typed, 7 * VAST_STRIDE_U32),
        C_AST_KIND_ASSIGN_EXPR,
        "first assignment inside statement expression must classify"
    );
    assert_eq!(
        word_at(&typed, 12 * VAST_STRIDE_U32),
        C_AST_KIND_ASSIGN_EXPR,
        "second assignment inside statement expression must classify"
    );
}

#[test]
fn statement_expression_in_condition_gpu_cpu_parity() {
    let fix = fixture_statement_expression_in_condition();
    assert_eq!(fix.tok_types[0], TOK_IF, "if must promote to keyword");
    assert_full_pipeline_parity(&fix, "statement_expression_in_condition");

    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);

    assert_eq!(
        word_at(&typed, 0 * VAST_STRIDE_U32),
        C_AST_KIND_IF_STMT,
        "if must classify as IF_STMT"
    );
    assert_eq!(
        word_at(&typed, 3 * VAST_STRIDE_U32),
        node_kind::BASIC_BLOCK,
        "statement-expression body brace must classify as BASIC_BLOCK"
    );
    assert_eq!(
        word_at(&typed, 6 * VAST_STRIDE_U32),
        C_AST_KIND_ASSIGN_EXPR,
        "assignment inside statement expression must classify"
    );
}
