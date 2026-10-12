// Regression for F-IR-32: opaque extension payloads are byte-identical
// across host endianness. The helpers in `vyre_foundation::opaque_payload`
// must emit the same bytes for the same value regardless of host byte
// order, so that a Program encoded on one architecture round-trips to the
// same [`vyre_foundation::ir::Program::hash`] on another.
//
// We cannot spawn a big-endian emulator from a test, so we pin the
// contract in three ways:
//
// 1. The encoded bytes match the canonical little-endian layout (probing
//    the `0x01020304` pattern for every width).
// 2. Round-trip on the host reproduces the input exactly.
// 3. Truncated inputs fail with an actionable diagnostic rather than
//    silently reading garbage.

use proptest::prelude::*;
use vyre_foundation::opaque_payload::{
    canonical_f32_zero, canonical_f64_zero, canonical_regex_flags, push_f32, push_f64, push_i16,
    push_i32, push_i64, push_u16, push_u32, push_u64, read_f32, read_f64, read_i16, read_i32,
    read_i64, read_u16, read_u32, read_u64,
};

#[test]
fn u16_little_endian_bytes_match_canonical_pattern() {
    let mut buf = Vec::new();
    push_u16(&mut buf, 0x0102);
    assert_eq!(buf, [0x02, 0x01], "u16 must serialize as little-endian");
    let (value, tail) = read_u16(&buf).expect("decoder must accept a full payload");
    assert_eq!(value, 0x0102);
    assert!(tail.is_empty());
}

#[test]
fn u32_little_endian_bytes_match_canonical_pattern() {
    let mut buf = Vec::new();
    push_u32(&mut buf, 0x01020304);
    assert_eq!(
        buf,
        [0x04, 0x03, 0x02, 0x01],
        "u32 must serialize as little-endian  -  `to_ne_bytes` would diverge on BE hosts",
    );
    let (value, tail) = read_u32(&buf).expect("decoder must accept a full payload");
    assert_eq!(value, 0x01020304);
    assert!(tail.is_empty());
}

#[test]
fn u64_little_endian_bytes_match_canonical_pattern() {
    let mut buf = Vec::new();
    push_u64(&mut buf, 0x01020304_05060708);
    assert_eq!(
        buf,
        [0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01],
        "u64 must serialize as little-endian",
    );
    let (value, tail) = read_u64(&buf).expect("decoder must accept a full payload");
    assert_eq!(value, 0x01020304_05060708);
    assert!(tail.is_empty());
}

#[test]
fn signed_integers_round_trip_through_helpers() {
    let mut buf = Vec::new();
    push_i16(&mut buf, -0x7f00);
    push_i32(&mut buf, -0x7f000000);
    push_i64(&mut buf, i64::MIN);

    let (a, tail) = read_i16(&buf).unwrap();
    let (b, tail) = read_i32(tail).unwrap();
    let (c, tail) = read_i64(tail).unwrap();
    assert_eq!((a, b, c), (-0x7f00, -0x7f000000, i64::MIN));
    assert!(tail.is_empty());
}

#[test]
fn floats_preserve_bit_pattern_via_le_bytes() {
    let mut buf = Vec::new();
    push_f32(&mut buf, f32::from_bits(0xDEADBEEF));
    push_f64(&mut buf, f64::from_bits(0xDEADBEEF_CAFEBABE));

    let (f, tail) = read_f32(&buf).unwrap();
    let (g, tail) = read_f64(tail).unwrap();
    assert_eq!(f.to_bits(), 0xDEADBEEF);
    assert_eq!(g.to_bits(), 0xDEADBEEF_CAFEBABE);
    assert!(tail.is_empty());
}

#[test]
fn concatenated_fields_decode_in_order() {
    let mut buf = Vec::new();
    push_u32(&mut buf, 42);
    push_u16(&mut buf, 7);
    push_i64(&mut buf, -1);

    let (a, tail) = read_u32(&buf).unwrap();
    let (b, tail) = read_u16(tail).unwrap();
    let (c, tail) = read_i64(tail).unwrap();
    assert_eq!((a, b, c), (42u32, 7u16, -1i64));
    assert!(tail.is_empty());
}

