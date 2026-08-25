//! Semantic canonicalisation for opaque-payload hash equality.
//!
//! Helpers that normalise payload bytes so two semantically-equivalent
//! programs hash to the same value under `crate::ir::Program::hash`.
//! These concerns are separate from wire-format endianness; they live
//! here so extension authors can opt into canonicalisation without
//! mixing it with the byte-level encode/decode primitives.

/// Canonicalise an inline regex-flag set so payloads encoding the same
/// semantic regex hash equal under `crate::ir::Program::hash`.
///
/// Rust/ICU-style inline regex literals of the shape `(?flags)pattern`
/// accept the flag characters in any order  -  `(?mi)` and `(?im)`
/// describe the same regex but the differing byte sequences hash
/// distinctly, defeating CSE and cache lookups (F-IR-02 / F-IR-37).
/// Extension authors who encode a regex literal into an opaque payload
/// MUST call this helper on the flag string before framing so two
/// semantically-equal regexes reach the encoder as the same bytes.
///
/// The returned string is the deduplicated, sorted flag set. The input
/// may contain any UTF-8 characters; deduplication is by `char` equality.
#[must_use]
pub fn canonical_regex_flags(flags: &str) -> String {
    let mut chars: Vec<char> = flags.chars().collect();
    chars.sort_unstable();
    chars.dedup();
    chars.into_iter().collect()
}

/// Canonicalise an `f32` bit pattern for opaque-payload hash equality.
///
/// `-0.0` and `+0.0` compare equal under IEEE-754 `==` but have
/// different bit patterns, so a naive `to_le_bytes` encoder makes them
/// hash distinctly. Extension authors who want two programs that differ
/// only by the sign of zero to hash equal call this helper before
/// [`super::endian::push_f32`]. Non-zero floats and every NaN bit pattern are
/// returned unchanged  -  callers preserve their distinct identities
/// because sign-of-zero is the only "semantically equal but
/// bit-distinct" shape common enough to justify canonicalising at the
/// wire layer.
///
/// See F-IR-37 in CRITIQUE_IR_SOUNDNESS_2026-04-22.md for the full
/// rationale. The 64-bit companion is [`canonical_f64_zero`]  -  every
/// extension that wants cross-width sign-of-zero canonicalisation
/// should use the helper matching the literal's width to avoid bit
/// pattern drift between f32 and f64 encodings.
#[must_use]
pub fn canonical_f32_zero(value: f32) -> f32 {
    if value == 0.0 && value.is_sign_negative() {
        0.0
    } else {
        value
    }
}

/// Canonicalise an `f64` bit pattern for opaque-payload hash equality.
///
/// Same contract as [`canonical_f32_zero`] but for the 64-bit width:
/// `-0.0f64` bit pattern (`0x8000_0000_0000_0000`) normalises to
/// `+0.0f64` (`0x0000_0000_0000_0000`) so two programs that differ
/// only by the sign of zero hash equal under `Program::hash`. Every
/// non-zero f64 (including every signalling and quiet NaN in either
/// sign, every subnormal, every finite with a non-zero mantissa) is
/// returned unchanged  -  callers preserve the distinct bit patterns
/// that carry IEEE-754 semantic meaning (NaN payloads, signed zero in
/// division, etc.).
///
/// CRITIQUE_FIX_REVIEW_2026-04-23 Finding #11: without this helper,
/// f64 opaque extensions had no canonicalisation path, so two
/// semantically-equal programs differing only by `-0.0f64 → +0.0f64`
/// would hash distinctly and defeat CSE / cache lookups.
#[must_use]
pub fn canonical_f64_zero(value: f64) -> f64 {
    if value == 0.0 && value.is_sign_negative() {
        0.0
    } else {
        value
    }
}
