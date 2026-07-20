// GPU/CPU parity tests for difficult C declarator edge cases.
//
// Coverage:
//   * arrays of function pointers
//   * function returning pointer to function
//   * nested type qualifiers (const, volatile, restrict, _Atomic)
//   * parameter typedef shadowing
//   * abstract declarators in casts and sizeof
//   * K&R-style function declarations
//   * deeply parenthesised declarators

// (cfg(feature = "c-parser") moved to parent c_ast_declarator_edge_cases.rs)

#[path = "../c_ast_gpu_parity_support/mod.rs"]
mod c_ast_gpu_parity_support;

use vyre_libs::parsing::c::lex::tokens::*;
use vyre_libs::parsing::c::lower::reference_ast_to_pg_nodes;
use vyre_libs::parsing::c::parse::vast::{
    reference_c11_annotate_typedef_names, reference_c11_build_vast_nodes,
    reference_c11_classify_vast_node_kinds, C_AST_KIND_ARRAY_DECL, C_AST_KIND_BIT_FIELD_DECL,
    C_AST_KIND_CAST_EXPR, C_AST_KIND_FIELD_DECL, C_AST_KIND_FUNCTION_DECLARATOR,
    C_AST_KIND_POINTER_DECL, C_AST_KIND_SIZEOF_EXPR,
};
use vyre_primitives::predicate::node_kind;

use c_ast_gpu_parity_support::{
    row_indices as typed_indices, run_gpu_classifier_with_count as run_gpu_classifier,
    run_gpu_full_typedef_annotation, run_gpu_pg_lower_with_count as run_gpu_pg_lower,
    run_gpu_vast_builder_from_parts as run_gpu_vast_builder, starts_for_lens, word_at,
    VAST_STRIDE_U32,
};

const TYPEDEF_FLAGS_FIELD: usize = 7;
const ORDINARY_FLAG_DECL: u32 = 1 << 2;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn node_count_from_vast(vast_bytes: &[u8]) -> u32 {
    (vast_bytes.len() / (VAST_STRIDE_U32 * 4)) as u32
}

fn run_gpu_annotate(raw_vast: &[u8], haystack: &[u8], _node_count: u32) -> Vec<u8> {
    run_gpu_full_typedef_annotation(haystack, raw_vast)
}

fn cpu_gpu_classified(tok_types: &[u32], tok_starts: &[u32], tok_lens: &[u32]) -> Vec<u8> {
    let raw = reference_c11_build_vast_nodes(tok_types, tok_starts, tok_lens);
    let expected = reference_c11_classify_vast_node_kinds(&raw);
    let gpu = run_gpu_classifier(&raw, node_count_from_vast(&raw));
    assert_eq!(
        gpu, expected,
        "GPU classifier must match CPU for declarator fixture"
    );
    expected
}

// ---------------------------------------------------------------------------
// Atom helpers for fixtures that need a haystack (typedef annotation)
// ---------------------------------------------------------------------------

enum Atom {
    Tok(u32),
    Ident(&'static str),
}

struct NamedFixture {
    tok_types: Vec<u32>,
    tok_starts: Vec<u32>,
    tok_lens: Vec<u32>,
    haystack: Vec<u8>,
}

fn tok(token: u32) -> Atom {
    Atom::Tok(token)
}

fn ident(name: &'static str) -> Atom {
    Atom::Ident(name)
}

fn named_fixture(atoms: &[Atom]) -> NamedFixture {
    let mut tok_types = Vec::with_capacity(atoms.len());
    let mut tok_starts = Vec::with_capacity(atoms.len());
    let mut tok_lens = Vec::with_capacity(atoms.len());
    let mut haystack = Vec::new();
    let mut cursor = 0u32;

    for atom in atoms {
        match atom {
            Atom::Tok(token) => {
                tok_types.push(*token);
                tok_starts.push(0);
                tok_lens.push(0);
            }
            Atom::Ident(name) => {
                tok_types.push(TOK_IDENTIFIER);
                tok_starts.push(cursor);
                tok_lens.push(name.len() as u32);
                haystack.extend_from_slice(name.as_bytes());
                cursor += name.len() as u32;
            }
        }
    }

    NamedFixture {
        tok_types,
        tok_starts,
        tok_lens,
        haystack,
    }
}

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// ```c
/// int (*handlers[4])(void *ctx, int opcode);
/// ```
fn fixture_array_of_function_pointers() -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_INT,
        TOK_LPAREN,
        TOK_STAR,
        TOK_IDENTIFIER,
        TOK_LBRACKET,
        TOK_INTEGER,
        TOK_RBRACKET,
        TOK_RPAREN,
        TOK_LPAREN,
        TOK_VOID,
        TOK_STAR,
        TOK_IDENTIFIER,
        TOK_COMMA,
        TOK_INT,
        TOK_IDENTIFIER,
        TOK_RPAREN,
        TOK_SEMICOLON,
    ];
    let tok_lens = vec![1; tok_types.len()];
    let tok_starts = starts_for_lens(&tok_lens);
    (tok_types, tok_starts, tok_lens)
}

