//! ROADMAP I1  -  hot-path recording into optimizer hints.
//!
//! Foundation-side substrate. Backends record per-Region dispatch
//! latency at runtime; the optimizer reads the recorded hints to
//! decide which passes to prioritise on the hot path and which to
//! skip on the cold path.
//!
//! ## Contract
//!
//! `HotPathHints` is the canonical key/value store. Backends append
//! `(region_generator, RegionRecord)` rows after each dispatch.
//! The optimizer queries `is_hot(region_generator)` /
//! `dispatch_count(region_generator)` /
//! `mean_kernel_ns(region_generator)` to decide pass scheduling
//! per region. The default `HotPathHints::default()` is empty  -
//! every region is assumed cold until recorded otherwise, so the
//! optimizer falls back to its default schedule when no PGO data
//! exists.
//!
//! Sample concentration: bounded LRU keyed by region generator,
//! capacity tunable via `with_capacity`. New samples accumulate
//! into the existing `RegionRecord` for the matching key (running
//! mean + max), so the table never grows beyond the capacity even
//! across long-running workloads.
//!
//! ## Why on-foundation, not on-driver
//!
//! The hint table is queried by the foundation_optimizer scheduler
//! before any backend is selected. Putting the table here lets
//! every backend feed into the same store without leaking backend-
//! specific types into the optimizer. Backends own *recording*
//! (each backend hooks its own dispatch finalize); the optimizer
//! owns *consuming*.
//!
//! ## Soundness for the optimizer
//!
//! The `is_hot` threshold is a heuristic  -  passes that consume the
//! hint must remain correct (just not optimal) when the hint is
//! absent or stale. The optimizer must NEVER turn a soundness gate
//! into a hot-path-only gate. The hint is a *prioritisation*
//! signal, not a *correctness* signal.

use dashmap::DashMap;
use std::sync::atomic::{AtomicU64, Ordering};

/// Per-region performance record. Mean over recorded samples is a
/// stable proxy for the steady-state kernel cost; max captures the
/// worst-case dispatch the backend has seen.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RegionRecord {
    /// Number of dispatch samples observed for this region
    /// generator across the recording window.
    pub dispatch_count: u64,
    /// Sum of `kernel_execute_ns` across all samples; divide by
    /// `dispatch_count` for the running mean.
    pub kernel_ns_total: u64,
    /// Maximum `kernel_execute_ns` seen so far.
    pub kernel_ns_max: u64,
    /// Sum of `bytes_touched` across all samples.
    pub bytes_total: u64,
}

impl RegionRecord {
    /// Mean kernel-execute time across recorded samples, in
    /// nanoseconds. Returns `0` when no sample has been recorded.
    #[must_use]
    pub fn mean_kernel_ns(&self) -> u64 {
        self.kernel_ns_total
            .checked_div(self.dispatch_count)
            .unwrap_or(0)
    }

    /// Mean bytes touched per dispatch.
    #[must_use]
    pub fn mean_bytes(&self) -> u64 {
        self.bytes_total
            .checked_div(self.dispatch_count)
            .unwrap_or(0)
    }
}

/// Entry stored in the concurrent hint map.  The `timestamp` is a
/// monotonic counter used for lazy LRU eviction.
#[derive(Clone, Debug)]
struct HintEntry {
    record: RegionRecord,
    timestamp: u64,
}

/// Bounded LRU store of per-region performance records. Cheap to
/// clone (independent copy of internal metadata), Send + Sync.
pub struct HotPathHints {
    records: DashMap<String, HintEntry>,
    capacity: usize,
    hot_ns_threshold: AtomicU64,
    clock: AtomicU64,
}

impl Clone for HotPathHints {
    fn clone(&self) -> Self {
        Self {
            records: self.records.clone(),
            capacity: self.capacity,
            hot_ns_threshold: AtomicU64::new(self.hot_ns_threshold.load(Ordering::Relaxed)),
            clock: AtomicU64::new(self.clock.load(Ordering::Relaxed)),
        }
    }
}

