//! Adapter capabilities + pass context shared by backend-aware passes that
//! adapt to the real device.
//!
//! The existing [`crate::optimizer::ProgramPass`] trait takes a `Program`
//! and returns a `PassResult`. That shape is fine for IR-only
//! rewrites. But backend-aware passes  -  fusion (Gemini C's C-B8),
//! subgroup-op lowering (C-B2), shared-memory allocator  -  need
//! access to adapter caps at scheduling time.
//!
//! This module ships the types every such pass consumes:
//!
//! * [`AdapterCaps`]  -  the subset of concrete adapter info that
//!   passes care about, in a backend-neutral shape. Backends fill
//!   this in; passes read it.
//! * [`PassCtx`]  -  the mutable context handed to passes that opt
//!   into the ctx-based API. Accretes [`crate::diagnostics::Diagnostic`]s,
//!   carries the caps, exposes a typed analysis cache.
//! * [`scheduling_error_to_diagnostic`]  -  maps the existing
//!   `crate::PassSchedulingError` onto a structured
//!   diagnostic with the stable `E-PASS-CYCLE` / `E-PASS-REQUIRE`
//!   codes.
//!
//! Program-IR passes continue to work unchanged. New passes (added
//! as part of A-C7b and the Gemini C perf blitz) can adopt the
//! ctx-based path without breaking the registry.

use crate::diagnostics::{Diagnostic, OpLocation};
use rustc_hash::FxHashMap;

/// The subset of device info passes read.
///
/// Concrete backends fill this from their native device-info,
/// feature, and limit surfaces. Passes
/// read it to decide whether subgroup ops emit intrinsics, how much
/// shared memory a kernel may use, whether to fuse large kernels,
/// etc.
///
/// `Default` is the conservative "assume nothing" configuration:
/// no subgroup ops, modest limits. A pass that gets a `Default`
/// [`AdapterCaps`] should emit the safe fallback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdapterCaps {
    /// Backend identifier.
    pub backend: &'static str,
    /// The adapter supports subgroup intrinsics.
    pub supports_subgroup_ops: bool,
    /// The adapter supports `dispatch_workgroups_indirect`.
    pub supports_indirect_dispatch: bool,
    /// The adapter supports specialization constants at pipeline
    /// creation.
    pub supports_specialization_constants: bool,
    /// Maximum compute workgroup size per dimension `[x, y, z]`.
    pub max_workgroup_size: [u32; 3],
    /// Maximum total invocations per workgroup.
    pub max_invocations_per_workgroup: u32,
    /// Maximum shared memory per workgroup, in bytes.
    pub max_shared_memory_bytes: u32,
    /// Maximum compute storage buffer binding size, in bytes.
    pub max_storage_buffer_binding_size: u64,
    /// Subgroup size (warp / wavefront). `0` when unknown.
    pub subgroup_size: u32,
    /// Physical compute-unit count. `0` when unknown.
    pub compute_units: u32,
    /// Maximum registers per thread. `0` when unknown.
    pub regs_per_thread_max: u32,
    /// L1 cache size in bytes. `0` when unknown.
    pub l1_cache_bytes: u32,
    /// L2 cache size in bytes. `0` when unknown.
    pub l2_cache_bytes: u32,
    /// Peak memory bandwidth in GB/s. `0` when unknown.
    pub mem_bw_gbps: u32,
    /// Device-signature preferred unroll depth. `0` when unknown.
    pub ideal_unroll_depth: u32,
    /// Device-signature preferred vector pack width in bits. `0` when unknown.
    pub ideal_vector_pack_bits: u32,
    /// Device-signature preferred workgroup tile. `[0, 0, 0]` when unknown.
    pub ideal_workgroup_tile: [u32; 3],
    /// Shared-memory bank count. `0` when unknown.
    pub shared_memory_bank_count: u32,
    /// Shared-memory bank width in bytes. `0` when unknown.
    pub shared_memory_bank_width_bytes: u32,
}

impl Default for AdapterCaps {
    fn default() -> Self {
        Self {
            backend: "unknown",
            supports_subgroup_ops: false,
            supports_indirect_dispatch: false,
            supports_specialization_constants: false,
            max_workgroup_size: [256, 256, 64],
            max_invocations_per_workgroup: 256,
            max_shared_memory_bytes: 16 * 1024,
            max_storage_buffer_binding_size: 128 * 1024 * 1024,
            subgroup_size: 0,
            compute_units: 0,
            regs_per_thread_max: 0,
            l1_cache_bytes: 0,
            l2_cache_bytes: 0,
            mem_bw_gbps: 0,
            ideal_unroll_depth: 0,
            ideal_vector_pack_bits: 0,
            ideal_workgroup_tile: [0, 0, 0],
            shared_memory_bank_count: 0,
            shared_memory_bank_width_bytes: 0,
        }
    }
}

