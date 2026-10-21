//! Adversarial tests for `vyre_libs::scan::regex_compile`.
//!
//! Exercises the regex AST → NfaPlan frontend with pathological,
//! malformed, and boundary inputs. Every test asserts a specific
//! contract  -  no panics, no silent swallowing, precise error
//! variants with correct metadata.

#![cfg(feature = "matching-regex")]

use vyre_libs::scan::{
    build_rule_pipeline_from_regex, compile_regex_set, MatchScan, RegexCompileError,
};

const STATE_CAP: usize = vyre_primitives::nfa::subgroup_nfa::LANES_PER_SUBGROUP * 32;

// ---------------------------------------------------------------------------
// 1. Malformed regex parser inputs  -  must return RegexCompileError::Parse
// ---------------------------------------------------------------------------

fn assert_parse_error(result: Result<(), RegexCompileError>, expected_index: usize) {
    match result {
        Err(RegexCompileError::Parse { pattern_index, .. }) => {
            assert_eq!(
                pattern_index, expected_index,
                "expected Parse error at index {expected_index}"
            );
        }
        other => panic!("expected Parse error, got {other:?}"),
    }
}

#[test]
fn malformed_unbalanced_paren() {
    assert_parse_error(compile_regex_set(&["(abc"]).map(|_| ()), 0);
}

#[test]
fn malformed_truncated_class() {
    assert_parse_error(compile_regex_set(&["[abc"]).map(|_| ()), 0);
}

#[test]
fn malformed_unterminated_escape() {
    assert_parse_error(compile_regex_set(&["foo\\"]).map(|_| ()), 0);
}

#[test]
fn malformed_invalid_quantifier_empty() {
    assert_parse_error(compile_regex_set(&["a{}"]).map(|_| ()), 0);
}

#[test]
fn malformed_invalid_quantifier_missing_start() {
    assert_parse_error(compile_regex_set(&["a{,65}"]).map(|_| ()), 0);
}

#[test]
fn malformed_empty_class() {
    assert_parse_error(compile_regex_set(&["[]"]).map(|_| ()), 0);
}

#[test]
fn malformed_unrecognized_flag_conditional() {
    assert_parse_error(compile_regex_set(&["(?(1)a|b)"]).map(|_| ()), 0);
}

#[test]
fn malformed_missing_expression_star() {
    assert_parse_error(compile_regex_set(&["*"]).map(|_| ()), 0);
}

#[test]
fn malformed_invalid_repetition_range() {
    assert_parse_error(compile_regex_set(&["a{1,0}"]).map(|_| ()), 0);
}

#[test]
fn malformed_comment_not_allowed() {
    assert_parse_error(compile_regex_set(&["(?#comment)"]).map(|_| ()), 0);
}

#[test]
fn malformed_bare_backslash() {
    assert_parse_error(compile_regex_set(&["\\"]).map(|_| ()), 0);
}

#[test]
fn malformed_unopened_group() {
    assert_parse_error(compile_regex_set(&[")"]).map(|_| ()), 0);
}

#[test]
fn malformed_at_index_1() {
    assert_parse_error(compile_regex_set(&["abc", "[def"]).map(|_| ()), 1);
}

// ---------------------------------------------------------------------------
// 2. Unsupported feature rejection  -  must return RegexCompileError::Unsupported
// ---------------------------------------------------------------------------

fn assert_unsupported_error(
    result: Result<(), RegexCompileError>,
    expected_index: usize,
    expected_feature_substr: &str,
) {
    match result {
        Err(RegexCompileError::Unsupported {
            pattern_index,
            feature,
        }) => {
            assert_eq!(
                pattern_index, expected_index,
                "expected Unsupported error at index {expected_index}"
            );
            assert!(
                feature.contains(expected_feature_substr),
                "expected feature description containing '{expected_feature_substr}', got '{feature}'"
            );
        }
        other => panic!("expected Unsupported error, got {other:?}"),
    }
}

#[test]
fn unsupported_anchor_start_caret() {
    assert_unsupported_error(compile_regex_set(&["^foo"]).map(|_| ()), 0, "anchors");
}

#[test]
fn unsupported_anchor_end_dollar() {
    assert_unsupported_error(compile_regex_set(&["foo$"]).map(|_| ()), 0, "anchors");
}

