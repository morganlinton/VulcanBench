//! Failure-oriented adversarial tests for matching primitives.
//!
//! Focus: hostile boundaries, overflow, invalid offsets, property invariants.
#![cfg(feature = "matching")]

use vyre_primitives::matching::{
    bracket_match::*, dfa_compile, dfa_compile_with_budget, DfaCompileError,
};

fn cpu_ref(kinds: &[u32], max_depth: u32) -> Vec<u32> {
    let mut pairs = vec![MATCH_NONE; kinds.len()];
    let mut stack = Vec::with_capacity(max_depth as usize);
    for (index, kind) in kinds.iter().copied().enumerate() {
        if kind == OPEN_BRACE {
            if stack.len() < max_depth as usize {
                stack.push(index as u32);
            }
        } else if kind == CLOSE_BRACE {
            if let Some(open) = stack.pop() {
                pairs[open as usize] = index as u32;
                pairs[index] = open;
            }
        }
    }
    pairs
}

#[test]
fn bracket_match_cpu_ref_empty_inputs() {
    let cases = [
        (vec![], 0, vec![]),
        (vec![], 1, vec![]),
        (vec![], 100, vec![]),
    ];
    for (kinds, max_depth, expected) in cases {
        let got = cpu_ref(&kinds, max_depth);
        assert_eq!(got, expected, "empty kinds must yield empty output");
    }
}

#[test]
fn bracket_match_cpu_ref_depth_zero_rejects_all_opens() {
    let kinds = vec![OPEN_BRACE, OPEN_BRACE, CLOSE_BRACE];
    let got = cpu_ref(&kinds, 0);
    assert_eq!(got, vec![MATCH_NONE, MATCH_NONE, MATCH_NONE]);
}

#[test]
fn bracket_match_cpu_ref_all_closes_with_empty_stack() {
    let got = cpu_ref(&[CLOSE_BRACE; 5], 10);
    assert_eq!(got, vec![MATCH_NONE; 5]);
}

#[test]
fn bracket_match_cpu_ref_overflow_length() {
    let n = 10_000usize;
    let kinds: Vec<u32> = (0..n)
        .map(|i| if i % 2 == 0 { OPEN_BRACE } else { CLOSE_BRACE })
        .collect();
    let got = cpu_ref(&kinds, n as u32);
    assert_eq!(got.len(), n);
    for i in (1..n).step_by(2) {
        assert_eq!(
            got[i],
            (i - 1) as u32,
            "close at {i} should match open at {}",
            i - 1
        );
    }
}

#[test]
fn bracket_match_cpu_ref_unbalanced_mixed() {
    let kinds = vec![
        OPEN_BRACE,
        OTHER,
        OPEN_BRACE,
        CLOSE_BRACE,
        CLOSE_BRACE,
        OTHER,
    ];
    let got = cpu_ref(&kinds, 10);
    assert_eq!(got, vec![4, MATCH_NONE, 3, 2, 0, MATCH_NONE]);
}

#[test]
fn dfa_compile_empty_patterns() {
    let dfa = dfa_compile(&[]);
    assert_eq!(dfa.state_count, 1);
    assert_eq!(dfa.transitions.len(), 256);
    assert!(dfa.accept.iter().all(|&a| a == 0));
}

#[test]
fn dfa_compile_single_byte_patterns() {
    let owned: Vec<Vec<u8>> = (0..=255).map(|b| vec![b]).collect();
    let patterns: Vec<&[u8]> = owned.iter().map(|v| v.as_slice()).collect();
    let dfa = dfa_compile(&patterns);
    for b in 0..=255u8 {
        let state = dfa.transitions[b as usize];
        assert!(dfa.accept[state as usize] != 0, "byte {b} must be accepted");
    }
}

#[test]
fn dfa_compile_budget_exhaustion() {
    let err = dfa_compile_with_budget(&[b"abcdefghijklmnopqrstuvwxyz"], 64).unwrap_err();
    assert!(matches!(
        err,
        DfaCompileError::TooLarge { .. } | DfaCompileError::TrieStateCapExceeded { .. }
    ));
}

#[test]
fn dfa_compile_zero_budget() {
    let err = dfa_compile_with_budget(&[b"a"], 0).unwrap_err();
    assert!(matches!(
        err,
        DfaCompileError::TrieStateCapExceeded { .. } | DfaCompileError::TooLarge { .. }
    ));
}

#[test]
fn dfa_compile_overlapping_patterns() {
    let patterns: [&[u8]; 4] = [b"he", b"she", b"his", b"hers"];
    let dfa = dfa_compile(&patterns);
    let mut state = 0u32;
    let mut matches = Vec::new();
    for &b in b"ushers" {
        state = dfa.transitions[(state as usize) * 256 + (b as usize)];
        let accept = dfa.accept[state as usize];
        if accept != 0 {
            matches.push(accept - 1);
        }
    }
    assert!(matches.contains(&1), "must accept `she`");
    assert!(
        matches.contains(&0) || matches.contains(&3),
        "must accept `he` or `hers`"
    );
}
