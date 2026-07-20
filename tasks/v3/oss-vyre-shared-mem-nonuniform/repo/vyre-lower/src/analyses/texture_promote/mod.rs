//! Texture-memory promotion candidate detection.
//!
//! Source-of-truth: `PERF_ROADMAP_2026-05-01.md` section B.3 item B11.
//!
//! GPU texture memory provides hardware-accelerated read paths for
//! 2D/3D spatially-coherent access patterns. Promoting a global
//! read-only buffer to texture memory yields:
//! - Cached reads with hardware filtering / interpolation.
//! - Better spatial locality than linear-array access.
//! - Free clamping / wrap-around at the boundary.
//!
//! Promotion is profitable when:
//! 1. The binding is read-only (writes preclude texture promotion).
//! 2. Multiple loads target the same binding (spatial reuse pays).
//! 3. Index patterns suggest 2D / 3D access (multi-dim stride math).
//!
//! Phase 1 (this module): detect read-only bindings with multiple
//! loads. Phase 2 (follow-up): infer 2D/3D access pattern via index
//! decomposition; emit substrate-specific texture binding decoration.

pub mod analysis;
pub mod plan;

pub use analysis::analyze;
pub use plan::{TextureCandidate, TexturePromotionPlan};
