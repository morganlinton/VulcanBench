use super::{
    MegakernelLaunchCacheStats, MegakernelLaunchPolicy, MegakernelLaunchRecommendation,
    MegakernelLaunchRequest,
};
use rustc_hash::FxHashMap;
use std::cell::RefCell;

const LAUNCH_RECOMMENDATION_CACHE_CAP: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct LaunchRecommendationCacheKey {
    pub(super) policy: MegakernelLaunchPolicy,
    pub(super) request: MegakernelLaunchRequest,
}

pub(super) struct LaunchRecommendationCache {
    pub(super) entries: FxHashMap<LaunchRecommendationCacheKey, LaunchRecommendationCacheEntry>,
    clock: u64,
    pub(super) hits: u64,
    pub(super) misses: u64,
}

pub(super) struct LaunchRecommendationCacheEntry {
    recommendation: MegakernelLaunchRecommendation,
    last_seen: u64,
}

impl LaunchRecommendationCache {
    pub(super) fn get(
        &mut self,
        key: &LaunchRecommendationCacheKey,
    ) -> Option<MegakernelLaunchRecommendation> {
        if self.clock == u64::MAX {
            self.clock = 0;
            for entry in self.entries.values_mut() {
                entry.last_seen = 0;
            }
        }
        let Some(entry) = self.entries.get_mut(key) else {
            self.misses = self.misses.saturating_add(1);
            return None;
        };
        self.clock += 1;
        entry.last_seen = self.clock;
        self.hits = self.hits.saturating_add(1);
        Some(entry.recommendation)
    }

    pub(super) fn insert(
        &mut self,
        key: LaunchRecommendationCacheKey,
        value: MegakernelLaunchRecommendation,
    ) {
        let tick = self.next_tick();
        self.entries.insert(
            key,
            LaunchRecommendationCacheEntry {
                recommendation: value,
                last_seen: tick,
            },
        );
        while self.entries.len() > LAUNCH_RECOMMENDATION_CACHE_CAP {
            let Some(evicted) = self
                .entries
                .iter()
                .filter(|(candidate, _)| **candidate != key)
                .min_by_key(|(_, entry)| entry.last_seen)
                .map(|(candidate, _)| *candidate)
            else {
                break;
            };
            self.entries.remove(&evicted);
        }
    }

    pub(super) fn stats(&self) -> MegakernelLaunchCacheStats {
        MegakernelLaunchCacheStats {
            entries: self.entries.len(),
            hits: self.hits,
            misses: self.misses,
        }
    }

    pub(super) fn clear(&mut self) {
        self.entries.clear();
        self.clock = 0;
        self.hits = 0;
        self.misses = 0;
    }

    fn next_tick(&mut self) -> u64 {
        if self.clock == u64::MAX {
            self.clock = 0;
            for entry in self.entries.values_mut() {
                entry.last_seen = 0;
            }
        }
        self.clock += 1;
        self.clock
    }
}

impl Default for LaunchRecommendationCache {
    fn default() -> Self {
        Self {
            entries: FxHashMap::with_capacity_and_hasher(
                LAUNCH_RECOMMENDATION_CACHE_CAP,
                Default::default(),
            ),
            clock: 0,
            hits: 0,
            misses: 0,
        }
    }
}

thread_local! {
    pub(super) static LAUNCH_RECOMMENDATION_CACHE: RefCell<LaunchRecommendationCache> =
        RefCell::new(LaunchRecommendationCache::default());
}
