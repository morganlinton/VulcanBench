//! End-to-end GPU/CPU parity tests for C control-flow AST node classification.
//!
//! These fixtures cover statement forms that appear constantly in kernel and
//! libc code: if/else, switch/case/default, for/while/do loops, and jump
//! statements. A missing GPU adapter is a configuration failure.

#![cfg(feature = "c-parser")]
#![allow(deprecated)]

use vyre_libs::parsing::c::lex::tokens::*;
use vyre_libs::parsing::c::parse::vast::{
    reference_c11_build_vast_nodes, reference_c11_classify_vast_node_kinds, C_AST_KIND_BREAK_STMT,
    C_AST_KIND_CASE_STMT, C_AST_KIND_CONTINUE_STMT, C_AST_KIND_DEFAULT_STMT, C_AST_KIND_DO_STMT,
    C_AST_KIND_ELSE_STMT, C_AST_KIND_FOR_STMT, C_AST_KIND_GOTO_STMT, C_AST_KIND_IF_STMT,
    C_AST_KIND_RETURN_STMT, C_AST_KIND_SWITCH_STMT, C_AST_KIND_WHILE_STMT,
};

mod c_ast_gpu_parity_support;
use c_ast_gpu_parity_support::{
    assert_words_eq, run_gpu_classifier, run_gpu_vast_builder_from_parts, starts_for_lens, word_at,
    VAST_STRIDE_U32,
};

fn assert_statement_kinds(label: &str, tok_types: &[u32], expected_kinds: &[(usize, u32)]) {
    let tok_lens = vec![1; tok_types.len()];
    let tok_starts = starts_for_lens(&tok_lens);

    let raw_cpu = reference_c11_build_vast_nodes(tok_types, &tok_starts, &tok_lens);
    let raw_gpu = run_gpu_vast_builder_from_parts(tok_types, &tok_starts, &tok_lens);
    assert_words_eq(
        &raw_gpu,
        &raw_cpu,
        &format!("{label}: raw VAST GPU/CPU parity"),
    );

    let typed_cpu = reference_c11_classify_vast_node_kinds(&raw_cpu);
    let typed_gpu = run_gpu_classifier(&raw_gpu);
    assert_words_eq(
        &typed_gpu,
        &typed_cpu,
        &format!("{label}: typed VAST GPU/CPU parity"),
    );

    for &(idx, kind) in expected_kinds {
        assert_eq!(
            word_at(&typed_cpu, idx * VAST_STRIDE_U32),
            kind,
            "{label}: CPU statement kind at row {idx}"
        );
        assert_eq!(
            word_at(&typed_gpu, idx * VAST_STRIDE_U32),
            kind,
            "{label}: GPU statement kind at row {idx}"
        );
    }
}

#[test]
fn if_else_return_statements_classify_on_gpu_and_cpu() {
    let tokens = [
        TOK_IF,
        TOK_LPAREN,
        TOK_IDENTIFIER,
        TOK_RPAREN,
        TOK_RETURN,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
        TOK_ELSE,
        TOK_RETURN,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
    ];
    assert_statement_kinds(
        "if/else returns",
        &tokens,
        &[
            (0, C_AST_KIND_IF_STMT),
            (4, C_AST_KIND_RETURN_STMT),
            (7, C_AST_KIND_ELSE_STMT),
            (8, C_AST_KIND_RETURN_STMT),
        ],
    );
}

#[test]
fn switch_case_default_and_jump_statements_classify_on_gpu_and_cpu() {
    let tokens = [
        TOK_SWITCH,
        TOK_LPAREN,
        TOK_IDENTIFIER,
        TOK_RPAREN,
        TOK_LBRACE,
        TOK_CASE,
        TOK_INTEGER,
        TOK_COLON,
        TOK_BREAK,
        TOK_SEMICOLON,
        TOK_DEFAULT,
        TOK_COLON,
        TOK_CONTINUE,
        TOK_SEMICOLON,
        TOK_RBRACE,
    ];
    assert_statement_kinds(
        "switch/case/default",
        &tokens,
        &[
            (0, C_AST_KIND_SWITCH_STMT),
            (5, C_AST_KIND_CASE_STMT),
            (8, C_AST_KIND_BREAK_STMT),
            (10, C_AST_KIND_DEFAULT_STMT),
            (12, C_AST_KIND_CONTINUE_STMT),
        ],
    );
}

#[test]
fn nested_loop_statements_classify_on_gpu_and_cpu() {
    let tokens = [
        TOK_FOR,
        TOK_LPAREN,
        TOK_IDENTIFIER,
        TOK_ASSIGN,
        TOK_INTEGER,
        TOK_SEMICOLON,
        TOK_IDENTIFIER,
        TOK_LT,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
        TOK_IDENTIFIER,
        TOK_INC,
        TOK_RPAREN,
        TOK_LBRACE,
        TOK_WHILE,
        TOK_LPAREN,
        TOK_IDENTIFIER,
        TOK_RPAREN,
        TOK_DO,
        TOK_IDENTIFIER,
        TOK_DEC,
        TOK_SEMICOLON,
        TOK_WHILE,
        TOK_LPAREN,
        TOK_IDENTIFIER,
        TOK_RPAREN,
        TOK_SEMICOLON,
        TOK_RBRACE,
    ];
    assert_statement_kinds(
        "nested loops",
        &tokens,
        &[
            (0, C_AST_KIND_FOR_STMT),
            (14, C_AST_KIND_WHILE_STMT),
            (18, C_AST_KIND_DO_STMT),
            (22, C_AST_KIND_WHILE_STMT),
        ],
    );
}

#[test]
fn goto_statement_classifies_on_gpu_and_cpu() {
    let tokens = [
        TOK_IDENTIFIER,
        TOK_COLON,
        TOK_GOTO,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
    ];
    assert_statement_kinds("goto", &tokens, &[(2, C_AST_KIND_GOTO_STMT)]);
}
