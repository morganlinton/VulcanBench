#![allow(
    clippy::doc_lazy_continuation,
    clippy::double_must_use,
    clippy::manual_div_ceil,
    clippy::needless_range_loop,
    clippy::collapsible_if,
    clippy::match_like_matches_macro,
    clippy::redundant_closure,
    clippy::too_many_arguments,
    clippy::nonminimal_bool,
    clippy::derivable_impls,
    clippy::unnecessary_lazy_evaluations,
    clippy::needless_lifetimes,
    clippy::bind_instead_of_map,
    clippy::needless_borrows_for_generic_args,
    clippy::map_entry,
    clippy::map_identity,
    clippy::manual_map,
    clippy::match_single_binding,
    clippy::field_reassign_with_default,
    dead_code,
    unused_variables
)]
//! Naga IR emitter for vyre `KernelDescriptor`.
//!
//! Consumes a substrate-neutral `vyre_lower::KernelDescriptor` and
//! produces a `naga::Module`. The emitter owns only Naga construction;
//! descriptor shaping and substrate-neutral analyses stay in
//! `vyre-lower`.

use std::collections::VecDeque;
use std::sync::{mpsc, Mutex, MutexGuard, OnceLock};

use rustc_hash::FxHashMap;
use vyre_lower::KernelDescriptor;

mod emitter;
mod error;
pub mod patterns;
pub mod program;
pub use error::EmitError;
pub use vyre_lower;

/// Stable diagnostic row emitted when binding a lowered Vyre operation into a
/// Naga module.
#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct BindResultEntry {
    /// Stable numeric operation id assigned by Vyre lowering.
    pub vyre_op_id: u32,
    /// Lowered operation kind used by the emitter dispatch table.
    pub op_kind: String,
    /// Naga expression or handle id used for the initial value path.
    pub init_handle: u32,
    /// Scalar kind attached to the initial value when one is known.
    pub init_scalar_kind: Option<String>,
    /// Nesting depth of the child body that produced this bind row.
    pub child_body_depth: usize,
    /// Bit-packed value-type summary observed at the call boundary.
    pub value_types_at_call: Option<u32>,
    /// Human-readable path describing where the value was published.
    pub publish_path: String,
    /// Allocated local type id when the bind operation materialized local storage.
    pub local_allocated_ty: Option<u32>,
}

const MODULE_CACHE_CAPACITY: usize = 64;
static MODULE_CACHE: OnceLock<Mutex<ModuleCache>> = OnceLock::new();

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct ModuleCacheKey([u8; 16]);

#[derive(Clone)]
struct CachedModule {
    descriptor: KernelDescriptor,
    module: naga::Module,
}

#[derive(Default)]
struct ModuleCache {
    entries: FxHashMap<ModuleCacheKey, CachedModule>,
    order: VecDeque<ModuleCacheKey>,
    #[cfg(test)]
    hits: usize,
}

impl ModuleCache {
    fn get(&mut self, key: ModuleCacheKey, desc: &KernelDescriptor) -> Option<naga::Module> {
        let cached = self.entries.get(&key)?;
        if cached.descriptor != *desc {
            return None;
        }
        #[cfg(test)]
        {
            self.hits += 1;
        }
        Some(cached.module.clone())
    }

    fn insert(&mut self, key: ModuleCacheKey, desc: &KernelDescriptor, module: &naga::Module) {
        if self.entries.contains_key(&key) {
            self.entries.insert(
                key,
                CachedModule {
                    descriptor: desc.clone(),
                    module: module.clone(),
                },
            );
            return;
        }
        if self.entries.len() >= MODULE_CACHE_CAPACITY {
            if let Some(oldest) = self.order.pop_front() {
                self.entries.remove(&oldest);
            }
        }
        self.order.push_back(key);
        self.entries.insert(
            key,
            CachedModule {
                descriptor: desc.clone(),
                module: module.clone(),
            },
        );
    }
}

fn module_cache() -> &'static Mutex<ModuleCache> {
    MODULE_CACHE.get_or_init(|| Mutex::new(ModuleCache::default()))
}

