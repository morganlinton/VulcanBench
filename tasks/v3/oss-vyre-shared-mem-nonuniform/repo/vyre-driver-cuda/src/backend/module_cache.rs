//! CUDA module cache: PTX text to loaded `CUfunction` lookup.

use std::cell::RefCell;
use std::ffi::CStr;
use std::hash::BuildHasherDefault;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;

use cudarc::driver::sys::{CUfunction, CUmodule, CUresult};
use dashmap::mapref::entry::Entry;
use dashmap::DashMap;
use rustc_hash::FxHasher;
use smallvec::SmallVec;
use vyre_driver::accounting::{
    checked_add_usize_lazy, checked_atomic_add_usize_with_order, pinning_atomic_increment_u32,
    pinning_atomic_increment_u64, rebasing_atomic_next_u64,
};
use vyre_driver::input_identity::domain_separated_exact_input_key;
use vyre_driver::{BackendError, DispatchConfig};
use vyre_foundation::ir::Program;

use super::staging_reserve::{reserve_smallvec, reserve_vec};
use crate::backend::accounting::checked_sub_usize;

/// Soft cap on loaded CUDA modules. Eviction drops the cache to half-capacity.
const MODULE_CACHE_SOFT_CAP: usize = 2048;
const MODULE_CACHE_RETAIN_AFTER_EVICTION: usize = MODULE_CACHE_SOFT_CAP / 2;
/// Soft cap on lowered PTX source strings retained before module loading.
const PTX_SOURCE_CACHE_SOFT_CAP: usize = 512;
const PTX_SOURCE_CACHE_RETAIN_AFTER_EVICTION: usize = PTX_SOURCE_CACHE_SOFT_CAP / 2;
const PTX_SOURCE_CACHE_SOFT_BYTES: usize = 256 * 1024 * 1024;
const PTX_SOURCE_CACHE_MAX_ARTIFACT_BYTES: u64 = 1024 * 1024 * 1024;
const PTX_LOWERING_CONTRACT: &[u8] =
    b"vyre-cuda-ptx-lowering-contract:v14:ssa-carrier-snapshots+f32-canonical+select-pred-normalization+bool-cast-boundary+f32-bool-nan-truthiness+bool-numeric-materialization+bool-memory-word-abi+f32-ne-unordered+masked-integer-shifts+no-mutable-loop-unroll+full-workgroup-entry+bounded-full-workgroup-stores+child-captured-producer-liveness+single-mad-dual-mul-liveness";
const CUDA_PTX_SOURCE_FROM_PROGRAM_DOMAIN: &[u8] = b"vyre.cuda.ptx-source-cache.program.v1";
const CUDA_MODULE_FROM_PTX_SOURCE_KEY_DOMAIN: &[u8] = b"vyre.cuda.module-cache.ptx-source-key.v1";
const CUDA_MODULE_FROM_RAW_PTX_ARTIFACT_DOMAIN: &[u8] =
    b"vyre.cuda.module-cache.raw-ptx-artifact.v1";
static PTX_CACHE_TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

thread_local! {
    static PTX_CSTR_SCRATCH: RefCell<Vec<u8>> = RefCell::new(Vec::new());
}

/// Stable key for one PTX module on one CUDA architecture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct ModuleCacheKey(pub(crate) [u8; 32]);

/// Stable key for cached PTX source before CUDA module loading.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct PtxSourceCacheKey([u8; 32]);

impl PtxSourceCacheKey {
    pub(crate) fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

fn vsa_fingerprint_cache_bytes(words: [u32; 8]) -> [u8; 32] {
    let mut bytes = [0u8; 32];
    for (index, word) in words.iter().enumerate() {
        let offset = index * std::mem::size_of::<u32>();
        bytes[offset..offset + std::mem::size_of::<u32>()].copy_from_slice(&word.to_le_bytes());
    }
    bytes
}

fn ptx_source_cache_key_from_program_identity(
    program: &Program,
    config: &DispatchConfig,
    ptx_target_sm: u32,
    subgroup_size: u32,
    feature_flags: vyre_driver::pipeline::PipelineFeatureFlags,
) -> Result<PtxSourceCacheKey, BackendError> {
    let normalized_digest = vyre_driver::pipeline::try_normalized_program_cache_digest(program)
        .map_err(|error| {
            BackendError::new(format!("CUDA PTX source cache digest failed: {error}"))
        })?;
    let vsa_bytes =
        vsa_fingerprint_cache_bytes(vyre_driver::program_vsa_fingerprint_words(program));
    let dispatch_policy_digest = vyre_driver::pipeline::dispatch_policy_cache_digest(config);
    let feature_flag_bytes = feature_flags.bits().to_le_bytes();
    let key = domain_separated_exact_input_key(
        CUDA_PTX_SOURCE_FROM_PROGRAM_DOMAIN,
        u64::from(ptx_target_sm),
        u64::from(subgroup_size),
        &[
            PTX_LOWERING_CONTRACT,
            &normalized_digest,
            &vsa_bytes,
            &dispatch_policy_digest,
            &feature_flag_bytes,
        ],
    )?;
    Ok(PtxSourceCacheKey(key))
}

fn module_cache_key_from_domain_digest(
    domain_tag: &[u8],
    compute_capability: (u32, u32),
    digest: &[u8; 32],
) -> Result<ModuleCacheKey, BackendError> {
    let key = domain_separated_exact_input_key(
        domain_tag,
        u64::from(compute_capability.0),
        u64::from(compute_capability.1),
        &[&digest[..]],
    )?;
    Ok(ModuleCacheKey(key))
}

/// Cache of lowered PTX text. This sits in front of the CUDA module cache so
/// ordinary dispatches avoid re-running descriptor validation and PTX emission
/// before discovering that the module is already warm.
#[derive(Debug)]
pub(crate) struct CudaPtxSourceCache {
    sources: DashMap<PtxSourceCacheKey, CachedPtxSource, BuildHasherDefault<FxHasher>>,
    hits: AtomicU64,
    misses: AtomicU64,
    cached_source_bytes: AtomicUsize,
}

#[derive(Debug)]
struct CachedPtxSource {
    source: Arc<str>,
    source_bytes: usize,
    access_count: AtomicU32,
}

/// Snapshot of the CUDA PTX source cache used before driver module loading.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CudaPtxSourceCacheSnapshot {
    /// Number of normalized PTX source entries retained in memory.
    pub entries: usize,
    /// Number of PTX source bytes retained in memory.
    pub cached_source_bytes: usize,
    /// Number of lookups served from an existing lowered PTX source.
    pub hits: u64,
    /// Number of lookups that had to lower PTX source before insertion.
    pub misses: u64,
}

impl CudaPtxSourceCache {
    pub(crate) fn new() -> Self {
        Self {
            sources: DashMap::with_hasher(BuildHasherDefault::<FxHasher>::default()),
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
            cached_source_bytes: AtomicUsize::new(0),
        }
    }

    pub(crate) fn key_for_program(
        &self,
        program: &Program,
        config: &DispatchConfig,
        ptx_target_sm: u32,
        subgroup_size: u32,
        feature_flags: vyre_driver::pipeline::PipelineFeatureFlags,
    ) -> Result<PtxSourceCacheKey, BackendError> {
        ptx_source_cache_key_from_program_identity(
            program,
            config,
            ptx_target_sm,
            subgroup_size,
            feature_flags,
        )
    }

    pub(crate) fn get_or_lower(
        &self,
        key: PtxSourceCacheKey,
        lower: impl FnOnce() -> Result<String, BackendError>,
    ) -> Result<Arc<str>, BackendError> {
        if let Some(source) = self.sources.get(&key) {
            increment_cache_access_u32(&source.access_count, "CUDA PTX source access count");
            increment_cache_counter_u64(&self.hits, "CUDA PTX source cache hits");
            return Ok(Arc::clone(&source.value().source));
        }
        // Disk persistence: PTX text is large (megabytes) but compresses
        // well; reading from disk is ~10 ms vs the multi-100 ms cost of
        // re-running the vyre IR -> PTX lowering on the same program
        // shape. Cross-process and across-runs: second run of the same
        // corpus loads every lowered PTX from disk, hitting the CUDA
        // driver's cuda-jit cache for PTX -> cuBIN compilation, and
        // skipping the vyre-side lowering entirely.
        if let Some(disk_source) = load_ptx_from_disk(&key)? {
            let arc: Arc<str> = disk_source.into();
            return self.insert_disk_cached_source(key, arc);
        }
        increment_cache_counter_u64(&self.misses, "CUDA PTX source cache misses");
        if self.sources.len() >= PTX_SOURCE_CACHE_SOFT_CAP {
            self.evict_submodular();
        }
        let source = match self.sources.entry(key) {
            Entry::Occupied(existing) => {
                increment_cache_access_u32(
                    &existing.get().access_count,
                    "CUDA PTX source access count",
                );
                Arc::clone(&existing.get().source)
            }
            Entry::Vacant(entry) => {
                let source: Arc<str> = lower()?.into();
                store_ptx_to_disk(&key, source.as_ref())?;
                let source_bytes = source.len();
                if source_bytes > PTX_SOURCE_CACHE_SOFT_BYTES {
                    return Ok(source);
                }
                reserve_cached_source_bytes(&self.cached_source_bytes, source_bytes)?;
                entry.insert(CachedPtxSource {
                    source: Arc::clone(&source),
                    source_bytes,
                    access_count: AtomicU32::new(1),
                });
                source
            }
        };
        if self.cached_source_bytes.load(Ordering::Acquire) > PTX_SOURCE_CACHE_SOFT_BYTES {
            self.evict_submodular();
        }
        Ok(source)
    }

