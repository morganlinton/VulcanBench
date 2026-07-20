//! Preprocessing-token synthesis helpers for macro `#` and `##`.

use crate::parsing::c::lex::tokens::{
    TOK_AMP, TOK_AMP_EQ, TOK_AND, TOK_ARROW, TOK_ASSIGN, TOK_BANG, TOK_CARET, TOK_CARET_EQ,
    TOK_DEC, TOK_DOT, TOK_EQ, TOK_FLOAT, TOK_GE, TOK_GT, TOK_HASH, TOK_HASHHASH, TOK_IDENTIFIER,
    TOK_INC, TOK_INTEGER, TOK_LE, TOK_LSHIFT, TOK_LSHIFT_EQ, TOK_LT, TOK_MINUS, TOK_MINUS_EQ,
    TOK_NE, TOK_OR, TOK_PERCENT, TOK_PERCENT_EQ, TOK_PIPE, TOK_PIPE_EQ, TOK_PLUS, TOK_PLUS_EQ,
    TOK_RSHIFT, TOK_RSHIFT_EQ, TOK_SLASH, TOK_SLASH_EQ, TOK_STAR, TOK_STAR_EQ, TOK_STRING,
};

/// `(left, right, synthesized)` token-type rules for bounded `##` lowering.
///
/// These rules intentionally cover token kinds whose spelling is recoverable
/// from the compact token stream. Cases that need raw spelling bytes are left
/// unresolved so callers can fail loudly instead of preserving unsynthesized punctuation.
pub const C_TOKEN_PASTE_RULES: &[(u32, u32, u32)] = &[
    (TOK_HASH, TOK_HASH, TOK_HASHHASH),
    (TOK_PLUS, TOK_PLUS, TOK_INC),
    (TOK_MINUS, TOK_MINUS, TOK_DEC),
    (TOK_MINUS, TOK_GT, TOK_ARROW),
    (TOK_LT, TOK_LT, TOK_LSHIFT),
    (TOK_GT, TOK_GT, TOK_RSHIFT),
    (TOK_LT, TOK_ASSIGN, TOK_LE),
    (TOK_GT, TOK_ASSIGN, TOK_GE),
    (TOK_ASSIGN, TOK_ASSIGN, TOK_EQ),
    (TOK_BANG, TOK_ASSIGN, TOK_NE),
    (TOK_AMP, TOK_AMP, TOK_AND),
    (TOK_PIPE, TOK_PIPE, TOK_OR),
    (TOK_PLUS, TOK_ASSIGN, TOK_PLUS_EQ),
    (TOK_MINUS, TOK_ASSIGN, TOK_MINUS_EQ),
    (TOK_STAR, TOK_ASSIGN, TOK_STAR_EQ),
    (TOK_SLASH, TOK_ASSIGN, TOK_SLASH_EQ),
    (TOK_PERCENT, TOK_ASSIGN, TOK_PERCENT_EQ),
    (TOK_AMP, TOK_ASSIGN, TOK_AMP_EQ),
    (TOK_PIPE, TOK_ASSIGN, TOK_PIPE_EQ),
    (TOK_CARET, TOK_ASSIGN, TOK_CARET_EQ),
    (TOK_LSHIFT, TOK_ASSIGN, TOK_LSHIFT_EQ),
    (TOK_RSHIFT, TOK_ASSIGN, TOK_RSHIFT_EQ),
    (TOK_DOT, TOK_INTEGER, TOK_FLOAT),
    (TOK_INTEGER, TOK_DOT, TOK_FLOAT),
    (TOK_INTEGER, TOK_INTEGER, TOK_INTEGER),
    (TOK_INTEGER, TOK_IDENTIFIER, TOK_INTEGER),
    (TOK_INTEGER, TOK_FLOAT, TOK_FLOAT),
    (TOK_FLOAT, TOK_IDENTIFIER, TOK_FLOAT),
    (TOK_FLOAT, TOK_INTEGER, TOK_FLOAT),
    (TOK_IDENTIFIER, TOK_IDENTIFIER, TOK_IDENTIFIER),
    (TOK_IDENTIFIER, TOK_INTEGER, TOK_IDENTIFIER),
];

/// Return the compact token kind synthesized by `left ## right`.
#[must_use]
pub fn synthesize_token_paste_type(left: u32, right: u32) -> Option<u32> {
    C_TOKEN_PASTE_RULES
        .iter()
        .find_map(|(lhs, rhs, out)| (*lhs == left && *rhs == right).then_some(*out))
}

/// Return whether `# parameter` can be represented in the current compact ABI.
#[must_use]
pub const fn stringification_token_type() -> u32 {
    TOK_STRING
}