fn lock_module_cache() -> MutexGuard<'static, ModuleCache> {
    match module_cache().lock() {
        Ok(guard) => guard,
        Err(error) => {
            let mut guard = error.into_inner();
            *guard = ModuleCache::default();
            guard
        }
    }
}

/// A `std::hash::Hasher` adapter that feeds bytes directly into a
/// `blake3::Hasher`. This lets us derive cache keys from the `Hash` impl of
/// `KernelDescriptor`, which already handles `LiteralValue::F32` by bit
/// pattern (`to_bits()`). Using `format!("{desc:?}")` here would collapse
/// all NaN variants to the same Debug string "NaN", causing two descriptors
/// with distinct NaN bit patterns to produce the same cache key but compare
/// unequal — either a spurious miss (NaN != NaN via PartialEq) or a spurious
/// hit (NaN1 and NaN2 with identical bit patterns via some future
/// bit-exact PartialEq). The Hash impl is the correct oracle.
struct Blake3StdHasher(blake3::Hasher);

impl std::hash::Hasher for Blake3StdHasher {
    fn write(&mut self, bytes: &[u8]) {
        self.0.update(bytes);
    }

    fn finish(&self) -> u64 {
        // The truncated u64 is never used as the final key; only the
        // `blake3::Hasher`'s internal state matters. Return a dummy value.
        0
    }
}

fn descriptor_cache_key(desc: &KernelDescriptor) -> ModuleCacheKey {
    use std::hash::Hash;
    // Hash via the `Hash` impl, not Debug output. The `LiteralValue::Hash`
    // impl already feeds f32/f64 as raw bits, so NaN bit-patterns are
    // distinguished correctly. This avoids the Debug-collapse-to-"NaN" bug
    // that caused spurious cache misses for NaN-containing descriptors.
    let mut std_hasher = Blake3StdHasher(blake3::Hasher::new());
    desc.hash(&mut std_hasher);
    let digest = std_hasher.0.finalize();
    let mut out = [0u8; 16];
    out.copy_from_slice(&digest.as_bytes()[..16]);
    ModuleCacheKey(out)
}

#[cfg(test)]
fn clear_module_cache_for_tests() {
    *lock_module_cache() = ModuleCache::default();
}

#[cfg(test)]
fn module_cache_hits_for_tests() -> usize {
    lock_module_cache().hits
}

/// Emit a `naga::Module` from a `KernelDescriptor` after running the
/// full `vyre_lower::rewrites::run_all` optimization pipeline.
///
/// This is the recommended emission entry point  -  call this whenever
/// you don't have a specific reason to emit the raw descriptor. The
/// optimized form has fewer ops (dead code dropped, identity ops
/// eliminated, common subexpressions merged, redundant loads
/// forwarded, etc.) and produces tighter Naga IR with no semantic
/// change.
///
/// # Errors
///
/// Same as [`emit`].
pub fn emit_optimized(desc: &KernelDescriptor) -> Result<naga::Module, EmitError> {
    emit_optimized_with_stats(desc).map(|(m, _)| m)
}

/// Like [`emit_optimized`] but also returns
/// [`vyre_lower::rewrites::OptimizationStats`] so the caller can see
/// what the rewrite stack did (op count delta, bindings dropped,
/// fixed-point iterations needed). No duplicate work  -  `emit_optimized`
/// is now a thin wrapper around this.
pub fn emit_optimized_with_stats(
    desc: &KernelDescriptor,
) -> Result<(naga::Module, vyre_lower::rewrites::OptimizationStats), EmitError> {
    // Verify the INPUT descriptor before the rewrite pipeline. Each rewrite
    // pass assumes a valid descriptor and, in debug builds, `assert!`s validity
    // after every pass (`debug_verify_after_rewrite`); feeding an already-invalid
    // descriptor would therefore panic inside a pass rather than surface as a
    // structured error. Fail closed here so invalid input is rejected identically
    // in debug and release, before any pass runs.
    vyre_lower::verify::verify(desc).map_err(|errors| {
        EmitError::InvalidDescriptor(format!(
            "invalid descriptor: input failed verification before optimization — {} error(s). \
             Fix: see vyre_lower::verify for the invariants the descriptor violated. \
             First error: {:?}",
            errors.len(),
            errors.first()
        ))
    })?;
    let (optimized, stats) = vyre_lower::rewrites::run_all_with_stats(desc);
    // Unconditionally verify the rewrite pipeline output. A `debug_assert!`
    // here silently skips this gate in release builds, where a buggy rewrite
    // could produce an invalid descriptor that proceeds to Naga emission and
    // yields a SPIR-V binary with different semantics from the original — a
    // silently-wrong result. Fail closed with a structured error instead.
    vyre_lower::verify::verify(&optimized).map_err(|errors| {
        EmitError::InvalidDescriptor(format!(
            "rewrite pipeline produced an invalid descriptor — {} error(s). \
             Fix: see vyre_lower::verify for the invariants the rewrite violated. \
             First error: {:?}",
            errors.len(),
            errors.first()
        ))
    })?;
    let module = emit(&optimized)?;
    Ok((module, stats))
}

