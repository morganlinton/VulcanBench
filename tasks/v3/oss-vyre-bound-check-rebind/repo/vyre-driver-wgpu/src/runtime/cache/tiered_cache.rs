use crate::runtime::cache::lru::{AccessTracker, IntrusiveLru};
use rustc_hash::FxHashMap;

/// Metadata for a cached entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct CacheEntry {
    /// Unique identifier for the entry.
    pub key: u64,
    /// Size of the entry in bytes.
    pub size: u64,
    /// Index of the tier the entry currently resides in.
    pub tier: usize,
}

/// A single cache tier with a fixed capacity.
///
/// Carries its own recency LRU so eviction picks the coldest entry
/// within the tier in O(1) instead of scanning the global
/// `AccessTracker` looking for a key that happens to live in this
/// tier. Before 0.6 the scan was O(N) in the global tracker size  -
/// catastrophic when the cold key was far from the tier boundary.
#[non_exhaustive]
pub struct CacheTier {
    /// Human-readable name for the tier.
    pub name: String,
    /// Total capacity of the tier in bytes.
    pub capacity: u64,
    /// Currently used bytes in the tier.
    pub used: u64,
    pub(crate) entries: FxHashMap<u64, CacheEntry>,
    pub(crate) lru: IntrusiveLru<u64, ()>,
}

impl CacheTier {
    /// Create a new empty tier.
    #[inline]
    pub fn new(name: impl Into<String>, capacity: u64) -> Self {
        let name = name.into();
        match Self::try_new(name.clone(), capacity) {
            Ok(tier) => tier,
            Err(error) => {
                tracing::error!(
                    tier = %name,
                    capacity,
                    error = %error,
                    "wgpu cache tier LRU reservation failed; continuing with grow-on-use metadata"
                );
                Self {
                    name,
                    capacity,
                    used: 0,
                    entries: FxHashMap::default(),
                    lru: IntrusiveLru::with_reserved_capacity(0),
                }
            }
        }
    }

    /// Fallible version of [`Self::new`].
    ///
    /// # Errors
    ///
    /// Returns [`vyre_driver::BackendError`] if tier LRU metadata cannot be
    /// reserved.
    #[inline]
    pub fn try_new(
        name: impl Into<String>,
        capacity: u64,
    ) -> Result<Self, vyre_driver::BackendError> {
        Ok(Self {
            name: name.into(),
            capacity,
            used: 0,
            entries: FxHashMap::default(),
            lru: IntrusiveLru::try_with_reserved_capacity(1024)?,
        })
    }
}

/// Access statistics used by [`LruPolicy`] promotion decisions.
#[non_exhaustive]
pub struct AccessStats {
    /// Number of recorded accesses.
    pub frequency: u32,
    /// Monotonic tick of the last access. Higher = more recent.
    /// Compare two entries' ticks to determine relative recency in O(1).
    pub last_access: u64,
    /// Size of the entry in bytes.
    pub size: u64,
}

/// LRU eviction policy with frequency-based promotion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct LruPolicy {
    /// Minimum access frequency required for promotion.
    pub promote_threshold: u32,
}

impl LruPolicy {
    /// Default access threshold for promotion.
    pub const DEFAULT_THRESHOLD: u32 = 3;

    /// Create a new policy with the given promotion threshold.
    #[inline]
    pub fn new(promote_threshold: u32) -> Self {
        Self { promote_threshold }
    }
}

impl Default for LruPolicy {
    fn default() -> Self {
        Self::new(Self::DEFAULT_THRESHOLD)
    }
}

impl LruPolicy {
    fn should_promote(&self, _key: u64, stats: &AccessStats) -> bool {
        stats.frequency >= self.promote_threshold
    }

    fn eviction_candidate_per_tier(
        &self,
        _tier: usize,
        entries: &FxHashMap<u64, CacheEntry>,
        _tracker: &AccessTracker,
        tier_lru: &IntrusiveLru<u64, ()>,
    ) -> Option<u64> {
        // O(1) fast path. Walk the tier's own LRU from coldest
        // (tail) until we find a key that still lives in `entries`.
        // Entries and the LRU are mutated in lockstep by
        // TieredCache, so the first iterator step almost always
        // yields the right answer; the loop only runs when a
        // previous eviction race left a stale LRU entry.
        for (key, _) in tier_lru.iter_coldest() {
            if entries.contains_key(key) {
                return Some(*key);
            }
        }
        entries.keys().copied().next()
    }
}

