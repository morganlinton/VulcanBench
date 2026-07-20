//! Dispatcher-local fixed-batch megakernel launch-plan cache.

use super::dispatcher::BatchDispatchConfig;
use std::cmp::{Ordering, Reverse};
use std::collections::{BinaryHeap, HashMap};
use vyre_runtime::megakernel::{MegakernelDispatchTopology, MegakernelLaunchRecommendation};

/// Reusable fixed-batch megakernel launch metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BatchDispatchPlan {
    /// Queue length this plan was derived for.
    pub queue_len: u32,
    /// Workgroups submitted by the compiled persistent megakernel.
    pub worker_groups: u32,
    /// Worker lanes per workgroup.
    pub workgroup_size_x: u32,
    /// Sparse hit-ring capacity compiled into the dispatcher pipeline.
    pub hit_capacity: u32,
    /// Estimated peak device bytes required by the selected launch plan.
    pub estimated_peak_device_bytes: u64,
    /// Hard device-memory budget applied to this plan. Zero means unbounded.
    pub device_memory_budget_bytes: u64,
    /// Scale-aware topology selected for this queue shape.
    pub topology: MegakernelDispatchTopology,
}

impl BatchDispatchPlan {
    pub(crate) fn from_recommendation(
        queue_len: u32,
        config: &BatchDispatchConfig,
        recommendation: MegakernelLaunchRecommendation,
    ) -> Self {
        Self {
            queue_len,
            worker_groups: recommendation.worker_groups,
            workgroup_size_x: config.workgroup_size_x,
            hit_capacity: recommendation.hit_capacity,
            estimated_peak_device_bytes: recommendation.estimated_peak_device_bytes,
            device_memory_budget_bytes: recommendation.device_memory_budget_bytes,
            topology: recommendation.topology,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct BatchDispatchPlanCacheEntry {
    plan: BatchDispatchPlan,
    last_seen: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BatchDispatchPlanLruEntry {
    last_seen: u64,
    queue_len: u32,
}

impl Ord for BatchDispatchPlanLruEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        self.last_seen
            .cmp(&other.last_seen)
            .then_with(|| self.queue_len.cmp(&other.queue_len))
    }
}

impl PartialOrd for BatchDispatchPlanLruEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Small LRU cache for repeated fixed-batch launch metadata.
#[derive(Debug)]
pub(crate) struct BatchDispatchPlanCache {
    entries: HashMap<u32, BatchDispatchPlanCacheEntry>,
    lru: BinaryHeap<Reverse<BatchDispatchPlanLruEntry>>,
    clock: u64,
    cap: usize,
}

impl BatchDispatchPlanCache {
    fn with_cap(cap: usize) -> Self {
        Self {
            entries: HashMap::with_capacity(cap),
            lru: BinaryHeap::with_capacity(cap),
            clock: 0,
            cap,
        }
    }

    pub(crate) fn get(&mut self, queue_len: u32) -> Option<BatchDispatchPlan> {
        let tick = self.next_tick();
        let entry = self.entries.get_mut(&queue_len)?;
        entry.last_seen = tick;
        let plan = entry.plan;
        self.lru.push(Reverse(BatchDispatchPlanLruEntry {
            last_seen: entry.last_seen,
            queue_len,
        }));
        self.compact_lru_if_needed();
        Some(plan)
    }

    pub(crate) fn insert(&mut self, plan: BatchDispatchPlan) {
        let tick = self.next_tick();
        if let Some(entry) = self.entries.get_mut(&plan.queue_len) {
            entry.plan = plan;
            entry.last_seen = tick;
            self.lru.push(Reverse(BatchDispatchPlanLruEntry {
                last_seen: entry.last_seen,
                queue_len: plan.queue_len,
            }));
            self.compact_lru_if_needed();
            return;
        }
        if self.cap == 0 {
            return;
        }
        while self.entries.len() >= self.cap {
            let Some(queue_len) = self.pop_lru_key() else {
                break;
            };
            self.entries.remove(&queue_len);
        }
        let last_seen = tick;
        self.entries.insert(
            plan.queue_len,
            BatchDispatchPlanCacheEntry { plan, last_seen },
        );
        self.lru.push(Reverse(BatchDispatchPlanLruEntry {
            last_seen,
            queue_len: plan.queue_len,
        }));
        self.compact_lru_if_needed();
    }

    pub(crate) fn len_u16(&self) -> u16 {
        u16::try_from(self.entries.len()).unwrap_or(u16::MAX)
    }

    fn next_tick(&mut self) -> u64 {
        if self.clock == u64::MAX {
            self.rebase_clock_to_zero();
        }
        self.clock += 1;
        self.clock
    }

    fn rebase_clock_to_zero(&mut self) {
        self.clock = 0;
        self.lru.clear();
        for (queue_len, entry) in &mut self.entries {
            entry.last_seen = 0;
            self.lru.push(Reverse(BatchDispatchPlanLruEntry {
                last_seen: 0,
                queue_len: *queue_len,
            }));
        }
    }

    fn pop_lru_key(&mut self) -> Option<u32> {
        while let Some(Reverse(entry)) = self.lru.pop() {
            if self
                .entries
                .get(&entry.queue_len)
                .is_some_and(|current| current.last_seen == entry.last_seen)
            {
                return Some(entry.queue_len);
            }
        }
        None
    }

    fn compact_lru_if_needed(&mut self) {
        let live = self.entries.len();
        if let Some(limit) = stale_lru_limit(live) {
            if self.lru.len() <= limit {
                return;
            }
        }
        self.lru.clear();
        self.lru
            .extend(self.entries.iter().map(|(&queue_len, entry)| {
                Reverse(BatchDispatchPlanLruEntry {
                    last_seen: entry.last_seen,
                    queue_len,
                })
            }));
    }
}

fn stale_lru_limit(live: usize) -> Option<usize> {
    live.checked_mul(4).map(|limit| limit.max(8))
}

impl Default for BatchDispatchPlanCache {
    fn default() -> Self {
        Self::with_cap(32)
    }
}

/// Result of looking up fixed-batch launch metadata.
#[derive(Debug, Clone, Copy)]
pub(crate) struct BatchDispatchPlanLookup {
    /// Cached or newly planned launch metadata.
    pub(crate) plan: BatchDispatchPlan,
    /// True when the dispatcher reused resident launch metadata.
    pub(crate) cache_hit: bool,
    /// Number of resident entries after lookup.
    pub(crate) cache_entries: u16,
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre_runtime::megakernel::{
        MegakernelDispatchTopology, MegakernelExecutionMode, MegakernelLaunchGeometry,
        MegakernelQueuePressure,
    };

    #[test]
    fn dispatch_plan_consumes_recommended_worker_groups() {
        let config = BatchDispatchConfig {
            worker_groups: 8,
            workgroup_size_x: 64,
            hit_capacity: 1024,
            ..Default::default()
        };
        let recommendation = MegakernelLaunchRecommendation {
            geometry: MegakernelLaunchGeometry {
                workgroup_size_x: 64,
                slot_count: 2048,
                dispatch_grid: [32, 1, 1],
            },
            worker_groups: 32,
            hit_capacity: 4096,
            pressure: MegakernelQueuePressure::Balanced,
            execution_mode: MegakernelExecutionMode::Interpreter,
            topology: MegakernelDispatchTopology::SparseFrontier,
            promote_hot_opcodes: false,
            promote_hot_windows: false,
            age_priority_work: false,
            estimated_peak_device_bytes: 65_536,
            device_memory_budget_bytes: 0,
        };

        let plan = BatchDispatchPlan::from_recommendation(1024, &config, recommendation);

        assert_eq!(
            plan.worker_groups, 32,
            "dispatch plan must use scale-policy worker_groups, not the constructor seed"
        );
        assert_eq!(
            plan.hit_capacity, 4096,
            "dispatch plan must use scale-policy hit_capacity, not stale config capacity"
        );
        assert_eq!(plan.estimated_peak_device_bytes, 65_536);
    }

    #[test]
    fn dispatch_plan_cache_lru_heap_stays_capacity_scale() {
        let config = BatchDispatchConfig {
            worker_groups: 8,
            workgroup_size_x: 64,
            hit_capacity: 1024,
            ..Default::default()
        };
        let mut cache = BatchDispatchPlanCache::with_cap(4);

        for queue_len in 1..128 {
            let recommendation = MegakernelLaunchRecommendation {
                geometry: MegakernelLaunchGeometry {
                    workgroup_size_x: 64,
                    slot_count: queue_len,
                    dispatch_grid: [32, 1, 1],
                },
                worker_groups: 32,
                hit_capacity: 4096,
                pressure: MegakernelQueuePressure::Balanced,
                execution_mode: MegakernelExecutionMode::Interpreter,
                topology: MegakernelDispatchTopology::SparseFrontier,
                promote_hot_opcodes: false,
                promote_hot_windows: false,
                age_priority_work: false,
                estimated_peak_device_bytes: 65_536,
                device_memory_budget_bytes: 0,
            };
            let plan = BatchDispatchPlan::from_recommendation(queue_len, &config, recommendation);
            cache.insert(plan);
            let _ = cache.get(queue_len);
        }

        assert_eq!(cache.entries.len(), 4);
        assert!(
            cache.lru.len() <= cache.entries.len().saturating_mul(4).max(8),
            "Fix: dispatch-plan LRU heap must compact stale recency entries to cache-capacity scale"
        );
    }
}
