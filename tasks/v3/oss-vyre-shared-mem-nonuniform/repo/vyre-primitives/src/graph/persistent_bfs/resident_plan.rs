use super::hash::persistent_bfs_program_layout_hash;
use super::layout::{
    persistent_bfs_batch_dispatch_grid, persistent_bfs_single_dispatch_grid,
    PersistentBfsBatchLayout, PersistentBfsFrontierLayout, PersistentBfsPlanCacheKey,
    PersistentBfsPlanCacheKind,
};
use super::program::{persistent_bfs, persistent_bfs_batch};
use crate::graph::program_graph::ProgramGraphShape;
use vyre_foundation::ir::Program;

/// Primitive-owned resident single-frontier persistent-BFS dispatch plan.
pub struct PersistentBfsResidentDispatchPlan {
    pub(super) frontier_layout: PersistentBfsFrontierLayout,
    pub(super) node_count: u32,
    pub(super) edge_count: u32,
    pub(super) allow_mask: u32,
    pub(super) max_iters: u32,
}

impl PersistentBfsResidentDispatchPlan {
    pub(super) fn new(
        frontier_layout: PersistentBfsFrontierLayout,
        node_count: u32,
        edge_count: u32,
        allow_mask: u32,
        max_iters: u32,
    ) -> Self {
        Self {
            frontier_layout,
            node_count,
            edge_count,
            allow_mask,
            max_iters,
        }
    }

    /// Validated resident frontier layout.
    #[must_use]
    pub const fn frontier_layout(&self) -> PersistentBfsFrontierLayout {
        self.frontier_layout
    }

    /// Number of words in the frontier bitset.
    #[must_use]
    pub const fn frontier_words(&self) -> usize {
        self.frontier_layout.words
    }

    /// Number of frontier words narrowed for cache keys.
    #[must_use]
    pub const fn words_u32(&self) -> u32 {
        self.frontier_layout.words_u32
    }

    /// Single-query dispatch grid.
    #[must_use]
    pub const fn dispatch_grid(&self) -> [u32; 3] {
        persistent_bfs_single_dispatch_grid(self.node_count)
    }

    /// Program graph shape with primitive-owned empty-edge padding.
    #[must_use]
    pub fn program_shape(&self) -> ProgramGraphShape {
        ProgramGraphShape::new(self.node_count, self.edge_count.max(1))
    }

    /// Build the canonical primitive program for this resident plan.
    #[must_use]
    pub fn program(&self, frontier_in: &str, frontier_out: &str) -> Program {
        persistent_bfs(
            self.program_shape(),
            frontier_in,
            frontier_out,
            self.allow_mask,
            self.max_iters,
        )
    }

    /// Build the primitive-owned program-cache key for this resident dispatch plan.
    #[must_use]
    pub const fn cache_key(
        &self,
        layout_hash: u64,
        device_features: u64,
    ) -> PersistentBfsPlanCacheKey {
        PersistentBfsPlanCacheKey {
            layout_hash,
            node_count: self.node_count,
            edge_count: self.edge_count,
            words_per_query: self.frontier_layout.words_u32,
            query_count: 1,
            allow_mask: self.allow_mask,
            max_iters: self.max_iters,
            device_features,
            kind: PersistentBfsPlanCacheKind::Single,
        }
    }

    /// Build a shape-only program cache key for this resident dispatch plan.
    #[must_use]
    pub fn program_cache_key(&self, device_features: u64) -> PersistentBfsPlanCacheKey {
        PersistentBfsPlanCacheKey {
            layout_hash: persistent_bfs_program_layout_hash(
                self.node_count,
                self.edge_count,
                self.frontier_layout.words_u32,
                1,
                PersistentBfsPlanCacheKind::Single,
            ),
            node_count: self.node_count,
            edge_count: self.edge_count,
            words_per_query: self.frontier_layout.words_u32,
            query_count: 1,
            allow_mask: self.allow_mask,
            max_iters: self.max_iters,
            device_features,
            kind: PersistentBfsPlanCacheKind::Single,
        }
    }
}

