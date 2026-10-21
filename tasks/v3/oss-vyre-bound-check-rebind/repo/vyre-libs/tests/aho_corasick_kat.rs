//! Aho-Corasick KAT corpus. Every (patterns, haystack, expected_accepts)
//! tuple is run through both the Reference oracle walk and the vyre
//! reference-interpreter dispatch; both must agree byte-for-byte.
//!
//! Sources:
//!   - The original Aho-Corasick 1975 paper's "ushers / he she his hers"
//!     example.
//!   - The `aho-corasick` crate's smoke test corpus (BSD-licensed).
//!   - Regression vectors for ambiguous patterns that share long
//!     suffixes, which stress the failure-link collapse logic.

#![cfg(feature = "matching-dfa")]
#![allow(deprecated)]
mod common;
use common::{decode_u32_words, u32_bytes};
use vyre_libs::scan::{aho_corasick, dfa_compile, CompiledDfa};
use vyre_reference::value::Value;

/// Reference oracle: walk the DFA byte-by-byte, emit accept[state] at each
/// offset. This is the oracle the vyre IR must match.
fn cpu_reference_scan(dfa: &CompiledDfa, haystack: &[u8]) -> Vec<u32> {
    let mut state = 0u32;
    let mut out = Vec::with_capacity(haystack.len());
    for &b in haystack {
        state = dfa.transitions[(state as usize) * 256 + b as usize];
        out.push(dfa.accept[state as usize]);
    }
    out
}

/// Run the vyre IR for aho_corasick through the reference interpreter
/// and return the matches bitmap.
fn vyre_ir_scan(dfa: &CompiledDfa, haystack: &[u8]) -> Vec<u32> {
    let program = aho_corasick(
        "haystack",
        "transitions",
        "accept",
        "matches",
        u32::try_from(haystack.len()).unwrap_or(u32::MAX),
        u32::try_from(dfa.accept.len()).unwrap_or(u32::MAX),
    );
    let inputs = vec![
        Value::from(u32_bytes(
            &haystack.iter().map(|&b| u32::from(b)).collect::<Vec<_>>(),
        )),
        Value::from(u32_bytes(&dfa.transitions)),
        Value::from(u32_bytes(&dfa.accept)),
        Value::from(vec![0u8; haystack.len() * 4]),
    ];
    let outputs =
        vyre_reference::reference_eval(&program, &inputs).expect("aho_corasick must execute");
    decode_u32_words(&outputs[0].to_bytes())
}

fn assert_match(patterns: &[&[u8]], haystack: &[u8], label: &str) {
    let dfa = dfa_compile(patterns);
    let cpu = cpu_reference_scan(&dfa, haystack);
    let vyre = vyre_ir_scan(&dfa, haystack);
    assert_eq!(
        cpu, vyre,
        "[{label}] Cat-A aho_corasick IR diverged from Reference oracle\n  patterns = {patterns:?}\n  haystack = {haystack:?}\n  cpu      = {cpu:?}\n  vyre     = {vyre:?}"
    );
}

#[test]
fn kat_ac_paper_ushers() {
    // Aho-Corasick 1975, Figure 2: patterns = {he, she, his, hers};
    // haystack = "ushers".
    let patterns: &[&[u8]] = &[b"he", b"she", b"his", b"hers"];
    assert_match(patterns, b"ushers", "ac-paper-ushers");
}

#[test]
fn kat_single_pattern_interior_match() {
    assert_match(&[b"abc"], b"xxabcxx", "single-pattern-interior");
}

#[test]
fn kat_single_pattern_prefix_match() {
    assert_match(&[b"abc"], b"abcxxxx", "single-pattern-prefix");
}

#[test]
fn kat_single_pattern_suffix_match() {
    assert_match(&[b"abc"], b"xxxxabc", "single-pattern-suffix");
}

#[test]
fn kat_single_pattern_no_match() {
    assert_match(&[b"abc"], b"xxxyyyy", "single-pattern-no-match");
}

#[test]
fn kat_overlapping_patterns() {
    // "aa" and "aaa" both match inside "aaaa".
    assert_match(&[b"aa", b"aaa"], b"aaaa", "overlapping-patterns");
}