/// Errors that can occur during cache operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum CacheError {
    /// The requested key does not exist in the cache.
    KeyNotFound,
    /// The entry is too large to fit in any tier.
    EntryTooLarge,
    /// Tier byte accounting overflowed or underflowed.
    CapacityAccountingOverflow,
}

impl std::fmt::Display for CacheError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::KeyNotFound => write!(
                f,
                "Key not found in cache. Fix: verify the key was inserted before operating on it."
            ),
            Self::EntryTooLarge => write!(
                f,
                "Entry size exceeds the capacity of the largest tier. Fix: reduce the buffer size or increase the tier capacity."
            ),
            Self::CapacityAccountingOverflow => write!(
                f,
                "Tiered cache byte accounting overflowed. Fix: rebuild the cache or shard entries before continuing."
            ),
        }
    }
}

impl std::error::Error for CacheError {}

/// Generic tiered cache for GPU buffers.
///
/// Tracks hot/cold buffers using the built-in [`LruPolicy`].
/// This is the vyre primitive that helix builds inference intelligence on top of.
#[non_exhaustive]
pub struct TieredCache {
    pub(crate) tiers: Vec<CacheTier>,
    pub(crate) tracker: AccessTracker,
    pub(crate) policy: LruPolicy,
    /// O(1) key → tier index. Eliminates the linear tier scan in `get`.
    index: FxHashMap<u64, usize>,
}

impl TieredCache {
    /// Create a new cache with the given tiers and a default [`LruPolicy`].
    #[inline]
    pub fn new(tiers: Vec<CacheTier>) -> Self {
        match Self::try_new(tiers) {
            Ok(cache) => cache,
            Err(error) => {
                tracing::error!(
                    error = %error,
                    "wgpu tiered cache tracker reservation failed; continuing with grow-on-use metadata"
                );
                Self::with_policy(Vec::new(), LruPolicy::default())
            }
        }
    }

    /// Fallible version of [`Self::new`].
    ///
    /// # Errors
    ///
    /// Returns [`vyre_driver::BackendError`] if cache access metadata cannot be
    /// reserved.
    #[inline]
    pub fn try_new(tiers: Vec<CacheTier>) -> Result<Self, vyre_driver::BackendError> {
        Self::try_with_policy(tiers, LruPolicy::default())
    }
}

impl TieredCache {
    /// Create a new cache with a custom LRU policy.
    #[inline]
    pub fn with_policy(tiers: Vec<CacheTier>, policy: LruPolicy) -> Self {
        match Self::try_with_policy(tiers, policy) {
            Ok(cache) => cache,
            Err(error) => {
                tracing::error!(
                    error = %error,
                    "wgpu tiered cache tracker reservation failed; continuing with grow-on-use metadata"
                );
                Self {
                    tiers: Vec::new(),
                    tracker: AccessTracker::new(),
                    policy,
                    index: FxHashMap::default(),
                }
            }
        }
    }

    /// Fallible version of [`Self::with_policy`].
    ///
    /// # Errors
    ///
    /// Returns [`vyre_driver::BackendError`] if cache access metadata cannot be
    /// reserved.
    #[inline]
    pub fn try_with_policy(
        tiers: Vec<CacheTier>,
        policy: LruPolicy,
    ) -> Result<Self, vyre_driver::BackendError> {
        Ok(Self {
            tiers,
            tracker: AccessTracker::try_new()?,
            policy,
            index: FxHashMap::default(),
        })
    }

    /// Return a reference to the entry with the given key, if it exists.
    #[inline]
    pub fn get(&self, key: u64) -> Option<&CacheEntry> {
        let &tier = self.index.get(&key)?;
        self.tiers[tier].entries.get(&key)
    }

    /// Insert a new entry into the lowest tier that can fit it.
    ///
    /// # Errors
    ///
    /// Returns [`CacheError::EntryTooLarge`] when no tier can hold the entry.
    #[inline]
    pub fn insert(&mut self, key: u64, size: u64) -> Result<(), CacheError> {
        if self.get(key).is_some() {
            self.evict(key);
        }
        self.tracker.set_size(key, size);
        self.insert_into_tier(key, size, 0)
    }

