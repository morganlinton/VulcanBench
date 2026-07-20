//! Dispatcher-local compiled megakernel pipeline variant cache.

use crate::pipeline::WgpuPipeline;
use std::cmp::{Ordering, Reverse};
use std::collections::{BinaryHeap, HashMap};
use std::sync::Arc;

/// Program-shaping fields that select a compiled batched megakernel pipeline.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct BatchPipelineShape {
    /// Worker lanes per workgroup.
    pub(crate) workgroup_size_x: u32,
    /// Workgroups submitted by this compiled variant.
    pub(crate) worker_groups: u32,
    /// Sparse hit-ring capacity compiled into this variant.
    pub(crate) hit_capacity: u32,
}

#[derive(Debug)]
struct BatchPipelineCacheEntry {
    pipeline: Arc<WgpuPipeline>,
    last_seen: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct BatchPipelineLruEntry {
    last_seen: u64,
    shape: BatchPipelineShape,
}

impl Ord for BatchPipelineLruEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        self.last_seen
            .cmp(&other.last_seen)
            .then_with(|| self.shape.cmp(&other.shape))
    }
}

impl PartialOrd for BatchPipelineLruEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Bounded LRU cache for compiled scale-aware batched megakernel pipelines.
#[derive(Debug)]
pub(crate) struct BatchPipelineCache {
    entries: HashMap<BatchPipelineShape, BatchPipelineCacheEntry>,
    lru: BinaryHeap<Reverse<BatchPipelineLruEntry>>,
    clock: u64,
    cap: usize,
}

impl BatchPipelineCache {
    pub(crate) fn with_cap(cap: usize) -> Self {
        Self {
            entries: HashMap::with_capacity(cap),
            lru: BinaryHeap::with_capacity(cap),
            clock: 0,
            cap,
        }
    }

    pub(crate) fn seed(&mut self, shape: BatchPipelineShape, pipeline: Arc<WgpuPipeline>) {
        self.insert(shape, pipeline);
    }

    pub(crate) fn get(&mut self, shape: BatchPipelineShape) -> Option<Arc<WgpuPipeline>> {
        let tick = self.next_tick();
        let entry = self.entries.get_mut(&shape)?;
        entry.last_seen = tick;
        let pipeline = entry.pipeline.clone();
        self.lru.push(Reverse(BatchPipelineLruEntry {
            last_seen: entry.last_seen,
            shape,
        }));
        self.compact_lru_if_needed();
        Some(pipeline)
    }

    pub(crate) fn insert(&mut self, shape: BatchPipelineShape, pipeline: Arc<WgpuPipeline>) {
        if self.cap == 0 {
            return;
        }
        let tick = self.next_tick();
        if let Some(entry) = self.entries.get_mut(&shape) {
            entry.pipeline = pipeline;
            entry.last_seen = tick;
            self.lru.push(Reverse(BatchPipelineLruEntry {
                last_seen: entry.last_seen,
                shape,
            }));
            self.compact_lru_if_needed();
            return;
        }
        while self.entries.len() >= self.cap {
            let Some(evict_shape) = self.pop_lru_shape() else {
                break;
            };
            self.entries.remove(&evict_shape);
        }
        let last_seen = tick;
        self.entries.insert(
            shape,
            BatchPipelineCacheEntry {
                pipeline,
                last_seen,
            },
        );
        self.lru
            .push(Reverse(BatchPipelineLruEntry { last_seen, shape }));
        self.compact_lru_if_needed();
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
        for (shape, entry) in &mut self.entries {
            entry.last_seen = 0;
            self.lru.push(Reverse(BatchPipelineLruEntry {
                last_seen: 0,
                shape: *shape,
            }));
        }
    }

    fn pop_lru_shape(&mut self) -> Option<BatchPipelineShape> {
        while let Some(Reverse(entry)) = self.lru.pop() {
            if self
                .entries
                .get(&entry.shape)
                .is_some_and(|current| current.last_seen == entry.last_seen)
            {
                return Some(entry.shape);
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
        self.lru.extend(self.entries.iter().map(|(&shape, entry)| {
            Reverse(BatchPipelineLruEntry {
                last_seen: entry.last_seen,
                shape,
            })
        }));
    }
}

fn stale_lru_limit(live: usize) -> Option<usize> {
    live.checked_mul(4).map(|limit| limit.max(8))
}
