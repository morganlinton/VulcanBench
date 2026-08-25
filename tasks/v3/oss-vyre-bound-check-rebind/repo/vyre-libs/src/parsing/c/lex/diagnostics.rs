//! Diagnostics for C lexer error tokens.

use crate::parsing::c::lex::tokens::{
    TOK_ERR_INVALID_ESCAPE, TOK_ERR_UNTERMINATED_CHAR, TOK_ERR_UNTERMINATED_COMMENT,
    TOK_ERR_UNTERMINATED_STRING,
};

/// Diagnostic category encoded by a C lexer error token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum C11LexerDiagnosticKind {
    /// A string literal reached a physical newline or end of input before a closing quote.
    UnterminatedString,
    /// A character literal reached a physical newline or end of input before a closing quote.
    UnterminatedChar,
    /// A `/* ... */` block comment reached end of input before `*/`.
    UnterminatedBlockComment,
    /// A string or character literal contains an escape sequence outside C/GNU C's valid forms.
    InvalidEscape,
}

impl C11LexerDiagnosticKind {
    /// Returns the diagnostic kind for an encoded lexer error token.
    #[must_use]
    pub fn from_token(token: u32) -> Option<Self> {
        match token {
            TOK_ERR_UNTERMINATED_STRING => Some(Self::UnterminatedString),
            TOK_ERR_UNTERMINATED_CHAR => Some(Self::UnterminatedChar),
            TOK_ERR_UNTERMINATED_COMMENT => Some(Self::UnterminatedBlockComment),
            TOK_ERR_INVALID_ESCAPE => Some(Self::InvalidEscape),
            _ => None,
        }
    }
}

/// A source-positioned lexer diagnostic decoded from the compact token stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct C11LexerDiagnostic {
    /// Diagnostic category.
    pub kind: C11LexerDiagnosticKind,
    /// Index in the emitted compact token stream.
    pub token_index: u32,
    /// Byte offset where the malformed token starts.
    pub byte_start: u32,
    /// Number of source bytes consumed by the malformed token.
    pub byte_len: u32,
}

/// Returns the first encoded lexer diagnostic in a compact token stream.
#[must_use]
pub fn first_c11_lexer_diagnostic(
    tok_types: &[u32],
    tok_starts: &[u32],
    tok_lens: &[u32],
) -> Option<C11LexerDiagnostic> {
    let limit = tok_types.len().min(tok_starts.len()).min(tok_lens.len());
    tok_types
        .iter()
        .take(limit)
        .enumerate()
        .find_map(|(idx, token)| {
            C11LexerDiagnosticKind::from_token(*token).map(|kind| C11LexerDiagnostic {
                kind,
                token_index: idx as u32,
                byte_start: tok_starts[idx],
                byte_len: tok_lens[idx],
            })
        })
}
