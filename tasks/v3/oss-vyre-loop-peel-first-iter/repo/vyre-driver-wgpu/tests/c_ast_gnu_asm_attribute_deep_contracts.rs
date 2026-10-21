//! Deep GPU/CPU parity contracts for extended GNU asm forms and GNU attributes.
//!
//! Covers:
//!   - asm volatile, asm goto, asm volatile goto with outputs/inputs/clobbers/labels
//!   - named asm operands (`[name] "constraint" (expr)`)
//!   - asm aliases on declarations (`int foo asm("bar") = 1;`)
//!   - GNU attributes: section, weak, alias, aligned, packed, cleanup,
//!     constructor, destructor, mode, visibility, naked, used, unused
//!
//! A missing GPU adapter is a configuration failure; tests panic loudly.

#![cfg(feature = "c-parser")]
#![allow(deprecated)]
mod c_ast_gpu_parity_support;

use c_ast_gpu_parity_support::{
    assert_full_pipeline_parity, c_fixture, classify, row_indices, word_at, Fixture,
    VAST_STRIDE_U32,
};
use vyre_libs::parsing::c::lex::tokens::*;
use vyre_libs::parsing::c::parse::vast::{
    C_AST_KIND_ASM_CLOBBERS_LIST, C_AST_KIND_ASM_GOTO_LABELS, C_AST_KIND_ASM_INPUT_OPERAND,
    C_AST_KIND_ASM_OUTPUT_OPERAND, C_AST_KIND_ASM_QUALIFIER, C_AST_KIND_ASM_TEMPLATE,
    C_AST_KIND_ATTRIBUTE_ALIAS, C_AST_KIND_ATTRIBUTE_ALIGNED, C_AST_KIND_ATTRIBUTE_CLEANUP,
    C_AST_KIND_ATTRIBUTE_CONSTRUCTOR, C_AST_KIND_ATTRIBUTE_DESTRUCTOR, C_AST_KIND_ATTRIBUTE_MODE,
    C_AST_KIND_ATTRIBUTE_NAKED, C_AST_KIND_ATTRIBUTE_PACKED, C_AST_KIND_ATTRIBUTE_SECTION,
    C_AST_KIND_ATTRIBUTE_UNUSED, C_AST_KIND_ATTRIBUTE_USED, C_AST_KIND_ATTRIBUTE_VISIBILITY,
    C_AST_KIND_ATTRIBUTE_WEAK, C_AST_KIND_FIELD_DECL, C_AST_KIND_FUNCTION_DEFINITION,
    C_AST_KIND_GNU_ATTRIBUTE, C_AST_KIND_GOTO_STMT, C_AST_KIND_INLINE_ASM,
};
use vyre_primitives::predicate::node_kind;

// ---------------------------------------------------------------------------
// Fixture builders  -  extended GNU asm
// ---------------------------------------------------------------------------

