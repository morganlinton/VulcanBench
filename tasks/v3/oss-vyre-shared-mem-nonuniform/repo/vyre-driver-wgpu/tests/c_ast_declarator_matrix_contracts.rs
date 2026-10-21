//! Integration contracts for Linux-grade C declarator matrices.
//!
//! Coverage:
//!   * pointer-to-array declarators (`int (*p)[4];`)
//!   * storage-class specifiers threaded through multi-declarator lists
//!   * parameter array declarators with `static` / `restrict` (C99)
//!   * nested typedef names inside declarators (function-pointer typedef reuse)
//!   * struct / union / enum tag definitions followed by mixed declarators
//!   * abstract declarators with qualifiers in cast contexts
//!   * GNU `__restrict` normalized to the C restrict qualifier
//!
//! Asserts:
//!   - specifier propagation: standard qualifiers and storage classes stay raw
//!     syntax while declarator identifiers, pointers, arrays and function parens
//!     get precise AST kinds.
//!   - AST classification: POINTER_DECL, ARRAY_DECL, FUNCTION_DECLARATOR,
//!     VARIABLE, FUNCTION_DECL, FIELD_DECL, STRUCT_DECL, UNION_DECL, ENUM_DECL,
//!     ENUMERATOR_DECL.
//!   - typedef annotations: typedef declarations carry TYPEDEF_FLAG_DECL;
//!     typedef uses inside declarator contexts carry TYPEDEF_FLAG_VISIBLE.
//!   - CPU/GPU parity for VAST builder, classifier and PG lowerer, including
//!     stage-specific parity for abstract-declarator casts without typedef names.
//!
//! A missing GPU adapter is a configuration failure, never a silent skip.

#![cfg(feature = "c-parser")]
#![allow(deprecated)]
#[path = "c_ast_gpu_parity_support/mod.rs"]
mod c_ast_gpu_parity_support;

use vyre_libs::parsing::c::lex::tokens::*;
use vyre_libs::parsing::c::lower::reference_ast_to_pg_nodes;
use vyre_libs::parsing::c::parse::vast::{
    reference_c11_annotate_typedef_names, reference_c11_build_vast_nodes,
    reference_c11_classify_vast_node_kinds, C_AST_KIND_ARRAY_DECL, C_AST_KIND_CAST_EXPR,
    C_AST_KIND_ENUMERATOR_DECL, C_AST_KIND_ENUM_DECL, C_AST_KIND_FIELD_DECL,
    C_AST_KIND_FUNCTION_DECLARATOR, C_AST_KIND_POINTER_DECL, C_AST_KIND_STRUCT_DECL,
    C_AST_KIND_TYPEDEF_DECL, C_AST_KIND_UNION_DECL,
};
use vyre_primitives::predicate::node_kind;

use c_ast_gpu_parity_support::{
    c_fixture, row_indices, run_gpu_classifier, run_gpu_fast_typedef_annotation,
    run_gpu_pg_lower_with_count as run_gpu_pg_lower, run_gpu_vast_builder_from_parts, word_at,
    Fixture, VAST_STRIDE_U32,
};

const PG_STRIDE_U32: usize = 6;
const TYPEDEF_FLAGS_FIELD: usize = 7;
const TYPEDEF_FLAG_DECL: u32 = 1 << 1;
const TYPEDEF_FLAG_VISIBLE: u32 = 1;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn kind_at(rows: &[u8], idx: usize) -> u32 {
    word_at(rows, idx * VAST_STRIDE_U32)
}

fn flags_at(rows: &[u8], idx: usize) -> u32 {
    word_at(rows, idx * VAST_STRIDE_U32 + TYPEDEF_FLAGS_FIELD)
}

fn node_count_from_vast(vast: &[u8]) -> u32 {
    (vast.len() / (VAST_STRIDE_U32 * 4)) as u32
}

fn pg_word_at(buf: &[u8], idx: usize, field: usize) -> u32 {
    word_at(buf, idx * PG_STRIDE_U32 + field)
}

