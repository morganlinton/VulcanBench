//! High-quality integration tests for GNU C and Linux-kernel-shaped constructs in
//! the C AST/VAST parser.
//!
//! Coverage:
//!   - GNU extended asm (templates, input/output operands, clobbers, goto labels)
//!   - GNU attributes: cleanup, alias, aligned, section
//!   - computed goto (`&&label`)
//!   - `__builtin_*` forms: expect, constant_p, choose_expr,
//!     types_compatible_p, plus unrecognized builtins as generic calls
//!   - `_Atomic` qualifier and type specifier
//!   - `typeof_unqual` / `__typeof_unqual__`
//!   - `__auto_type`
//!   - `__int128`
//!   - declarator ambiguity (pointer vs array precedence)
//!   - C99 for-loop declarations
//!   - abstract function pointers in parameter position
//!   - Linux-kernel-shaped declarations (attributes + typeof + function pointers)
//!
//! Tests assert the *intended contract* (distinct VAST kinds, correct tree
//! parentage, no collapse into generic CALL/BINARY) rather than snapshotting
//! current output.
//!
//! A missing GPU adapter is a configuration failure; tests do not skip.

#![cfg(feature = "c-parser")]
#![allow(deprecated)]
mod c_ast_gpu_parity_support;

use c_ast_gpu_parity_support::{
    assert_full_pipeline_parity, build_fixture, row_indices, word_at, Fixture, FixtureToken,
    VAST_STRIDE_U32,
};
use vyre_libs::parsing::c::lex::tokens::*;
use vyre_libs::parsing::c::parse::gnu_builtins::try_classify_gnu_builtin_name;
use vyre_libs::parsing::c::parse::vast::{
    reference_c11_annotate_typedef_names, reference_c11_build_vast_nodes,
    reference_c11_classify_vast_node_kinds, C_AST_KIND_ARRAY_DECL, C_AST_KIND_ASM_CLOBBERS_LIST,
    C_AST_KIND_ASM_GOTO_LABELS, C_AST_KIND_ASM_INPUT_OPERAND, C_AST_KIND_ASM_OUTPUT_OPERAND,
    C_AST_KIND_ASM_TEMPLATE, C_AST_KIND_ASSIGN_EXPR, C_AST_KIND_ATTRIBUTE_ALIAS,
    C_AST_KIND_ATTRIBUTE_ALIGNED, C_AST_KIND_ATTRIBUTE_CLEANUP, C_AST_KIND_ATTRIBUTE_SECTION,
    C_AST_KIND_BUILTIN_CHOOSE_EXPR, C_AST_KIND_BUILTIN_CONSTANT_P_EXPR,
    C_AST_KIND_BUILTIN_EXPECT_EXPR, C_AST_KIND_BUILTIN_TYPES_COMPATIBLE_P_EXPR,
    C_AST_KIND_CAST_EXPR, C_AST_KIND_FOR_STMT, C_AST_KIND_FUNCTION_DECLARATOR,
    C_AST_KIND_FUNCTION_DEFINITION, C_AST_KIND_GNU_ATTRIBUTE, C_AST_KIND_GNU_LABEL_ADDRESS_EXPR,
    C_AST_KIND_GOTO_STMT, C_AST_KIND_INLINE_ASM, C_AST_KIND_LABEL_STMT, C_AST_KIND_POINTER_DECL,
};
use vyre_primitives::predicate::node_kind;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn classify(fix: &Fixture) -> Vec<u8> {
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    reference_c11_classify_vast_node_kinds(&annotated)
}

fn parent_of(rows: &[u8], idx: usize) -> u32 {
    word_at(rows, idx * VAST_STRIDE_U32 + 1)
}

// ---------------------------------------------------------------------------
// 1. GNU asm
// ---------------------------------------------------------------------------

fn fixture_asm_goto_with_labels() -> Fixture {
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

fn fixture_asm_extended_io_clobbers() -> Fixture {
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
        FixtureToken::new("\"memory\"", TOK_STRING),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

// Flat include  -  parts share fixture defs, per-part `mod {}` wrappers
// hid them from each other.
include!("__split/c_ast_gnu_and_kernel_construct_integration_part1.rs");
include!("__split/c_ast_gnu_and_kernel_construct_integration_part2.rs");
