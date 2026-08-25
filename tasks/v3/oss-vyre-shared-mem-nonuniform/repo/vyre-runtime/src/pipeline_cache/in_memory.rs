//! [`InMemoryPipelineCache`]  -  sharded zero-persistence cache. The hot
//! path for in-process pipeline reuse.

use std::sync::{Arc, Mutex, MutexGuard};

use rustc_hash::FxHashMap;

use super::fingerprint::PipelineFingerprint;
use super::metrics::{PipelineCacheCounters, PipelineCacheMetrics};
use super::store::PipelineCacheStore;

/// In-memory pipeline cache  -  zero-persistence, zero-network, sharded
/// `FxHashMap`s behind mutexes so concurrent `get`/`put` on different
/// fingerprints rarely contend (VYRE_RUNTIME / PERF hot-cache audit).
#[derive(Debug)]
pub struct InMemoryPipelineCache {
    shards: [Mutex<InMemoryCacheShard>; Self::SHARD_COUNT],
    max_entries_per_shard: usize,
    max_bytes_per_shard: usize,
    metrics: PipelineCacheCounters,
}

impl InMemoryPipelineCache {
    pub(super) const SHARD_COUNT: usize = 256;
    pub(super) const MAX_ENTRIES_PER_SHARD: usize = 256;
    pub(super) const MAX_BYTES_PER_SHARD: usize = 16 * 1024 * 1024;

    #[inline]
    fn shard_index(fp: &PipelineFingerprint) -> usize {
        usize::from(fp.0[0]) % Self::SHARD_COUNT
    }

    fn lock_shard(shard: &Mutex<InMemoryCacheShard>) -> MutexGuard<'_, InMemoryCacheShard> {
        // Fail closed on poison. `PoisonError::into_inner` would silently hand
        // back a guard over shard state left half-mutated by a panicking writer
        // — every subsequent cache read/write would then trust corrupt data with
        // no signal. A poisoned pipeline-cache shard is unrecoverable; surface it
        // loudly instead of laundering it.
        shard
            .lock()
            .unwrap_or_else(|_| panic!("pipeline cache shard lock was poisoned"))
    }

    /// Construct an empty cache.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Construct an empty cache with explicit per-shard entry and byte budgets.
    ///
    /// A zero entry budget or zero byte budget creates a disabled cache that
    /// accepts `put` calls but retains no artifacts.
    #[must_use]
    pub fn with_limits(max_entries_per_shard: usize, max_bytes_per_shard: usize) -> Self {
        Self {
            shards: std::array::from_fn(|_| Mutex::new(InMemoryCacheShard::default())),
            max_entries_per_shard,
            max_bytes_per_shard,
            metrics: PipelineCacheCounters::default(),
        }
    }

    /// Current entry count. Thread-safe snapshot.
    pub fn len(&self) -> usize {
        self.shards
            .iter()
            .map(|s| Self::lock_shard(s).entries.len())
            .fold(0usize, |acc, value| {
                cache_usize_add(
                    acc,
                    value,
                    "entry count",
                    "shard cache metrics before snapshotting",
                )
            })
    }

    /// Current cached artifact bytes. Thread-safe snapshot.
    pub fn cached_bytes(&self) -> usize {
        self.shards
            .iter()
            .map(|s| Self::lock_shard(s).bytes)
            .fold(0usize, |acc, value| {
                cache_usize_add(
                    acc,
                    value,
                    "byte count",
                    "shard cache metrics before snapshotting",
                )
            })
    }

    /// Whether the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.shards
            .iter()
            .all(|s| Self::lock_shard(s).entries.is_empty())
    }

    /// Snapshot the most recent eviction report from every shard that has
    /// evicted at least one artifact.
    #[must_use]
    pub fn eviction_reports(&self) -> Vec<InMemoryEvictionReport> {
        let mut reports = Vec::new();
        for shard in &self.shards {
            if let Some(report) = Self::lock_shard(shard).last_eviction {
                reports.push(report);
            }
        }
        reports
    }
}

impl Default for InMemoryPipelineCache {
    fn default() -> Self {
        Self::with_limits(Self::MAX_ENTRIES_PER_SHARD, Self::MAX_BYTES_PER_SHARD)
    }
}

fn cache_usize_add(lhs: usize, rhs: usize, _label: &'static str, _fix: &'static str) -> usize {
    lhs.saturating_add(rhs)
}

fn cache_usize_sub(lhs: usize, rhs: usize, _label: &'static str, _fix: &'static str) -> usize {
    lhs.saturating_sub(rhs)
}

fn cache_u64_add(lhs: u64, rhs: u64, _label: &'static str, _fix: &'static str) -> u64 {
    lhs.saturating_add(rhs)
}

