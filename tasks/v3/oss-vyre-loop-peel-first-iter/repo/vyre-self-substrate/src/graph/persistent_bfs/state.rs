use crate::graph::plan_cache::GraphPlanCache;
use crate::graph::resident_handles::free_unique_resident_handles;
use crate::optimizer::dispatcher::{DispatchError, OptimizerDispatcher};
use vyre_primitives::graph::persistent_bfs::{
    copy_persistent_bfs_batch_seed_and_clear_changed_into, copy_persistent_bfs_seed_frontier_into,
    PersistentBfsPlanCacheKey, PersistentBfsStaticInputKey,
};

/// Caller-owned GPU dispatch scratch for persistent BFS expansion.
#[derive(Debug, Default)]
pub struct PersistentBfsGpuScratch {
    pub(super) inputs: Vec<Vec<u8>>,
    pub(super) changed: Vec<u32>,
    pub(super) static_input_key: Option<PersistentBfsStaticInputKey>,
    pub(super) plan_cache: PersistentBfsPlanCache,
}

/// Device-resident CSR graph for repeated persistent-BFS/dataflow queries.
#[derive(Debug, Clone)]
pub struct ResidentBfsGraph {
    pub(super) node_count: u32,
    pub(super) edge_count: u32,
    pub(super) words: usize,
    pub(super) words_u32: u32,
    pub(super) layout_hash: u64,
    pub(super) handles: [u64; 5],
}

impl ResidentBfsGraph {
    /// Number of graph nodes represented by this resident CSR.
    #[must_use]
    pub fn node_count(&self) -> u32 {
        self.node_count
    }

    /// Number of logical CSR edges represented by this resident CSR.
    #[must_use]
    pub fn edge_count(&self) -> u32 {
        self.edge_count
    }

    /// Number of u32 words in each frontier bitset.
    #[must_use]
    pub fn words(&self) -> usize {
        self.words
    }

    /// Stable in-session hash of the CSR graph layout and edge masks.
    #[must_use]
    pub fn layout_hash(&self) -> u64 {
        self.layout_hash
    }

    /// Resident handles in ProgramGraph buffer order:
    /// nodes, edge_offsets, edge_targets, edge_kind_mask, node_tags.
    #[must_use]
    pub fn handles(&self) -> [u64; 5] {
        self.handles
    }

    /// Free the resident graph buffers.
    ///
    /// # Errors
    ///
    /// Returns the first backend free failure, after attempting every handle.
    pub fn free(self, dispatcher: &dyn OptimizerDispatcher) -> Result<(), DispatchError> {
        free_unique_resident_handles(dispatcher, &self.handles, "resident BFS graph")
    }
}

/// Caller-owned resident scratch for repeated BFS queries over a resident graph.
#[derive(Debug, Default)]
pub struct PersistentBfsResidentScratch {
    pub(super) frontier_handles: Option<[u64; 3]>,
    pub(super) frontier_bytes: usize,
    pub(super) changed_bytes: usize,
    pub(super) frontier_in_bytes: Vec<u8>,
    pub(super) readbacks: Vec<Vec<u8>>,
    pub(super) changed: Vec<u32>,
    pub(super) plan_cache: PersistentBfsPlanCache,
}

impl PersistentBfsResidentScratch {
    /// Snapshot plan-cache counters for residency and repeated-query tests.
    #[must_use]
    pub fn plan_cache_snapshot(&self) -> PersistentBfsPlanCacheSnapshot {
        let snapshot = self.plan_cache.snapshot();
        PersistentBfsPlanCacheSnapshot {
            entries: snapshot.entries,
            hits: snapshot.hits,
            misses: snapshot.misses,
        }
    }

    /// Free resident frontier/change buffers owned by this scratch object.
    ///
    /// # Errors
    ///
    /// Returns the first backend free failure, after attempting every handle.
    pub fn free(&mut self, dispatcher: &dyn OptimizerDispatcher) -> Result<(), DispatchError> {
        let Some(handles) = self.frontier_handles.take() else {
            return Ok(());
        };
        self.frontier_bytes = 0;
        self.changed_bytes = 0;
        free_unique_resident_handles(dispatcher, &handles, "resident BFS scratch")
    }
}

/// Persistent BFS plan-cache counters.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PersistentBfsPlanCacheSnapshot {
    /// Number of cached resident/non-resident BFS plans.
    pub entries: usize,
    /// Number of lookups served from the cache.
    pub hits: u64,
    /// Number of Programs built and inserted a new plan.
    pub misses: u64,
}

pub(super) type PersistentBfsPlanCache = GraphPlanCache<PersistentBfsPlanCacheKey>;

pub(super) fn copy_frontier_seed_into(
    frontier_out: &mut Vec<u32>,
    frontier_in: &[u32],
    context: &'static str,
) -> Result<(), DispatchError> {
    copy_persistent_bfs_seed_frontier_into(
        frontier_out,
        frontier_in,
        context,
        DispatchError::BackendError,
    )
}

pub(super) fn copy_frontier_batch_seed_and_clear_changed(
    frontier_outputs: &mut Vec<u32>,
    frontier_inputs: &[u32],
    changed_outputs: &mut Vec<u32>,
    query_count: usize,
    context: &'static str,
) -> Result<(), DispatchError> {
    copy_persistent_bfs_batch_seed_and_clear_changed_into(
        frontier_outputs,
        frontier_inputs,
        changed_outputs,
        query_count,
        context,
        DispatchError::BackendError,
    )
}