    /// Record an access for the given key.
    #[inline]
    pub fn record_access(&mut self, key: u64) {
        if let Some(&tier_id) = self.index.get(&key) {
            self.tracker.record(key);
            // Touch the per-tier recency LRU so eviction keeps the
            // hot key at the head and the coldest key at the tail.
            self.tiers[tier_id].lru.touch(key);
        }
    }

    /// Promote the entry to the next faster tier if the policy allows it.
    ///
    /// # Errors
    ///
    /// Returns [`CacheError::KeyNotFound`] when the key does not exist.
    #[inline]
    pub fn promote(&mut self, key: u64) -> Result<(), CacheError> {
        let entry = self.get(key).copied().ok_or(CacheError::KeyNotFound)?;
        let stats = self.tracker.stats(key).ok_or(CacheError::KeyNotFound)?;
        if !self.policy.should_promote(key, &stats) {
            return Ok(());
        }
        let target = entry
            .tier
            .checked_add(1)
            .ok_or(CacheError::CapacityAccountingOverflow)?;
        if target >= self.tiers.len() {
            return Ok(());
        }
        let size = entry.size;
        self.remove_entry(key);
        self.move_into_tier(key, size, target, entry.tier)
    }

    /// Demote the entry to the next slower tier.
    ///
    /// # Errors
    ///
    /// Returns [`CacheError::KeyNotFound`] when the key does not exist.
    #[inline]
    pub fn demote(&mut self, key: u64) -> Result<(), CacheError> {
        let entry = self.get(key).copied().ok_or(CacheError::KeyNotFound)?;
        if entry.tier == 0 {
            return Ok(());
        }
        let target = entry.tier - 1;
        let size = entry.size;
        self.remove_entry(key);
        self.move_into_tier(key, size, target, entry.tier)
    }

    fn insert_into_tier(
        &mut self,
        key: u64,
        size: u64,
        mut start: usize,
    ) -> Result<(), CacheError> {
        while start < self.tiers.len() {
            if size > self.tiers[start].capacity {
                start += 1;
                continue;
            }
            if self.make_room(start, size) {
                self.tiers[start].used = checked_tier_used_add(self.tiers[start].used, size)?;
                self.tiers[start].entries.insert(
                    key,
                    CacheEntry {
                        key,
                        size,
                        tier: start,
                    },
                );
                // Register the key in the tier's per-tier LRU so the
                // fast-path eviction can pop its tail in O(1).
                self.tiers[start].lru.ensure(key);
                self.tiers[start].lru.touch(key);
                self.index.insert(key, start);
                return Ok(());
            }
            start += 1;
        }
        Err(CacheError::EntryTooLarge)
    }

    fn move_into_tier(
        &mut self,
        key: u64,
        size: u64,
        target: usize,
        fallback: usize,
    ) -> Result<(), CacheError> {
        if self.make_room(target, size) {
            self.tiers[target].used = checked_tier_used_add(self.tiers[target].used, size)?;
            self.tiers[target].entries.insert(
                key,
                CacheEntry {
                    key,
                    size,
                    tier: target,
                },
            );
            self.tiers[target].lru.ensure(key);
            self.tiers[target].lru.touch(key);
            self.index.insert(key, target);
            Ok(())
        } else {
            self.insert_into_tier(key, size, fallback)
        }
    }

    fn make_room(&mut self, tier: usize, size: u64) -> bool {
        loop {
            let used = self.tiers[tier].used;
            let cap = self.tiers[tier].capacity;
            if used.checked_add(size).is_some_and(|total| total <= cap) {
                return true;
            }
            // O(1) fast-path eviction using the tier's own recency
            // LRU. The default `TierPolicy::eviction_candidate_per_tier`
            // delegates to the slow path so custom policies still work;
            // `LruPolicy` overrides it to pop the tier LRU tail
            // directly.
            let candidate = {
                let tier_ref = &self.tiers[tier];
                self.policy.eviction_candidate_per_tier(
                    tier,
                    &tier_ref.entries,
                    &self.tracker,
                    &tier_ref.lru,
                )
            };
            if let Some(key) = candidate {
                self.evict_from_tier(key, tier);
            } else {
                return false;
            }
        }
    }