    pub(crate) fn clear(&self) {
        self.sources.clear();
        self.hits.store(0, Ordering::Release);
        self.misses.store(0, Ordering::Release);
        self.cached_source_bytes.store(0, Ordering::Release);
    }

    pub(crate) fn snapshot(&self) -> CudaPtxSourceCacheSnapshot {
        CudaPtxSourceCacheSnapshot {
            entries: self.sources.len(),
            cached_source_bytes: self.cached_source_bytes.load(Ordering::Acquire),
            hits: self.hits.load(Ordering::Acquire),
            misses: self.misses.load(Ordering::Acquire),
        }
    }

    fn insert_disk_cached_source(
        &self,
        key: PtxSourceCacheKey,
        source: Arc<str>,
    ) -> Result<Arc<str>, BackendError> {
        let source_bytes = source.len();
        if source_bytes > PTX_SOURCE_CACHE_SOFT_BYTES {
            return Ok(source);
        }
        let cached_source_bytes = self.cached_source_bytes.load(Ordering::Acquire);
        if self.sources.len() >= PTX_SOURCE_CACHE_SOFT_CAP
            || cached_source_bytes > PTX_SOURCE_CACHE_SOFT_BYTES - source_bytes
        {
            self.evict_submodular();
        }
        match self.sources.entry(key) {
            Entry::Occupied(existing) => {
                increment_cache_access_u32(
                    &existing.get().access_count,
                    "CUDA PTX source access count",
                );
                increment_cache_counter_u64(&self.hits, "CUDA PTX source cache disk hits");
                Ok(Arc::clone(&existing.get().source))
            }
            Entry::Vacant(entry) => {
                reserve_cached_source_bytes(&self.cached_source_bytes, source_bytes)?;
                entry.insert(CachedPtxSource {
                    source: Arc::clone(&source),
                    source_bytes,
                    access_count: AtomicU32::new(1),
                });
                increment_cache_counter_u64(&self.hits, "CUDA PTX source cache disk hits");
                Ok(source)
            }
        }
    }

    fn evict_submodular(&self) {
        let mut keys = SmallVec::<[PtxSourceCacheKey; PTX_SOURCE_CACHE_SOFT_CAP]>::new();
        let mut gains = SmallVec::<[u32; PTX_SOURCE_CACHE_SOFT_CAP]>::new();
        for entry in self.sources.iter() {
            keys.push(*entry.key());
            gains.push(entry.access_count.load(Ordering::Relaxed));
        }
        let Some((n, k)) = retention_problem_size(
            gains.len(),
            PTX_SOURCE_CACHE_RETAIN_AFTER_EVICTION,
            "CUDA PTX source cache",
        ) else {
            self.sources.clear();
            self.cached_source_bytes.store(0, Ordering::Release);
            vyre_driver::cache_eviction::record_eviction_counts(keys.len(), keys.len());
            return;
        };
        let retention =
            match vyre_driver::cache_eviction::try_select_retention_set(&mut gains, n, k) {
                Ok(retention) => retention,
                Err(error) => {
                    tracing::error!(
                    "CUDA PTX source cache eviction could not allocate retention state: {error}"
                );
                    self.sources.clear();
                    self.cached_source_bytes.store(0, Ordering::Release);
                    vyre_driver::cache_eviction::record_eviction_counts(keys.len(), keys.len());
                    return;
                }
            };

        let mut to_remove: SmallVec<[PtxSourceCacheKey; PTX_SOURCE_CACHE_SOFT_CAP]> =
            SmallVec::new();
        if let Err(error) = reserve_smallvec(
            &mut to_remove,
            retention.len(),
            "PTX source cache eviction removal key",
        ) {
            tracing::error!(
                "CUDA PTX source cache eviction could not reserve {} removal key slot(s): {error}",
                retention.len()
            );
            self.sources.clear();
            self.cached_source_bytes.store(0, Ordering::Release);
            vyre_driver::cache_eviction::record_eviction_counts(keys.len(), keys.len());
            return;
        }
        for (i, retain) in retention.iter().enumerate() {
            if *retain == 0 {
                if let Some(key) = keys.get(i) {
                    to_remove.push(*key);
                }
            }
        }
        let dropped = to_remove.len();
        let total = keys.len().max(1);
        let mut dropped_bytes = 0usize;
        for key in &to_remove {
            if let Some((_, removed)) = self.sources.remove(key) {
                let Ok(next) = checked_add_usize_lazy(dropped_bytes, removed.source_bytes, || ())
                else {
                    self.sources.clear();
                    self.cached_source_bytes.store(0, Ordering::Release);
                    vyre_driver::cache_eviction::record_eviction_counts(keys.len(), keys.len());
                    return;
                };
                dropped_bytes = next;
            }
        }
        if dropped_bytes != 0 {
            if release_cached_source_bytes(&self.cached_source_bytes, dropped_bytes).is_err() {
                self.sources.clear();
                self.cached_source_bytes.store(0, Ordering::Release);
                vyre_driver::cache_eviction::record_eviction_counts(keys.len(), keys.len());
                return;
            }
        }
        vyre_driver::cache_eviction::record_eviction_counts(dropped, total);
    }
}

fn reserve_cached_source_bytes(
    cached_source_bytes: &AtomicUsize,
    source_bytes: usize,
) -> Result<(), BackendError> {
    checked_atomic_add_usize_with_order(
        cached_source_bytes,
        source_bytes,
        Ordering::Acquire,
        Ordering::AcqRel,
        Ordering::Acquire,
        |observed, attempted| {
            BackendError::new(format!(
                "CUDA PTX source cache byte accounting overflowed while adding {attempted} bytes to {observed}. Fix: shard generated PTX or clear the source cache before inserting another artifact."
            ))
        },
    )
}

fn ptx_disk_cache_root() -> Result<PathBuf, BackendError> {
    if let Some(p) = std::env::var_os("VYRE_PTX_SOURCE_CACHE_DIR") {
        let path = PathBuf::from(p);
        if path.as_os_str().is_empty() {
            return Err(BackendError::new(
                "VYRE_PTX_SOURCE_CACHE_DIR is empty. Fix: set it to a writable persistent directory or unset it so XDG/HOME cache discovery can run."
                    .to_string(),
            ));
        }
        return Ok(path);
    }
    if let Some(xdg) = std::env::var_os("XDG_CACHE_HOME") {
        return Ok(PathBuf::from(xdg).join("vyre").join("ptx-source"));
    }
    if let Some(home) = std::env::var_os("HOME") {
        return Ok(PathBuf::from(home)
            .join(".cache")
            .join("vyre")
            .join("ptx-source"));
    }
    Err(BackendError::new(
        "CUDA PTX source cache has no VYRE_PTX_SOURCE_CACHE_DIR, XDG_CACHE_HOME, or HOME. Fix: configure a writable persistent cache root; temporary fallback is forbidden for production compile performance."
            .to_string(),
    ))
}

fn retention_problem_size(
    len: usize,
    retain_after_eviction: usize,
    label: &str,
) -> Option<(u32, u32)> {
    let n = match u32::try_from(len) {
        Ok(value) => value,
        Err(source) => {
            tracing::error!("{label} retention candidate count cannot fit u32: {source}. Fix: lower cache soft caps or shard eviction telemetry.");
            return None;
        }
    };
    let k = match u32::try_from(retain_after_eviction) {
        Ok(value) => value,
        Err(source) => {
            tracing::error!("{label} retention target count cannot fit u32: {source}. Fix: lower cache soft caps or shard eviction telemetry.");
            return None;
        }
    };
    if k > n {
        tracing::error!("{label} retention target exceeds candidate count: retain={k}, candidates={n}. Fix: trigger eviction only after the cache reaches its soft cap or correct the retention policy.");
        return None;
    }
    Some((n, k))
}

