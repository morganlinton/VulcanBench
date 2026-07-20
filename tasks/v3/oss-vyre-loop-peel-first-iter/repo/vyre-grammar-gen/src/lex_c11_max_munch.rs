//! C11 host max-munch lexer using the **same** pattern list as
//! `c11_lexer::add_c11_patterns`, with [`regex::bytes::Regex`] (longest
//! match, tie → earlier pattern in [`C11_PATTERNS`]).
//!
//! This replaces hand-simulation over `DfaTable` for goldens: the DFA
//! `token_ids` do not by themselves record **where** a token ends; the
//! `regex` engine applies leftmost–longest matching correctly.

use std::sync::OnceLock;

use regex::bytes::Regex;

use crate::c11_lexer::{C11_PATTERNS, TOK_PREPROC};
use crate::max_munch_cpu::LexCpuError;

/// One anchored regex per [`C11_PATTERNS`] entry, plus parallel kind ids.
struct Compiled {
    re: Vec<Regex>,
    kinds: Vec<u32>,
}

static C11_COMPILED: OnceLock<Compiled> = OnceLock::new();

// INTENTIONAL: C11_PATTERNS is a compile-time constant. Any pattern that fails
// to compile is a programmer error; we must abort loudly rather than silently
// drop the failed pattern and produce invisible token-kind recall loss.
#[allow(clippy::panic, clippy::unwrap_used)]
fn c11_compiled() -> &'static Compiled {
    C11_COMPILED.get_or_init(|| {
        let mut re = Vec::with_capacity(C11_PATTERNS.len());
        let mut kinds = Vec::with_capacity(C11_PATTERNS.len());
        for &(kind, pat) in C11_PATTERNS {
            let anchored = format!("^({pat})");
            let regex = Regex::new(&anchored).unwrap_or_else(|e| {
                panic!(
                    "Fix: C11 lexer pattern for token kind {kind} failed to compile: {e}. \
                     C11_PATTERNS is a compile-time constant — fix the broken pattern. \
                     Silently dropping it would cause that token kind to be unrecognised, \
                     producing invisible recall loss."
                )
            });
            re.push(regex);
            kinds.push(kind);
        }
        // Invariant: every entry in C11_PATTERNS produced exactly one compiled regex.
        assert_eq!(
            re.len(),
            C11_PATTERNS.len(),
            "Fix: C11 lexer compiled {} patterns but C11_PATTERNS has {}; \
             one or more token kinds would be silently unrecognised.",
            re.len(),
            C11_PATTERNS.len()
        );
        Compiled { re, kinds }
    })
}

fn is_preproc_directive_position(input: &[u8], pos: usize) -> bool {
    let line_start = input[..pos]
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map_or(0, |idx| idx + 1);
    input[line_start..pos]
        .iter()
        .all(|byte| matches!(*byte, b' ' | b'\t' | b'\r' | 0x0b | 0x0c))
}

/// Maximal-munch over [`C11_PATTERNS`]: at each `pos`, the **longest** `^` match
/// among patterns wins; equal length → **earlier** pattern in the list.
pub fn lex_c11_max_munch_kinds(input: &[u8]) -> Result<Vec<u32>, LexCpuError> {
    let c = c11_compiled();
    let mut out = Vec::new();
    let mut pos = 0usize;
    while pos < input.len() {
        let rest = &input[pos..];
        let mut best: Option<(usize, usize, u32)> = None; // (len, pat_i, kind)
        for (i, regex) in c.re.iter().enumerate() {
            let kind = c.kinds[i];
            if kind == TOK_PREPROC && !is_preproc_directive_position(input, pos) {
                continue;
            }
            let Some(m) = regex.find(rest) else {
                continue;
            };
            if m.start() != 0 {
                continue;
            }
            let len = m.end();
            if len == 0 {
                continue;
            }
            match &best {
                None => best = Some((len, i, kind)),
                Some((ol, _oi, _)) if len > *ol => best = Some((len, i, kind)),
                Some((ol, oi, _)) if len == *ol && i < *oi => best = Some((len, i, kind)),
                _ => {}
            }
        }
        let (len, _, kind) = best.ok_or(LexCpuError::NoTokenAt { offset: pos })?;
        out.push(kind);
        pos = pos.saturating_add(len);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_line_is_whitespace_alone() {
        let v = lex_c11_max_munch_kinds(b"\n").expect("ok");
        assert_eq!(v, vec![201]);
    }

    #[test]
    fn hash_after_token_is_hash_punctuation_not_directive() {
        let v = lex_c11_max_munch_kinds(b"a # # b").expect("ok");
        assert_eq!(v, vec![1, 201, 33, 201, 33, 201, 1]);
    }

    #[test]
    fn hash_after_leading_line_whitespace_is_directive() {
        let v = lex_c11_max_munch_kinds(b"  #define X 1\nx").expect("ok");
        assert_eq!(v, vec![201, 202, 201, 1]);
    }

    #[test]
    fn c11_compiled_has_exactly_one_regex_per_pattern_no_silent_drop() {
        // Verify that c11_compiled() produced exactly one regex per C11_PATTERNS
        // entry. Before the fix, a failed regex compilation was silently skipped
        // (c11-regex-silent-drop); the token kind would never be recognised.
        // This test catches both the silent-drop path AND any future pattern
        // regression that causes a compile failure at init time.
        let c = c11_compiled();
        assert_eq!(
            c.re.len(),
            C11_PATTERNS.len(),
            "Fix: c11_compiled must produce exactly one regex per C11_PATTERNS entry. \
             Expected {} regexes, got {}; one or more token kinds are silently unrecognised.",
            C11_PATTERNS.len(),
            c.re.len()
        );
        assert_eq!(
            c.kinds.len(),
            C11_PATTERNS.len(),
            "Fix: c11_compiled kinds vec must have one entry per C11_PATTERNS entry. \
             Expected {}, got {}.",
            C11_PATTERNS.len(),
            c.kinds.len()
        );
        // Spot-check: the first entry in C11_PATTERNS maps to the exact token id
        // stored at position 0 in the compiled kinds vector.
        let (expected_kind, _) = C11_PATTERNS[0];
        assert_eq!(
            c.kinds[0],
            expected_kind,
            "Fix: compiled kinds[0] must match C11_PATTERNS[0] token id ({expected_kind}), \
             got {}. A pattern was dropped or reordered.",
            c.kinds[0]
        );
    }
}
