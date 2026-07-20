//! Shared-memory bank-conflict analysis for vyre kernels.
//!
//! Source-of-truth: `PERF_ROADMAP_2026-05-01.md` section B.3 item B13.
//!
//! Shared memory on modern GPUs is divided into N banks. Each bank can
//! serve one read or write per cycle. When K threads in the same
//! warp/subgroup access K different addresses that map to the **same
//! bank**, those accesses serialize  -  costing up to 32x throughput
//! for the worst case (32-way conflict).
//!
//! A common cause: a stride pattern where `addr % BANK_COUNT` is the
//! same for every thread. Classic example: a 32x32 tile in shared
//! memory accessed column-major with stride 32  -  all 32 threads in a
//! warp hit bank 0, full 32-way serialization.
//!
//! This crate detects bank-conflict candidates among shared-memory
//! load/store ops in a `KernelDescriptor`. Operates substrate-neutrally
//! on the post-lowering descriptor; emit-time concerns (per-substrate
//! bank count, swizzle-padding strategies) live in emitter crates.
//!
//! Phase 1 (this crate today): detection only. Walk every
//! `LoadShared`/`StoreShared` op, look at the index expression's
//! stride, classify as `NoConflict` / `Conflict` / `Unknown`, return
//! a `BankConflictReport`. Phase 2 (follow-up): rewrites that pad
//! shared-mem allocations or swizzle indices to break conflict
//! patterns.
//!
//! Caller can override the default bank count via
//! `analyze_with_bank_count`.

pub mod analysis;
pub mod report;

pub use analysis::{analyze, analyze_with_bank_count};
pub use report::{BankAccessSite, BankConflictKind, BankConflictReport, ConflictSeverity};

/// Default bank count. This is a reasonable pessimistic default for
/// discrete GPU substrates.
pub const DEFAULT_BANK_COUNT: u32 = 32;
