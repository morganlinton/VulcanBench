//! Memory-coalescing analysis for vyre kernels.
//!
//! Source-of-truth: `PERF_ROADMAP_2026-05-01.md` section B.3 item B14.
//!
//! On every GPU substrate, global-memory reads and writes are
//! dramatically faster  -  up to 32x  -  when adjacent threads in a
//! warp/subgroup access adjacent memory addresses (a
//! "coalesced" access pattern). When they don't, each thread's access
//! becomes its own memory transaction and throughput collapses.
//!
//! The analysis operates on `vyre_lower::KernelDescriptor`
//! post-lowering and pre-emission, so every emitter can share the same
//! access-pattern signal instead of re-deriving it per backend.
//!
//! ## What "coalesced" means here
//!
//! A load/store is **coalesced** when the index expression evaluates,
//! across the workgroup's thread axis, to a strictly-increasing
//! sequence with stride 1 (i.e., thread `t` reads address `base + t`).
//! It is **strided** when the stride is a non-1 constant > 0. It is
//! **scattered** when the index depends on data the analysis can't
//! prove constant-stride (e.g., `load(buf, indirect[t])`).
//!
//! This analysis is conservative: anything it can't prove coalesced is
//! reported as `Scattered`. False positives (reporting coalesced as
//! scattered) cost only the chance for a future rewrite. False
//! negatives (reporting scattered as coalesced) would be a correctness
//! bug for any rewrite that depends on this report, so the
//! classification is intentionally conservative.

pub mod analysis;
pub mod report;

pub use analysis::analyze;
pub use report::{AccessPattern, AccessSite, CoalescenceReport};