fn cache_u64_sub(lhs: u64, rhs: u64, _label: &'static str, _fix: &'static str) -> u64 {
    lhs.saturating_sub(rhs)
}

fn cache_usize_to_u64(value: usize, _label: &'static str, _fix: &'static str) -> u64 {
    match u64::try_from(value) {
        Ok(value) => value,
        Err(_) => u64::MAX,
    }
}

#[derive(Debug, Default)]
struct InMemoryCacheShard {
    entries: FxHashMap<PipelineFingerprint, InMemoryCacheEntry>,
    bytes: usize,
    clock: u64,
    last_eviction: Option<InMemoryEvictionReport>,
}

impl InMemoryCacheShard {
    fn next_tick(&mut self) -> u64 {
        self.clock = cache_u64_add(
            self.clock,
            1,
            "shard clock",
            "recreate the cache before LRU timestamps wrap",
        );
        self.clock
    }

    fn evict_to_limits(
        &mut self,
        max_entries: usize,
        max_bytes: usize,
    ) -> Option<InMemoryEvictionReport> {
        let mut report = None;
        while self.entries.len() > max_entries || self.bytes > max_bytes {
            let reason = InMemoryEvictionReason::from_limits(
                self.entries.len(),
                self.bytes,
                max_entries,
                max_bytes,
            );
            let Some(victim) = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.last_used)
                .map(|(fp, _)| *fp)
            else {
                self.bytes = 0;
                self.last_eviction = report;
                return report;
            };
            if let Some(removed) = self.entries.remove(&victim) {
                self.bytes = cache_usize_sub(
                    self.bytes,
                    removed.bytes,
                    "byte accounting during eviction",
                    "rebuild the cache",
                );
                record_eviction_report(&mut report, reason, self.clock, &removed);
            }
        }
        if report.is_some() {
            self.last_eviction = report;
        }
        report
    }
}

#[derive(Debug)]
struct InMemoryCacheEntry {
    artifact: Arc<Vec<u8>>,
    bytes: usize,
    last_used: u64,
}

/// Reason the in-memory pipeline cache evicted retained artifacts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InMemoryEvictionReason {
    /// Entry-count budget was exceeded.
    EntryLimit,
    /// Byte budget was exceeded.
    ByteLimit,
    /// Entry-count and byte budgets were both exceeded.
    EntryAndByteLimit,
    /// A rejected put removed an existing artifact for the same key.
    RejectedPut,
}

impl InMemoryEvictionReason {
    fn from_limits(entries: usize, bytes: usize, max_entries: usize, max_bytes: usize) -> Self {
        match (entries > max_entries, bytes > max_bytes) {
            (true, true) => Self::EntryAndByteLimit,
            (true, false) => Self::EntryLimit,
            (false, true) => Self::ByteLimit,
            (false, false) => Self::EntryLimit,
        }
    }
}

/// Structured eviction report for one in-memory cache shard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InMemoryEvictionReport {
    /// Number of entries removed by the eviction decision.
    pub entries: u64,
    /// Bytes removed by the eviction decision.
    pub bytes: u64,
    /// Maximum age of an evicted entry in shard clock ticks.
    pub max_age_ticks: u64,
    /// Why eviction happened.
    pub reason: InMemoryEvictionReason,
}

fn record_eviction_report(
    report: &mut Option<InMemoryEvictionReport>,
    reason: InMemoryEvictionReason,
    now: u64,
    removed: &InMemoryCacheEntry,
) {
    let removed_bytes = cache_usize_to_u64(
        removed.bytes,
        "evicted byte count",
        "shard cache artifacts before eviction",
    );
    let age = cache_u64_sub(
        now,
        removed.last_used,
        "evicted entry age",
        "rebuild the cache LRU clock",
    );
    match report {
        Some(report) => {
            report.entries = cache_u64_add(
                report.entries,
                1,
                "eviction count",
                "shard cache eviction work",
            );
            report.bytes = cache_u64_add(
                report.bytes,
                removed_bytes,
                "evicted byte count",
                "shard cache eviction work",
            );
            report.max_age_ticks = report.max_age_ticks.max(age);
        }
        None => {
            *report = Some(InMemoryEvictionReport {
                entries: 1,
                bytes: removed_bytes,
                max_age_ticks: age,
                reason,
            });
        }
    }
}

impl PipelineCacheStore for InMemoryPipelineCache {
    fn get(&self, fp: &PipelineFingerprint) -> Option<Vec<u8>> {
        self.get_arc(fp).map(|artifact| (*artifact).clone())
    }