fn fixture_asm_goto_with_outputs_inputs_clobbers_labels() -> Fixture {
    c_fixture![
        ("asm", TOK_IDENTIFIER),
        ("goto", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("\"jmp %l0\"", TOK_STRING),
        (":", TOK_COLON),
        ("\"=r\"", TOK_STRING),
        ("(", TOK_LPAREN),
        ("out", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        (":", TOK_COLON),
        ("\"r\"", TOK_STRING),
        ("(", TOK_LPAREN),
        ("in", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        (":", TOK_COLON),
        ("\"memory\"", TOK_STRING),
        (",", TOK_COMMA),
        ("\"cc\"", TOK_STRING),
        (":", TOK_COLON),
        ("fail", TOK_IDENTIFIER),
        (",", TOK_COMMA),
        ("ok", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        (";", TOK_SEMICOLON),
    ]
}

fn fixture_asm_volatile_goto_with_outputs_inputs_clobbers_labels() -> Fixture {
    c_fixture![
        ("asm", TOK_IDENTIFIER),
        ("volatile", TOK_IDENTIFIER),
        ("goto", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("\"jmp %l0\"", TOK_STRING),
        (":", TOK_COLON),
        ("\"=r\"", TOK_STRING),
        ("(", TOK_LPAREN),
        ("out", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        (":", TOK_COLON),
        ("\"r\"", TOK_STRING),
        ("(", TOK_LPAREN),
        ("in", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        (":", TOK_COLON),
        ("\"memory\"", TOK_STRING),
        (":", TOK_COLON),
        ("fail", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        (";", TOK_SEMICOLON),
    ]
}

fn fixture_asm_volatile_with_outputs_inputs_clobbers() -> Fixture {
    c_fixture![
        ("asm", TOK_IDENTIFIER),
        ("volatile", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("\"mov %1, %0\"", TOK_STRING),
        (":", TOK_COLON),
        ("\"=r\"", TOK_STRING),
        ("(", TOK_LPAREN),
        ("out", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        (":", TOK_COLON),
        ("\"r\"", TOK_STRING),
        ("(", TOK_LPAREN),
        ("in", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        (":", TOK_COLON),
        ("\"memory\"", TOK_STRING),
        (",", TOK_COMMA),
        ("\"cc\"", TOK_STRING),
        (")", TOK_RPAREN),
        (";", TOK_SEMICOLON),
    ]
}

fn fixture_asm_named_operands() -> Fixture {
    c_fixture![
        ("asm", TOK_IDENTIFIER),
        ("volatile", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("\"mov %[src], %[dst]\"", TOK_STRING),
        (":", TOK_COLON),
        ("[", TOK_LBRACKET),
        ("dst", TOK_IDENTIFIER),
        ("]", TOK_RBRACKET),
        ("\"=r\"", TOK_STRING),
        ("(", TOK_LPAREN),
        ("out", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        (":", TOK_COLON),
        ("[", TOK_LBRACKET),
        ("src", TOK_IDENTIFIER),
        ("]", TOK_RBRACKET),
        ("\"r\"", TOK_STRING),
        ("(", TOK_LPAREN),
        ("in", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        (")", TOK_RPAREN),
        (";", TOK_SEMICOLON),
    ]
}

fn fixture_asm_alias_declaration() -> Fixture {
    c_fixture![
        ("int", TOK_IDENTIFIER),
        ("foo", TOK_IDENTIFIER),
        ("asm", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("\"bar\"", TOK_STRING),
        (")", TOK_RPAREN),
        ("=", TOK_ASSIGN),
        ("1", TOK_INTEGER),
        (";", TOK_SEMICOLON),
    ]
}

// ---------------------------------------------------------------------------
// Fixture builders  -  GNU attribute-specific kinds (deep coverage)
// ---------------------------------------------------------------------------

fn fixture_attribute_cleanup() -> Fixture {
    c_fixture![
        ("int", TOK_IDENTIFIER),
        ("x", TOK_IDENTIFIER),
        ("__attribute__", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("(", TOK_LPAREN),
        ("cleanup", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("clean", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        (")", TOK_RPAREN),
        (")", TOK_RPAREN),
        (";", TOK_SEMICOLON),
    ]
}

fn fixture_attribute_constructor() -> Fixture {
    c_fixture![
        ("__attribute__", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("(", TOK_LPAREN),
        ("constructor", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        (")", TOK_RPAREN),
        ("void", TOK_IDENTIFIER),
        ("init", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("void", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        ("{", TOK_LBRACE),
        ("}", TOK_RBRACE),
    ]
}

fn fixture_attribute_destructor() -> Fixture {
    c_fixture![
        ("__attribute__", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("(", TOK_LPAREN),
        ("destructor", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        (")", TOK_RPAREN),
        ("void", TOK_IDENTIFIER),
        ("fini", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("void", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        ("{", TOK_LBRACE),
        ("}", TOK_RBRACE),
    ]
}

fn fixture_attribute_mode() -> Fixture {
    c_fixture![
        ("__attribute__", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("(", TOK_LPAREN),
        ("mode", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("SI", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        (")", TOK_RPAREN),
        (")", TOK_RPAREN),
        ("int", TOK_IDENTIFIER),
        ("x", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
    ]
}

fn fixture_attribute_packed_struct() -> Fixture {
    c_fixture![
        ("struct", TOK_IDENTIFIER),
        ("__attribute__", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("(", TOK_LPAREN),
        ("packed", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        (")", TOK_RPAREN),
        ("S", TOK_IDENTIFIER),
        ("{", TOK_LBRACE),
        ("int", TOK_IDENTIFIER),
        ("x", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
        ("}", TOK_RBRACE),
        (";", TOK_SEMICOLON),
    ]
}

fn fixture_attribute_combined_variable() -> Fixture {
    c_fixture![
        ("__attribute__", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("(", TOK_LPAREN),
        ("section", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("\".text\"", TOK_STRING),
        (")", TOK_RPAREN),
        (",", TOK_COMMA),
        ("weak", TOK_IDENTIFIER),
        (",", TOK_COMMA),
        ("alias", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("\"real\"", TOK_STRING),
        (")", TOK_RPAREN),
        (",", TOK_COMMA),
        ("aligned", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("8", TOK_INTEGER),
        (")", TOK_RPAREN),
        (",", TOK_COMMA),
        ("used", TOK_IDENTIFIER),
        (",", TOK_COMMA),
        ("unused", TOK_IDENTIFIER),
        (",", TOK_COMMA),
        ("naked", TOK_IDENTIFIER),
        (",", TOK_COMMA),
        ("visibility", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("\"hidden\"", TOK_STRING),
        (")", TOK_RPAREN),
        (")", TOK_RPAREN),
        (")", TOK_RPAREN),
        ("int", TOK_IDENTIFIER),
        ("combo", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
    ]
}

fn fixture_non_attribute_identifiers() -> Fixture {
    c_fixture![
        ("void", TOK_IDENTIFIER),
        ("cleanup", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("int", TOK_IDENTIFIER),
        ("section", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        (";", TOK_SEMICOLON),
    ]
}

// ---------------------------------------------------------------------------
// Tests  -  extended GNU asm forms
// ---------------------------------------------------------------------------

mod c_ast_gnu_asm_attribute_deep_contracts_part1 {

    include!("__split/c_ast_gnu_asm_attribute_deep_contracts_part1.rs");
}
mod c_ast_gnu_asm_attribute_deep_contracts_part2 {
    include!("__split/c_ast_gnu_asm_attribute_deep_contracts_part2.rs");
}