#[test]
fn truncated_payload_returns_actionable_error() {
    let bytes = [0x01, 0x02]; // 2 bytes, not 4
    let err = read_u32(&bytes).expect_err("short payload must fail");
    let message = format!("{err}");
    assert!(
        message.contains("u32"),
        "message must name the field: {message}"
    );
    assert!(
        message.contains("expected 4"),
        "message must name the width: {message}"
    );
    assert!(
        message.contains("got 2"),
        "message must name actual size: {message}"
    );
    assert!(
        message.contains("Fix:"),
        "message must include Fix hint: {message}"
    );
}

// ------------------------------------------------------------------
// Adversarial proptests: every integer and float type round-trips
// through push/read preserving exact bits (F-IR-32).
// ------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig { cases: 256, .. ProptestConfig::default() })]

    #[test]
    fn all_integer_types_round_trip_exact_bits(
        v_u16 in any::<u16>(),
        v_u32 in any::<u32>(),
        v_u64 in any::<u64>(),
        v_i16 in any::<i16>(),
        v_i32 in any::<i32>(),
        v_i64 in any::<i64>(),
    ) {
        let mut buf = Vec::new();
        push_u16(&mut buf, v_u16);
        push_u32(&mut buf, v_u32);
        push_u64(&mut buf, v_u64);
        push_i16(&mut buf, v_i16);
        push_i32(&mut buf, v_i32);
        push_i64(&mut buf, v_i64);

        let (r_u16, tail) = read_u16(&buf).unwrap();
        let (r_u32, tail) = read_u32(tail).unwrap();
        let (r_u64, tail) = read_u64(tail).unwrap();
        let (r_i16, tail) = read_i16(tail).unwrap();
        let (r_i32, tail) = read_i32(tail).unwrap();
        let (r_i64, tail) = read_i64(tail).unwrap();

        prop_assert_eq!(r_u16, v_u16);
        prop_assert_eq!(r_u32, v_u32);
        prop_assert_eq!(r_u64, v_u64);
        prop_assert_eq!(r_i16, v_i16);
        prop_assert_eq!(r_i32, v_i32);
        prop_assert_eq!(r_i64, v_i64);
        prop_assert!(tail.is_empty());
    }

    #[test]
    fn all_float_types_round_trip_exact_bits(
        v_f32 in any::<u32>().prop_map(f32::from_bits),
        v_f64 in any::<u64>().prop_map(f64::from_bits),
    ) {
        let mut buf = Vec::new();
        push_f32(&mut buf, v_f32);
        push_f64(&mut buf, v_f64);

        let (r_f32, tail) = read_f32(&buf).unwrap();
        let (r_f64, tail) = read_f64(tail).unwrap();

        prop_assert_eq!(r_f32.to_bits(), v_f32.to_bits());
        prop_assert_eq!(r_f64.to_bits(), v_f64.to_bits());
        prop_assert!(tail.is_empty());
    }

    // CRITIQUE_THIRD_PASS_2026-04-23 Finding 07: proptest over the
    // canonical_f64_zero contract across every u64 bit pattern. A
    // refactor that replaces `value == 0.0` with, say,
    // `value.is_subnormal()` would silently flip the mapping from
    // +0.0 / -0.0 to +0.0; the tiny unit suite wouldn't catch it.
    // This proptest does.
    #[test]
    fn canonical_f64_zero_preserves_every_nonzero_bit_pattern(
        bits in any::<u64>(),
    ) {
        let input = f64::from_bits(bits);
        let out = canonical_f64_zero(input);
        if bits == 0x0000_0000_0000_0000 || bits == 0x8000_0000_0000_0000 {
            prop_assert_eq!(
                out.to_bits(),
                0u64,
                "Fix: both +0.0 and -0.0 must canonicalise to +0.0 bits; \
                 input bits {:#018x} produced {:#018x}",
                bits,
                out.to_bits()
            );
        } else {
            prop_assert_eq!(
                out.to_bits(),
                bits,
                "Fix: non-zero f64 must pass through unchanged; input \
                 bits {:#018x} produced {:#018x}",
                bits,
                out.to_bits()
            );
        }
    }

    // Mirror proptest for canonical_f32_zero across every u32 bit
    // pattern  -  same contract, narrower width.
    #[test]
    fn canonical_f32_zero_preserves_every_nonzero_bit_pattern(
        bits in any::<u32>(),
    ) {
        let input = f32::from_bits(bits);
        let out = canonical_f32_zero(input);
        if bits == 0x0000_0000 || bits == 0x8000_0000 {
            prop_assert_eq!(out.to_bits(), 0u32);
        } else {
            prop_assert_eq!(out.to_bits(), bits);
        }
    }
}

