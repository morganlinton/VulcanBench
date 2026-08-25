#![allow(clippy::unwrap_used)]
//! Fuse multiple independent Programs into a single combined Program.
//!
//! This is the cross-dispatch fusion layer that the megakernel builder and
//! rule-composition pipeline use to collapse sibling dispatches into one
//! kernel body.  It is **not** the expression-level fusion pass
//! (`optimizer::passes::fusion`)  -  that pass lives inside one Program.
//!
//! Audit-fix A31 split this module into:
//!  - `mod.rs`: crate-level attribute + error types + module decls/re-exports
//!  - `fuse.rs`: `fuse_programs` family + multi-program implementation
//!  - `collectors.rs`: `collect_*_targets_*` walkers
//!  - `divergence.rs`: divergence + invocation-gate analysis
//!  - `helpers.rs`: misc small helpers
//!  - `tests.rs`: full proptest + unit-test suite
//!
//! # Safety invariants
//!
//! * Every buffer name that appears in more than one arm is treated as the
//!   *same* physical GPU buffer. The caller must ensure this is intentional.
//! * Access-mode upgrades are applied automatically (`ReadOnly` -> `ReadWrite`)
//!   when any arm needs to write.
//! * A `Node::Barrier` is inserted between arms when a later arm writes a
//!   buffer that an earlier arm reads, preventing write-after-read corruption.
//! * Programs marked `non_composable_with_self` cannot be fused with another
//!   copy of the same `entry_op_id`.

mod alpha_rename;
mod collectors;
mod divergence;
mod fuse;
mod helpers;

#[cfg(test)]
mod tests;

pub use fuse::{fuse_programs, fuse_programs_vec, merge_programs_shared};

/// Error returned when a fusion batch cannot be combined safely.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum FusionError {
    /// Two copies of a non-composable parser were placed in the same batch.
    SelfAliasing(FusionSelfAliasingError),
    /// A cross-arm buffer alias was detected that cannot be fixed by a
    /// barrier (e.g. both arms write the same buffer without an intervening
    /// read-only phase).
    Aliasing(FusionAliasingError),
    /// The fused launch geometry would over-dispatch the largest arm by
    /// more than the shared scheduling policy allows. Caller should fall back
    /// to per-arm dispatch or split the batch.
    OverDispatch(FusionOverDispatchError),
}

impl std::fmt::Display for FusionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FusionError::SelfAliasing(e) => write!(f, "{e}"),
            FusionError::Aliasing(e) => write!(f, "{e}"),
            FusionError::OverDispatch(e) => write!(f, "{e}"),
        }
    }
}

/// Axis-wise workgroup-max would over-dispatch far above any single arm.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FusionOverDispatchError {
    /// Total threads required by the largest single arm.
    pub max_arm_threads: u64,
    /// Total threads the fused launch geometry would request.
    pub fused_threads: u64,
    /// Actionable fix hint.
    pub fix: &'static str,
}

impl std::fmt::Display for FusionOverDispatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "fusion would over-dispatch: fused geometry launches {} threads vs largest single arm {}. Fix: {}",
            self.fused_threads, self.max_arm_threads, self.fix
        )
    }
}

impl std::error::Error for FusionError {}

/// Two copies of the same parser appeared in one fusion batch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FusionSelfAliasingError {
    /// Operation id shared by both programs.
    pub op_id: String,
    /// Actionable fix hint.
    pub fix: &'static str,
}

impl std::fmt::Display for FusionSelfAliasingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "fusion self-aliasing on op_id `{}`: two copies of a non-composable parser were fused. Fix: {}",
            self.op_id, self.fix
        )
    }
}

/// Cross-arm buffer access hazard that cannot be repaired automatically.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FusionAliasingError {
    /// Buffer involved in the hazard.
    pub buffer_name: String,
    /// Index of the arm that reads the buffer.
    pub read_arm: usize,
    /// Index of the arm that writes the buffer.
    pub write_arm: usize,
    /// Actionable fix hint.
    pub fix_hint: &'static str,
}

impl std::fmt::Display for FusionAliasingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "fusion aliasing on buffer `{}`: arm {} reads and arm {} writes without a barrier. Fix: {}",
            self.buffer_name, self.read_arm, self.write_arm, self.fix_hint
        )
    }
}
