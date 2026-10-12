//! Vec2/vec4 packing analysis for vyre kernels.
//!
//! Source-of-truth: `PERF_ROADMAP_2026-05-01.md` section B.1 item B1.
//!
//! Modern GPU shader languages (WGSL, GLSL/SPIR-V, CUDA C++) all
//! support packed vector loads and stores: a single `vec4<f32>` load
//! moves 16 bytes in one transaction instead of four scalar 4-byte
//! transactions. On memory-bound ops this is up to 4x throughput.
//!
//! This crate detects packing candidates: groups of adjacent
//! `LoadGlobal` (or `StoreGlobal`) ops that:
//!
//! 1. Read/write the same buffer.
//! 2. Have indices `i, i+1, i+2, ...` for the same base.
//! 3. Have the same dtype.
//! 4. Are not interleaved with side-effecting ops on the same buffer
//!    (RAW/WAR hazards).
//! 5. Fall in {2, 3, 4} consecutive ops (vec2, vec3, vec4 group sizes).
//!
//! Phase 1 (this crate today): detection only. Walk the op stream,
//! emit a `PackingPlan` listing every detected group with op-index
//! ranges and the proposed packed dtype. Phase 2 (follow-up):
//! emitter-side rewrite that fuses the group into one packed op.
//!
//! Same shape as vyre-coalesce, vyre-shared-mem-promote, and
//! vyre-bank-conflict  -  all four analyses operate on KernelDescriptor
//! and live one layer below the per-substrate emitters.

pub mod analysis;
pub mod plan;

pub use analysis::analyze;
pub use plan::{PackGroup, PackKind, PackingPlan};
