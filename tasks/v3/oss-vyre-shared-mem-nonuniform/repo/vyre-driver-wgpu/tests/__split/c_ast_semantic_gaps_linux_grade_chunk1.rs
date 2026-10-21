// Semantic-gap contracts for Linux-grade C AST / compiler front-end.
//
// High-signal tests that encode desired behavior for constructs not fully
// exercised by the existing hostile-parser or corpus suites:
//   * inner-typedef shadowing an outer typedef in nested block scopes
//   * enum definitions carrying GNU attributes
//   * GNU attributes on function parameters
//   * asm aliases on function declarations
//   * mixed designated / non-designated initializers
//   * incomplete initializer lists
//   * typedef-of-function-pointer used as a type specifier
//   * AST-to-PG lowering preservation for the above

// cfg(feature = "c-parser")  -  moved to parent

use vyre_libs::parsing::c::lex::tokens::*;
use vyre_libs::parsing::c::lower::reference_ast_to_pg_nodes;
use vyre_libs::parsing::c::parse::vast::{
    reference_c11_annotate_typedef_names, reference_c11_build_vast_nodes,
    reference_c11_classify_vast_node_kinds, C_AST_KIND_ASM_TEMPLATE, C_AST_KIND_ASSIGN_EXPR,
    C_AST_KIND_ATTRIBUTE_PACKED, C_AST_KIND_ATTRIBUTE_UNUSED, C_AST_KIND_ENUMERATOR_DECL,
    C_AST_KIND_ENUM_DECL, C_AST_KIND_FUNCTION_DECLARATOR, C_AST_KIND_GNU_ATTRIBUTE,
    C_AST_KIND_INITIALIZER_LIST, C_AST_KIND_INLINE_ASM, C_AST_KIND_MEMBER_ACCESS_EXPR,
    C_AST_KIND_TYPEDEF_DECL,
};
use vyre_primitives::predicate::node_kind;

const VAST_STRIDE_U32: usize = 10;
const PG_STRIDE_U32: usize = 6;
const TYPEDEF_FLAGS_FIELD: usize = 7;
const TYPEDEF_FLAG_VISIBLE: u32 = 1;
const TYPEDEF_FLAG_DECL: u32 = 1 << 1;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

// (mod common; replaced  -  use the shared c_ast_gpu_parity_support re-exports)
use crate::c_ast_gpu_parity_support::{
    build_fixture, run_gpu_classifier_with_count as run_gpu_classifier,
    run_gpu_pg_lower_with_count as run_gpu_pg_lower, Fixture, FixtureToken,
};

fn word_at(buf: &[u8], word: usize) -> u32 {
    let off = word * 4;
    u32::from_le_bytes(buf[off..off + 4].try_into().unwrap())
}

fn pg_word_at(buf: &[u8], idx: usize, field: usize) -> u32 {
    word_at(buf, idx * PG_STRIDE_U32 + field)
}

fn row_indices(rows: &[u8], kind: u32) -> Vec<usize> {
    rows.chunks_exact(VAST_STRIDE_U32 * 4)
        .enumerate()
        .filter_map(|(idx, row)| {
            let row_kind = u32::from_le_bytes(row[0..4].try_into().unwrap());
            (row_kind == kind).then_some(idx)
        })
        .collect()
}

fn kind_at(rows: &[u8], idx: usize) -> u32 {
    word_at(rows, idx * VAST_STRIDE_U32)
}

fn flags_at(rows: &[u8], idx: usize) -> u32 {
    word_at(rows, idx * VAST_STRIDE_U32 + TYPEDEF_FLAGS_FIELD)
}

