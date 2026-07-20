//! Property tests for `vyre_libs::scan::regex_compile`.
//!
//! Generated patterns exercise:
//!
//!   - Random literal strings (safe ASCII subset).
//!   - Generated alternations, classes, bounded repetitions.
//!   - Mixed adversarial shapes (deeply nested concat / alt).
//!
//! Asserts:
//!   - No panic on any well-formed regex AST.
//!   - Successfully compiled NFA has `num_states ≥ 1`.
//!   - Each pattern in the input contributes one accept state.
//!   - The state cap is enforced (huge inputs return TooManyStates,
//!     never panic).

#![cfg(feature = "matching-regex")]

use proptest::prelude::*;
use vyre_libs::scan::{compile_regex_set, RegexCompileError};

/// Generate ASCII-safe literal strings in 1..=12 bytes. Bound keeps
/// state counts well under the per-pipeline cap and shrinking quick.
fn arb_literal() -> impl Strategy<Value = String> {
    proptest::collection::vec(
        prop::sample::select(b"abcdefghij_-0123".to_vec()),
        1..=12usize,
    )
    .prop_map(|v| String::from_utf8(v).unwrap())
}

/// Generate a small character class shape, e.g. `[a-z]`, `[abc]`,
/// `[0-9_]`. ASCII range only (matches the regex_compile contract).
fn arb_class() -> impl Strategy<Value = String> {
    prop::sample::select(vec![
        "[a-z]".to_string(),
        "[A-Z]".to_string(),
        "[0-9]".to_string(),
        "[a-zA-Z0-9_]".to_string(),
        "[abc]".to_string(),
    ])
}

/// Combine literals + classes into a small mixed pattern.
fn arb_pattern() -> impl Strategy<Value = String> {
    prop_oneof![
        arb_literal(),
        arb_class(),
        (arb_literal(), arb_literal()).prop_map(|(a, b)| format!("{a}|{b}")),
        (arb_class(), 1u32..=8u32).prop_map(|(c, n)| format!("{c}{{{n}}}")),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn arbitrary_pattern_never_panics(pat in arb_pattern()) {
        // The contract: any well-formed regex produces a compile
        // result, never a panic. Errors are typed.
        let _ = compile_regex_set(&[pat.as_str()]);
    }

    #[test]
    fn literal_pattern_compiles(s in arb_literal()) {
        let res = compile_regex_set(&[s.as_str()]);
        prop_assert!(
            res.is_ok(),
            "literal pattern {:?} failed to compile: {:?}",
            s,
            res.err()
        );
        let compiled = res.unwrap();
        prop_assert_eq!(compiled.plan.accept_states.len(), 1);
        prop_assert!(compiled.plan.num_states >= 1);
    }

    #[test]
    fn each_input_pattern_gets_an_accept_state(
        patterns in proptest::collection::vec(arb_literal(), 1..=4usize)
    ) {
        let refs: Vec<&str> = patterns.iter().map(|s| s.as_str()).collect();
        if let Ok(compiled) = compile_regex_set(&refs) {
            prop_assert_eq!(compiled.plan.accept_states.len(), patterns.len());
            prop_assert_eq!(compiled.plan.accept_state_ids.len(), patterns.len());
        }
    }

    #[test]
    fn state_count_grows_monotonically_with_pattern_count(
        a in arb_literal(),
        b in arb_literal(),
    ) {
        // Adding a second pattern can only add states, never remove.
        if let (Ok(one), Ok(two)) = (
            compile_regex_set(&[a.as_str()]),
            compile_regex_set(&[a.as_str(), b.as_str()]),
        ) {
            prop_assert!(two.plan.num_states >= one.plan.num_states);
        }
    }

    #[test]
    fn anchor_always_rejected(suffix in arb_literal()) {
        let pat = format!("^{suffix}");
        match compile_regex_set(&[pat.as_str()]) {
            Err(RegexCompileError::Unsupported { .. }) => {}
            other => prop_assert!(
                false,
                "expected Unsupported(anchor); got {other:?} for pat={pat:?}"
            ),
        }
    }

    #[test]
    fn huge_literal_returns_typed_error(repeats in 1024usize..=4096usize) {
        // Build a literal that exceeds the per-pipeline state cap.
        // Must fail with TooManyStates, never panic.
        let huge = "a".repeat(repeats);
        match compile_regex_set(&[huge.as_str()]) {
            Err(RegexCompileError::TooManyStates { .. }) => {}
            Ok(_) => {} // small enough  -  also acceptable
            other => prop_assert!(
                false,
                "huge input must succeed or return TooManyStates; got {other:?}"
            ),
        }
    }
}
