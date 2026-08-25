// Gemini-mandated aggressive AST contract tests for VYRE C parser.
//
// Scope: Typedef shadowing, Cast/Pointer ambiguity, Nested FnPtrs,
// Compound Literals, GNU Attributes, Tag Separation, PG Parity.

// cfg(feature = "c-parser")  -  moved to parent
// allow(clippy::erasing_op)  -  moved to parent

#[path = "../c_ast_gpu_parity_support/mod.rs"]
mod c_ast_gpu_parity_support;

use vyre_libs::parsing::c::lex::tokens::*;
use vyre_libs::parsing::c::lower::reference_ast_to_pg_nodes;
use vyre_libs::parsing::c::parse::vast::*;
use vyre_primitives::predicate::node_kind;

use c_ast_gpu_parity_support::{
    row_indices as typed_indices, run_gpu_classifier_with_count as run_gpu_classifier,
    run_gpu_pg_lower_with_count as run_gpu_pg_lower, starts_for_lens, word_at, VAST_STRIDE_U32,
};

// ---------------------------------------------------------------------------
// Helpers
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
                cursor = cursor.saturating_add(name.len() as u32);
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

fn annotated_named_vast(fix: &NamedFixture) -> Vec<u8> {
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    reference_c11_annotate_typedef_names(&raw, &fix.haystack)
}

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// typedef int T;
/// void f() {
///   T x;
///   {
///     float T;
///     T = 1.0f;
///   }
///   T y;
/// }
fn fixture_typedef_shadowing() -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_TYPEDEF,
        TOK_INT,
        TOK_IDENTIFIER,
        TOK_SEMICOLON, // 0-3: typedef int T;
        TOK_VOID,
        TOK_IDENTIFIER,
        TOK_LPAREN,
        TOK_VOID,
        TOK_RPAREN, // 4-8: void f(void)
        TOK_LBRACE, // 9: {
        TOK_IDENTIFIER,
        TOK_IDENTIFIER,
        TOK_SEMICOLON, // 10-12: T x;
        TOK_LBRACE,    // 13: {
        TOK_FLOAT_KW,
        TOK_IDENTIFIER,
        TOK_SEMICOLON, // 14-16: float T;
        TOK_IDENTIFIER,
        TOK_ASSIGN,
        TOK_FLOAT,
        TOK_SEMICOLON, // 17-20: T = 1.0f;
        TOK_RBRACE,    // 21: }
        TOK_IDENTIFIER,
        TOK_IDENTIFIER,
        TOK_SEMICOLON, // 22-24: T y;
        TOK_RBRACE,    // 25: }
    ];
    let tok_lens = vec![1; tok_types.len()];
    let tok_starts = starts_for_lens(&tok_lens);
    (tok_types, tok_starts, tok_lens)
}

/// typedef int T;
/// void f() {
///   (T)*x;  // cast
///   int T;
///   (T)*x;  // multiply
/// }
fn fixture_cast_vs_multiply() -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_TYPEDEF,
        TOK_INT,
        TOK_IDENTIFIER,
        TOK_SEMICOLON, // 0-3: typedef int T;
        TOK_VOID,
        TOK_IDENTIFIER,
        TOK_LPAREN,
        TOK_VOID,
        TOK_RPAREN, // 4-8
        TOK_LBRACE, // 9
        TOK_LPAREN,
        TOK_IDENTIFIER,
        TOK_RPAREN,
        TOK_STAR,
        TOK_IDENTIFIER,
        TOK_SEMICOLON, // 10-15: (T)*x;
        TOK_INT,
        TOK_IDENTIFIER,
        TOK_SEMICOLON, // 16-18: int T;
        TOK_LPAREN,
        TOK_IDENTIFIER,
        TOK_RPAREN,
        TOK_STAR,
        TOK_IDENTIFIER,
        TOK_SEMICOLON, // 19-24: (T)*x;
        TOK_RBRACE,
    ];
    let tok_lens = vec![1; tok_types.len()];
    let tok_starts = starts_for_lens(&tok_lens);
    (tok_types, tok_starts, tok_lens)
}

/// int (*(*f)(int))(float);
fn fixture_nested_fnptr() -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_INT,
        TOK_LPAREN,
        TOK_STAR,
        TOK_LPAREN,
        TOK_STAR,
        TOK_IDENTIFIER,
        TOK_RPAREN,
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