// ------------------------------------------------------------------
// Adversarial truncation tests: every reader must report field,
// expected, available, and a Fix: hint (F-IR-32).
// ------------------------------------------------------------------

#[test]
fn truncated_u16_reports_all_fields() {
    let err = read_u16(&[0x01]).expect_err("1 byte is not a u16");
    assert_eq!(err.field, "u16");
    assert_eq!(err.expected, 2);
    assert_eq!(err.available, 1);
    assert!(
        format!("{err}").contains("Fix:"),
        "Fix: hint must be present"
    );
}

#[test]
fn truncated_u32_zero_bytes_reports_all_fields() {
    let err = read_u32(&[]).expect_err("0 bytes is not a u32");
    assert_eq!(err.field, "u32");
    assert_eq!(err.expected, 4);
    assert_eq!(err.available, 0);
}

#[test]
fn truncated_u64_reports_all_fields() {
    let err =
        read_u64(&[0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07]).expect_err("7 bytes is not a u64");
    assert_eq!(err.field, "u64");
    assert_eq!(err.expected, 8);
    assert_eq!(err.available, 7);
}

#[test]
fn truncated_i16_reports_all_fields() {
    let err = read_i16(&[]).expect_err("0 bytes is not an i16");
    assert_eq!(err.field, "i16");
    assert_eq!(err.expected, 2);
    assert_eq!(err.available, 0);
}

#[test]
fn truncated_f32_reports_all_fields() {
    let err = read_f32(&[0x01, 0x02, 0x03]).expect_err("3 bytes is not an f32");
    assert_eq!(err.field, "f32");
    assert_eq!(err.expected, 4);
    assert_eq!(err.available, 3);
}

#[test]
fn truncated_f64_reports_all_fields() {
    let err = read_f64(&[0x01; 7]).expect_err("7 bytes is not an f64");
    assert_eq!(err.field, "f64");
    assert_eq!(err.expected, 8);
    assert_eq!(err.available, 7);
}

// ------------------------------------------------------------------
// Adversarial canonical_regex_flags: Unicode, zero-width, combining
// marks must sort deterministically by char (F-IR-32).
// ------------------------------------------------------------------

#[test]
fn canonical_regex_flags_sorts_multibyte_utf8() {
    // Adversarial: multi-byte chars (hiragana, emoji) must not be split
    // into bytes or re-encoded; sorting is by Rust `char`.
    let flags = "ひ🎯";
    let canonical = canonical_regex_flags(flags);
    // '🎯' (U+1F3AF) sorts after 'ひ' (U+3072) by codepoint.
    assert_eq!(canonical, "ひ🎯", "multi-byte chars must sort by codepoint");
}

#[test]
fn canonical_regex_flags_zero_width_joiner_and_combining_marks() {
    // Adversarial: zero-width joiner (U+200D) and combining mark (U+0301)
    // must be treated as distinct chars and sorted deterministically.
    let flags = "\u{200D}\u{0301}a";
    let canonical = canonical_regex_flags(flags);
    // U+0301 (769) < U+200D (8205) < 'a' (97) ... wait, 'a' is 97, so 'a' < U+0301 < U+200D
    assert_eq!(
        canonical, "a\u{0301}\u{200D}",
        "combining marks and ZWJ sort by codepoint"
    );
}

#[test]
fn canonical_regex_flags_empty_returns_empty() {
    assert_eq!(canonical_regex_flags(""), "");
}