fn ptx_disk_cache_path(key: &PtxSourceCacheKey) -> Result<PathBuf, BackendError> {
    let mut hex = [0u8; 64];
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for (i, &b) in key.0.iter().enumerate() {
        hex[i * 2] = HEX[usize::from(b >> 4)];
        hex[i * 2 + 1] = HEX[usize::from(b & 0x0f)];
    }
    let stem = std::str::from_utf8(&hex).map_err(|error| {
        BackendError::new(format!(
            "CUDA PTX source cache generated a non-UTF8 hex key from fixed lowercase ASCII digits: {error}. Fix: inspect cache key generation before publishing PTX artifacts."
        ))
    })?;
    let dir = ptx_disk_cache_root()?.join(&stem[..2]);
    Ok(dir.join(format!("{stem}.ptx")))
}

fn load_ptx_from_disk(key: &PtxSourceCacheKey) -> Result<Option<String>, BackendError> {
    let path = ptx_disk_cache_path(key)?;
    match std::fs::metadata(&path) {
        Ok(metadata) => {
            validate_ptx_disk_cache_file_len(metadata.len(), &path)?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(BackendError::new(format!(
                "failed to stat CUDA PTX source cache `{}`: {error}. Fix: repair cache file permissions or remove the corrupt cache entry; do not silently relower around a broken production cache.",
                path.display()
            )));
        }
    }
    read_ptx_disk_cache_source_bounded(&path)
}

fn read_ptx_disk_cache_source_bounded(
    path: &std::path::Path,
) -> Result<Option<String>, BackendError> {
    let mut reader = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(BackendError::new(format!(
                "failed to read CUDA PTX source cache `{}`: {error}. Fix: repair cache file permissions or remove the corrupt cache entry; do not silently relower around a broken production cache.",
                path.display()
            )));
        }
    };
    let mut bytes = Vec::new();
    let mut total = 0u64;
    let mut chunk = [0u8; 8192];
    loop {
        let read = std::io::Read::read(&mut reader, &mut chunk).map_err(|error| {
            BackendError::new(format!(
                "failed to read CUDA PTX source cache `{}`: {error}. Fix: repair cache file permissions or remove the corrupt cache entry; do not silently relower around a broken production cache.",
                path.display()
            ))
        })?;
        if read == 0 {
            return String::from_utf8(bytes).map(Some).map_err(|error| {
                BackendError::new(format!(
                    "CUDA PTX source cache `{}` is not UTF-8: {error}. Fix: remove the corrupt cache artifact.",
                    path.display()
                ))
            });
        }
        let read = read as u64;
        // Use checked_add, matching the checked-arithmetic contract that every
        // other counter in this module follows (see `pinning_atomic_increment_u64`
        // and the source-contract test `module_cache_eviction_buffers_fit_soft_cap_inline`).
        // saturating_add would silently freeze `total` at u64::MAX on
        // pathological input, breaking the safety-cap check that follows.
        total = total.checked_add(read).ok_or_else(|| {
            BackendError::new(format!(
                "CUDA PTX source cache `{}` size accumulator overflowed u64 during bounded read. \
                 Fix: remove the corrupt cache artifact and re-prime the cache.",
                path.display()
            ))
        })?;
        if total > PTX_SOURCE_CACHE_MAX_ARTIFACT_BYTES {
            return Err(BackendError::new(format!(
                "CUDA PTX source cache `{}` exceeds the {} byte safety limit while reading. Fix: remove the corrupt cache artifact or raise the cap deliberately after reviewing compile-cache memory pressure.",
                path.display(),
                PTX_SOURCE_CACHE_MAX_ARTIFACT_BYTES
            )));
        }
        bytes.extend_from_slice(&chunk[..read as usize]);
    }
}

fn validate_ptx_disk_cache_file_len(
    byte_len: u64,
    path: &std::path::Path,
) -> Result<(), BackendError> {
    if byte_len > PTX_SOURCE_CACHE_MAX_ARTIFACT_BYTES {
        return Err(BackendError::new(format!(
            "CUDA PTX source cache `{}` is {byte_len} bytes, above the {} byte safety limit. Fix: remove the corrupt cache artifact or raise the artifact cap deliberately after reviewing compile-cache memory pressure.",
            path.display(),
            PTX_SOURCE_CACHE_MAX_ARTIFACT_BYTES
        )));
    }
    Ok(())
}

fn store_ptx_to_disk(key: &PtxSourceCacheKey, source: &str) -> Result<(), BackendError> {
    let source_len = u64::try_from(source.len()).map_err(|error| {
        BackendError::new(format!(
            "CUDA PTX source cache artifact length cannot fit u64: {error}. Fix: split the generated Program before attempting disk persistence."
        ))
    })?;
    if source_len > PTX_SOURCE_CACHE_MAX_ARTIFACT_BYTES {
        return Err(BackendError::new(format!(
            "refusing to write {} byte CUDA PTX source cache artifact above the {} byte safety limit. Fix: split the generated Program, reduce monomorphized PTX size, or raise the artifact cap deliberately after reviewing compile-cache memory pressure.",
            source_len,
            PTX_SOURCE_CACHE_MAX_ARTIFACT_BYTES
        )));
    }
    let path = ptx_disk_cache_path(key)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
            BackendError::new(format!(
                "failed to create CUDA PTX source cache directory `{}`: {error}. Fix: set VYRE_PTX_SOURCE_CACHE_DIR to a writable cache directory or repair directory permissions.",
                parent.display()
            ))
        })?;
    }
    let tmp_id = allocate_ptx_cache_tmp_id()?;
    let tmp = path.with_extension(format!("ptx.{}.{}.tmp", std::process::id(), tmp_id));
    std::fs::write(&tmp, source.as_bytes()).map_err(|error| {
        BackendError::new(format!(
            "failed to write CUDA PTX source cache temp file `{}`: {error}. Fix: set VYRE_PTX_SOURCE_CACHE_DIR to a writable cache directory or repair filesystem permissions.",
            tmp.display()
        ))
    })?;
    std::fs::rename(&tmp, &path).map_err(|error| {
        let cleanup = match std::fs::remove_file(&tmp) {
            Ok(()) => String::new(),
            Err(cleanup_error) if cleanup_error.kind() == std::io::ErrorKind::NotFound => {
                String::new()
            }
            Err(cleanup_error) => {
                format!(" Temp cleanup also failed: {cleanup_error}. Fix: repair cache directory permissions and remove stale temp files.")
            }
        };
        BackendError::new(format!(
            "failed to publish CUDA PTX source cache `{}` from temp `{}`: {error}.{cleanup} Fix: repair cache directory permissions and filesystem atomic-rename support.",
            path.display(),
            tmp.display()
        ))
    })?;
    Ok(())
}

