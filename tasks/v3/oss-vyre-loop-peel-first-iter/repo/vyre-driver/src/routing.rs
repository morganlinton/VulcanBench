//! Runtime distribution-aware algorithm routing.
//!
//! `routing` records light-weight input distribution summaries per call site
//! and chooses a byte-identical algorithm variant for the next dispatch. The
//! first users are sort-like ops where tiny inputs prefer insertion sort and
//! skewed large inputs prefer radix-style passes.

/// Profile-guided backend routing table and cert-gate latency measurement.
pub mod pgo;

use dashmap::DashMap;
use std::borrow::Cow;
use std::sync::Arc;

/// Sort algorithm variants with identical output contracts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SortBackend {
    /// Insertion sort: minimal fixed overhead for small inputs.
    InsertionSort,
    /// Radix sort: stable throughput for skewed integer distributions.
    RadixSort,
    /// Bitonic sort: GPU-friendly general-purpose fallback.
    BitonicSort,
}

/// Observed input distribution for one call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Distribution {
    len: usize,
    unique: usize,
    max_run: usize,
}

impl Distribution {
    /// Build a distribution summary from u32 inputs.
    #[must_use]
    pub fn observe(values: &[u32]) -> Self {
        if values.is_empty() {
            return Self {
                len: 0,
                unique: 0,
                max_run: 0,
            };
        }
        let mut unique = FixedUniqueU32::default();
        let mut max_run = 1usize;
        let mut current_run = 1usize;
        unique.observe(values[0]);
        for window in values.windows(2) {
            unique.observe(window[1]);
            if window[0] == window[1] {
                current_run += 1;
                max_run = max_run.max(current_run);
            } else {
                current_run = 1;
            }
        }
        Self {
            len: values.len(),
            unique: unique.unique_len(values.len()),
            max_run,
        }
    }

    #[must_use]
    fn skew_ratio(self) -> f32 {
        if self.len == 0 {
            return 0.0;
        }
        1.0 - (self.unique as f32 / self.len as f32)
    }
}

const INLINE_UNIQUE_CAP: usize = 512;

struct FixedUniqueU32 {
    values: [u32; INLINE_UNIQUE_CAP],
    len: usize,
    overflowed: bool,
}

impl Default for FixedUniqueU32 {
    fn default() -> Self {
        Self {
            values: [0; INLINE_UNIQUE_CAP],
            len: 0,
            overflowed: false,
        }
    }
}

impl FixedUniqueU32 {
    fn observe(&mut self, value: u32) {
        if self.values[..self.len].contains(&value) {
            return;
        }
        if self.len == INLINE_UNIQUE_CAP {
            self.overflowed = true;
            return;
        }
        self.values[self.len] = value;
        self.len += 1;
    }

    fn unique_len(&self, input_len: usize) -> usize {
        if self.overflowed {
            input_len
        } else {
            self.len
        }
    }
}

/// Per-call-site profile used by routing decisions.
#[derive(Debug, Default)]
pub struct RoutingTable {
    profiles: DashMap<Arc<str>, Distribution>,
}

impl RoutingTable {
    /// Record one call-site observation and return the selected backend.
    ///
    /// # Errors
    ///
    /// Returns an error if the routing table mutex is poisoned.
    pub fn observe_sort_u32(
        &self,
        call_site: Cow<'_, str>,
        values: &[u32],
    ) -> Result<SortBackend, String> {
        let distribution = Distribution::observe(values);
        let key = match call_site {
            Cow::Borrowed(value) => Arc::<str>::from(value),
            Cow::Owned(value) => Arc::<str>::from(value.into_boxed_str()),
        };
        self.profiles.insert(key, distribution);
        Ok(select_sort_backend(distribution))
    }

    /// Return the last observed distribution for a call site.
    #[must_use]
    pub fn distribution(&self, call_site: &str) -> Option<Distribution> {
        self.profiles.get(call_site).map(|profile| *profile)
    }
}

/// Pick a sort backend from a distribution summary.
#[must_use]
pub fn select_sort_backend(distribution: Distribution) -> SortBackend {
    if distribution.len <= 32 {
        SortBackend::InsertionSort
    } else if distribution.skew_ratio() >= 0.75 || distribution.max_run >= 16 {
        SortBackend::RadixSort
    } else {
        SortBackend::BitonicSort
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skewed_input_picks_radix_sort() {
        let table = RoutingTable::default();
        let mut values = vec![7u32; 240];
        values.extend(0..16);
        let selected = table
            .observe_sort_u32(Cow::Borrowed("sort.callsite.skewed"), &values)
            .expect("Fix: routing profile should record");
        assert_eq!(selected, SortBackend::RadixSort);
    }

    #[test]
    fn small_input_picks_insertion_sort() {
        let table = RoutingTable::default();
        let selected = table
            .observe_sort_u32(Cow::Borrowed("sort.callsite.small"), &[4, 1, 3, 2])
            .expect("Fix: routing profile should record");
        assert_eq!(selected, SortBackend::InsertionSort);
    }

    #[test]
    fn pgo_picks_fastest_backend_per_op() {
        let table = RoutingTable::default();
        assert_eq!(
            table
                .observe_sort_u32(Cow::Borrowed("op.sort"), &[8, 3, 1])
                .unwrap(),
            SortBackend::InsertionSort
        );
        assert_eq!(
            table
                .observe_sort_u32(Cow::Borrowed("op.sort"), &vec![42; 128])
                .unwrap(),
            SortBackend::RadixSort
        );
        assert_eq!(
            table
                .distribution("op.sort")
                .expect("Fix: profile retained")
                .len,
            128
        );
    }
}
