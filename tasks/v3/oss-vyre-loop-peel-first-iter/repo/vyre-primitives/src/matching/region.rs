//! Span-region dedup primitive.
//!
//! Every multimatch consumer (`vyre-libs::matching` engines, scanner consumer,
//! external analyzer) ends up doing the same operation after the GPU dispatch
//! returns: take the raw `Vec<Match>`, collapse adjacent overlapping
//! or duplicate spans into a representative, return the deduped set.
//! Each consumer wrote it differently  -  some by `(detector_id,
//! credential)` HashMap, some by `(start, end)` pair sort, some by ad-
//! hoc loop. The lego-block fix is one primitive every consumer calls.
//!
//! # Algorithm
//!
//! Given a slice of `(pid, start, end)` triples sorted by `(pid, start, end)`,
//! emit one representative per maximal cluster of triples that
//! overlap or touch (`start[i] <= end[max_end_so_far]`) AND have the same
//! `pid`. This collapses both:
//!
//!   - `(pid=0, 5, 10)` and `(pid=0, 6, 11)` → `(pid=0, 5, 11)`
//!     (overlapping, same pattern  -  extend span).
//!   - `(pid=0, 5, 10)` and `(pid=0, 5, 10)` → one entry
//!     (exact dup).
//!
//! Distinct `pid`s never merge  -  two patterns matching the same
//! region produce two output spans (cross-pattern dedup is a
//! different operation; consumers that want it apply a second pass).
//!
//! # CPU + GPU
//!
//! - `dedup_regions_cpu` is the reference implementation: pure data,
//!   no IR, no backend. CPU-side consumers and parity tests use it.
//! - `region_sort_program` and `dedup_regions_cluster_program` emit
//!   GPU-resident sorted spans, survivor flags, and merged cluster ends
//!   so parser/scanner pipelines can compact deduped triples without a
//!   host readback between stages.
//!
//! Both share a single golden test fixture set so any divergence is
//! caught at conform time.

use std::cmp::Ordering;

pub use super::region_programs::{
    dedup_regions_cluster_program, dedup_regions_flag_program, region_dedup_dispatch_grid,
    region_sort_program, DEDUP_REGIONS_CLUSTER_OP_ID, DEDUP_REGIONS_FLAG_OP_ID,
    REGION_DEDUP_WORKGROUP_SIZE,
};

/// One match as exposed by `vyre_foundation::match_result::Match`  -
/// duplicated here as a plain triple so this primitive doesn't depend
/// on foundation. Consumers convert at the boundary.
///
/// `pid`: pattern id; `start` / `end`: byte offsets, half-open
/// `[start, end)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegionTriple {
    /// Pattern id (which detector emitted this match).
    pub pid: u32,
    /// Inclusive start byte offset.
    pub start: u32,
    /// Exclusive end byte offset.
    pub end: u32,
}

impl RegionTriple {
    /// Construct a region triple. `end` must be `>= start`; equal
    /// values represent a zero-width match (legal for some regex
    /// constructs).
    #[must_use]
    pub const fn new(pid: u32, start: u32, end: u32) -> Self {
        Self { pid, start, end }
    }
}

impl Ord for RegionTriple {
    fn cmp(&self, other: &Self) -> Ordering {
        // Sort by (pid, start, end) so the dedup loop sees cluster
        // members consecutively without a secondary group-by pass.
        self.pid
            .cmp(&other.pid)
            .then(self.start.cmp(&other.start))
            .then(self.end.cmp(&other.end))
    }
}

impl PartialOrd for RegionTriple {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Reference CPU implementation: collapse same-pid overlapping spans.
///
/// Sort happens inline (`sort_unstable`); the input may arrive in any
/// order. Pre-sorted callers should still see linear behavior since
/// `sort_unstable` is `O(n log n)` worst case, `O(n)` on already-
/// sorted input.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn dedup_regions_cpu(input: Vec<RegionTriple>) -> Vec<RegionTriple> {
    let mut owned = input;
    dedup_regions_inplace(&mut owned);
    owned
}

/// CPU reference for [`region_sort_program`]  -  stable lexicographic
/// sort of `(pid, start, end)` triples by composite key.
///
/// `dedup_regions_inplace` already sorts internally, so callers that
/// only want dedup don't need this helper. It exists for parity tests
/// against the GPU sort and for pipelines that need the sorted-but-
/// not-yet-deduped view (e.g. when stream_compact runs separately).
#[cfg(any(test, feature = "cpu-parity"))]
pub fn sort_regions_cpu(input: &mut [RegionTriple]) {
    input.sort();
}

/// Sort and merge overlapping regions in place.
///
/// Regions are ordered by `(pid, start, end)`. Adjacent entries with the same
/// pattern id and overlapping or touching byte spans are coalesced into a
/// single [`RegionTriple`]. The vector is truncated to the deduplicated length.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn dedup_regions_inplace(input: &mut Vec<RegionTriple>) {
    if input.is_empty() {
        return;
    }
    input.sort_unstable();

    // Two-cursor compaction: `write` indexes the next slot to populate,
    // `read` walks the (sorted) input. Each merge folds the read entry
    // into `input[write - 1]`; each non-merge advances `write`.
    let mut write = 1usize;
    for read in 1..input.len() {
        let next = input[read];
        let last = input[write - 1];
        let same_pid = next.pid == last.pid;
        let overlap_or_touch = next.start <= last.end;
        if same_pid && overlap_or_touch {
            if next.end > last.end {
                input[write - 1].end = next.end;
            }
        } else {
            input[write] = next;
            write += 1;
        }
    }
    input.truncate(write);
}
