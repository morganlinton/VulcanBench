//! Endian-fixed encode/decode helpers + semantic canonicalisation for
//! opaque extension payloads.
//!
//! `Expr::Opaque` and `Node::Opaque` carry a `Vec<u8>` payload owned by the
//! dialect that issued the node. The wire framing is byte-identical across
//! architectures, so extension authors MUST NOT use `to_ne_bytes` or any
//! host-endian serialization  -  a Program encoded on one host and decoded on
//! another must reproduce the same `crate::ir::Program::hash` and the same
//! IR. Using `to_le_bytes` everywhere is the only way to honour that
//! contract.
//!
//! This module collects the little-endian primitives an extension
//! implementor is most likely to reach for. They are thin wrappers around
//! the standard-library `to_le_bytes` / `from_le_bytes` methods, intended
//! to make the right choice the obvious one and surface actionable
//! diagnostics when an input buffer is truncated.
//!
//! See `vyre-foundation/tests/opaque_payload_endian.rs` for the regression
//! suite covering F-IR-32.

pub mod canonicalize;
pub mod endian;

// Re-exports for backward compatibility during the migration window.
pub use canonicalize::{canonical_f32_zero, canonical_f64_zero, canonical_regex_flags};
pub use endian::{
    push_f32, push_f64, push_i16, push_i32, push_i64, push_u16, push_u32, push_u64, read_f32,
    read_f64, read_i16, read_i32, read_i64, read_u16, read_u32, read_u64, LeBytesWriter,
    OpaquePayloadTruncated,
};

#[cfg(test)]
mod tests {
    use super::canonicalize::{canonical_f32_zero, canonical_f64_zero, canonical_regex_flags};

    #[test]
    fn regex_flags_are_sorted_and_deduped() {
        assert_eq!(canonical_regex_flags("im"), canonical_regex_flags("mi"));
        assert_eq!(canonical_regex_flags("mim"), "im");
        assert_eq!(canonical_regex_flags(""), "");
        assert_eq!(canonical_regex_flags("abc"), "abc");
        assert_eq!(canonical_regex_flags("cba"), "abc");
    }

    #[test]
    fn negative_zero_canonicalises_to_positive_zero() {
        assert_eq!(canonical_f32_zero(-0.0).to_bits(), 0);
        assert_eq!(canonical_f32_zero(0.0).to_bits(), 0);
    }

    #[test]
    fn non_zero_floats_and_nans_pass_through_unchanged() {
        let payload = f32::from_bits(0xDEADBEEF);
        assert_eq!(canonical_f32_zero(payload).to_bits(), 0xDEADBEEF);
        assert_eq!(canonical_f32_zero(1.0).to_bits(), 1.0f32.to_bits());
        assert_eq!(canonical_f32_zero(-1.0).to_bits(), (-1.0f32).to_bits());
    }

    // CRITIQUE_FIX_REVIEW_2026-04-23 Finding #11 regressions.

    #[test]
    fn f64_negative_zero_canonicalises_to_positive_zero() {
        assert_eq!(canonical_f64_zero(-0.0_f64).to_bits(), 0u64);
        assert_eq!(canonical_f64_zero(0.0_f64).to_bits(), 0u64);
    }

    #[test]
    fn f64_non_zero_floats_pass_through_unchanged() {
        // Smallest negative subnormal  -  has sign bit set but is NOT zero.
        let smallest_neg_subnormal = f64::from_bits(0x8000_0000_0000_0001);
        assert_eq!(
            canonical_f64_zero(smallest_neg_subnormal).to_bits(),
            0x8000_0000_0000_0001
        );
        // Negative finite.
        assert_eq!(canonical_f64_zero(-1.0_f64).to_bits(), (-1.0_f64).to_bits());
        // Quiet NaN with sign bit set  -  payload preserved.
        let qnan_neg = f64::from_bits(0xFFF8_0000_0000_0001);
        assert_eq!(
            canonical_f64_zero(qnan_neg).to_bits(),
            0xFFF8_0000_0000_0001
        );
        // Signalling NaN with sign bit clear  -  payload preserved.
        let snan_pos = f64::from_bits(0x7FF0_0000_0000_0001);
        assert_eq!(
            canonical_f64_zero(snan_pos).to_bits(),
            0x7FF0_0000_0000_0001
        );
    }
}
