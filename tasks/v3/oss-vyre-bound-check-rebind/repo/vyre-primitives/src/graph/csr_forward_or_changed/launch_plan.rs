use vyre_foundation::ir::Program;

use super::hash::csr_forward_or_changed_padded_slice_fingerprint;
use super::layout::{
    CsrForwardOrChangedLayout, CsrForwardOrChangedProgramKey, CsrForwardOrChangedStaticInputKey,
};
use super::program_dispatch::build_csr_forward_or_changed_dispatch_program;

/// Lightweight primitive-owned dispatch plan without an allocated [`Program`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CsrForwardOrChangedLaunchPlan {
    key: CsrForwardOrChangedProgramKey,
    dispatch_grid: [u32; 3],
}

impl CsrForwardOrChangedLaunchPlan {
    #[must_use]
    pub(crate) const fn new(key: CsrForwardOrChangedProgramKey, dispatch_grid: [u32; 3]) -> Self {
        Self { key, dispatch_grid }
    }

    /// Validated CSR/frontier layout.
    #[must_use]
    pub const fn layout(&self) -> CsrForwardOrChangedLayout {
        self.key.layout()
    }

    /// Stable key for caching the generated primitive program.
    #[must_use]
    pub const fn program_key(&self) -> CsrForwardOrChangedProgramKey {
        self.key
    }

    /// Build the selected primitive program.
    ///
    /// # Errors
    ///
    /// Returns an actionable diagnostic when the selected changed-history
    /// program cannot be represented.
    pub fn program(&self) -> Result<Program, String> {
        build_csr_forward_or_changed_dispatch_program(self.key)
    }

    /// Number of u32 words in the changed readback.
    #[must_use]
    pub const fn changed_words(&self) -> usize {
        self.key.changed_slots() as usize
    }

    /// True when the launch uses per-iteration changed history and a slot input.
    #[must_use]
    pub const fn uses_changed_history(&self) -> bool {
        self.key.uses_changed_history()
    }

    /// Changed-slot value to upload for this iteration when the fast path is active.
    #[must_use]
    pub const fn changed_slot_value(&self, iteration: u32) -> Option<u32> {
        if self.key.uses_changed_history() {
            Some(iteration)
        } else {
            None
        }
    }

    /// Index in the changed readback that carries this iteration's convergence flag.
    ///
    /// # Errors
    ///
    /// Returns an actionable diagnostic when the caller asks for an iteration
    /// outside the changed-history buffer selected by this primitive plan.
    pub fn changed_read_index(&self, iteration: u32) -> Result<usize, String> {
        if !self.key.uses_changed_history() {
            return Ok(0);
        }
        let index = usize::try_from(iteration).map_err(|_| {
            format!(
                "Fix: csr_forward_or_changed iteration {iteration} cannot be represented as a changed-history readback index."
            )
        })?;
        if index >= self.changed_words() {
            return Err(format!(
                "Fix: csr_forward_or_changed iteration {iteration} is outside changed-history length {}.",
                self.changed_words()
            ));
        }
        Ok(index)
    }

    /// Dispatch grid for one expansion pass.
    #[must_use]
    pub const fn dispatch_grid(&self) -> [u32; 3] {
        self.dispatch_grid
    }

    /// Number of u32 words in the frontier accumulator.
    #[must_use]
    pub const fn frontier_words(&self) -> usize {
        self.key.layout().frontier_words
    }

    /// Number of u32 words in node-indexed scratch buffers.
    #[must_use]
    pub const fn node_words(&self) -> usize {
        self.key.layout().node_words
    }

    /// Number of u32 words in the edge-offset buffer.
    #[must_use]
    pub const fn edge_offset_words(&self) -> usize {
        self.key.layout().edge_offset_words
    }

    /// Number of u32 words in edge-indexed target/kind buffers after padding.
    #[must_use]
    pub const fn edge_storage_words(&self) -> usize {
        self.key.layout().edge_storage_words
    }

    /// Return the primitive-owned cache identity for static CSR graph inputs.
    ///
    /// Edge arrays must match the edge count represented by the launch plan.
    /// Empty edge-offset slices are accepted for zero-edge graphs because they
    /// normalize to the same zero-padded upload as canonical `[0; n + 1]`
    /// offsets.
    ///
    /// # Errors
    ///
    /// Returns an actionable diagnostic when the supplied CSR slices no longer
    /// match the validated launch-plan shape.
    pub fn static_input_key(
        &self,
        edge_offsets: &[u32],
        edge_targets: &[u32],
        edge_kind_mask: &[u32],
    ) -> Result<CsrForwardOrChangedStaticInputKey, String> {
        let layout = self.layout();
        if !edge_offsets.is_empty() && edge_offsets.len() != layout.edge_offset_words {
            return Err(format!(
                "Fix: csr_forward_or_changed static key expected either empty zero-edge offsets or {} offset words, got {}.",
                layout.edge_offset_words,
                edge_offsets.len()
            ));
        }
        let expected_edges = layout.shape_edge_count as usize;
        if edge_targets.len() != expected_edges {
            return Err(format!(
                "Fix: csr_forward_or_changed static key expected {expected_edges} edge target word(s), got {}.",
                edge_targets.len()
            ));
        }
        if edge_kind_mask.len() != expected_edges {
            return Err(format!(
                "Fix: csr_forward_or_changed static key expected {expected_edges} edge kind word(s), got {}.",
                edge_kind_mask.len()
            ));
        }
        Ok(CsrForwardOrChangedStaticInputKey {
            program_key: self.program_key(),
            edge_offset_words: layout.edge_offset_words,
            edge_storage_words: layout.edge_storage_words,
            changed_words: self.changed_words(),
            edge_offsets_hash: csr_forward_or_changed_padded_slice_fingerprint(
                edge_offsets,
                layout.edge_offset_words,
            ),
            edge_targets_hash: csr_forward_or_changed_padded_slice_fingerprint(
                edge_targets,
                layout.edge_storage_words,
            ),
            edge_kind_mask_hash: csr_forward_or_changed_padded_slice_fingerprint(
                edge_kind_mask,
                layout.edge_storage_words,
            ),
        })
    }
}
