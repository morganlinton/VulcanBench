//! Keyword promotion: identifier → keyword token id.

use super::tokens::*;

static KEYWORDS: &[(&str, u16)] = &[
    ("fn", KW_FN),
    ("let", KW_LET),
    ("mut", KW_MUT),
    ("if", KW_IF),
    ("else", KW_ELSE),
    ("return", KW_RETURN),
    ("while", KW_WHILE),
    ("for", KW_FOR),
    ("in", KW_IN),
    ("true", LITERAL_BOOL),
    ("false", LITERAL_BOOL),
    ("i32", KW_I32),
    ("bool", KW_BOOL),
];

/// Promote an identifier string to its keyword token id.
pub fn promote(ident: &str) -> Option<u16> {
    for (kw, tok) in KEYWORDS {
        if *kw == ident {
            return Some(*tok);
        }
    }
    None
}