    /// V7-PERF-009: zero-clone hot-path lookup. The cache already stores
    /// payloads behind `Arc<Vec<u8>>`, so a hit is one refcount bump.
    fn get_arc(&self, fp: &PipelineFingerprint) -> Option<Arc<Vec<u8>>> {
        PipelineCacheCounters::increment(&self.metrics.lookups, "lookups");
        let i = Self::shard_index(fp);
        let mut shard = Self::lock_shard(&self.shards[i]);
        let tick = shard.next_tick();
        let Some(entry) = shard.entries.get_mut(fp) else {
            PipelineCacheCounters::increment(&self.metrics.misses, "misses");
            return None;
        };
        entry.last_used = tick;
        PipelineCacheCounters::increment(&self.metrics.hits, "hits");
        Some(Arc::clone(&entry.artifact))
    }

    fn put(&self, fp: PipelineFingerprint, artifact: Vec<u8>) {
        let i = Self::shard_index(&fp);
        let mut shard = Self::lock_shard(&self.shards[i]);
        let bytes = artifact.len();
        if self.max_entries_per_shard == 0
            || self.max_bytes_per_shard == 0
            || bytes > self.max_bytes_per_shard
        {
            PipelineCacheCounters::increment(&self.metrics.rejected_puts, "rejected puts");
            if let Some(removed) = shard.entries.remove(&fp) {
                let tick = shard.next_tick();
                shard.bytes = cache_usize_sub(
                    shard.bytes,
                    removed.bytes,
                    "byte accounting while rejecting put",
                    "rebuild the cache",
                );
                let mut report = None;
                record_eviction_report(
                    &mut report,
                    InMemoryEvictionReason::RejectedPut,
                    tick,
                    &removed,
                );
                shard.last_eviction = report;
                PipelineCacheCounters::increment(&self.metrics.evictions, "evictions");
                PipelineCacheCounters::add(
                    &self.metrics.evicted_bytes,
                    cache_usize_to_u64(
                        removed.bytes,
                        "evicted byte count",
                        "shard cache artifacts before eviction",
                    ),
                    "evicted bytes",
                );
            }
            return;
        }

        if let Some(existing) = shard.entries.remove(&fp) {
            shard.bytes = cache_usize_sub(
                shard.bytes,
                existing.bytes,
                "byte accounting while replacing entry",
                "rebuild the cache",
            );
        }
        let tick = shard.next_tick();
        shard.bytes = cache_usize_add(
            shard.bytes,
            bytes,
            "byte accounting while inserting entry",
            "lower per-shard cache byte budget",
        );
        shard.entries.insert(
            fp,
            InMemoryCacheEntry {
                artifact: Arc::new(artifact),
                bytes,
                last_used: tick,
            },
        );
        PipelineCacheCounters::increment(&self.metrics.puts, "puts");
        if let Some(report) =
            shard.evict_to_limits(self.max_entries_per_shard, self.max_bytes_per_shard)
        {
            PipelineCacheCounters::add(&self.metrics.evictions, report.entries, "evictions");
            PipelineCacheCounters::add(&self.metrics.evicted_bytes, report.bytes, "evicted bytes");
        }
    }

