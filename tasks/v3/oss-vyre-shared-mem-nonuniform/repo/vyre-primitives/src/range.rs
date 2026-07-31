//! Byte-range primitive  -  a domain-neutral `(tag, start, end)` triple.
//!
//! CRITIQUE_VISION_ALIGNMENT_2026-04-23 V1 (forward-compatible half):
//! `vyre-foundation` currently ships a matching-domain-flavoured
//! `Match { pattern_id, start, end }` struct as its Tier-1 scan-result
//! type. That name (`Match`) and the field (`pattern_id`) pre-decide
//! that byte ranges are *matches* from a *pattern*  -  a Tier-3
//! matching-dialect concept. A crypto-decoder dialect returning decode
//! spans, an AST-span dialect, a taint-source locator, a regex
//! capture-group emitter  -  none of them have "patterns". They all
//! want `(tag, start, end)`.
//!
//! This module introduces the neutral name **without breaking the
//! foundation API**: `ByteRange` is a brand-new Tier 2.5 type that
//! lives under `vyre_primitives::range`. New dialects adopt
//! `ByteRange` directly. Legacy callers keep using `vyre::Match` as
//! long as they want; zero-cost `From` conversions bridge the two so
//! a consumer can accept either.
//!
//! The bridge below is the migration surface: new code uses
//! `ByteRange`, while legacy `vyre::Match` callers interoperate
//! through zero-cost conversions.

/// A tagged, half-open byte range `[start, end)`.
///
/// `tag` is a producer-chosen 32-bit identifier  -  a matching dialect
/// can pass a `pattern_id`, a decoder can pass an encoding ID, an
/// AST-span emitter can pass a node kind, a taint-source locator can
/// pass a source index. The producer decides what it means; the type
/// carries no domain assumption.
///
/// The struct is deliberately `#[repr(C)]` so FFI and backend
/// marshalling share one layout, and `#[non_exhaustive]` so future
/// fields (capture groups, confidence, …) can be added without
/// breaking the API.
///
/// # Examples
///
/// ```
/// use vyre_primitives::range::ByteRange;
///
/// let r = ByteRange::new(7, 10, 20);
/// assert_eq!(r.tag, 7);
/// assert_eq!(r.start, 10);
/// assert_eq!(r.end, 20);
/// assert_eq!(r.len(), 10);
/// ```
#[repr(C)]
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ByteRange {
    /// Producer-chosen 32-bit identifier. Not interpreted by this crate.
    pub tag: u32,
    /// Inclusive byte start offset.
    pub start: u32,
    /// Exclusive byte end offset.
    pub end: u32,
}

impl ByteRange {
    /// Construct a range. `end` MUST be `>= start`; the assertion
    /// catches reversed ranges in both debug and release so producers
    /// hit the bug at the call site instead of downstream.
    ///
    /// AUDIT_2026-04-24 F-RANGE-01: promoted `debug_assert!` to
    /// `assert!` so release builds can't silently accept a reversed
    /// range (which used to cascade into `len()` returning `0` and
    /// every range-containment predicate answering the wrong way).
    #[must_use]
    pub const fn new(tag: u32, start: u32, end: u32) -> Self {
        assert!(
            end >= start,
            "ByteRange end must be greater than or equal to start"
        );
        Self { tag, start, end }
    }

    /// Length of the range in bytes.
    ///
    /// AUDIT_2026-04-24 F-RANGE-02: uses plain subtraction so any
    /// reversed range triggers a panic in release. Prior
    /// `saturating_sub` hid producer bugs by returning `0` for
    /// ill-formed ranges; `new()`'s release-time assertion now
    /// prevents that state from reaching here in the first place,
    /// and the plain op gives a second fail-loud line of defense if
    /// a caller forged a `ByteRange` through the public fields.
    #[must_use]
    pub const fn len(&self) -> u32 {
        self.end - self.start
    }

    /// True when the range has zero length.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.end == self.start
    }

    /// True when `self` contains `other` (both start and end).
    #[must_use]
    pub const fn contains(&self, other: &ByteRange) -> bool {
        self.start <= other.start && other.end <= self.end
    }

    /// True when `self` ends at or before `other` starts (disjoint,
    /// `self` first). Mirrors the `Before` predicate surfaced by
    /// scanner dialects.
    #[must_use]
    pub const fn ends_before(&self, other: &ByteRange) -> bool {
        self.end <= other.start
    }
}

// Bridges to/from `vyre_foundation::match_result::Match` live
// behind any Tier-2.5 domain feature that already pulls
// vyre-foundation. The default build stays dep-free; enabling any
// primitive domain flag enables the bridges too.
#[cfg(feature = "vyre-foundation")]
mod match_bridge {
    use super::ByteRange;

    impl From<vyre_foundation::match_result::Match> for ByteRange {
        fn from(m: vyre_foundation::match_result::Match) -> Self {
            ByteRange::new(m.pattern_id, m.start, m.end)
        }
    }

    impl From<ByteRange> for vyre_foundation::match_result::Match {
        fn from(r: ByteRange) -> Self {
            vyre_foundation::match_result::Match::new(r.tag, r.start, r.end)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_roundtrip() {
        let r = ByteRange::new(42, 100, 200);
        assert_eq!(r.tag, 42);
        assert_eq!(r.start, 100);
        assert_eq!(r.end, 200);
        assert_eq!(r.len(), 100);
        assert!(!r.is_empty());
    }

    #[test]
    fn empty_is_zero_length() {
        let r = ByteRange::new(1, 5, 5);
        assert!(r.is_empty());
        assert_eq!(r.len(), 0);
    }

    #[test]
    fn contains_inclusive_bounds() {
        let outer = ByteRange::new(0, 0, 100);
        let inner = ByteRange::new(0, 10, 90);
        assert!(outer.contains(&inner));
        assert!(!inner.contains(&outer));
        // A range contains itself.
        assert!(outer.contains(&outer));
    }

    #[test]
    fn ends_before_requires_disjoint() {
        let a = ByteRange::new(0, 0, 10);
        let b = ByteRange::new(0, 10, 20);
        let c = ByteRange::new(0, 5, 15);
        assert!(a.ends_before(&b));
        assert!(!a.ends_before(&c));
    }

    #[cfg(feature = "vyre-foundation")]
    #[test]
    fn bridge_from_match_preserves_fields() {
        let m = vyre_foundation::match_result::Match::new(7, 11, 22);
        let r: ByteRange = m.into();
        assert_eq!(r.tag, 7);
        assert_eq!(r.start, 11);
        assert_eq!(r.end, 22);
    }

    #[cfg(feature = "vyre-foundation")]
    #[test]
    fn bridge_back_to_match_preserves_fields() {
        let r = ByteRange::new(9, 13, 33);
        let m: vyre_foundation::match_result::Match = r.into();
        assert_eq!(m.pattern_id, 9);
        assert_eq!(m.start, 13);
        assert_eq!(m.end, 33);
    }

    #[test]
    fn layout_is_repr_c_u32x3() {
        // The layout stability is load-bearing for backend marshalling
        // and FFI. Pinning here means future field additions cannot
        // quietly break the wire shape without this test flipping.
        assert_eq!(std::mem::size_of::<ByteRange>(), 12);
        assert_eq!(std::mem::align_of::<ByteRange>(), 4);
    }
}
