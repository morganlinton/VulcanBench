//! Integration contracts for bitfield and abstract-declarator type propagation.

#![cfg(feature = "c-parser")]

mod common;
use common::words_to_bytes;

#[path = "c_ast_gpu_parity_support/mod.rs"]
mod c_ast_gpu_parity_support;

use c_ast_gpu_parity_support::dispatch_gpu_program;
use vyre::ir::Expr;
use vyre_libs::parsing::c::lex::tokens::*;
use vyre_libs::parsing::c::parse::declarations::opt_propagate_type_specifiers;

fn run_type_propagation(tok_types: &[u32], tok_depths: &[u32]) -> Vec<u32> {
    assert_eq!(tok_types.len(), tok_depths.len());
    let program = opt_propagate_type_specifiers(
        "tok_types",
        "tok_depths",
        "node_out",
        Expr::u32(tok_types.len() as u32),
    );
    let tok_bytes = words_to_bytes(tok_types);
    let depth_bytes = words_to_bytes(tok_depths);
    let outputs = dispatch_gpu_program(
        "GPU C type propagation",
        program,
        vec![tok_bytes, depth_bytes],
    );
    assert_eq!(outputs.len(), 1);
    outputs[0]
        .chunks_exact(4)
        .map(|chunk| u32::from_le_bytes(chunk.try_into().unwrap()))
        .take(tok_types.len())
        .collect()
}

#[test]
fn abstract_function_pointer_declarator_keeps_type_inside_nested_suffixes() {
    let tok_types = [
        TOK_LPAREN,
        TOK_CONST,
        TOK_INT,
        TOK_LPAREN,
        TOK_STAR,
        TOK_RPAREN,
        TOK_LPAREN,
        TOK_VOID,
        TOK_RPAREN,
        TOK_RPAREN,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
    ];
    let tok_depths = [0, 1, 1, 1, 2, 2, 1, 2, 2, 1, 0, 0];

    let out = run_type_propagation(&tok_types, &tok_depths);

    assert_eq!(out[4], TOK_INT, "`*` in abstract function pointer sees int");
    assert_eq!(
        out[6], TOK_INT,
        "function suffix keeps abstract type context"
    );
    assert_eq!(
        out[7], TOK_VOID,
        "parameter type remains local to parameter"
    );
    assert_eq!(
        out[10], 0,
        "cast operand must not inherit type from type name"
    );
}

#[test]
fn parameter_array_static_restrict_qualifiers_keep_the_parameter_type() {
    let tok_types = [
        TOK_VOID,
        TOK_IDENTIFIER,
        TOK_LPAREN,
        TOK_INT,
        TOK_IDENTIFIER,
        TOK_LBRACKET,
        TOK_STATIC,
        TOK_RESTRICT,
        TOK_CONST,
        TOK_INTEGER,
        TOK_RBRACKET,
        TOK_RPAREN,
        TOK_SEMICOLON,
    ];
    let tok_depths = [0, 0, 0, 1, 1, 1, 2, 2, 2, 2, 1, 0, 0];

    let out = run_type_propagation(&tok_types, &tok_depths);

    assert_eq!(out[4], TOK_INT, "array parameter declarator inherits int");
    assert_eq!(out[5], TOK_INT, "array suffix inherits int");
    assert_eq!(
        out[6], TOK_INT,
        "`static` in array bounds stays in int context"
    );
    assert_eq!(
        out[7], TOK_INT,
        "`restrict` in array bounds stays in int context"
    );
    assert_eq!(
        out[8], TOK_INT,
        "`const` in array bounds stays in int context"
    );
    assert_eq!(out[9], TOK_INT, "array bound expression keeps int context");
}

#[test]
fn vla_and_multidimensional_array_qualifiers_keep_declarator_type() {
    let tok_types = [
        TOK_INT,
        TOK_IDENTIFIER,
        TOK_LBRACKET,
        TOK_STATIC,
        TOK_INTEGER,
        TOK_RBRACKET,
        TOK_LBRACKET,
        TOK_VOLATILE,
        TOK_STAR,
        TOK_RBRACKET,
        TOK_SEMICOLON,
    ];
    let tok_depths = [0, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0];

    let out = run_type_propagation(&tok_types, &tok_depths);

    assert_eq!(out[2], TOK_INT, "first array suffix inherits int");
    assert_eq!(out[3], TOK_INT, "`static` VLA bound keeps int context");
    assert_eq!(out[6], TOK_INT, "second array suffix inherits int");
    assert_eq!(out[7], TOK_INT, "`volatile` VLA marker keeps int context");
    assert_eq!(out[8], TOK_INT, "`*` VLA marker keeps int context");
}

#[test]
fn alignas_and_typeof_complex_declarators_preserve_base_type_context() {
    let alignas_tokens = [
        TOK_ALIGNAS,
        TOK_LPAREN,
        TOK_INTEGER,
        TOK_RPAREN,
        TOK_INT,
        TOK_LPAREN,
        TOK_STAR,
        TOK_IDENTIFIER,
        TOK_RPAREN,
        TOK_LBRACKET,
        TOK_INTEGER,
        TOK_RBRACKET,
        TOK_SEMICOLON,
    ];
    let alignas_depths = [0, 0, 1, 0, 0, 0, 1, 1, 0, 0, 1, 0, 0];
    let alignas_out = run_type_propagation(&alignas_tokens, &alignas_depths);

    assert_eq!(alignas_out[6], TOK_INT, "`*p` inherits int after _Alignas");
    assert_eq!(
        alignas_out[9], TOK_INT,
        "pointer-to-array suffix inherits int"
    );

    let typeof_tokens = [
        TOK_GNU_TYPEOF,
        TOK_LPAREN,
        TOK_INT,
        TOK_LPAREN,
        TOK_STAR,
        TOK_RPAREN,
        TOK_LBRACKET,
        TOK_INTEGER,
        TOK_RBRACKET,
        TOK_RPAREN,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
    ];
    let typeof_depths = [0, 0, 1, 1, 2, 2, 1, 2, 1, 0, 0, 0];
    let typeof_out = run_type_propagation(&typeof_tokens, &typeof_depths);

    assert_eq!(
        typeof_out[10], TOK_GNU_TYPEOF,
        "`typeof(int (*)[4]) x` exposes typeof as x's declaration type"
    );
}

#[test]
fn anonymous_bitfield_width_does_not_leak_past_the_member_semicolon() {
    let tok_types = [
        TOK_STRUCT,
        TOK_LBRACE,
        TOK_UNSIGNED,
        TOK_INT,
        TOK_COLON,
        TOK_INTEGER,
        TOK_SEMICOLON,
        TOK_CHAR_KW,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
        TOK_RBRACE,
        TOK_SEMICOLON,
    ];
    let tok_depths = [0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0];

    let out = run_type_propagation(&tok_types, &tok_depths);

    assert_eq!(
        out[4], TOK_INT,
        "anonymous bitfield colon stays in int context"
    );
    assert_eq!(
        out[5], TOK_INT,
        "anonymous bitfield width stays in int context"
    );
    assert_eq!(
        out[7], TOK_CHAR_KW,
        "next member starts a fresh char context"
    );
    assert_eq!(out[8], TOK_CHAR_KW, "next member declarator inherits char");
    assert_eq!(
        out[10], TOK_STRUCT,
        "record close returns to aggregate context"
    );
    assert_eq!(
        out[11], 0,
        "declaration semicolon must not keep member type context"
    );
}
