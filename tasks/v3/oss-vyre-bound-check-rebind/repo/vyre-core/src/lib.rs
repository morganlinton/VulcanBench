#![forbid(unsafe_code)]
#![warn(missing_docs)]
// Every lint below is allowed for a documented reason. New lints from
// nursery/pedantic/restriction are NOT auto-allowed  -  broad blanket allows
// were removed deliberately so that future clippy findings surface as CI
// warnings instead of being silently swallowed.
#![allow(
    // Auto-generated op wrappers replay derive attributes by design.
    clippy::duplicated_attributes,
    // GPU buffer layout types (bind-group slot tuples) are inherently complex.
    clippy::type_complexity,
    // Shader-side math and wire-format POD structs do intentional integer
    // casts; the conform gate verifies byte-identity with the CPU reference.
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    // Explicit clones on Copy improve readability in serial layers where
    // semantic ownership matters more than cycle count.
    clippy::clone_on_copy,
    // Three-branch comparisons are natural in range-check oracles.
    clippy::comparison_chain,
    // Vyre uses explicit invariant violations (expect/unwrap) with `Fix:`
    // prose  -  not graceful degradation  -  per the engineering standard.
    clippy::expect_used,
    // Generic collections take external hashers by design.
    clippy::implicit_hasher,
    // SHA/hash compressors use the canonical single-letter state vars
    // (a,b,c,d,e,f,g,h per FIPS 180-4).
    clippy::many_single_char_names,
    // Error prose is centralized in the `Error` enum; per-fn `# Errors`
    // sections duplicate that contract.
    clippy::missing_errors_doc,
    // Panics document invariant violations with `Fix:` prose inline.
    clippy::missing_panics_doc,
    // Template-generated ops don't always merit `#[must_use]`.
    clippy::must_use_candidate,
    // Builder APIs take owned values by design.
    clippy::needless_pass_by_value,
    // Indexed arithmetic is clearer than iterator chains for GPU-shape loops.
    clippy::needless_range_loop,
    // Generated target-text strings use `r##` for quote safety.
    clippy::needless_raw_string_hashes,
    // Type names repeat module names for cross-crate discoverability.
    clippy::module_name_repetitions,
    // `mod X` in `X.rs` is the canonical vyre module layout.
    clippy::module_inception,
    // Math code uses short similar names (a/A, x/X) by convention.
    clippy::similar_names,
    // Internal helpers with stdlib-adjacent names are intentional for clarity.
    clippy::should_implement_trait,
    // Enforcer dispatch arms can share a body but represent distinct cases.
    clippy::match_same_arms,
    // Hot paths in the pipeline assemble strings incrementally.
    clippy::format_push_string,
    // GPU kernel dispatchers take many parameters by design (buffer slots).
    clippy::too_many_arguments,
    // Hash compressors and regex compilers have long inlined bodies.
    clippy::too_many_lines,
    // Trait signatures force `&T` for small Copy types.
    clippy::trivially_copy_pass_by_ref,
    // `Result<T, E>` with a single error variant keeps the API
    // forward-compatible as new error variants land.
    clippy::unnecessary_wraps,
    // Or-patterns are expanded for readability in large match tables.
    clippy::unnested_or_patterns,
    // GPU buffer sizes like `0x12345678` are more readable without `_`
    // separators in shader contexts.
    clippy::unreadable_literal,
    // Prose doc comments use type names that clippy wants backticked; our
    // doc style sentences already read naturally.
    clippy::doc_markdown
)]
#![cfg_attr(not(test), deny(clippy::todo, clippy::unimplemented))]
//! # vyre  -  LLVM-for-GPU
//!
//! Vyre is a GPU compute substrate centered on the `Program` type. Just as
//! LLVM lets frontends emit a single IR that lowers to many processor targets,
//! vyre lets frontends emit a single `Program` that lowers through any
//! registered backend or the pure-Rust reference interpreter. The crate root
//! re-exports the frozen public API: the `Program` type, the `VyreBackend`
//! trait, and the standard operation library.
//!
//! Frontends, backends, and conformance tools depend only on the stable
//! types exported here. Changing the target-text lowering path never breaks a
//! frontend; changing a frontend AST never affects backend dispatch logic.
//! This module is the single source of truth for the vyre public API.

