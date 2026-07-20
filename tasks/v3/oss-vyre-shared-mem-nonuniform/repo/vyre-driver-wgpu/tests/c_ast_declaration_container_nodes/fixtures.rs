// Integration test module for the containing Vyre package.

use super::support::starts_for_lens;
use vyre_libs::parsing::c::lex::tokens::*;

/// ```c
/// struct S { int x; };
/// ```
pub(crate) fn fixture_struct_definition() -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_STRUCT,
        TOK_IDENTIFIER, // S
        TOK_LBRACE,
        TOK_INT,
        TOK_IDENTIFIER, // x
        TOK_SEMICOLON,
        TOK_RBRACE,
        TOK_SEMICOLON,
    ];
    let tok_lens = vec![1; tok_types.len()];
    let tok_starts = starts_for_lens(&tok_lens);
    (tok_types, tok_starts, tok_lens)
}

/// ```c
/// struct S;
/// ```
pub(crate) fn fixture_struct_forward_declaration() -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let tok_types = vec![TOK_STRUCT, TOK_IDENTIFIER, TOK_SEMICOLON];
    let tok_lens = vec![1; tok_types.len()];
    let tok_starts = starts_for_lens(&tok_lens);
    (tok_types, tok_starts, tok_lens)
}

/// ```c
/// union U { int i; float f; };
/// ```
pub(crate) fn fixture_union_definition() -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_UNION,
        TOK_IDENTIFIER, // U
        TOK_LBRACE,
        TOK_INT,
        TOK_IDENTIFIER, // i
        TOK_SEMICOLON,
        TOK_FLOAT_KW,
        TOK_IDENTIFIER, // f
        TOK_SEMICOLON,
        TOK_RBRACE,
        TOK_SEMICOLON,
    ];
    let tok_lens = vec![1; tok_types.len()];
    let tok_starts = starts_for_lens(&tok_lens);
    (tok_types, tok_starts, tok_lens)
}

/// ```c
/// union U;
/// ```
pub(crate) fn fixture_union_forward_declaration() -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let tok_types = vec![TOK_UNION, TOK_IDENTIFIER, TOK_SEMICOLON];
    let tok_lens = vec![1; tok_types.len()];
    let tok_starts = starts_for_lens(&tok_lens);
    (tok_types, tok_starts, tok_lens)
}

/// ```c
/// enum E { A, B };
/// ```
pub(crate) fn fixture_enum_definition() -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_ENUM,
        TOK_IDENTIFIER, // E
        TOK_LBRACE,
        TOK_IDENTIFIER, // A
        TOK_COMMA,
        TOK_IDENTIFIER, // B
        TOK_RBRACE,
        TOK_SEMICOLON,
    ];
    let tok_lens = vec![1; tok_types.len()];
    let tok_starts = starts_for_lens(&tok_lens);
    (tok_types, tok_starts, tok_lens)
}

/// ```c
/// enum E;
/// ```
pub(crate) fn fixture_enum_forward_declaration() -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let tok_types = vec![TOK_ENUM, TOK_IDENTIFIER, TOK_SEMICOLON];
    let tok_lens = vec![1; tok_types.len()];
    let tok_starts = starts_for_lens(&tok_lens);
    (tok_types, tok_starts, tok_lens)
}

/// ```c
/// typedef int T;
/// ```
pub(crate) fn fixture_typedef_declaration() -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_TYPEDEF,
        TOK_INT,
        TOK_IDENTIFIER, // T
        TOK_SEMICOLON,
    ];
    let tok_lens = vec![1; tok_types.len()];
    let tok_starts = starts_for_lens(&tok_lens);
    (tok_types, tok_starts, tok_lens)
}

/// ```c
/// int f(void) { return 0; }
/// ```
pub(crate) fn fixture_function_definition() -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_INT,
        TOK_IDENTIFIER, // f
        TOK_LPAREN,
        TOK_VOID,
        TOK_RPAREN,
        TOK_LBRACE,
        TOK_RETURN,
        TOK_INTEGER, // 0
        TOK_SEMICOLON,
        TOK_RBRACE,
    ];
    let tok_lens = vec![1; tok_types.len()];
    let tok_starts = starts_for_lens(&tok_lens);
    (tok_types, tok_starts, tok_lens)
}

/// ```c
/// int f(void);
/// ```
pub(crate) fn fixture_function_prototype() -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_INT,
        TOK_IDENTIFIER, // f
        TOK_LPAREN,
        TOK_VOID,
        TOK_RPAREN,
        TOK_SEMICOLON,
    ];
    let tok_lens = vec![1; tok_types.len()];
    let tok_starts = starts_for_lens(&tok_lens);
    (tok_types, tok_starts, tok_lens)
}

/// ```c
/// struct { int a : 4; unsigned int : 0; };
/// ```
pub(crate) fn fixture_bitfield() -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_STRUCT,
        TOK_LBRACE,
        TOK_INT,
        TOK_IDENTIFIER, // a
        TOK_COLON,
        TOK_INTEGER, // 4
        TOK_SEMICOLON,
        TOK_UNSIGNED,
        TOK_INT,
        TOK_COLON,
        TOK_INTEGER, // 0
        TOK_SEMICOLON,
        TOK_RBRACE,
        TOK_SEMICOLON,
    ];
    let tok_lens = vec![1; tok_types.len()];
    let tok_starts = starts_for_lens(&tok_lens);
    (tok_types, tok_starts, tok_lens)
}

/// ```c
/// _Static_assert(1, "ok");
/// ```
pub(crate) fn fixture_static_assert() -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_STATIC_ASSERT,
        TOK_LPAREN,
        TOK_INTEGER, // 1
        TOK_COMMA,
        TOK_STRING, // "ok"
        TOK_RPAREN,
        TOK_SEMICOLON,
    ];
    let tok_lens = vec![1; tok_types.len()];
    let tok_starts = starts_for_lens(&tok_lens);
    (tok_types, tok_starts, tok_lens)
}
