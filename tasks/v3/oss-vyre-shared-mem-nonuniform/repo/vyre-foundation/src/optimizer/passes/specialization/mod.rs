//! Specialization / dispatch catalog (Phase 4G).
//!
//! Workload-aware rewrites that trade cost on one dimension for a
//! runtime-safety guarantee on another (autotune adds bounds-check
//! guards in exchange for tighter workgroup-size selection).

/// Dynamic workgroup-size autotuning.
pub mod autotune;