/// The vyre Program model.
///
/// This module defines `Program`, the frozen, serializable model that every
/// frontend emits and every backend consumes. It has zero external
/// dependencies so that spec tools can parse it without pulling in GPU
/// libraries.
/// Public API re-export.
pub use vyre_foundation::ir;

/// Soundness lattice for dataflow primitives. Canonical home is
/// `vyre-foundation`; re-exported here so vyre-libs (and any downstream
/// consumer) reaches it via `vyre::soundness`. Per the LEGO discipline,
/// vyre never imports from domain dataflow crates  -  this is the originating definition.
pub use vyre_foundation::soundness;

// Layer 1 and Layer 2 operation specifications live in vyre-libs.
// The crate root remains the single stable import surface for consumers.

/// Program lowering to the substrate-neutral kernel descriptor.
///
/// Lowering transforms a validated `Program` into
/// [`lower::KernelDescriptor`]. Emit crates then turn that descriptor into
/// target artifacts. Frontends do not depend on this module; it is consumed
/// by backend and emitter implementations.
/// Public API re-export.
pub mod lower {
    /// Canonical Program -> KernelDescriptor lowering entry point.
    pub use vyre_lower::lower::lower;
    pub use vyre_lower::*;
}

/// IR-to-IR optimizer pass framework.
///
/// `optimizer` provides the registered pass scheduler and reference
/// optimization passes used by frontends that want fixpoint IR cleanup before
/// lowering.
/// Public API re-export.
pub use vyre_foundation::optimizer;

/// Wire-format CPU-reference byte ABI contract.
/// Public API re-export.
#[cfg(any(test, feature = "cpu-parity"))]
pub use vyre_foundation::cpu_op;
/// CPU reference implementations shared across backends.
/// Public API re-export.
#[cfg(any(test, feature = "cpu-parity"))]
pub use vyre_foundation::cpu_references;
/// Substrate-neutral memory ordering model.
/// Public API re-export.
pub use vyre_foundation::memory_model;
/// Substrate-neutral memory ordering type.
/// Public API re-export.
pub use vyre_foundation::MemoryOrdering;

/// Distribution-aware runtime algorithm selection.
/// Public API re-export.
pub use vyre_driver::routing;

/// Substrate-neutral execution planning for performance and accuracy tracks.
/// Public API re-export.
pub use vyre_foundation::execution_plan;

/// Unified error types for the entire crate.
/// Public API re-export.
pub use vyre_driver::error;

/// Structured, machine-readable diagnostics.
/// Public API re-export.
pub use vyre_driver::diagnostics;

/// Backend trait surface  -  `VyreBackend`, `Executable`,
/// `Streamable`, `DispatchConfig`, `BackendError`,
/// `ErrorCode`. The whole backend contract every driver crate
/// implements against.
/// Public API re-export.
/// Public API re-export.
pub use vyre_driver::backend;
/// Re-export of the native scan match result type from the foundation crate.
/// Public API re-export.
/// Public API re-export.
pub use vyre_foundation::match_result;

/// Pipeline-mode dispatch: compile a Program once, dispatch repeatedly.
/// Public API re-export.
/// Public API re-export.
pub use vyre_driver::pipeline;

// Previously: pub mod bytecode  -  a 637-LOC stack-machine VM publicly
// re-exported from core. Deleted 2026-04-17. The NFA scan micro-interpreter
// that carried the remaining bytecode was deleted 2026-04-19. Rule evaluators
// compose ops in vyre IR directly. No interpreter surface remains in core.

pub use vyre_driver::{
    BackendError, BackendRegistration, CompiledPipeline, DispatchConfig, Error, Executable, Memory,
    MemoryRef, OutputBuffers, ResidentGraphReuseTelemetry, ResidentGraphReuseTelemetryError,
    TypedDispatchExt, VyreBackend,
};

