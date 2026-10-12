//! Adaptive CSR / dense-bitmatrix traversal (G4).
//!
//! # What this is
//!
//! `csr_forward_traverse` is ideal when the BFS frontier is sparse
//! (<~5% of nodes). When the frontier saturates, a dense-bitmatrix
//! step (adjacency × frontier) wins  -  each tile's adjacency bitrow
//! × its frontier bitset is one vectorised OR over a pair of 32-bit
//! words, with contiguous DRAM access patterns that outrun CSR.
//!
//! This module exposes both a dense step and a hybrid sparse/dense
//! step. The hybrid step consumes a device-resident frontier popcount
//! buffer, so a prior GPU reduction can select CSR or dense execution
//! without reading the frontier back to the CPU:
//!
//! ```text
//!   density_pct = 100 * popcount(frontier_in) / node_count
//!   if density_pct >= DENSE_THRESHOLD_PCT: dense step
//!   else: CSR step
//! ```
//!
//! The dense step is a bitmatrix multiply:
//!
//! ```text
//!   for dst in 0..node_count:
//!     if (adj_row[dst] & frontier_in) != 0:
//!       frontier_out[dst] = 1
//! ```
//!
//! where `adj_row[dst]` is a bitset over source-node predecessors
//! (reverse adjacency, encoded as one row of `bitset_words(node_count)`
//! u32s per destination node).
//!
//! # Buffers
//!
//! - `frontier_in`   -  ReadOnly, packed bitset, `bitset_words(n)` u32.
//! - `frontier_out`  -  ReadWrite, same shape.
//! - `frontier_popcount`  -  ReadOnly, one u32 set-bit count for
//!   device-side sparse/dense selection in the hybrid step.
//! - `edge_offsets`, `edge_targets`, `edge_kind_mask`  -  CSR graph
//!   buffers for sparse expansion in the hybrid step.
//! - `adj_rows_dense`  -  ReadOnly, `node_count × bitset_words(n)` u32.
//!   Row `d` is the bitset of predecessors of node `d`.

use std::hash::{Hash, Hasher};
use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

use crate::bitset::{
    bitset_words,
    four_russians::{
        dense_matvec_byte_lut, dense_matvec_byte_lut_words, four_russians_dense_matvec_byte_lut,
        frontier_words_for_byte_tiles,
    },
    frontier::frontier_tail_mask,
};

/// Density threshold (percent). Tiles with ≥ this fraction of
/// frontier bits set use the dense-bitmatrix step; below it, CSR.
/// 25% is the empirical crossover on current desktop GPU architectures.
pub const DENSE_THRESHOLD_PCT: u32 = 25;

/// Canonical op id for the dense step.
pub const OP_ID: &str = "vyre-primitives::graph::adaptive_traverse_dense";
/// Canonical op id for the device-selected sparse/dense step.
pub const HYBRID_OP_ID: &str = "vyre-primitives::graph::adaptive_traverse_sparse_dense";
/// Canonical op id for graph-level dense Four-Russians traversal planning.
pub const FOUR_RUSSIANS_DENSE_OP_ID: &str =
    "vyre-primitives::graph::adaptive_traverse_four_russians_dense";

/// Canonical input-frontier buffer name.
pub const NAME_FRONTIER_IN: &str = "adap_frontier_in";
/// Canonical output-frontier buffer name.
pub const NAME_FRONTIER_OUT: &str = "adap_frontier_out";
/// Canonical frontier-popcount buffer name.
pub const NAME_FRONTIER_POPCOUNT: &str = "adap_frontier_popcount";
/// Canonical CSR row-offset buffer name.
pub const NAME_EDGE_OFFSETS: &str = "adap_edge_offsets";
/// Canonical CSR edge-target buffer name.
pub const NAME_EDGE_TARGETS: &str = "adap_edge_targets";
/// Canonical CSR edge-kind mask buffer name.
pub const NAME_EDGE_KIND_MASK: &str = "adap_edge_kind_mask";
/// Canonical dense adjacency-row buffer name.
pub const NAME_ADJ_ROWS_DENSE: &str = "adap_adj_rows_dense";

/// Runtime traversal strategy selected from frontier and graph statistics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdaptiveTraversalMode {
    /// Materialize active source nodes into a device queue, then consume only
    /// queued CSR rows. Best for low-density frontiers.
    SparseQueue,
    /// Let the GPU selector choose sparse CSR vs dense reverse-bitmatrix from
    /// a device-resident frontier popcount.
    SparseDense,
}

/// Dense-frontier kernel selected after the sparse/dense branch chooses dense.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DenseTraversalKernel {
    /// Scan one dense reverse-adjacency row per destination node.
    RowScanBitmatrix,
    /// Use byte-tile Four-Russians source-column LUTs.
    FourRussiansByteTile,
}

/// Primitive-owned resident adaptive traversal program identity.
///
/// Self-substrate and future CUDA/WGSL/SPIR-V dispatch layers use this as the
/// stable cache-key taxonomy instead of forking per-wrapper enums.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum AdaptiveTraversalProgramKind {
    /// Count set bits in the input frontier.
    Popcount,
    /// Clear the output frontier before an OR-writing traversal kernel.
    ClearFrontierOut,
    /// Initialize the active queue length before sparse queue compaction.
    QueueLenInit,
    /// Device-selected CSR/dense reverse-bitmatrix traversal.
    SparseDense,
    /// Compact active source ids from a frontier bitset into a queue.
    FrontierToQueue,
    /// Compute per-word active-node prefix counts for packed-frontier queues.
    FrontierWordCounts,
    /// Convert packed-frontier block totals into exclusive block offsets.
    FrontierWordBlockOffsets,
    /// Scatter packed frontier words into a deterministic active-source queue.
    FrontierWordPrefixQueue,
    /// Scatter packed frontier words using precomputed block offsets.
    FrontierWordBlockOffsetsQueue,
    /// Consume a compacted active-source queue through CSR rows.
    QueueForward,
    /// Consume a compacted active-source queue with lane teams for skewed rows.
    QueueForwardStrided,
    /// Expand low-degree queued rows and compact only high-degree rows.
    QueueSplitLow,
    /// Dense graph traversal through a reusable Four-Russians byte-tile LUT.
    FourRussiansDense,
}

/// Stable cache key for resident adaptive traversal Programs.
///
/// The key deliberately includes program layout identity, frontier width, queue
/// capacity, traversal masks, threshold policy, and backend feature bits so a
/// cached Program cannot be reused across incompatible CUDA/WGSL/SPIR-V shapes.
/// Resident graph contents are represented by dispatch handles, not shader
/// source, so same-shape resident graphs reuse compiled Programs.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct AdaptiveTraversalPlanCacheKey {
    /// Shape-only hash of the resident Program layout.
    pub layout_hash: u64,
    /// Number of graph nodes.
    pub node_count: u32,
    /// Number of logical CSR edges.
    pub edge_count: u32,
    /// Number of u32 words in one frontier bitset.
    pub words: u32,
    /// Active-source queue capacity for sparse-queue Programs.
    pub queue_capacity: u32,
    /// Allowed edge-kind mask baked into traversal Programs.
    pub allow_mask: u32,
    /// Dense cutover threshold baked into sparse/dense Programs.
    pub dense_threshold_pct: u32,
    /// Backend feature fingerprint from the dispatcher.
    pub device_features: u64,
    /// Resident Program shape represented by this key.
    pub kind: AdaptiveTraversalProgramKind,
}

