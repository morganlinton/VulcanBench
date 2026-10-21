//! Unit tests extracted from `runtime/cache/lru.rs`.

use crate::runtime::cache::lru::{AccessTracker, IntrusiveLru, DEFAULT_INTRUSIVE_LRU_CAPACITY};

#[test]
#[inline]
fn touch_present_and_missing_keys() {
    let mut lru = IntrusiveLru::<u32, u32>::new();
    lru.ensure(1);
    lru.ensure(2);

    // Touching an existing key should move it to the front.
    lru.touch(1);
    assert_eq!(lru.hottest(2), vec![1, 2]);

    // Touching a missing key must not panic.
    lru.touch(99);
    assert_eq!(lru.hottest(2), vec![1, 2]);
}

#[test]
#[inline]
fn capacity_evicts_coldest_without_growing_live_set() {
    let mut lru = IntrusiveLru::<u32, u32>::with_capacity(2);
    *lru.ensure(1) = 10;
    *lru.ensure(2) = 20;
    lru.touch(1);
    *lru.ensure(3) = 30;

    assert_eq!(lru.get(&1), Some(&10));
    assert_eq!(lru.get(&2), None);
    assert_eq!(lru.get(&3), Some(&30));
    assert_eq!(lru.hottest(3), vec![3, 1]);
}

#[test]
#[inline]
fn reserved_capacity_does_not_evict_live_metadata() {
    let mut lru = IntrusiveLru::<u32, u32>::with_reserved_capacity(2);
    *lru.ensure(1) = 10;
    *lru.ensure(2) = 20;
    *lru.ensure(3) = 30;

    assert_eq!(lru.get(&1), Some(&10));
    assert_eq!(lru.get(&2), Some(&20));
    assert_eq!(lru.get(&3), Some(&30));
    assert_eq!(lru.hottest(3), vec![3, 2, 1]);
}

#[test]
#[inline]
fn access_tracker_retains_stats_beyond_default_reservation() {
    let mut tracker = AccessTracker::new();
    let overflow_key = DEFAULT_INTRUSIVE_LRU_CAPACITY as u64;
    for key in 0..=overflow_key {
        tracker.record(key);
    }

    assert!(
        tracker.stats(0).is_some(),
        "Fix: access stats for live cache entries must not disappear when the tracker exceeds its initial reservation"
    );
    assert!(
        tracker.stats(overflow_key).is_some(),
        "Fix: access stats for newly recorded cache entries must remain available after reservation growth"
    );
}

#[test]
#[inline]
fn with_capacity_reserves_full_slab_and_index_budget() {
    let lru = IntrusiveLru::<u32, u32>::with_capacity(4096);
    let (nodes, indices, free) = lru.reserved_capacity_for_diagnostics();

    assert!(
        nodes >= 4096,
        "Fix: LRU slab must reserve full requested capacity, got {nodes}"
    );
    assert!(
        indices >= 4096,
        "Fix: LRU index must reserve full requested capacity, got {indices}"
    );
    assert!(
        free >= 4096,
        "Fix: LRU free list must reserve full requested capacity, got {free}"
    );
}
