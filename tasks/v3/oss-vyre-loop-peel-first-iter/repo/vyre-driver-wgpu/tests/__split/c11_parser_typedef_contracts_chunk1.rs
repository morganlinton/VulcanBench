// Contract tests for C typedef/name disambiguation.
//
// These tests assert the *correct* C semantics for typedef/name
// disambiguation.  Where the current reference implementation deviates
// from the standard, the tests fail and document the gap.
//
// Coverage:
//   * typedef T vs variable x in `(T)*p` (cast+deref) vs `(x)*p` (multiply)
//   * typedef shadowing in nested block scopes
//   * struct/enum tag names versus typedef names in declaration contexts
//   * pointer / array / function declarator context preservation

// cfg(feature = "c-parser")  -  moved to parent

#[path = "../c_ast_gpu_parity_support/mod.rs"]
mod c_ast_gpu_parity_support;

use vyre_libs::parsing::c::lex::tokens::*;
use vyre_libs::parsing::c::lower::reference_ast_to_pg_nodes;
use vyre_libs::parsing::c::parse::vast::{
    reference_c11_build_vast_nodes, reference_c11_classify_vast_node_kinds,
    C_AST_KIND_ARRAY_DECL, C_AST_KIND_CAST_EXPR,
    C_AST_KIND_FUNCTION_DECLARATOR, C_AST_KIND_POINTER_DECL, C_AST_KIND_UNARY_EXPR,
};
use vyre_primitives::predicate::node_kind;

use c_ast_gpu_parity_support::{
    row_indices as typed_indices, run_gpu_classifier_with_count as run_gpu_classifier,
    run_gpu_pg_lower_with_count as run_gpu_pg_lower,
    run_gpu_vast_builder_from_parts as run_gpu_vast_builder, starts_for_lens, word_at,
    VAST_STRIDE_U32,
};

const PG_STRIDE_U32: usize = 6;

fn assert_kind(rows: &[u8], idx: usize, kind: u32) {
    assert_eq!(word_at(rows, idx * VAST_STRIDE_U32), kind, "kind[{idx}]");
}

fn pg_word_at(buf: &[u8], idx: usize, field: usize) -> u32 {
    word_at(buf, idx * PG_STRIDE_U32 + field)
}

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// typedef int T;
/// void f(void) {
///   (T)*p;   -- cast expression: T is a typedef name
///   (x)*p;   -- multiplication: x is a variable, not a type
/// }
fn fixture_typedef_cast_vs_expr_multiply() -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_TYPEDEF,
        TOK_INT,
        TOK_IDENTIFIER, // T
        TOK_SEMICOLON,
        TOK_VOID,
        TOK_IDENTIFIER, // f
        TOK_LPAREN,
        TOK_VOID,
        TOK_RPAREN,
        TOK_LBRACE,
        TOK_LPAREN,     // (T)
        TOK_IDENTIFIER, // T
        TOK_RPAREN,
        TOK_STAR,
        TOK_IDENTIFIER, // p
        TOK_SEMICOLON,
        TOK_LPAREN,     // (x)
        TOK_IDENTIFIER, // x
        TOK_RPAREN,
        TOK_STAR,
        TOK_IDENTIFIER, // p
        TOK_SEMICOLON,
        TOK_RBRACE,
    ];
    let tok_lens = vec![1; tok_types.len()];
    let tok_starts = starts_for_lens(&tok_lens);
    (tok_types, tok_starts, tok_lens)
}

/// typedef int T;
/// void f(void) {
///   {
///     int T;   -- shadows the typedef
///     T * b;   -- multiplication, not pointer declaration
///   }
/// }
fn fixture_typedef_shadowing_nested() -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_TYPEDEF,
        TOK_INT,
        TOK_IDENTIFIER, // T
        TOK_SEMICOLON,
        TOK_VOID,
        TOK_IDENTIFIER, // f
        TOK_LPAREN,
        TOK_VOID,
        TOK_RPAREN,
        TOK_LBRACE,
        TOK_LBRACE,
        TOK_INT,
        TOK_IDENTIFIER, // T (variable)
        TOK_SEMICOLON,
        TOK_IDENTIFIER, // T
        TOK_STAR,
        TOK_IDENTIFIER, // b
        TOK_SEMICOLON,
        TOK_RBRACE,
        TOK_RBRACE,
    ];
    let tok_lens = vec![1; tok_types.len()];
    let tok_starts = starts_for_lens(&tok_lens);
    (tok_types, tok_starts, tok_lens)
}

