//! Megakernel-related optimizer subsystem.
//!
//! Audit cleanup A9 (2026-04-30): hoisted from `pass_substrate/` so the
//! megakernel-fusion scheduler concept lives in one place. Three legitimate
//! scheduler concepts in vyre, one home each:
//!
//! - **Pass-scheduler** (`super::scheduler`)  -  picks which optimizer pass
//!   to run next inside the optimizer fixpoint loop.
//! - **Megakernel-fusion-scheduler** (this module)  -  picks which Programs
//!   to fuse into a megakernel BEFORE dispatch.
//! - **Dispatch-scheduler** (`vyre-runtime::scheduler`)  -  picks which
//!   Program/megakernel to dispatch on which device, when.
//!
//! ## Layout
//!
//! - `schedule_oracle.rs`  -  homotopy-weighted fusion-weight oracle. Given
//!   per-pass costs, computes normalized fusion weights via a linear
//!   homotopy + Euler predictor. Used by `PassScheduler::homotopy_megakernel_weights`.
//! - `matroid_subset.rs`  -  greedy matroid subset selection. Picks a
//!   bounded fusion subset from an exchange-compatibility graph.
//!   Used by `PassScheduler::max_fusion_subset`.

/// Homotopy-weighted megakernel fusion weight oracle.
pub mod schedule_oracle;

/// Greedy matroid-style fusion subset selection.
pub mod matroid_subset;

/// ROADMAP A13  -  escape-fact-driven scratch reuse plan. Walks
/// every Region in the program and queries `ProgramFacts::
/// buffer_escapes` to identify per-arm buffers the runtime can
/// recycle into a shared scratch pool.
pub mod scratch_reuse;