impl AdaptiveTraversalPlanCacheKey {
    /// Construct a cache key for a resident adaptive traversal Program.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        layout_hash: u64,
        node_count: u32,
        edge_count: u32,
        words: u32,
        queue_capacity: u32,
        allow_mask: u32,
        dense_threshold_pct: u32,
        device_features: u64,
        kind: AdaptiveTraversalProgramKind,
    ) -> Self {
        Self {
            layout_hash,
            node_count,
            edge_count,
            words,
            queue_capacity,
            allow_mask,
            dense_threshold_pct,
            device_features,
            kind,
        }
    }

    /// Cache key for the frontier popcount Program.
    #[must_use]
    pub const fn popcount(
        _layout_hash: u64,
        node_count: u32,
        edge_count: u32,
        words: u32,
        device_features: u64,
    ) -> Self {
        let layout_hash = adaptive_traversal_program_layout_hash(
            node_count,
            edge_count,
            words,
            0,
            AdaptiveTraversalProgramKind::Popcount,
        );
        Self::new(
            layout_hash,
            node_count,
            edge_count,
            words,
            0,
            0,
            0,
            device_features,
            AdaptiveTraversalProgramKind::Popcount,
        )
    }

    /// Cache key for clearing the output frontier.
    #[must_use]
    pub const fn clear_frontier_out(
        _layout_hash: u64,
        node_count: u32,
        edge_count: u32,
        words: u32,
        device_features: u64,
    ) -> Self {
        let layout_hash = adaptive_traversal_program_layout_hash(
            node_count,
            edge_count,
            words,
            0,
            AdaptiveTraversalProgramKind::ClearFrontierOut,
        );
        Self::new(
            layout_hash,
            node_count,
            edge_count,
            words,
            0,
            0,
            0,
            device_features,
            AdaptiveTraversalProgramKind::ClearFrontierOut,
        )
    }

    /// Cache key for device-selected sparse/dense traversal.
    #[must_use]
    pub const fn sparse_dense(
        _layout_hash: u64,
        node_count: u32,
        edge_count: u32,
        words: u32,
        allow_mask: u32,
        dense_threshold_pct: u32,
        device_features: u64,
    ) -> Self {
        let layout_hash = adaptive_traversal_program_layout_hash(
            node_count,
            edge_count,
            words,
            0,
            AdaptiveTraversalProgramKind::SparseDense,
        );
        Self::new(
            layout_hash,
            node_count,
            edge_count,
            words,
            0,
            allow_mask,
            dense_threshold_pct,
            device_features,
            AdaptiveTraversalProgramKind::SparseDense,
        )
    }

    /// Cache key for the active-queue length initialization Program.
    #[must_use]
    pub const fn queue_len_init(
        _layout_hash: u64,
        node_count: u32,
        edge_count: u32,
        words: u32,
        queue_capacity: u32,
        device_features: u64,
    ) -> Self {
        let layout_hash = adaptive_traversal_program_layout_hash(
            node_count,
            edge_count,
            words,
            queue_capacity,
            AdaptiveTraversalProgramKind::QueueLenInit,
        );
        Self::new(
            layout_hash,
            node_count,
            edge_count,
            words,
            queue_capacity,
            0,
            0,
            device_features,
            AdaptiveTraversalProgramKind::QueueLenInit,
        )
    }

    /// Cache key for frontier-to-active-queue compaction.
    #[must_use]
    pub const fn frontier_to_queue(
        _layout_hash: u64,
        node_count: u32,
        edge_count: u32,
        words: u32,
        queue_capacity: u32,
        device_features: u64,
    ) -> Self {
        let layout_hash = adaptive_traversal_program_layout_hash(
            node_count,
            edge_count,
            words,
            queue_capacity,
            AdaptiveTraversalProgramKind::FrontierToQueue,
        );
        Self::new(
            layout_hash,
            node_count,
            edge_count,
            words,
            queue_capacity,
            0,
            0,
            device_features,
            AdaptiveTraversalProgramKind::FrontierToQueue,
        )
    }

    /// Cache key for packed-frontier word-count scan.
    #[must_use]
    pub const fn frontier_word_counts(
        _layout_hash: u64,
        node_count: u32,
        edge_count: u32,
        words: u32,
        device_features: u64,
    ) -> Self {
        let layout_hash = adaptive_traversal_program_layout_hash(
            node_count,
            edge_count,
            words,
            0,
            AdaptiveTraversalProgramKind::FrontierWordCounts,
        );
        Self::new(
            layout_hash,
            node_count,
            edge_count,
            words,
            0,
            0,
            0,
            device_features,
            AdaptiveTraversalProgramKind::FrontierWordCounts,
        )
    }

    /// Cache key for packed-frontier block-offset scan.
    #[must_use]
    pub const fn frontier_word_block_offsets(
        _layout_hash: u64,
        node_count: u32,
        edge_count: u32,
        words: u32,
        device_features: u64,
    ) -> Self {
        let layout_hash = adaptive_traversal_program_layout_hash(
            node_count,
            edge_count,
            words,
            0,
            AdaptiveTraversalProgramKind::FrontierWordBlockOffsets,
        );
        Self::new(
            layout_hash,
            node_count,
            edge_count,
            words,
            0,
            0,
            0,
            device_features,
            AdaptiveTraversalProgramKind::FrontierWordBlockOffsets,
        )
    }

    /// Cache key for deterministic packed-frontier queue scatter.
    #[must_use]
    pub const fn frontier_word_prefix_queue(
        _layout_hash: u64,
        node_count: u32,
        edge_count: u32,
        words: u32,
        queue_capacity: u32,
        device_features: u64,
    ) -> Self {
        let layout_hash = adaptive_traversal_program_layout_hash(
            node_count,
            edge_count,
            words,
            queue_capacity,
            AdaptiveTraversalProgramKind::FrontierWordPrefixQueue,
        );
        Self::new(
            layout_hash,
            node_count,
            edge_count,
            words,
            queue_capacity,
            0,
            0,
            device_features,
            AdaptiveTraversalProgramKind::FrontierWordPrefixQueue,
        )
    }

    /// Cache key for deterministic packed-frontier queue scatter with block offsets.
    #[must_use]
    pub const fn frontier_word_block_offsets_queue(
        _layout_hash: u64,
        node_count: u32,
        edge_count: u32,
        words: u32,
        queue_capacity: u32,
        device_features: u64,
    ) -> Self {
        let layout_hash = adaptive_traversal_program_layout_hash(
            node_count,
            edge_count,
            words,
            queue_capacity,
            AdaptiveTraversalProgramKind::FrontierWordBlockOffsetsQueue,
        );
        Self::new(
            layout_hash,
            node_count,
            edge_count,
            words,
            queue_capacity,
            0,
            0,
            device_features,
            AdaptiveTraversalProgramKind::FrontierWordBlockOffsetsQueue,
        )
    }

    /// Cache key for queue-driven CSR traversal.
    #[must_use]
    pub const fn queue_forward(
        _layout_hash: u64,
        node_count: u32,
        edge_count: u32,
        words: u32,
        queue_capacity: u32,
        allow_mask: u32,
        device_features: u64,
    ) -> Self {
        let layout_hash = adaptive_traversal_program_layout_hash(
            node_count,
            edge_count,
            words,
            queue_capacity,
            AdaptiveTraversalProgramKind::QueueForward,
        );
        Self::new(
            layout_hash,
            node_count,
            edge_count,
            words,
            queue_capacity,
            allow_mask,
            0,
            device_features,
            AdaptiveTraversalProgramKind::QueueForward,
        )
    }

    /// Cache key for row-strided queue-driven CSR traversal.
    #[must_use]
    pub const fn queue_forward_strided(
        _layout_hash: u64,
        node_count: u32,
        edge_count: u32,
        words: u32,
        queue_capacity: u32,
        allow_mask: u32,
        device_features: u64,
    ) -> Self {
        let layout_hash = adaptive_traversal_program_layout_hash(
            node_count,
            edge_count,
            words,
            queue_capacity,
            AdaptiveTraversalProgramKind::QueueForwardStrided,
        );
        Self::new(
            layout_hash,
            node_count,
            edge_count,
            words,
            queue_capacity,
            allow_mask,
            0,
            device_features,
            AdaptiveTraversalProgramKind::QueueForwardStrided,
        )
    }

    /// Cache key for the low-row half of mixed queue-driven CSR traversal.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub const fn queue_split_low(
        _layout_hash: u64,
        node_count: u32,
        edge_count: u32,
        words: u32,
        queue_capacity: u32,
        high_queue_capacity: u32,
        high_degree_threshold: u32,
        allow_mask: u32,
        device_features: u64,
    ) -> Self {
        let layout_hash = adaptive_traversal_split_program_layout_hash(
            node_count,
            edge_count,
            words,
            queue_capacity,
            high_queue_capacity,
            high_degree_threshold,
            AdaptiveTraversalProgramKind::QueueSplitLow,
        );
        Self::new(
            layout_hash,
            node_count,
            edge_count,
            words,
            queue_capacity,
            allow_mask,
            0,
            device_features,
            AdaptiveTraversalProgramKind::QueueSplitLow,
        )
    }

    /// Cache key for dense Four-Russians traversal through a resident LUT.
    #[must_use]
    pub const fn four_russians_dense(
        _layout_hash: u64,
        node_count: u32,
        words: u32,
        device_features: u64,
    ) -> Self {
        let layout_hash = adaptive_traversal_program_layout_hash(
            node_count,
            0,
            words,
            0,
            AdaptiveTraversalProgramKind::FourRussiansDense,
        );
        Self::new(
            layout_hash,
            node_count,
            0,
            words,
            0,
            0,
            0,
            device_features,
            AdaptiveTraversalProgramKind::FourRussiansDense,
        )
    }
}

const fn adaptive_traversal_program_kind_tag(kind: AdaptiveTraversalProgramKind) -> u64 {
    match kind {
        AdaptiveTraversalProgramKind::Popcount => 1,
        AdaptiveTraversalProgramKind::ClearFrontierOut => 2,
        AdaptiveTraversalProgramKind::SparseDense => 3,
        AdaptiveTraversalProgramKind::QueueLenInit => 4,
        AdaptiveTraversalProgramKind::FrontierToQueue => 5,
        AdaptiveTraversalProgramKind::QueueForward => 6,
        AdaptiveTraversalProgramKind::FourRussiansDense => 7,
        AdaptiveTraversalProgramKind::FrontierWordCounts => 8,
        AdaptiveTraversalProgramKind::FrontierWordPrefixQueue => 9,
        AdaptiveTraversalProgramKind::FrontierWordBlockOffsets => 10,
        AdaptiveTraversalProgramKind::FrontierWordBlockOffsetsQueue => 11,
        AdaptiveTraversalProgramKind::QueueForwardStrided => 12,
        AdaptiveTraversalProgramKind::QueueSplitLow => 13,
    }
}

const fn adaptive_traversal_hash_mix(hash: u64, value: u64) -> u64 {
    (hash ^ value).wrapping_mul(0x0000_0100_0000_01B3)
}

/// Shape-only hash for resident adaptive traversal program layouts.
///
/// This excludes resident graph contents and dense LUT source rows; those are
/// already bound through resident handles. Including content here fragments the
/// compiled-program cache without changing generated code.
#[must_use]
pub const fn adaptive_traversal_program_layout_hash(
    node_count: u32,
    edge_count: u32,
    words: u32,
    queue_capacity: u32,
    kind: AdaptiveTraversalProgramKind,
) -> u64 {
    let hash = adaptive_traversal_hash_mix(0xcbf2_9ce4_8422_2325, 0x4154_5241_5645_5253);
    let hash = adaptive_traversal_hash_mix(hash, node_count as u64);
    let hash = adaptive_traversal_hash_mix(hash, edge_count as u64);
    let hash = adaptive_traversal_hash_mix(hash, words as u64);
    let hash = adaptive_traversal_hash_mix(hash, queue_capacity as u64);
    adaptive_traversal_hash_mix(hash, adaptive_traversal_program_kind_tag(kind))
}

/// Shape-only hash for mixed queue traversal programs whose low-row half also
/// depends on high-row queue capacity and the high-degree threshold.
#[must_use]
pub const fn adaptive_traversal_split_program_layout_hash(
    node_count: u32,
    edge_count: u32,
    words: u32,
    queue_capacity: u32,
    high_queue_capacity: u32,
    high_degree_threshold: u32,
    kind: AdaptiveTraversalProgramKind,
) -> u64 {
    let hash =
        adaptive_traversal_program_layout_hash(node_count, edge_count, words, queue_capacity, kind);
    let hash = adaptive_traversal_hash_mix(hash, high_queue_capacity as u64);
    adaptive_traversal_hash_mix(hash, high_degree_threshold as u64)
}

/// In-session content hash for resident adaptive CSR+dense graph uploads.
///
/// This hashes graph contents, unlike [`adaptive_traversal_program_layout_hash`],
/// which intentionally hashes only generated-program shape. Resident upload
/// wrappers use this to identify uploaded graph layouts without forking the
/// primitive's graph identity contract.
#[must_use]
pub fn adaptive_traversal_graph_content_hash(
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    adj_rows_dense: &[u32],
) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    node_count.hash(&mut hasher);
    edge_offsets.hash(&mut hasher);
    edge_targets.hash(&mut hasher);
    edge_kind_mask.hash(&mut hasher);
    adj_rows_dense.hash(&mut hasher);
    hasher.finish()
}

/// In-session content hash for resident adaptive sparse-queue CSR uploads.
#[must_use]
pub fn adaptive_sparse_queue_graph_content_hash(
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    node_count.hash(&mut hasher);
    edge_offsets.hash(&mut hasher);
    edge_targets.hash(&mut hasher);
    edge_kind_mask.hash(&mut hasher);
    hasher.finish()
}

/// In-session content hash for resident adaptive Four-Russians dense LUT uploads.
#[must_use]

