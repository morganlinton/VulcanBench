//! Shared-memory promotion analysis for vyre kernels.
//!
//! Source-of-truth: `PERF_ROADMAP_2026-05-01.md` section B.3 item B12.
//!
//! Workgroup-shared memory is roughly 100x faster than global memory on
//! modern GPUs. When the same data is read multiple times by threads in
//! the same workgroup, promoting it to shared memory eliminates the
//! repeated global transactions  -  typical speedup is 5-50x on tiled ops
//! (matmul, conv, attention).
//!
//! This crate detects promotion candidates. A buffer is a candidate
//! when:
//!
//! 1. It's accessed multiple times within one workgroup execution.
//! 2. Total accessed bytes per workgroup fit in the per-workgroup
//!    shared-memory budget (default 48 KiB; configurable per
//!    substrate).
//! 3. The access pattern is amenable to a tile-load (cooperative
//!    coalesced load into shared, then per-thread reads from shared).
//!
//! Phase 1 (this crate today): detection only. Walk every
//! `LoadGlobal` op, count how many times each binding slot is
//! accessed per workgroup execution, identify reuse candidates,
//! return a `PromotionPlan`. Phase 2 (follow-up): actually rewrite
//! the descriptor to insert tile-load + barrier + shared-read ops.
//!
//! Inputs:
//! - `KernelDescriptor` (post-lowering, pre-emit)
//! - `CoalescenceReport` from vyre-coalesce (so we know which
//!   accesses are coalesced; a coalesced load doesn't need
//!   promotion as urgently as a strided one).
//!
//! Output: `PromotionPlan` with one entry per candidate binding,
//! including projected savings.

pub mod analysis;
pub mod plan;

pub use analysis::analyze;
pub use plan::{PromotionCandidate, PromotionPlan};

/// Default per-workgroup shared-memory budget, in bytes. Callers with
/// tighter backend limits should pass their real budget into the
/// analysis entry point.
pub const DEFAULT_SHARED_BUDGET_BYTES: u32 = 48 * 1024;
