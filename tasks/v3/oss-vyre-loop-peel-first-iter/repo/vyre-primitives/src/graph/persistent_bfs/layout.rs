use crate::graph::program_graph::BINDING_PRIMITIVE_START;

/// Canonical op id.
pub const OP_ID: &str = "vyre-primitives::graph::persistent_bfs";
/// Canonical op id for batched persistent BFS over many seed frontiers.
pub const BATCH_OP_ID: &str = "vyre-primitives::graph::persistent_bfs_batch";

/// Canonical binding index for the input frontier bitset.
pub const BINDING_FRONTIER_IN: u32 = BINDING_PRIMITIVE_START;
/// Canonical binding index for the output frontier bitset.
pub const BINDING_FRONTIER_OUT: u32 = BINDING_PRIMITIVE_START + 1;
/// Canonical binding index for the global changed flag.
pub const BINDING_CHANGED: u32 = BINDING_PRIMITIVE_START + 2;
/// Canonical workgroup size for persistent BFS programs.
pub const PERSISTENT_BFS_WORKGROUP_SIZE: [u32; 3] = [256, 1, 1];
/// One-block dispatch grid used by the compact single-workgroup BFS path.
pub(crate) const PERSISTENT_BFS_SINGLE_DISPATCH_GRID: [u32; 3] = [1, 1, 1];

/// Dispatch grid for a single persistent-BFS query.
#[must_use]
pub const fn persistent_bfs_single_dispatch_grid(node_count: u32) -> [u32; 3] {
    [persistent_bfs_grid_x(node_count), 1, 1]
}

/// Dispatch grid for a batched persistent-BFS query set.
#[must_use]
pub const fn persistent_bfs_batch_dispatch_grid(node_count: u32, query_count: u32) -> [u32; 3] {
    if query_count == 0 {
        [1, 1, 1]
    } else {
        [persistent_bfs_grid_x(node_count), query_count, 1]
    }
}

const fn persistent_bfs_grid_x(node_count: u32) -> u32 {
    if node_count == 0 {
        1
    } else {
        ((node_count - 1) / PERSISTENT_BFS_WORKGROUP_SIZE[0]) + 1
    }
}

/// Validated persistent-BFS graph layout metadata.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PersistentBfsLayout {
    /// Number of graph nodes accepted by the primitive.
    pub node_count: u32,
    /// Number of logical CSR edges.
    pub edge_count: u32,
    /// Number of u32 words in one frontier bitset.
    pub words: usize,
    /// Number of u32 words in one frontier bitset, narrowed for cache keys.
    pub words_u32: u32,
    /// Number of u32 words required by node-indexed scratch buffers.
    pub node_words: usize,
    /// Number of u32 words required by physical edge buffers after padding.
    pub edge_storage_words: usize,
}

/// Validated flat-frontier batch metadata for persistent BFS.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PersistentBfsBatchLayout {
    /// Number of queries in the batch, narrowed for GPU grid dimensions.
    pub query_count: u32,
    /// Total number of u32 words in the flat `[query][word]` frontier array.
    pub total_words: usize,
}

/// Validated single-frontier metadata for resident persistent BFS.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PersistentBfsFrontierLayout {
    /// Number of u32 words in the frontier bitset.
    pub words: usize,
    /// Number of u32 words in the frontier bitset, narrowed for primitive metadata.
    pub words_u32: u32,
}

/// Primitive program-cache class for persistent-BFS dispatch plans.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PersistentBfsPlanCacheKind {
    /// One seed frontier for one graph.
    Single,
    /// Many seed frontiers batched over one graph.
    Batch,
}

/// Primitive-owned persistent-BFS program cache key.
///
/// Dispatch wrappers add only backend feature bits; graph identity, frontier
/// width, query count, masks, iteration budget, and plan class are owned here
/// so every backend caches the same primitive program shapes.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct PersistentBfsPlanCacheKey {
    /// Stable discriminator for the cached program layout.
    ///
    /// Content-addressed graph staging should use [`persistent_bfs_layout_hash`].
    /// Program caches should prefer [`persistent_bfs_program_layout_hash`] so
    /// same-shape CSR contents reuse the same compiled persistent-BFS program.
    pub layout_hash: u64,
    /// Number of graph nodes in the primitive program shape.
    pub node_count: u32,
    /// Number of logical graph edges in the primitive program shape.
    pub edge_count: u32,
    /// Number of frontier words per query.
    pub words_per_query: u32,
    /// Number of queries represented by the program.
    pub query_count: u32,
    /// Edge-kind allow mask compiled into the primitive program.
    pub allow_mask: u32,
    /// Iteration budget compiled into the primitive program.
    pub max_iters: u32,
    /// Backend/device feature key supplied by the dispatch wrapper.
    pub device_features: u64,
    /// Single-query or batched-query plan kind.
    pub kind: PersistentBfsPlanCacheKind,
}

/// Primitive-owned identity for immutable non-resident persistent-BFS inputs.
///
/// Dynamic frontier input/output and changed buffers are intentionally omitted:
/// dispatch wrappers refresh those every call. This key covers graph contents
/// and shape that decide when static CSR/device inputs must be refreshed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PersistentBfsStaticInputKey {
    /// Stable graph-content hash from [`persistent_bfs_layout_hash`].
    pub layout_hash: u64,
    /// Number of graph nodes.
    pub node_count: u32,
    /// Number of logical CSR edges.
    pub edge_count: u32,
    /// Number of frontier words.
    pub words: u32,
}
