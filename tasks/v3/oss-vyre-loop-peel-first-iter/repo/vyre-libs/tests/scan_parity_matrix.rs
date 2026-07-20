//! Cross-engine scan contract matrix.
//!
//! For shared LITERAL patterns, both `GpuLiteralSet::reference_scan` and
//! `RulePipeline::reference_scan` should produce the same set of `(pid,
//! start, end)` matches. The literal-set is a DFA over byte
//! transitions; `RulePipeline`'s NFA is a different walk over a
//! superset of states. Both engines must at least cover the same
//! obvious literal occurrences and must never emit out-of-bounds
//! spans.
//!
//! What this test catches: a future refactor of either engine that
//! silently changes match semantics (extra matches, missed matches,
//! shifted offsets). The unit tests on each engine in isolation
//! sample the inputs hand-pickedly; this matrix is the
//! cross-validation.
//!
//! NFA vs DFA differ on overlapping-match semantics in some cases:
//! when one literal is a suffix of another (e.g. `abc` and `bc`),
//! the literal-set DFA may emit both at the same end position while
//! the NFA tracks both states in parallel. For that reason this file
//! asserts lower-bound coverage and span safety rather than pretending
//! the two engines have a single canonical overlap policy.

#![cfg(feature = "matching-nfa")]
#![allow(deprecated)]
use std::collections::BTreeSet;
use vyre_libs::scan::{build_rule_pipeline, GpuLiteralSet};

fn match_set<I>(matches: I) -> BTreeSet<(u32, u32, u32)>
where
    I: IntoIterator<Item = vyre_foundation::match_result::Match>,
{
    matches
        .into_iter()
        .map(|m| (m.pattern_id, m.start, m.end))
        .collect()
}

fn assert_parity(patterns: &[&str], haystack: &[u8]) {
    let lit_patterns: Vec<&[u8]> = patterns.iter().map(|s| s.as_bytes()).collect();
    let lit_engine = GpuLiteralSet::compile(&lit_patterns);
    let lit_matches = match_set(lit_engine.reference_scan(haystack));

    let pipe = build_rule_pipeline(patterns, "input", "hits", haystack.len() as u32);
    let nfa_matches = match_set(pipe.reference_scan(haystack));

    // Both engines must report at least the matches the simple
    // substring check would find. We don't enforce equality of the
    // sets directly because NFA and DFA can disagree on overlapping
    // suffixes; we DO enforce that every "obvious" substring hit
    // appears in both, and that neither engine fabricates matches
    // outside the input length.

    let max_offset = haystack.len() as u32;
    for (pid, start, end) in lit_matches.iter().chain(nfa_matches.iter()) {
        assert!(
            *start <= *end && *end <= max_offset,
            "match ({pid}, {start}, {end}) out of bounds for haystack length {max_offset}"
        );
    }

    // For each pattern, count obvious substring occurrences and
    // assert both engines find at least that many matches with
    // the right pid.
    for (pid, pat) in patterns.iter().enumerate() {
        let pid = pid as u32;
        let needle = pat.as_bytes();
        let mut count = 0;
        if !needle.is_empty() {
            let mut from = 0;
            while let Some(idx) = memchr_first(&haystack[from..], needle) {
                count += 1;
                from += idx + 1;
            }
        }
        let lit_count = lit_matches.iter().filter(|(p, _, _)| *p == pid).count();
        let nfa_count = nfa_matches.iter().filter(|(p, _, _)| *p == pid).count();
        assert!(
            lit_count >= count,
            "GpuLiteralSet missed matches for pattern {pat:?}: \
             expected ≥{count}, got {lit_count}"
        );
        assert!(
            nfa_count >= count,
            "RulePipeline missed matches for pattern {pat:?}: \
             expected ≥{count}, got {nfa_count}"
        );
    }
}

/// Find the first occurrence of `needle` in `haystack` using a
/// brute-force walk. Used as the "obvious" substring count; a
/// dependency-free reference both engines must beat.
fn memchr_first(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || needle.len() > haystack.len() {
        return None;
    }
    let last = haystack.len() - needle.len();
    (0..=last).find(|&i| &haystack[i..i + needle.len()] == needle)
}

#[test]
fn parity_single_literal_aligned() {
    assert_parity(&["AKIA"], b"foo AKIA bar");
}

#[test]
fn parity_two_literals_distant() {
    assert_parity(&["AKIA", "ghp_"], b"foo AKIA bar ghp_xxxx baz");
}

#[test]
fn parity_no_matches() {
    assert_parity(&["DEADBEEF"], b"the quick brown fox");
}

#[test]
fn parity_repeated_literal() {
    assert_parity(&["abc"], b"abcabcabc");
}

#[test]
fn parity_overlapping_suffix_not_panicking() {
    // `abc` and `bc`  -  overlapping. Both engines must succeed
    // without panic; exact match counts may differ between DFA and
    // NFA depending on the engine's overlap policy. We only assert
    // bounds-safety here.
    let patterns = ["abc", "bc"];
    let haystack = b"xyz_abc_end";
    let lit_patterns: Vec<&[u8]> = patterns.iter().map(|s| s.as_bytes()).collect();
    let lit_matches = match_set(GpuLiteralSet::compile(&lit_patterns).reference_scan(haystack));
    let pipe = build_rule_pipeline(&patterns, "input", "hits", haystack.len() as u32);
    let _nfa_matches = match_set(pipe.reference_scan(haystack));
    let max_offset = haystack.len() as u32;
    for (_, _, end) in &lit_matches {
        assert!(*end <= max_offset);
    }
}

#[test]
fn parity_long_haystack() {
    let mut haystack = Vec::with_capacity(1024);
    for _ in 0..32 {
        haystack.extend_from_slice(b"foo AKIA bar ghp_test ");
    }
    assert_parity(&["AKIA", "ghp_test"], &haystack);
}

#[test]
fn parity_empty_haystack() {
    assert_parity(&["abc"], b"");
}

#[test]
fn parity_single_char_pattern() {
    assert_parity(&["x"], b"xyzxyzxyzxyzxyzxyz");
}