fn allocate_ptx_cache_tmp_id() -> Result<u64, BackendError> {
    Ok(rebasing_atomic_next_u64(
        &PTX_CACHE_TMP_COUNTER,
        1,
        Ordering::Acquire,
        Ordering::AcqRel,
        Ordering::Acquire,
        |_, _| {
            tracing::error!(
                "CUDA PTX source cache temp-file counter overflowed u64; rebasing sequence to keep disk cache publication alive. Fix: inspect unexpectedly high cache write churn."
            );
        },
    ))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{
        allocate_ptx_cache_tmp_id, ptx_disk_cache_path, validate_ptx_disk_cache_file_len,
        CudaPtxSourceCache, PtxSourceCacheKey, PTX_CACHE_TMP_COUNTER,
        PTX_SOURCE_CACHE_MAX_ARTIFACT_BYTES,
    };
    use std::sync::atomic::Ordering;
    use vyre_foundation::ir::Program;

    #[test]
    fn ptx_source_cache_snapshot_tracks_hits_misses_and_clear() {
        let cache = CudaPtxSourceCache::new();
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"ptx_source_cache_snapshot_tracks_hits_misses_and_clear");
        hasher.update(&std::process::id().to_le_bytes());
        hasher.update(
            &SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("Fix: system clock must be after Unix epoch")
                .as_nanos()
                .to_le_bytes(),
        );
        let key = PtxSourceCacheKey(*hasher.finalize().as_bytes());
        let disk_path = ptx_disk_cache_path(&key)
            .expect("Fix: PTX source cache path should resolve on the test host.");
        match std::fs::remove_file(&disk_path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => panic!(
                "failed to remove pre-existing PTX cache artifact `{}` before deterministic cache-counter test: {error}",
                disk_path.display()
            ),
        }

        let first = cache
            .get_or_lower(key, || Ok("cached-ptx-source".to_string()))
            .expect("Fix: first PTX source lowering should populate cache");
        let second = cache
            .get_or_lower(key, || panic!("cache hit must not relower PTX source"))
            .expect("Fix: second PTX source lookup should hit cache");

        assert!(Arc::ptr_eq(&first, &second));
        let snapshot = cache.snapshot();
        assert_eq!(snapshot.entries, 1);
        assert_eq!(snapshot.cached_source_bytes, "cached-ptx-source".len());
        assert_eq!(snapshot.hits, 1);
        assert_eq!(snapshot.misses, 1);

        cache.clear();
        let snapshot = cache.snapshot();
        assert_eq!(snapshot.entries, 0);
        assert_eq!(snapshot.cached_source_bytes, 0);
        assert_eq!(snapshot.hits, 0);
        assert_eq!(snapshot.misses, 0);

        let _ = std::fs::remove_file(disk_path);
    }

    #[test]
    fn ptx_disk_cache_rejects_oversized_artifact_before_reading() {
        let path = std::path::PathBuf::from("/tmp/vyre-oversized-ptx-cache-artifact.ptx");
        let error =
            validate_ptx_disk_cache_file_len(PTX_SOURCE_CACHE_MAX_ARTIFACT_BYTES + 1, &path)
                .expect_err("oversized PTX cache artifact must be rejected before allocation");

        let message = error.to_string();
        assert!(message.contains("above the"));
        assert!(message.contains("safety limit"));
        assert!(message.contains("remove the corrupt cache artifact"));
    }

    #[test]
    fn ptx_source_cache_temp_id_rebases_after_counter_overflow() {
        PTX_CACHE_TMP_COUNTER.store(u64::MAX, Ordering::Release);

        let id = allocate_ptx_cache_tmp_id().expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - PTX temp-file id allocation must rebase instead of failing on counter overflow",
        );

        assert_eq!(id, u64::MAX);
        assert_eq!(PTX_CACHE_TMP_COUNTER.load(Ordering::Acquire), 1);
    }

    #[test]
    fn cache_hit_miss_counters_saturate_instead_of_wrapping_to_zero() {
        let counter = std::sync::atomic::AtomicU64::new(u64::MAX - 1);

        super::increment_cache_counter_u64(&counter, "test CUDA cache counter");
        assert_eq!(
            counter.load(Ordering::Acquire),
            u64::MAX,
            "Fix: CUDA cache telemetry must still reach u64::MAX exactly."
        );

        super::increment_cache_counter_u64(&counter, "test CUDA cache counter");
        assert_eq!(
            counter.load(Ordering::Acquire),
            u64::MAX,
            "Fix: CUDA cache telemetry must saturate at u64::MAX instead of wrapping to zero."
        );
    }

    #[test]
    fn module_cache_eviction_buffers_fit_soft_cap_inline() {
        let source = include_str!("module_cache.rs");

        assert!(
            source.contains("SmallVec::<[PtxSourceCacheKey; PTX_SOURCE_CACHE_SOFT_CAP]>::new()")
                && source.contains("SmallVec::<[u32; PTX_SOURCE_CACHE_SOFT_CAP]>::new()")
                && source.contains("SmallVec<[PtxSourceCacheKey; PTX_SOURCE_CACHE_SOFT_CAP]>"),
            "Fix: PTX source cache eviction scans the full soft cap, so eviction scratch must fit the full cap inline instead of spilling at the retained-half capacity."
        );
        assert!(
            source.contains("SmallVec::<[ModuleCacheKey; MODULE_CACHE_SOFT_CAP]>::new()")
                && source.contains("SmallVec::<[u32; MODULE_CACHE_SOFT_CAP]>::new()")
                && source.contains("SmallVec<[ModuleCacheKey; MODULE_CACHE_SOFT_CAP]>"),
            "Fix: CUDA module cache eviction scans the full soft cap, so eviction scratch must fit the full cap inline instead of spilling at the retained-half capacity."
        );
        assert!(
            !source.contains(concat!("unwrap_or", "(u32::MAX)")),
            "Fix: CUDA module-cache eviction must not silently cap retention problem sizes; impossible oversize states should evict instead of feeding fake counts to the optimizer."
        );
        assert!(
            !source.contains(concat!("cached_source_bytes", "\n                    .fetch_add")),
            "Fix: CUDA PTX source-cache byte accounting must reserve with checked arithmetic, not unchecked atomic fetch_add."
        );
        assert!(
            !source.contains(concat!("dropped-byte", " accounting overflowed")),
            "Fix: CUDA PTX source-cache eviction byte accounting must repair cache state instead of panicking."
        );
        assert!(
            !source.contains(concat!("cached_source_bytes", "\n                .fetch_sub")),
            "Fix: CUDA PTX source-cache byte release must use checked arithmetic, not wrapping atomic fetch_sub."
        );
        assert!(
            !source.contains(concat!("PTX_CACHE_TMP_COUNTER", ".fetch_add")),
            "Fix: CUDA PTX source-cache temp ids must use checked atomic allocation, not wrapping fetch_add."
        );
        assert!(
            !source.contains(concat!("access_count", ".fetch_add")),
            "Fix: CUDA module/source cache eviction priority counters must use bounded updates instead of raw wrapping."
        );
        assert!(
            !source.contains(concat!("hits", ".fetch_add"))
                && !source.contains(concat!("misses", ".fetch_add")),
            "Fix: CUDA module/source cache hit-miss counters must use explicit rebase helpers instead of raw wrapping."
        );
        assert!(
            !source.contains(concat!("fn ptx_disk_cache_root", "() -> PathBuf")),
            "Fix: CUDA PTX source-cache root discovery must return typed errors instead of panicking."
        );
        let production = source
            .split("#[cfg(test)]")
            .next()
            .expect("Fix: module-cache source must have production section before tests");
        assert!(
            !production.contains(concat!("panic", "!("))
                && !production.contains(concat!(".unwrap_or_else", "(|source|")),
            "Fix: CUDA module/PTX cache production eviction and counter paths must repair or return errors instead of panicking."
        );
        assert!(
            production.contains("increment_cache_counter_u64(&self.hits")
                && production.contains("increment_cache_access_u32(&source.access_count")
                && production.contains("rebasing sequence to keep disk cache publication alive")
                && production.contains("record_eviction_counts(keys.len(), keys.len())"),
            "Fix: CUDA module/PTX cache counters must not fail valid cache hits, and impossible eviction states must repair the cache."
        );
        let counter_section = source
            .rsplit("fn increment_cache_counter_u64(counter")
            .next()
            .expect("Fix: CUDA module/PTX cache counter helper must exist")
            .split("fn increment_cache_access_u32")
            .next()
            .expect("Fix: u64 cache counter helper must precede u32 access counter helper");
        assert!(
            counter_section.contains("pinning_atomic_increment_u64")
                && counter_section.contains("reached u64::MAX and is pinned")
                && !counter_section.contains("compare_exchange_weak")
                && !counter_section.contains("wrapping_add(1)"),
            "Fix: CUDA module/PTX cache telemetry counters must use shared pinning accounting instead of wrapping or carrying a local CAS loop."
        );
        assert!(
            !production.contains("fn eviction_ratio")
                && !production.contains("dropped as f64")
                && !production.contains("total.max(1) as f64"),
            "Fix: CUDA module/PTX cache eviction telemetry must use backend-neutral exact count accounting, not local lossy ratios."
        );
        assert!(
            production.contains("cache_eviction::try_select_retention_set")
                && !production.contains(concat!(
                    "cache_eviction::select_retention_set",
                    "(&mut gains"
                )),
            "Fix: CUDA module/PTX cache eviction must use the fallible backend-neutral selector on release paths."
        );
    }

    #[test]
    fn ptx_source_cache_keys_use_shared_domain_identity_for_generated_configs() {
        let cache = CudaPtxSourceCache::new();
        let program = Program::wrapped(vec![], [64, 1, 1], vec![]);

        for case in 0..2048u32 {
            let mut config = vyre_driver::DispatchConfig::default();
            if case & 1 != 0 {
                config.ulp_budget = Some((case as u8).wrapping_mul(11).wrapping_add(1));
            }
            if case & 2 != 0 {
                config.workgroup_override = Some([
                    1 + (case & 127),
                    1 + ((case.rotate_left(7) >> 3) & 31),
                    1 + ((case.rotate_right(5) >> 2) & 7),
                ]);
            }
            let flags = match case & 3 {
                0 => vyre_driver::pipeline::PipelineFeatureFlags::empty(),
                1 => vyre_driver::pipeline::PipelineFeatureFlags::SUBGROUP_OPS,
                2 => vyre_driver::pipeline::PipelineFeatureFlags::F16
                    .union(vyre_driver::pipeline::PipelineFeatureFlags::BF16),
                _ => vyre_driver::pipeline::PipelineFeatureFlags::TENSOR_CORES
                    .union(vyre_driver::pipeline::PipelineFeatureFlags::ASYNC_COMPUTE),
            };
            let ptx_target_sm = 70 + (case % 30);
            let subgroup_size = 1 + (case.rotate_left(3) % 64);
            let key = cache
                .key_for_program(&program, &config, ptx_target_sm, subgroup_size, flags)
                .expect("Fix: generated PTX source cache key must fit shared identity envelope");
            assert_eq!(
                key,
                cache
                    .key_for_program(&program, &config, ptx_target_sm, subgroup_size, flags)
                    .expect("Fix: repeated generated PTX source cache key must fit"),
                "Fix: CUDA PTX source cache identity must be stable for generated case {case}."
            );
            assert_ne!(
                key,
                cache
                    .key_for_program(&program, &config, ptx_target_sm + 1, subgroup_size, flags)
                    .expect("Fix: generated PTX target variation cache key must fit"),
                "Fix: CUDA PTX source cache identity must include target SM for generated case {case}."
            );
            assert_ne!(
                key,
                cache
                    .key_for_program(&program, &config, ptx_target_sm, subgroup_size + 1, flags)
                    .expect("Fix: generated subgroup variation cache key must fit"),
                "Fix: CUDA PTX source cache identity must include subgroup size for generated case {case}."
            );

            let changed_flags =
                flags.union(vyre_driver::pipeline::PipelineFeatureFlags::PERSISTENT_THREAD);
            assert_ne!(
                key,
                cache
                    .key_for_program(
                        &program,
                        &config,
                        ptx_target_sm,
                        subgroup_size,
                        changed_flags,
                    )
                    .expect("Fix: generated feature-flag variation cache key must fit"),
                "Fix: CUDA PTX source cache identity must include feature flags for generated case {case}."
            );

            let mut changed_config = config.clone();
            changed_config.ulp_budget = Some(config.ulp_budget.unwrap_or(0).wrapping_add(1));
            assert_ne!(
                key,
                cache
                    .key_for_program(
                        &program,
                        &changed_config,
                        ptx_target_sm,
                        subgroup_size,
                        flags,
                    )
                    .expect("Fix: generated dispatch-policy variation cache key must fit"),
                "Fix: CUDA PTX source cache identity must include dispatch policy for generated case {case}."
            );
        }
    }

    #[test]
    fn ptx_source_cache_key_derivation_is_single_sourced_through_driver_identity() {
        let source = include_str!("module_cache.rs");
        let helper_section = source
            .split("fn ptx_source_cache_key_from_program_identity")
            .nth(1)
            .expect("Fix: module cache source must keep PTX source key helper visible")
            .split("fn module_cache_key_from_domain_digest")
            .next()
            .expect(
                "Fix: module cache source must keep PTX source key helper before module key helper",
            );
        let key_section = source
            .split("pub(crate) fn key_for_program")
            .nth(1)
            .expect("Fix: module cache source must keep PTX source-key derivation visible")
            .split("pub(crate) fn get_or_lower")
            .next()
            .expect("Fix: module cache source must keep PTX source lookup after key derivation");
        assert!(
            helper_section.contains("domain_separated_exact_input_key(")
                && helper_section.contains("dispatch_policy_cache_digest(config)")
                && helper_section.contains("PTX_LOWERING_CONTRACT")
                && key_section.contains("ptx_source_cache_key_from_program_identity("),
            "Fix: CUDA PTX source cache keys must route through shared driver identity helpers instead of local tuple hashing."
        );
        assert!(
            !key_section.contains(&["blake", "3::Hasher::new()"].concat()),
            "Fix: CUDA PTX source-cache key derivation must not fork local tuple hashing."
        );
    }

    #[test]
    fn ptx_source_cache_contract_versions_full_workgroup_entry_semantics() {
        let source = include_str!("module_cache.rs");
        assert!(
            source.contains("vyre-cuda-ptx-lowering-contract:v14")
                && source.contains("full-workgroup-entry")
                && source.contains("bounded-full-workgroup-stores")
                && source.contains("child-captured-producer-liveness")
                && source.contains("single-mad-dual-mul-liveness"),
            "Fix: CUDA PTX source cache keys must change when PTX lowering changes barrier/shared-memory entry behavior, full-workgroup store predication, parent-result liveness across child bodies, or MAD deferral liveness."
        );
    }
}

