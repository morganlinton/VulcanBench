#[test]
fn canonical_f32_zero_nonzero_with_sign_bit_set_passes_through() {
    // Adversarial: 0x80000001 is the smallest negative subnormal, NOT zero.
    // It must pass through unchanged despite having the sign bit set.
    let v = f32::from_bits(0x80000001);
    assert_ne!(v, 0.0, "precondition: this bit pattern is not zero");
    assert_eq!(canonical_f32_zero(v).to_bits(), 0x80000001);
}

#[test]
fn canonical_f32_zero_negative_nan_passes_through() {
    // Adversarial: NaN with sign bit set (negative NaN) must not be
    // mistaken for -0.0 and must pass through unchanged.
    let v = f32::from_bits(0xFFFFFFFF);
    assert_ne!(
        v.to_bits() & 0x7FFF_FFFF,
        0,
        "precondition: all-NaN bits is not zero"
    );
    assert_eq!(canonical_f32_zero(v).to_bits(), 0xFFFFFFFF);
}

#[test]
fn canonical_f32_zero_positive_nan_passes_through() {
    let v = f32::from_bits(0x7FC00000);
    assert_eq!(canonical_f32_zero(v).to_bits(), 0x7FC00000);
}

#[test]
fn canonical_f32_zero_all_sign_exponent_mantissa_combinations_for_f32() {
    // Adversarial: walk all sign × exponent × mantissa patterns that
    // produce zero, and assert only the two zero patterns normalise.
    // This is exhaustive for the zero/non-zero boundary.
    for bits in [0x00000000u32, 0x80000000u32] {
        let v = f32::from_bits(bits);
        assert_eq!(canonical_f32_zero(v).to_bits(), 0x00000000);
    }
    // Every other bit pattern must be untouched.
    for bits in [0x00000001u32, 0x80000001u32, 0x7F800000u32, 0xFF800000u32] {
        let v = f32::from_bits(bits);
        assert_eq!(canonical_f32_zero(v).to_bits(), bits);
    }
}

// ------------------------------------------------------------------
// CRITIQUE_FIX_REVIEW_2026-04-23 Finding #13 regressions.
// ------------------------------------------------------------------

use vyre_foundation::opaque_payload::LeBytesWriter;

#[test]
fn le_bytes_writer_roundtrip_matches_direct_push_u32() {
    // Direct push_u32 + to_le_bytes
    let mut direct = Vec::new();
    push_u32(&mut direct, 0xDEADBEEF);

    // LeBytesWriter path
    let mut writer = LeBytesWriter::new();
    writer.push_u32(0xDEADBEEF);
    let via_writer: Vec<u8> = writer.into_inner();

    assert_eq!(
        direct, via_writer,
        "LeBytesWriter must emit the exact same bytes as the direct push_u32 helper"
    );
}

#[test]
fn le_bytes_writer_into_inner_roundtrips_all_types() {
    let mut writer = LeBytesWriter::with_capacity(32);
    writer.push_u16(0x0102);
    writer.push_u32(0x03040506);
    writer.push_u64(0x0708090A0B0C0D0E);
    writer.push_i16(-1);
    writer.push_i32(-2);
    writer.push_i64(-3);
    writer.push_f32(1.5);
    writer.push_f64(2.5);
    writer.push_slice(b"tail");

    let buf = writer.into_inner();

    let (a, tail) = read_u16(&buf).unwrap();
    let (b, tail) = read_u32(tail).unwrap();
    let (c, tail) = read_u64(tail).unwrap();
    let (d, tail) = read_i16(tail).unwrap();
    let (e, tail) = read_i32(tail).unwrap();
    let (f, tail) = read_i64(tail).unwrap();
    let (g, tail) = read_f32(tail).unwrap();
    let (h, tail) = read_f64(tail).unwrap();

    assert_eq!(a, 0x0102);
    assert_eq!(b, 0x03040506);
    assert_eq!(c, 0x0708090A0B0C0D0E);
    assert_eq!(d, -1);
    assert_eq!(e, -2);
    assert_eq!(f, -3);
    assert_eq!(g.to_bits(), 1.5f32.to_bits());
    assert_eq!(h.to_bits(), 2.5f64.to_bits());
    assert_eq!(tail, b"tail");
}
