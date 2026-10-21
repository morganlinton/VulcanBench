use rustc_hash::FxHashMap;

use crate::allocation::{reserve_hash_map_to_capacity, reserve_vec_to_capacity};

/// Default initial node reservation used by [`IntrusiveLru`].
pub const DEFAULT_INTRUSIVE_LRU_CAPACITY: usize = 65_536;

/// Intrusive doubly-linked LRU over a slab allocator.
///
/// O(1) record, remove, and hottest/coldest iteration.
pub struct IntrusiveLru<K, V> {
    nodes: Vec<Node<K, V>>,
    indices: FxHashMap<K, usize>,
    free: Vec<usize>,
    head: Option<usize>,
    tail: Option<usize>,
    live_limit: Option<usize>,
}

struct Node<K, V> {
    key: K,
    value: V,
    prev: Option<usize>,
    next: Option<usize>,
    active: bool,
}

impl<K, V> IntrusiveLru<K, V>
where
    K: std::hash::Hash + Eq + Copy,
    V: Default,
{
    /// Create an LRU with the default live-node capacity.
    #[inline]
    pub fn new() -> Self {
        match Self::try_new() {
            Ok(lru) => lru,
            Err(error) => {
                tracing::error!(
                    error = %error,
                    "wgpu intrusive LRU default reservation failed; continuing with grow-on-use storage"
                );
                Self::empty_with_policy(None)
            }
        }
    }

    /// Fallible version of [`Self::new`].
    ///
    /// # Errors
    ///
    /// Returns [`vyre_driver::BackendError`] if default LRU backing storage
    /// cannot be reserved.
    #[inline]
    pub fn try_new() -> Result<Self, vyre_driver::BackendError> {
        Self::try_with_reserved_capacity(DEFAULT_INTRUSIVE_LRU_CAPACITY)
    }

    /// Create an LRU with a fixed live-node capacity.
    ///
    /// A zero capacity is clamped to one so externally-derived
    /// capacity budgets cannot disable the LRU by accident.
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        let capacity = capacity.max(1);
        match Self::try_with_capacity(capacity) {
            Ok(lru) => lru,
            Err(error) => {
                tracing::error!(
                    capacity,
                    error = %error,
                    "wgpu intrusive LRU bounded reservation failed; continuing with grow-on-use storage"
                );
                Self::empty_with_policy(Some(capacity))
            }
        }
    }

    /// Fallible version of [`Self::with_capacity`].
    ///
    /// # Errors
    ///
    /// Returns [`vyre_driver::BackendError`] if LRU backing storage cannot be
    /// reserved.
    #[inline]
    pub fn try_with_capacity(capacity: usize) -> Result<Self, vyre_driver::BackendError> {
        // Defensive: a capacity of 0 would make the LRU unusable; clamp to 1
        // so callers that compute capacity from external config never panic.
        let capacity = capacity.max(1);
        Self::try_with_capacity_policy(capacity, Some(capacity))
    }

    /// Create an LRU that reserves `capacity` slots but does not silently evict
    /// live nodes when the reservation is exceeded.
    ///
    /// Cache metadata uses this path because the owning cache, not the LRU
    /// backing store, defines when an entry is evicted. Dropping metadata while
    /// the cache entry is still live would make promotion stats disappear and
    /// force cold-path scans at scale.
    #[inline]
    pub fn with_reserved_capacity(capacity: usize) -> Self {
        match Self::try_with_reserved_capacity(capacity) {
            Ok(lru) => lru,
            Err(error) => {
                tracing::error!(
                    capacity,
                    error = %error,
                    "wgpu intrusive LRU reservation failed; continuing with grow-on-use storage"
                );
                Self::empty_with_policy(None)
            }
        }
    }

    /// Fallible version of [`Self::with_reserved_capacity`].
    ///
    /// # Errors
    ///
    /// Returns [`vyre_driver::BackendError`] if LRU backing storage cannot be
    /// reserved.
    #[inline]
    pub fn try_with_reserved_capacity(capacity: usize) -> Result<Self, vyre_driver::BackendError> {
        let capacity = capacity.max(1);
        Self::try_with_capacity_policy(capacity, None)
    }

    fn try_with_capacity_policy(
        capacity: usize,
        live_limit: Option<usize>,
    ) -> Result<Self, vyre_driver::BackendError> {
        let mut nodes = Vec::new();
        reserve_vec_to_capacity(
            &mut nodes,
            capacity,
            "wgpu intrusive LRU",
            "node slot",
            "reduce runtime cache capacity or shard cache metadata",
        )?;
        let mut indices = FxHashMap::default();
        reserve_hash_map_to_capacity(
            &mut indices,
            capacity,
            "wgpu intrusive LRU",
            "index entry",
            "reduce runtime cache capacity or shard cache metadata",
        )?;
        let mut free = Vec::new();
        reserve_vec_to_capacity(
            &mut free,
            capacity,
            "wgpu intrusive LRU",
            "free-list slot",
            "reduce runtime cache capacity or shard cache metadata",
        )?;
        Ok(Self {
            nodes,
            indices,
            free,
            head: None,
            tail: None,
            live_limit,
        })
    }

    fn empty_with_policy(live_limit: Option<usize>) -> Self {
        Self {
            nodes: Vec::new(),
            indices: FxHashMap::default(),
            free: Vec::new(),
            head: None,
            tail: None,
            live_limit,
        }
    }

    /// Ensure a node exists for `key` and return a mutable value reference.
    #[inline]
    pub fn ensure(&mut self, key: K) -> &mut V {
        if let Some(&index) = self.indices.get(&key) {
            return &mut self.nodes[index].value;
        }
        let index = self.alloc_node(key);
        &mut self.nodes[index].value
    }

    /// Ensure a node exists for `key`, move it to the hot end, and
    /// return a mutable value reference.
    #[inline]
    pub fn ensure_front(&mut self, key: K) -> &mut V {
        let index = if let Some(&index) = self.indices.get(&key) {
            self.move_to_front(index);
            index
        } else {
            self.alloc_node(key)
        };
        &mut self.nodes[index].value
    }

    /// Move `key` to the front if it is present.
    #[inline]
    pub fn touch(&mut self, key: K) {
        if let Some(&index) = self.indices.get(&key) {
            self.move_to_front(index);
        }
    }

    /// Remove a key if it is present.
    #[inline]
    pub fn remove(&mut self, key: &K) {
        let Some(index) = self.indices.remove(key) else {
            return;
        };
        self.detach(index);
        let node = &mut self.nodes[index];
        node.active = false;
        self.free.push(index);
    }

    /// Return the value for `key` if it is currently active.
    #[inline]
    pub fn get(&self, key: &K) -> Option<&V> {
        let &index = self.indices.get(key)?;
        let node = &self.nodes[index];
        node.active.then_some(&node.value)
    }

    /// Return the `n` hottest keys in most-recent-first order.
    #[inline]
    pub fn hottest(&self, n: usize) -> Vec<K> {
        let mut keys = Vec::new();
        keys.extend(self.iter_hottest().map(|(key, _)| *key).take(n));
        keys
    }

    /// Iterate entries from most recent to least recent.
    #[inline]
    pub fn iter_hottest(&self) -> impl Iterator<Item = (&K, &V)> + '_ {
        let mut current = self.head;
        std::iter::from_fn(move || {
            let index = current?;
            let node = &self.nodes[index];
            current = node.next;
            Some((&node.key, &node.value))
        })
    }

    /// Iterate entries from least recent to most recent.
    #[inline]
    pub fn iter_coldest(&self) -> impl Iterator<Item = (&K, &V)> + '_ {
        let mut current = self.tail;
        std::iter::from_fn(move || {
            let index = current?;
            let node = &self.nodes[index];
            current = node.prev;
            Some((&node.key, &node.value))
        })
    }

    fn alloc_node(&mut self, key: K) -> usize {
        if self.live_limit == Some(self.indices.len()) {
            if let Some(coldest) = self.tail {
                let evicted_key = self.nodes[coldest].key;
                self.remove(&evicted_key);
            }
        }
        let index = if let Some(index) = self.free.pop() {
            self.nodes[index] = Node {
                key,
                value: V::default(),
                prev: None,
                next: None,
                active: true,
            };
            index
        } else {
            self.nodes.push(Node {
                key,
                value: V::default(),
                prev: None,
                next: None,
                active: true,
            });
            self.nodes.len() - 1
        };
        self.indices.insert(key, index);
        self.attach_front(index);
        index
    }

    /// Return backing-store capacities for cache diagnostics.
    ///
    /// This is intentionally public rather than test-only so structure
    /// contracts do not need inline test-only hooks in production modules.
    #[doc(hidden)]
    pub fn reserved_capacity_for_diagnostics(&self) -> (usize, usize, usize) {
        (
            self.nodes.capacity(),
            self.indices.capacity(),
            self.free.capacity(),
        )
    }

    fn move_to_front(&mut self, index: usize) {
        if self.head == Some(index) {
            return;
        }
        self.detach(index);
        self.attach_front(index);
    }

    fn attach_front(&mut self, index: usize) {
        self.nodes[index].prev = None;
        self.nodes[index].next = self.head;
        if let Some(head) = self.head {
            self.nodes[head].prev = Some(index);
        } else {
            self.tail = Some(index);
        }
        self.head = Some(index);
    }

    fn detach(&mut self, index: usize) {
        let prev = self.nodes[index].prev;
        let next = self.nodes[index].next;
        if let Some(prev) = prev {
            self.nodes[prev].next = next;
        } else if self.head == Some(index) {
            self.head = next;
        }
        if let Some(next) = next {
            self.nodes[next].prev = prev;
        } else if self.tail == Some(index) {
            self.tail = prev;
        }
        self.nodes[index].prev = None;
        self.nodes[index].next = None;
    }
}

