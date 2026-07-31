//! Bounded cache for WGPU pipeline artifacts.

use crate::pipeline::CachedPipelineArtifact;
use crate::staging_reserve::reserve_backend_vec;
use dashmap::DashMap;
use rustc_hash::FxHasher;
use std::hash::BuildHasherDefault;
use std::sync::atomic::{AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use vyre_driver::accounting::{
    checked_atomic_add_u64_with_order, checked_atomic_add_usize_with_order,
    checked_atomic_sub_usize_with_order, pinning_atomic_increment_u32,
    pinning_atomic_increment_u64,
};
use vyre_driver::cache_eviction_heat::CacheEntryStats;
use vyre_driver::BackendError;

/// Bounded cache for WGPU pipeline artifacts using shared driver-tier
/// retention policy. Despite the legacy name, this is not LRU.
pub(crate) struct LruPipelineCache {
    artifacts: DashMap<[u8; 32], PipelineCacheEntry, BuildHasherDefault<FxHasher>>,
    cached_bytes: AtomicUsize,
    hits: AtomicU64,
    misses: AtomicU64,
    insertions: AtomicU64,
    evictions: AtomicU64,
    max_entries: u32,
    max_bytes: usize,
}

struct PipelineCacheEntry {
    artifact: Arc<CachedPipelineArtifact>,
    gain: AtomicU32,
    last_hit_time_s: AtomicU64,
    cost: usize,
}

impl std::fmt::Debug for LruPipelineCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LruPipelineCache")
            .field("entries", &self.len())
            .finish_non_exhaustive()
    }
}

impl LruPipelineCache {
    /// Create a cache capped at `max_entries`.
    #[cfg(test)]
    pub(crate) fn new(max_entries: u32) -> Self {
        Self::with_limits(max_entries, 256 * 1024 * 1024)
    }

    /// Create a cache capped by entry count and estimated artifact bytes.
    pub(crate) fn with_limits(max_entries: u32, max_bytes: usize) -> Self {
        Self {
            artifacts: DashMap::with_hasher(BuildHasherDefault::<FxHasher>::default()),
            cached_bytes: AtomicUsize::new(0),
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
            insertions: AtomicU64::new(0),
            evictions: AtomicU64::new(0),
            max_entries: max_entries.max(1),
            max_bytes: max_bytes.max(1),
        }
    }

    /// Retrieve an artifact and update its recency/gain.
    pub(crate) fn get(&self, fingerprint: &[u8; 32]) -> Option<Arc<CachedPipelineArtifact>> {
        if let Some(entry) = self.artifacts.get(fingerprint) {
            let artifact = Arc::clone(&entry.artifact);
            pinning_atomic_increment_u32(&entry.gain, Ordering::Relaxed, Ordering::Relaxed, || {
                tracing::error!(
                        "pipeline cache gain reached u32::MAX and was pinned. Fix: shard pipeline-cache telemetry collection before wrap."
                    );
            });
            entry
                .last_hit_time_s
                .store(f64_to_atomic(now_seconds()), Ordering::Relaxed);
            pinning_atomic_increment_u64(&self.hits, Ordering::Relaxed, Ordering::Relaxed, || {
                tracing::error!(
                        "pipeline cache hits reached u64::MAX and was pinned. Fix: shard pipeline-cache telemetry collection before wrap."
                    );
            });
            Some(artifact)
        } else {
            pinning_atomic_increment_u64(
                &self.misses,
                Ordering::Relaxed,
                Ordering::Relaxed,
                || {
                    tracing::error!(
                        "pipeline cache misses reached u64::MAX and was pinned. Fix: shard pipeline-cache telemetry collection before wrap."
                    );
                },
            );
            None
        }
    }

    /// Insert an artifact, evicting cold entries until capacity is available.
    pub(crate) fn insert(&self, fingerprint: [u8; 32], artifact: Arc<CachedPipelineArtifact>) {
        let cost = artifact.cache_cost_bytes();
        if cost > self.max_bytes {
            self.remove_key(&fingerprint);
            return;
        }

        let previous = self.artifacts.insert(
            fingerprint,
            PipelineCacheEntry {
                artifact,
                gain: AtomicU32::new(1),
                last_hit_time_s: AtomicU64::new(f64_to_atomic(now_seconds())),
                cost,
            },
        );
        match previous {
            Some(old) => {
                if cost >= old.cost {
                    if !try_atomic_add_usize(&self.cached_bytes, cost - old.cost) {
                        self.clear();
                        return;
                    }
                } else {
                    if !try_atomic_sub_usize(&self.cached_bytes, old.cost - cost) {
                        self.rebuild_cached_bytes();
                    }
                }
            }
            None => {
                if !try_atomic_add_usize(&self.cached_bytes, cost) {
                    self.clear();
                    return;
                }
            }
        }
        rebasing_atomic_add_u64(&self.insertions, 1, "pipeline cache insertions");

        self.evict_over_capacity();
    }