    fn metrics(&self) -> PipelineCacheMetrics {
        self.metrics
            .snapshot(
                cache_usize_to_u64(
                    self.cached_bytes(),
                    "retained byte snapshot",
                    "shard cache metrics before snapshotting",
                ),
                cache_usize_to_u64(
                    self.len(),
                    "entry count snapshot",
                    "shard cache metrics before snapshotting",
                ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline_cache::test_helpers::tiny_program;

    #[test]
    fn in_memory_cache_roundtrip() {
        let cache = InMemoryPipelineCache::new();
        let fp = PipelineFingerprint::of(&tiny_program());
        assert!(cache.get(&fp).is_none());
        cache.put(fp, b"target-bytes".to_vec());
        assert_eq!(cache.get(&fp).unwrap(), b"target-bytes".to_vec());
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn in_memory_cache_caps_each_shard() {
        let cache = InMemoryPipelineCache::new();
        for i in 0..(InMemoryPipelineCache::MAX_ENTRIES_PER_SHARD + 17) {
            let mut bytes = [0_u8; 32];
            bytes[1..9].copy_from_slice(&(i as u64).to_le_bytes());
            cache.put(PipelineFingerprint(bytes), vec![i as u8]);
        }
        assert_eq!(cache.len(), InMemoryPipelineCache::MAX_ENTRIES_PER_SHARD);
    }

    #[test]
    fn in_memory_cache_evicts_least_recently_used_entry() {
        let cache = InMemoryPipelineCache::with_limits(2, 1024);
        let a = PipelineFingerprint([0; 32]);
        let mut b_bytes = [0; 32];
        b_bytes[1] = 1;
        let b = PipelineFingerprint(b_bytes);
        let mut c_bytes = [0; 32];
        c_bytes[1] = 2;
        let c = PipelineFingerprint(c_bytes);

        cache.put(a, b"a".to_vec());
        cache.put(b, b"b".to_vec());
        assert_eq!(cache.get(&a).unwrap(), b"a".to_vec());
        cache.put(c, b"c".to_vec());

        assert_eq!(cache.get(&a).unwrap(), b"a".to_vec());
        assert!(cache.get(&b).is_none());
        assert_eq!(cache.get(&c).unwrap(), b"c".to_vec());
    }

    #[test]
    fn in_memory_cache_enforces_byte_budget() {
        let cache = InMemoryPipelineCache::with_limits(8, 10);
        let a = PipelineFingerprint([0; 32]);
        let mut b_bytes = [0; 32];
        b_bytes[1] = 1;
        let b = PipelineFingerprint(b_bytes);
        let mut too_large_bytes = [0; 32];
        too_large_bytes[1] = 2;
        let too_large = PipelineFingerprint(too_large_bytes);

        cache.put(a, vec![1; 6]);
        cache.put(b, vec![2; 6]);
        assert!(cache.get(&a).is_none());
        assert_eq!(cache.get(&b).unwrap(), vec![2; 6]);
        assert_eq!(cache.cached_bytes(), 6);

        cache.put(too_large, vec![3; 11]);
        assert!(cache.get(&too_large).is_none());
        assert_eq!(cache.cached_bytes(), 6);
    }

    #[test]
    fn in_memory_cache_metrics_track_hits_misses_and_evictions() {
        let cache = InMemoryPipelineCache::with_limits(1, 8);
        let a = PipelineFingerprint([0; 32]);
        let mut b_bytes = [0; 32];
        b_bytes[1] = 1;
        let b = PipelineFingerprint(b_bytes);

        assert!(cache.get(&a).is_none());
        cache.put(a, vec![1; 4]);
        assert!(cache.get(&a).is_some());
        cache.put(b, vec![2; 4]);

        let metrics = cache.metrics();
        assert_eq!(metrics.lookups, 2);
        assert_eq!(metrics.hits, 1);
        assert_eq!(metrics.misses, 1);
        assert_eq!(metrics.puts, 2);
        assert_eq!(metrics.evictions, 1);
        assert_eq!(metrics.cached_bytes, 4);
        assert_eq!(metrics.entries, 1);
        assert_eq!(metrics.hit_rate_ppm(), 500_000);
    }

    #[test]
    fn in_memory_cache_eviction_report_records_reason_entries_bytes_and_age() {
        let cache = InMemoryPipelineCache::with_limits(1, 8);
        let a = PipelineFingerprint([0; 32]);
        let mut b_bytes = [0; 32];
        b_bytes[1] = 1;
        let b = PipelineFingerprint(b_bytes);

        cache.put(a, vec![1; 4]);
        assert!(cache.get(&a).is_some());
        cache.put(b, vec![2; 4]);

        let reports = cache.eviction_reports();
        assert_eq!(reports.len(), 1);
        let report = reports[0];
        assert_eq!(report.reason, InMemoryEvictionReason::EntryLimit);
        assert_eq!(report.entries, 1);
        assert_eq!(report.bytes, 4);
        assert!(
            report.max_age_ticks > 0,
            "Fix: eviction reports must expose LRU age, got {report:?}"
        );
    }

    #[test]
    fn rejected_oversize_put_records_eviction_reason_for_replaced_entry() {
        let cache = InMemoryPipelineCache::with_limits(8, 8);
        let fp = PipelineFingerprint([0; 32]);

        cache.put(fp, vec![1; 4]);
        cache.put(fp, vec![2; 9]);

        assert!(cache.get(&fp).is_none());
        let reports = cache.eviction_reports();
        assert_eq!(reports.len(), 1);
        let report = reports[0];
        assert_eq!(report.reason, InMemoryEvictionReason::RejectedPut);
        assert_eq!(report.entries, 1);
        assert_eq!(report.bytes, 4);
    }

    #[test]
    fn poisoned_cache_shard_is_not_silently_recovered() {
        let cache = Arc::new(InMemoryPipelineCache::new());
        let poisoned = Arc::clone(&cache);
        let _ = std::thread::spawn(move || {
            let _guard = InMemoryPipelineCache::lock_shard(&poisoned.shards[0]);
            panic!("poison in-memory pipeline cache shard");
        })
        .join();

        let panic = std::panic::catch_unwind(|| {
            let _ = cache.len();
        })
        .expect_err("poisoned pipeline cache shard must panic instead of recovering");
        let message = panic
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| panic.downcast_ref::<&'static str>().copied())
            .unwrap_or("<non-string panic>");
        assert!(
            message.contains("pipeline cache shard lock was poisoned"),
            "{message}"
        );
    }
}