/// Loaded CUDA module and its `main` entry function.
#[derive(Debug)]
struct CachedModule {
    module: CUmodule,
    main: CUfunction,
    /// Device pointer + byte size of the module-scope cooperative grid-barrier
    /// counter (`_vyre_grid_barrier`). `Some` only when the PTX declares it
    /// (a grid-sync program); the host zeroes this counter before each
    /// cooperative launch. `None` for every non-grid-sync kernel.
    grid_barrier_global: Option<(u64, usize)>,
    access_count: AtomicU32,
}

// SAFETY: FFI to libcuda.so. Pointer args were validated by the matching alloc
// / store API; lifetimes are documented in the surrounding function.
// cuda_check (or matching CUresult guard) propagates non-success codes as
// BackendError.
unsafe impl Send for CachedModule {}
// SAFETY: FFI to libcuda.so. Pointer args were validated by the matching alloc
// / store API; lifetimes are documented in the surrounding function.
// cuda_check (or matching CUresult guard) propagates non-success codes as
// BackendError.
unsafe impl Sync for CachedModule {}

impl Drop for CachedModule {
    fn drop(&mut self) {
        unload_cuda_module_or_log(self.module, "CUDA module cache drop");
    }
}

/// Sharded CUDA module cache with lock-free hit counters.
#[derive(Debug)]
pub(crate) struct CudaModuleCache {
    modules: DashMap<ModuleCacheKey, CachedModule, BuildHasherDefault<FxHasher>>,
    hits: AtomicU64,
    misses: AtomicU64,
}

impl CudaModuleCache {
    pub(crate) fn new() -> Self {
        Self {
            modules: DashMap::with_hasher(BuildHasherDefault::<FxHasher>::default()),
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
        }
    }

    pub(crate) fn key_for_ptx_source_key(
        &self,
        ptx_source_key: PtxSourceCacheKey,
        compute_capability: (u32, u32),
    ) -> Result<ModuleCacheKey, BackendError> {
        module_cache_key_from_domain_digest(
            CUDA_MODULE_FROM_PTX_SOURCE_KEY_DOMAIN,
            compute_capability,
            ptx_source_key.as_bytes(),
        )
    }

    pub(crate) fn key_for_raw_ptx_artifact(
        &self,
        raw_ptx_source: &str,
        compute_capability: (u32, u32),
    ) -> Result<ModuleCacheKey, BackendError> {
        let raw_artifact_digest = blake3::hash(raw_ptx_source.as_bytes());
        module_cache_key_from_domain_digest(
            CUDA_MODULE_FROM_RAW_PTX_ARTIFACT_DOMAIN,
            compute_capability,
            raw_artifact_digest.as_bytes(),
        )
    }

    pub(crate) fn function_for_ptx(
        &self,
        ptx_src: &str,
        key: ModuleCacheKey,
        ptx_target_sm: u32,
    ) -> Result<CUfunction, BackendError> {
        if let Some(module) = self.modules.get(&key) {
            increment_cache_access_u32(&module.access_count, "CUDA module cache access count");
            increment_cache_counter_u64(&self.hits, "CUDA module cache hits");
            return Ok(module.main);
        }
        increment_cache_counter_u64(&self.misses, "CUDA module cache misses");

        if self.modules.len() >= MODULE_CACHE_SOFT_CAP {
            self.evict_submodular();
        }
        match self.modules.entry(key) {
            Entry::Occupied(existing) => {
                increment_cache_access_u32(
                    &existing.get().access_count,
                    "CUDA module cache access count",
                );
                increment_cache_counter_u64(&self.hits, "CUDA module cache hits");
                Ok(existing.get().main)
            }
            Entry::Vacant(entry) => {
                let loaded = load_module(ptx_src, ptx_target_sm)?;
                let main = loaded.main;
                entry.insert(loaded);
                Ok(main)
            }
        }
    }