/// ```c
/// int (*f(int))(float);
/// ```
fn fixture_function_returning_fnptr() -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_INT,
        TOK_LPAREN,
        TOK_STAR,
        TOK_IDENTIFIER,
        TOK_LPAREN,
        TOK_INT,
        TOK_RPAREN,
        TOK_RPAREN,
        TOK_LPAREN,
        TOK_FLOAT_KW,
        TOK_RPAREN,
        TOK_SEMICOLON,
    ];
    let tok_lens = vec![1; tok_types.len()];
    let tok_starts = starts_for_lens(&tok_lens);
    (tok_types, tok_starts, tok_lens)
}

/// ```c
/// const int * const * volatile p;
/// ```
fn fixture_nested_qualifiers() -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_CONST,
        TOK_INT,
        TOK_STAR,
        TOK_CONST,
        TOK_STAR,
        TOK_VOLATILE,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
    ];
    let tok_lens = vec![1; tok_types.len()];
    let tok_starts = starts_for_lens(&tok_lens);
    (tok_types, tok_starts, tok_lens)
}

/// ```c
/// typedef int T;
/// void f(int T) {
///   T * y;
/// }
/// ```
fn fixture_parameter_typedef_shadowing() -> NamedFixture {
    named_fixture(&[
        tok(TOK_TYPEDEF),
        tok(TOK_INT),
        ident("T"),
        tok(TOK_SEMICOLON),
        tok(TOK_VOID),
        ident("f"),
        tok(TOK_LPAREN),
        tok(TOK_INT),
        ident("T"),
        tok(TOK_RPAREN),
        tok(TOK_LBRACE),
        ident("T"),
        tok(TOK_STAR),
        ident("y"),
        tok(TOK_SEMICOLON),
        tok(TOK_RBRACE),
    ])
}

/// ```c
/// (void (*)(int))p;
/// ```
fn fixture_abstract_declarator_cast() -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_LPAREN,
        TOK_VOID,
        TOK_LPAREN,
        TOK_STAR,
        TOK_RPAREN,
        TOK_LPAREN,
        TOK_INT,
        TOK_RPAREN,
        TOK_RPAREN,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
    ];
    let tok_lens = vec![1; tok_types.len()];
    let tok_starts = starts_for_lens(&tok_lens);
    (tok_types, tok_starts, tok_lens)
}

/// ```c
/// sizeof(void (*)(int));
/// ```
fn fixture_abstract_declarator_sizeof() -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_SIZEOF,
        TOK_LPAREN,
        TOK_VOID,
        TOK_LPAREN,
        TOK_STAR,
        TOK_RPAREN,
        TOK_LPAREN,
        TOK_INT,
        TOK_RPAREN,
        TOK_RPAREN,
        TOK_SEMICOLON,
    ];
    let tok_lens = vec![1; tok_types.len()];
    let tok_starts = starts_for_lens(&tok_lens);
    (tok_types, tok_starts, tok_lens)
}

/// ```c
/// int f(a, b)
/// int a;
/// int b;
/// {
///   return a;
/// }
/// ```
fn fixture_kr_function() -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_INT,
        TOK_IDENTIFIER,
        TOK_LPAREN,
        TOK_IDENTIFIER,
        TOK_COMMA,
        TOK_IDENTIFIER,
        TOK_RPAREN,
        TOK_INT,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
        TOK_INT,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
        TOK_LBRACE,
        TOK_RETURN,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
        TOK_RBRACE,
    ];
    let tok_lens = vec![1; tok_types.len()];
    let tok_starts = starts_for_lens(&tok_lens);
    (tok_types, tok_starts, tok_lens)
}

/// ```c
/// int (((*p)));
/// ```
fn fixture_deeply_parenthesised_pointer() -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_INT,
        TOK_LPAREN,
        TOK_LPAREN,
        TOK_LPAREN,
        TOK_STAR,
        TOK_IDENTIFIER,
        TOK_RPAREN,
        TOK_RPAREN,
        TOK_RPAREN,
        TOK_SEMICOLON,
    ];
    let tok_lens = vec![1; tok_types.len()];
    let tok_starts = starts_for_lens(&tok_lens);
    (tok_types, tok_starts, tok_lens)
}

/// ```c
/// volatile int * const arr[8];
/// ```
fn fixture_qualified_pointer_array() -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_VOLATILE,
        TOK_INT,
        TOK_STAR,
        TOK_CONST,
        TOK_IDENTIFIER,
        TOK_LBRACKET,
        TOK_INTEGER,
        TOK_RBRACKET,
        TOK_SEMICOLON,
    ];
    let tok_lens = vec![1; tok_types.len()];
    let tok_starts = starts_for_lens(&tok_lens);
    (tok_types, tok_starts, tok_lens)
}
