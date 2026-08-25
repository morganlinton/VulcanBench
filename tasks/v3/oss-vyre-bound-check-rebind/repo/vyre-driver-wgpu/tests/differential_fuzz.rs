//! P5.3  -  Differential fuzzer.
//!
//! Generates random Cat-A witness inputs, runs them through the
//! CPU reference interpreter, and asserts byte-identity against a
//! hand-written CPU oracle for each op. Any divergence is a P0
//! correctness bug. This fuzzer doesn't need a GPU adapter  -
//! backend-divergence testing lives in `vyre-driver-wgpu`'s
//! differential harness.
//!
//! The proptest corpus is bounded (ProptestConfig::with_cases(128)
//! per op) so CI pays a known per-PR cost. Nightly jobs bump the
//! case count for wider coverage.
//!
//! Scope today: substring_search, aho_corasick, dot, scan_prefix_sum.
//! Coverage grows as new Cat-A ops ship  -  each op's author adds a
//! proptest function here per AUTHORING.md step 4b.

#![allow(deprecated)]
#![cfg(all(
    feature = "math-linalg",
    feature = "math-scan",
    feature = "matching-substring",
    feature = "matching-dfa",
))]

mod common;

use common::{decode_u32_words, u32_bytes};
use proptest::prelude::*;
use vyre::ir::Program;
use vyre_reference::value::Value;

fn run(program: &Program, inputs: Vec<Vec<u8>>) -> Vec<Vec<u8>> {
    let values: Vec<Value> = inputs.into_iter().map(Value::from).collect();
    let outs = vyre_reference::reference_eval(program, &values).expect("execute");
    outs.into_iter().map(|v| v.to_bytes()).collect()
}

// ---------- Dot product ----------

fn cpu_dot(lhs: &[u32], rhs: &[u32]) -> u32 {
    lhs.iter()
        .zip(rhs.iter())
        .map(|(a, b)| a.wrapping_mul(*b))
        .fold(0u32, |acc, x| acc.wrapping_add(x))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn differential_dot(
        lhs in prop::collection::vec(any::<u32>(), 1..32),
        rhs_seed in any::<u64>(),
    ) {
        use vyre_libs::math::dot;
        let rhs: Vec<u32> = (0..lhs.len())
            .map(|i| ((rhs_seed.wrapping_mul(i as u64 + 1) ^ 0xdead_beef) as u32))
            .collect();
        let program = dot("a", "b", "c", lhs.len() as u32).unwrap();
        let outputs = run(
            &program,
            vec![u32_bytes(&lhs), u32_bytes(&rhs), vec![0u8; 4]],
        );
        let got = decode_u32_words(&outputs[0])[0];
        let expected = cpu_dot(&lhs, &rhs);
        prop_assert_eq!(got, expected, "dot diverged");
    }
}

// ---------- Scan prefix sum ----------

fn cpu_scan_prefix_sum(input: &[u32]) -> Vec<u32> {
    let mut acc = 0u32;
    input
        .iter()
        .map(|&v| {
            acc = acc.wrapping_add(v);
            acc
        })
        .collect()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn differential_scan_prefix_sum(input in prop::collection::vec(any::<u32>(), 1..64)) {
        use vyre_libs::math::scan_prefix_sum;
        let n = input.len() as u32;
        let program = scan_prefix_sum("input", "output", n);
        let outputs = run(
            &program,
            vec![u32_bytes(&input), vec![0u8; input.len() * 4]],
        );
        let got = decode_u32_words(&outputs[0]);
        let expected = cpu_scan_prefix_sum(&input);
        prop_assert_eq!(got, expected, "scan_prefix_sum diverged");
    }
}

// ---------- Substring search ----------

fn cpu_substring_search(haystack: &[u8], needle: &[u8]) -> Vec<u32> {
    let mut out = vec![0u32; haystack.len()];
    if needle.is_empty() {
        // Empty needle matches at every valid starting offset per
        // the builder's `needle_len <= haystack_len` convention  -
        // when needle_len is 0, every offset trivially matches.
        for m in out.iter_mut() {
            *m = 1;
        }
        return out;
    }
    if needle.len() > haystack.len() {
        return out;
    }
    for i in 0..=(haystack.len() - needle.len()) {
        if haystack[i..i + needle.len()] == *needle {
            out[i] = 1;
        }
    }
    out
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn differential_substring_search(
        haystack in "[a-e]{1,24}",
        needle in "[a-e]{1,4}",
    ) {
        use vyre_libs::scan::substring_search;
        let haystack_bytes = haystack.as_bytes();
        let needle_bytes = needle.as_bytes();
        let program = substring_search(
            "haystack",
            "needle",
            "matches",
            haystack_bytes.len() as u32,
            needle_bytes.len() as u32,
        );
        let outputs = run(
            &program,
            vec![
                u32_bytes(&haystack_bytes.iter().map(|&b| u32::from(b)).collect::<Vec<_>>()),
                u32_bytes(&needle_bytes.iter().map(|&b| u32::from(b)).collect::<Vec<_>>()),
                vec![0u8; haystack_bytes.len() * 4],
            ],
        );
        let got = decode_u32_words(&outputs[0]);
        let expected = cpu_substring_search(haystack_bytes, needle_bytes);
        prop_assert_eq!(got, expected, "substring_search diverged on haystack={:?} needle={:?}", haystack, needle);
    }
}

// ---------- Aho-Corasick ----------

fn cpu_aho_corasick(dfa: &vyre_libs::scan::CompiledDfa, haystack: &[u8]) -> Vec<u32> {
    let mut state = 0u32;
    let mut out = Vec::with_capacity(haystack.len());
    for &b in haystack {
        state = dfa.transitions[(state as usize) * 256 + b as usize];
        out.push(dfa.accept[state as usize]);
    }
    out
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn differential_aho_corasick(
        patterns in prop::collection::vec("[a-c]{1,4}", 1..6),
        haystack in "[a-c]{1,32}",
    ) {
        use vyre_libs::scan::{aho_corasick, dfa_compile};

        let pattern_bytes: Vec<&[u8]> = patterns.iter().map(|p| p.as_bytes()).collect();
        let compiled = dfa_compile(&pattern_bytes);

        let program = aho_corasick(
            "haystack",
            "transitions",
            "accept",
            "matches",
            haystack.len() as u32,
            compiled.accept.len() as u32,
        );
        let outputs = run(
            &program,
            vec![
                u32_bytes(&haystack.as_bytes().iter().map(|&b| u32::from(b)).collect::<Vec<_>>()),
                u32_bytes(&compiled.transitions),
                u32_bytes(&compiled.accept),
                vec![0u8; haystack.len() * 4],
            ],
        );
        let got = decode_u32_words(&outputs[0]);
        let expected = cpu_aho_corasick(&compiled, haystack.as_bytes());
        prop_assert_eq!(got, expected, "aho_corasick diverged on haystack={:?} patterns={:?}", haystack, patterns);
    }
}