    /// Device pointer + byte size of this kernel's cooperative grid-barrier
    /// counter, loading the module if it is not cached. Returns `None` for a
    /// kernel that declares no grid barrier (the caller must already know,
    /// from `contains_grid_sync`, whether one is required and treat `None` on
    /// a grid-sync program as a hard codegen error).
    pub(crate) fn grid_barrier_global_for_ptx(
        &self,
        ptx_src: &str,
        key: ModuleCacheKey,
        ptx_target_sm: u32,
    ) -> Result<Option<(u64, usize)>, BackendError> {
        if let Some(module) = self.modules.get(&key) {
            increment_cache_access_u32(&module.access_count, "CUDA module cache access count");
            increment_cache_counter_u64(&self.hits, "CUDA module cache hits");
            return Ok(module.grid_barrier_global);
        }
        increment_cache_counter_u64(&self.misses, "CUDA module cache misses");

        if self.modules.len() >= MODULE_CACHE_SOFT_CAP {
            self.evict_submodular();
        }
        match self.modules.entry(key) {
            Entry::Occupied(existing) => {
                increment_cache_access_u32(
                    &existing.get().access_count,
                    "CUDA module cache access count",
                );
                increment_cache_counter_u64(&self.hits, "CUDA module cache hits");
                Ok(existing.get().grid_barrier_global)
            }
            Entry::Vacant(entry) => {
                let loaded = load_module(ptx_src, ptx_target_sm)?;
                let global = loaded.grid_barrier_global;
                entry.insert(loaded);
                Ok(global)
            }
        }
    }

    pub(crate) fn clear(&self) {
        self.modules.clear();
    }

    pub(crate) fn len(&self) -> usize {
        self.modules.len()
    }

    pub(crate) fn snapshot(&self) -> vyre_driver::pipeline::PipelineCacheSnapshot {
        vyre_driver::pipeline::PipelineCacheSnapshot {
            hits: self.hits.load(Ordering::Relaxed),
            misses: self.misses.load(Ordering::Relaxed),
        }
    }

    fn evict_submodular(&self) {
        let mut keys = SmallVec::<[ModuleCacheKey; MODULE_CACHE_SOFT_CAP]>::new();
        let mut gains = SmallVec::<[u32; MODULE_CACHE_SOFT_CAP]>::new();
        for entry in self.modules.iter() {
            keys.push(*entry.key());
            gains.push(entry.access_count.load(Ordering::Relaxed));
        }
        let Some((n, k)) = retention_problem_size(
            gains.len(),
            MODULE_CACHE_RETAIN_AFTER_EVICTION,
            "CUDA module cache",
        ) else {
            self.modules.clear();
            vyre_driver::cache_eviction::record_eviction_counts(keys.len(), keys.len());
            return;
        };

        let retention =
            match vyre_driver::cache_eviction::try_select_retention_set(&mut gains, n, k) {
                Ok(retention) => retention,
                Err(error) => {
                    tracing::error!(
                        "CUDA module cache eviction could not allocate retention state: {error}"
                    );
                    self.modules.clear();
                    vyre_driver::cache_eviction::record_eviction_counts(keys.len(), keys.len());
                    return;
                }
            };

        let mut to_remove: SmallVec<[ModuleCacheKey; MODULE_CACHE_SOFT_CAP]> = SmallVec::new();
        if let Err(error) = reserve_smallvec(
            &mut to_remove,
            retention.len(),
            "module cache eviction removal key",
        ) {
            tracing::error!(
                "CUDA module cache eviction could not reserve {} removal key slot(s): {error}",
                retention.len()
            );
            self.modules.clear();
            vyre_driver::cache_eviction::record_eviction_counts(keys.len(), keys.len());
            return;
        }
        for (i, retain) in retention.iter().enumerate() {
            if *retain == 0 {
                if let Some(key) = keys.get(i) {
                    to_remove.push(*key);
                }
            }
        }
        let dropped = to_remove.len();
        let total = keys.len().max(1);
        for key in &to_remove {
            self.modules.remove(key);
        }
        vyre_driver::cache_eviction::record_eviction_counts(dropped, total);
    }
}

fn release_cached_source_bytes(
    cached_source_bytes: &AtomicUsize,
    dropped_bytes: usize,
) -> Result<(), BackendError> {
    checked_sub_usize(cached_source_bytes, dropped_bytes, |observed, dropped| {
        BackendError::new(format!(
                "CUDA PTX source-cache byte accounting underflowed while dropping {dropped} bytes from {observed}. Fix: clear the source cache and rebuild PTX cache residency from live entries."
            ))
    })
}

fn increment_cache_counter_u64(counter: &AtomicU64, label: &'static str) {
    pinning_atomic_increment_u64(counter, Ordering::Relaxed, Ordering::Relaxed, || {
        tracing::error!(
            "{label} reached u64::MAX and is pinned. Fix: scrape CUDA cache telemetry before u64::MAX or shard the telemetry window."
        );
    });
}

fn increment_cache_access_u32(counter: &AtomicU32, label: &'static str) {
    pinning_atomic_increment_u32(counter, Ordering::Relaxed, Ordering::Relaxed, || {
        tracing::error!(
            "{label} reached u32::MAX and is pinned for retention scoring. Fix: clear the CUDA cache or shard retention windows."
        );
    });
}

fn load_module(ptx_src: &str, ptx_target_sm: u32) -> Result<CachedModule, BackendError> {
    let mut module = std::ptr::null_mut();
    PTX_CSTR_SCRATCH.with(|scratch| {
        let mut ptx_c = scratch.borrow_mut();
        ptx_c.clear();
        let ptx_c_capacity = ptx_src
            .len()
            .checked_add(1)
            .ok_or_else(|| BackendError::new("CUDA module PTX C-string length overflowed usize. Fix: split generated PTX before module loading."))?;
        reserve_vec(
            &mut ptx_c,
            ptx_c_capacity,
            "cuda module PTX C-string scratch",
        )?;
        ptx_c.extend_from_slice(ptx_src.as_bytes());
        ptx_c.push(0);
        if let Some(dir) = std::env::var_os("VYRE_PTX_DUMP_ALL_DIR") {
            write_ptx_dump(dir, ptx_src, "VYRE_PTX_DUMP_ALL_DIR")?;
        }
        module = match load_cuda_module_data(ptx_c.as_slice()) {
            Ok(module) => module,
            Err(res) => {
                if let Some(dir) = std::env::var_os("VYRE_PTX_DUMP_DIR") {
                    let path = write_ptx_dump(dir, ptx_src, "VYRE_PTX_DUMP_DIR")?;
                    tracing::warn!("VYRE_PTX_DUMP: wrote failing PTX to {}", path.display());
                }
                return Err(BackendError::KernelCompileFailed {
                    backend: crate::CUDA_BACKEND_ID.to_string(),
                    compiler_message: format!(
                        "cuModuleLoadData failed with {res:?} for sm_{ptx_target_sm} and PTX length {} bytes. Fix: run the PTX smoke test for this Program and verify the live CUDA driver supports the emitted PTX ISA.",
                        ptx_src.len()
                    ),
                });
            }
        };
        Ok(())
    })?;
    let func_name =
        CStr::from_bytes_with_nul(b"main\0").map_err(|error| BackendError::KernelCompileFailed {
            backend: crate::CUDA_BACKEND_ID.to_string(),
            compiler_message: format!(
                "CUDA module function symbol literal was invalid: {error}. Fix: restore the static `main` CUDA entry symbol."
            ),
        })?;
    let func = match get_cuda_module_function(module, func_name) {
        Ok(func) => func,
        Err(res) => {
            unload_cuda_module_or_log(module, "CUDA module cleanup after function lookup failure");
            return Err(BackendError::KernelCompileFailed {
                backend: crate::CUDA_BACKEND_ID.to_string(),
                compiler_message: format!(
                    "cuModuleGetFunction(main) failed with {res:?} for sm_{ptx_target_sm}. Fix: ensure CUDA PTX emission still declares `.visible .entry main`."
                ),
            });
        }
    };
    // Resolve the cooperative grid-barrier counter when this kernel declares
    // one. The emitter always names the symbol `_vyre_grid_barrier`, so the
    // PTX text is an exact, non-lossy signal of its presence (no swallowed
    // NOT_FOUND on the 99% of kernels that have no grid barrier).
    let grid_barrier_global = if ptx_src.contains(GRID_BARRIER_SYMBOL_NAME) {
        let symbol = CStr::from_bytes_with_nul(GRID_BARRIER_SYMBOL_CSTR).map_err(|error| {
            BackendError::KernelCompileFailed {
                backend: crate::CUDA_BACKEND_ID.to_string(),
                compiler_message: format!(
                    "CUDA grid-barrier symbol literal was invalid: {error}. Fix: restore the `{GRID_BARRIER_SYMBOL_NAME}` module-scope counter declaration in the PTX emitter."
                ),
            }
        })?;
        match get_cuda_module_global(module, symbol) {
            Ok(global) => Some(global),
            Err(res) => {
                unload_cuda_module_or_log(
                    module,
                    "CUDA module cleanup after grid-barrier global lookup failure",
                );
                return Err(BackendError::KernelCompileFailed {
                    backend: crate::CUDA_BACKEND_ID.to_string(),
                    compiler_message: format!(
                        "cuModuleGetGlobal({GRID_BARRIER_SYMBOL_NAME}) failed with {res:?} for sm_{ptx_target_sm} even though the PTX text declares it. Fix: ensure the PTX emitter emits `.global .align 4 .u32 {GRID_BARRIER_SYMBOL_NAME}[1];` at module scope for grid-sync kernels."
                    ),
                });
            }
        }
    } else {
        None
    };
    Ok(CachedModule {
        module,
        main: func,
        grid_barrier_global,
        access_count: AtomicU32::new(1),
    })
}