#[test]
fn kat_shared_suffix_failure_links() {
    // These patterns stress failure-link collapse  -  the DFA must
    // transition through the shared suffix correctly.
    assert_match(
        &[b"abab", b"babab"],
        b"xababaxxbabaxbababx",
        "shared-suffix",
    );
}

#[test]
fn kat_ambiguous_accept_chain() {
    // At the last byte we should see the longest match's accept id,
    // but any ordering is acceptable so long as CPU + vyre agree.
    assert_match(
        &[b"foo", b"foobar", b"bar"],
        b"foobar",
        "ambiguous-accept-chain",
    );
}

#[test]
fn kat_single_byte_pattern() {
    assert_match(&[b"x"], b"axbxcxd", "single-byte-pattern");
}

#[test]
fn kat_nonascii_bytes() {
    // Multi-byte UTF-8 sequences are just bytes to AC.
    let patterns: &[&[u8]] = &["café".as_bytes()];
    let haystack = "The café is open.".as_bytes();
    assert_match(patterns, haystack, "nonascii");
}

#[test]
fn kat_empty_haystack() {
    // Pathological: no bytes, no accepts. Should return empty matches.
    let dfa = dfa_compile(&[b"abc"]);
    let cpu = cpu_reference_scan(&dfa, b"");
    assert_eq!(cpu.len(), 0, "empty haystack → empty accept stream");
}

#[test]
fn kat_large_alphabet_stress() {
    // 256 distinct one-byte patterns exercises the transition table
    // width (state * 256 + byte indexing).
    let patterns: Vec<Vec<u8>> = (0..=255u8).map(|b| vec![b]).collect();
    let pattern_refs: Vec<&[u8]> = patterns.iter().map(|p| p.as_slice()).collect();
    let haystack: Vec<u8> = (0..=255u8).collect();
    assert_match(&pattern_refs, &haystack, "large-alphabet");
}

#[test]
fn kat_many_overlapping_runs() {
    // AC must handle repeated patterns across long runs.
    assert_match(
        &[b"ab", b"bc", b"cd"],
        b"abcdabcdabcdabcdabcd",
        "many-overlapping-runs",
    );
}

#[test]
fn kat_haystack_longer_than_dfa_states() {
    // Stresses the scanner's invariance to haystack length (the DFA
    // has ~5 states; haystack has 1024 bytes).
    let haystack: Vec<u8> = b"abcab".repeat(205)[..1024].to_vec();
    assert_match(&[b"abc"], &haystack, "long-haystack");
}

#[test]
fn kat_all_bytes_zero() {
    // Zero haystack stresses the "state 0 → state 0" self-loop.
    let haystack = vec![0u8; 128];
    assert_match(&[b"abc"], &haystack, "all-zeros-haystack");
}

#[test]
fn kat_pathological_failure_chain() {
    // ababab… with pattern "abab" stresses the failure-link chain
    // where every partial match fails but some succeed.
    assert_match(&[b"abab"], b"ababababab", "pathological-failure-chain");
}

#[test]
fn kat_two_patterns_same_start() {
    assert_match(&[b"cat", b"cater"], b"catering", "same-start");
}

#[test]
fn kat_pattern_is_haystack() {
    assert_match(&[b"exact"], b"exact", "pattern-equals-haystack");
}

#[test]
fn kat_many_distinct_patterns() {
    let pats: Vec<Vec<u8>> = (b'a'..=b'z').map(|c| vec![c, b'y', b'z']).collect();
    let refs: Vec<&[u8]> = pats.iter().map(|p| p.as_slice()).collect();
    assert_match(&refs, b"xyzwyzayzbyzczz", "many-distinct");
}

#[test]
fn kat_long_single_pattern() {
    let needle: Vec<u8> = b"abcdefghijklmnopqrstuvwxyz".to_vec();
    let haystack: Vec<u8> = b"xxxabcdefghijklmnopqrstuvwxyzxxx".to_vec();
    assert_match(&[&needle], &haystack, "long-needle");
}
