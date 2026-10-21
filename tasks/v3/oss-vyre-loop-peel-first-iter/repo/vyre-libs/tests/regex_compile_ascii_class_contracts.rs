//! Regex compile ASCII class contracts.
#![cfg(feature = "matching-regex")]

use vyre_libs::scan::{compile_regex_set, RegexCompileError};
use vyre_primitives::nfa::subgroup_nfa::LANES_PER_SUBGROUP;

fn transition_mask(compiled: &vyre_libs::scan::CompiledRegexSet, state: u32, byte: u8) -> u32 {
    let idx = state as usize * 256 * LANES_PER_SUBGROUP + byte as usize * LANES_PER_SUBGROUP;
    compiled.transition_table[idx]
}

fn state_bit(state: u32) -> u32 {
    1u32 << (state % 32)
}

#[test]
fn digit_escape_compiles_to_ascii_digit_byte_set() {
    let compiled = compile_regex_set(&["\\d"]).expect("ASCII digit class must compile");
    assert_eq!(compiled.plan.num_states, 3);
    assert_eq!(compiled.plan.accept_states, vec![(0, 1)]);
    assert_eq!(compiled.plan.accept_state_ids, vec![2]);

    let accept_bit = state_bit(2);
    for byte in b'0'..=b'9' {
        assert_eq!(
            transition_mask(&compiled, 1, byte),
            accept_bit,
            "digit byte {byte:?} must transition to accept state"
        );
    }
    for byte in [b'/', b':', b'a', b'Z', b'_'] {
        assert_eq!(
            transition_mask(&compiled, 1, byte),
            0,
            "non-digit byte {byte:?} must not transition through \\d"
        );
    }
}

#[test]
fn inverted_digit_escape_compiles_to_complement_byte_set() {
    let compiled = compile_regex_set(&["\\D"]).expect("ASCII non-digit class must compile");
    let accept_bit = state_bit(2);

    for byte in b'0'..=b'9' {
        assert_eq!(
            transition_mask(&compiled, 1, byte),
            0,
            "digit byte {byte:?} must be excluded from \\D"
        );
    }
    for byte in [0, b'/', b':', b'a', b'Z', b'_', 0xFF] {
        assert_eq!(
            transition_mask(&compiled, 1, byte),
            accept_bit,
            "non-digit byte {byte:?} must transition through \\D"
        );
    }
}

#[test]
fn unicode_class_reports_unsupported_instead_of_widening_byte_automaton() {
    let err = compile_regex_set(&["\\p{Greek}"]).expect_err("unicode class must reject");
    assert!(
        matches!(
            err,
            RegexCompileError::Unsupported { .. } | RegexCompileError::Parse { .. }
        ),
        "unicode classes must not silently widen byte automata: {err}"
    );
}

#[test]
fn multiple_literal_patterns_keep_distinct_accept_rows() {
    let compiled = compile_regex_set(&["foo", "bar"]).expect("literal pattern set must compile");
    assert_eq!(compiled.plan.accept_states.len(), 2);
    assert_eq!(compiled.plan.accept_state_ids.len(), 2);
}

#[test]
fn ten_literal_patterns_keep_one_accept_row_per_pattern() {
    let patterns: Vec<String> = (0..10).map(|i| format!("pat{i}")).collect();
    let refs: Vec<&str> = patterns.iter().map(String::as_str).collect();
    let compiled = compile_regex_set(&refs).expect("literal pattern set must compile");
    assert_eq!(compiled.plan.accept_states.len(), refs.len());
    assert_eq!(compiled.plan.accept_state_ids.len(), refs.len());
}
