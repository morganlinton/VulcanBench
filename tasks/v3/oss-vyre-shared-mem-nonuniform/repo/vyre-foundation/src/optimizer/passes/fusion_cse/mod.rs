//! Fusion + CSE / DCE catalog.
//!
//! Single-Program kernel fusion + classical CSE/DCE + cross-Program
//! megakernel rule-fusion ([`fuse_cse`]) which takes many Programs and
//! emits one fused Program with shared subexpressions deduplicated
//! across rules.

/// Common-subexpression elimination  -  engine + ProgramPass registration colocated.
pub mod cse;
pub use cse::CsePass;
/// Dead-code elimination  -  engine + ProgramPass registration colocated.
pub mod dce;
pub use dce::DcePass;
/// G2: megakernel rule-fusion with cross-rule CSE  -  takes many
/// Programs, emits one fused Program with shared subexpressions
/// deduplicated across rules.
pub mod fuse_cse;
/// Kernel fusion by eliminating pure single-use scalar intermediates.
pub mod fusion;

#[cfg(test)]
pub(super) mod fusion_tests;