/// struct S { int x; };
/// typedef struct S S;
/// void f(void) {
///   struct S *a;   -- tag name in declaration
///   S *b;          -- typedef name in declaration
/// }
fn fixture_struct_tag_vs_typedef() -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_STRUCT,
        TOK_IDENTIFIER, // S
        TOK_LBRACE,
        TOK_INT,
        TOK_IDENTIFIER, // x
        TOK_SEMICOLON,
        TOK_RBRACE,
        TOK_SEMICOLON,
        TOK_TYPEDEF,
        TOK_STRUCT,
        TOK_IDENTIFIER, // S
        TOK_IDENTIFIER, // S
        TOK_SEMICOLON,
        TOK_VOID,
        TOK_IDENTIFIER, // f
        TOK_LPAREN,
        TOK_VOID,
        TOK_RPAREN,
        TOK_LBRACE,
        TOK_STRUCT,
        TOK_IDENTIFIER, // S (tag)
        TOK_STAR,
        TOK_IDENTIFIER, // a
        TOK_SEMICOLON,
        TOK_IDENTIFIER, // S (typedef)
        TOK_STAR,
        TOK_IDENTIFIER, // b
        TOK_SEMICOLON,
        TOK_RBRACE,
    ];
    let tok_lens = vec![1; tok_types.len()];
    let tok_starts = starts_for_lens(&tok_lens);
    (tok_types, tok_starts, tok_lens)
}

/// void f(void) {
///   int *a[10];      -- array of pointers
///   int (*a)[10];    -- pointer to array
///   int *f(int);     -- function returning pointer
///   int (*f)(int);   -- pointer to function
/// }
fn fixture_declarator_contexts() -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_VOID,
        TOK_IDENTIFIER, // f
        TOK_LPAREN,
        TOK_VOID,
        TOK_RPAREN,
        TOK_LBRACE,
        TOK_INT,
        TOK_STAR,
        TOK_IDENTIFIER, // a
        TOK_LBRACKET,
        TOK_INTEGER, // 10
        TOK_RBRACKET,
        TOK_SEMICOLON,
        TOK_INT,
        TOK_LPAREN,
        TOK_STAR,
        TOK_IDENTIFIER, // a
        TOK_RPAREN,
        TOK_LBRACKET,
        TOK_INTEGER, // 10
        TOK_RBRACKET,
        TOK_SEMICOLON,
        TOK_INT,
        TOK_STAR,
        TOK_IDENTIFIER, // f
        TOK_LPAREN,
        TOK_INT,
        TOK_RPAREN,
        TOK_SEMICOLON,
        TOK_INT,
        TOK_LPAREN,
        TOK_STAR,
        TOK_IDENTIFIER, // f
        TOK_RPAREN,
        TOK_LPAREN,
        TOK_INT,
        TOK_RPAREN,
        TOK_SEMICOLON,
        TOK_RBRACE,
    ];
    let tok_lens = vec![1; tok_types.len()];
    let tok_starts = starts_for_lens(&tok_lens);
    (tok_types, tok_starts, tok_lens)
}

// ---------------------------------------------------------------------------
// CPU reference contract tests
// ---------------------------------------------------------------------------

#[test]
fn cpu_reference_typedef_cast_vs_expr_multiply() {
    let (tok_types, tok_starts, tok_lens) = fixture_typedef_cast_vs_expr_multiply();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    // (T)*p  -  T is a typedef name, so (T) introduces a cast.
    assert_eq!(
        word_at(&typed, 10 * VAST_STRIDE_U32),
        C_AST_KIND_CAST_EXPR,
        "(T)*p where T is typedef must classify (T) as cast expression"
    );
    // The * is unary dereference in a cast context.
    assert_eq!(
        word_at(&typed, 13 * VAST_STRIDE_U32),
        C_AST_KIND_UNARY_EXPR,
        "(T)*p star must be unary dereference"
    );

    // (x)*p  -  x is a variable, so (x) is a parenthesised expression and * is multiply.
    assert_ne!(
        word_at(&typed, 16 * VAST_STRIDE_U32),
        C_AST_KIND_CAST_EXPR,
        "(x)*p where x is a variable must NOT classify (x) as cast expression"
    );
    assert_eq!(
        word_at(&typed, 19 * VAST_STRIDE_U32),
        node_kind::BINARY,
        "(x)*p star must be binary multiplication"
    );
}