impl HotPathHints {
    /// Build a hint store with the given LRU capacity. `capacity == 0`
    /// disables recording (all queries return defaults). The default
    /// hot-ns threshold is `100_000` (100µs)  -  overrideable via
    /// [`with_hot_threshold_ns`](Self::with_hot_threshold_ns).
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            records: DashMap::with_capacity(capacity.max(1)),
            capacity,
            hot_ns_threshold: AtomicU64::new(100_000),
            clock: AtomicU64::new(0),
        }
    }

    /// Override the kernel-execute-ns threshold above which a
    /// region is considered hot. Default is 100µs.
    #[must_use]
    pub fn with_hot_threshold_ns(self, threshold_ns: u64) -> Self {
        self.hot_ns_threshold.store(threshold_ns, Ordering::Relaxed);
        self
    }

    /// Record a dispatch sample for `region_generator`. Existing
    /// record gets accumulated; new region triggers LRU eviction
    /// when at capacity.
    pub fn record(&self, region_generator: &str, kernel_ns: u64, bytes_touched: u64) {
        if self.capacity == 0 {
            return;
        }
        let key = region_generator.to_owned();
        let timestamp = self.clock.fetch_add(1, Ordering::Relaxed);
        let mut entry = self.records.entry(key.clone()).or_insert(HintEntry {
            record: RegionRecord {
                dispatch_count: 0,
                kernel_ns_total: 0,
                kernel_ns_max: 0,
                bytes_total: 0,
            },
            timestamp,
        });
        entry.record.dispatch_count = entry.record.dispatch_count.saturating_add(1);
        entry.record.kernel_ns_total = entry.record.kernel_ns_total.saturating_add(kernel_ns);
        if kernel_ns > entry.record.kernel_ns_max {
            entry.record.kernel_ns_max = kernel_ns;
        }
        entry.record.bytes_total = entry.record.bytes_total.saturating_add(bytes_touched);
        entry.timestamp = timestamp;
        drop(entry);

        while self.records.len() > self.capacity {
            let oldest = self
                .records
                .iter()
                .map(|e| (e.key().clone(), e.value().timestamp))
                .min_by_key(|(_, ts)| *ts)
                .map(|(k, _)| k);
            if let Some(k) = oldest {
                self.records.remove(&k);
            } else {
                break;
            }
        }
    }

    /// True iff `region_generator`'s recorded mean kernel-ns
    /// exceeds the hot threshold. Cold (or unrecorded) regions
    /// return false  -  passes that gate on `is_hot` must remain
    /// correct on the cold path.
    #[must_use]
    pub fn is_hot(&self, region_generator: &str) -> bool {
        let threshold = self.hot_ns_threshold.load(Ordering::Relaxed);
        self.records
            .get(region_generator)
            .is_some_and(|r| r.record.mean_kernel_ns() >= threshold)
    }

    /// Number of dispatch samples recorded for the region. Returns
    /// `0` for unrecorded regions.
    #[must_use]
    pub fn dispatch_count(&self, region_generator: &str) -> u64 {
        self.records
            .get(region_generator)
            .map_or(0, |r| r.record.dispatch_count)
    }

    /// Mean kernel-execute time in nanoseconds. Returns `0` for
    /// unrecorded regions.
    #[must_use]
    pub fn mean_kernel_ns(&self, region_generator: &str) -> u64 {
        self.records
            .get(region_generator)
            .map_or(0, |r| r.record.mean_kernel_ns())
    }

    /// Snapshot of the full record for `region_generator`, or
    /// `None` if no samples have been recorded.
    #[must_use]
    pub fn record_for(&self, region_generator: &str) -> Option<RegionRecord> {
        self.records.get(region_generator).map(|r| r.record)
    }

    /// Total number of distinct regions currently recorded.
    #[must_use]
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// `true` iff zero records.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for HotPathHints {
    fn default() -> Self {
        Self::with_capacity(256)
    }
}

impl std::fmt::Debug for HotPathHints {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HotPathHints")
            .field("capacity", &self.capacity)
            .field(
                "hot_ns_threshold",
                &self.hot_ns_threshold.load(Ordering::Relaxed),
            )
            .field("records_len", &self.records.len())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Empty hints reject every is_hot query and return zero counts.
    #[test]
    fn empty_hints_returns_defaults() {
        let hints = HotPathHints::default();
        assert!(hints.is_empty());
        assert!(!hints.is_hot("any-region"));
        assert_eq!(hints.dispatch_count("any-region"), 0);
        assert_eq!(hints.mean_kernel_ns("any-region"), 0);
        assert!(hints.record_for("any-region").is_none());
    }

    /// Recording N samples accumulates into a single record with
    /// running mean + max.
    #[test]
    fn record_accumulates_into_running_mean_and_max() {
        let hints = HotPathHints::default();
        hints.record("matmul", 1_000, 100);
        hints.record("matmul", 3_000, 300);
        hints.record("matmul", 2_000, 200);
        let rec = hints.record_for("matmul").expect("Fix: recorded");
        assert_eq!(rec.dispatch_count, 3);
        assert_eq!(rec.kernel_ns_total, 6_000);
        assert_eq!(rec.kernel_ns_max, 3_000);
        assert_eq!(rec.bytes_total, 600);
        assert_eq!(rec.mean_kernel_ns(), 2_000);
        assert_eq!(rec.mean_bytes(), 200);
    }

    /// `is_hot` flips true once the recorded mean crosses the
    /// threshold (default 100µs).
    #[test]
    fn is_hot_uses_recorded_mean() {
        let hints = HotPathHints::default().with_hot_threshold_ns(1_000);
        hints.record("region", 500, 0);
        assert!(!hints.is_hot("region"), "below threshold");
        hints.record("region", 2_000, 0);
        // mean = (500 + 2000) / 2 = 1250; >= 1000 → hot.
        assert!(hints.is_hot("region"));
    }

    /// LRU eviction kicks the oldest entry when capacity is hit.
    #[test]
    fn lru_evicts_oldest_when_capacity_reached() {
        let hints = HotPathHints::with_capacity(2);
        hints.record("a", 100, 10);
        hints.record("b", 200, 20);
        hints.record("c", 300, 30);
        assert_eq!(hints.len(), 2);
        assert!(hints.record_for("a").is_none(), "oldest evicted");
        assert!(hints.record_for("b").is_some());
        assert!(hints.record_for("c").is_some());
    }

    /// Recording the same key bumps recency so subsequent eviction
    /// targets a different entry.
    #[test]
    fn lru_recency_promotes_on_repeat_record() {
        let hints = HotPathHints::with_capacity(2);
        hints.record("a", 100, 10);
        hints.record("b", 200, 20);
        hints.record("a", 100, 10); // bumps a to most recent
        hints.record("c", 300, 30);
        assert!(
            hints.record_for("a").is_some(),
            "a was bumped, must survive"
        );
        assert!(hints.record_for("b").is_none(), "b was oldest, evicted");
        assert!(hints.record_for("c").is_some());
    }

    /// Capacity 0 disables recording entirely.
    #[test]
    fn capacity_zero_disables_recording() {
        let hints = HotPathHints::with_capacity(0);
        hints.record("a", 100, 10);
        assert!(hints.is_empty());
        assert!(!hints.is_hot("a"));
    }

    /// Hints are Send + Sync so a backend on one thread can record
    /// while the optimizer on another thread queries.
    #[test]
    fn hints_are_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<HotPathHints>();
        assert_sync::<HotPathHints>();
    }
}