pub fn adaptive_four_russians_graph_content_hash(node_count: u32, adj_rows_dense: &[u32]) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    node_count.hash(&mut hasher);
    adj_rows_dense.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod resident_content_hash_tests {
    use super::*;

    #[test]
    fn graph_content_hash_tracks_csr_masks_and_dense_rows() {
        let offsets = [0, 1, 1];
        let targets = [1];
        let masks = [7];
        let dense = [0b10, 0];
        let baseline = adaptive_traversal_graph_content_hash(2, &offsets, &targets, &masks, &dense);
        let changed_mask =
            adaptive_traversal_graph_content_hash(2, &offsets, &targets, &[3], &dense);
        let changed_dense =
            adaptive_traversal_graph_content_hash(2, &offsets, &targets, &masks, &[0, 1]);

        assert_ne!(baseline, changed_mask);
        assert_ne!(baseline, changed_dense);
    }

    #[test]
    fn sparse_queue_content_hash_tracks_csr_without_dense_rows() {
        let offsets = [0, 1, 1];
        let targets = [1];
        let masks = [7];
        let baseline = adaptive_sparse_queue_graph_content_hash(2, &offsets, &targets, &masks);
        let changed_mask = adaptive_sparse_queue_graph_content_hash(2, &offsets, &targets, &[3]);
        let changed_target = adaptive_sparse_queue_graph_content_hash(2, &offsets, &[0], &masks);

        assert_ne!(baseline, changed_mask);
        assert_ne!(baseline, changed_target);
    }

    #[test]
    fn four_russians_content_hash_tracks_lut_source_rows() {
        let baseline = adaptive_four_russians_graph_content_hash(8, &[1, 0, 0, 0, 0, 0, 0, 0]);
        let changed = adaptive_four_russians_graph_content_hash(8, &[2, 0, 0, 0, 0, 0, 0, 0]);

        assert_ne!(baseline, changed);
    }
}

/// Validated adaptive traversal graph layout metadata.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdaptiveTraversalLayout {
    /// Number of logical CSR edges.
    pub edge_count: u32,
    /// Largest CSR row degree in the sparse graph.
    pub max_row_degree: u32,
    /// Number of u32 words required by physical edge buffers after padding.
    pub edge_storage_words: usize,
    /// Number of u32 words in one frontier bitset.
    pub words: usize,
    /// Number of u32 words in the dense reverse-adjacency matrix.
    pub dense_words: usize,
}

/// Validated frontier bitset shape for adaptive traversal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdaptiveFrontierLayout {
    /// Number of u32 words in one frontier bitset.
    pub words: usize,
    /// Number of u32 words in one frontier bitset, narrowed for primitive metadata.
    pub words_u32: u32,
}

/// Primitive-owned work classification for a validated adaptive frontier.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdaptiveFrontierWorkPlan {
    /// Validated frontier layout.
    pub layout: AdaptiveFrontierLayout,
    /// Whether any in-domain frontier bit is active.
    pub has_active_bits: bool,
}

/// In-domain frontier statistics for adaptive traversal planning.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdaptiveFrontierStats {
    /// Validated frontier layout.
    pub layout: AdaptiveFrontierLayout,
    /// Set bits at node ids `< node_count`, excluding padding in the tail word.
    pub popcount: u32,
    /// Packed words with at least one in-domain active bit.
    pub nonzero_words: usize,
}

/// Workgroup lane count used by resident linear adaptive traversal kernels.
pub const ADAPTIVE_TRAVERSAL_LINEAR_WORKGROUP_LANES: u32 = 256;
/// Workgroup shape for node- and word-linear adaptive traversal kernels.
pub const ADAPTIVE_TRAVERSAL_LINEAR_WORKGROUP_SIZE: [u32; 3] =
    [ADAPTIVE_TRAVERSAL_LINEAR_WORKGROUP_LANES, 1, 1];
/// Byte length of one resident u32 popcount scalar.
pub const ADAPTIVE_TRAVERSAL_POPCOUNT_BYTES: usize = std::mem::size_of::<u32>();

/// Primitive-owned resident frontier launch and scratch plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdaptiveResidentFrontierPlan {
    /// Validated frontier work classification.
    pub work: AdaptiveFrontierWorkPlan,
    /// Number of bytes in one frontier bitset.
    pub frontier_bytes: usize,
    /// Number of bytes in one resident popcount scalar.
    pub popcount_bytes: usize,
    /// Grid for kernels that process frontier words.
    pub frontier_word_grid: [u32; 3],
    /// Grid for kernels that process graph nodes.
    pub node_grid: [u32; 3],
}

/// Primitive-owned resident sparse-queue launch and scratch plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdaptiveResidentSparseQueuePlan {
    /// Shared frontier launch and scratch plan.
    pub frontier: AdaptiveResidentFrontierPlan,
    /// Packed frontier words with at least one in-domain active bit.
    pub frontier_nonzero_words: usize,
    /// Active-source queue capacity in u32 node ids.
    pub queue_capacity: u32,
    /// Number of bytes in the resident active-source queue.
    pub queue_bytes: usize,
    /// Grid for kernels that process the active-source queue.
    pub queue_grid: [u32; 3],
}

/// Primitive-owned auto-mode resident traversal plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdaptiveResidentAutoStepPlan {
    /// Shared frontier launch and scratch plan.
    pub frontier: AdaptiveResidentFrontierPlan,
    /// Host-visible frontier popcount used only for mode selection.
    pub frontier_popcount: u32,
    /// Selected traversal mode.
    pub mode: AdaptiveTraversalMode,
}

#[must_use]
fn dense_cutover_nodes(node_count: u32, threshold_pct: u32) -> u32 {
    if node_count == 0 {
        return u32::MAX;
    }
    let numerator = u64::from(node_count).saturating_mul(u64::from(threshold_pct));
    let cutover = numerator.div_ceil(100);
    cutover.min(u64::from(u32::MAX)) as u32
}

#[must_use]
fn should_use_dense_with_popcount(popcount: u32, node_count: u32, threshold_pct: u32) -> bool {
    if node_count == 0 {
        return false;
    }
    popcount >= dense_cutover_nodes(node_count, threshold_pct)
}

/// Host-side density probe. Returns `true` iff
/// `popcount(frontier_in) / node_count ≥ DENSE_THRESHOLD_PCT / 100`.
///
/// `frontier_in` is the packed bitset; `node_count` is the total
/// number of nodes (not necessarily a multiple of 32). Integer-only
/// comparison  -  no floating-point rounding surprises.
#[must_use]
pub fn should_use_dense(frontier_in: &[u32], node_count: u32) -> bool {
    if node_count == 0 {
        return false;
    }
    let expected_words = bitset_words(node_count) as usize;
    let final_word_mask = frontier_tail_mask(node_count);
    let popcount: u32 = frontier_in
        .iter()
        .take(expected_words)
        .enumerate()
        .map(|(index, &word)| {
            if index + 1 == expected_words {
                word & final_word_mask
            } else {
                word
            }
            .count_ones()
        })
        .sum();
    should_use_dense_with_popcount(popcount, node_count, DENSE_THRESHOLD_PCT)
}

/// Select an adaptive traversal mode from measured frontier/graph statistics.
///
/// The sparse queue path removes whole-graph lane waste, but pays an extra
/// queue zero/upload and one atomic append per active source. The sparse/dense
/// path is better once the frontier is broad enough that scanning node lanes is
/// not mostly empty or when graph average degree makes queue materialization
/// less decisive than dense row coalescing.
#[must_use]
pub fn select_adaptive_traversal_mode(
    node_count: u32,
    edge_count: u32,
    frontier_popcount: u32,
    dense_threshold_pct: u32,
) -> AdaptiveTraversalMode {
    if node_count == 0 || frontier_popcount == 0 {
        return AdaptiveTraversalMode::SparseQueue;
    }
    let frontier_bps = (u64::from(frontier_popcount) * 10_000) / u64::from(node_count);
    let dense_cutover_bps = u64::from(dense_threshold_pct).saturating_mul(100);
    if frontier_bps >= dense_cutover_bps {
        return AdaptiveTraversalMode::SparseDense;
    }
    let avg_degree_x100 = (u64::from(edge_count) * 100) / u64::from(node_count);
    if frontier_bps <= 625 || (frontier_bps <= 1_250 && avg_degree_x100 >= 400) {
        AdaptiveTraversalMode::SparseQueue
    } else {
        AdaptiveTraversalMode::SparseDense
    }
}

/// Select the dense traversal kernel after the sparse/dense cutover fires.
///
/// Four-Russians byte tiles amortize a larger LUT over repeated graph waves.
/// They are selected only when the frontier is dense, the graph is large
/// enough for row-scan waste to matter, and the caller expects to reuse the
/// precomputed tile LUT across at least two traversal steps.
#[must_use]
pub fn select_dense_traversal_kernel(
    node_count: u32,
    frontier_popcount: u32,
    expected_lut_reuse_steps: u32,
) -> DenseTraversalKernel {
    if node_count < 64 || frontier_popcount == 0 || expected_lut_reuse_steps < 2 {
        return DenseTraversalKernel::RowScanBitmatrix;
    }
    if should_use_dense_with_popcount(frontier_popcount, node_count, DENSE_THRESHOLD_PCT) {
        DenseTraversalKernel::FourRussiansByteTile
    } else {
        DenseTraversalKernel::RowScanBitmatrix
    }
}

