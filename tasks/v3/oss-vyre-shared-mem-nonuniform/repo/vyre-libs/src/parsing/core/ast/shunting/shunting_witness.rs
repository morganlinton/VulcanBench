use crate::parsing::c::lex::tokens::TOK_IDENTIFIER;
use crate::parsing::core::ast::node::AST_VAR;
use vyre::ir::Expr;

use super::{ast_shunting_yard, pack_u32, MAX_TOK_SCAN, OP_ID};

inventory::submit! {
    crate::harness::OpEntry {
        id: OP_ID,
        build: || ast_shunting_yard(
            "tok_types", "statements", Expr::u32(100),
            "out_ast_nodes", "out_ast_count", "out_statement_roots",
            "scratch_val_stack", "scratch_op_stack"
        ),
        test_inputs: Some(|| vec![vec![
            shunting_token_fixture(),
            shunting_statement_fixture(),
            vec![0u8; MAX_TOK_SCAN as usize * 4 * 4],
            vec![0u8; 4],
            vec![0u8; 100 * 4],
            vec![0u8; 6_400 * 4],
            vec![0u8; 6_400 * 4],
        ]]),
        expected_output: Some(shunting_expected_output),
        category: Some("parsing"),
    }
}

fn shunting_token_fixture() -> Vec<u8> {
    let mut tokens = vec![0u32; MAX_TOK_SCAN as usize];
    tokens[0] = TOK_IDENTIFIER;
    pack_u32(&tokens)
}

fn shunting_statement_fixture() -> Vec<u8> {
    let mut statements = vec![0u32; 200];
    statements[1] = 1;
    pack_u32(&statements)
}

fn shunting_expected_output() -> Vec<Vec<Vec<u8>>> {
    let mut ast_nodes = vec![0u32; MAX_TOK_SCAN as usize * 4];
    ast_nodes[0..4].copy_from_slice(&[AST_VAR, u32::MAX, u32::MAX, 0]);
    let mut roots = vec![u32::MAX; 100];
    roots[0] = 0;
    vec![vec![
        pack_u32(&ast_nodes),
        pack_u32(&[4]),
        pack_u32(&roots),
        Vec::new(),
        Vec::new(),
    ]]
}
