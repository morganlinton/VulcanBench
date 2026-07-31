use crate::graph::plan_cache::GraphPlanCache;
use crate::graph::resident_handles::free_unique_resident_handles;
use crate::optimizer::dispatcher::{DispatchError, OptimizerDispatcher};
use vyre_primitives::graph::adaptive_traverse::AdaptiveTraversalPlanCacheKey;

pub(super) use vyre_primitives::graph::adaptive_traverse::{
    adaptive_four_russians_graph_content_hash as adaptive_four_russians_layout_hash,
    adaptive_traversal_graph_content_hash as adaptive_traversal_layout_hash,
};

/// Reusable resident frontier/count scratch for adaptive traversal.
#[derive(Debug, Default)]
pub struct AdaptiveTraversalResidentScratch {
    pub(super) handles: Option<[u64; 3]>,
    pub(super) queue_handle: Option<u64>,
    pub(super) high_queue_handle: Option<u64>,
    pub(super) high_len_handle: Option<u64>,
    pub(super) word_partials_handle: Option<u64>,
    pub(super) word_block_totals_handle: Option<u64>,
    pub(super) frontier_bytes: usize,
    pub(super) queue_bytes: usize,
    pub(super) high_queue_bytes: usize,
    pub(super) word_partials_bytes: usize,
    pub(super) word_block_totals_bytes: usize,
    pub(super) frontier_in_bytes: Vec<u8>,
    pub(super) readbacks: Vec<Vec<u8>>,
    pub(super) plan_cache: AdaptiveTraversalPlanCache,
}

impl AdaptiveTraversalResidentScratch {
    /// Snapshot plan-cache counters for repeated adaptive traversal tests.
    #[must_use]
    pub fn plan_cache_snapshot(&self) -> AdaptiveTraversalPlanCacheSnapshot {
        let snapshot = self.plan_cache.snapshot();
        AdaptiveTraversalPlanCacheSnapshot {
            entries: snapshot.entries,
            hits: snapshot.hits,
            misses: snapshot.misses,
        }
    }

    /// Free resident frontier/output/count buffers owned by this scratch.
    ///
    /// # Errors
    ///
    /// Returns the first backend free failure after attempting all handles.
    pub fn free(&mut self, dispatcher: &dyn OptimizerDispatcher) -> Result<(), DispatchError> {
        let Some(handles) = self.handles.take() else {
            let mut scratch_handles = [0_u64; 5];
            let mut scratch_count = 0;
            if let Some(queue_handle) = self.queue_handle.take() {
                scratch_handles[scratch_count] = queue_handle;
                scratch_count += 1;
            }
            if let Some(high_queue_handle) = self.high_queue_handle.take() {
                scratch_handles[scratch_count] = high_queue_handle;
                scratch_count += 1;
            }
            if let Some(high_len_handle) = self.high_len_handle.take() {
                scratch_handles[scratch_count] = high_len_handle;
                scratch_count += 1;
            }
            if let Some(word_partials_handle) = self.word_partials_handle.take() {
                scratch_handles[scratch_count] = word_partials_handle;
                scratch_count += 1;
            }
            if let Some(word_block_totals_handle) = self.word_block_totals_handle.take() {
                scratch_handles[scratch_count] = word_block_totals_handle;
                scratch_count += 1;
            }
            self.queue_bytes = 0;
            self.high_queue_bytes = 0;
            self.word_partials_bytes = 0;
            self.word_block_totals_bytes = 0;
            if scratch_count != 0 {
                return free_unique_resident_handles(
                    dispatcher,
                    &scratch_handles[..scratch_count],
                    "resident adaptive traversal scratch",
                );
            }
            return Ok(());
        };
        self.frontier_bytes = 0;
        let mut all_handles = [0_u64; 8];
        all_handles[..3].copy_from_slice(&handles);
        let mut handle_count = 3;
        if let Some(queue_handle) = self.queue_handle.take() {
            all_handles[handle_count] = queue_handle;
            handle_count += 1;
            self.queue_bytes = 0;
        }
        if let Some(high_queue_handle) = self.high_queue_handle.take() {
            all_handles[handle_count] = high_queue_handle;
            handle_count += 1;
            self.high_queue_bytes = 0;
        }
        if let Some(high_len_handle) = self.high_len_handle.take() {
            all_handles[handle_count] = high_len_handle;
            handle_count += 1;
        }
        if let Some(word_partials_handle) = self.word_partials_handle.take() {
            all_handles[handle_count] = word_partials_handle;
            handle_count += 1;
            self.word_partials_bytes = 0;
        }
        if let Some(word_block_totals_handle) = self.word_block_totals_handle.take() {
            all_handles[handle_count] = word_block_totals_handle;
            handle_count += 1;
            self.word_block_totals_bytes = 0;
        }
        free_unique_resident_handles(
            dispatcher,
            &all_handles[..handle_count],
            "resident adaptive traversal scratch",
        )
    }
}

/// Adaptive traversal plan-cache counters.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AdaptiveTraversalPlanCacheSnapshot {
    /// Number of cached Programs.
    pub entries: usize,
    /// Number of lookups served from cache.
    pub hits: u64,
    /// Number of Programs built and inserted.
    pub misses: u64,
}

pub(super) type AdaptiveTraversalPlanCache = GraphPlanCache<AdaptiveTraversalPlanCacheKey>;