#[test]
fn unsupported_word_boundary() {
    assert_unsupported_error(compile_regex_set(&["\\bword"]).map(|_| ()), 0, "anchors");
}

#[test]
fn unsupported_negated_word_boundary() {
    assert_unsupported_error(compile_regex_set(&["\\Bword"]).map(|_| ()), 0, "anchors");
}

#[test]
fn unsupported_anchor_start_alt() {
    assert_unsupported_error(compile_regex_set(&["\\Afoo"]).map(|_| ()), 0, "anchors");
}

#[test]
fn unsupported_anchor_end_alt() {
    assert_unsupported_error(compile_regex_set(&["\\z"]).map(|_| ()), 0, "anchors");
}

#[test]
fn unsupported_repetition_upper_bound_too_large() {
    assert_unsupported_error(
        compile_regex_set(&["a{0,128}"]).map(|_| ()),
        0,
        "repetition",
    );
}

#[test]
fn unsupported_repetition_min_bound_too_large() {
    assert_unsupported_error(compile_regex_set(&["a{65,}"]).map(|_| ()), 0, "repetition");
}

#[test]
fn unsupported_repetition_exact_too_large() {
    assert_unsupported_error(compile_regex_set(&["a{65}"]).map(|_| ()), 0, "repetition");
}

#[test]
fn unsupported_at_index_1_anchor() {
    assert_unsupported_error(
        compile_regex_set(&["abc", "^def"]).map(|_| ()),
        1,
        "anchors",
    );
}

#[test]
fn unsupported_at_index_1_repetition() {
    assert_unsupported_error(
        compile_regex_set(&["abc", "x{0,128}"]).map(|_| ()),
        1,
        "repetition",
    );
}

#[test]
fn unsupported_lone_anchor_caret() {
    assert_unsupported_error(compile_regex_set(&["^"]).map(|_| ()), 0, "anchors");
}

// ---------------------------------------------------------------------------
// 3. State-cap stress  -  exactly at cap, one over cap, pathological shapes
// ---------------------------------------------------------------------------

#[test]
fn state_cap_literal_exactly_at_cap() {
    // A literal of length L produces: 1 entry + 1 start + L byte states = L + 2.
    // For L = 1022 we get exactly 1024 states.
    let pattern: String = "a".repeat(STATE_CAP - 2);
    let compiled = compile_regex_set(&[&pattern]).unwrap();
    assert_eq!(compiled.plan.num_states as usize, STATE_CAP);
}

#[test]
fn state_cap_literal_one_over_cap() {
    let pattern: String = "a".repeat(STATE_CAP - 1);
    match compile_regex_set(&[&pattern]) {
        Err(RegexCompileError::TooManyStates { states, cap }) => {
            assert_eq!(cap, STATE_CAP);
            assert!(
                states > STATE_CAP,
                "expected states > {STATE_CAP}, got {states}"
            );
        }
        other => panic!("expected TooManyStates, got {other:?}"),
    }
}

#[test]
fn state_cap_wide_alternation_under_cap() {
    // Distinct 2-char literals so regex-syntax does not fold into a class.
    // Each branch = 3 states; fork + join = 2; entry = 1 => total = 3 + 3N.
    // N = 340 gives 3 + 1020 = 1023 states (< 1024).
    let parts: Vec<String> = (0..340)
        .map(|i| {
            let a = (b'a' + (i % 26) as u8) as char;
            let b = (b'a' + ((i / 26) % 26) as u8) as char;
            format!("{}{}", a, b)
        })
        .collect();
    let pattern = parts.join("|");
    let compiled = compile_regex_set(&[&pattern]).unwrap();
    assert!(compiled.plan.num_states as usize <= STATE_CAP);
}

#[test]
fn state_cap_wide_alternation_over_cap() {
    // N = 341 gives 3 + 1023 = 1026 states (> 1024).
    let parts: Vec<String> = (0..341)
        .map(|i| {
            let a = (b'a' + (i % 26) as u8) as char;
            let b = (b'a' + ((i / 26) % 26) as u8) as char;
            format!("{}{}", a, b)
        })
        .collect();
    let pattern = parts.join("|");
    match compile_regex_set(&[&pattern]) {
        Err(RegexCompileError::TooManyStates { states, cap }) => {
            assert_eq!(cap, STATE_CAP);
            assert!(states > STATE_CAP);
        }
        other => panic!("expected TooManyStates, got {other:?}"),
    }
}

