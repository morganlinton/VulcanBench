//! Shared token predicates for Go structural extraction kernels.

use crate::parsing::go::lex::TOK_IDENTIFIER;
use vyre::ir::Expr;

/// Test a token kind at `idx`.
pub(super) fn token_type_eq(tok_types: &str, idx: Expr, token: u32) -> Expr {
    Expr::eq(Expr::load(tok_types, idx), Expr::u32(token))
}

/// Load a token start offset.
pub(super) fn token_start(tok_starts: &str, idx: Expr) -> Expr {
    Expr::load(tok_starts, idx)
}

/// Load a token byte length.
pub(super) fn token_len(tok_lens: &str, idx: Expr) -> Expr {
    Expr::load(tok_lens, idx)
}

/// Compare a token's source bytes against a static byte string.
pub(super) fn token_bytes_eq(
    haystack: &str,
    tok_starts: &str,
    tok_lens: &str,
    idx: Expr,
    needle: &[u8],
) -> Expr {
    let mut expr = Expr::eq(
        token_len(tok_lens, idx.clone()),
        Expr::u32(needle.len() as u32),
    );
    for (offset, byte) in needle.iter().enumerate() {
        expr = Expr::and(
            expr,
            Expr::eq(
                Expr::bitand(
                    Expr::load(
                        haystack,
                        Expr::add(
                            token_start(tok_starts, idx.clone()),
                            Expr::u32(offset as u32),
                        ),
                    ),
                    Expr::u32(0xFF),
                ),
                Expr::u32(u32::from(*byte)),
            ),
        );
    }
    expr
}

/// Test whether a token is an identifier.
pub(super) fn token_is_ident(tok_types: &str, idx: Expr) -> Expr {
    token_type_eq(tok_types, idx, TOK_IDENTIFIER)
}

/// Test whether an identifier token matches a keyword spelling.
pub(super) fn token_is_keyword(
    haystack: &str,
    tok_types: &str,
    tok_starts: &str,
    tok_lens: &str,
    idx: Expr,
    keyword: &[u8],
) -> Expr {
    Expr::and(
        token_is_ident(tok_types, idx.clone()),
        token_bytes_eq(haystack, tok_starts, tok_lens, idx, keyword),
    )
}