impl AdapterCaps {
    /// Conservative profile: "assume nothing advanced".
    ///
    /// A pass scheduled against this profile must take the
    /// fallback path for every optional feature.
    #[must_use]
    pub const fn conservative() -> Self {
        Self {
            backend: "conservative",
            supports_subgroup_ops: false,
            supports_indirect_dispatch: false,
            supports_specialization_constants: false,
            max_workgroup_size: [256, 1, 1],
            max_invocations_per_workgroup: 256,
            max_shared_memory_bytes: 16 * 1024,
            max_storage_buffer_binding_size: 128 * 1024 * 1024,
            subgroup_size: 0,
            compute_units: 0,
            regs_per_thread_max: 0,
            l1_cache_bytes: 0,
            l2_cache_bytes: 0,
            mem_bw_gbps: 0,
            ideal_unroll_depth: 0,
            ideal_vector_pack_bits: 0,
            ideal_workgroup_tile: [0, 0, 0],
            shared_memory_bank_count: 0,
            shared_memory_bank_width_bytes: 0,
        }
    }

    /// High-end profile used by tests and synthetic planners.
    ///
    /// Used by benches and tests that want to measure the fast
    /// path without probing a real adapter.
    #[must_use]
    pub const fn high_end() -> Self {
        Self {
            backend: "high-end-dispatch",
            supports_subgroup_ops: true,
            supports_indirect_dispatch: true,
            supports_specialization_constants: true,
            max_workgroup_size: [1024, 1024, 64],
            max_invocations_per_workgroup: 1024,
            max_shared_memory_bytes: 128 * 1024,
            max_storage_buffer_binding_size: 2 * 1024 * 1024 * 1024,
            subgroup_size: 32,
            compute_units: 128,
            regs_per_thread_max: 255,
            l1_cache_bytes: 128 * 1024,
            l2_cache_bytes: 64 * 1024 * 1024,
            mem_bw_gbps: 1700,
            ideal_unroll_depth: 8,
            ideal_vector_pack_bits: 128,
            ideal_workgroup_tile: [16, 16, 1],
            shared_memory_bank_count: 32,
            shared_memory_bank_width_bytes: 4,
        }
    }
}

/// Typed analysis cache that passes share between runs.
///
/// Analyses (e.g., "this program uses shared memory", "this
/// program has at most N dispatches") compute once and cache here.
/// A pass that `provides` an analysis inserts an entry; a pass
/// that `requires` it reads one.
///
/// The value is `Box<dyn Any>` so every pass can stash its own
/// strongly-typed analysis without teaching this module about it.
#[derive(Default)]
pub struct AnalysisCache {
    entries: FxHashMap<&'static str, Box<dyn std::any::Any + Send + Sync>>,
}

impl AnalysisCache {
    /// New empty cache.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Stash an analysis result under a key.
    pub fn insert<T: std::any::Any + Send + Sync>(&mut self, key: &'static str, value: T) {
        self.entries.insert(key, Box::new(value));
    }

    /// Retrieve a typed analysis result previously inserted under `key`.
    #[must_use]
    pub fn get<T: std::any::Any>(&self, key: &'static str) -> Option<&T> {
        // CAT-B-OK: AnalysisCache is a CONTAINED type-erased store scoped
        // to the pass scheduler. Both insert and get are parametrized by
        // the same `T: Any`; the store never exposes a raw `TypeId` probe
        // nor the `Box<dyn Any>` values. This is not the type-erasure
        // anti-pattern the Cat-B rule targets.
        self.entries.get(key).and_then(|v| v.downcast_ref::<T>())
    }

    /// Drop every cached analysis  -  called between fixpoint
    /// iterations so stale analyses cannot survive an invalidation.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

impl std::fmt::Debug for AnalysisCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AnalysisCache")
            .field("entries", &self.entries.len())
            .finish()
    }
}

/// Mutable context handed to ctx-aware passes.
///
/// See [`crate::optimizer::ProgramPass`] for the ctx-aware extension API; ctx-aware
/// passes take a `PassCtx` instead and push diagnostics onto
/// [`PassCtx::diagnostics`] rather than returning them from the
/// `transform` method.
pub struct PassCtx<'a> {
    /// The program under transformation. Passes mutate this in
    /// place; fixpoint convergence is tracked by the scheduler.
    pub program: &'a mut crate::ir_inner::model::program::Program,
    /// The adapter capabilities the final backend will see.
    pub adapter_caps: &'a AdapterCaps,
    /// Analysis cache shared across passes in one schedule run.
    pub analyses: &'a mut AnalysisCache,
    /// Shared fact substrate (shape / use / type facts) for this
    /// schedule run. The scheduler initializes it before the first
    /// pass and invalidates it whenever a pass changes the program.
    pub fact_substrate: &'a mut crate::optimizer::fact_substrate::FactSubstrate,
    /// Diagnostics accumulated during this pass run. Severity
    /// `Error` halts the scheduler; `Warning` and `Note` surface
    /// after the run completes.
    pub diagnostics: &'a mut Vec<Diagnostic>,
}