#[test]
fn state_cap_repeated_optional_under_cap() {
    // a{0,64} unrolls to 193 pattern states + 1 entry = 194 per pattern.
    // 5 patterns => 1 + 5 * 193 = 966 states.
    let patterns: Vec<&str> = vec!["a{0,64}"; 5];
    let compiled = compile_regex_set(&patterns).unwrap();
    assert!(compiled.plan.num_states as usize <= STATE_CAP);
}

#[test]
fn state_cap_repeated_optional_over_cap() {
    // 6 patterns => 1 + 6 * 193 = 1159 states.
    let patterns: Vec<&str> = vec!["a{0,64}"; 6];
    match compile_regex_set(&patterns) {
        Err(RegexCompileError::TooManyStates { states, cap }) => {
            assert_eq!(cap, STATE_CAP);
            assert!(states > STATE_CAP);
        }
        other => panic!("expected TooManyStates, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// 4. Cross-pattern interaction  -  multi-pattern, stability, cache keys
// ---------------------------------------------------------------------------

#[test]
fn cross_pattern_max_plausible_patterns_compile() {
    // 64 simple literals is well under the state cap and exercises the
    // multi-pattern path without blowing up compile time.
    let patterns: Vec<String> = (0..64).map(|i| format!("pat{}", i)).collect();
    let refs: Vec<&str> = patterns.iter().map(|s| s.as_str()).collect();
    let compiled = compile_regex_set(&refs).unwrap();
    assert_eq!(compiled.plan.accept_states.len(), 64);
    assert_eq!(compiled.plan.accept_state_ids.len(), 64);
}

#[test]
fn cross_pattern_pid_stable_across_rebuilds() {
    let compiled_a = compile_regex_set(&["foo", "bar", "baz"]).unwrap();
    let compiled_b = compile_regex_set(&["foo", "bar", "baz"]).unwrap();
    assert_eq!(compiled_a.plan.accept_states, compiled_b.plan.accept_states);
    assert_eq!(
        compiled_a.plan.accept_state_ids,
        compiled_b.plan.accept_state_ids
    );
}

#[test]
fn cross_pattern_accept_state_count_matches_input_len() {
    for n in [1usize, 2, 4, 8, 16, 32] {
        let patterns: Vec<String> = (0..n).map(|i| format!("p{}", i)).collect();
        let refs: Vec<&str> = patterns.iter().map(|s| s.as_str()).collect();
        let compiled = compile_regex_set(&refs).unwrap();
        assert_eq!(
            compiled.plan.accept_states.len(),
            n,
            "accept_states.len() must equal pattern count"
        );
        assert_eq!(
            compiled.plan.accept_state_ids.len(),
            n,
            "accept_state_ids.len() must equal pattern count"
        );
    }
}

#[test]
fn cross_pattern_cache_key_changes_on_order_swap() {
    let a = build_rule_pipeline_from_regex(&["foo", "bar"], "input", "hit", 0).unwrap();
    let b = build_rule_pipeline_from_regex(&["bar", "foo"], "input", "hit", 0).unwrap();
    assert_ne!(
        a.cache_key(),
        b.cache_key(),
        "swapping pattern order must change cache key"
    );
}

#[test]
fn cross_pattern_cache_key_stable_for_identical_set() {
    let a = build_rule_pipeline_from_regex(&["foo", "bar"], "input", "hit", 0).unwrap();
    let b = build_rule_pipeline_from_regex(&["foo", "bar"], "input", "hit", 0).unwrap();
    assert_eq!(
        a.cache_key(),
        b.cache_key(),
        "identical patterns must produce identical cache key"
    );
}

#[test]
fn cross_pattern_cache_key_stable_across_rebuilds() {
    let a = build_rule_pipeline_from_regex(&["a", "bb", "ccc"], "in", "hit", 16).unwrap();
    let b = build_rule_pipeline_from_regex(&["a", "bb", "ccc"], "in", "hit", 16).unwrap();
    assert_eq!(a.cache_key(), b.cache_key());
}