impl<K, V> Default for IntrusiveLru<K, V>
where
    K: std::hash::Hash + Eq + Copy,
    V: Default,
{
    fn default() -> Self {
        Self::new()
    }
}

/// Metadata attached to each LRU node inside [`AccessTracker`].
#[derive(Debug, Clone, Copy, Default)]
pub struct AccessMeta {
    /// Number of recorded accesses.
    pub frequency: u32,
    /// Entry size in bytes.
    pub size: u64,
    /// Monotonic tick recorded for the last access.
    pub last_access: u64,
}

/// Tracks access patterns for cache entries.
#[non_exhaustive]
pub struct AccessTracker {
    lru: IntrusiveLru<u64, AccessMeta>,
    tick: u64,
}

impl AccessTracker {
    /// Create a new empty tracker.
    #[inline]
    pub fn new() -> Self {
        match Self::try_new() {
            Ok(tracker) => tracker,
            Err(error) => {
                tracing::error!(
                    error = %error,
                    "wgpu access tracker reservation failed; continuing with grow-on-use storage"
                );
                Self {
                    lru: IntrusiveLru::empty_with_policy(None),
                    tick: 0,
                }
            }
        }
    }

    /// Fallible version of [`Self::new`].
    ///
    /// # Errors
    ///
    /// Returns [`vyre_driver::BackendError`] if tracker backing storage cannot
    /// be reserved.
    #[inline]
    pub fn try_new() -> Result<Self, vyre_driver::BackendError> {
        Ok(Self {
            lru: IntrusiveLru::try_new()?,
            tick: 0,
        })
    }