fn node_count_from_vast(vast: &[u8]) -> u32 {
    (vast.len() / (VAST_STRIDE_U32 * 4)) as u32
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

/// ```c
/// typedef int T;
/// void f(void) {
///     typedef long T;
///     T x;
/// }
/// T y;
/// ```
fn fixture_inner_typedef_shadows_outer() -> Fixture {
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
        FixtureToken::new("typedef", TOK_IDENTIFIER),
        FixtureToken::new("long", TOK_IDENTIFIER),
        FixtureToken::new("T", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("T", TOK_IDENTIFIER),
        FixtureToken::new("x", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
        FixtureToken::new("T", TOK_IDENTIFIER),
        FixtureToken::new("y", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

/// ```c
/// enum __attribute__((packed)) E { A, B };
/// ```
fn fixture_enum_with_attribute() -> Fixture {
    build_fixture(&[
        FixtureToken::new("enum", TOK_IDENTIFIER),
        FixtureToken::new("__attribute__", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("packed", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("E", TOK_IDENTIFIER),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("A", TOK_IDENTIFIER),
        FixtureToken::new(",", TOK_COMMA),
        FixtureToken::new("B", TOK_IDENTIFIER),
        FixtureToken::new("}", TOK_RBRACE),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

/// ```c
/// void f(int __attribute__((unused)) x);
/// ```
fn fixture_parameter_with_attribute() -> Fixture {
    build_fixture(&[
        FixtureToken::new("void", TOK_IDENTIFIER),
        FixtureToken::new("f", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new("__attribute__", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("unused", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("x", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

/// ```c
/// void foo(void) __asm__("real_foo");
/// ```
fn fixture_asm_alias() -> Fixture {
    build_fixture(&[
        FixtureToken::new("void", TOK_IDENTIFIER),
        FixtureToken::new("foo", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("void", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("__asm__", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("\"real_foo\"", TOK_STRING),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

/// ```c
/// struct S s = { 1, .b = 2, 3 };
/// ```
fn fixture_mixed_designated_and_plain_init() -> Fixture {
    build_fixture(&[
        FixtureToken::new("struct", TOK_IDENTIFIER),
        FixtureToken::new("S", TOK_IDENTIFIER),
        FixtureToken::new("s", TOK_IDENTIFIER),
        FixtureToken::new("=", TOK_ASSIGN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("1", TOK_INTEGER),
        FixtureToken::new(",", TOK_COMMA),
        FixtureToken::new(".", TOK_DOT),
        FixtureToken::new("b", TOK_IDENTIFIER),
        FixtureToken::new("=", TOK_ASSIGN),
        FixtureToken::new("2", TOK_INTEGER),
        FixtureToken::new(",", TOK_COMMA),
        FixtureToken::new("3", TOK_INTEGER),
        FixtureToken::new("}", TOK_RBRACE),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

/// ```c
/// int arr[4] = { 1, 2 };
/// ```
fn fixture_incomplete_array_init() -> Fixture {
    build_fixture(&[
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new("arr", TOK_IDENTIFIER),
        FixtureToken::new("[", TOK_LBRACKET),
        FixtureToken::new("4", TOK_INTEGER),
        FixtureToken::new("]", TOK_RBRACKET),
        FixtureToken::new("=", TOK_ASSIGN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("1", TOK_INTEGER),
        FixtureToken::new(",", TOK_COMMA),
        FixtureToken::new("2", TOK_INTEGER),
        FixtureToken::new("}", TOK_RBRACE),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

/// ```c
/// typedef int (*fn_t)(int);
/// fn_t f;
/// ```
fn fixture_function_pointer_typedef_usage() -> Fixture {
    build_fixture(&[
        FixtureToken::new("typedef", TOK_IDENTIFIER),
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("*", TOK_STAR),
        FixtureToken::new("fn_t", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("fn_t", TOK_IDENTIFIER),
        FixtureToken::new("f", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

// ---------------------------------------------------------------------------
// CPU reference contracts
// ---------------------------------------------------------------------------

#[test]
fn cpu_inner_typedef_shadows_outer_typedef() {
    let fix = fixture_inner_typedef_shadows_outer();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);

    // Outer typedef declaration
    assert_ne!(
        flags_at(&annotated, 2) & TYPEDEF_FLAG_DECL,
        0,
        "outer typedef `T` must carry TYPEDEF_FLAG_DECL"
    );
    // Inner typedef declaration
    assert_ne!(
        flags_at(&annotated, 12) & TYPEDEF_FLAG_DECL,
        0,
        "inner typedef `T` must carry TYPEDEF_FLAG_DECL"
    );
    // Use of inner typedef
    assert_ne!(
        flags_at(&annotated, 14) & TYPEDEF_FLAG_VISIBLE,
        0,
        "`T` inside `f` must be visible as the inner typedef"
    );
    assert_eq!(
        kind_at(&typed, 15),
        node_kind::VARIABLE,
        "`x` declared with inner typedef must classify as VARIABLE"
    );
    // Use of outer typedef after block
    assert_ne!(
        flags_at(&annotated, 18) & TYPEDEF_FLAG_VISIBLE,
        0,
        "`T` after `f` must be visible as the outer typedef"
    );
    assert_eq!(
        kind_at(&typed, 19),
        node_kind::VARIABLE,
        "`y` declared with restored outer typedef must classify as VARIABLE"
    );
}

#[test]
fn cpu_enum_with_attribute_classifies() {
    let fix = fixture_enum_with_attribute();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);

    assert_eq!(
        kind_at(&typed, 0),
        C_AST_KIND_ENUM_DECL,
        "enum keyword must classify as ENUM_DECL"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_GNU_ATTRIBUTE),
        vec![1],
        "__attribute__ must classify as GNU_ATTRIBUTE"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ATTRIBUTE_PACKED),
        vec![4],
        "packed must classify as ATTRIBUTE_PACKED"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ENUMERATOR_DECL),
        vec![9, 11],
        "enumerators A and B must classify as ENUMERATOR_DECL"
    );
}

#[test]
fn cpu_parameter_with_attribute_classifies() {
    let fix = fixture_parameter_with_attribute();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);

    assert_eq!(
        kind_at(&typed, 1),
        node_kind::FUNCTION_DECL,
        "function name `f` must classify as FUNCTION_DECL"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_FUNCTION_DECLARATOR),
        vec![2],
        "parameter-list paren must classify as FUNCTION_DECLARATOR"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_GNU_ATTRIBUTE),
        vec![4],
        "parameter attribute must classify as GNU_ATTRIBUTE"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ATTRIBUTE_UNUSED),
        vec![7],
        "unused must classify as ATTRIBUTE_UNUSED"
    );
    assert_eq!(
        kind_at(&typed, 10),
        node_kind::VARIABLE,
        "parameter name `x` must classify as VARIABLE"
    );
}