#[test]
fn cpu_reference_typedef_shadowing_nested() {
    let (tok_types, tok_starts, tok_lens) = fixture_typedef_shadowing_nested();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    // Inside the inner block, T is a variable, so T * b is multiplication.
    assert_eq!(
        word_at(&typed, 15 * VAST_STRIDE_U32),
        node_kind::BINARY,
        "shadowed typedef: T * b must be binary multiplication, not pointer declarator"
    );

    // T itself should be VARIABLE in the inner block (not a type).
    assert_eq!(
        word_at(&typed, 14 * VAST_STRIDE_U32),
        node_kind::VARIABLE,
        "shadowed typedef name used as value must classify as VARIABLE"
    );
}

#[test]
fn cpu_reference_struct_tag_vs_typedef() {
    let (tok_types, tok_starts, tok_lens) = fixture_struct_tag_vs_typedef();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    // struct S *a;  -  * must be a pointer declarator because we are in declaration context.
    assert_eq!(
        word_at(&typed, 21 * VAST_STRIDE_U32),
        C_AST_KIND_POINTER_DECL,
        "struct S *a star must be POINTER_DECL"
    );

    // S *b;  -  typedef name in declaration position, star must also be POINTER_DECL.
    assert_eq!(
        word_at(&typed, 25 * VAST_STRIDE_U32),
        C_AST_KIND_POINTER_DECL,
        "typedef S *b star must be POINTER_DECL"
    );

    // Both a and b must classify as variables (identifiers in declaration).
    assert_eq!(
        word_at(&typed, 22 * VAST_STRIDE_U32),
        node_kind::VARIABLE,
        "identifier a in struct S *a must be VARIABLE"
    );
    assert_eq!(
        word_at(&typed, 26 * VAST_STRIDE_U32),
        node_kind::VARIABLE,
        "identifier b in S *b must be VARIABLE"
    );
}

#[test]
fn cpu_reference_declarator_contexts() {
    let (tok_types, tok_starts, tok_lens) = fixture_declarator_contexts();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    // int *a[10];   -  star is POINTER_DECL, bracket is ARRAY_DECL
    assert_eq!(
        word_at(&typed, 7 * VAST_STRIDE_U32),
        C_AST_KIND_POINTER_DECL,
        "int *a[10] star must be POINTER_DECL"
    );
    assert_eq!(
        word_at(&typed, 9 * VAST_STRIDE_U32),
        C_AST_KIND_ARRAY_DECL,
        "int *a[10] bracket must be ARRAY_DECL"
    );

    // int (*a)[10];  -  star is POINTER_DECL, bracket is ARRAY_DECL
    assert_eq!(
        word_at(&typed, 15 * VAST_STRIDE_U32),
        C_AST_KIND_POINTER_DECL,
        "int (*a)[10] inner star must be POINTER_DECL"
    );
    assert_eq!(
        word_at(&typed, 18 * VAST_STRIDE_U32),
        C_AST_KIND_ARRAY_DECL,
        "int (*a)[10] bracket must be ARRAY_DECL"
    );

    // int *f(int);  -  star is POINTER_DECL, f is FUNCTION_DECL, ( is FUNCTION_DECLARATOR
    assert_eq!(
        word_at(&typed, 23 * VAST_STRIDE_U32),
        C_AST_KIND_POINTER_DECL,
        "int *f(int) star must be POINTER_DECL"
    );
    assert_eq!(
        word_at(&typed, 24 * VAST_STRIDE_U32),
        node_kind::FUNCTION_DECL,
        "int *f(int) identifier f must be FUNCTION_DECL"
    );
    assert_eq!(
        word_at(&typed, 25 * VAST_STRIDE_U32),
        C_AST_KIND_FUNCTION_DECLARATOR,
        "int *f(int) parameter paren must be FUNCTION_DECLARATOR"
    );

    // int (*f)(int);  -  star is POINTER_DECL, parameter ( is FUNCTION_DECLARATOR
    assert_eq!(
        word_at(&typed, 31 * VAST_STRIDE_U32),
        C_AST_KIND_POINTER_DECL,
        "int (*f)(int) inner star must be POINTER_DECL"
    );
    assert_eq!(
        word_at(&typed, 34 * VAST_STRIDE_U32),
        C_AST_KIND_FUNCTION_DECLARATOR,
        "int (*f)(int) parameter paren must be FUNCTION_DECLARATOR"
    );
}
