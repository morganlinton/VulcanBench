use crate::graph::resident_handles::free_unique_resident_handles;
use crate::optimizer::dispatcher::{DispatchError, OptimizerDispatcher};

/// Device-resident graph layouts for adaptive sparse/dense traversal.
#[derive(Debug, Clone)]
pub struct ResidentAdaptiveTraversalGraph {
    pub(crate) node_count: u32,
    pub(crate) edge_count: u32,
    pub(crate) max_row_degree: u32,
    pub(crate) high_degree_source_count: u32,
    pub(crate) words: usize,
    pub(crate) layout_hash: u64,
    pub(crate) handles: [u64; 4],
}

/// Device-resident CSR graph for adaptive sparse-queue traversal.
#[derive(Debug, Clone)]
pub struct ResidentAdaptiveSparseQueueGraph {
    pub(crate) node_count: u32,
    pub(crate) edge_count: u32,
    pub(crate) max_row_degree: u32,
    pub(crate) high_degree_source_count: u32,
    pub(crate) words: usize,
    pub(crate) layout_hash: u64,
    pub(crate) handles: [u64; 3],
}

/// Device-resident Four-Russians dense traversal LUT for adaptive graph waves.
#[derive(Debug, Clone)]
pub struct ResidentAdaptiveFourRussiansDenseGraph {
    pub(crate) node_count: u32,
    pub(crate) words: usize,
    pub(crate) layout_hash: u64,
    pub(crate) lut_handle: u64,
}

impl ResidentAdaptiveFourRussiansDenseGraph {
    /// Number of graph nodes.
    #[must_use]
    pub fn node_count(&self) -> u32 {
        self.node_count
    }

    /// Number of u32 words per frontier bitset.
    #[must_use]
    pub fn words(&self) -> usize {
        self.words
    }

    /// Stable in-session hash of the dense LUT source layout.
    #[must_use]
    pub fn layout_hash(&self) -> u64 {
        self.layout_hash
    }

    /// Resident handle for the dense byte-tile LUT.
    #[must_use]
    pub fn lut_handle(&self) -> u64 {
        self.lut_handle
    }

    /// Free graph-resident LUT buffer.
    ///
    /// # Errors
    ///
    /// Returns the backend free failure, if any.
    pub fn free(self, dispatcher: &dyn OptimizerDispatcher) -> Result<(), DispatchError> {
        dispatcher.free_resident(self.lut_handle)
    }
}

impl ResidentAdaptiveSparseQueueGraph {
    /// Number of graph nodes.
    #[must_use]
    pub fn node_count(&self) -> u32 {
        self.node_count
    }

    /// Number of logical CSR edges.
    #[must_use]
    pub fn edge_count(&self) -> u32 {
        self.edge_count
    }

    /// Largest sparse CSR row degree.
    #[must_use]
    pub fn max_row_degree(&self) -> u32 {
        self.max_row_degree
    }

    /// Number of sparse CSR rows at or above the mixed-split high-degree threshold.
    #[must_use]
    pub fn high_degree_source_count(&self) -> u32 {
        self.high_degree_source_count
    }

    /// Number of u32 words per frontier bitset.
    #[must_use]
    pub fn words(&self) -> usize {
        self.words
    }

    /// Stable in-session hash of CSR graph layout.
    #[must_use]
    pub fn layout_hash(&self) -> u64 {
        self.layout_hash
    }

    /// Resident handles in adaptive sparse-queue order:
    /// edge_offsets, edge_targets, edge_kind_mask.
    #[must_use]
    pub fn handles(&self) -> [u64; 3] {
        self.handles
    }

    /// Free graph-resident CSR buffers.
    ///
    /// # Errors
    ///
    /// Returns the first backend free failure after attempting all handles.
    pub fn free(self, dispatcher: &dyn OptimizerDispatcher) -> Result<(), DispatchError> {
        free_unique_resident_handles(
            dispatcher,
            &self.handles,
            "resident adaptive sparse-queue graph",
        )
    }
}

impl ResidentAdaptiveTraversalGraph {
    /// Number of graph nodes.
    #[must_use]
    pub fn node_count(&self) -> u32 {
        self.node_count
    }

    /// Number of logical CSR edges.
    #[must_use]
    pub fn edge_count(&self) -> u32 {
        self.edge_count
    }

    /// Largest sparse CSR row degree.
    #[must_use]
    pub fn max_row_degree(&self) -> u32 {
        self.max_row_degree
    }

    /// Number of sparse CSR rows at or above the mixed-split high-degree threshold.
    #[must_use]
    pub fn high_degree_source_count(&self) -> u32 {
        self.high_degree_source_count
    }

    /// Number of u32 words per frontier bitset.
    #[must_use]
    pub fn words(&self) -> usize {
        self.words
    }

    /// Stable in-session hash of CSR and dense graph layouts.
    #[must_use]
    pub fn layout_hash(&self) -> u64 {
        self.layout_hash
    }

    /// Resident handles in adaptive traversal order:
    /// edge_offsets, edge_targets, edge_kind_mask, adj_rows_dense.
    #[must_use]
    pub fn handles(&self) -> [u64; 4] {
        self.handles
    }

    /// Free graph-resident buffers.
    ///
    /// # Errors
    ///
    /// Returns the first backend free failure after attempting all handles.
    pub fn free(self, dispatcher: &dyn OptimizerDispatcher) -> Result<(), DispatchError> {
        free_unique_resident_handles(
            dispatcher,
            &self.handles,
            "resident adaptive traversal graph",
        )
    }
}
