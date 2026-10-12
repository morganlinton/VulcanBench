//! Heap-backed LRU index shared by GPU preprocessor resident caches.

use std::cmp::{Ordering, Reverse};
use std::collections::BinaryHeap;

#[derive(Debug)]
pub(super) struct LruIndex<K> {
    heap: BinaryHeap<Reverse<LruTouch<K>>>,
    serial: u64,
}

#[derive(Debug)]
struct LruTouch<K> {
    last_access: u64,
    serial: u64,
    key: K,
}

impl<K> PartialEq for LruTouch<K> {
    fn eq(&self, other: &Self) -> bool {
        self.last_access == other.last_access && self.serial == other.serial
    }
}

impl<K> Eq for LruTouch<K> {}

impl<K> Ord for LruTouch<K> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.last_access
            .cmp(&other.last_access)
            .then_with(|| self.serial.cmp(&other.serial))
    }
}

impl<K> PartialOrd for LruTouch<K> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<K> LruIndex<K> {
    pub(super) fn with_capacity(capacity: usize) -> Self {
        Self {
            heap: BinaryHeap::with_capacity(capacity),
            serial: 0,
        }
    }

    pub(super) fn record(&mut self, key: K, last_access: u64) {
        let serial = self.next_serial();
        self.heap.push(Reverse(LruTouch {
            last_access,
            serial,
            key,
        }));
    }

    pub(super) fn pop_valid<F>(&mut self, mut is_current: F) -> Option<K>
    where
        F: FnMut(&K, u64) -> bool,
    {
        while let Some(Reverse(touch)) = self.heap.pop() {
            if is_current(&touch.key, touch.last_access) {
                return Some(touch.key);
            }
        }
        None
    }

    pub(super) fn compact_if_needed<I>(&mut self, live_entries: usize, entries: I)
    where
        I: IntoIterator<Item = (K, u64)>,
    {
        let Some(compaction_threshold) = live_entries.checked_mul(4).map(|value| value.max(8))
        else {
            tracing::error!(
                "vyre-libs gpu preprocessor LRU index live-entry count {live_entries} overflowed compaction threshold. Fix: shard process-local preprocessor caches."
            );
            return;
        };
        if self.heap.len() <= compaction_threshold {
            return;
        }
        let mut compacted = BinaryHeap::new();
        if let Err(error) = compacted.try_reserve(live_entries) {
            tracing::error!(
                "vyre-libs gpu preprocessor LRU index could not reserve {live_entries} compacted entries: {error:?}. Fix: shard process-local preprocessor caches."
            );
            return;
        }
        for (key, last_access) in entries {
            let serial = self.next_serial();
            compacted.push(Reverse(LruTouch {
                last_access,
                serial,
                key,
            }));
        }
        self.heap = compacted;
    }

    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.heap.len()
    }

    fn next_serial(&mut self) -> u64 {
        self.serial = self.serial.saturating_add(1);
        self.serial
    }
}
