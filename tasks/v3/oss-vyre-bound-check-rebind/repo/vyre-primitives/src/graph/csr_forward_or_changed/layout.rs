use crate::graph::program_graph::BINDING_PRIMITIVE_START;

/// Canonical op id.
pub(crate) const OP_ID: &str = "vyre-primitives::graph::csr_forward_or_changed";
/// Canonical binding index for the frontier accumulator.
pub(crate) const CSR_FORWARD_OR_CHANGED_FRONTIER_BUFFER: u32 = BINDING_PRIMITIVE_START;
/// Canonical binding index for the changed flag/history buffer.
pub(crate) const CSR_FORWARD_OR_CHANGED_CHANGED_BUFFER: u32 = BINDING_PRIMITIVE_START + 1;
/// Canonical one-lane workgroup for CSR forward-or-changed programs.
pub(crate) const CSR_FORWARD_OR_CHANGED_WORKGROUP_SIZE: [u32; 3] = [1, 1, 1];
/// Source-lane workgroup for node-parallel CSR forward-or-changed programs.
pub(crate) const CSR_FORWARD_OR_CHANGED_PARALLEL_WORKGROUP_SIZE: [u32; 3] = [256, 1, 1];
/// Iteration ceiling where a changed-history buffer avoids per-iteration zeroing.
pub(crate) const CSR_FORWARD_OR_CHANGED_HISTORY_FAST_PATH_MAX_ITERS: u32 = 64;

/// Dispatch grid for a node-parallel CSR forward-or-changed pass.
#[must_use]
pub const fn csr_forward_or_changed_parallel_grid(node_count: u32) -> [u32; 3] {
    [
        ceil_div_u32(
            at_least_one(node_count),
            CSR_FORWARD_OR_CHANGED_PARALLEL_WORKGROUP_SIZE[0],
        ),
        1,
        1,
    ]
}

/// Dispatch grid for a batched node-parallel CSR forward-or-changed pass.
#[must_use]
pub const fn csr_forward_or_changed_parallel_batch_grid(
    node_count: u32,
    query_count: u32,
) -> [u32; 3] {
    [
        ceil_div_u32(
            at_least_one(node_count),
            CSR_FORWARD_OR_CHANGED_PARALLEL_WORKGROUP_SIZE[0],
        ),
        at_least_one(query_count),
        1,
    ]
}

const fn at_least_one(value: u32) -> u32 {
    if value == 0 {
        1
    } else {
        value
    }
}

const fn ceil_div_u32(value: u32, divisor: u32) -> u32 {
    ((value - 1) / divisor) + 1
}

/// Validated dispatch layout for the forward-or-changed CSR primitive.
///
/// The primitive owns these derived counts so dispatch wrappers do not fork CSR
/// offset, edge-array, frontier, or scratch sizing rules.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CsrForwardOrChangedLayout {
    /// Number of nodes accepted by the primitive.
    pub node_count: u32,
    /// Number of words required by node-indexed scratch buffers.
    pub node_words: usize,
    /// Number of words required by the edge-offset buffer.
    pub edge_offset_words: usize,
    /// Number of edge-array words supplied to the primitive.
    pub edge_storage_words: usize,
    /// Edge count used when constructing [`ProgramGraphShape`].
    pub shape_edge_count: u32,
    /// Number of frontier words used by the dispatch buffer.
    pub frontier_words: usize,
}

/// Program identity for the forward-or-changed CSR primitive.
///
/// Dispatch consumers can cache generated programs by this key without
/// re-implementing CSR validation, changed-history selection, or launch-grid
/// policy outside `vyre-primitives`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CsrForwardOrChangedProgramKey {
    layout: CsrForwardOrChangedLayout,
    allow_mask: u32,
    changed_slots: u32,
    uses_changed_history: bool,
}

/// Primitive-owned identity for reusable CSR forward-or-changed static inputs.
///
/// Dispatch wrappers stage edge offsets, targets, masks, and changed-history
/// buffers according to the primitive launch plan. This key keeps the content
/// identity next to that plan so wrappers do not fork graph-fingerprint rules.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CsrForwardOrChangedStaticInputKey {
    /// Program identity selected by the primitive launch planner.
    pub program_key: CsrForwardOrChangedProgramKey,
    /// Words in the staged edge-offset input.
    pub edge_offset_words: usize,
    /// Words in each staged edge-indexed input.
    pub edge_storage_words: usize,
    /// Words in the changed readback/scratch buffer.
    pub changed_words: usize,
    /// Stable fingerprint of the padded edge-offset upload.
    pub edge_offsets_hash: u64,
    /// Stable fingerprint of the padded edge-target upload.
    pub edge_targets_hash: u64,
    /// Stable fingerprint of the padded edge-kind upload.
    pub edge_kind_mask_hash: u64,
}

impl CsrForwardOrChangedProgramKey {
    #[must_use]
    pub(crate) const fn new(
        layout: CsrForwardOrChangedLayout,
        allow_mask: u32,
        changed_slots: u32,
        uses_changed_history: bool,
    ) -> Self {
        Self {
            layout,
            allow_mask,
            changed_slots,
            uses_changed_history,
        }
    }

    /// Validated CSR/frontier layout represented by this program.
    #[must_use]
    pub const fn layout(&self) -> CsrForwardOrChangedLayout {
        self.layout
    }

    /// Edge-kind mask accepted by this program.
    #[must_use]
    pub const fn allow_mask(&self) -> u32 {
        self.allow_mask
    }

    /// Number of changed-buffer slots this program writes.
    #[must_use]
    pub const fn changed_slots(&self) -> u32 {
        self.changed_slots
    }

    /// True when this program uses the dynamic changed-history fast path.
    #[must_use]
    pub const fn uses_changed_history(&self) -> bool {
        self.uses_changed_history
    }
}