/// Persistent-thread dispatch policy for dispatch paths.
pub use vyre_driver::persistent::PersistentThreadMode;
/// Speculation policy for dispatch paths.
pub use vyre_driver::speculate::SpeculationMode;

/// Re-export of the core IR program type and validation entry point.
///
/// `Program` is the frozen IR container. `validate` is the function that
/// checks a program for structural and semantic correctness before it is
/// handed to a backend.
pub use ir::{validate, InterpCtx, NodeId, NodeStorage, OpId, Program, Value};

/// Re-export of the native scan match result type.
///
/// `Match` represents a byte-range hit produced by pattern-scanning engines.
pub use vyre_foundation::match_result::Match;

/// Domain-neutral byte-range type.
pub use vyre_foundation::ByteRange;

/// R2: single canonical pre-lowering optimize entry point.
///
/// Bundles the canonical pre-lowering pipeline so every consumer wires one
/// function instead of three. Today consumers separately call
/// `pre_lowering::optimize`, then `vyre_lower::lower`, then a
/// backend-specific emit. This wrapper keeps the optimization stage  -
/// the part that's stable across backends  -  behind one symbol so
/// adding a new substrate row does not require N consumer changes.
///
/// The lowering and emit stages remain backend-specific and are
/// invoked separately by the chosen `VyreBackend`. This function
/// returns the optimized `Program` ready to hand to any backend's
/// `dispatch` / `compile` path.
///
/// **N9 substrate composition fingerprint cache.** Repeated identical
/// inputs (same `program.fingerprint()`) skip the substrate stack
/// entirely. The cache is process-local, bounded to
/// [`OPTIMIZE_CACHE_CAPACITY`] entries, and uses O(1) fingerprint lookup
/// with FIFO eviction  -  long-running daemons get the cache without
/// unbounded memory.
/// On a cache hit, `optimize` clones the cached `Program` instead of
/// re-running the (canonicalize + region_inline + scheduler fixpoint
/// + CSE + DCE + phase-4) pipeline. The substrate stack is purely
/// functional in `Program`, so caching by structural fingerprint is
/// safe  -  same input bytes, same output bytes.
///
/// # Example
///
/// ```no_run
/// use vyre::{optimize, Program};
/// fn run(program: Program) -> Program {
///     optimize(program)
/// }
/// ```
#[must_use]
pub fn optimize(program: Program) -> Result<Program, vyre_foundation::optimizer::OptimizerError> {
    let key = program.fingerprint();
    if let Some(cached) = optimize_cache::get(&key) {
        return Ok(cached);
    }
    // Use try_optimize so scheduler failures (non-convergence, bad pass
    // metadata) propagate as structured errors rather than silently returning
    // an un-optimized program. This makes the Err variant of this function's
    // Result type reachable for the first time.
    let optimized = vyre_foundation::optimizer::pre_lowering::try_optimize(program)?;
    optimize_cache::put(key, &optimized);
    Ok(optimized)
}

/// Device-aware public optimizer entry point.
///
/// Runs adapter-shaped workgroup autotuning from a neutral
/// [`DeviceProfile`] before the canonical pre-lowering optimization
/// pipeline. Consumers with a live backend should prefer
/// [`optimize_for_backend`]; consumers with a saved device signature
/// can call this directly.
#[must_use]
pub fn optimize_for_device(
    program: Program,
    profile: &vyre_driver::DeviceProfile,
) -> Result<Program, vyre_foundation::optimizer::OptimizerError> {
    let key = device_optimize_key(&program, profile);
    if let Some(cached) = optimize_cache::get_device(&key) {
        return Ok(cached);
    }
    let tuned = vyre_foundation::optimizer::passes::autotune::Autotune::transform_for_adapter(
        program,
        &profile.adapter_caps(),
    )
    .program;
    let optimized = optimize(tuned)?;
    optimize_cache::put_device(key, &optimized);
    Ok(optimized)
}