/// Validate CSR plus dense reverse-adjacency rows for adaptive traversal.
///
/// # Errors
///
/// Returns an actionable diagnostic when the layout is empty, malformed,
/// exceeds u32 edge-count indexing, has non-monotonic offsets, contains
/// out-of-range CSR targets, or has the wrong dense matrix length.
pub fn validate_adaptive_traversal_layout(
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    adj_rows_dense: &[u32],
) -> Result<AdaptiveTraversalLayout, String> {
    if node_count == 0 {
        return Err("Fix: adaptive traversal requires node_count > 0.".to_string());
    }
    let expected_offsets = (node_count as usize).checked_add(1).ok_or_else(|| {
        format!(
            "Fix: adaptive traversal node_count + 1 overflows usize for node_count={node_count}."
        )
    })?;
    if edge_offsets.len() != expected_offsets {
        return Err(format!(
            "Fix: adaptive traversal expected {expected_offsets} CSR offsets for {node_count} nodes, got {}.",
            edge_offsets.len()
        ));
    }
    if edge_targets.len() != edge_kind_mask.len() {
        return Err(format!(
            "Fix: adaptive traversal target/mask length mismatch: {} targets, {} masks.",
            edge_targets.len(),
            edge_kind_mask.len()
        ));
    }
    let edge_count = u32::try_from(edge_targets.len()).map_err(|_| {
        format!(
            "Fix: adaptive traversal edge count {} exceeds u32 index space.",
            edge_targets.len()
        )
    })?;
    let final_offset = edge_offsets[expected_offsets - 1] as usize;
    if final_offset != edge_targets.len() {
        return Err(format!(
            "Fix: adaptive traversal final CSR offset {final_offset} must equal edge_count {}.",
            edge_targets.len()
        ));
    }
    let mut max_row_degree = 0u32;
    for (row, pair) in edge_offsets.windows(2).enumerate() {
        if pair[0] > pair[1] {
            return Err(format!(
                "Fix: adaptive traversal CSR offsets are non-monotonic at row {row}: {} > {}.",
                pair[0], pair[1]
            ));
        }
        max_row_degree = max_row_degree.max(pair[1] - pair[0]);
    }
    for (idx, &target) in edge_targets.iter().enumerate() {
        if target >= node_count {
            return Err(format!(
                "Fix: adaptive traversal CSR target[{idx}]={target} is outside node_count {node_count}."
            ));
        }
    }

    let words = bitset_words(node_count) as usize;
    let dense_words = (node_count as usize).checked_mul(words).ok_or_else(|| {
        format!(
            "Fix: adaptive traversal dense adjacency word count overflows usize for {node_count} nodes and {words} words."
        )
    })?;
    if adj_rows_dense.len() != dense_words {
        return Err(format!(
            "Fix: adaptive traversal expected {dense_words} dense adjacency words, got {}.",
            adj_rows_dense.len()
        ));
    }

    Ok(AdaptiveTraversalLayout {
        edge_count,
        max_row_degree,
        edge_storage_words: edge_targets.len().max(1),
        words,
        dense_words,
    })
}

/// Validate a packed frontier bitset for adaptive traversal.
///
/// # Errors
///
/// Returns an actionable diagnostic when `node_count` is zero or the frontier
/// slice length does not match `bitset_words(node_count)`.
pub fn validate_adaptive_frontier(
    node_count: u32,
    frontier_in: &[u32],
) -> Result<AdaptiveFrontierLayout, String> {
    if node_count == 0 {
        return Err("Fix: adaptive traversal frontier requires node_count > 0.".to_string());
    }
    let words_u32 = bitset_words(node_count);
    let words = words_u32 as usize;
    if frontier_in.len() != words {
        return Err(format!(
            "Fix: adaptive traversal frontier expected {words} word(s) for node_count={node_count}, got {}.",
            frontier_in.len()
        ));
    }
    Ok(AdaptiveFrontierLayout { words, words_u32 })
}

/// Validate and classify an adaptive traversal frontier.
///
/// The all-zero frontier is a primitive identity case: every adaptive
/// traversal variant produces an all-zero output and does not need a resident
/// popcount, queue compaction, dense traversal, or readback kernel.
///
/// # Errors
///
/// Returns the same frontier-shape diagnostics as [`validate_adaptive_frontier`].
pub fn plan_adaptive_frontier_work(
    node_count: u32,
    frontier_in: &[u32],
) -> Result<AdaptiveFrontierWorkPlan, String> {
    let stats =
        adaptive_frontier_stats(node_count, frontier_in, "adaptive traversal frontier work")?;
    Ok(AdaptiveFrontierWorkPlan {
        layout: stats.layout,
        has_active_bits: stats.popcount != 0,
    })
}

/// Checked physical-word popcount for an adaptive traversal frontier.
///
/// # Errors
///
/// Returns an actionable diagnostic if the frontier contains more set bits than
/// can be represented by the primitive's u32 resident popcount scalar.
pub fn adaptive_frontier_popcount(frontier_in: &[u32], context: &str) -> Result<u32, String> {
    let mut popcount = 0u32;
    for &word in frontier_in {
        popcount = popcount.checked_add(word.count_ones()).ok_or_else(|| {
            format!(
                "Fix: {context} frontier popcount exceeds u32::MAX for {} frontier words.",
                frontier_in.len()
            )
        })?;
    }
    Ok(popcount)
}

/// Checked in-domain popcount for an adaptive traversal frontier.
///
/// # Errors
///
/// Returns frontier-shape diagnostics or an actionable diagnostic if the
/// in-domain frontier contains more set bits than fit in a u32 scalar.
pub fn adaptive_frontier_popcount_in_domain(
    node_count: u32,
    frontier_in: &[u32],
    context: &str,
) -> Result<u32, String> {
    adaptive_frontier_stats(node_count, frontier_in, context).map(|stats| stats.popcount)
}

/// Validate and count only frontier bits whose node ids are in domain.
///
/// # Errors
///
/// Returns frontier-shape diagnostics or an actionable diagnostic if the
/// in-domain frontier contains more set bits than fit in a u32 scalar.
pub fn adaptive_frontier_stats(
    node_count: u32,
    frontier_in: &[u32],
    context: &str,
) -> Result<AdaptiveFrontierStats, String> {
    let layout = validate_adaptive_frontier(node_count, frontier_in)?;
    let final_word_mask = frontier_tail_mask(node_count);
    let mut popcount = 0u32;
    let mut nonzero_words = 0usize;
    for (index, &word) in frontier_in.iter().enumerate() {
        let in_domain_word = if index + 1 == layout.words {
            word & final_word_mask
        } else {
            word
        };
        if in_domain_word != 0 {
            nonzero_words += 1;
        }
        popcount = popcount
            .checked_add(in_domain_word.count_ones())
            .ok_or_else(|| {
                format!(
                    "Fix: {context} frontier popcount exceeds u32::MAX for {} frontier words.",
                    frontier_in.len()
                )
            })?;
    }
    Ok(AdaptiveFrontierStats {
        layout,
        popcount,
        nonzero_words,
    })
}

/// Validate and plan resident frontier scratch plus launch grids.
///
/// # Errors
///
/// Returns frontier-shape diagnostics or byte-size overflow diagnostics.
pub fn plan_adaptive_resident_frontier_step(
    node_count: u32,
    frontier_in: &[u32],
) -> Result<AdaptiveResidentFrontierPlan, String> {
    let work = plan_adaptive_frontier_work(node_count, frontier_in)?;
    adaptive_resident_frontier_plan_from_work(node_count, work)
}

/// Validate and plan a queue-driven resident traversal step.
///
/// # Errors
///
/// Returns frontier-shape diagnostics or queue/frontier byte-size overflow
/// diagnostics. The active queue is sized from the host-visible frontier
/// popcount and rounded to a power-of-two bucket so sparse frontiers do not pay
/// full-graph queue allocation or launch width.
pub fn plan_adaptive_resident_sparse_queue_step(
    node_count: u32,
    frontier_in: &[u32],
) -> Result<AdaptiveResidentSparseQueuePlan, String> {
    let stats = adaptive_frontier_stats(
        node_count,
        frontier_in,
        "adaptive resident sparse queue step",
    )?;
    let work = AdaptiveFrontierWorkPlan {
        layout: stats.layout,
        has_active_bits: stats.popcount != 0,
    };
    let frontier = adaptive_resident_frontier_plan_from_work(node_count, work)?;
    let queue_capacity = adaptive_sparse_queue_capacity(node_count, stats.popcount);
    let queue_bytes = adaptive_u32_byte_len(
        queue_capacity as usize,
        "adaptive traversal resident active-source queue",
    )?;
    Ok(AdaptiveResidentSparseQueuePlan {
        frontier,
        frontier_nonzero_words: stats.nonzero_words,
        queue_capacity,
        queue_bytes,
        queue_grid: adaptive_linear_grid(queue_capacity),
    })
}

fn adaptive_sparse_queue_capacity(node_count: u32, frontier_popcount: u32) -> u32 {
    let active = frontier_popcount.min(node_count).max(1);
    active
        .checked_next_power_of_two()
        .unwrap_or(u32::MAX)
        .min(node_count.max(1))
}

/// Validate, count, and select resident traversal mode in one primitive-owned plan.
///
/// # Errors
///
/// Returns frontier-shape diagnostics or byte-size overflow diagnostics.
pub fn plan_adaptive_resident_auto_step(
    node_count: u32,
    edge_count: u32,
    frontier_in: &[u32],
    dense_threshold_pct: u32,
) -> Result<AdaptiveResidentAutoStepPlan, String> {
    let stats = adaptive_frontier_stats(node_count, frontier_in, "adaptive resident auto step")?;
    let work = AdaptiveFrontierWorkPlan {
        layout: stats.layout,
        has_active_bits: stats.popcount != 0,
    };
    let frontier = adaptive_resident_frontier_plan_from_work(node_count, work)?;
    let mode =
        select_adaptive_traversal_mode(node_count, edge_count, stats.popcount, dense_threshold_pct);
    Ok(AdaptiveResidentAutoStepPlan {
        frontier,
        frontier_popcount: stats.popcount,
        mode,
    })
}

fn adaptive_resident_frontier_plan_from_work(
    node_count: u32,
    work: AdaptiveFrontierWorkPlan,
) -> Result<AdaptiveResidentFrontierPlan, String> {
    let frontier_bytes =
        adaptive_u32_byte_len(work.layout.words, "adaptive traversal resident frontier")?;
    let frontier_word_grid = adaptive_linear_grid(work.layout.words_u32);
    Ok(AdaptiveResidentFrontierPlan {
        work,
        frontier_bytes,
        popcount_bytes: ADAPTIVE_TRAVERSAL_POPCOUNT_BYTES,
        frontier_word_grid,
        node_grid: adaptive_node_dispatch_grid(node_count),
    })
}

fn adaptive_u32_byte_len(words: usize, context: &str) -> Result<usize, String> {
    words.checked_mul(std::mem::size_of::<u32>()).ok_or_else(|| {
        format!(
            "Fix: {context} byte length overflows usize for {words} u32 word(s). Shard the graph before resident dispatch."
        )
    })
}

const fn adaptive_linear_grid(items: u32) -> [u32; 3] {
    let groups = items.div_ceil(ADAPTIVE_TRAVERSAL_LINEAR_WORKGROUP_LANES);
    if groups == 0 {
        [1, 1, 1]
    } else {
        [groups, 1, 1]
    }
}

/// Dispatch grid for adaptive traversal kernels that process one node per lane.
#[must_use]
pub const fn adaptive_node_dispatch_grid(node_count: u32) -> [u32; 3] {
    adaptive_linear_grid(node_count)
}