/// Module-scope cooperative grid-barrier counter symbol, emitted by
/// `vyre-emit-ptx` as `.global .align 4 .u32 _vyre_grid_barrier[1];`.
pub(crate) const GRID_BARRIER_SYMBOL_NAME: &str = "_vyre_grid_barrier";
const GRID_BARRIER_SYMBOL_CSTR: &[u8] = b"_vyre_grid_barrier\0";

pub(crate) fn load_cuda_module_data(image_with_nul: &[u8]) -> Result<CUmodule, CUresult> {
    if image_with_nul.last().copied() != Some(0) {
        return Err(CUresult::CUDA_ERROR_INVALID_VALUE);
    }
    let mut module = std::ptr::null_mut();
    // SAFETY: `image_with_nul` is a live PTX/CUBIN image buffer for the call
    // duration and is checked to be NUL-terminated above.
    let result = unsafe {
        cudarc::driver::sys::cuModuleLoadData(&mut module, image_with_nul.as_ptr().cast())
    };
    if result != CUresult::CUDA_SUCCESS {
        return Err(result);
    }
    if module.is_null() {
        return Err(CUresult::CUDA_ERROR_INVALID_VALUE);
    }
    Ok(module)
}

pub(crate) fn get_cuda_module_function(
    module: CUmodule,
    name: &CStr,
) -> Result<CUfunction, CUresult> {
    if module.is_null() {
        return Err(CUresult::CUDA_ERROR_INVALID_VALUE);
    }
    let mut func = std::ptr::null_mut();
    let result = {
        // SAFETY: `module` is a CUDA module handle and `name` is a NUL-terminated
        // function symbol for the duration of the FFI call.
        unsafe { cudarc::driver::sys::cuModuleGetFunction(&mut func, module, name.as_ptr()) }
    };
    if result != CUresult::CUDA_SUCCESS {
        return Err(result);
    }
    if func.is_null() {
        return Err(CUresult::CUDA_ERROR_INVALID_VALUE);
    }
    Ok(func)
}

/// Resolve a `.global` symbol's device pointer and byte size from a loaded
/// module. Used to locate the cooperative grid-barrier counter
/// (`_vyre_grid_barrier`) that the PTX emitter declares at module scope; the
/// host zeroes it before each cooperative launch.
pub(crate) fn get_cuda_module_global(
    module: CUmodule,
    name: &CStr,
) -> Result<(u64, usize), CUresult> {
    if module.is_null() {
        return Err(CUresult::CUDA_ERROR_INVALID_VALUE);
    }
    let mut dptr: cudarc::driver::sys::CUdeviceptr = 0;
    let mut bytes: usize = 0;
    let result = {
        // SAFETY: `module` is a loaded CUDA module handle and `name` is a
        // NUL-terminated symbol for the duration of the FFI call. `dptr`/`bytes`
        // are local out-params.
        unsafe {
            cudarc::driver::sys::cuModuleGetGlobal_v2(&mut dptr, &mut bytes, module, name.as_ptr())
        }
    };
    if result != CUresult::CUDA_SUCCESS {
        return Err(result);
    }
    if dptr == 0 || bytes == 0 {
        return Err(CUresult::CUDA_ERROR_INVALID_VALUE);
    }
    Ok((dptr, bytes))
}

pub(crate) fn unload_cuda_module(module: CUmodule) -> Result<(), CUresult> {
    if module.is_null() {
        return Ok(());
    }
    // SAFETY: `module` is an owned CUDA module handle; CUDA validates the
    // opaque handle and returns a CUresult.
    let result = unsafe { cudarc::driver::sys::cuModuleUnload(module) };
    if result == CUresult::CUDA_SUCCESS {
        Ok(())
    } else {
        Err(result)
    }
}

fn unload_cuda_module_or_log(module: CUmodule, label: &str) {
    if let Err(result) = unload_cuda_module(module) {
        tracing::error!(
            "Fix: cuModuleUnload failed during {label} with {result:?}; ensure all launches using the module have completed."
        );
    }
}

#[cfg(test)]
mod module_lifecycle_tests {
    use cudarc::driver::sys::CUresult;
    use std::collections::HashSet;

    #[test]
    fn cuda_module_lifecycle_helpers_reject_invalid_handles_before_ffi() {
        let main = std::ffi::CStr::from_bytes_with_nul(b"main\0")
            .expect("Fix: test CUDA module symbol must be NUL-terminated.");
        assert_eq!(
            super::load_cuda_module_data(b".version 8.0\n").unwrap_err(),
            CUresult::CUDA_ERROR_INVALID_VALUE
        );
        assert_eq!(
            super::get_cuda_module_function(std::ptr::null_mut(), main).unwrap_err(),
            CUresult::CUDA_ERROR_INVALID_VALUE
        );
        assert!(
            super::unload_cuda_module(std::ptr::null_mut()).is_ok(),
            "Fix: null CUDA module unload should be a no-op so cleanup paths can be idempotent."
        );
    }

    #[test]
    fn cuda_module_lifecycle_ffi_is_single_sourced_for_ptx_probe_and_cache() {
        let module_cache = include_str!("module_cache.rs");
        let ptx_target = include_str!("ptx_target.rs");
        let load_ffi = concat!("cudarc::driver::sys::", "cuModuleLoadData(");
        let get_ffi = concat!("cudarc::driver::sys::", "cuModuleGetFunction(");
        let unload_ffi = concat!("cudarc::driver::sys::", "cuModuleUnload(");

        assert_eq!(
            module_cache.matches(load_ffi).count(),
            1,
            "Fix: CUDA module loading must keep raw cuModuleLoadData behind load_cuda_module_data."
        );
        assert_eq!(
            module_cache.matches(get_ffi).count(),
            1,
            "Fix: CUDA module function lookup must keep raw cuModuleGetFunction behind get_cuda_module_function."
        );
        assert_eq!(
            module_cache.matches(unload_ffi).count(),
            1,
            "Fix: CUDA module unload must keep raw cuModuleUnload behind unload_cuda_module."
        );
        assert_eq!(
            ptx_target.matches(load_ffi).count()
                + ptx_target.matches(get_ffi).count()
                + ptx_target.matches(unload_ffi).count(),
            0,
            "Fix: PTX target probing must consume module lifecycle helpers instead of duplicating raw module FFI."
        );
        assert!(
            ptx_target.contains("load_cuda_module_data(cstring.as_bytes_with_nul())")
                && ptx_target.contains("unload_cuda_module(module)"),
            "Fix: PTX target probing must use shared module load/unload helpers."
        );
    }

