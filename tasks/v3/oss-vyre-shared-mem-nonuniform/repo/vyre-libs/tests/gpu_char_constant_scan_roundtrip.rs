//! GPU char-constant scanner reference roundtrip  -  phase 17b.3a.
//!
//! Pins the scanner against C-preprocessor `consume_char_constant`
//! semantics for prefix tolerance, single-char constants, and the
//! simple escape table. Numeric escapes (octal/hex/UCN) land in 17b.3b
//! and are intentionally not tested here.

#![cfg(feature = "c-parser")]
#![allow(deprecated)]
use vyre_libs::parsing::c::preprocess::gpu_char_constant_scan::gpu_char_constant_scan;
use vyre_libs::scan::dispatch_io::pack_u32_slice as pack_u32_le;
use vyre_reference::value::Value;

fn unpack_u32(bytes: &[u8]) -> u32 {
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

/// Run the GPU scanner once. Returns `(value, bytes_consumed, ok)`.
fn run_scanner(source: &[u8], start: u32) -> (u32, u32, u32) {
    let mut src = source.to_vec();
    // `source` is declared as packed U32 words; pad to multiple of 4.
    src.resize((source.len().div_ceil(4) * 4).max(4), 0);
    let prog = gpu_char_constant_scan(source.len() as u32);
    let outputs = vyre_reference::reference_eval(
        &prog,
        &[
            Value::from(src),
            Value::from(pack_u32_le(&[start])),
            Value::from(vec![0u8; 4]),
            Value::from(vec![0u8; 4]),
            Value::from(vec![0u8; 4]),
        ],
    )
    .expect("gpu_char_constant_scan reference eval");
    (
        unpack_u32(&outputs[0].to_bytes()),
        unpack_u32(&outputs[1].to_bytes()),
        unpack_u32(&outputs[2].to_bytes()),
    )
}

// ---- Single-char constants ----

#[test]
fn unprefixed_ascii_letter() {
    assert_eq!(run_scanner(b"'A'", 0), (b'A' as u32, 3, 1));
}

#[test]
fn unprefixed_digit() {
    assert_eq!(run_scanner(b"'7'", 0), (b'7' as u32, 3, 1));
}

#[test]
fn unprefixed_space() {
    assert_eq!(run_scanner(b"' '", 0), (b' ' as u32, 3, 1));
}

// ---- Prefix tolerance ----

#[test]
fn capital_l_prefix_is_consumed() {
    assert_eq!(run_scanner(b"L'A'", 0), (b'A' as u32, 4, 1));
}

#[test]
fn lowercase_u_prefix_is_consumed() {
    assert_eq!(run_scanner(b"u'X'", 0), (b'X' as u32, 4, 1));
}

#[test]
fn uppercase_u_prefix_is_consumed() {
    assert_eq!(run_scanner(b"U'Y'", 0), (b'Y' as u32, 4, 1));
}

#[test]
fn u8_prefix_is_consumed_as_two_bytes() {
    assert_eq!(run_scanner(b"u8'Z'", 0), (b'Z' as u32, 5, 1));
}

// ---- Simple escape table ----

#[test]
fn newline_escape() {
    assert_eq!(run_scanner(b"'\\n'", 0), (b'\n' as u32, 4, 1));
}

#[test]
fn tab_escape() {
    assert_eq!(run_scanner(b"'\\t'", 0), (b'\t' as u32, 4, 1));
}

#[test]
fn carriage_return_escape() {
    assert_eq!(run_scanner(b"'\\r'", 0), (b'\r' as u32, 4, 1));
}

#[test]
fn alert_escape_is_seven() {
    assert_eq!(run_scanner(b"'\\a'", 0), (7, 4, 1));
}

#[test]
fn backspace_escape_is_eight() {
    assert_eq!(run_scanner(b"'\\b'", 0), (8, 4, 1));
}

#[test]
fn form_feed_escape_is_twelve() {
    assert_eq!(run_scanner(b"'\\f'", 0), (12, 4, 1));
}

#[test]
fn vertical_tab_escape_is_eleven() {
    assert_eq!(run_scanner(b"'\\v'", 0), (11, 4, 1));
}

#[test]
fn null_escape_is_zero() {
    assert_eq!(run_scanner(b"'\\0'", 0), (0, 4, 1));
}

#[test]
fn escaped_backslash() {
    assert_eq!(run_scanner(b"'\\\\'", 0), (b'\\' as u32, 4, 1));
}

#[test]
fn escaped_single_quote() {
    assert_eq!(run_scanner(b"'\\''", 0), (b'\'' as u32, 4, 1));
}

#[test]
fn escaped_double_quote() {
    assert_eq!(run_scanner(b"'\\\"'", 0), (b'"' as u32, 4, 1));
}

#[test]
fn escaped_question_mark() {
    assert_eq!(run_scanner(b"'\\?'", 0), (b'?' as u32, 4, 1));
}

// ---- Multi-byte concatenation ----

#[test]
fn multi_char_constant_shifts_and_ors_bytes() {
    // 'AB' → ((0 << 8) | 'A') = 'A', then ((..<<8)|'B') = 0x4142.
    let expected = ((b'A' as u32) << 8) | b'B' as u32;
    assert_eq!(run_scanner(b"'AB'", 0), (expected, 4, 1));
}

#[test]
fn four_char_constant_packs_into_u32() {
    let expected =
        ((b'A' as u32) << 24) | ((b'B' as u32) << 16) | ((b'C' as u32) << 8) | b'D' as u32;
    assert_eq!(run_scanner(b"'ABCD'", 0), (expected, 6, 1));
}

// ---- Negative cases ----

#[test]
fn no_quote_returns_ok_zero() {
    let (_v, c, ok) = run_scanner(b"hello", 0);
    assert_eq!((c, ok), (0, 0));
}

#[test]
fn empty_quote_returns_ok_zero() {
    let (_v, c, ok) = run_scanner(b"''", 0);
    assert_eq!((c, ok), (0, 0));
}

#[test]
fn unterminated_constant_returns_ok_zero() {
    let (_v, c, ok) = run_scanner(b"'A", 0);
    assert_eq!((c, ok), (0, 0));
}

#[test]
fn embedded_newline_returns_ok_zero() {
    let (_v, c, ok) = run_scanner(b"'A\n'", 0);
    assert_eq!((c, ok), (0, 0));
}

// ---- Numeric escapes (17b.3b) ----

#[test]
fn octal_one_digit() {
    // '\0' is octal-style: a single octal digit 0 → value 0, 4 bytes consumed.
    assert_eq!(run_scanner(b"'\\0'", 0), (0, 4, 1));
}

#[test]
fn octal_two_digits() {
    // '\07' → 7, 5 bytes (\\, 0, 7, ').
    assert_eq!(run_scanner(b"'\\07'", 0), (7, 5, 1));
}

#[test]
fn octal_three_digits() {
    // '\012' → 0o12 = 10, 6 bytes.
    assert_eq!(run_scanner(b"'\\012'", 0), (0o12, 6, 1));
}

#[test]
fn octal_caps_at_three_digits() {
    // '\0123'  -  only \012 is the escape; the trailing '3' is a
    // separate char (multi-char concat). value = (10 << 8) | 0x33.
    let expected = (0o12u32 << 8) | b'3' as u32;
    assert_eq!(run_scanner(b"'\\0123'", 0), (expected, 7, 1));
}

#[test]
fn octal_stops_at_eight() {
    // '\18'  -  only '\1' is the octal; '8' is a separate char.
    let expected = (1u32 << 8) | b'8' as u32;
    assert_eq!(run_scanner(b"'\\18'", 0), (expected, 5, 1));
}

#[test]
fn hex_two_digits() {
    assert_eq!(run_scanner(b"'\\xff'", 0), (0xff, 6, 1));
}

#[test]
fn hex_uppercase() {
    assert_eq!(run_scanner(b"'\\xFF'", 0), (0xff, 6, 1));
}

#[test]
fn hex_one_digit_works() {
    assert_eq!(run_scanner(b"'\\x7'", 0), (0x7, 5, 1));
}

#[test]
fn hex_no_digits_is_error() {
    // '\x' with no hex digits is malformed per the CPU `consume_hex_escape`.
    let (_, c, ok) = run_scanner(b"'\\x'", 0);
    assert_eq!((c, ok), (0, 0));
}

#[test]
fn hex_stops_at_non_hex() {
    // '\x7g'  -  \x consumes only the '7'; 'g' is concatenated.
    let expected = (7u32 << 8) | b'g' as u32;
    assert_eq!(run_scanner(b"'\\x7g'", 0), (expected, 6, 1));
}

#[test]
fn ucn4_basic_ascii() {
    // A = 'A' (U+0041), value 0x41, 8 bytes (A = 6 + ' open + ' close).
    assert_eq!(run_scanner(b"'\\u0041'", 0), (0x41, 8, 1));
}

#[test]
fn ucn8_basic_ascii() {
    // \U00000041 = 'A', value 0x41, 12 bytes.
    assert_eq!(run_scanner(b"'\\U00000041'", 0), (0x41, 12, 1));
}

// ---- Sub-buffer offset ----

#[test]
fn scanner_honours_start_offset() {
    // "  'A'"  -  start at 2.
    assert_eq!(run_scanner(b"  'A'", 2), (b'A' as u32, 3, 1));
}

#[test]
fn scanner_at_start_of_a_macro_body_only_consumes_the_constant() {
    let (v, c, ok) = run_scanner(b"#define BLANK ' '\n", 14);
    assert_eq!((v, c, ok), (b' ' as u32, 3, 1));
}
