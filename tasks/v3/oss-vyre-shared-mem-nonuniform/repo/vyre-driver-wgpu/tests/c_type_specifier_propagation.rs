//! GPU contracts for declaration-local C type-specifier propagation.

#![cfg(feature = "c-parser")]

mod common;
use common::words_to_bytes;

use vyre::ir::Expr;
use vyre::VyreBackend;
use vyre_driver_wgpu::WgpuBackend;
use vyre_libs::parsing::c::lex::tokens::*;
use vyre_libs::parsing::c::parse::declarations::opt_propagate_type_specifiers;

fn run_gpu(tok_types: &[u32], tok_depths: &[u32]) -> Vec<u32> {
    assert_eq!(
        tok_types.len(),
        tok_depths.len(),
        "fixture must provide one depth per token"
    );
    let backend = WgpuBackend::new()
        .expect("Fix: WgpuBackend::new failed on a machine that must have a GPU.");
    let tok_type_bytes = words_to_bytes(tok_types);
    let tok_depth_bytes = words_to_bytes(tok_depths);
    let program = opt_propagate_type_specifiers(
        "tok_types",
        "tok_depths",
        "node_out",
        Expr::u32(tok_types.len() as u32),
    );
    let inputs: Vec<&[u8]> = vec![&tok_type_bytes, &tok_depth_bytes];
    let outputs = backend
        .dispatch_borrowed(&program, &inputs, &Default::default())
        .expect("GPU type-specifier propagation dispatch must succeed");
    assert_eq!(
        outputs.len(),
        1,
        "type-specifier propagation must produce one output buffer"
    );
    outputs[0]
        .chunks_exact(4)
        .map(|chunk| u32::from_le_bytes(chunk.try_into().unwrap()))
        .take(tok_types.len())
        .collect()
}

#[test]
fn comma_separated_declarators_inherit_same_depth_type() {
    let tok_types = [
        TOK_INT,
        TOK_IDENTIFIER,
        TOK_COMMA,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
        TOK_CHAR_KW,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
    ];
    let tok_depths = [0; 8];

    let out = run_gpu(&tok_types, &tok_depths);

    assert_eq!(
        out,
        [
            TOK_INT,
            TOK_INT,
            TOK_INT,
            TOK_INT,
            0,
            TOK_CHAR_KW,
            TOK_CHAR_KW,
            0
        ],
        "`int a, b; char c;` must propagate `int` across the first declarator list \
         and stop at the semicolon before `char c`"
    );
}

#[test]
fn outer_declaration_does_not_leak_across_nested_braces() {
    let tok_types = [
        TOK_INT,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
        TOK_LBRACE,
        TOK_CHAR_KW,
        TOK_IDENTIFIER,
        TOK_COMMA,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
        TOK_RBRACE,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
    ];
    let tok_depths = [0, 0, 0, 0, 1, 1, 1, 1, 1, 0, 0, 0];

    let out = run_gpu(&tok_types, &tok_depths);

    assert_eq!(
        out,
        [
            TOK_INT,
            TOK_INT,
            0,
            0,
            TOK_CHAR_KW,
            TOK_CHAR_KW,
            TOK_CHAR_KW,
            TOK_CHAR_KW,
            0,
            0,
            0,
            0,
        ],
        "same-depth propagation must stop at semicolons/braces and must not leak \
         the outer `int` onto tokens after the nested block"
    );
}

#[test]
fn linux_gnu_type_specifiers_propagate_through_declarators() {
    let tok_types = [
        TOK_GNU_AUTO_TYPE,
        TOK_IDENTIFIER,
        TOK_ASSIGN,
        TOK_INTEGER,
        TOK_SEMICOLON,
        TOK_UNSIGNED,
        TOK_GNU_INT128,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
        TOK_GNU_TYPEOF_UNQUAL,
        TOK_LPAREN,
        TOK_IDENTIFIER,
        TOK_RPAREN,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
    ];
    let tok_depths = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0];

    let out = run_gpu(&tok_types, &tok_depths);

    assert_eq!(out[1], TOK_GNU_AUTO_TYPE, "__auto_type must drive x");
    assert_eq!(out[7], TOK_GNU_INT128, "__int128 must drive wide");
    assert_eq!(
        out[13], TOK_GNU_TYPEOF_UNQUAL,
        "typeof_unqual(...) must drive the following declarator"
    );
}