#[test]
fn canonical_regex_flags_single_char_returns_itself() {
    assert_eq!(canonical_regex_flags("m"), "m");
}

#[test]
fn canonical_regex_flags_deduplicates_by_char() {
    assert_eq!(canonical_regex_flags("mim"), "im");
    assert_eq!(canonical_regex_flags("🎯🎯"), "🎯");
}

#[test]
fn canonical_regex_flags_stress_10mb_alternating_must_dedup_to_one_pair() {
    // CRITIQUE_FIX_REVIEW_2026-04-23 Finding #21: existing coverage is
    // limited to tiny inputs. A 10MB string of alternating 'a' and 'b'
    // probes both allocator stress and the O(n log n) sort path (a
    // naive O(n²) dedup would time out). The output must be exactly
    // two chars ("ab") after dedup, and the call must complete well
    // under 2 seconds on a cold cache (typical real hardware finishes
    // in well under 100ms; we pick 2s as the fail-loud ceiling so
    // the test isn't flaky on shared CI).
    use std::time::Instant;

    let size = 10 * 1024 * 1024;
    let mut buf = String::with_capacity(size);
    for i in 0..size {
        buf.push(if i % 2 == 0 { 'a' } else { 'b' });
    }
    assert_eq!(buf.len(), size);

    let start = Instant::now();
    let canonical = canonical_regex_flags(&buf);
    let elapsed = start.elapsed();

    let limit_secs = if cfg!(debug_assertions) { 15 } else { 2 };
    assert_eq!(canonical, "ab", "dedup must collapse to two chars");
    assert!(
        elapsed.as_secs() < limit_secs,
        "Fix: canonical_regex_flags on 10MB alternating input took {elapsed:?}, \
         exceeding the {limit_secs}s ceiling. Likely a regression to O(n²) dedup; \
         restore O(n log n) via sort_unstable + dedup."
    );
}

#[test]
fn canonical_regex_flags_stress_1mb_random_flag_chars() {
    // Complement the alternating stress: 1MB of pseudo-random ASCII
    // flag-like chars to probe the codepoint-sort path with a realistic
    // distribution. Output length must equal the unique-codepoint count
    // and must not exceed the 128-ASCII ceiling.
    use std::time::Instant;

    let size = 1024 * 1024;
    let mut buf = String::with_capacity(size);
    // Linear congruential sequence keeps the test deterministic without a RNG
    // dependency while still producing a dense distribution across printable
    // ASCII.
    let mut state: u32 = 0x1234_5678;
    for _ in 0..size {
        state = state.wrapping_mul(1_103_515_245).wrapping_add(12345);
        let c = 0x20 + ((state >> 16) as u8 & 0x5F);
        buf.push(c as char);
    }

    let start = Instant::now();
    let canonical = canonical_regex_flags(&buf);
    let elapsed = start.elapsed();

    assert!(
        canonical.chars().count() <= 128,
        "Fix: dedup produced {} > 128 unique ASCII chars, which is \
         impossible  -  check the dedup implementation.",
        canonical.chars().count()
    );
    let limit_secs = if cfg!(debug_assertions) { 15 } else { 2 };
    assert!(
        elapsed.as_secs() < limit_secs,
        "Fix: 1MB random stress took {elapsed:?}; expected <{limit_secs}s"
    );
    // Canonical form is sorted by codepoint  -  probe adjacent pair.
    let chars: Vec<char> = canonical.chars().collect();
    for pair in chars.windows(2) {
        assert!(
            pair[0] < pair[1],
            "Fix: canonical output must be strictly sorted; got {pair:?}"
        );
    }
}

// ------------------------------------------------------------------
// Adversarial canonical_f32_zero: edge cases around signed zero and
// near-zero bit patterns (F-IR-32).
// ------------------------------------------------------------------

#[test]
fn canonical_f32_zero_negative_zero_normalises() {
    assert_eq!(canonical_f32_zero(-0.0).to_bits(), 0);
    assert_eq!(canonical_f32_zero(0.0).to_bits(), 0);
}

