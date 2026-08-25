//! Differential oracle support: validate the reusable Rust lexer substrate
//! against `rustc_lexer` at the byte level.
//!
//! The substrate is a cooked, keyword-aware lexer (`==` -> EQ, `->` -> ARROW,
//! `&mut` -> AMP_MUT, keywords promoted to KW_*), while `rustc_lexer` is a raw
//! lexer (single-char punctuation, every keyword is an `Ident`). Comparing
//! token kinds directly is therefore unsound: it would mismatch on every
//! keyword and multi-char operator. Instead we assert the strongest
//! granularity-independent property: both lexers agree, byte-for-byte and in
//! order, on which bytes are content (non-trivia), and the substrate never
//! introduces a token boundary where rustc has none. This catches dropped,
//! spurious, or mis-spanned tokens without coupling to either tokenizer's
//! granularity.

use std::collections::BTreeSet;

use rustc_lexer::TokenKind;
use vyre_libs::parsing::rust::lex::lexer::core::lex;
use vyre_libs::parsing::rust::lex::tokens as tok;

/// Result of an oracle comparison.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum OracleResult {
    /// Both lexers agree on the content byte stream and boundaries.
    Match,
    /// Divergence, with a human-readable reason.
    Mismatch(String),
}

/// Compare the reusable lexer against `rustc_lexer` by content-byte agreement.
pub(crate) fn lexer_parity(source: &[u8]) -> OracleResult {
    let src = match std::str::from_utf8(source) {
        Ok(s) => s,
        Err(_) => return OracleResult::Mismatch("source is not valid UTF-8".into()),
    };

    let ours = match lex(source) {
        Ok(t) => t,
        Err(off) => return OracleResult::Mismatch(format!("substrate lexer failed at byte {off}")),
    };

    // Substrate content tokens (drop the synthetic EOF), as (start, text).
    let our_spans: Vec<(usize, &str)> = ours
        .iter()
        .filter(|t| t.kind != tok::EOF)
        .map(|t| {
            let s = t.start as usize;
            let e = s + t.len as usize;
            (s, &src[s..e])
        })
        .collect();

    // rustc raw tokens, dropping trivia (whitespace / comments), as (start, text).
    let mut rustc_spans: Vec<(usize, &str)> = Vec::new();
    let mut pos = 0usize;
    for t in rustc_lexer::tokenize(src) {
        let len = t.len as usize;
        let s = pos;
        pos += len;
        if matches!(
            t.kind,
            TokenKind::Whitespace | TokenKind::LineComment | TokenKind::BlockComment { .. }
        ) {
            continue;
        }
        rustc_spans.push((s, &src[s..s + len]));
    }

    // Granularity-independent invariant: the concatenated content byte streams
    // agree in order. The substrate may coalesce raw tokens (==, ->, &mut), so
    // we require equal content bytes, not equal token counts.
    let our_concat: String = our_spans.iter().map(|(_, t)| *t).collect();
    let rustc_concat: String = rustc_spans.iter().map(|(_, t)| *t).collect();
    if our_concat != rustc_concat {
        return OracleResult::Mismatch(format!(
            "content byte streams differ:\n  substrate: {our_concat:?}\n  rustc:     {rustc_concat:?}"
        ));
    }

    // The substrate may merge raw tokens but must never split where rustc has
    // no boundary: every substrate token start must be a rustc token start.
    let rustc_starts: BTreeSet<usize> = rustc_spans.iter().map(|(s, _)| *s).collect();
    for (s, t) in &our_spans {
        if !rustc_starts.contains(s) {
            return OracleResult::Mismatch(format!(
                "substrate token {t:?} starts at byte {s}, which is not a rustc token boundary"
            ));
        }
    }

    OracleResult::Match
}
