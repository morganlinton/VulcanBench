//! Buffer layout transformation candidate detection (AoS → SoA).
//!
//! Source-of-truth: `PERF_ROADMAP_2026-05-01.md` section J item J1.
//!
//! On GPUs, "Structure of Arrays" (SoA) layout outperforms "Array of
//! Structures" (AoS) for compute-bound workloads because consecutive
//! threads read consecutive memory addresses (coalesced) instead of
//! struct-stride-spaced addresses (scattered).
//!
//! Phase-1 detection: identify bindings whose dtype is a `Vec` /
//! `TensorShaped` / `Array` (i.e., compound element types) accessed
//! by multiple loads at consecutive thread indices. Such bindings
//! benefit from being split into one-component-per-binding SoA.
//!
//! The actual rewrite (split one binding into N component bindings,
//! re-route every load through the matching component) is phase 2 and
//! is invasive  -  it changes the kernel's binding signature, so the
//! host dispatcher needs to be updated too. Phase 1 surfaces the
//! candidates so the optimizer can decide whether to invoke the
//! rewrite.

pub mod analysis;
pub mod plan;

pub use analysis::analyze;
pub use plan::{LayoutCandidate, LayoutTransformPlan};