    /// Record an access for the given key.
    #[inline]
    pub fn record(&mut self, key: u64) {
        self.advance_tick();
        let meta = self.lru.ensure_front(key);
        meta.frequency = bounded_frequency_increment(meta.frequency);
        meta.last_access = self.tick;
    }

    /// Return the `n` hottest keys in most-recent-first order.
    #[inline]
    pub fn hot_set(&self, n: usize) -> Vec<u64> {
        self.lru.hottest(n)
    }

    #[inline]
    pub(crate) fn set_size(&mut self, key: u64, size: u64) {
        self.lru.ensure(key).size = size;
    }

    #[inline]
    pub(crate) fn remove(&mut self, key: u64) {
        self.lru.remove(&key);
    }

    #[inline]
    pub(crate) fn get_meta(&self, key: u64) -> Option<&AccessMeta> {
        self.lru.get(&key)
    }

    /// Return access statistics for a key.
    #[inline]
    pub fn stats(&self, key: u64) -> Option<crate::runtime::cache::AccessStats> {
        let meta = self.get_meta(key)?;
        // O(1) relative-recency via monotonic tick counter instead of O(N)
        // linear scan through the intrusive list.
        Some(crate::runtime::cache::AccessStats {
            frequency: meta.frequency,
            last_access: meta.last_access,
            size: meta.size,
        })
    }