    fn evict_over_capacity(&self) {
        while self.artifacts.len() > self.max_entries()
            || self.cached_bytes.load(Ordering::Relaxed) > self.max_bytes
        {
            let entries = match self.eviction_snapshot() {
                Ok(entries) => entries,
                Err(error) => {
                    tracing::error!(
                        "WGPU pipeline cache eviction snapshot allocation failed: {error}. Fix: lower pipeline-cache capacity or shard pipeline compilation."
                    );
                    self.clear();
                    return;
                }
            };
            if entries.is_empty() {
                self.artifacts.clear();
                self.cached_bytes.store(0, Ordering::Relaxed);
                return;
            }

            let mut removed_count = 0u64;
            let evict = match self.eviction_keys(&entries) {
                Ok(evict) => evict,
                Err(error) => {
                    tracing::error!(
                        "WGPU pipeline cache eviction ranking allocation failed: {error}. Fix: lower pipeline-cache capacity or shard pipeline compilation."
                    );
                    self.clear();
                    return;
                }
            };
            for key in evict {
                if let Some((_, removed)) = self.artifacts.remove(&key) {
                    if !try_atomic_sub_usize(&self.cached_bytes, removed.cost) {
                        self.rebuild_cached_bytes();
                    }
                    if removed_count == u64::MAX {
                        rebasing_atomic_add_u64(
                            &self.evictions,
                            removed_count,
                            "pipeline cache evictions",
                        );
                        removed_count = 0;
                    }
                    removed_count += 1;
                }
            }
            if removed_count == 0 {
                return;
            }
            rebasing_atomic_add_u64(&self.evictions, removed_count, "pipeline cache evictions");
            vyre_driver::cache_eviction::record_eviction(
                removed_count as f64 / entries.len() as f64,
            );
        }
    }

    fn eviction_snapshot(&self) -> Result<Vec<EvictionEntry>, BackendError> {
        let mut entries = Vec::new();
        reserve_backend_vec(
            &mut entries,
            self.artifacts.len(),
            "pipeline cache eviction snapshot",
        )?;
        for entry in self.artifacts.iter() {
            entries.push(EvictionEntry {
                key: *entry.key(),
                gain: entry.gain.load(Ordering::Relaxed),
                last_hit_time_s: atomic_to_f64(entry.last_hit_time_s.load(Ordering::Relaxed)),
                cost: entry.cost,
            });
        }
        Ok(entries)
    }

    fn eviction_keys(&self, entries: &[EvictionEntry]) -> Result<Vec<[u8; 32]>, BackendError> {
        let mut retained_len = entries.len();
        let mut retained_bytes = entries
            .iter()
            .try_fold(0usize, |total, entry| total.checked_add(entry.cost))
            .unwrap_or(usize::MAX);
        let now = now_seconds();
        let mut ranked = Vec::new();
        reserve_backend_vec(
            &mut ranked,
            entries.len(),
            "pipeline cache eviction heat ranking",
        )?;
        ranked.extend(entries.iter().enumerate().map(|(idx, entry)| {
            let id = u64::try_from(idx).unwrap_or(u64::MAX);
            let stats = CacheEntryStats {
                id,
                hit_count: entry.gain,
                last_hit_time_s: entry.last_hit_time_s,
            };
            (idx, stats.heat(now))
        }));
        ranked.sort_by(|left, right| {
            left.1
                .total_cmp(&right.1)
                .then_with(|| left.0.cmp(&right.0))
        });
        let mut keys = Vec::new();
        reserve_backend_vec(&mut keys, entries.len(), "pipeline cache eviction key list")?;
        for (cold_idx, _) in ranked {
            if retained_len <= self.max_entries() && retained_bytes <= self.max_bytes {
                break;
            }
            let entry = &entries[cold_idx];
            keys.push(entry.key);
            retained_len = if retained_len == 0 {
                0
            } else {
                retained_len - 1
            };
            retained_bytes = if entry.cost > retained_bytes {
                0
            } else {
                retained_bytes - entry.cost
            };
        }
        Ok(keys)
    }

    fn remove_key(&self, fingerprint: &[u8; 32]) {
        if let Some((_, removed)) = self.artifacts.remove(fingerprint) {
            if !try_atomic_sub_usize(&self.cached_bytes, removed.cost) {
                self.rebuild_cached_bytes();
            }
        }
    }

    fn rebuild_cached_bytes(&self) {
        let mut total = 0usize;
        for entry in self.artifacts.iter() {
            let Some(next) = total.checked_add(entry.cost) else {
                self.clear();
                return;
            };
            total = next;
        }
        self.cached_bytes.store(total, Ordering::Relaxed);
    }

    /// Remove every cached artifact.
    pub(crate) fn clear(&self) {
        self.artifacts.clear();
        self.cached_bytes.store(0, Ordering::Relaxed);
    }

    /// Invalidate entries impacted by a change in the rule dependency graph.
    ///
    /// This implements the #36 recursion thesis: vyre using its own
    /// `do_calculus` primitive to perform formal causal change-impact
    /// analysis on its own rule graph.
    pub(crate) fn invalidate_impacted(&self, impact_mask: &[u32], keys: &[[u8; 32]]) {
        for (i, &is_impacted) in impact_mask.iter().enumerate() {
            if is_impacted != 0 {
                if let Some(key) = keys.get(i) {
                    self.remove_key(key);
                }
            }
        }
    }

