//! Match post-processing: dedup, entropy, and confidence in one pass.
//!
//! The module is the canonical host reference for matcher output shaping.
//! Consumers that need device-resident post-processing use the same field
//! contract: sorted non-overlapping `(pattern_id, start, end)` spans plus
//! deterministic entropy and confidence signals.

use vyre_foundation::match_result::Match;
use vyre_primitives::matching::region::{dedup_regions_inplace, RegionTriple};

/// Post-processing contract violation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostProcessError {
    /// A match range does not fit inside the haystack that was scanned.
    InvalidRange {
        /// Pattern id attached to the invalid match.
        pattern_id: u32,
        /// Inclusive start byte offset.
        start: u32,
        /// Exclusive end byte offset.
        end: u32,
        /// Haystack length in bytes.
        haystack_len: usize,
    },
}

impl std::fmt::Display for PostProcessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            Self::InvalidRange {
                pattern_id,
                start,
                end,
                haystack_len,
            } => write!(
                f,
                "match range is outside the scanned haystack: pattern_id={pattern_id}, start={start}, end={end}, haystack_len={haystack_len}. Fix: preserve matcher readback bounds and reject corrupt hit triples before scoring."
            ),
        }
    }
}

impl std::error::Error for PostProcessError {}

/// Output of [`try_reference_post_process`]. Carries the deduped match and the
/// two derived signals every downstream consumer reads.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PostProcessedMatch {
    /// Pattern id from the original `Match`.
    pub pattern_id: u32,
    /// Inclusive start byte offset.
    pub start: u32,
    /// Exclusive end byte offset.
    pub end: u32,
    /// Shannon entropy in bits/byte over `haystack[start..end]`. `0.0`
    /// for zero-width matches.
    pub entropy_bits_per_byte: f32,
    /// `[0.0, 1.0]` confidence score combining length + entropy.
    /// Specifically `min(1, len/16) * (entropy / 8)`  -  the same
    /// heuristic a scan consumer's per-match scorer applies. The factor of 16
    /// matches the typical AKIA / ghp_ token width; entropy is
    /// normalised against the 8 bits/byte ceiling for binary-uniform
    /// data.
    pub confidence: f32,
}

/// Fuse `dedup_regions_inplace`, entropy-per-span, and confidence into one
/// Reference oracle pass over the input.
///
/// Returned vector is sorted by `(pid, start, end)` (the dedup
/// post-condition). `haystack` is the same byte buffer the matcher scanned.
///
/// # Errors
///
/// Returns [`PostProcessError::InvalidRange`] if any deduped match points
/// outside `haystack`.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_reference_post_process(
    matches: &[Match],
    haystack: &[u8],
) -> Result<Vec<PostProcessedMatch>, PostProcessError> {
    let mut triples = Vec::new();
    let mut out = Vec::new();
    try_reference_post_process_into(matches, haystack, &mut triples, &mut out)?;
    Ok(out)
}

/// Caller-owned variant of [`try_reference_post_process`].
///
/// Reuses `triples` and `out` across scans. This is the hot-path API for
/// daemons and benchmark loops that post-process thousands of small readbacks.
///
/// # Errors
///
/// Returns [`PostProcessError::InvalidRange`] if any deduped match points
/// outside `haystack`.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_reference_post_process_into(
    matches: &[Match],
    haystack: &[u8],
    triples: &mut Vec<RegionTriple>,
    out: &mut Vec<PostProcessedMatch>,
) -> Result<(), PostProcessError> {
    triples.clear();
    out.clear();
    if matches.is_empty() {
        return Ok(());
    }

    triples.reserve(matches.len());
    triples.extend(
        matches
            .iter()
            .map(|m| RegionTriple::new(m.pattern_id, m.start, m.end)),
    );
    dedup_regions_inplace(triples);

    out.reserve(triples.len());
    for &t in triples.iter() {
        let s = t.start as usize;
        let e = t.end as usize;
        if e > haystack.len() || s > e {
            out.clear();
            return Err(PostProcessError::InvalidRange {
                pattern_id: t.pid,
                start: t.start,
                end: t.end,
                haystack_len: haystack.len(),
            });
        }
        let bytes = &haystack[s..e];
        let entropy = shannon_entropy_bits_per_byte(bytes);
        let len_score = (bytes.len() as f32 / 16.0).min(1.0);
        let entropy_score = entropy / 8.0;
        let confidence = (len_score * entropy_score).clamp(0.0, 1.0);
        out.push(PostProcessedMatch {
            pattern_id: t.pid,
            start: t.start,
            end: t.end,
            entropy_bits_per_byte: entropy,
            confidence,
        });
    }
    Ok(())
}