    fn advance_tick(&mut self) {
        if let Some(next) = self.tick.checked_add(1) {
            self.tick = next;
            return;
        }
        self.rebase_ticks_by_lru_order();
        self.tick = match self.tick.checked_add(1) {
            Some(next) => next,
            None => u64::MAX,
        };
    }

    fn rebase_ticks_by_lru_order(&mut self) {
        let mut current = self.lru.tail;
        let mut tick = 0_u64;
        while let Some(index) = current {
            let next = self.lru.nodes[index].prev;
            if self.lru.nodes[index].active {
                tick = match tick.checked_add(1) {
                    Some(next_tick) => next_tick,
                    None => u64::MAX,
                };
                self.lru.nodes[index].value.last_access = tick;
            }
            current = next;
        }
        self.tick = tick;
    }
}

fn bounded_frequency_increment(value: u32) -> u32 {
    match value.checked_add(1) {
        Some(next) => next,
        None => u32::MAX,
    }
}

impl Default for AccessTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intrusive_lru_constructors_use_shared_fallible_reservation() {
        let bounded = IntrusiveLru::<u64, AccessMeta>::try_with_capacity(4)
            .expect("Fix: bounded LRU capacity should reserve");
        let reserved = IntrusiveLru::<u64, AccessMeta>::try_with_reserved_capacity(4)
            .expect("Fix: reserved LRU capacity should reserve");

        assert!(bounded.reserved_capacity_for_diagnostics().0 >= 4);
        assert!(reserved.reserved_capacity_for_diagnostics().0 >= 4);

        let production = include_str!("lru.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("Fix: lru.rs must contain production section");
        assert!(
            production.contains("fn try_with_capacity_policy")
                && production.contains("reserve_vec_to_capacity")
                && production.contains("reserve_hash_map_to_capacity")
                && production.contains("pub fn try_new()")
                && !production.contains("Vec::with_capacity")
                && !production.contains("FxHashMap::with_capacity_and_hasher"),
            "Fix: WGPU runtime LRU constructors must share fallible reservation rather than duplicating infallible capacity constructors."
        );
        assert!(
            !production.contains(".expect("),
            "Fix: WGPU runtime LRU production constructors must not panic on allocation pressure."
        );
    }

    #[test]
    fn access_tracker_rebases_ticks_in_lru_order_instead_of_panicking() {
        let mut tracker = AccessTracker::new();
        tracker.record(10);
        tracker.record(20);
        tracker.record(30);
        tracker.tick = u64::MAX;

        tracker.record(20);

        assert_eq!(tracker.hot_set(3), vec![20, 30, 10]);
        let hot = tracker.stats(20).expect("Fix: hot key must remain tracked");
        let warm = tracker
            .stats(30)
            .expect("Fix: warm key must remain tracked");
        let cold = tracker
            .stats(10)
            .expect("Fix: cold key must remain tracked");
        assert!(hot.last_access > warm.last_access);
        assert!(warm.last_access > cold.last_access);
    }

    #[test]
    fn access_tracker_frequency_pins_instead_of_panicking() {
        let mut tracker = AccessTracker::new();
        tracker.record(7);
        tracker.lru.ensure(7).frequency = u32::MAX;

        tracker.record(7);

        assert_eq!(
            tracker
                .stats(7)
                .expect("Fix: tracked key must have stats")
                .frequency,
            u32::MAX
        );
    }

    #[test]
    fn access_tracker_source_has_no_release_path_panic_counters() {
        let source = include_str!("lru.rs");
        let production = source
            .split("#[cfg(test)]")
            .next()
            .expect("Fix: LRU production source must precede tests");
        assert!(
            !production.contains(concat!("panic", "!("))
                && !production.contains(".unwrap_or_else("),
            "Fix: runtime cache LRU counters must rebase or pin instead of aborting."
        );
        assert!(
            production.contains("rebase_ticks_by_lru_order")
                && production.contains("bounded_frequency_increment"),
            "Fix: runtime cache LRU must preserve recency across tick exhaustion and pin access frequency."
        );
    }
}
