//! Runtime / evaluation surface  -  IR-time and dispatch-time machinery.
//!
//! Audit cleanup A12 (2026-04-30): grouped from `vyre-foundation/src/`
//! root scatter so the layout scales beyond a flat 22-file root.

/// CPU-side `Op` trait + helpers for op-by-op evaluation.
pub mod cpu_op;
/// CPU reference implementations of substrate primitives.
pub mod cpu_references;
/// Compile-time IR evaluator (used by tests + const-fold spec checks).
pub mod ir_eval;
/// Engine-wide match-result type (carries findings + telemetry).
pub mod match_result;
/// Memory model: ordering, scope, and consistency rules.
pub mod memory_model;
/// Performance counter + telemetry surface.
pub mod perf;
/// Program-level capability bits surfaced from the runtime.
pub mod program_caps;
