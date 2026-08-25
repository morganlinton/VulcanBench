//! GPU/CPU parity end-to-end tests for GNU C extensions common in kernel code.
//!
//! Extensions under test:
//!   - `__attribute__` before/after declarators
//!   - statement expressions `({ ... })`
//!   - `typeof` / `__typeof__` as GNU type-name prefixes
//!   - inline asm (`asm`, `__asm__`)
//!   - labels-as-values `&&label`
//!
//! Every test fails loudly if a GPU adapter cannot be acquired; there is no
//! graceful skip path.

#![cfg(feature = "c-parser")]
#![allow(deprecated)]

use c_grammar_gen::lex_c11_max_munch_kinds;
use vyre_libs::parsing::c::lex::tokens::*;
use vyre_libs::parsing::c::parse::vast::{
    reference_c11_annotate_typedef_names, reference_c11_build_vast_nodes,
    reference_c11_classify_vast_node_kinds, C_AST_KIND_ASM_CLOBBERS_LIST,
    C_AST_KIND_ASM_GOTO_LABELS, C_AST_KIND_ASM_INPUT_OPERAND, C_AST_KIND_ASM_OUTPUT_OPERAND,
    C_AST_KIND_ASM_TEMPLATE, C_AST_KIND_ASSIGN_EXPR, C_AST_KIND_ATTRIBUTE_ALIAS,
    C_AST_KIND_ATTRIBUTE_ALIGNED, C_AST_KIND_ATTRIBUTE_NAKED, C_AST_KIND_ATTRIBUTE_SECTION,
    C_AST_KIND_ATTRIBUTE_UNUSED, C_AST_KIND_ATTRIBUTE_USED, C_AST_KIND_ATTRIBUTE_VISIBILITY,
    C_AST_KIND_ATTRIBUTE_WEAK, C_AST_KIND_BUILTIN_CHOOSE_EXPR, C_AST_KIND_BUILTIN_CONSTANT_P_EXPR,
    C_AST_KIND_BUILTIN_TYPES_COMPATIBLE_P_EXPR, C_AST_KIND_FIELD_DECL,
    C_AST_KIND_FUNCTION_DEFINITION, C_AST_KIND_GENERIC_SELECTION_EXPR, C_AST_KIND_GNU_ATTRIBUTE,
    C_AST_KIND_GNU_LABEL_ADDRESS_EXPR, C_AST_KIND_GOTO_STMT, C_AST_KIND_INLINE_ASM,
    C_AST_KIND_RANGE_DESIGNATOR_EXPR,
};
use vyre_primitives::predicate::node_kind;

mod c_ast_gpu_parity_support;
use c_ast_gpu_parity_support::{
    assert_full_pipeline_parity, c_fixture, row_indices, word_at, Fixture, VAST_STRIDE_U32,
};

// ---------------------------------------------------------------------------
// Fixture builders
// ---------------------------------------------------------------------------

