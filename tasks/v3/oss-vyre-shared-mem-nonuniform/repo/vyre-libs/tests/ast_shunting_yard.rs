//! Reference-oracle tests for C expression AST shunting-yard lowering.

#![cfg(feature = "c-parser")]
#![allow(deprecated)]
mod common;

use common::bytes_from_words;
use common::words_from_bytes;
use vyre::ir::Expr;
use vyre_libs::parsing::c::lex::tokens::*;
use vyre_libs::parsing::core::ast::node::*;
use vyre_libs::parsing::core::ast::shunting::ast_shunting_yard_with_capacity;
use vyre_reference::value::Value;

const MAX_TOK_SCAN: usize = 65_536;
const STACK_SLOTS: usize = 64;
const SENTINEL: u32 = u32::MAX;

struct ParsedAst {
    ast_words: Vec<u32>,
    count_words: u32,
    root: u32,
}

fn parse_tokens(tokens: &[u32]) -> ParsedAst {
    assert!(tokens.len() <= MAX_TOK_SCAN);
    let token_capacity = tokens.len().max(1);
    let mut tok_words = vec![0u32; token_capacity];
    tok_words[..tokens.len()].copy_from_slice(tokens);
    let statements = [
        0u32,
        u32::try_from(tokens.len()).expect("test token count fits u32"),
    ];

    let inputs = [
        bytes_from_words(&tok_words),
        bytes_from_words(&statements),
        vec![0u8; token_capacity * 4 * 4],
        vec![0u8; 4],
        vec![0u8; 4],
        vec![0u8; STACK_SLOTS * 4],
        vec![0u8; STACK_SLOTS * 4],
    ];
    let values = inputs
        .iter()
        .map(|bytes| Value::Bytes(bytes.as_slice().into()))
        .collect::<Vec<_>>();

    let program = ast_shunting_yard_with_capacity(
        "tok_types",
        "statements",
        Expr::u32(1),
        "out_ast_nodes",
        "out_ast_count",
        "out_statement_roots",
        "scratch_val_stack",
        "scratch_op_stack",
        u32::try_from(token_capacity).expect("test token capacity fits u32"),
        1,
    );
    let outputs = vyre_reference::reference_eval(&program, &values)
        .expect("shunting-yard parser must execute under the reference oracle");

    ParsedAst {
        ast_words: words_from_bytes(&outputs[0].to_bytes()),
        count_words: words_from_bytes(&outputs[1].to_bytes())[0],
        root: words_from_bytes(&outputs[2].to_bytes())[0],
    }
}

fn node(ast: &ParsedAst, offset: u32) -> [u32; 4] {
    let base = usize::try_from(offset).expect("node offset fits usize");
    ast.ast_words[base..base + 4]
        .try_into()
        .expect("node has four words")
}

#[test]
fn multiplication_binds_tighter_than_addition() {
    let ast = parse_tokens(&[
        TOK_IDENTIFIER,
        TOK_PLUS,
        TOK_IDENTIFIER,
        TOK_STAR,
        TOK_IDENTIFIER,
    ]);

    assert_eq!(ast.count_words, 20);
    assert_eq!(ast.root, 16);
    assert_eq!(node(&ast, 12), [AST_MUL, 4, 8, SENTINEL]);
    assert_eq!(node(&ast, 16), [AST_ADD, 0, 12, SENTINEL]);
}

#[test]
fn parentheses_override_binary_precedence() {
    let ast = parse_tokens(&[
        TOK_LPAREN,
        TOK_IDENTIFIER,
        TOK_PLUS,
        TOK_IDENTIFIER,
        TOK_RPAREN,
        TOK_STAR,
        TOK_IDENTIFIER,
    ]);

    assert_eq!(ast.count_words, 20);
    assert_eq!(ast.root, 16);
    assert_eq!(node(&ast, 8), [AST_ADD, 0, 4, SENTINEL]);
    assert_eq!(node(&ast, 16), [AST_MUL, 8, 12, SENTINEL]);
}

#[test]
fn assignment_is_right_associative() {
    let ast = parse_tokens(&[
        TOK_IDENTIFIER,
        TOK_ASSIGN,
        TOK_IDENTIFIER,
        TOK_ASSIGN,
        TOK_IDENTIFIER,
    ]);

    assert_eq!(ast.count_words, 20);
    assert_eq!(ast.root, 16);
    assert_eq!(node(&ast, 12), [AST_ASSIGN, 4, 8, SENTINEL]);
    assert_eq!(node(&ast, 16), [AST_ASSIGN, 0, 12, SENTINEL]);
}
