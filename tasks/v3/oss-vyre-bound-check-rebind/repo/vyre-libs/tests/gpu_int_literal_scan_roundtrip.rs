//! GPU integer-literal scanner reference roundtrip.
//!
//! Drives the kernel for each literal-form test case and asserts the
//! `(value, bytes_consumed)` pair matches the C-preprocessor literal
//! semantics (mirrors the CPU `consume_integer` from
//! `vyre-libs::parsing::c::preprocess::expr_parser`).
//!
//! The kernel uses u32 saturating arithmetic. Oversized literals consume
//! their full scanned digit run and emit `u32::MAX`.

#![cfg(feature = "c-parser")]
#![allow(deprecated)]
use vyre_libs::parsing::c::preprocess::gpu_int_literal_scan::gpu_int_literal_scan;
use vyre_libs::scan::dispatch_io::pack_u32_slice as pack_u32_le;
use vyre_reference::value::Value;

fn unpack_u32(bytes: &[u8]) -> u32 {
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

/// Run the GPU scanner once on `source` starting at byte position
/// `start`. Returns `(value, bytes_consumed)`.
fn run_scanner(source: &[u8], start: u32) -> (u32, u32) {
    let mut src = source.to_vec();
    // `source` is declared as packed U32 words; pad to multiple of 4.
    src.resize((source.len().div_ceil(4) * 4).max(4), 0);
    let prog = gpu_int_literal_scan(source.len() as u32);
    let outputs = vyre_reference::reference_eval(
        &prog,
        &[
            Value::from(src),
            Value::from(pack_u32_le(&[start])),
            Value::from(vec![0u8; 4]),
            Value::from(vec![0u8; 4]),
        ],
    )
    .expect("gpu_int_literal_scan reference eval");
    (
        unpack_u32(&outputs[0].to_bytes()),
        unpack_u32(&outputs[1].to_bytes()),
    )
}

// ---- Decimal ----

#[test]
fn decimal_zero() {
    assert_eq!(run_scanner(b"0", 0), (0, 1));
}

#[test]
fn decimal_one_digit() {
    assert_eq!(run_scanner(b"7", 0), (7, 1));
}

#[test]
fn decimal_multi_digit() {
    assert_eq!(run_scanner(b"12345", 0), (12345, 5));
}

#[test]
fn decimal_max_u32() {
    assert_eq!(run_scanner(b"4294967295", 0), (u32::MAX, 10));
}

#[test]
fn decimal_followed_by_non_digit_stops_at_first_non_digit() {
    let (v, c) = run_scanner(b"42xyz", 0);
    assert_eq!((v, c), (42, 2));
}

// ---- Hex ----

#[test]
fn hex_lowercase_prefix_uppercase_digits() {
    assert_eq!(run_scanner(b"0xDEADBEEF", 0), (0xDEAD_BEEF, 10));
}

#[test]
fn hex_uppercase_prefix_lowercase_digits() {
    assert_eq!(run_scanner(b"0Xcafe", 0), (0xCAFE, 6));
}

#[test]
fn hex_zero_is_one_digit_after_prefix() {
    assert_eq!(run_scanner(b"0x0", 0), (0, 3));
}

#[test]
fn hex_mixed_case_digits() {
    assert_eq!(run_scanner(b"0xAbCdEf", 0), (0x00AB_CDEF, 8));
}

#[test]
fn hex_stops_at_g() {
    let (v, c) = run_scanner(b"0xFFGG", 0);
    assert_eq!((v, c), (0xFF, 4));
}

// ---- Binary ----

#[test]
fn binary_zero() {
    assert_eq!(run_scanner(b"0b0", 0), (0, 3));
}

#[test]
fn binary_one_bit() {
    assert_eq!(run_scanner(b"0b1", 0), (1, 3));
}

#[test]
fn binary_multi_bit() {
    assert_eq!(run_scanner(b"0b1010", 0), (10, 6));
}

#[test]
fn binary_uppercase_prefix() {
    assert_eq!(run_scanner(b"0B11111111", 0), (255, 10));
}

#[test]
fn binary_stops_at_two() {
    let (v, c) = run_scanner(b"0b102", 0);
    assert_eq!((v, c), (0b10, 4));
}

// ---- Octal ----

#[test]
fn octal_explicit_zero_one_digit() {
    // CPU semantics: lone "0" is a single-digit octal literal that
    // evaluates to 0 with consumed=1.
    assert_eq!(run_scanner(b"0", 0), (0, 1));
}

#[test]
fn octal_value() {
    assert_eq!(run_scanner(b"0777", 0), (0o777, 4));
}

#[test]
fn octal_stops_at_eight() {
    let (v, c) = run_scanner(b"0778", 0);
    assert_eq!((v, c), (0o77, 3));
}

// ---- Suffixes ----

#[test]
fn unsigned_suffix_is_consumed() {
    assert_eq!(run_scanner(b"42u", 0), (42, 3));
    assert_eq!(run_scanner(b"42U", 0), (42, 3));
}

#[test]
fn long_suffix_is_consumed() {
    assert_eq!(run_scanner(b"42l", 0), (42, 3));
    assert_eq!(run_scanner(b"42L", 0), (42, 3));
}

#[test]
fn double_long_suffix_is_consumed() {
    assert_eq!(run_scanner(b"42ll", 0), (42, 4));
    assert_eq!(run_scanner(b"42LL", 0), (42, 4));
}

#[test]
fn ull_suffix_combination_is_consumed() {
    // Three suffix bytes (u + l + l) + two digit bytes = 5 consumed.
    assert_eq!(run_scanner(b"42ull", 0), (42, 5));
    assert_eq!(run_scanner(b"42ULL", 0), (42, 5));
}

#[test]
fn hex_with_suffix() {
    assert_eq!(run_scanner(b"0xFFu", 0), (0xFF, 5));
}

// ---- Negative cases ----

#[test]
fn non_digit_returns_zero_consumed() {
    let (v, c) = run_scanner(b"hello", 0);
    assert_eq!((v, c), (0, 0));
}

#[test]
fn empty_buffer_returns_zero_consumed() {
    let (v, c) = run_scanner(b"", 0);
    assert_eq!((v, c), (0, 0));
}

// ---- Saturation contract ----

#[test]
fn overflow_saturates_to_u32_max() {
    let (v, c) = run_scanner(b"99999999999", 0);
    assert_eq!(c, 11, "all 11 digits consumed even on overflow");
    assert_eq!(v, u32::MAX, "overflow must saturate instead of wrapping");
}

// ---- Sub-buffer offset ----

#[test]
fn scanner_honours_start_offset() {
    // "  42"  -  start at 2 should still parse "42".
    assert_eq!(run_scanner(b"  42", 2), (42, 2));
}

#[test]
fn scanner_at_start_of_a_macro_body_only_consumes_the_literal() {
    let (v, c) = run_scanner(b"#define FOO 100\n", 12);
    assert_eq!((v, c), (100, 3), "consumes 100 then stops at newline");
}