fn fixture_attribute_before_declarator() -> Fixture {
    c_fixture![
        ("__attribute__", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("(", TOK_LPAREN),
        ("noinline", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        (")", TOK_RPAREN),
        ("int", TOK_IDENTIFIER),
        ("foo", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("void", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        ("{", TOK_LBRACE),
        ("return", TOK_IDENTIFIER),
        ("0", TOK_INTEGER),
        (";", TOK_SEMICOLON),
        ("}", TOK_RBRACE),
    ]
}

fn fixture_attribute_after_declarator() -> Fixture {
    c_fixture![
        ("int", TOK_IDENTIFIER),
        ("bar", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("void", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        ("__attribute__", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("(", TOK_LPAREN),
        ("noreturn", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        (")", TOK_RPAREN),
        (";", TOK_SEMICOLON),
    ]
}

fn fixture_statement_expression() -> Fixture {
    c_fixture![
        ("int", TOK_IDENTIFIER),
        ("x", TOK_IDENTIFIER),
        ("=", TOK_ASSIGN),
        ("(", TOK_LPAREN),
        ("{", TOK_LBRACE),
        ("int", TOK_IDENTIFIER),
        ("y", TOK_IDENTIFIER),
        ("=", TOK_ASSIGN),
        ("1", TOK_INTEGER),
        (";", TOK_SEMICOLON),
        ("y", TOK_IDENTIFIER),
        ("+", TOK_PLUS),
        ("2", TOK_INTEGER),
        (";", TOK_SEMICOLON),
        ("}", TOK_RBRACE),
        (")", TOK_RPAREN),
        (";", TOK_SEMICOLON),
    ]
}

fn fixture_typeof() -> Fixture {
    c_fixture![
        ("typeof", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("int", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        (";", TOK_SEMICOLON),
        ("__typeof__", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("int", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        (";", TOK_SEMICOLON),
    ]
}

fn fixture_inline_asm() -> Fixture {
    c_fixture![
        ("asm", TOK_IDENTIFIER),
        ("volatile", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("\"nop\"", TOK_STRING),
        (":", TOK_COLON),
        (":", TOK_COLON),
        (":", TOK_COLON),
        ("\"memory\"", TOK_STRING),
        (")", TOK_RPAREN),
        (";", TOK_SEMICOLON),
    ]
}

fn fixture_extended_asm_operands() -> Fixture {
    c_fixture![
        ("asm", TOK_IDENTIFIER),
        ("volatile", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("\"mov %1, %0\"", TOK_STRING),
        (":", TOK_COLON),
        ("\"=r\"", TOK_STRING),
        ("(", TOK_LPAREN),
        ("result", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        (":", TOK_COLON),
        ("\"r\"", TOK_STRING),
        ("(", TOK_LPAREN),
        ("input", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        (":", TOK_COLON),
        ("\"memory\"", TOK_STRING),
        (")", TOK_RPAREN),
        (";", TOK_SEMICOLON),
    ]
}

fn fixture_asm_goto() -> Fixture {
    c_fixture![
        ("asm", TOK_IDENTIFIER),
        ("goto", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("\"jmp %l0\"", TOK_STRING),
        (":", TOK_COLON),
        (":", TOK_COLON),
        (":", TOK_COLON),
        (":", TOK_COLON),
        ("error", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        (";", TOK_SEMICOLON),
    ]
}

fn fixture_labels_as_values() -> Fixture {
    c_fixture![
        ("void", TOK_IDENTIFIER),
        ("*", TOK_STAR),
        ("p", TOK_IDENTIFIER),
        ("=", TOK_ASSIGN),
        ("&&", TOK_AND),
        ("label", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
    ]
}

fn fixture_extension_statement_expression() -> Fixture {
    c_fixture![
        ("__extension__", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("{", TOK_LBRACE),
        ("int", TOK_IDENTIFIER),
        ("x", TOK_IDENTIFIER),
        ("=", TOK_ASSIGN),
        ("1", TOK_INTEGER),
        (";", TOK_SEMICOLON),
        ("x", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
        ("}", TOK_RBRACE),
        (")", TOK_RPAREN),
        (";", TOK_SEMICOLON),
    ]
}

fn fixture_extension_declaration() -> Fixture {
    c_fixture![
        ("__extension__", TOK_IDENTIFIER),
        ("int", TOK_IDENTIFIER),
        ("y", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
    ]
}

fn fixture_builtin_and_generic_expressions() -> Fixture {
    c_fixture![
        ("int", TOK_IDENTIFIER),
        ("x", TOK_IDENTIFIER),
        ("=", TOK_ASSIGN),
        ("__builtin_constant_p", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("n", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        ("?", TOK_QUESTION),
        ("1", TOK_INTEGER),
        (":", TOK_COLON),
        ("0", TOK_INTEGER),
        (";", TOK_SEMICOLON),
        ("int", TOK_IDENTIFIER),
        ("y", TOK_IDENTIFIER),
        ("=", TOK_ASSIGN),
        ("__builtin_choose_expr", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("1", TOK_INTEGER),
        (",", TOK_COMMA),
        ("2", TOK_INTEGER),
        (",", TOK_COMMA),
        ("3", TOK_INTEGER),
        (")", TOK_RPAREN),
        (";", TOK_SEMICOLON),
        ("int", TOK_IDENTIFIER),
        ("z", TOK_IDENTIFIER),
        ("=", TOK_ASSIGN),
        ("__builtin_types_compatible_p", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("int", TOK_IDENTIFIER),
        (",", TOK_COMMA),
        ("long", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        (";", TOK_SEMICOLON),
        ("int", TOK_IDENTIFIER),
        ("g", TOK_IDENTIFIER),
        ("=", TOK_ASSIGN),
        ("_Generic", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("x", TOK_IDENTIFIER),
        (",", TOK_COMMA),
        ("int", TOK_IDENTIFIER),
        (":", TOK_COLON),
        ("1", TOK_INTEGER),
        (",", TOK_COMMA),
        ("default", TOK_IDENTIFIER),
        (":", TOK_COLON),
        ("0", TOK_INTEGER),
        (")", TOK_RPAREN),
        (";", TOK_SEMICOLON),
    ]
}

fn fixture_range_designator() -> Fixture {
    c_fixture![
        ("int", TOK_IDENTIFIER),
        ("arr", TOK_IDENTIFIER),
        ("[", TOK_LBRACKET),
        ("]", TOK_RBRACKET),
        ("=", TOK_ASSIGN),
        ("{", TOK_LBRACE),
        ("[", TOK_LBRACKET),
        ("0", TOK_INTEGER),
        ("...", TOK_ELLIPSIS),
        ("3", TOK_INTEGER),
        ("]", TOK_RBRACKET),
        ("=", TOK_ASSIGN),
        ("7", TOK_INTEGER),
        ("}", TOK_RBRACE),
        (";", TOK_SEMICOLON),
    ]
}

fn fixture_packed_struct_attribute() -> Fixture {
    c_fixture![
        ("struct", TOK_IDENTIFIER),
        ("__attribute__", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("(", TOK_LPAREN),
        ("packed", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        (")", TOK_RPAREN),
        ("foo", TOK_IDENTIFIER),
        ("{", TOK_LBRACE),
        ("int", TOK_IDENTIFIER),
        ("x", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
        ("}", TOK_RBRACE),
        (";", TOK_SEMICOLON),
    ]
}

fn fixture_attribute_names() -> Fixture {
    c_fixture![
        ("__attribute__", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("(", TOK_LPAREN),
        ("section", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("\".text.fast\"", TOK_STRING),
        (")", TOK_RPAREN),
        (",", TOK_COMMA),
        ("weak", TOK_IDENTIFIER),
        (",", TOK_COMMA),
        ("alias", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("\"real_fn\"", TOK_STRING),
        (")", TOK_RPAREN),
        (",", TOK_COMMA),
        ("aligned", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("16", TOK_INTEGER),
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
        ("\"default\"", TOK_STRING),
        (")", TOK_RPAREN),
        (")", TOK_RPAREN),
        (")", TOK_RPAREN),
        ("int", TOK_IDENTIFIER),
        ("x", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
    ]
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

mod c_ast_gnu_extensions_e2e_part1 {
    include!("__split/c_ast_gnu_extensions_e2e_part1.rs");
}
mod c_ast_gnu_extensions_e2e_part2 {
    include!("__split/c_ast_gnu_extensions_e2e_part2.rs");
}
