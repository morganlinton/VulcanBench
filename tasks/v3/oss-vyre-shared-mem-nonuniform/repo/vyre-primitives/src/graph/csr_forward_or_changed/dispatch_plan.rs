use vyre_foundation::ir::Program;

use super::launch_plan::CsrForwardOrChangedLaunchPlan;
use super::layout::{CsrForwardOrChangedLayout, CsrForwardOrChangedProgramKey};

/// Primitive-owned CSR forward-or-changed dispatch plan.
pub(crate) struct CsrForwardOrChangedDispatchPlan {
    launch: CsrForwardOrChangedLaunchPlan,
    program: Program,
}

impl CsrForwardOrChangedDispatchPlan {
    #[must_use]
    pub(crate) const fn new(launch: CsrForwardOrChangedLaunchPlan, program: Program) -> Self {
        Self { launch, program }
    }

    /// Validated CSR/frontier layout.
    #[must_use]
    pub(crate) const fn layout(&self) -> CsrForwardOrChangedLayout {
        self.launch.layout()
    }

    /// Lightweight launch plan used to build this dispatch plan.
    #[must_use]
    pub(crate) const fn launch(&self) -> CsrForwardOrChangedLaunchPlan {
        self.launch
    }

    /// Stable key for caching the generated primitive program.
    #[must_use]
    pub(crate) const fn program_key(&self) -> CsrForwardOrChangedProgramKey {
        self.launch.program_key()
    }

    /// Program selected by the primitive launch planner.
    #[must_use]
    pub(crate) const fn program(&self) -> &Program {
        &self.program
    }

    /// Number of u32 words in the changed readback.
    #[must_use]
    pub(crate) const fn changed_words(&self) -> usize {
        self.launch.changed_words()
    }

    /// True when the launch uses per-iteration changed history and a slot input.
    #[must_use]
    pub(crate) const fn uses_changed_history(&self) -> bool {
        self.launch.uses_changed_history()
    }

    /// Changed-slot value to upload for this iteration when the fast path is active.
    #[must_use]
    pub(crate) const fn changed_slot_value(&self, iteration: u32) -> Option<u32> {
        self.launch.changed_slot_value(iteration)
    }

    /// Index in the changed readback that carries this iteration's convergence flag.
    ///
    /// # Errors
    ///
    /// Returns an actionable diagnostic when the caller asks for an iteration
    /// outside the changed-history buffer selected by this primitive plan.
    pub(crate) fn changed_read_index(&self, iteration: u32) -> Result<usize, String> {
        self.launch.changed_read_index(iteration)
    }

    /// Dispatch grid for one expansion pass.
    #[must_use]
    pub(crate) const fn dispatch_grid(&self) -> [u32; 3] {
        self.launch.dispatch_grid()
    }

    /// Number of u32 words in the frontier accumulator.
    #[must_use]
    pub(crate) const fn frontier_words(&self) -> usize {
        self.launch.frontier_words()
    }

    /// Number of u32 words in node-indexed scratch buffers.
    #[must_use]
    pub(crate) const fn node_words(&self) -> usize {
        self.launch.node_words()
    }

    /// Number of u32 words in the edge-offset buffer.
    #[must_use]
    pub(crate) const fn edge_offset_words(&self) -> usize {
        self.launch.edge_offset_words()
    }

    /// Number of u32 words in edge-indexed target/kind buffers after padding.
    #[must_use]
    pub(crate) const fn edge_storage_words(&self) -> usize {
        self.launch.edge_storage_words()
    }
}
