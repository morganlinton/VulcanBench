//! Unit tests for tiered cache index and O(1) stats.

use crate::runtime::cache::tiered_cache::{CacheTier, LruPolicy, TieredCache};
use crate::runtime::cache::AccessTracker;

#[test]
fn get_returns_entry_after_insert() {
    let mut cache = TieredCache::new(vec![CacheTier::new("L1", 1024)]);
    cache.insert(1, 100).unwrap();
    let entry = cache.get(1).unwrap();
    assert_eq!(entry.key, 1);
    assert_eq!(entry.size, 100);
    assert_eq!(entry.tier, 0);
}

#[test]
fn get_missing_returns_none() {
    let cache = TieredCache::new(vec![CacheTier::new("L1", 1024)]);
    assert!(cache.get(99).is_none());
}

#[test]
fn insert_replaces_existing_key() {
    let mut cache = TieredCache::new(vec![CacheTier::new("L1", 1024)]);
    cache.insert(1, 100).unwrap();
    cache.insert(1, 200).unwrap();
    let entry = cache.get(1).unwrap();
    assert_eq!(entry.size, 200);
}

#[test]
fn promote_moves_to_higher_tier() {
    let mut cache = TieredCache::new(vec![CacheTier::new("L1", 1024), CacheTier::new("L2", 1024)]);
    cache.insert(1, 100).unwrap();
    // Record enough accesses to meet promote threshold.
    for _ in 0..LruPolicy::DEFAULT_THRESHOLD {
        cache.record_access(1);
    }
    cache.promote(1).unwrap();
    let entry = cache.get(1).unwrap();
    assert_eq!(entry.tier, 1);
}

#[test]
fn demote_moves_to_lower_tier() {
    let mut cache = TieredCache::new(vec![CacheTier::new("L1", 1024), CacheTier::new("L2", 1024)]);
    cache.insert(1, 100).unwrap();
    for _ in 0..LruPolicy::DEFAULT_THRESHOLD {
        cache.record_access(1);
    }
    cache.promote(1).unwrap();
    cache.demote(1).unwrap();
    let entry = cache.get(1).unwrap();
    assert_eq!(entry.tier, 0);
}

#[test]
fn insert_replacement_evicts_old_entry() {
    let mut cache = TieredCache::new(vec![CacheTier::new("L1", 1024)]);
    cache.insert(1, 100).unwrap();
    cache.insert(1, 200).unwrap();
    assert!(cache.get(1).is_some());
    assert_eq!(cache.get(1).unwrap().size, 200);
}

#[test]
fn make_room_evicts_coldest() {
    let mut cache = TieredCache::new(vec![CacheTier::new("L1", 200)]);
    cache.insert(1, 100).unwrap();
    cache.insert(2, 100).unwrap();
    // Touch key 1 so key 2 is coldest.
    cache.record_access(1);
    // Inserting a third entry should evict key 2.
    cache.insert(3, 100).unwrap();
    assert!(cache.get(1).is_some());
    assert!(cache.get(2).is_none());
    assert!(cache.get(3).is_some());
}

#[test]
fn stats_returns_last_access_not_rank() {
    let mut tracker = AccessTracker::new();
    tracker.set_size(1, 100);
    tracker.record(1);
    tracker.record(2);
    tracker.record(1);
    let stats1 = tracker.stats(1).unwrap();
    let stats2 = tracker.stats(2).unwrap();
    // Higher tick = more recent.
    assert!(stats1.last_access > stats2.last_access);
    assert_eq!(stats1.frequency, 2);
    assert_eq!(stats2.frequency, 1);
}

#[test]
fn get_after_evict_from_tier_during_promote() {
    let mut cache = TieredCache::new(vec![CacheTier::new("L1", 200), CacheTier::new("L2", 200)]);
    cache.insert(1, 100).unwrap();
    cache.insert(2, 100).unwrap();
    for _ in 0..LruPolicy::DEFAULT_THRESHOLD {
        cache.record_access(1);
    }
    // Promote 1 to L2; no eviction needed.
    cache.promote(1).unwrap();
    assert!(cache.get(1).is_some());
    assert!(cache.get(2).is_some());
}

#[test]
fn get_after_tier_full_and_insert_evicts() {
    let mut cache = TieredCache::new(vec![CacheTier::new("L1", 250)]);
    cache.insert(1, 100).unwrap();
    cache.insert(2, 100).unwrap();
    // Both fit (200 <= 250). Touch 1 so it becomes hottest.
    cache.record_access(1);
    // Insert 3; needs 300 bytes total, so one entry must be evicted.
    // 2 is coldest because 1 was just touched.
    cache.insert(3, 100).unwrap();
    assert!(cache.get(1).is_some());
    assert!(cache.get(2).is_none());
    assert!(cache.get(3).is_some());
}
