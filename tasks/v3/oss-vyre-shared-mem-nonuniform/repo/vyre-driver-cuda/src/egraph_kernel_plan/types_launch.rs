use super::constants::{DEFAULT_MAX_BLOCKS_PER_LAUNCH, DEFAULT_THREADS_PER_BLOCK};
use crate::egraph_device_image::CudaEGraphDeviceKernelView;

/// E-graph kernel pass represented in CUDA launch planning.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CudaEGraphKernelPass {
    /// Per-row canonicalization or op/arity scanning.
    RowScan,
    /// Per-child-edge traversal over the flat child e-class column.
    ChildEdgeScan,
    /// Per-e-class grouped-row processing.
    EclassGroupScan,
    /// Per-candidate structural-signature row-pair comparison.
    StructuralSignaturePairScan,
}

/// Launch-shaping controls for e-graph kernel work planning.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CudaEGraphKernelLaunchConfig {
    /// CUDA threads per block.
    pub threads_per_block: u32,
    /// Maximum blocks emitted into one launch wave.
    pub max_blocks_per_launch: u32,
}

impl Default for CudaEGraphKernelLaunchConfig {
    fn default() -> Self {
        Self {
            threads_per_block: DEFAULT_THREADS_PER_BLOCK,
            max_blocks_per_launch: DEFAULT_MAX_BLOCKS_PER_LAUNCH,
        }
    }
}

/// One bounded CUDA launch wave for an e-graph kernel pass.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CudaEGraphKernelWave {
    /// Kernel pass.
    pub pass: CudaEGraphKernelPass,
    /// First logical row/edge/group item handled by this wave.
    pub first_item: u64,
    /// Logical row/edge/group item count handled by this wave.
    pub item_count: u64,
    /// CUDA blocks for this wave.
    pub blocks: u32,
    /// CUDA threads per block for this wave.
    pub threads_per_block: u32,
}

/// Complete launch plan for resident e-graph kernel passes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CudaEGraphKernelWorkPlan {
    /// Checked resident image view used by kernels.
    pub view: CudaEGraphDeviceKernelView,
    /// Bounded launch waves in deterministic pass order.
    pub waves: Vec<CudaEGraphKernelWave>,
    /// Sum of logical items across all waves.
    pub total_items: u64,
    /// Sum of CUDA blocks across all waves.
    pub total_blocks: u64,
}