    /// Number of cached artifact keys.
    pub(crate) fn len(&self) -> usize {
        self.artifacts.len()
    }

    /// Estimated bytes retained by cached artifacts.
    pub(crate) fn cached_bytes(&self) -> usize {
        self.cached_bytes.load(Ordering::Relaxed)
    }

    /// Entry budget.
    pub(crate) fn max_entries(&self) -> usize {
        usize::try_from(self.max_entries).unwrap_or(usize::MAX)
    }

    /// Estimated byte budget.
    pub(crate) fn max_bytes(&self) -> usize {
        self.max_bytes
    }

    /// Cache lookup hits.
    pub(crate) fn hits(&self) -> u64 {
        self.hits.load(Ordering::Relaxed)
    }

    /// Cache lookup misses.
    pub(crate) fn misses(&self) -> u64 {
        self.misses.load(Ordering::Relaxed)
    }

    /// Cache insertions.
    pub(crate) fn insertions(&self) -> u64 {
        self.insertions.load(Ordering::Relaxed)
    }

    /// Capacity-driven evictions.
    pub(crate) fn evictions(&self) -> u64 {
        self.evictions.load(Ordering::Relaxed)
    }
}

struct EvictionEntry {
    key: [u8; 32],
    gain: u32,
    last_hit_time_s: f64,
    cost: usize,
}

fn now_seconds() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |duration| duration.as_secs_f64())
}

fn f64_to_atomic(value: f64) -> u64 {
    value.to_bits()
}

fn atomic_to_f64(bits: u64) -> f64 {
    f64::from_bits(bits)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(byte: u8) -> [u8; 32] {
        [byte; 32]
    }

    fn entry(key_byte: u8, gain: u32, last_hit_time_s: f64, cost: usize) -> EvictionEntry {
        EvictionEntry {
            key: key(key_byte),
            gain,
            last_hit_time_s,
            cost,
        }
    }

    #[test]
    fn pipeline_cache_eviction_uses_heat_not_insert_order() {
        let cache = LruPipelineCache::with_limits(2, 1024);
        let entries = [
            entry(1, 1, 100.0, 1),
            entry(2, 100, 100.0, 1),
            entry(3, 50, 100.0, 1),
        ];
        assert_eq!(cache.eviction_keys(&entries).unwrap(), vec![key(1)]);
    }

    #[test]
    fn pipeline_cache_eviction_continues_until_byte_budget_fits() {
        let cache = LruPipelineCache::with_limits(8, 10);
        let entries = [
            entry(1, 1, 100.0, 8),
            entry(2, 2, 100.0, 8),
            entry(3, 100, 100.0, 2),
        ];
        assert_eq!(cache.eviction_keys(&entries).unwrap(), vec![key(1)]);
    }

    #[test]
    fn production_pipeline_cache_uses_fallible_eviction_staging() {
        let production = include_str!("pipeline.rs")
            .split("\n#[cfg(test)]\nmod tests")
            .next()
            .expect("Fix: pipeline cache production section should precede tests");

        assert!(
            !production.contains("Vec::with_capacity"),
            "Fix: pipeline-cache eviction must not allocate ranking/snapshot vectors infallibly."
        );
        assert!(
            production.contains("reserve_backend_vec"),
            "Fix: pipeline-cache eviction should reserve through the shared WGPU staging helper."
        );
        assert!(
            production.contains("eviction_snapshot(&self) -> Result"),
            "Fix: pipeline-cache eviction snapshot allocation failures must be represented explicitly."
        );
    }
}

fn try_atomic_add_usize(counter: &AtomicUsize, value: usize) -> bool {
    checked_atomic_add_usize_with_order(
        counter,
        value,
        Ordering::Relaxed,
        Ordering::Relaxed,
        Ordering::Relaxed,
        |_, _| (),
    )
    .is_ok()
}

fn try_atomic_sub_usize(counter: &AtomicUsize, value: usize) -> bool {
    checked_atomic_sub_usize_with_order(
        counter,
        value,
        Ordering::Relaxed,
        Ordering::Relaxed,
        Ordering::Relaxed,
        |_, _| (),
    )
    .is_ok()
}

fn rebasing_atomic_add_u64(counter: &AtomicU64, value: u64, label: &'static str) {
    if value == 0 {
        return;
    }
    if value == 1 {
        pinning_atomic_increment_u64(counter, Ordering::Relaxed, Ordering::Relaxed, || {
            tracing::error!(
                "{label} reached u64::MAX and was pinned. Fix: shard pipeline-cache telemetry collection before wrap."
            );
        });
        return;
    }
    if checked_atomic_add_u64_with_order(
        counter,
        value,
        Ordering::Relaxed,
        Ordering::Relaxed,
        Ordering::Relaxed,
        |_, _| (),
    )
    .is_err()
    {
        counter.store(u64::MAX, Ordering::Relaxed);
        tracing::error!(
            "{label} exceeded u64::MAX and was pinned at u64::MAX. Fix: shard pipeline-cache telemetry collection before wrap."
        );
    }
}