/// Map a [`crate::optimizer::PassSchedulingError`] onto a structured
/// [`Diagnostic`] with a stable code.
///
/// Existing callers still receive the typed `PassSchedulingError`;
/// this function exposes the same information via the diagnostic surface
/// tooling already consumes (IDE, CI annotators, LSP).
#[must_use]
pub fn scheduling_error_to_diagnostic(err: &crate::optimizer::PassSchedulingError) -> Diagnostic {
    use crate::optimizer::PassSchedulingError as E;
    match err {
        E::UnknownRequire { pass, missing } => Diagnostic::error(format!(
            "OPTSCHED001: pass `{pass}` requires unknown pass `{missing}`. Fix: register `{missing}` or drop the requirement."
        ))
        .with_location(OpLocation::op(pass.to_string())),
        E::Cycle { pass_ids, fix } => Diagnostic::error(format!(
            "OPTSCHED002: cycle among passes {pass_ids:?}. Fix: {fix}"
        )),
        E::DuplicateId { id } => Diagnostic::error(format!(
            "OPTSCHED003: duplicate pass id `{id}`. Fix: assign every pass a unique stable id."
        )),
        E::OrderViolation { pass, requirement } => Diagnostic::error(format!(
            "OPTSCHED004: pass `{pass}` is scheduled before required pass `{requirement}`. Fix: move `{requirement}` earlier or remove the stale requirement."
        ))
        .with_location(OpLocation::op(pass.to_string())),
        E::StorageReserveFailed {
            context,
            requested,
            message,
        } => Diagnostic::error(format!(
            "OPTSCHED005: scheduler could not reserve {requested} {context} slot(s): {message}. Fix: reduce the pass set or schedule it in shards."
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_caps_conservative() {
        let caps = AdapterCaps::default();
        assert_eq!(caps.backend, "unknown");
        assert!(!caps.supports_subgroup_ops);
        assert!(!caps.supports_indirect_dispatch);
        assert_eq!(caps.subgroup_size, 0);
    }

    #[test]
    fn conservative_profile() {
        let caps = AdapterCaps::conservative();
        assert_eq!(caps.backend, "conservative");
        assert_eq!(caps.max_workgroup_size, [256, 1, 1]);
    }

    #[test]
    fn high_end_profile() {
        let caps = AdapterCaps::high_end();
        assert_eq!(caps.backend, "high-end-dispatch");
        assert!(caps.supports_subgroup_ops);
        assert!(caps.supports_indirect_dispatch);
        assert!(caps.supports_specialization_constants);
        assert_eq!(caps.subgroup_size, 32);
        assert_eq!(caps.max_invocations_per_workgroup, 1024);
    }

    #[test]
    fn analysis_cache_insert_and_get() {
        let mut cache = AnalysisCache::new();
        cache.insert("node_count", 42u32);
        assert_eq!(cache.get::<u32>("node_count"), Some(&42));
    }

    #[test]
    fn analysis_cache_get_missing() {
        let cache = AnalysisCache::new();
        assert_eq!(cache.get::<u32>("nonexistent"), None);
    }

    #[test]
    fn analysis_cache_type_mismatch() {
        let mut cache = AnalysisCache::new();
        cache.insert("node_count", 42u32);
        // Asking for wrong type returns None.
        assert_eq!(cache.get::<String>("node_count"), None);
    }

    #[test]
    fn analysis_cache_clear() {
        let mut cache = AnalysisCache::new();
        cache.insert("a", 1u32);
        cache.clear();
        assert_eq!(cache.get::<u32>("a"), None);
    }

    #[test]
    fn analysis_cache_debug() {
        let cache = AnalysisCache::new();
        let debug = format!("{cache:?}");
        assert!(debug.contains("AnalysisCache"));
    }

    #[test]
    fn scheduling_error_unknown_require() {
        let err = crate::optimizer::PassSchedulingError::UnknownRequire {
            pass: "fusion",
            missing: "dead_buffer_elim",
        };
        let diag = scheduling_error_to_diagnostic(&err);
        assert!(diag.message.contains("OPTSCHED001"));
        assert!(diag.message.contains("fusion"));
    }

    #[test]
    fn scheduling_error_cycle() {
        let err = crate::optimizer::PassSchedulingError::Cycle {
            pass_ids: vec!["a", "b"],
            fix: "break the cycle",
        };
        let diag = scheduling_error_to_diagnostic(&err);
        assert!(diag.message.contains("OPTSCHED002"));
    }

    #[test]
    fn scheduling_error_duplicate_id() {
        let err = crate::optimizer::PassSchedulingError::DuplicateId { id: "dup_pass" };
        let diag = scheduling_error_to_diagnostic(&err);
        assert!(diag.message.contains("OPTSCHED003"));
        assert!(diag.message.contains("dup_pass"));
    }
}