/// Device-aware public optimizer entry point for a live backend.
#[must_use]
pub fn optimize_for_backend(
    program: Program,
    backend: &dyn vyre_driver::VyreBackend,
) -> Result<Program, vyre_foundation::optimizer::OptimizerError> {
    let profile = backend.device_profile();
    optimize_for_device(program, &profile)
}

fn device_optimize_key(program: &Program, profile: &vyre_driver::DeviceProfile) -> [u8; 32] {
    // Hash ALL fields consumed by adapter_caps() (device_profile.rs:213-235).
    // Any field that influences Autotune::transform_for_adapter or other
    // per-adapter passes MUST be included here, otherwise two calls with
    // programs of the same fingerprint but different adapter profiles can
    // collide on the same cache key and the second call silently gets the
    // first profile's optimized IR with the wrong workgroup size / tile / etc.
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"vyre-core-optimize-device-v2\0"); // version bump: key now covers all adapter fields
    hasher.update(&program.fingerprint());
    hasher.update(profile.backend.as_bytes());
    hasher.update(&[u8::from(profile.supports_subgroup_ops)]);
    hasher.update(&[u8::from(profile.supports_indirect_dispatch)]);
    hasher.update(&[u8::from(profile.supports_f16)]);
    hasher.update(&[u8::from(profile.supports_bf16)]);
    hasher.update(&[u8::from(profile.supports_tensor_cores)]);
    hasher.update(&[u8::from(profile.supports_specialization_constants)]);
    hasher.update(&profile.max_workgroup_size[0].to_le_bytes());
    hasher.update(&profile.max_workgroup_size[1].to_le_bytes());
    hasher.update(&profile.max_workgroup_size[2].to_le_bytes());
    hasher.update(&profile.max_invocations_per_workgroup.to_le_bytes());
    hasher.update(&profile.max_shared_memory_bytes.to_le_bytes());
    hasher.update(&profile.max_storage_buffer_binding_size.to_le_bytes());
    hasher.update(&profile.subgroup_size.to_le_bytes());
    hasher.update(&profile.compute_units.to_le_bytes());
    hasher.update(&profile.regs_per_thread_max.to_le_bytes());
    hasher.update(&profile.l1_cache_bytes.to_le_bytes());
    hasher.update(&profile.l2_cache_bytes.to_le_bytes());
    hasher.update(&profile.mem_bw_gbps.to_le_bytes());
    hasher.update(&profile.ideal_unroll_depth.to_le_bytes());
    hasher.update(&profile.ideal_vector_pack_bits.to_le_bytes());
    hasher.update(&profile.ideal_workgroup_tile[0].to_le_bytes());
    hasher.update(&profile.ideal_workgroup_tile[1].to_le_bytes());
    hasher.update(&profile.ideal_workgroup_tile[2].to_le_bytes());
    hasher.update(&profile.shared_memory_bank_count.to_le_bytes());
    hasher.update(&profile.shared_memory_bank_width_bytes.to_le_bytes());
    *hasher.finalize().as_bytes()
}

/// N9 cache capacity (entries). Sized to hold the working set of a
/// long-running scanner without unbounded growth  -  each entry is
/// roughly the size of one optimized `Program`. 256 entries is
/// `~10MB` worst-case for typical scanner-shaped Programs.
pub const OPTIMIZE_CACHE_CAPACITY: usize = 256;

/// Process-local fingerprint -> Program cache for [`optimize`].
mod optimize_cache {
    use super::Program;
    use super::OPTIMIZE_CACHE_CAPACITY;
    use std::collections::{HashMap, VecDeque};
    use std::sync::Mutex;

    struct ProgramCacheShard {
        entries: HashMap<[u8; 32], Program>,
        fifo: VecDeque<[u8; 32]>,
    }

    impl ProgramCacheShard {
        fn new() -> Self {
            Self {
                entries: HashMap::with_capacity(OPTIMIZE_CACHE_CAPACITY),
                fifo: VecDeque::with_capacity(OPTIMIZE_CACHE_CAPACITY),
            }
        }

        fn get(&self, key: &[u8; 32]) -> Option<Program> {
            self.entries.get(key).cloned()
        }

