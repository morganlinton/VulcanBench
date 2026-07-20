//! Native scan match result  -  **legacy scan-domain shim.**
//!
//! CRITIQUE_VISION_ALIGNMENT_2026-04-23 V1: this type was the Tier-1
//! return shape for every byte-range scan in vyre. Its field name
//! (`pattern_id`) pre-decided that every byte range is a "match"
//! from a "pattern"  -  a matching-dialect concept that shouldn't
//! live in foundation. A crypto decoder, an AST-span emitter, or a
//! capture-group producer would either adopt matching vocabulary
//! awkwardly or ship a parallel type.
//!
//! The canonical neutral name is `ByteRange`. `Match` remains here as
//! a backward-compat scan-domain shape. Bridges between the two types
//! are zero-cost (`repr(C)` u32×3 on both sides).
//!
//! The full migration removes `Match` entirely; we keep it for one
//! release so dependent crates don't hard-break.

/// A tagged, half-open byte range `[start, end)`.
///
/// `tag` is a producer-chosen 32-bit identifier. A matching dialect can pass a
/// pattern id, a decoder can pass an encoding id, and a source-span producer
/// can pass a node kind. Foundation does not interpret the field.
#[repr(C)]
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ByteRange {
    /// Producer-chosen 32-bit identifier.
    pub tag: u32,
    /// Inclusive byte start offset.
    pub start: u32,
    /// Exclusive byte end offset.
    pub end: u32,
}

impl ByteRange {
    /// Construct a range. Reversed ranges fail loudly because accepting them
    /// corrupts every downstream range predicate.
    #[must_use]
    pub const fn new(tag: u32, start: u32, end: u32) -> Self {
        assert!(
            end >= start,
            "ByteRange::new requires end >= start. Fix: pass half-open byte ranges as [start, end)."
        );
        Self { tag, start, end }
    }

    /// Length of the range in bytes.
    #[must_use]
    pub const fn len(&self) -> u32 {
        self.end - self.start
    }

    /// True when the range has zero length.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.end == self.start
    }

    /// True when `self` contains `other`.
    #[must_use]
    pub const fn contains(&self, other: &ByteRange) -> bool {
        self.start <= other.start && other.end <= self.end
    }

    /// True when `self` ends at or before `other` starts.
    #[must_use]
    pub const fn ends_before(&self, other: &ByteRange) -> bool {
        self.end <= other.start
    }
}

/// A byte-range match emitted by vyre scanning engines.
///
/// **Deprecated:** callers should migrate to
/// [`ByteRange`]. The two types share layout and the `From` bridges are
/// zero-cost.
///
/// Background: `pattern_id` is a matching-dialect concept. The
/// neutral name on the new type is `tag`; the producer decides what
/// it means (pattern id, encoding id, AST kind, source index, …).
/// CRITIQUE_VISION_ALIGNMENT_2026-04-23 V1.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Match {
    /// Stable pattern identifier that produced the match.
    pub pattern_id: u32,
    /// Inclusive byte start offset.
    pub start: u32,
    /// Exclusive byte end offset.
    pub end: u32,
}

impl Match {
    /// Construct a match from its pattern id and byte range.
    ///
    /// This constructor is a const fn so that engines can emit match
    /// literals at compile time. The byte range is half-open `[start, end)`
    /// to match Rust slicing conventions.
    ///
    /// # Examples
    ///
    /// ```
    /// use vyre::Match;
    ///
    /// let m = Match::new(1, 10, 20);
    /// assert_eq!(m.pattern_id, 1);
    /// assert_eq!(m.start, 10);
    /// assert_eq!(m.end, 20);
    /// ```
    #[must_use]
    pub const fn new(pattern_id: u32, start: u32, end: u32) -> Self {
        Self {
            pattern_id,
            start,
            end,
        }
    }
}

impl From<Match> for ByteRange {
    fn from(value: Match) -> Self {
        ByteRange::new(value.pattern_id, value.start, value.end)
    }
}

impl From<ByteRange> for Match {
    fn from(value: ByteRange) -> Self {
        Match::new(value.tag, value.start, value.end)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn construction() {
        let m = Match::new(1, 10, 20);
        assert_eq!(m.pattern_id, 1);
        assert_eq!(m.start, 10);
        assert_eq!(m.end, 20);
    }

    #[test]
    fn ordering() {
        let a = Match::new(0, 5, 10);
        let b = Match::new(0, 15, 20);
        let c = Match::new(1, 0, 5);
        let mut v = [c, a, b];
        v.sort();
        assert_eq!(v[0].start, 5);
        assert_eq!(v[1].start, 15);
        assert_eq!(v[2].pattern_id, 1);
    }

    #[test]
    fn clone_and_eq() {
        let a = Match::new(1, 0, 100);
        let b = a;
        assert_eq!(a, b);
    }

    #[test]
    fn hash_consistency() {
        let mut set = HashSet::new();
        let m = Match::new(1, 0, 10);
        set.insert(m);
        assert!(set.contains(&Match::new(1, 0, 10)));
        assert!(!set.contains(&Match::new(2, 0, 10)));
    }

    #[test]
    fn byte_range_bridge_preserves_fields() {
        let range = ByteRange::new(7, 11, 22);
        let matched: Match = range.into();
        assert_eq!(matched.pattern_id, 7);
        assert_eq!(matched.start, 11);
        assert_eq!(matched.end, 22);
        let roundtrip: ByteRange = matched.into();
        assert_eq!(roundtrip, range);
    }

    #[test]
    #[should_panic(expected = "ByteRange::new requires end >= start")]
    fn byte_range_rejects_reversed_ranges() {
        let _ = ByteRange::new(1, 10, 9);
    }
}