/// Infallible reference wrapper for callers whose matcher contract has
/// already proved all ranges are within `haystack`.
///
/// Panics on corrupt match triples. Callers that need recoverable diagnostics
/// use [`try_reference_post_process`].
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_post_process(matches: &[Match], haystack: &[u8]) -> Vec<PostProcessedMatch> {
    try_reference_post_process(matches, haystack).unwrap_or_else(|error| {
        panic!("vyre-libs scan Reference oracle post-process contract failed: {error}")
    })
}

/// Shannon entropy in bits/byte. Returns `0.0` on an empty slice. The
/// implementation is straight `-sum(p_i log2 p_i)` over a 256-bucket
/// histogram  -  match cost is dominated by the haystack scan, so a
/// fixed stack histogram here is amortised on every realistic input.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn shannon_entropy_bits_per_byte(bytes: &[u8]) -> f32 {
    if bytes.is_empty() {
        return 0.0;
    }
    let counts = vyre_primitives::text::byte_histogram::reference_byte_histogram(bytes);
    let n = bytes.len() as f32;
    let mut h = 0.0_f32;
    for c in counts {
        if c == 0 {
            continue;
        }
        let p = c as f32 / n;
        h -= p * p.log2();
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn into_reuses_scratch_and_matches_allocating_api() {
        let haystack = b"AKIA1234567890ZZ";
        let matches = [
            Match::new(7, 0, 8),
            Match::new(7, 0, 8),
            Match::new(8, 4, 12),
        ];

        let expected = try_reference_post_process(&matches, haystack).unwrap();
        let mut triples = Vec::with_capacity(16);
        let triples_ptr = triples.as_ptr();
        let mut out = Vec::with_capacity(16);
        let out_ptr = out.as_ptr();

        try_reference_post_process_into(&matches, haystack, &mut triples, &mut out).unwrap();

        assert_eq!(out, expected);
        assert_eq!(triples.as_ptr(), triples_ptr);
        assert_eq!(out.as_ptr(), out_ptr);
    }

    #[test]
    fn into_clears_outputs_on_empty_input() {
        let mut triples = vec![RegionTriple::new(1, 0, 1)];
        let mut out = vec![PostProcessedMatch {
            pattern_id: 1,
            start: 0,
            end: 1,
            entropy_bits_per_byte: 0.0,
            confidence: 0.0,
        }];

        try_reference_post_process_into(&[], b"", &mut triples, &mut out).unwrap();

        assert!(triples.is_empty());
        assert!(out.is_empty());
    }

    #[test]
    fn into_reports_invalid_ranges_without_partial_output() {
        let mut triples = Vec::new();
        let mut out = Vec::new();
        let err = try_reference_post_process_into(
            &[Match::new(1, 10, 12)],
            b"short",
            &mut triples,
            &mut out,
        )
        .unwrap_err();

        assert_eq!(
            err,
            PostProcessError::InvalidRange {
                pattern_id: 1,
                start: 10,
                end: 12,
                haystack_len: 5,
            }
        );
        assert!(out.is_empty());
    }

    #[test]
    #[should_panic(expected = "post-process contract failed")]
    fn infallible_wrapper_panics_on_corrupt_ranges() {
        let _ = reference_post_process(&[Match::new(1, 10, 12)], b"short");
    }
}