/// Build the GPU Program for one dense step. Invocation `d`
/// computes `frontier_out[d] = any bit of (adj_rows[d] &
/// frontier_in) is set`.
#[must_use]
pub fn adaptive_dense_step(
    frontier_in: &str,
    frontier_out: &str,
    adj_rows_dense: &str,
    node_count: u32,
) -> Program {
    if node_count == 0 {
        return crate::invalid_output_program(
            OP_ID,
            frontier_out,
            DataType::U32,
            "Fix: adaptive_dense_step requires node_count > 0, got 0.".to_string(),
        );
    }
    let words = bitset_words(node_count);
    // PHASE7_GRAPH C1: the adjacency buffer size is `node_count *
    // words`. A u32 × u32 multiply wraps silently for non-trivial
    // inputs (e.g. node_count ≈ 400k, words ≈ 12.5k wraps past
    // u32::MAX), producing a tiny buffer and catastrophic OOB
    // reads/writes. Check in u64 first and refuse programs we
    // cannot represent faithfully.
    let Some(adj_count) = u64::from(node_count).checked_mul(u64::from(words)) else {
        return crate::invalid_output_program(
            OP_ID,
            frontier_out,
            DataType::U32,
            format!("Fix: adaptive_dense_step buffer size overflows u64 ({node_count} nodes x {words} words)."),
        );
    };
    if adj_count > u64::from(u32::MAX) {
        return crate::invalid_output_program(
            OP_ID,
            frontier_out,
            DataType::U32,
            format!("Fix: adaptive_dense_step buffer size {adj_count} exceeds u32::MAX ({node_count} nodes x {words} words). Partition the graph or use csr_forward_traverse."),
        );
    }
    let adj_count_u32 = adj_count as u32;
    let d = Expr::InvocationId { axis: 0 };

    let body: Vec<Node> = vec![
        Node::let_bind("row_start", Expr::mul(d.clone(), Expr::u32(words))),
        Node::let_bind("hit", Expr::u32(0)),
        Node::loop_for(
            "w",
            Expr::u32(0),
            Expr::u32(words),
            vec![Node::assign(
                "hit",
                Expr::bitor(
                    Expr::var("hit"),
                    Expr::bitand(
                        Expr::load(
                            adj_rows_dense,
                            Expr::add(Expr::var("row_start"), Expr::var("w")),
                        ),
                        Expr::load(frontier_in, Expr::var("w")),
                    ),
                ),
            )],
        ),
        Node::if_then(
            Expr::ne(Expr::var("hit"), Expr::u32(0)),
            vec![
                Node::let_bind("word_idx", Expr::shr(d.clone(), Expr::u32(5))),
                Node::let_bind(
                    "bit_mask",
                    Expr::shl(Expr::u32(1), Expr::bitand(d.clone(), Expr::u32(31))),
                ),
                Node::let_bind(
                    "_",
                    Expr::atomic_or(frontier_out, Expr::var("word_idx"), Expr::var("bit_mask")),
                ),
            ],
        ),
    ];

    Program::wrapped(
        vec![
            BufferDecl::storage(frontier_in, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(words),
            BufferDecl::storage(frontier_out, 1, BufferAccess::ReadWrite, DataType::U32)
                .with_count(words),
            BufferDecl::storage(adj_rows_dense, 2, BufferAccess::ReadOnly, DataType::U32)
                .with_count(adj_count_u32),
        ],
        ADAPTIVE_TRAVERSAL_LINEAR_WORKGROUP_SIZE,
        vec![Node::Region {
            generator: Ident::from(OP_ID),
            source_region: None,
            body: Arc::new(vec![Node::if_then(
                Expr::lt(d.clone(), Expr::u32(node_count)),
                body,
            )]),
        }],
    )
}

/// Source-byte tile count for Four-Russians dense graph traversal.
#[must_use]

pub const fn four_russians_source_tile_count(node_count: u32) -> u32 {
    node_count.div_ceil(8)
}

/// Frontier word count for graph-level Four-Russians dense traversal.
#[must_use]
pub const fn four_russians_frontier_words(node_count: u32) -> u32 {
    frontier_words_for_byte_tiles(four_russians_source_tile_count(node_count))
}

/// LUT word count for graph-level Four-Russians dense traversal.
#[must_use]
pub fn four_russians_dense_lut_words(node_count: u32) -> u32 {
    dense_matvec_byte_lut_words(
        four_russians_source_tile_count(node_count),
        bitset_words(node_count),
    )
}

/// Transpose dense reverse-adjacency rows into source-column words.
///
/// `adj_rows_dense[dst][src] == 1` becomes `columns[src][dst] == 1`,
/// grouped by 8-source byte tiles for Four-Russians LUT construction.
///
/// # Errors
///
/// Returns an actionable diagnostic when `node_count` is zero, the dense row
/// matrix has the wrong shape, or the derived column table overflows `usize`.
pub fn four_russians_dense_columns_from_adj_rows(
    node_count: u32,
    adj_rows_dense: &[u32],
) -> Result<Vec<u32>, String> {
    if node_count == 0 {
        return Err(
            "Fix: Four-Russians adaptive dense traversal requires node_count > 0.".to_string(),
        );
    }
    let words = bitset_words(node_count) as usize;
    let expected_rows = (node_count as usize).checked_mul(words).ok_or_else(|| {
        format!(
            "Fix: Four-Russians adaptive dense row count overflows usize for {node_count} nodes and {words} words."
        )
    })?;
    if adj_rows_dense.len() != expected_rows {
        return Err(format!(
            "Fix: Four-Russians adaptive dense traversal expected {expected_rows} row words for {node_count} nodes, got {}.",
            adj_rows_dense.len()
        ));
    }
    let tile_count = four_russians_source_tile_count(node_count) as usize;
    let column_count = tile_count
        .checked_mul(8)
        .and_then(|columns| columns.checked_mul(words))
        .ok_or_else(|| {
            format!(
                "Fix: Four-Russians adaptive dense column table overflows usize for {node_count} nodes and {words} destination words."
            )
        })?;
    let mut columns = vec![0u32; column_count];

    for dst in 0..node_count as usize {
        let row_start = dst * words;
        let dst_word = dst / 32;
        let dst_bit = 1u32 << (dst % 32);
        for src_word in 0..words {
            let mut word = adj_rows_dense[row_start + src_word];
            while word != 0 {
                let bit = word.trailing_zeros() as usize;
                let src = src_word * 32 + bit;
                if src < node_count as usize {
                    let source_column = (src / 8) * 8 + (src % 8);
                    let column_idx = source_column * words + dst_word;
                    columns[column_idx] |= dst_bit;
                }
                word &= word - 1;
            }
        }
    }

    Ok(columns)
}

/// Build a Four-Russians dense traversal LUT from dense reverse rows.
///
/// # Errors
///
/// Propagates dense-row validation failures from
/// [`four_russians_dense_columns_from_adj_rows`].
pub fn four_russians_dense_lut_from_adj_rows(
    node_count: u32,
    adj_rows_dense: &[u32],
) -> Result<Vec<u32>, String> {
    let columns = four_russians_dense_columns_from_adj_rows(node_count, adj_rows_dense)?;
    Ok(dense_matvec_byte_lut(
        &columns,
        four_russians_source_tile_count(node_count),
        bitset_words(node_count),
    ))
}

/// Build the graph-level Four-Russians dense traversal Program.
#[must_use]
pub fn adaptive_four_russians_dense_step(
    frontier_in: &str,
    tile_lut: &str,
    frontier_out: &str,
    node_count: u32,
) -> Program {
    if node_count == 0 {
        return crate::invalid_output_program(
            FOUR_RUSSIANS_DENSE_OP_ID,
            frontier_out,
            DataType::U32,
            "Fix: adaptive_four_russians_dense_step requires node_count > 0, got 0.".to_string(),
        );
    }
    four_russians_dense_matvec_byte_lut(
        frontier_in,
        tile_lut,
        frontier_out,
        four_russians_source_tile_count(node_count),
        bitset_words(node_count),
    )
}

/// Build the GPU Program for one adaptive sparse/dense step.
///
/// Each invocation uses the device-resident `frontier_popcount[0]` to choose
/// the path. Below `dense_threshold_pct`, invocation `src` expands the CSR row
/// for one active source node. At or above the threshold, invocation `dst`
/// scans the dense reverse-adjacency row for one destination node.
///
/// This is intentionally a single primitive contract: callers can keep
/// `frontier_in`, `frontier_popcount`, CSR buffers, dense rows, and
/// `frontier_out` resident across fixpoint iterations, eliminating the old
/// CPU branch/readback boundary from the release path.
#[must_use]
pub fn adaptive_sparse_dense_step(
    frontier_in: &str,
    frontier_out: &str,
    frontier_popcount: &str,
    edge_offsets: &str,
    edge_targets: &str,
    edge_kind_mask: &str,
    adj_rows_dense: &str,
    node_count: u32,
    edge_count: u32,
    allow_mask: u32,
    dense_threshold_pct: u32,
) -> Program {
    if node_count == 0 {
        return crate::invalid_output_program(
            HYBRID_OP_ID,
            frontier_out,
            DataType::U32,
            "Fix: adaptive_sparse_dense_step requires node_count > 0, got 0.".to_string(),
        );
    }

    let words = bitset_words(node_count);
    let Some(adj_count) = u64::from(node_count).checked_mul(u64::from(words)) else {
        return crate::invalid_output_program(
            HYBRID_OP_ID,
            frontier_out,
            DataType::U32,
            format!("Fix: adaptive_sparse_dense_step dense buffer size overflows u64 ({node_count} nodes x {words} words)."),
        );
    };
    if adj_count > u64::from(u32::MAX) {
        return crate::invalid_output_program(
            HYBRID_OP_ID,
            frontier_out,
            DataType::U32,
            format!("Fix: adaptive_sparse_dense_step dense buffer size {adj_count} exceeds u32::MAX ({node_count} nodes x {words} words). Partition the graph."),
        );
    }
    let Some(offset_count) = node_count.checked_add(1) else {
        return crate::invalid_output_program(
            HYBRID_OP_ID,
            frontier_out,
            DataType::U32,
            "Fix: adaptive_sparse_dense_step CSR offset count overflows u32. Partition the graph."
                .to_string(),
        );
    };
    let physical_edge_count = edge_count.max(1);

    let lane = Expr::InvocationId { axis: 0 };
    let dense_cutover = dense_cutover_nodes(node_count, dense_threshold_pct);
    let dense_body: Vec<Node> = vec![
        Node::let_bind("dense_row_start", Expr::mul(lane.clone(), Expr::u32(words))),
        Node::let_bind("dense_hit", Expr::u32(0)),
        Node::loop_for(
            "dense_w",
            Expr::u32(0),
            Expr::u32(words),
            vec![Node::assign(
                "dense_hit",
                Expr::bitor(
                    Expr::var("dense_hit"),
                    Expr::bitand(
                        Expr::load(
                            adj_rows_dense,
                            Expr::add(Expr::var("dense_row_start"), Expr::var("dense_w")),
                        ),
                        Expr::load(frontier_in, Expr::var("dense_w")),
                    ),
                ),
            )],
        ),
        Node::if_then(
            Expr::ne(Expr::var("dense_hit"), Expr::u32(0)),
            vec![
                Node::let_bind("dense_word_idx", Expr::shr(lane.clone(), Expr::u32(5))),
                Node::let_bind(
                    "dense_bit_mask",
                    Expr::shl(Expr::u32(1), Expr::bitand(lane.clone(), Expr::u32(31))),
                ),
                Node::let_bind(
                    "_dense_prev",
                    Expr::atomic_or(
                        frontier_out,
                        Expr::var("dense_word_idx"),
                        Expr::var("dense_bit_mask"),
                    ),
                ),
            ],
        ),
    ];

    let sparse_body: Vec<Node> = vec![
        Node::let_bind("sparse_word_idx", Expr::shr(lane.clone(), Expr::u32(5))),
        Node::let_bind(
            "sparse_bit_mask",
            Expr::shl(Expr::u32(1), Expr::bitand(lane.clone(), Expr::u32(31))),
        ),
        Node::let_bind(
            "sparse_src_word",
            Expr::load(frontier_in, Expr::var("sparse_word_idx")),
        ),
        Node::if_then(
            Expr::ne(
                Expr::bitand(Expr::var("sparse_src_word"), Expr::var("sparse_bit_mask")),
                Expr::u32(0),
            ),
            vec![
                Node::let_bind("sparse_edge_start", Expr::load(edge_offsets, lane.clone())),
                Node::let_bind(
                    "sparse_edge_end",
                    Expr::load(edge_offsets, Expr::add(lane.clone(), Expr::u32(1))),
                ),
                Node::loop_for(
                    "sparse_e",
                    Expr::var("sparse_edge_start"),
                    Expr::var("sparse_edge_end"),
                    vec![
                        Node::let_bind(
                            "sparse_kind_mask",
                            Expr::load(edge_kind_mask, Expr::var("sparse_e")),
                        ),
                        Node::if_then(
                            Expr::ne(
                                Expr::bitand(Expr::var("sparse_kind_mask"), Expr::u32(allow_mask)),
                                Expr::u32(0),
                            ),
                            vec![
                                Node::let_bind(
                                    "sparse_dst",
                                    Expr::load(edge_targets, Expr::var("sparse_e")),
                                ),
                                Node::if_then(
                                    Expr::lt(Expr::var("sparse_dst"), Expr::u32(node_count)),
                                    vec![
                                        Node::let_bind(
                                            "sparse_dst_word_idx",
                                            Expr::shr(Expr::var("sparse_dst"), Expr::u32(5)),
                                        ),
                                        Node::let_bind(
                                            "sparse_dst_bit",
                                            Expr::shl(
                                                Expr::u32(1),
                                                Expr::bitand(
                                                    Expr::var("sparse_dst"),
                                                    Expr::u32(31),
                                                ),
                                            ),
                                        ),
                                        Node::let_bind(
                                            "_sparse_prev",
                                            Expr::atomic_or(
                                                frontier_out,
                                                Expr::var("sparse_dst_word_idx"),
                                                Expr::var("sparse_dst_bit"),
                                            ),
                                        ),
                                    ],
                                ),
                            ],
                        ),
                    ],
                ),
            ],
        ),
    ];

    let body = vec![
        Node::let_bind(
            "frontier_popcount_total",
            Expr::load(frontier_popcount, Expr::u32(0)),
        ),
        Node::if_then_else(
            Expr::ge(
                Expr::var("frontier_popcount_total"),
                Expr::u32(dense_cutover),
            ),
            dense_body,
            sparse_body,
        ),
    ];

    Program::wrapped(
        vec![
            BufferDecl::storage(frontier_in, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(words),
            BufferDecl::storage(frontier_out, 1, BufferAccess::ReadWrite, DataType::U32)
                .with_count(words),
            BufferDecl::storage(frontier_popcount, 2, BufferAccess::ReadOnly, DataType::U32)
                .with_count(1),
            BufferDecl::storage(edge_offsets, 3, BufferAccess::ReadOnly, DataType::U32)
                .with_count(offset_count),
            BufferDecl::storage(edge_targets, 4, BufferAccess::ReadOnly, DataType::U32)
                .with_count(physical_edge_count),
            BufferDecl::storage(edge_kind_mask, 5, BufferAccess::ReadOnly, DataType::U32)
                .with_count(physical_edge_count),
            BufferDecl::storage(adj_rows_dense, 6, BufferAccess::ReadOnly, DataType::U32)
                .with_count(adj_count as u32),
        ],
        ADAPTIVE_TRAVERSAL_LINEAR_WORKGROUP_SIZE,
        vec![Node::Region {
            generator: Ident::from(HYBRID_OP_ID),
            source_region: None,
            body: Arc::new(vec![Node::if_then(
                Expr::lt(lane.clone(), Expr::u32(node_count)),
                body,
            )]),
        }],
    )
}

/// CPU reference for the dense step. `frontier_in` is a packed
/// bitset over `node_count` nodes; `adj_rows_dense` is the reverse
/// adjacency laid out as `node_count × bitset_words(node_count)`.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn cpu_dense_step(frontier_in: &[u32], adj_rows_dense: &[u32], node_count: u32) -> Vec<u32> {
    let words = bitset_words(node_count) as usize;

    let mut out = vec![0_u32; words];
    for d in 0..node_count as usize {
        let row_start = d * words;
        let mut hit: u32 = 0;
        for w in 0..words {
            let adj = adj_rows_dense.get(row_start + w).copied().unwrap_or(0);
            let frontier = frontier_in.get(w).copied().unwrap_or(0);
            hit |= adj & frontier;
        }
        if hit != 0 {
            out[d / 32] |= 1 << (d % 32);
        }
    }
    out
}

/// CPU reference for graph-level Four-Russians dense traversal.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn cpu_four_russians_dense_step(
    frontier_in: &[u32],
    adj_rows_dense: &[u32],
    node_count: u32,
) -> Result<Vec<u32>, String> {
    let lut = four_russians_dense_lut_from_adj_rows(node_count, adj_rows_dense)?;
    Ok(crate::bitset::four_russians::dense_matvec_cpu_ref(
        frontier_in,
        &lut,
        four_russians_source_tile_count(node_count),
        bitset_words(node_count),
    ))
}

