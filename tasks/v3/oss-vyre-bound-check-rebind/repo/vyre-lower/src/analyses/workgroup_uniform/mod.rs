//! Workgroup-uniform branch detection.
//!
//! Source-of-truth: `PERF_ROADMAP_2026-05-01.md` section B.3 item B15.
//!
//! On every modern GPU, when all threads in a workgroup take the same
//! branch (the "uniform" path), hardware can use a more efficient
//! single-instruction-multiple-thread dispatch. When threads diverge,
//! both sides serialize  -  costing roughly 2x throughput per branch.
//!
//! This analysis detects `StructuredIfThen` / `StructuredIfThenElse`
//! ops whose conditions are provably workgroup-uniform  -  i.e., the
//! condition does NOT depend on `LocalInvocationId` (or any value
//! derived from it). When uniform, the emitter can annotate the
//! branch with the substrate's native uniform-branch perf hint.
//!
//! Phase 1 (this module): detection only. Walks every if-branch op,
//! traces its condition's data dependency back through the op stream;
//! if the dependency closure NEVER touches `LocalInvocationId` /
//! `SubgroupLocalId`, the branch is uniform. Emitters consume the
//! report for backend-native annotations.

pub mod analysis;
pub mod report;

pub use analysis::analyze;
pub use report::{BranchSite, BranchUniformity, WorkgroupUniformReport};
