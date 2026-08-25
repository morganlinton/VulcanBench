// Integration test module for the containing Vyre package.

use vyre_libs::parsing::c::lex::tokens::*;

/// ```c
/// int a = ++x;
/// int b = y--;
/// int c = &z;
/// int d = *w;
/// int e = +v;
/// int f = -u;
/// int g = ~t;
/// int h = !s;
/// ```
pub(crate) fn fixture_unary_prefix_and_postfix() -> (Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_INT,
        TOK_IDENTIFIER,
        TOK_ASSIGN,
        TOK_INC,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
        TOK_INT,
        TOK_IDENTIFIER,
        TOK_ASSIGN,
        TOK_IDENTIFIER,
        TOK_DEC,
        TOK_SEMICOLON,
        TOK_INT,
        TOK_IDENTIFIER,
        TOK_ASSIGN,
        TOK_AMP,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
        TOK_INT,
        TOK_IDENTIFIER,
        TOK_ASSIGN,
        TOK_STAR,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
        TOK_INT,
        TOK_IDENTIFIER,
        TOK_ASSIGN,
        TOK_PLUS,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
        TOK_INT,
        TOK_IDENTIFIER,
        TOK_ASSIGN,
        TOK_MINUS,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
        TOK_INT,
        TOK_IDENTIFIER,
        TOK_ASSIGN,
        TOK_TILDE,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
        TOK_INT,
        TOK_IDENTIFIER,
        TOK_ASSIGN,
        TOK_BANG,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
    ];
    let tok_lens = vec![1; tok_types.len()];
    (tok_types, tok_lens)
}

/// ```c
/// int a = (int)x;
/// ```
pub(crate) fn fixture_cast_expr() -> (Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_INT,
        TOK_IDENTIFIER,
        TOK_ASSIGN,
        TOK_LPAREN,
        TOK_INT,
        TOK_RPAREN,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
    ];
    let tok_lens = vec![1; tok_types.len()];
    (tok_types, tok_lens)
}

/// ```c
/// int a = s.m;
/// int b = p->m;
/// ```
pub(crate) fn fixture_member_access() -> (Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_INT,
        TOK_IDENTIFIER,
        TOK_ASSIGN,
        TOK_IDENTIFIER,
        TOK_DOT,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
        TOK_INT,
        TOK_IDENTIFIER,
        TOK_ASSIGN,
        TOK_IDENTIFIER,
        TOK_ARROW,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
    ];
    let tok_lens = vec![1; tok_types.len()];
    (tok_types, tok_lens)
}

/// ```c
/// int a = arr[0];
/// ```
pub(crate) fn fixture_array_subscript() -> (Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_INT,
        TOK_IDENTIFIER,
        TOK_ASSIGN,
        TOK_IDENTIFIER,
        TOK_LBRACKET,
        TOK_INTEGER,
        TOK_RBRACKET,
        TOK_SEMICOLON,
    ];
    let tok_lens = vec![1; tok_types.len()];
    (tok_types, tok_lens)
}

/// ```c
/// struct S s = { .x = 1, [0] = 2 };
/// ```
pub(crate) fn fixture_designated_initializer() -> (Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_STRUCT,
        TOK_IDENTIFIER,
        TOK_IDENTIFIER,
        TOK_ASSIGN,
        TOK_LBRACE,
        TOK_DOT,
        TOK_IDENTIFIER,
        TOK_ASSIGN,
        TOK_INTEGER,
        TOK_COMMA,
        TOK_LBRACKET,
        TOK_INTEGER,
        TOK_RBRACKET,
        TOK_ASSIGN,
        TOK_INTEGER,
        TOK_RBRACE,
        TOK_SEMICOLON,
    ];
    let tok_lens = vec![1; tok_types.len()];
    (tok_types, tok_lens)
}

/// ```c
/// int a[] = { [0 ... 1] = 2 };
/// ```
pub(crate) fn fixture_array_range_designator() -> (Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_INT,
        TOK_IDENTIFIER,
        TOK_LBRACKET,
        TOK_RBRACKET,
        TOK_ASSIGN,
        TOK_LBRACE,
        TOK_LBRACKET,
        TOK_INTEGER,
        TOK_ELLIPSIS,
        TOK_INTEGER,
        TOK_RBRACKET,
        TOK_ASSIGN,
        TOK_INTEGER,
        TOK_RBRACE,
        TOK_SEMICOLON,
    ];
    let tok_lens = vec![1; tok_types.len()];
    (tok_types, tok_lens)
}

/// ```c
/// switch(x) { case 1 ... 5: break; }
/// ```
pub(crate) fn fixture_gnu_case_range() -> (Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_SWITCH,
        TOK_LPAREN,
        TOK_IDENTIFIER,
        TOK_RPAREN,
        TOK_LBRACE,
        TOK_CASE,
        TOK_INTEGER,
        TOK_ELLIPSIS,
        TOK_INTEGER,
        TOK_COLON,
        TOK_BREAK,
        TOK_SEMICOLON,
        TOK_RBRACE,
    ];
    let tok_lens = vec![1; tok_types.len()];
    (tok_types, tok_lens)
}