/// Emit many independent descriptors after running the canonical lower rewrite
/// pipeline on each descriptor.
///
/// Results preserve input order. Each descriptor still flows through the
/// process-wide module cache, so repeated arms return cached `naga::Module`
/// clones while unrelated arms can lower concurrently.
#[must_use]
pub fn emit_many_optimized(descs: &[KernelDescriptor]) -> Vec<Result<naga::Module, EmitError>> {
    emit_many_with(descs, emit_optimized)
}

/// Emit a `naga::Module` from a `KernelDescriptor`.
///
/// Lowers the descriptor exactly as given. Use [`emit_optimized`] if
/// you also want the rewrite stack applied first.
///
/// # Errors
///
/// Returns [`EmitError`] when a binding layout cannot be represented in
/// Naga IR or when the descriptor contains an operation outside this emitter's
/// supported lowering set.
pub fn emit(desc: &KernelDescriptor) -> Result<naga::Module, EmitError> {
    let cache_key = descriptor_cache_key(desc);
    if let Some(module) = lock_module_cache().get(cache_key, desc) {
        return Ok(module);
    }
    let module = emitter::emit_uncached(desc)?;
    lock_module_cache().insert(cache_key, desc, &module);
    Ok(module)
}

/// Emit many independent descriptors exactly as provided.
///
/// Results preserve input order and each descriptor uses the same cache path as
/// [`emit`]. Use [`emit_many_optimized`] for the canonical optimized path.
#[must_use]
pub fn emit_many(descs: &[KernelDescriptor]) -> Vec<Result<naga::Module, EmitError>> {
    emit_many_with(descs, emit)
}

fn emit_many_with(
    descs: &[KernelDescriptor],
    emit_one: fn(&KernelDescriptor) -> Result<naga::Module, EmitError>,
) -> Vec<Result<naga::Module, EmitError>> {
    if descs.len() <= 1 {
        return descs.iter().map(emit_one).collect();
    }
    let worker_count = std::thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(1)
        .min(descs.len())
        .max(1);
    let chunk_size = descs.len().div_ceil(worker_count);
    let (tx, rx) = mpsc::channel();
    std::thread::scope(|scope| {
        for (chunk_index, chunk) in descs.chunks(chunk_size).enumerate() {
            let tx = tx.clone();
            let start = chunk_index * chunk_size;
            scope.spawn(move || {
                for (offset, desc) in chunk.iter().enumerate() {
                    if tx.send((start + offset, emit_one(desc))).is_err() {
                        break;
                    }
                }
            });
        }
    });
    drop(tx);

    let mut results: Vec<Option<Result<naga::Module, EmitError>>> =
        std::iter::repeat_with(|| None).take(descs.len()).collect();
    for (index, result) in rx {
        if let Some(slot) = results.get_mut(index) {
            *slot = Some(result);
        }
    }
    results
        .into_iter()
        .enumerate()
        .map(|(index, result)| {
            result.unwrap_or_else(|| {
                Err(EmitError::InvalidDescriptor(format!(
                    "parallel Naga emit worker did not return descriptor index {index}. Fix: keep emit_many chunk scheduling and result collection synchronized."
                )))
            })
        })
        .collect()
}

#[cfg(test)]
mod tests;
