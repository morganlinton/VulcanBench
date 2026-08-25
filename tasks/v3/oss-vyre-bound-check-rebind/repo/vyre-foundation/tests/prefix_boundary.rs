//! Boundary tests for prefix-array engine helpers.
//!
//! `brace_depth_prefix`, `nested_depth_prefix`, and `newline_prefix_sum`
//! process untrusted byte slices. They must reject oversized input and
//! handle edge cases (empty, all delimiters, no delimiters) without panic.

use vyre_foundation::engine::prefix::{
    brace_depth_prefix, nested_depth_prefix, newline_prefix_sum, validate_prefix_input,
};

// ------------------------------------------------------------------
// Empty / trivial input
// ------------------------------------------------------------------

#[test]
fn brace_depth_empty_is_zero() {
    let result = brace_depth_prefix(b"").unwrap();
    assert_eq!(result, vec![0]);
}

#[test]
fn nested_depth_empty_is_zero() {
    let result = nested_depth_prefix(b"").unwrap();
    assert_eq!(result, vec![0]);
}

#[test]
fn newline_sum_empty_is_zero() {
    let result = newline_prefix_sum(b"").unwrap();
    assert_eq!(result, vec![0]);
}

// ------------------------------------------------------------------
// Basic correctness
// ------------------------------------------------------------------

#[test]
fn brace_depth_counts_curly_braces() {
    // result[i] = depth BEFORE byte i
    let result = brace_depth_prefix(b"{a{b}c}").unwrap();
    assert_eq!(result, vec![0, 1, 1, 2, 2, 1, 1]);
}

#[test]
fn brace_depth_negative_is_clamped_to_zero() {
    let result = brace_depth_prefix(b"}a{}").unwrap();
    assert_eq!(result[0], 0);
}

#[test]
fn nested_depth_counts_parens_and_braces_only() {
    // nested_depth_prefix only counts '(' ')' '{' '}', NOT '[' ']'
    let result = nested_depth_prefix(b"([{}])").unwrap();
    assert_eq!(result, vec![0, 1, 1, 2, 1, 1]);
}

#[test]
fn newline_sum_counts_newlines() {
    // result[0] = 0, then after each byte
    let result = newline_prefix_sum(b"a\nb\nc").unwrap();
    assert_eq!(result, vec![0, 0, 1, 1, 2, 2]);
}

#[test]
fn newline_sum_no_newlines_is_all_zeros() {
    let result = newline_prefix_sum(b"abc").unwrap();
    assert_eq!(result, vec![0, 0, 0, 0]);
}

// ------------------------------------------------------------------
// Oversized input rejection
// ------------------------------------------------------------------

#[test]
fn validate_prefix_input_rejects_too_large() {
    let err = validate_prefix_input(128 * 1024 * 1024).unwrap_err();
    assert!(err.to_string().contains("Fix:"));
}

#[test]
fn validate_prefix_input_accepts_small() {
    assert_eq!(validate_prefix_input(1024), Ok(()));
}

#[test]
fn validate_prefix_input_accepts_zero() {
    assert_eq!(validate_prefix_input(0), Ok(()));
}

#[test]
fn brace_depth_rejects_oversized_input() {
    let huge = vec![b'x'; 128 * 1024 * 1024];
    let err = brace_depth_prefix(&huge).unwrap_err();
    assert!(err.to_string().contains("Fix:"));
}

#[test]
fn nested_depth_rejects_oversized_input() {
    let huge = vec![b'x'; 128 * 1024 * 1024];
    let err = nested_depth_prefix(&huge).unwrap_err();
    assert!(err.to_string().contains("Fix:"));
}

#[test]
fn newline_sum_rejects_oversized_input() {
    let huge = vec![b'x'; 128 * 1024 * 1024];
    let err = newline_prefix_sum(&huge).unwrap_err();
    assert!(err.to_string().contains("Fix:"));
}

// ------------------------------------------------------------------
// Edge cases
// ------------------------------------------------------------------

#[test]
fn brace_depth_all_opening() {
    let result = brace_depth_prefix(b"{{{").unwrap();
    assert_eq!(result, vec![0, 1, 2]);
}

#[test]
fn brace_depth_all_closing() {
    let result = brace_depth_prefix(b"}}}").unwrap();
    assert_eq!(result, vec![0, 0, 0]);
}

#[test]
fn nested_depth_mismatched_closing_first() {
    let result = nested_depth_prefix(b"]}").unwrap();
    assert_eq!(result, vec![0, 0]);
}

#[test]
fn newline_sum_all_newlines() {
    let result = newline_prefix_sum(b"\n\n\n").unwrap();
    assert_eq!(result, vec![0, 1, 2, 3]);
}

#[test]
fn newline_sum_mixed_crlf() {
    // Only '\n' counts; '\r' does not
    let result = newline_prefix_sum(b"a\r\nb").unwrap();
    assert_eq!(result, vec![0, 0, 0, 1, 1]);
}