    fn remove_entry(&mut self, key: u64) -> Option<CacheEntry> {
        let &tier_id = self.index.get(&key)?;
        let tier = &mut self.tiers[tier_id];
        let entry = tier.entries.remove(&key)?;
        tier.lru.remove(&key);
        debit_tier_used(tier, entry.size);
        self.index.remove(&key);
        Some(entry)
    }

    fn evict(&mut self, key: u64) -> Option<CacheEntry> {
        let &tier_id = self.index.get(&key)?;
        let tier = &mut self.tiers[tier_id];
        let entry = tier.entries.remove(&key)?;
        tier.lru.remove(&key);
        debit_tier_used(tier, entry.size);
        self.index.remove(&key);
        self.tracker.remove(key);
        Some(entry)
    }

    /// Find and remove the coldest entry from the cache.
    ///
    /// This follows the LRU policy across all tiers, starting from the
    /// lowest (coldest) tier. Returns the key of the evicted entry.
    pub fn evict_coldest(&mut self) -> Option<u64> {
        for (tier_idx, tier) in self.tiers.iter().enumerate() {
            if let Some(key) = self.policy.eviction_candidate_per_tier(
                tier_idx,
                &tier.entries,
                &self.tracker,
                &tier.lru,
            ) {
                self.evict_from_tier(key, tier_idx);
                return Some(key);
            }
        }
        None
    }

    fn evict_from_tier(&mut self, key: u64, tier: usize) -> Option<CacheEntry> {
        let tier = &mut self.tiers[tier];
        let entry = tier.entries.remove(&key)?;
        tier.lru.remove(&key);
        debit_tier_used(tier, entry.size);
        self.index.remove(&key);
        self.tracker.remove(key);
        Some(entry)
    }
}

fn checked_tier_used_add(used: u64, size: u64) -> Result<u64, CacheError> {
    used.checked_add(size)
        .ok_or(CacheError::CapacityAccountingOverflow)
}

fn debit_tier_used(tier: &mut CacheTier, size: u64) {
    match tier.used.checked_sub(size) {
        Some(used) => {
            tier.used = used;
        }
        None => {
            tracing::error!(
                tier = %tier.name,
                used = tier.used,
                removed_size = size,
                "tiered cache byte accounting underflowed; repairing from live entries. Fix: investigate mismatched cache tier metadata."
            );
            tier.used = recompute_tier_used(tier);
        }
    }
}

fn recompute_tier_used(tier: &CacheTier) -> u64 {
    let mut total = 0_u64;
    for entry in tier.entries.values() {
        total = match total.checked_add(entry.size) {
            Some(next) => next,
            None => {
                tracing::error!(
                    tier = %tier.name,
                    "tiered cache byte accounting overflowed while repairing from live entries; pinning used bytes to u64::MAX."
                );
                return u64::MAX;
            }
        };
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiered_cache_repairs_used_bytes_after_underflow_instead_of_panicking() {
        let mut cache = TieredCache::new(vec![CacheTier::new("gpu", 128)]);
        cache.insert(1, 64).expect("Fix: test insert must fit");
        cache.tiers[0].used = 0;

        let removed = cache
            .evict(1)
            .expect("Fix: corrupted entry should still evict");

        assert_eq!(removed.size, 64);
        assert_eq!(cache.tiers[0].used, 0);
        assert!(cache.get(1).is_none());
    }

    #[test]
    fn tiered_cache_source_has_no_release_path_panic_accounting() {
        let source = include_str!("tiered_cache.rs");
        let production = source
            .split("#[cfg(test)]")
            .next()
            .expect("Fix: tiered cache production source must precede tests");
        assert!(
            !production.contains(concat!("panic", "!("))
                && !production.contains(".unwrap_or_else("),
            "Fix: tiered cache accounting must repair or return typed errors instead of aborting."
        );
        assert!(
            production.contains("debit_tier_used")
                && production.contains("recompute_tier_used")
                && production.contains("repairing from live entries"),
            "Fix: tiered cache underflow must be repaired from live entry metadata with a loud diagnostic."
        );
    }
}
