//! Integration contracts for VYRE C parser covering Linux-GNU declaration,
//! type, and preprocessor syntax contracts.
//!
//! Coverage:
//!   * typedef shadowing (including by __auto_type)
//!   * enum/tag scopes and forward declarations
//!   * GNU __attribute__ on structs and function-pointer typedefs
//!   * __auto_type declarations
//!   * typeof / typeof_unqual in declarators
//!   * _Alignas and __attribute__((aligned(...)))
//!   * nested designated initializers with range designators
//!   * bit-fields (named, unnamed, zero-width, with attributes)
//!   * flexible array members
//!   * complex function pointers (signal-like)
//!   * abstract declarators in casts
//!   * statement expressions containing inline asm in initializers
//!   * macro-shaped declarations (call-like macro syntax at top level)
//!   * nested conditional preprocessing (#if / #ifdef / #else / #elif / #endif)
//!
//! Every test asserts parser/VAST/PG contracts and GPU/CPU parity where
//! applicable. A missing GPU adapter is a configuration failure.
#![cfg(feature = "c-parser")]
#![allow(deprecated)]

mod common;
use common::{decode_u32_words, u32_bytes};
mod c_ast_gpu_parity_support;
use c_ast_gpu_parity_support::{
    assert_full_pipeline_parity, build_fixture, row_indices, word_at, Fixture, FixtureToken,
    VAST_STRIDE_U32,
};
use vyre::ir::Expr;
use vyre_libs::parsing::c::lex::keyword::reference_c_keyword_types;
use vyre_libs::parsing::c::lex::tokens::*;
use vyre_libs::parsing::c::lower::reference_ast_to_pg_nodes;
use vyre_libs::parsing::c::parse::vast::{
    reference_c11_annotate_typedef_names, reference_c11_build_vast_nodes,
    reference_c11_classify_vast_node_kinds, C_AST_KIND_ARRAY_DECL, C_AST_KIND_ARRAY_SUBSCRIPT_EXPR,
    C_AST_KIND_ASM_INPUT_OPERAND, C_AST_KIND_ASM_OUTPUT_OPERAND, C_AST_KIND_ASM_QUALIFIER,
    C_AST_KIND_ASM_TEMPLATE, C_AST_KIND_ASSIGN_EXPR, C_AST_KIND_ATTRIBUTE_ALIGNED,
    C_AST_KIND_ATTRIBUTE_PACKED, C_AST_KIND_BIT_FIELD_DECL, C_AST_KIND_CAST_EXPR,
    C_AST_KIND_ENUM_DECL, C_AST_KIND_FIELD_DECL, C_AST_KIND_FUNCTION_DECLARATOR,
    C_AST_KIND_GNU_ATTRIBUTE, C_AST_KIND_INITIALIZER_LIST, C_AST_KIND_INLINE_ASM,
    C_AST_KIND_MEMBER_ACCESS_EXPR, C_AST_KIND_POINTER_DECL, C_AST_KIND_RANGE_DESIGNATOR_EXPR,
    C_AST_KIND_STRUCT_DECL, C_AST_KIND_TYPEDEF_DECL, C_AST_KIND_UNARY_EXPR,
};
use vyre_libs::parsing::c::preprocess::expansion::opt_conditional_mask_with_directives;
use vyre_libs::parsing::c::preprocess::reference_c_preprocessor_directive_metadata;
use vyre_primitives::predicate::node_kind;
use vyre_reference::value::Value;
const PG_STRIDE_U32: usize = 6;
const FLAGS_FIELD: usize = 7;
const TYPEDEF_FLAG_VISIBLE: u32 = 1;
const TYPEDEF_FLAG_DECL: u32 = 1 << 1;
const ORDINARY_FLAG_DECL: u32 = 1 << 2;
// ---------------------------------------------------------------------------
// Local helpers
// ---------------------------------------------------------------------------
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
fn classify_fixture(fix: &Fixture) -> Vec<u8> {
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    reference_c11_classify_vast_node_kinds(&annotated)
}
/// Build a [`Fixture`] from explicit lexemes, skipping whitespace in the
/// token lists (so the VAST builder sees only real tokens) while preserving
/// newlines in the source string for preprocessor-aware tests.
fn assemble_fixture(lexemes: &[(&str, u32)]) -> Fixture {
    let mut source = String::new();
    let mut tok_starts = Vec::new();
    let mut tok_lens = Vec::new();
    let mut raw_kinds = Vec::new();
    for (lex, kind) in lexemes {
        if *kind == TOK_WHITESPACE || *kind == TOK_COMMENT {
            source.push_str(lex);
            continue;
        }
        if !source.is_empty() && !source.ends_with('\n') {
            source.push(' ');
        }
        tok_starts.push(source.len() as u32);
        source.push_str(lex);
        tok_lens.push(lex.len() as u32);
        raw_kinds.push(*kind);
    }
    let tok_types =
        reference_c_keyword_types(&raw_kinds, &tok_starts, &tok_lens, source.as_bytes());
    Fixture {
        source,
        raw_kinds,
        tok_types,
        tok_starts,
        tok_lens,
    }
}
// ---------------------------------------------------------------------------
// 1. Typedef shadowing
// ---------------------------------------------------------------------------
fn fixture_typedef_shadowed_by_auto_type() -> Fixture {
    build_fixture(&[
        FixtureToken::new("typedef", TOK_IDENTIFIER),
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new("T", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("void", TOK_IDENTIFIER),
        FixtureToken::new("f", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("void", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("__auto_type", TOK_IDENTIFIER),
        FixtureToken::new("T", TOK_IDENTIFIER),
        FixtureToken::new("=", TOK_ASSIGN),
        FixtureToken::new("1", TOK_INTEGER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("T", TOK_IDENTIFIER),
        FixtureToken::new("*", TOK_STAR),
        FixtureToken::new("p", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
    ])
}
// Flat include of all parts so cross-part fixture references resolve
// in a single scope (per-part `mod {...}` wrappers broke this).
include!("__split/c_ast_linux_gnu_declarations_preprocessor_contracts_part1.rs");
include!("__split/c_ast_linux_gnu_declarations_preprocessor_contracts_part2.rs");
include!("__split/c_ast_linux_gnu_declarations_preprocessor_contracts_part3.rs");
include!("__split/c_ast_linux_gnu_declarations_preprocessor_contracts_part4.rs");
include!("__split/c_ast_linux_gnu_declarations_preprocessor_contracts_part5.rs");
include!("__split/c_ast_linux_gnu_declarations_preprocessor_contracts_part6.rs");
include!("__split/c_ast_linux_gnu_declarations_preprocessor_contracts_part7.rs");
include!("__split/c_ast_linux_gnu_declarations_preprocessor_contracts_part8.rs");
include!("__split/c_ast_linux_gnu_declarations_preprocessor_contracts_part9.rs");
include!("__split/c_ast_linux_gnu_declarations_preprocessor_contracts_part10.rs");
include!("__split/c_ast_linux_gnu_declarations_preprocessor_contracts_part11.rs");
include!("__split/c_ast_linux_gnu_declarations_preprocessor_contracts_part12.rs");
include!("__split/c_ast_linux_gnu_declarations_preprocessor_contracts_part13.rs");
include!("__split/c_ast_linux_gnu_declarations_preprocessor_contracts_part14.rs");
