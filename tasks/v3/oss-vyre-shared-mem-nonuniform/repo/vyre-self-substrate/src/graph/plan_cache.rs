//! Shared bounded Program cache for graph self-substrate dispatch wrappers.
//!
//! Persistent graph wrappers rebuild the same primitive `Program` shapes across
//! repeated queries. Caching those Programs is required for throughput, but an
//! unbounded cache lets hostile shape/key churn grow memory without limit. This
//! module centralizes the cache policy so wrappers do not each carry a private
//! `HashMap` with subtly different eviction behavior.

use std::collections::{HashMap, VecDeque};
use std::hash::Hash;

use vyre_foundation::ir::Program;

const DEFAULT_GRAPH_PLAN_CACHE_CAPACITY: usize = 128;

/// Snapshot of a graph Program cache.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct GraphPlanCacheSnapshot {
    /// Number of cached Programs.
    pub entries: usize,
    /// Number of lookups served from cache.
    pub hits: u64,
    /// Number of Programs built and inserted.
    pub misses: u64,
    /// Number of cached Programs evicted to maintain the capacity bound.
    pub evictions: u64,
}

/// Small bounded LRU cache for generated graph `Program` plans.
#[derive(Debug)]
pub(crate) struct GraphPlanCache<K> {
    entries: HashMap<K, Program>,
    lru: VecDeque<K>,
    capacity: usize,
    hits: u64,
    misses: u64,
    evictions: u64,
}

impl<K> Default for GraphPlanCache<K> {
    fn default() -> Self {
        Self {
            entries: HashMap::new(),
            lru: VecDeque::new(),
            capacity: DEFAULT_GRAPH_PLAN_CACHE_CAPACITY,
            hits: 0,
            misses: 0,
            evictions: 0,
        }
    }
}

impl<K> GraphPlanCache<K>
where
    K: Copy + Eq + Hash,
{
    /// Construct a cache with an explicit non-zero capacity.
    #[cfg(test)]
    pub(crate) fn with_capacity(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            ..Self::default()
        }
    }

    /// Return an existing cached Program or build, insert, and return a new one.
    pub(crate) fn get_or_build(&mut self, key: K, build: impl FnOnce() -> Program) -> Program {
        if let Some(program) = self.entries.get(&key).cloned() {
            self.hits = self.hits.saturating_add(1);
            self.touch(key);
            return program;
        }

        self.misses = self.misses.saturating_add(1);
        let program = build();
        while self.entries.len() >= self.capacity {
            let Some(victim) = self.lru.pop_front() else {
                break;
            };
            if self.entries.remove(&victim).is_some() {
                self.evictions = self.evictions.saturating_add(1);
                break;
            }
        }
        self.entries.insert(key, program.clone());
        self.lru.push_back(key);
        program
    }

    /// Return current cache counters.
    #[must_use]
    pub(crate) fn snapshot(&self) -> GraphPlanCacheSnapshot {
        GraphPlanCacheSnapshot {
            entries: self.entries.len(),
            hits: self.hits,
            misses: self.misses,
            evictions: self.evictions,
        }
    }

    fn touch(&mut self, key: K) {
        if let Some(index) = self.lru.iter().position(|candidate| *candidate == key) {
            self.lru.remove(index);
        }
        self.lru.push_back(key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre_foundation::ir::{Node, Program};

    fn empty_program() -> Program {
        Program::wrapped(Vec::new(), [1, 1, 1], vec![Node::Return])
    }

    #[test]
    fn cache_hits_reuse_existing_program_and_update_counters() {
        let mut cache = GraphPlanCache::with_capacity(4);

        cache.get_or_build(7u32, empty_program);
        cache.get_or_build(7u32, || panic!("cache hit must not rebuild"));

        let snapshot = cache.snapshot();
        assert_eq!(snapshot.entries, 1);
        assert_eq!(snapshot.hits, 1);
        assert_eq!(snapshot.misses, 1);
        assert_eq!(snapshot.evictions, 0);
    }

    #[test]
    fn cache_eviction_is_bounded_and_lru() {
        let mut cache = GraphPlanCache::with_capacity(2);

        cache.get_or_build(1u32, empty_program);
        cache.get_or_build(2u32, empty_program);
        cache.get_or_build(1u32, empty_program);
        cache.get_or_build(3u32, empty_program);

        let snapshot = cache.snapshot();
        assert_eq!(snapshot.entries, 2);
        assert_eq!(snapshot.hits, 1);
        assert_eq!(snapshot.misses, 3);
        assert_eq!(snapshot.evictions, 1);

        cache.get_or_build(2u32, empty_program);
        let snapshot = cache.snapshot();
        assert_eq!(snapshot.misses, 4, "key 2 should have been evicted first");
    }
}
