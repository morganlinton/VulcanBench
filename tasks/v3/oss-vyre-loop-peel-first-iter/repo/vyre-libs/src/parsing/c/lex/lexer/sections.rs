//! Extracted `c11_lexer` body sub-builders. Each function returns a
//! `Vec<Node>` to be appended to the `classify_at_pos` accumulator
//! inside [`super::core::c11_lexer`]. Splitting these out keeps
//! `core.rs` under the 500-LOC source-file cap.

use vyre::ir::{Expr, Node};

use crate::parsing::c::lex::tokens::*;

use super::helpers::{byte_at_or_zero, byte_eq, set_token};

/// Operator + multi-byte punctuation table (LSHIFT_EQ, RSHIFT_EQ,
/// ARROW, INC/DEC, all the `?_EQ` operators, the doubled `&&/||/<</>>`
/// pairs, the `...` ellipsis, every single-byte punctuation token, and
/// the standalone `#` token). All entries are `set_token` predicates
/// that read the runtime `byte`/`next_byte`/`next2_byte` IR variables.
#[must_use]
pub(super) fn operator_punct_pushes() -> Vec<Node> {
    // Pre-size  -  the table emits ~45 set_token nodes (2 three-byte
    // ops + 20 two-byte ops + 1 ellipsis + ~22 single-byte tokens).
    // Capacity 64 fits with headroom and avoids 6 grow-by-doublings
    // while the lexer is being assembled.
    let mut nodes = Vec::with_capacity(64);
    for (token, first, second, third) in [
        (TOK_LSHIFT_EQ, b'<', b'<', b'='),
        (TOK_RSHIFT_EQ, b'>', b'>', b'='),
    ] {
        nodes.push(set_token(
            Expr::and(
                Expr::and(
                    byte_eq(Expr::var("byte"), first),
                    byte_eq(Expr::var("next_byte"), second),
                ),
                byte_eq(Expr::var("next2_byte"), third),
            ),
            token,
            Expr::u32(3),
        ));
    }

    for (token, first, second) in [
        (TOK_ARROW, b'-', b'>'),
        (TOK_INC, b'+', b'+'),
        (TOK_DEC, b'-', b'-'),
        (TOK_PLUS_EQ, b'+', b'='),
        (TOK_MINUS_EQ, b'-', b'='),
        (TOK_STAR_EQ, b'*', b'='),
        (TOK_SLASH_EQ, b'/', b'='),
        (TOK_PERCENT_EQ, b'%', b'='),
        (TOK_AMP_EQ, b'&', b'='),
        (TOK_PIPE_EQ, b'|', b'='),
        (TOK_CARET_EQ, b'^', b'='),
        (TOK_HASHHASH, b'#', b'#'),
        (TOK_EQ, b'=', b'='),
        (TOK_NE, b'!', b'='),
        (TOK_LE, b'<', b'='),
        (TOK_GE, b'>', b'='),
        (TOK_AND, b'&', b'&'),
        (TOK_OR, b'|', b'|'),
        (TOK_LSHIFT, b'<', b'<'),
        (TOK_RSHIFT, b'>', b'>'),
    ] {
        nodes.push(set_token(
            Expr::and(
                byte_eq(Expr::var("byte"), first),
                byte_eq(Expr::var("next_byte"), second),
            ),
            token,
            Expr::u32(2),
        ));
    }

    nodes.push(set_token(
        Expr::and(
            Expr::and(
                byte_eq(Expr::var("byte"), b'.'),
                byte_eq(Expr::var("next_byte"), b'.'),
            ),
            byte_eq(Expr::var("next2_byte"), b'.'),
        ),
        TOK_ELLIPSIS,
        Expr::u32(3),
    ));

    for (token, byte) in [
        (TOK_LPAREN, b'('),
        (TOK_RPAREN, b')'),
        (TOK_LBRACE, b'{'),
        (TOK_RBRACE, b'}'),
        (TOK_LBRACKET, b'['),
        (TOK_RBRACKET, b']'),
        (TOK_SEMICOLON, b';'),
        (TOK_COMMA, b','),
        (TOK_DOT, b'.'),
        (TOK_PLUS, b'+'),
        (TOK_MINUS, b'-'),
        (TOK_STAR, b'*'),
        (TOK_SLASH, b'/'),
        (TOK_PERCENT, b'%'),
        (TOK_AMP, b'&'),
        (TOK_PIPE, b'|'),
        (TOK_CARET, b'^'),
        (TOK_TILDE, b'~'),
        (TOK_BANG, b'!'),
        (TOK_ASSIGN, b'='),
        (TOK_LT, b'<'),
        (TOK_GT, b'>'),
        (TOK_QUESTION, b'?'),
        (TOK_COLON, b':'),
    ] {
        nodes.push(set_token(
            byte_eq(Expr::var("byte"), byte),
            token,
            Expr::u32(1),
        ));
    }
    nodes.push(set_token(
        byte_eq(Expr::var("byte"), b'#'),
        TOK_HASH,
        Expr::u32(1),
    ));
    nodes
}