        fn put(&mut self, key: [u8; 32], program: &Program) {
            if self.entries.contains_key(&key) {
                return;
            }
            if self.entries.len() >= OPTIMIZE_CACHE_CAPACITY {
                if let Some(evicted) = self.fifo.pop_front() {
                    self.entries.remove(&evicted);
                }
            }
            self.fifo.push_back(key);
            self.entries.insert(key, program.clone());
        }

        #[cfg(test)]
        fn clear(&mut self) {
            self.entries.clear();
            self.fifo.clear();
        }

        #[cfg(test)]
        fn len(&self) -> usize {
            self.entries.len()
        }
    }

    struct Cache {
        host: ProgramCacheShard,
        device: ProgramCacheShard,
    }

    impl Cache {
        fn new() -> Self {
            Self {
                host: ProgramCacheShard::new(),
                device: ProgramCacheShard::new(),
            }
        }
    }

    fn cache() -> &'static Mutex<Cache> {
        use std::sync::OnceLock;
        static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();
        CACHE.get_or_init(|| Mutex::new(Cache::new()))
    }

    pub(super) fn get(key: &[u8; 32]) -> Option<Program> {
        // Recover from mutex poison: the poisoned thread already panicked and
        // is gone; the cache data it held is still structurally valid. Silently
        // returning None (the old `.ok()?` path) caused infinite re-optimization
        // in a recovered daemon with no operator-visible signal. Recovery
        // preserves cache correctness; if the state were corrupt, Put would
        // have been the corrupt write and the subsequent Get would simply miss.
        let cache = cache()
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        cache.host.get(key)
    }

    pub(super) fn put(key: [u8; 32], program: &Program) {
        // Same recovery rationale as get(): silently skipping the write on
        // poison caused every subsequent call to pay the full optimization
        // cost forever. Recovering the guard lets us still store the result.
        let mut cache = cache()
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        cache.host.put(key, program);
    }

    pub(super) fn get_device(key: &[u8; 32]) -> Option<Program> {
        let cache = cache()
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        cache.device.get(key)
    }

    pub(super) fn put_device(key: [u8; 32], program: &Program) {
        let mut cache = cache()
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        cache.device.put(key, program);
    }

    #[cfg(test)]
    pub(super) fn clear() {
        let mut cache = cache().lock().unwrap_or_else(|e| e.into_inner());
        cache.host.clear();
        cache.device.clear();
    }

    #[cfg(test)]
    pub(super) fn len() -> usize {
        cache().lock().unwrap_or_else(|e| e.into_inner()).host.len()
    }

    #[cfg(test)]
    pub(super) fn len_device() -> usize {
        cache().lock().unwrap_or_else(|e| e.into_inner()).device.len()
    }

    /// Single process-wide serialization guard shared by EVERY cache test,
    /// across all test modules. The optimize cache is a process-global
    /// singleton; any test that asserts on `len()`/`len_device()` or relies on
    /// `clear()` must hold this guard so a concurrent test in another module
    /// cannot insert/evict between the clear and the assertion. A per-module
    /// mutex is insufficient — two modules with separate mutexes still race on
    /// the one shared cache (observed: device len 5≠1, 248≠256).
    #[cfg(test)]
    pub(super) fn test_serial() -> std::sync::MutexGuard<'static, ()> {
        use std::sync::{Mutex, OnceLock};
        static M: OnceLock<Mutex<()>> = OnceLock::new();
        M.get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|e| e.into_inner())
    }
}

#[cfg(test)]
mod optimize_tests {
    use super::*;
    use std::sync::MutexGuard;
    use vyre_foundation::ir::{BufferDecl, DataType, Expr, Node};