/// struct T { int x; };
/// typedef int T;
/// void f() {
///   struct T a;
///   T b;
/// }
fn fixture_tag_separation() -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_STRUCT,
        TOK_IDENTIFIER,
        TOK_LBRACE,
        TOK_INT,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
        TOK_RBRACE,
        TOK_SEMICOLON,
        TOK_TYPEDEF,
        TOK_INT,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
        TOK_VOID,
        TOK_IDENTIFIER,
        TOK_LPAREN,
        TOK_VOID,
        TOK_RPAREN,
        TOK_LBRACE,
        TOK_STRUCT,
        TOK_IDENTIFIER,
        TOK_IDENTIFIER,
        TOK_SEMICOLON, // struct T a;
        TOK_IDENTIFIER,
        TOK_IDENTIFIER,
        TOK_SEMICOLON, // T b;
        TOK_RBRACE,
    ];
    let tok_lens = vec![1; tok_types.len()];
    let tok_starts = starts_for_lens(&tok_lens);
    (tok_types, tok_starts, tok_lens)
}

/// __attribute__((pure)) int g(int x);
fn fixture_gnu_attributes() -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_GNU_ATTRIBUTE,
        TOK_LPAREN,
        TOK_LPAREN,
        TOK_IDENTIFIER,
        TOK_RPAREN,
        TOK_RPAREN,
        TOK_INT,
        TOK_IDENTIFIER,
        TOK_LPAREN,
        TOK_INT,
        TOK_IDENTIFIER,
        TOK_RPAREN,
        TOK_SEMICOLON,
    ];
    let tok_lens = vec![1; tok_types.len()];
    let tok_starts = starts_for_lens(&tok_lens);
    (tok_types, tok_starts, tok_lens)
}

/// (int){1}
fn fixture_compound_literal() -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_LPAREN,
        TOK_INT,
        TOK_RPAREN,
        TOK_LBRACE,
        TOK_INTEGER,
        TOK_RBRACE,
        TOK_SEMICOLON,
    ];
    let tok_lens = vec![1; tok_types.len()];
    let tok_starts = starts_for_lens(&tok_lens);
    (tok_types, tok_starts, tok_lens)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn cpu_reference_typedef_shadowing() {
    let (tok_types, tok_starts, tok_lens) = fixture_typedef_shadowing();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    assert_eq!(
        word_at(&typed, 11 * VAST_STRIDE_U32),
        node_kind::VARIABLE,
        "x must be a variable"
    );
    assert_eq!(
        word_at(&typed, 15 * VAST_STRIDE_U32),
        node_kind::VARIABLE,
        "shadowing T must be a variable"
    );
    assert_eq!(
        word_at(&typed, 17 * VAST_STRIDE_U32),
        node_kind::VARIABLE,
        "use of shadowed T must be a variable"
    );
    assert_eq!(
        word_at(&typed, 23 * VAST_STRIDE_U32),
        node_kind::VARIABLE,
        "y must be a variable after shadow block"
    );
}

#[test]
fn cpu_reference_cast_vs_multiply() {
    let fix = named_fixture(&[
        tok(TOK_TYPEDEF),
        tok(TOK_INT),
        ident("T"),
        tok(TOK_SEMICOLON),
        tok(TOK_VOID),
        ident("f"),
        tok(TOK_LPAREN),
        tok(TOK_VOID),
        tok(TOK_RPAREN),
        tok(TOK_LBRACE),
        tok(TOK_LPAREN),
        ident("T"),
        tok(TOK_RPAREN),
        tok(TOK_STAR),
        ident("x"),
        tok(TOK_SEMICOLON),
        tok(TOK_INT),
        ident("T"),
        tok(TOK_SEMICOLON),
        tok(TOK_LPAREN),
        ident("T"),
        tok(TOK_RPAREN),
        tok(TOK_STAR),
        ident("x"),
        tok(TOK_SEMICOLON),
        tok(TOK_RBRACE),
    ]);
    let typed = reference_c11_classify_vast_node_kinds(&annotated_named_vast(&fix));

    assert_eq!(
        word_at(&typed, 10 * VAST_STRIDE_U32),
        C_AST_KIND_CAST_EXPR,
        "(T)*x must be cast when T is typedef"
    );
    assert_ne!(
        word_at(&typed, 19 * VAST_STRIDE_U32),
        C_AST_KIND_CAST_EXPR,
        "(T)*x must NOT be cast when T is shadowed by variable"
    );
}

#[test]
fn cpu_reference_nested_fnptr() {
    let (tok_types, tok_starts, tok_lens) = fixture_nested_fnptr();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    let ptr_decls = typed_indices(&typed, C_AST_KIND_POINTER_DECL);
    assert_eq!(
        ptr_decls.len(),
        2,
        "must find two pointer declarators in nested fnptr"
    );
    assert!(ptr_decls.contains(&2));
    assert!(ptr_decls.contains(&4));

    let fn_decls = typed_indices(&typed, C_AST_KIND_FUNCTION_DECLARATOR);
    assert_eq!(
        fn_decls.len(),
        2,
        "must find two function declarators in nested fnptr"
    );
}
