//! Expression helpers for the Rust GPU lexer program.

use vyre::ir::Expr;

use crate::parsing::rust::lex::tokens::*;

pub(super) fn keyword_or_ident_until(
    haystack: &str,
    source_end: Expr,
    pos: Expr,
    len: Expr,
) -> Expr {
    let mut out = Expr::u32(u32::from(IDENT));
    for &(text, kind) in &[
        (b"fn".as_slice(), KW_FN),
        (b"let".as_slice(), KW_LET),
        (b"mut".as_slice(), KW_MUT),
        (b"if".as_slice(), KW_IF),
        (b"else".as_slice(), KW_ELSE),
        (b"return".as_slice(), KW_RETURN),
        (b"while".as_slice(), KW_WHILE),
        (b"for".as_slice(), KW_FOR),
        (b"in".as_slice(), KW_IN),
        (b"true".as_slice(), LITERAL_BOOL),
        (b"false".as_slice(), LITERAL_BOOL),
        (b"i32".as_slice(), KW_I32),
        (b"bool".as_slice(), KW_BOOL),
    ] {
        out = Expr::select(
            bytes_eq_until(haystack, source_end.clone(), pos.clone(), len.clone(), text),
            Expr::u32(u32::from(kind)),
            out,
        );
    }
    out
}

fn bytes_eq_until(haystack: &str, source_end: Expr, pos: Expr, len: Expr, text: &[u8]) -> Expr {
    let mut cond = Expr::eq(len, Expr::u32(text.len() as u32));
    for (offset, byte) in text.iter().enumerate() {
        cond = Expr::and(
            cond,
            byte_eq(
                byte_before_or_zero(
                    haystack,
                    source_end.clone(),
                    Expr::add(pos.clone(), Expr::u32(offset as u32)),
                ),
                *byte,
            ),
        );
    }
    cond
}

pub(super) fn byte_load(haystack: &str, index: Expr) -> Expr {
    Expr::load(haystack, index)
}

pub(super) fn byte_before_or_zero(haystack: &str, source_end: Expr, index: Expr) -> Expr {
    Expr::select(
        Expr::lt(index.clone(), source_end),
        byte_load(haystack, index),
        Expr::u32(0),
    )
}

pub(super) fn byte_eq(value: Expr, byte: u8) -> Expr {
    Expr::eq(value, Expr::u32(u32::from(byte)))
}

pub(super) fn is_ascii_whitespace(value: Expr) -> Expr {
    Expr::or(
        byte_eq(value.clone(), b' '),
        Expr::or(
            byte_eq(value.clone(), b'\n'),
            Expr::or(byte_eq(value.clone(), b'\r'), byte_eq(value, b'\t')),
        ),
    )
}

pub(super) fn is_digit(value: Expr) -> Expr {
    Expr::and(
        Expr::ge(value.clone(), Expr::u32(u32::from(b'0'))),
        Expr::le(value, Expr::u32(u32::from(b'9'))),
    )
}

pub(super) fn is_ident_start(value: Expr) -> Expr {
    Expr::or(byte_eq(value.clone(), b'_'), is_alpha(value))
}

pub(super) fn is_ident_continue(value: Expr) -> Expr {
    Expr::or(is_ident_start(value.clone()), is_digit(value))
}

fn is_alpha(value: Expr) -> Expr {
    Expr::or(
        Expr::and(
            Expr::ge(value.clone(), Expr::u32(u32::from(b'a'))),
            Expr::le(value.clone(), Expr::u32(u32::from(b'z'))),
        ),
        Expr::and(
            Expr::ge(value.clone(), Expr::u32(u32::from(b'A'))),
            Expr::le(value, Expr::u32(u32::from(b'Z'))),
        ),
    )
}

pub(super) fn unhandled() -> Expr {
    Expr::eq(Expr::var("handled"), Expr::u32(0))
}