    /// Serialise cache tests against the process-global singleton. Delegates to
    /// the ONE shared guard in `optimize_cache` so this module mutually excludes
    /// with `optimize_cache_runtime_tests` too — a per-module mutex would let
    /// the other module's eviction-fill race this module's count assertions.
    fn serial() -> MutexGuard<'static, ()> {
        optimize_cache::test_serial()
    }

    fn sample_program() -> Program {
        Program::wrapped(
            vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
            [1, 1, 1],
            vec![Node::store("out", Expr::u32(0), Expr::u32(42))],
        )
    }

    /// Return true iff the program's entry contains a Store of exactly u32
    /// literal `expected_value` into the "out" buffer — recursing through
    /// `Region` wrappers. `Program::wrapped` nests the body inside a
    /// `vyre.program.root` Region, so the Store is never a top-level entry
    /// node; a non-recursive scan false-negatives on every correctly
    /// optimized program.
    fn entry_stores_literal(program: &Program, expected_value: u32) -> bool {
        use vyre_foundation::ir::Node;
        fn node_stores_literal(n: &Node, expected_value: u32) -> bool {
            match n {
                Node::Store {
                    value: Expr::LitU32(v),
                    ..
                } => *v == expected_value,
                Node::Region { body, .. } => body
                    .iter()
                    .any(|child| node_stores_literal(child, expected_value)),
                _ => false,
            }
        }
        program
            .entry()
            .iter()
            .any(|n| node_stores_literal(n, expected_value))
    }

    #[test]
    fn optimize_is_cached_by_fingerprint() {
        let _g = serial();
        optimize_cache::clear();
        let p1 = sample_program();
        let p2 = sample_program();
        let first = optimize(p1).expect("Fix: optimize must succeed on sample_program");
        // sample_program stores LitU32(42) — after optimization the store must
        // still be present with the correct value, not just be non-empty.
        assert!(
            entry_stores_literal(&first, 42),
            "optimized sample_program must contain a Store of LitU32(42): {:?}",
            first.entry()
        );
        let before = optimize_cache::len();
        let second = optimize(p2).expect("Fix: optimize must succeed on cache-hit path");
        assert!(
            entry_stores_literal(&second, 42),
            "cache-hit optimized sample_program must contain a Store of LitU32(42): {:?}",
            second.entry()
        );
        let after = optimize_cache::len();
        assert_eq!(
            before, after,
            "second optimize on identical fingerprint must hit the cache"
        );
        assert_eq!(before, 1, "cache must contain exactly one entry");
    }

    #[test]
    fn optimize_returns_equivalent_program_on_cache_hit() {
        let _g = serial();
        optimize_cache::clear();
        let p = sample_program();
        let first = optimize(p.clone()).expect("Fix: optimize must succeed on sample_program");
        let second = optimize(p).expect("Fix: optimize must succeed on cache-hit path");
        assert_eq!(
            first.fingerprint(),
            second.fingerprint(),
            "cache hit must return a Program with identical fingerprint"
        );
    }

    fn program_with_literal(value: u32) -> Program {
        Program::wrapped(
            vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
            [1, 1, 1],
            vec![Node::store("out", Expr::u32(0), Expr::u32(value))],
        )
    }

    #[test]
    fn optimize_cache_evicts_at_capacity() {
        let _g = serial();
        optimize_cache::clear();

        // Build OPTIMIZE_CACHE_CAPACITY + 1 distinct programs by varying the
        // stored literal — each gets a unique fingerprint. Capture the key of
        // the first-inserted program to verify FIFO eviction.
        let first_key = {
            let p0 = program_with_literal(0);
            let key = p0.fingerprint();
            let opt = optimize(p0).expect("Fix: optimize must succeed on first eviction probe");
            assert!(
                entry_stores_literal(&opt, 0),
                "optimize of literal-0 program must preserve LitU32(0): {:?}",
                opt.entry()
            );
            key
        };

        for i in 1..=(OPTIMIZE_CACHE_CAPACITY) {
            let prog = program_with_literal(i as u32);
            let optimized =
                optimize(prog).expect("Fix: optimize must succeed on cache-eviction probe");
            assert!(
                entry_stores_literal(&optimized, i as u32),
                "optimized program must store literal {i}: {:?}",
                optimized.entry()
            );
        }

        assert_eq!(
            optimize_cache::len(),
            OPTIMIZE_CACHE_CAPACITY,
            "cache must cap at OPTIMIZE_CACHE_CAPACITY entries"
        );

        // FIFO: the first-inserted entry (literal 0) must be evicted.
        assert!(
            optimize_cache::get(&first_key).is_none(),
            "Fix: FIFO eviction must have removed the first-inserted entry (literal 0)"
        );

        // The last-inserted entry must survive.
        let last_key = program_with_literal(OPTIMIZE_CACHE_CAPACITY as u32).fingerprint();
        assert!(
            optimize_cache::get(&last_key).is_some(),
            "Fix: last-inserted entry (literal {OPTIMIZE_CACHE_CAPACITY}) must survive eviction"
        );

        // Cache hit for a surviving entry must return the correct program.
        let cached = optimize_cache::get(&last_key)
            .expect("Fix: surviving cache entry must be retrievable");
        assert!(
            entry_stores_literal(&cached, OPTIMIZE_CACHE_CAPACITY as u32),
            "cached surviving program must store the correct literal {OPTIMIZE_CACHE_CAPACITY}: {:?}",
            cached.entry()
        );
    }

    fn program_with_buffer_and_workgroup_1_1_1() -> Program {
        // A 1D kernel with a large buffer — Autotune will upscale the
        // workgroup to max_invocations_per_workgroup for this shape.
        Program::wrapped(
            vec![BufferDecl::output("out", 0, DataType::U32).with_count(4096)],
            [1, 1, 1],
            vec![Node::store("out", Expr::u32(0), Expr::u32(1))],
        )
    }

    #[test]
    fn optimize_for_device_uses_device_specific_cache() {
        let _g = serial();
        optimize_cache::clear();
        let mut profile = vyre_driver::DeviceProfile::conservative("test");
        profile.max_workgroup_size = [256, 1, 1];
        profile.max_invocations_per_workgroup = 256;
        let p1 = sample_program();
        let p2 = sample_program();
        let first =
            optimize_for_device(p1, &profile).expect("Fix: optimize_for_device must succeed");
        let second = optimize_for_device(p2, &profile)
            .expect("Fix: optimize_for_device must succeed on cache hit");
        assert_eq!(first.fingerprint(), second.fingerprint());
        assert_eq!(
            optimize_cache::len_device(),
            1,
            "same program+device profile must hit the device optimize cache"
        );
        assert_eq!(
            optimize_cache::len(),
            1,
            "device optimization should still reuse the canonical optimize cache after tuning"
        );
    }

    #[test]
    fn optimize_for_device_different_ideal_workgroup_tile_produces_different_cached_program() {
        // Regression test for the missing `ideal_workgroup_tile` field in
        // device_optimize_key: two profiles that differ only in
        // ideal_workgroup_tile must produce different cache keys and different
        // optimized programs. Before the fix, both calls returned the first
        // profile's optimized IR — silently wrong workgroup size.
        let _g = serial();
        optimize_cache::clear();

        let p = program_with_buffer_and_workgroup_1_1_1();

        let mut compact = vyre_driver::DeviceProfile::conservative("test");
        compact.max_workgroup_size = [256, 256, 64];
        compact.max_invocations_per_workgroup = 256;
        compact.subgroup_size = 32;
        compact.ideal_workgroup_tile = [8, 8, 1];

        let mut wide = compact.clone();
        wide.ideal_workgroup_tile = [16, 16, 1];

        let r_compact = optimize_for_device(p.clone(), &compact)
            .expect("Fix: optimize_for_device with compact tile must succeed");
        let r_wide = optimize_for_device(p.clone(), &wide)
            .expect("Fix: optimize_for_device with wide tile must succeed");

        assert_ne!(
            r_compact.fingerprint(),
            r_wide.fingerprint(),
            "Fix: different ideal_workgroup_tile must produce different optimized programs \
             (different cache keys); if this fails, device_optimize_key is missing \
             ideal_workgroup_tile[0..2]"
        );

        // Autotune with compact [8,8,1] tile → workgroup [64,1,1];
        // with wide [16,16,1] tile → workgroup [256,1,1].
        // (These values match the existing autotune.rs test at line 433-434.)
        assert_eq!(
            r_compact.workgroup_size(),
            [64, 1, 1],
            "compact ideal_workgroup_tile [8,8,1] must autotune to workgroup [64,1,1]"
        );
        assert_eq!(
            r_wide.workgroup_size(),
            [256, 1, 1],
            "wide ideal_workgroup_tile [16,16,1] must autotune to workgroup [256,1,1]"
        );
    }
}