/// CPU reference for the adaptive sparse/dense step.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn cpu_sparse_dense_step(
    frontier_in: &[u32],
    frontier_popcount: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    adj_rows_dense: &[u32],
    node_count: u32,
    allow_mask: u32,
    dense_threshold_pct: u32,
) -> Vec<u32> {
    if should_use_dense_with_popcount(frontier_popcount, node_count, dense_threshold_pct) {
        return cpu_dense_step(frontier_in, adj_rows_dense, node_count);
    }

    let words = bitset_words(node_count) as usize;
    let mut out = vec![0_u32; words];
    for src in 0..node_count as usize {
        let word_idx = src / 32;
        let bit_mask = 1_u32 << (src % 32);
        if frontier_in.get(word_idx).copied().unwrap_or(0) & bit_mask == 0 {
            continue;
        }
        let edge_start = edge_offsets.get(src).copied().unwrap_or(0) as usize;
        let edge_end = edge_offsets
            .get(src + 1)
            .copied()
            .unwrap_or(edge_start as u32) as usize;
        for e in edge_start..edge_end {
            if edge_kind_mask.get(e).copied().unwrap_or(0) & allow_mask == 0 {
                continue;
            }
            let Some(dst) = edge_targets.get(e).copied() else {
                continue;
            };
            if dst < node_count {
                out[dst as usize / 32] |= 1_u32 << (dst % 32);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pack_nodes(bits: &[u32], node_count: u32) -> Vec<u32> {
        let mut buf = vec![0_u32; bitset_words(node_count) as usize];
        for &b in bits {
            buf[(b as usize) / 32] |= 1 << (b % 32);
        }
        buf
    }

    fn build_dense_adj(edges: &[(u32, u32)], node_count: u32) -> Vec<u32> {
        let words = bitset_words(node_count) as usize;
        let mut rows = vec![0_u32; (node_count as usize) * words];
        for &(src, dst) in edges {
            let idx = (dst as usize) * words + (src as usize) / 32;
            rows[idx] |= 1 << (src % 32);
        }
        rows
    }

    #[test]
    fn should_use_dense_empty_frontier_is_false() {
        assert!(!should_use_dense(&[0_u32], 32));
    }

    #[test]
    fn should_use_dense_zero_nodes_returns_false() {
        assert!(!should_use_dense(&[], 0));
    }

    #[test]
    fn should_use_dense_full_frontier_is_true() {
        let f = vec![0xFFFF_FFFF_u32; 4];
        assert!(should_use_dense(&f, 128));
    }

    #[test]
    fn should_use_dense_quarter_frontier_at_threshold() {
        // 32 nodes, 8 bits set = 25% (exactly threshold).
        assert!(should_use_dense(&[0xFF_u32], 32));
    }

    #[test]
    fn should_use_dense_just_under_threshold_is_false() {
        // 32 nodes, 7 bits set = ~21%, below 25%.
        assert!(!should_use_dense(&[0x7F_u32], 32));
    }

    #[test]
    fn dense_cutover_rounds_up_without_u32_multiply_overflow() {
        assert_eq!(dense_cutover_nodes(32, 25), 8);
        assert_eq!(dense_cutover_nodes(33, 25), 9);
        assert_eq!(dense_cutover_nodes(u32::MAX, 100), u32::MAX);
    }

    #[test]
    fn cpu_dense_step_empty_frontier_produces_empty() {
        let frontier_in = pack_nodes(&[], 16);
        let adj = build_dense_adj(&[(0, 1), (1, 2)], 16);
        let out = cpu_dense_step(&frontier_in, &adj, 16);
        assert_eq!(out, vec![0; bitset_words(16) as usize]);
    }

    #[test]
    fn cpu_dense_step_single_edge() {
        let out = cpu_dense_step(&pack_nodes(&[0], 16), &build_dense_adj(&[(0, 1)], 16), 16);
        assert_eq!(out, pack_nodes(&[1], 16));
    }

    #[test]
    fn cpu_dense_step_fanout() {
        let out = cpu_dense_step(
            &pack_nodes(&[0], 16),
            &build_dense_adj(&[(0, 1), (0, 2), (0, 5)], 16),
            16,
        );
        assert_eq!(out, pack_nodes(&[1, 2, 5], 16));
    }

    #[test]
    fn cpu_dense_step_fanin() {
        let out = cpu_dense_step(
            &pack_nodes(&[1, 2], 16),
            &build_dense_adj(&[(1, 3), (2, 3), (4, 3)], 16),
            16,
        );
        assert_eq!(out, pack_nodes(&[3], 16));
    }

    #[test]
    fn cpu_dense_step_cross_word_boundary() {
        // 70 nodes → 3 words. Edge src=5 (word 0) → dst=65 (word 2).
        let out = cpu_dense_step(&pack_nodes(&[5], 70), &build_dense_adj(&[(5, 65)], 70), 70);
        assert_eq!(out, pack_nodes(&[65], 70));
    }

    #[test]
    fn cpu_dense_step_short_buffers_treat_missing_words_as_zero() {
        let out = cpu_dense_step(&[1], &[], 16);
        assert!(out.iter().all(|&word| word == 0));
    }

    #[test]
    fn cpu_dense_step_is_one_hop_only() {
        // Single invocation is one hop. 0 → 1 → 2 → 3; seeded with
        // {0} yields {1}, not the full closure.
        let out = cpu_dense_step(
            &pack_nodes(&[0], 16),
            &build_dense_adj(&[(0, 1), (1, 2), (2, 3)], 16),
            16,
        );
        assert_eq!(out, pack_nodes(&[1], 16));
    }

    #[test]
    fn emitted_program_has_expected_shape() {
        let p = adaptive_dense_step("fin", "fout", "adj", 64);
        assert_eq!(p.workgroup_size, ADAPTIVE_TRAVERSAL_LINEAR_WORKGROUP_SIZE);
        let names: Vec<&str> = p.buffers.iter().map(|b| b.name()).collect();
        assert_eq!(names, vec!["fin", "fout", "adj"]);
        let find = |name: &str| p.buffers.iter().find(|b| b.name() == name).unwrap().count;
        let words = bitset_words(64);
        assert_eq!(find("fin"), words);
        assert_eq!(find("fout"), words);
        assert_eq!(find("adj"), 64 * words);
    }

    #[test]
    fn emitted_hybrid_program_has_device_selector_and_both_graph_layouts() {
        let p = adaptive_sparse_dense_step(
            "fin", "fout", "count", "offs", "tgts", "kinds", "adj", 64, 7, 1, 25,
        );
        assert_eq!(p.workgroup_size, ADAPTIVE_TRAVERSAL_LINEAR_WORKGROUP_SIZE);
        let names: Vec<&str> = p.buffers.iter().map(|b| b.name()).collect();
        assert_eq!(
            names,
            vec!["fin", "fout", "count", "offs", "tgts", "kinds", "adj"]
        );
        let find = |name: &str| p.buffers.iter().find(|b| b.name() == name).unwrap().count;
        let words = bitset_words(64);
        assert_eq!(find("fin"), words);
        assert_eq!(find("fout"), words);
        assert_eq!(find("count"), 1);
        assert_eq!(find("offs"), 65);
        assert_eq!(find("tgts"), 7);
        assert_eq!(find("kinds"), 7);
        assert_eq!(find("adj"), 64 * words);
    }

    #[test]
    fn cpu_hybrid_sparse_branch_uses_csr_not_dense_rows() {
        let frontier = pack_nodes(&[0], 8);
        let offsets = vec![0, 1, 1, 1, 1, 1, 1, 1, 1];
        let targets = vec![1];
        let kinds = vec![1];
        let dense = build_dense_adj(&[(0, 2)], 8);
        let out = cpu_sparse_dense_step(&frontier, 1, &offsets, &targets, &kinds, &dense, 8, 1, 50);
        assert_eq!(out, pack_nodes(&[1], 8));
    }

    #[test]
    fn cpu_hybrid_dense_branch_uses_dense_rows_not_csr() {
        let frontier = pack_nodes(&[0, 1, 2, 3], 8);
        let offsets = vec![0, 1, 1, 1, 1, 1, 1, 1, 1];
        let targets = vec![1];
        let kinds = vec![1];
        let dense = build_dense_adj(&[(0, 5)], 8);
        let out = cpu_sparse_dense_step(&frontier, 4, &offsets, &targets, &kinds, &dense, 8, 1, 50);
        assert_eq!(out, pack_nodes(&[5], 8));
    }

    #[test]
    fn selector_roundtrip_common_density_profiles() {
        // Sparse (1% density) → CSR.
        assert!(!should_use_dense(&pack_nodes(&[5], 512), 512));

        // Dense (50% density) → dense.
        let mut f = vec![0_u32; bitset_words(512) as usize];
        for b in 0..256_u32 {
            f[b as usize / 32] |= 1 << (b % 32);
        }
        assert!(should_use_dense(&f, 512));
    }

    #[test]
    fn mode_selector_keeps_ultra_sparse_frontiers_on_queue_path() {
        assert_eq!(
            select_adaptive_traversal_mode(1_000, 10_000, 3, 25),
            AdaptiveTraversalMode::SparseQueue
        );
        assert_eq!(
            select_adaptive_traversal_mode(1_000, 10_000, 250, 25),
            AdaptiveTraversalMode::SparseDense
        );
        assert_eq!(
            select_adaptive_traversal_mode(1_000, 1_000, 100, 25),
            AdaptiveTraversalMode::SparseDense
        );
    }

    #[test]
    fn adaptive_plan_cache_keys_pin_resident_program_identity() {
        let sparse_dense =
            AdaptiveTraversalPlanCacheKey::sparse_dense(7, 64, 9, 2, 0x55, 25, 0xA11CE);
        assert_eq!(sparse_dense.kind, AdaptiveTraversalProgramKind::SparseDense);
        assert_eq!(
            sparse_dense.layout_hash,
            adaptive_traversal_program_layout_hash(
                64,
                9,
                2,
                0,
                AdaptiveTraversalProgramKind::SparseDense,
            )
        );
        assert_eq!(sparse_dense.queue_capacity, 0);
        assert_eq!(sparse_dense.allow_mask, 0x55);
        assert_eq!(sparse_dense.dense_threshold_pct, 25);
        assert_eq!(
            sparse_dense,
            AdaptiveTraversalPlanCacheKey::sparse_dense(99, 64, 9, 2, 0x55, 25, 0xA11CE),
            "resident graph contents must not fragment adaptive traversal Program caches"
        );

        assert_ne!(
            sparse_dense,
            AdaptiveTraversalPlanCacheKey::sparse_dense(7, 64, 9, 2, 0xAA, 25, 0xA11CE),
            "edge-mask policy must be part of sparse/dense resident Program identity"
        );
        assert_ne!(
            sparse_dense,
            AdaptiveTraversalPlanCacheKey::sparse_dense(7, 64, 9, 2, 0x55, 50, 0xA11CE),
            "dense cutover policy must be part of sparse/dense resident Program identity"
        );
        assert_ne!(
            sparse_dense,
            AdaptiveTraversalPlanCacheKey::sparse_dense(7, 64, 9, 2, 0x55, 25, 0xC0DA),
            "backend feature bits must be part of resident Program identity"
        );

        let queue_forward =
            AdaptiveTraversalPlanCacheKey::queue_forward(7, 64, 9, 2, 64, 0x55, 0xA11CE);
        assert_eq!(
            queue_forward.kind,
            AdaptiveTraversalProgramKind::QueueForward
        );
        assert_eq!(queue_forward.queue_capacity, 64);
        assert_eq!(queue_forward.allow_mask, 0x55);
        let queue_forward_strided =
            AdaptiveTraversalPlanCacheKey::queue_forward_strided(7, 64, 9, 2, 64, 0x55, 0xA11CE);
        assert_eq!(
            queue_forward_strided.kind,
            AdaptiveTraversalProgramKind::QueueForwardStrided
        );
        assert_ne!(
            queue_forward, queue_forward_strided,
            "serial and row-strided queue consumers must not alias in resident Program caches"
        );
        let queue_split_low =
            AdaptiveTraversalPlanCacheKey::queue_split_low(7, 64, 9, 2, 64, 4, 1024, 0x55, 0xA11CE);
        assert_eq!(
            queue_split_low.kind,
            AdaptiveTraversalProgramKind::QueueSplitLow
        );
        assert_eq!(queue_split_low.queue_capacity, 64);
        assert_eq!(queue_split_low.dense_threshold_pct, 0);
        assert_ne!(
            queue_split_low,
            AdaptiveTraversalPlanCacheKey::queue_split_low(7, 64, 9, 2, 64, 8, 1024, 0x55, 0xA11CE,),
            "mixed split queue programs must distinguish high-row queue capacity"
        );
        assert_ne!(
            queue_split_low,
            AdaptiveTraversalPlanCacheKey::queue_split_low(7, 64, 9, 2, 64, 4, 2048, 0x55, 0xA11CE,),
            "mixed split queue programs must distinguish high-degree threshold"
        );
        assert_ne!(
            queue_forward,
            AdaptiveTraversalPlanCacheKey::frontier_to_queue(7, 64, 9, 2, 64, 0xA11CE)
        );
        let word_counts =
            AdaptiveTraversalPlanCacheKey::frontier_word_counts(7, 8_192, 9, 256, 0xA11CE);
        assert_eq!(
            word_counts.kind,
            AdaptiveTraversalProgramKind::FrontierWordCounts
        );
        assert_eq!(word_counts.queue_capacity, 0);
        let block_offsets = AdaptiveTraversalPlanCacheKey::frontier_word_block_offsets(
            7, 32_897, 9, 1_029, 0xA11CE,
        );
        assert_eq!(
            block_offsets.kind,
            AdaptiveTraversalProgramKind::FrontierWordBlockOffsets
        );
        assert_eq!(block_offsets.queue_capacity, 0);
        let word_prefix = AdaptiveTraversalPlanCacheKey::frontier_word_prefix_queue(
            7, 8_192, 9, 256, 8_192, 0xA11CE,
        );
        assert_eq!(
            word_prefix.kind,
            AdaptiveTraversalProgramKind::FrontierWordPrefixQueue
        );
        assert_eq!(word_prefix.queue_capacity, 8_192);
        assert_ne!(
            word_prefix,
            AdaptiveTraversalPlanCacheKey::frontier_to_queue(7, 8_192, 9, 256, 8_192, 0xA11CE),
            "deterministic word-prefix queue programs must not alias atomic queue builders"
        );
        let block_offset_queue = AdaptiveTraversalPlanCacheKey::frontier_word_block_offsets_queue(
            7, 32_897, 9, 1_029, 32_897, 0xA11CE,
        );
        assert_eq!(
            block_offset_queue.kind,
            AdaptiveTraversalProgramKind::FrontierWordBlockOffsetsQueue
        );
        assert_eq!(block_offset_queue.queue_capacity, 32_897);
        assert_ne!(
            block_offset_queue, word_prefix,
            "block-offset queue programs must not alias the previous-block-loop scatter"
        );

        let dense = AdaptiveTraversalPlanCacheKey::four_russians_dense(99, 128, 4, 0xA11CE);
        assert_eq!(dense.kind, AdaptiveTraversalProgramKind::FourRussiansDense);
        assert_eq!(dense.edge_count, 0);
        assert_eq!(dense.queue_capacity, 0);
        assert_eq!(
            dense,
            AdaptiveTraversalPlanCacheKey::four_russians_dense(7, 128, 4, 0xA11CE),
            "resident Four-Russians LUT contents must not fragment dense Program caches"
        );
    }

    #[test]
    fn adaptive_layout_validation_accepts_valid_csr_and_dense_rows() {
        let layout = validate_adaptive_traversal_layout(
            3,
            &[0, 1, 2, 2],
            &[1, 2],
            &[1, 1],
            &build_dense_adj(&[(0, 1), (1, 2)], 3),
        )
        .unwrap();
        assert_eq!(layout.edge_count, 2);
        assert_eq!(layout.max_row_degree, 1);
        assert_eq!(layout.edge_storage_words, 2);
        assert_eq!(layout.words, 1);
        assert_eq!(layout.dense_words, 3);
    }

    #[test]
    fn adaptive_layout_validation_rejects_malformed_layouts() {
        let dense = build_dense_adj(&[(0, 1)], 2);
        let err =
            validate_adaptive_traversal_layout(2, &[0, 2, 1], &[1], &[1], &dense).unwrap_err();
        assert!(err.contains("final CSR offset") || err.contains("non-monotonic"));

        let err =
            validate_adaptive_traversal_layout(2, &[0, 1, 1], &[2], &[1], &dense).unwrap_err();
        assert!(err.contains("outside node_count"));

        let err = validate_adaptive_traversal_layout(2, &[0, 1, 1], &[1], &[1], &[]).unwrap_err();
        assert!(err.contains("dense adjacency words"));
    }

    #[test]
    fn adaptive_frontier_validation_accepts_canonical_frontier() {
        assert_eq!(
            validate_adaptive_frontier(64, &[1, 0]).unwrap(),
            AdaptiveFrontierLayout {
                words: 2,
                words_u32: 2,
            }
        );
    }

    #[test]
    fn adaptive_frontier_work_plan_classifies_zero_and_nonzero_frontiers() {
        assert_eq!(
            plan_adaptive_frontier_work(64, &[0, 0]).unwrap(),
            AdaptiveFrontierWorkPlan {
                layout: AdaptiveFrontierLayout {
                    words: 2,
                    words_u32: 2,
                },
                has_active_bits: false,
            }
        );

        assert!(
            plan_adaptive_frontier_work(64, &[0, 1])
                .unwrap()
                .has_active_bits
        );
    }

    #[test]
    fn adaptive_frontier_stats_ignore_tail_padding_bits() {
        let stats = adaptive_frontier_stats(35, &[0b101, u32::MAX & !0b111], "tail stats")
            .expect("Fix: tail-padded frontier should be valid");

        assert_eq!(stats.popcount, 2);
        assert_eq!(stats.nonzero_words, 1);
        assert_eq!(
            adaptive_frontier_popcount_in_domain(35, &[0b101, u32::MAX & !0b111], "tail popcount")
                .expect("Fix: tail-padded frontier should count"),
            2
        );
        assert!(
            !plan_adaptive_frontier_work(35, &[0, u32::MAX & !0b111])
                .expect("Fix: tail-only padding frontier should be valid")
                .has_active_bits,
            "tail padding bits beyond node_count must not trigger resident traversal work"
        );
        assert!(
            !should_use_dense(&[0, u32::MAX & !0b111], 35),
            "tail padding bits must not push adaptive mode selection toward dense traversal"
        );
    }

    #[test]
    fn adaptive_frontier_validation_rejects_zero_nodes_and_wrong_width() {
        let err = validate_adaptive_frontier(0, &[]).unwrap_err();
        assert!(err.contains("node_count > 0"));

        let err = validate_adaptive_frontier(64, &[1]).unwrap_err();
        assert!(err.contains("expected 2 word"));
    }

    #[test]
    fn resident_frontier_plan_centralizes_bytes_and_grids() {
        let plan = plan_adaptive_resident_frontier_step(8_193, &[1; 257])
            .expect("Fix: resident frontier plan should accept a correctly shaped frontier");

        assert!(plan.work.has_active_bits);
        assert_eq!(plan.work.layout.words_u32, 257);
        assert_eq!(plan.frontier_bytes, 257 * std::mem::size_of::<u32>());
        assert_eq!(plan.popcount_bytes, std::mem::size_of::<u32>());
        assert_eq!(plan.frontier_word_grid, [2, 1, 1]);
        assert_eq!(plan.node_grid, [33, 1, 1]);
    }

    #[test]
    fn adaptive_node_dispatch_grid_packs_node_lanes_into_blocks() {
        assert_eq!(adaptive_node_dispatch_grid(0), [1, 1, 1]);
        assert_eq!(adaptive_node_dispatch_grid(1), [1, 1, 1]);
        assert_eq!(adaptive_node_dispatch_grid(256), [1, 1, 1]);
        assert_eq!(adaptive_node_dispatch_grid(257), [2, 1, 1]);
        assert_eq!(adaptive_node_dispatch_grid(513), [3, 1, 1]);
    }

    #[test]
    fn generated_adaptive_node_dispatch_grid_covers_all_shapes_to_8192() {
        for node_count in 0..=8_192 {
            let grid = adaptive_node_dispatch_grid(node_count);
            assert_eq!(
                grid[1], 1,
                "Fix: adaptive node grid y dimension drifted at node_count={node_count}"
            );
            assert_eq!(
                grid[2], 1,
                "Fix: adaptive node grid z dimension drifted at node_count={node_count}"
            );
            assert!(
                grid[0] >= 1,
                "Fix: adaptive node grid must keep empty traversal launchable"
            );
            assert!(
                grid[0] * ADAPTIVE_TRAVERSAL_LINEAR_WORKGROUP_LANES >= node_count.max(1),
                "Fix: adaptive node grid under-covers node_count={node_count}"
            );
            assert!(
                grid[0] == 1
                    || (grid[0] - 1) * ADAPTIVE_TRAVERSAL_LINEAR_WORKGROUP_LANES
                        < node_count.max(1),
                "Fix: adaptive node grid over-launches an avoidable extra block at node_count={node_count}"
            );
        }
    }

    #[test]
    fn resident_sparse_queue_plan_centralizes_queue_shape() {
        let plan = plan_adaptive_resident_sparse_queue_step(513, &[1; 17])
            .expect("Fix: resident sparse-queue plan should accept a correctly shaped frontier");

        assert_eq!(plan.frontier.work.layout.words, 17);
        assert_eq!(plan.frontier_nonzero_words, 17);
        assert_eq!(plan.queue_capacity, 32);
        assert_eq!(plan.queue_bytes, 32 * std::mem::size_of::<u32>());
        assert_eq!(plan.queue_grid, [1, 1, 1]);
    }

    #[test]
    fn resident_sparse_queue_plan_sizes_queue_from_active_frontier() {
        let node_count = 1_000_000u32;
        let mut frontier = vec![0u32; bitset_words(node_count) as usize];
        frontier[0] = 1;

        let single = plan_adaptive_resident_sparse_queue_step(node_count, &frontier)
            .expect("Fix: resident sparse-queue plan should accept a single active source");

        assert_eq!(single.queue_capacity, 1);
        assert_eq!(single.frontier_nonzero_words, 1);
        assert_eq!(single.queue_bytes, std::mem::size_of::<u32>());
        assert_eq!(single.queue_grid, [1, 1, 1]);

        for node in 1..257u32 {
            frontier[(node / 32) as usize] |= 1 << (node % 32);
        }
        let bucketed = plan_adaptive_resident_sparse_queue_step(node_count, &frontier)
            .expect("Fix: resident sparse-queue plan should accept a sparse active frontier");

        assert_eq!(bucketed.queue_capacity, 512);
        assert_eq!(bucketed.frontier_nonzero_words, 9);
        assert_eq!(bucketed.queue_bytes, 512 * std::mem::size_of::<u32>());
        assert_eq!(bucketed.queue_grid, [2, 1, 1]);
    }

    #[test]
    fn generated_sparse_queue_capacity_covers_active_count_without_graph_sized_overlaunch() {
        for seed in 0..10_000u32 {
            let node_count = 1 + (mix32(seed) % 1_000_000);
            let frontier_popcount = mix32(seed ^ 0xA57A_5A7A);
            let active = frontier_popcount.min(node_count);
            let capacity = adaptive_sparse_queue_capacity(node_count, frontier_popcount);

            assert!(capacity >= active.max(1));
            assert!(capacity <= node_count);
            if active <= node_count / 2 && active > 0 {
                assert!(
                    capacity <= active.saturating_mul(2),
                    "Fix: sparse queue capacity should bucket active_count={active} tightly, got {capacity}"
                );
            }
        }
    }

    #[test]
    fn resident_auto_plan_selects_mode_from_primitive_popcount() {
        let mut frontier = vec![0u32; bitset_words(1_000) as usize];
        for node in 0..260u32 {
            frontier[(node / 32) as usize] |= 1 << (node % 32);
        }

        let plan = plan_adaptive_resident_auto_step(1_000, 10_000, &frontier, 25)
            .expect("Fix: resident auto plan should accept a correctly shaped frontier");

        assert_eq!(plan.frontier_popcount, 260);
        assert_eq!(plan.mode, AdaptiveTraversalMode::SparseDense);
        assert!(plan.frontier.work.has_active_bits);
    }

    #[test]
    fn resident_auto_plan_zero_frontier_keeps_sparse_queue_identity_case() {
        let plan = plan_adaptive_resident_auto_step(64, 128, &[0, 0], 25)
            .expect("Fix: zero frontier still has a valid resident auto plan");

        assert_eq!(plan.frontier_popcount, 0);
        assert_eq!(plan.mode, AdaptiveTraversalMode::SparseQueue);
        assert!(!plan.frontier.work.has_active_bits);
    }

    fn mix32(mut value: u32) -> u32 {
        value ^= value >> 16;
        value = value.wrapping_mul(0x7feb_352d);
        value ^= value >> 15;
        value = value.wrapping_mul(0x846c_a68b);
        value ^ (value >> 16)
    }
}