/// Store-token + line-allows-directive update + cursor-advance epilogue.
/// This is the per-iteration tail of the lexer's `classify_at_pos` Vec
///  -  it persists the classified token to the output buffers and moves
/// `cursor` by `tok_len` (or 1 if no token was emitted).
#[must_use]
pub(super) fn store_token_and_advance_pushes(
    haystack: &str,
    haystack_len: u32,
    out_tok_types: &str,
    out_tok_starts: &str,
    out_tok_lens: &str,
) -> Vec<Node> {
    vec![
        Node::let_bind(
            "store_token",
            Expr::and(
                Expr::eq(Expr::var("emit"), Expr::u32(1)),
                Expr::and(
                    Expr::ne(Expr::var("tok_type"), Expr::u32(TOK_WHITESPACE)),
                    Expr::ne(Expr::var("tok_type"), Expr::u32(TOK_COMMENT)),
                ),
            ),
        ),
        Node::if_then(
            Expr::var("store_token"),
            vec![
                Node::store(out_tok_types, Expr::var("tok_idx"), Expr::var("tok_type")),
                Node::store(out_tok_starts, Expr::var("tok_idx"), Expr::var("pos")),
                Node::store(out_tok_lens, Expr::var("tok_idx"), Expr::var("tok_len")),
                Node::assign("tok_idx", Expr::add(Expr::var("tok_idx"), Expr::u32(1))),
            ],
        ),
        Node::let_bind(
            "tok_last_byte",
            byte_at_or_zero(
                haystack,
                Expr::add(
                    Expr::var("pos"),
                    Expr::saturating_sub(Expr::var("tok_len"), Expr::u32(1)),
                ),
                haystack_len,
            ),
        ),
        Node::if_then_else(
            Expr::eq(Expr::var("tok_type"), Expr::u32(TOK_PREPROC)),
            vec![Node::assign("line_allows_directive", Expr::u32(1))],
            vec![Node::if_then_else(
                Expr::or(
                    byte_eq(Expr::var("byte"), b'\n'),
                    Expr::or(
                        byte_eq(Expr::var("byte"), b'\r'),
                        Expr::or(
                            byte_eq(Expr::var("tok_last_byte"), b'\n'),
                            byte_eq(Expr::var("tok_last_byte"), b'\r'),
                        ),
                    ),
                ),
                vec![Node::assign("line_allows_directive", Expr::u32(1))],
                vec![Node::if_then(
                    Expr::not(Expr::and(
                        Expr::eq(Expr::var("line_allows_directive"), Expr::u32(1)),
                        Expr::or(
                            byte_eq(Expr::var("byte"), b' '),
                            byte_eq(Expr::var("byte"), b'\t'),
                        ),
                    )),
                    vec![Node::assign("line_allows_directive", Expr::u32(0))],
                )],
            )],
        ),
        Node::assign(
            "cursor",
            Expr::add(
                Expr::var("cursor"),
                Expr::select(
                    Expr::eq(Expr::var("emit"), Expr::u32(1)),
                    Expr::var("tok_len"),
                    Expr::u32(1),
                ),
            ),
        ),
    ]
}