#[cfg(test)]
mod optimize_cache_runtime_tests {
    use super::*;
    use std::sync::MutexGuard;
    use vyre_foundation::ir::{BufferDecl, DataType, Expr, Node};

    /// Delegates to the ONE shared cache-test guard so this module's
    /// CAPACITY+1 eviction fills mutually exclude with `optimize_tests`'
    /// exact-count assertions on the same process-global cache.
    fn serial() -> MutexGuard<'static, ()> {
        optimize_cache::test_serial()
    }

    fn program_with_literal(value: u32) -> Program {
        Program::wrapped(
            vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
            [1, 1, 1],
            vec![Node::store("out", Expr::u32(0), Expr::u32(value))],
        )
    }

    #[test]
    fn host_cache_fifo_eviction() {
        // Replaces the source-text assertion test: verifies runtime FIFO
        // eviction behavior of the shared ProgramCacheShard used by both
        // host and device caches.
        let _g = serial();
        optimize_cache::clear();

        // Fill the host cache exactly to capacity.
        let first_key = program_with_literal(0).fingerprint();
        for i in 0..=OPTIMIZE_CACHE_CAPACITY {
            let p = program_with_literal(i as u32);
            optimize(p).expect("Fix: optimize must succeed filling host cache");
        }

        assert_eq!(
            optimize_cache::len(),
            OPTIMIZE_CACHE_CAPACITY,
            "host cache must stay at capacity after inserting CAPACITY+1 entries"
        );
        assert!(
            optimize_cache::get(&first_key).is_none(),
            "Fix: FIFO eviction must remove the first-inserted host cache entry"
        );
    }

    #[test]
    fn device_cache_fifo_eviction() {
        let _g = serial();
        optimize_cache::clear();

        let mut profile = vyre_driver::DeviceProfile::conservative("test");
        profile.max_workgroup_size = [256, 1, 1];
        profile.max_invocations_per_workgroup = 256;

        let first_key = {
            let p = program_with_literal(0);
            let key = super::device_optimize_key(&p, &profile);
            optimize_for_device(p, &profile)
                .expect("Fix: optimize_for_device must succeed filling device cache");
            key
        };

        for i in 1..=OPTIMIZE_CACHE_CAPACITY {
            let p = program_with_literal(i as u32);
            optimize_for_device(p, &profile)
                .expect("Fix: optimize_for_device must succeed in device eviction fill");
        }

        assert_eq!(
            optimize_cache::len_device(),
            OPTIMIZE_CACHE_CAPACITY,
            "device cache must stay at capacity after inserting CAPACITY+1 entries"
        );
        assert!(
            optimize_cache::get_device(&first_key).is_none(),
            "Fix: FIFO eviction must remove the first-inserted device cache entry"
        );
    }

    #[test]
    fn host_and_device_caches_are_independent() {
        // A host-cache hit for key K must not satisfy a device-cache lookup
        // for key K (the two shards are separate; using the same backing
        // structure does not mean the same logical space).
        let _g = serial();
        optimize_cache::clear();

        let p = program_with_literal(99);
        let host_key = p.fingerprint();
        optimize(p.clone()).expect("Fix: optimize must populate host cache");

        // Only the host cache should have the entry; the device cache is empty.
        assert!(
            optimize_cache::get(&host_key).is_some(),
            "Fix: host cache must contain the entry just inserted"
        );
        assert_eq!(
            optimize_cache::len_device(),
            0,
            "device cache must be empty when only host-optimize was called"
        );
    }
}