fn run_gpu_annotate(fix: &Fixture, raw_vast: &[u8]) -> Vec<u8> {
    run_gpu_fast_typedef_annotation(fix.source.as_bytes(), raw_vast)
}

fn assert_full_pipeline_parity(fix: &Fixture, label: &str) {
    let raw_cpu = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let raw_gpu = run_gpu_vast_builder_from_parts(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    assert_words_eq(
        &raw_gpu,
        &raw_cpu,
        &format!("{label}: raw VAST GPU/CPU parity"),
    );

    let annotated_cpu = reference_c11_annotate_typedef_names(&raw_cpu, fix.source.as_bytes());
    let annotated_gpu = run_gpu_annotate(fix, &raw_gpu);
    assert_words_eq(
        &annotated_gpu,
        &annotated_cpu,
        &format!("{label}: annotated VAST GPU/CPU parity"),
    );

    let typed_cpu = reference_c11_classify_vast_node_kinds(&annotated_cpu);
    let typed_gpu = run_gpu_classifier(&annotated_gpu);
    assert_words_eq(
        &typed_gpu,
        &typed_cpu,
        &format!("{label}: typed VAST GPU/CPU parity"),
    );
}

fn assert_words_eq(actual: &[u8], expected: &[u8], context: &str) {
    if actual == expected {
        return;
    }
    let limit = (actual.len() / 4).min(expected.len() / 4);
    for w in 0..limit {
        let a = word_at(actual, w);
        let e = word_at(expected, w);
        if a != e {
            panic!(
                "{context}: word {w} differs (row={}, field={}): actual={a}, expected={e}",
                w / VAST_STRIDE_U32,
                w % VAST_STRIDE_U32
            );
        }
    }
    panic!(
        "{context}: byte lengths differ: actual={}, expected={}",
        actual.len(),
        expected.len()
    );
}

fn assert_pg_preserves_row(
    typed_vast: &[u8],
    pg: &[u8],
    tok_starts: &[u32],
    tok_lens: &[u32],
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
        tok_starts[idx],
        "PG span_start mismatch at row {idx}"
    );
    assert_eq!(
        pg_word_at(pg, idx, 2),
        tok_starts[idx] + tok_lens[idx],
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

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// `int (*p)[4];`
fn fixture_pointer_to_array() -> Fixture {
    c_fixture![
        ("int", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("*", TOK_STAR),
        ("p", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        ("[", TOK_LBRACKET),
        ("4", TOK_INTEGER),
        ("]", TOK_RBRACKET),
        (";", TOK_SEMICOLON),
    ]
}

/// `static const int *p, arr[4];`
fn fixture_storage_class_multi_declarator() -> Fixture {
    c_fixture![
        ("static", TOK_IDENTIFIER),
        ("const", TOK_IDENTIFIER),
        ("int", TOK_IDENTIFIER),
        ("*", TOK_STAR),
        ("p", TOK_IDENTIFIER),
        (",", TOK_COMMA),
        ("arr", TOK_IDENTIFIER),
        ("[", TOK_LBRACKET),
        ("4", TOK_INTEGER),
        ("]", TOK_RBRACKET),
        (";", TOK_SEMICOLON),
    ]
}

/// `void f(int arr[static restrict 10]);`
fn fixture_parameter_array_static_restrict() -> Fixture {
    c_fixture![
        ("void", TOK_IDENTIFIER),
        ("f", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("int", TOK_IDENTIFIER),
        ("arr", TOK_IDENTIFIER),
        ("[", TOK_LBRACKET),
        ("static", TOK_IDENTIFIER),
        ("restrict", TOK_IDENTIFIER),
        ("10", TOK_INTEGER),
        ("]", TOK_RBRACKET),
        (")", TOK_RPAREN),
        (";", TOK_SEMICOLON),
    ]
}

/// ```c
/// typedef int (*fn_t)(int);
/// fn_t f;
/// ```
fn fixture_nested_typedef_complex_declarator() -> Fixture {
    c_fixture![
        ("typedef", TOK_IDENTIFIER),
        ("int", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("*", TOK_STAR),
        ("fn_t", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        ("(", TOK_LPAREN),
        ("int", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        (";", TOK_SEMICOLON),
        ("fn_t", TOK_IDENTIFIER),
        ("f", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
    ]
}

/// ```c
/// struct foo { int x; } *p, arr[2];
/// ```
fn fixture_struct_tag_with_mixed_declarators() -> Fixture {
    c_fixture![
        ("struct", TOK_IDENTIFIER),
        ("foo", TOK_IDENTIFIER),
        ("{", TOK_LBRACE),
        ("int", TOK_IDENTIFIER),
        ("x", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
        ("}", TOK_RBRACE),
        ("*", TOK_STAR),
        ("p", TOK_IDENTIFIER),
        (",", TOK_COMMA),
        ("arr", TOK_IDENTIFIER),
        ("[", TOK_LBRACKET),
        ("2", TOK_INTEGER),
        ("]", TOK_RBRACKET),
        (";", TOK_SEMICOLON),
    ]
}

/// ```c
/// union cell { char c; int i; } u, *up;
/// ```
fn fixture_union_tag_with_mixed_declarators() -> Fixture {
    c_fixture![
        ("union", TOK_IDENTIFIER),
        ("cell", TOK_IDENTIFIER),
        ("{", TOK_LBRACE),
        ("char", TOK_IDENTIFIER),
        ("c", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
        ("int", TOK_IDENTIFIER),
        ("i", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
        ("}", TOK_RBRACE),
        ("u", TOK_IDENTIFIER),
        (",", TOK_COMMA),
        ("*", TOK_STAR),
        ("up", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
    ]
}

/// ```c
/// enum mode { ON, OFF } ev, *ep;
/// ```
fn fixture_enum_tag_with_mixed_declarators() -> Fixture {
    c_fixture![
        ("enum", TOK_IDENTIFIER),
        ("mode", TOK_IDENTIFIER),
        ("{", TOK_LBRACE),
        ("ON", TOK_IDENTIFIER),
        (",", TOK_COMMA),
        ("OFF", TOK_IDENTIFIER),
        ("}", TOK_RBRACE),
        ("ev", TOK_IDENTIFIER),
        (",", TOK_COMMA),
        ("*", TOK_STAR),
        ("ep", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
    ]
}

/// `extern volatile char * const * restrict x, y[8];`
fn fixture_heavy_qualifiers_and_storage_multi_decl() -> Fixture {
    c_fixture![
        ("extern", TOK_IDENTIFIER),
        ("volatile", TOK_IDENTIFIER),
        ("char", TOK_IDENTIFIER),
        ("*", TOK_STAR),
        ("const", TOK_IDENTIFIER),
        ("*", TOK_STAR),
        ("restrict", TOK_IDENTIFIER),
        ("x", TOK_IDENTIFIER),
        (",", TOK_COMMA),
        ("y", TOK_IDENTIFIER),
        ("[", TOK_LBRACKET),
        ("8", TOK_INTEGER),
        ("]", TOK_RBRACKET),
        (";", TOK_SEMICOLON),
    ]
}

/// `(const int (*)(void))p;`
fn fixture_abstract_declarator_with_qualifiers() -> Fixture {
    c_fixture![
        ("(", TOK_LPAREN),
        ("const", TOK_IDENTIFIER),
        ("int", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("*", TOK_STAR),
        (")", TOK_RPAREN),
        ("(", TOK_LPAREN),
        ("void", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        (")", TOK_RPAREN),
        ("p", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
    ]
}

/// `char * __restrict z;`
fn fixture_gnu_restrict_qualifier() -> Fixture {
    c_fixture![
        ("char", TOK_IDENTIFIER),
        ("*", TOK_STAR),
        ("__restrict", TOK_IDENTIFIER),
        ("z", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
    ]
}

// ---------------------------------------------------------------------------
// CPU reference contracts
// ---------------------------------------------------------------------------

mod c_ast_declarator_matrix_contracts_part1 {

    include!("__split/c_ast_declarator_matrix_contracts_part1.rs");
}
mod c_ast_declarator_matrix_contracts_part2 {
    include!("__split/c_ast_declarator_matrix_contracts_part2.rs");
}