/// Primitive-owned resident batched persistent-BFS dispatch plan.
pub struct PersistentBfsResidentBatchDispatchPlan {
    pub(super) batch_layout: PersistentBfsBatchLayout,
    pub(super) node_count: u32,
    pub(super) edge_count: u32,
    pub(super) words_per_query: u32,
    pub(super) allow_mask: u32,
    pub(super) max_iters: u32,
}

impl PersistentBfsResidentBatchDispatchPlan {
    pub(super) fn new(
        batch_layout: PersistentBfsBatchLayout,
        node_count: u32,
        edge_count: u32,
        words_per_query: u32,
        allow_mask: u32,
        max_iters: u32,
    ) -> Self {
        Self {
            batch_layout,
            node_count,
            edge_count,
            words_per_query,
            allow_mask,
            max_iters,
        }
    }

    /// Validated flat-frontier batch layout.
    #[must_use]
    pub const fn batch_layout(&self) -> PersistentBfsBatchLayout {
        self.batch_layout
    }

    /// Query count as `usize` for host buffers.
    #[must_use]
    pub const fn query_count(&self) -> usize {
        self.batch_layout.query_count as usize
    }

    /// Query count narrowed for GPU grid dimensions and cache keys.
    #[must_use]
    pub const fn query_count_u32(&self) -> u32 {
        self.batch_layout.query_count
    }

    /// Total flat frontier words across every query.
    #[must_use]
    pub const fn total_words(&self) -> usize {
        self.batch_layout.total_words
    }

    /// Batch dispatch grid.
    #[must_use]
    pub const fn dispatch_grid(&self) -> [u32; 3] {
        persistent_bfs_batch_dispatch_grid(self.node_count, self.batch_layout.query_count)
    }

    /// Program graph shape with primitive-owned empty-edge padding.
    #[must_use]
    pub fn program_shape(&self) -> ProgramGraphShape {
        ProgramGraphShape::new(self.node_count, self.edge_count.max(1))
    }

    /// Build the canonical primitive batch program for this resident plan.
    #[must_use]
    pub fn program(&self, frontier_in: &str, frontier_out: &str, changed: &str) -> Program {
        persistent_bfs_batch(
            self.program_shape(),
            frontier_in,
            frontier_out,
            changed,
            self.batch_layout.query_count,
            self.allow_mask,
            self.max_iters,
        )
    }

    /// Number of words per query narrowed for cache keys.
    #[must_use]
    pub const fn words_per_query(&self) -> u32 {
        self.words_per_query
    }

    /// Build the primitive-owned program-cache key for this resident batch plan.
    #[must_use]
    pub const fn cache_key(
        &self,
        layout_hash: u64,
        device_features: u64,
    ) -> PersistentBfsPlanCacheKey {
        PersistentBfsPlanCacheKey {
            layout_hash,
            node_count: self.node_count,
            edge_count: self.edge_count,
            words_per_query: self.words_per_query,
            query_count: self.batch_layout.query_count,
            allow_mask: self.allow_mask,
            max_iters: self.max_iters,
            device_features,
            kind: PersistentBfsPlanCacheKind::Batch,
        }
    }

    /// Build a shape-only program cache key for this resident batch plan.
    #[must_use]
    pub fn program_cache_key(&self, device_features: u64) -> PersistentBfsPlanCacheKey {
        PersistentBfsPlanCacheKey {
            layout_hash: persistent_bfs_program_layout_hash(
                self.node_count,
                self.edge_count,
                self.words_per_query,
                self.batch_layout.query_count,
                PersistentBfsPlanCacheKind::Batch,
            ),
            node_count: self.node_count,
            edge_count: self.edge_count,
            words_per_query: self.words_per_query,
            query_count: self.batch_layout.query_count,
            allow_mask: self.allow_mask,
            max_iters: self.max_iters,
            device_features,
            kind: PersistentBfsPlanCacheKind::Batch,
        }
    }
}
