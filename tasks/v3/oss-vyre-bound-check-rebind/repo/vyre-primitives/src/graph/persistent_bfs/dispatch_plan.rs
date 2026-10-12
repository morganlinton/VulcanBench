use super::hash::persistent_bfs_program_layout_hash;
use super::layout::{
    persistent_bfs_single_dispatch_grid, PersistentBfsLayout, PersistentBfsPlanCacheKey,
    PersistentBfsPlanCacheKind, PersistentBfsStaticInputKey,
};
use super::program::persistent_bfs;
use crate::graph::program_graph::ProgramGraphShape;
use vyre_foundation::ir::Program;

/// Primitive-owned non-resident persistent-BFS dispatch plan.
pub struct PersistentBfsDispatchPlan {
    pub(super) layout: PersistentBfsLayout,
    pub(super) layout_hash: u64,
    pub(super) allow_mask: u32,
    pub(super) max_iters: u32,
}

impl PersistentBfsDispatchPlan {
    pub(super) fn new(
        layout: PersistentBfsLayout,
        layout_hash: u64,
        allow_mask: u32,
        max_iters: u32,
    ) -> Self {
        Self {
            layout,
            layout_hash,
            allow_mask,
            max_iters,
        }
    }

    /// Validated graph/frontier layout.
    #[must_use]
    pub const fn layout(&self) -> PersistentBfsLayout {
        self.layout
    }

    /// Stable graph-layout hash for plan caches.
    #[must_use]
    pub const fn layout_hash(&self) -> u64 {
        self.layout_hash
    }

    /// Number of words in each frontier bitset.
    #[must_use]
    pub const fn frontier_words(&self) -> usize {
        self.layout.words
    }

    /// Number of u32 words required by node-indexed scratch buffers.
    #[must_use]
    pub const fn node_words(&self) -> usize {
        self.layout.node_words
    }

    /// Number of u32 words required by edge-indexed buffers after zero padding.
    #[must_use]
    pub const fn edge_storage_words(&self) -> usize {
        self.layout.edge_storage_words
    }

    /// Single-query dispatch grid.
    #[must_use]
    pub const fn dispatch_grid(&self) -> [u32; 3] {
        persistent_bfs_single_dispatch_grid(self.layout.node_count)
    }

    /// Program graph shape with primitive-owned empty-edge padding.
    #[must_use]
    pub fn program_shape(&self) -> ProgramGraphShape {
        ProgramGraphShape::new(self.layout.node_count, self.layout.edge_count.max(1))
    }

    /// Build the canonical primitive program for this plan.
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

    /// Build the primitive-owned program-cache key for this dispatch plan.
    #[must_use]
    pub const fn cache_key(&self, device_features: u64) -> PersistentBfsPlanCacheKey {
        PersistentBfsPlanCacheKey {
            layout_hash: self.layout_hash,
            node_count: self.layout.node_count,
            edge_count: self.layout.edge_count,
            words_per_query: self.layout.words_u32,
            query_count: 1,
            allow_mask: self.allow_mask,
            max_iters: self.max_iters,
            device_features,
            kind: PersistentBfsPlanCacheKind::Single,
        }
    }

    /// Build a shape-only program cache key for this non-resident dispatch plan.
    ///
    /// The persistent-BFS program depends on graph dimensions, frontier width,
    /// traversal options, dispatch kind, and device features. CSR edge contents
    /// are dispatch inputs, not shader source, so they must not fragment the
    /// compiled-program cache.
    #[must_use]
    pub fn program_cache_key(&self, device_features: u64) -> PersistentBfsPlanCacheKey {
        PersistentBfsPlanCacheKey {
            layout_hash: persistent_bfs_program_layout_hash(
                self.layout.node_count,
                self.layout.edge_count,
                self.layout.words_u32,
                1,
                PersistentBfsPlanCacheKind::Single,
            ),
            node_count: self.layout.node_count,
            edge_count: self.layout.edge_count,
            words_per_query: self.layout.words_u32,
            query_count: 1,
            allow_mask: self.allow_mask,
            max_iters: self.max_iters,
            device_features,
            kind: PersistentBfsPlanCacheKind::Single,
        }
    }

    /// Stable identity for immutable graph inputs associated with this plan.
    #[must_use]
    pub const fn static_input_key(&self) -> PersistentBfsStaticInputKey {
        PersistentBfsStaticInputKey {
            layout_hash: self.layout_hash,
            node_count: self.layout.node_count,
            edge_count: self.layout.edge_count,
            words: self.layout.words_u32,
        }
    }
}