    #[test]
    fn generated_module_keys_reuse_source_digest_without_ptx_rehash_churn() {
        let cache = super::CudaModuleCache::new();
        let mut seen = HashSet::new();

        for case in 0..4096u32 {
            let mut source_digest = [0u8; 32];
            source_digest[..4].copy_from_slice(&case.to_le_bytes());
            source_digest[4..8].copy_from_slice(&case.rotate_left(13).to_le_bytes());
            source_digest[8..12].copy_from_slice(&(!case).to_le_bytes());
            source_digest[12..16].copy_from_slice(&case.wrapping_mul(0x9e37_79b9).to_le_bytes());
            let source_key = super::PtxSourceCacheKey(source_digest);

            let key = cache
                .key_for_ptx_source_key(source_key, (9, 0))
                .expect("Fix: generated source digest module key must fit");
            assert_eq!(
                key,
                cache
                    .key_for_ptx_source_key(source_key, (9, 0))
                    .expect("Fix: repeated generated source digest module key must fit"),
                "Fix: PTX source digest to CUDA module key derivation must be stable for generated case {case}."
            );
            assert_ne!(
                key,
                cache
                    .key_for_ptx_source_key(source_key, (9, 1))
                    .expect("Fix: device-scoped generated source digest module key must fit"),
                "Fix: CUDA module keys must remain device-capability scoped for generated case {case}."
            );
            assert!(
                seen.insert(key.0),
                "Fix: generated PTX source digest case {case} collided in module-cache key space."
            );
        }
    }

    #[test]
    fn module_cache_keys_use_shared_domain_separated_identity_for_generated_inputs() {
        let cache = super::CudaModuleCache::new();

        for case in 0..2048u32 {
            let mut source_digest = [0u8; 32];
            let mut state = case ^ 0xCADA_CAFE;
            for (index, byte) in source_digest.iter_mut().enumerate() {
                state = state
                    .wrapping_mul(1_664_525)
                    .wrapping_add(1_013_904_223)
                    .rotate_left((index as u32) & 15);
                *byte = (state >> ((index & 3) * 8)) as u8;
            }
            let source_key = super::PtxSourceCacheKey(source_digest);
            let compute_capability = (7 + (case % 4), case.rotate_left(5) % 10);
            let key = cache
                .key_for_ptx_source_key(source_key, compute_capability)
                .expect("Fix: generated shared source-key module identity must fit");
            let repeated = cache
                .key_for_ptx_source_key(source_key, compute_capability)
                .expect("Fix: repeated generated shared source-key module identity must fit");
            assert_eq!(
                key, repeated,
                "Fix: shared CUDA module identity must be stable for generated source-key case {case}."
            );

            let mut changed_digest = source_digest;
            changed_digest[(case as usize) & 31] ^= 0x80 | (case as u8 & 0x7f);
            let changed_source_key = super::PtxSourceCacheKey(changed_digest);
            assert_ne!(
                key,
                cache
                    .key_for_ptx_source_key(changed_source_key, compute_capability)
                    .expect("Fix: changed generated source digest module identity must fit"),
                "Fix: one-byte PTX source digest mutations must change CUDA module keys for generated case {case}."
            );
            assert_ne!(
                key,
                cache
                    .key_for_ptx_source_key(
                        source_key,
                        (compute_capability.0, compute_capability.1.wrapping_add(1)),
                    )
                    .expect("Fix: changed generated device capability module identity must fit"),
                "Fix: CUDA module keys must include compute-capability scope for generated case {case}."
            );

            let raw_ptx = format!(
                "// generated raw ptx artifact {case}\n.version 8.0\n.target sm_{}{}\n.visible .entry main() {{ ret; }}\n",
                compute_capability.0, compute_capability.1
            );
            let raw_key = cache
                .key_for_raw_ptx_artifact(&raw_ptx, compute_capability)
                .expect("Fix: generated raw PTX artifact module identity must fit");
            let repeated_raw_key = cache
                .key_for_raw_ptx_artifact(&raw_ptx, compute_capability)
                .expect("Fix: repeated generated raw PTX artifact module identity must fit");
            assert_eq!(
                raw_key, repeated_raw_key,
                "Fix: raw PTX artifact module identity must be stable for generated case {case}."
            );
            assert_ne!(
                key, raw_key,
                "Fix: source-key and raw-artifact CUDA module cache domains must not alias for generated case {case}."
            );
        }
    }

    #[test]
    fn module_cache_key_derivation_is_single_sourced_through_driver_identity() {
        let source = include_str!("module_cache.rs");
        let helper_section = source
            .split("fn module_cache_key_from_domain_digest")
            .nth(1)
            .expect("Fix: module cache source must keep shared key helper visible")
            .split("/// Cache of lowered PTX text")
            .next()
            .expect("Fix: module cache source must keep helper before PTX source cache");
        let key_section = source
            .split("pub(crate) fn key_for_ptx_source_key")
            .nth(1)
            .expect("Fix: module cache source must keep PTX source-key derivation visible")
            .split("pub(crate) fn function_for_ptx")
            .next()
            .expect("Fix: module cache source must keep function lookup after key derivation");
        assert!(
            helper_section.contains("domain_separated_exact_input_key(")
                && key_section.contains("module_cache_key_from_domain_digest(")
                && key_section.contains("CUDA_MODULE_FROM_PTX_SOURCE_KEY_DOMAIN")
                && key_section.contains("CUDA_MODULE_FROM_RAW_PTX_ARTIFACT_DOMAIN"),
            "Fix: CUDA module cache keys must route through the shared domain-separated exact-input identity helper."
        );
        assert!(
            !helper_section.contains(&["blake", "3::Hasher::new()"].concat())
                && !key_section.contains(&["blake", "3::Hasher::new()"].concat()),
            "Fix: CUDA module cache source/raw-artifact key derivation must not fork local tuple hashing."
        );
    }

    /// Source-contract canary: the production section of
    /// `read_ptx_disk_cache_source_bounded` must use `checked_add`, not
    /// `saturating_add`, matching the pinning-counter contract used throughout
    /// this module.
    ///
    /// This test FAILS against the pre-fix code (which had `saturating_add`)
    /// and PASSES after the fix (which uses `checked_add`).
    ///
    /// Why not a fully-behavioral test?  The safety cap is 1 GiB; writing a
    /// 1 GiB temp file in a unit test would be prohibitively slow and disk-
    /// intensive.  The source-text canary is the correct mechanism here:
    /// `saturating_add` at `u64::MAX` would freeze `total`, disabling the
    /// cap check — any future edit that reintroduces it is caught immediately
    /// by this test failing, while the existing
    /// `module_cache_eviction_buffers_fit_soft_cap_inline` test reinforces
    /// the wider module-level contract.
    #[test]
    fn ptx_disk_cache_size_accumulator_uses_checked_not_saturating() {
        let source = include_str!("module_cache.rs");
        // Isolate the body of `read_ptx_disk_cache_source_bounded` up to the
        // next `fn` definition so we check only that function's accumulator.
        let fn_body = source
            .split("fn read_ptx_disk_cache_source_bounded(")
            .nth(1)
            .expect("Fix: read_ptx_disk_cache_source_bounded must exist in this file.")
            .split("\nfn ")
            .next()
            .expect("Fix: function body must be followed by another fn or EOF.");
        // Match the CALL syntax `saturating_add(`, not the bare word: the
        // accumulator's explanatory comment legitimately mentions
        // "saturating_add" when stating why it is forbidden, and a bare-word
        // match would false-positive on that comment even though the code is
        // correct. Only an actual `.saturating_add(` call is a regression.
        assert!(
            !fn_body.contains("saturating_add("),
            "Fix: PTX disk-cache size accumulator must use checked_add, not saturating_add — \
             every other counter in this module uses the checked pinning contract. \
             saturating_add freezes at u64::MAX, bypassing the safety-cap check."
        );
        assert!(
            fn_body.contains("checked_add"),
            "Fix: PTX disk-cache size accumulator must use checked_add to match the \
             pinning-counter contract used by every other counter in this module."
        );
    }
}

fn write_ptx_dump(
    dir: std::ffi::OsString,
    ptx_src: &str,
    env_name: &'static str,
) -> Result<std::path::PathBuf, BackendError> {
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).map_err(|error| BackendError::KernelCompileFailed {
        backend: crate::CUDA_BACKEND_ID.to_string(),
        compiler_message: format!(
            "{env_name} points at `{}` but the directory could not be created: {error}. Fix: choose a writable PTX dump directory or unset {env_name}.",
            dir.display()
        ),
    })?;
    let hash = blake3::hash(ptx_src.as_bytes());
    let path = dir.join(format!("ptx-{}.ptx", &hash.to_hex().as_str()[..16]));
    std::fs::write(&path, ptx_src).map_err(|error| BackendError::KernelCompileFailed {
        backend: crate::CUDA_BACKEND_ID.to_string(),
        compiler_message: format!(
            "{env_name} could not write PTX dump `{}`: {error}. Fix: choose a writable PTX dump directory or unset {env_name}.",
            path.display()
        ),
    })?;
    Ok(path)
}
