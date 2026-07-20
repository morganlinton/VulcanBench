//! Shared persistent cache for backend compiled-pipeline blobs.

use super::hashing::{
    dispatch_policy_cache_digest, dispatch_policy_cache_string, hex_encode,
    normalized_program_cache_digest, try_normalized_program_cache_digest,
    PipelineDeviceFingerprint,
};
use super::CURRENT_PIPELINE_CACHE_KEY_VERSION;
use crate::backend::DispatchConfig;
use std::sync::{Arc, MutexGuard};
use vyre_foundation::ir::Program;
use vyre_spec::BackendId;

/// Maximum persistent pipeline blob read into memory.
pub const MAX_DISK_PIPELINE_BLOB_BYTES: u64 = 64 * 1024 * 1024;

/// Disk cache for compiled pipeline blobs keyed by program and device.
pub struct DiskPipelineCache {
    root: std::path::PathBuf,
    pending_flushes: std::sync::Mutex<Vec<std::path::PathBuf>>,
}

impl DiskPipelineCache {
    fn try_lock_pending_flushes(&self) -> std::io::Result<MutexGuard<'_, Vec<std::path::PathBuf>>> {
        self.pending_flushes.lock().map_err(|error| {
            std::io::Error::other(format!(
                "Vyre disk pipeline cache pending-flush lock was poisoned: {error}. Fix: discard this cache instance after a panic; continuing could lose or duplicate compiled-pipeline fsync work."
            ))
        })
    }

    /// Open a cache rooted at `root`.
    ///
    /// # Errors
    ///
    /// Returns when the root directory cannot be created.
    pub fn open(root: impl Into<std::path::PathBuf>) -> std::io::Result<Self> {
        let root = root.into();
        std::fs::create_dir_all(&root)?;
        Ok(Self {
            root,
            pending_flushes: std::sync::Mutex::new(Vec::new()),
        })
    }

    /// Default cache directory.
    #[must_use]
    pub fn default_root() -> std::path::PathBuf {
        if let Some(xdg) = std::env::var_os("XDG_CACHE_HOME") {
            return std::path::PathBuf::from(xdg).join("vyre").join("pipelines");
        }
        if let Some(home) = std::env::var_os("HOME") {
            #[cfg(target_os = "macos")]
            {
                return std::path::PathBuf::from(home)
                    .join("Library")
                    .join("Caches")
                    .join("vyre")
                    .join("pipelines");
            }
            #[cfg(not(target_os = "macos"))]
            {
                return std::path::PathBuf::from(home)
                    .join(".cache")
                    .join("vyre")
                    .join("pipelines");
            }
        }
        if let Some(appdata) = std::env::var_os("LOCALAPPDATA") {
            return std::path::PathBuf::from(appdata)
                .join("vyre")
                .join("pipelines");
        }
        std::path::PathBuf::from("./vyre-cache/pipelines")
    }

    /// Derive the cache path for a program digest and device fingerprint.
    #[must_use]
    pub fn path_for(
        &self,
        program_digest: [u8; 32],
        fingerprint: PipelineDeviceFingerprint,
    ) -> std::path::PathBuf {
        let key = fingerprint.cache_key(program_digest);
        let mut file_name = hex_encode(&key);
        let mut path = self.root.join(&file_name[..2]);
        file_name.push_str(".bin");
        path.push(file_name);
        path
    }

    /// Read a cached blob. Returns `None` on a miss.
    ///
    /// # Errors
    ///
    /// Returns when an existing entry cannot be read.
    pub fn read(
        &self,
        program_digest: [u8; 32],
        fingerprint: PipelineDeviceFingerprint,
    ) -> std::io::Result<Option<Vec<u8>>> {
        let path = self.path_for(program_digest, fingerprint);
        match read_bounded(&path, MAX_DISK_PIPELINE_BLOB_BYTES) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }

    /// Write a cache blob with atomic install.
    ///
    /// # Errors
    ///
    /// Returns when the entry is oversized or cannot be written.
    pub fn write(
        &self,
        program_digest: [u8; 32],
        fingerprint: PipelineDeviceFingerprint,
        bytes: &[u8],
    ) -> std::io::Result<()> {
        if bytes.len() as u64 > MAX_DISK_PIPELINE_BLOB_BYTES {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("pipeline cache blob exceeds {MAX_DISK_PIPELINE_BLOB_BYTES} byte limit"),
            ));
        }
        let path = self.path_for(program_digest, fingerprint);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp = self.tmp_path_for(&path);
        let write_result = (|| -> std::io::Result<()> {
            let mut file = std::fs::File::create(&tmp)?;
            use std::io::Write as _;
            file.write_all(bytes)?;
            drop(file);
            std::fs::rename(&tmp, &path)
        })();
        if write_result.is_err() {
            remove_failed_atomic_write(&tmp)?;
        }
        write_result?;
        self.try_lock_pending_flushes()?.push(path);
        Ok(())
    }

    /// Durably flush entries written by [`Self::write`].
    ///
    /// # Errors
    ///
    /// Returns when a pending path cannot be flushed.
    pub fn flush(&self) -> std::io::Result<()> {
        let paths = {
            let mut pending = self.try_lock_pending_flushes()?;
            pending.sort();
            pending.dedup();
            std::mem::take(&mut *pending)
        };
        if let Err(error) = flush_paths(&paths) {
            self.try_lock_pending_flushes()?.extend(paths);
            return Err(error);
        }
        Ok(())
    }

    /// Remove entries selected by an impact mask.
    ///
    /// # Errors
    ///
    /// Returns when an impacted entry exists but cannot be removed.
    pub fn invalidate_impacted(
        &self,
        impact_mask: &[u32],
        program_digests: &[[u8; 32]],
        fingerprint: PipelineDeviceFingerprint,
    ) -> std::io::Result<()> {
        for (index, &is_impacted) in impact_mask.iter().enumerate() {
            if is_impacted != 0 {
                if let Some(&digest) = program_digests.get(index) {
                    let path = self.path_for(digest, fingerprint);
                    if path.exists() {
                        std::fs::remove_file(path)?;
                    }
                }
            }
        }
        Ok(())
    }

    /// Root directory used by this cache.
    #[must_use]
    pub fn root(&self) -> &std::path::Path {
        &self.root
    }

    fn tmp_path_for(&self, path: &std::path::Path) -> std::path::PathBuf {
        static TMP_COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let tmp_id = TMP_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        path.with_extension(format!("bin.tmp.{}.{}", std::process::id(), tmp_id))
    }
}

fn remove_failed_atomic_write(path: &std::path::Path) -> std::io::Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn read_bounded(path: &std::path::Path, max_bytes: u64) -> std::io::Result<Vec<u8>> {
    use std::io::Read as _;

    let mut file = std::fs::File::open(path)?;
    let metadata = file.metadata()?;
    if metadata.len() > max_bytes {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("pipeline cache blob exceeds {max_bytes} byte limit"),
        ));
    }
    let byte_capacity = usize::try_from(metadata.len()).map_err(|error| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!(
                "pipeline cache blob length {} does not fit usize: {error}",
                metadata.len()
            ),
        )
    })?;
    let mut bytes = Vec::new();
    crate::allocation::try_reserve_vec_to_capacity(&mut bytes, byte_capacity).map_err(|error| {
        std::io::Error::new(
            std::io::ErrorKind::OutOfMemory,
            format!(
                "pipeline cache bounded read could not reserve {byte_capacity} byte(s): {error}. Fix: lower the pipeline cache blob limit or evict oversized entries."
            ),
        )
    })?;
    file.by_ref().take(max_bytes + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > max_bytes {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("pipeline cache blob exceeded {max_bytes} byte bounded read limit"),
        ));
    }
    Ok(bytes)
}

fn flush_paths(paths: &[std::path::PathBuf]) -> std::io::Result<()> {
    let mut parents = Vec::new();
    crate::allocation::try_reserve_vec_to_capacity(&mut parents, paths.len()).map_err(|error| {
        std::io::Error::new(
            std::io::ErrorKind::OutOfMemory,
            format!(
                "pipeline cache flush could not reserve {} parent path slot(s): {error}. Fix: flush fewer cache paths per batch.",
                paths.len()
            ),
        )
    })?;
    sync_files_bounded(
        paths,
        std::fs::File::sync_data,
        "disk cache file sync worker panicked",
    )?;
    for path in paths {
        if let Some(parent) = path.parent() {
            parents.push(parent.to_path_buf());
        }
    }
    parents.sort();
    parents.dedup();
    sync_parent_dirs(&parents)?;
    Ok(())
}

#[cfg(unix)]
fn sync_parent_dirs(parents: &[std::path::PathBuf]) -> std::io::Result<()> {
    sync_files_bounded(
        parents,
        std::fs::File::sync_all,
        "disk cache dir sync worker panicked",
    )
}

#[cfg(not(unix))]
fn sync_parent_dirs(_parents: &[std::path::PathBuf]) -> std::io::Result<()> {
    Ok(())
}

fn sync_files_bounded(
    paths: &[std::path::PathBuf],
    sync: fn(&std::fs::File) -> std::io::Result<()>,
    panic_message: &'static str,
) -> std::io::Result<()> {
    if paths.is_empty() {
        return Ok(());
    }
    let workers = std::thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(1)
        .clamp(1, 16);
    for chunk in paths.chunks(workers) {
        std::thread::scope(|scope| {
            let mut handles = Vec::new();
            crate::allocation::try_reserve_vec_to_capacity(&mut handles, chunk.len()).map_err(|error| {
                std::io::Error::new(
                    std::io::ErrorKind::OutOfMemory,
                    format!(
                        "pipeline cache sync could not reserve {} worker handle(s): {error}. Fix: lower pipeline cache sync fan-out.",
                        chunk.len()
                    ),
                )
            })?;
            for path in chunk {
                handles.push(scope.spawn(move || {
                    let file = std::fs::File::open(path)?;
                    sync(&file)
                }));
            }
            for handle in handles {
                handle
                    .join()
                    .map_err(|_| std::io::Error::other(panic_message))??;
            }
            Ok::<(), std::io::Error>(())
        })?;
    }
    Ok(())
}

/// Capability bits that participate in pipeline-cache identity.
///
/// Two otherwise-identical pipelines compiled with different
/// `PipelineFeatureFlags` produce different cache keys  -  a pipeline
/// that assumed subgroup-op support cannot be reused on an adapter
/// that does not expose subgroup ops even if the shader bytes match.
///
/// Encoded as a bitfield so the wire form is compact and trivially
/// hashable. Bits `0x01..0x80` are allocated here; higher bits are
/// reserved for additive backend capability flags.
#[derive(
    Copy, Clone, Debug, Default, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct PipelineFeatureFlags(pub u32);

impl PipelineFeatureFlags {
    /// Pipeline was compiled against a lowering that emits subgroup /
    /// wave intrinsics.
    pub const SUBGROUP_OPS: Self = Self(1 << 0);
    /// Pipeline was compiled with native `f16` support.
    pub const F16: Self = Self(1 << 1);
    /// Pipeline was compiled with native `bf16` support.
    pub const BF16: Self = Self(1 << 2);
    /// Pipeline was compiled with tensor-core / matrix-engine
    /// intrinsics enabled.
    pub const TENSOR_CORES: Self = Self(1 << 3);
    /// Pipeline expects an async-compute queue at dispatch time.
    pub const ASYNC_COMPUTE: Self = Self(1 << 4);
    /// Pipeline expects push-constant support at dispatch time.
    pub const PUSH_CONSTANTS: Self = Self(1 << 5);
    /// Pipeline emits indirect-dispatch commands.
    pub const INDIRECT_DISPATCH: Self = Self(1 << 6);
    /// Pipeline was compiled for speculative (fused prefilter+confirmer)
    /// dispatch.
    pub const SPECULATIVE: Self = Self(1 << 7);
    /// Pipeline was compiled for persistent-thread (device-side work queue)
    /// dispatch.
    pub const PERSISTENT_THREAD: Self = Self(1 << 8);

    /// Empty flag set.
    #[must_use]
    pub const fn empty() -> Self {
        Self(0)
    }

    /// Contains at least every bit of `other`.
    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Union of two flag sets.
    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Raw bit representation.
    #[must_use]
    pub const fn bits(self) -> u32 {
        self.0
    }
}

/// Versioned pipeline-cache key shared by every backend.
///
/// Replaces the pre-0.6 pattern of using a raw blake3 hash as the key.
/// A raw hash is not robust: two pipelines that should miss (different
/// bind-group layout, different push-constant size, different
/// workgroup-size selection) hashed identically because the hash
/// covered the shader source only. Silent cache hits against a
/// non-equivalent pipeline are a correctness hazard (wrong bind-group
/// layout binds undefined data; wrong workgroup-size launches beyond
/// guarantees).
///
/// `#[non_exhaustive]` is enforced at the type level via the private
/// `__phantom` field: external callers construct keys through
/// [`PipelineCacheKey::new`] and cannot match exhaustively, so additive
/// key fields do not break downstream matches.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]

pub struct PipelineCacheKey {
    /// Key format version. Bumped to invalidate every cache entry
    /// without an API break.
    pub version: u32,
    /// blake3 hash of the canonical backend pipeline-source bytes.
    pub shader_hash: [u8; 32],
    /// Structural hash of the bind-group layout descriptors. Not the
    /// backend handle; the bytes that describe slot count, types,
    /// visibility, and access modes per bind group.
    pub bind_group_layout_hash: [u8; 32],
    /// Push-constant range in bytes. Included so a pipeline compiled
    /// for 16 B push constants never reuses against a layout that
    /// expects 32 B.
    pub push_constant_size: u32,
    /// Workgroup-size `[x, y, z]` the pipeline was specialized for.
    pub workgroup_size: [u32; 3],
    /// Feature-flag bits the pipeline assumes at dispatch time.
    pub feature_flags: PipelineFeatureFlags,
    /// Backend identity. Prevents pipelines from different backends from
    /// colliding when they happen to produce identical shader hashes.
    pub backend_id: BackendId,
    /// Reserved private field so `PipelineCacheKey` cannot be
    /// constructed by structural literal (forward-compatibility lever).
    #[allow(dead_code)]
    __phantom: core::marker::PhantomData<()>,
}

impl PipelineCacheKey {
    /// Construct a key at the current version.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        shader_hash: [u8; 32],
        bind_group_layout_hash: [u8; 32],
        push_constant_size: u32,
        workgroup_size: [u32; 3],
        feature_flags: PipelineFeatureFlags,
        backend_id: BackendId,
    ) -> Self {
        Self {
            version: CURRENT_PIPELINE_CACHE_KEY_VERSION,
            shader_hash,
            bind_group_layout_hash,
            push_constant_size,
            workgroup_size,
            feature_flags,
            backend_id,
            __phantom: core::marker::PhantomData,
        }
    }
}

/// Shared in-memory identity for a backend-compiled pipeline.
///
/// Backends may keep their own cache maps and compiled handles, but this object
/// keeps the identity facts single-sourced: normalized Program digest, dispatch
/// policy digest, device/runtime fingerprint, and the final digest used as the
/// lookup key.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct PipelineCacheIdentity {
    /// Final tuple-boundary-preserving lookup digest.
    pub digest: [u8; 32],
    /// Normalized Program digest used by backend pipeline caches.
    pub program_digest: [u8; 32],
    /// Dispatch policy fields that alter generated backend code.
    pub policy_digest: [u8; 32],
    /// Backend/device/runtime identity participating in the final digest.
    pub device_fingerprint: PipelineDeviceFingerprint,
}

impl PipelineCacheIdentity {
    /// Build identity from already-computed shared components.
    #[must_use]
    pub fn from_parts(
        program_digest: [u8; 32],
        policy_digest: [u8; 32],
        device_fingerprint: PipelineDeviceFingerprint,
    ) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"vyre-pipeline-cache-identity-v1\0program\0");
        hasher.update(&program_digest);
        hasher.update(b"\0policy\0");
        hasher.update(&policy_digest);
        hasher.update(b"\0vendor\0");
        hasher.update(&device_fingerprint.vendor.to_le_bytes());
        hasher.update(b"\0device\0");
        hasher.update(&device_fingerprint.device.to_le_bytes());
        hasher.update(b"\0driver\0");
        hasher.update(&device_fingerprint.driver_digest);
        Self {
            digest: *hasher.finalize().as_bytes(),
            program_digest,
            policy_digest,
            device_fingerprint,
        }
    }

    /// Build identity from a public Program and dispatch config.
    ///
    /// # Errors
    ///
    /// Returns when the Program cannot be serialized into stable cache
    /// identity. Callers should surface the error through their backend error
    /// type instead of hashing invalid IR lossy.
    pub fn try_from_program(
        program: &Program,
        config: &DispatchConfig,
        device_fingerprint: PipelineDeviceFingerprint,
    ) -> Result<Self, String> {
        let program_digest = try_normalized_program_cache_digest(program)?;
        let policy_digest = dispatch_policy_cache_digest(config);
        Ok(Self::from_parts(
            program_digest,
            policy_digest,
            device_fingerprint,
        ))
    }
}

/// Evidence a backend can provide when a pipeline-cache lookup misses.
///
/// The classifier is intentionally backend-neutral: a concrete driver keeps its
/// own fast cache key, but records enough adjacent identity facts to explain why
/// a miss happened without duplicating per-backend reason logic.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct PipelineCacheMissEvidence {
    /// Number of compiled pipeline entries present at lookup time.
    pub total_entries: usize,
    /// Entries whose normalized Program digest matched the requested program.
    pub same_program_entries: usize,
    /// Entries whose Program digest and dispatch policy digest both matched.
    pub same_program_and_policy_entries: usize,
    /// Entries whose Program digest, dispatch policy digest, and
    /// device/runtime fingerprint all matched.
    pub same_program_policy_and_device_entries: usize,
}

impl PipelineCacheMissEvidence {
    /// Build miss evidence from cached identities adjacent to the requested key.
    #[must_use]
    pub fn from_identities<'a>(
        cached: impl Iterator<Item = &'a PipelineCacheIdentity>,
        requested: &PipelineCacheIdentity,
    ) -> Self {
        let mut evidence = Self::default();
        for identity in cached {
            evidence.total_entries += 1;
            if identity.program_digest == requested.program_digest {
                evidence.same_program_entries += 1;
                if identity.policy_digest == requested.policy_digest {
                    evidence.same_program_and_policy_entries += 1;
                    if identity.device_fingerprint == requested.device_fingerprint {
                        evidence.same_program_policy_and_device_entries += 1;
                    }
                }
            }
        }
        evidence
    }
}

/// Backend-neutral cache-miss reason for operator telemetry.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum PipelineCacheMissReason {
    /// The backend had no compiled entries at lookup time.
    EmptyCache,
    /// Existing entries were for different normalized programs.
    ProgramChanged,
    /// The same program existed, but dispatch policy changed the generated
    /// pipeline identity.
    DispatchPolicyChanged,
    /// Program and dispatch policy matched, but device/runtime identity changed.
    DeviceOrRuntimeChanged,
    /// The supplied evidence says adjacent identity matched but the final key
    /// still missed; this catches future key fields and malformed bookkeeping.
    KeyAbsent,
}

impl PipelineCacheMissReason {
    /// Classify a miss from backend-supplied adjacent identity evidence.
    #[must_use]
    pub const fn classify(evidence: PipelineCacheMissEvidence) -> Self {
        if evidence.total_entries == 0 {
            Self::EmptyCache
        } else if evidence.same_program_entries == 0 {
            Self::ProgramChanged
        } else if evidence.same_program_and_policy_entries == 0 {
            Self::DispatchPolicyChanged
        } else if evidence.same_program_policy_and_device_entries == 0 {
            Self::DeviceOrRuntimeChanged
        } else {
            Self::KeyAbsent
        }
    }

    /// Classify a miss directly from cached pipeline identities.
    #[must_use]
    pub fn classify_identities<'a>(
        cached: impl Iterator<Item = &'a PipelineCacheIdentity>,
        requested: &PipelineCacheIdentity,
    ) -> Self {
        Self::classify(PipelineCacheMissEvidence::from_identities(
            cached, requested,
        ))
    }

    /// Stable metric suffix for backend metric snapshots.
    #[must_use]
    pub const fn metric_suffix(self) -> &'static str {
        match self {
            Self::EmptyCache => "empty_cache",
            Self::ProgramChanged => "program_changed",
            Self::DispatchPolicyChanged => "dispatch_policy_changed",
            Self::DeviceOrRuntimeChanged => "device_or_runtime_changed",
            Self::KeyAbsent => "key_absent",
        }
    }
}

#[cfg(test)]

mod pipeline_cache_key_tests {
    use super::*;

    fn hash32(byte: u8) -> [u8; 32] {
        [byte; 32]
    }

    #[test]
    fn different_workgroup_size_differs() {
        let a = PipelineCacheKey::new(
            hash32(1),
            hash32(2),
            0,
            [64, 1, 1],
            PipelineFeatureFlags::empty(),
            BackendId::from("backend-a"),
        );
        let b = PipelineCacheKey::new(
            hash32(1),
            hash32(2),
            0,
            [128, 1, 1],
            PipelineFeatureFlags::empty(),
            BackendId::from("backend-a"),
        );
        assert_ne!(a, b);
    }

    #[test]
    fn different_feature_flags_differ() {
        let a = PipelineCacheKey::new(
            hash32(1),
            hash32(2),
            0,
            [1, 1, 1],
            PipelineFeatureFlags::empty(),
            BackendId::from("backend-a"),
        );
        let b = PipelineCacheKey::new(
            hash32(1),
            hash32(2),
            0,
            [1, 1, 1],
            PipelineFeatureFlags::SUBGROUP_OPS,
            BackendId::from("backend-a"),
        );
        assert_ne!(a, b);
    }

    #[test]
    fn different_backend_id_differs() {
        let a = PipelineCacheKey::new(
            hash32(1),
            hash32(2),
            0,
            [1, 1, 1],
            PipelineFeatureFlags::empty(),
            BackendId::from("backend-a"),
        );
        let b = PipelineCacheKey::new(
            hash32(1),
            hash32(2),
            0,
            [1, 1, 1],
            PipelineFeatureFlags::empty(),
            BackendId::from("backend-b"),
        );
        assert_ne!(a, b);
    }

    #[test]
    fn flag_containment_is_correct() {
        let a = PipelineFeatureFlags::SUBGROUP_OPS.union(PipelineFeatureFlags::F16);
        assert!(a.contains(PipelineFeatureFlags::SUBGROUP_OPS));
        assert!(a.contains(PipelineFeatureFlags::F16));
        assert!(!a.contains(PipelineFeatureFlags::TENSOR_CORES));
    }

    #[test]
    fn version_is_current() {
        let k = PipelineCacheKey::new(
            hash32(1),
            hash32(2),
            0,
            [1, 1, 1],
            PipelineFeatureFlags::empty(),
            BackendId::from("backend-a"),
        );
        assert_eq!(k.version, CURRENT_PIPELINE_CACHE_KEY_VERSION);
    }

    #[test]
    fn shared_cache_identity_separates_program_policy_and_device_facts() {
        let program_a = hash32(1);
        let program_b = hash32(2);
        let policy_a = hash32(3);
        let policy_b = hash32(4);
        let device_a = PipelineDeviceFingerprint::from_parts(1, 2, "driver-a", "runtime-a");
        let device_b = PipelineDeviceFingerprint::from_parts(1, 2, "driver-a", "runtime-b");

        let base = PipelineCacheIdentity::from_parts(program_a, policy_a, device_a);

        assert_eq!(base.program_digest, program_a);
        assert_eq!(base.policy_digest, policy_a);
        assert_eq!(base.device_fingerprint, device_a);
        assert_ne!(
            base.digest,
            PipelineCacheIdentity::from_parts(program_b, policy_a, device_a).digest,
            "Fix: shared pipeline cache identity must include the normalized Program digest."
        );
        assert_ne!(
            base.digest,
            PipelineCacheIdentity::from_parts(program_a, policy_b, device_a).digest,
            "Fix: shared pipeline cache identity must include dispatch policy as its own tuple field."
        );
        assert_ne!(
            base.digest,
            PipelineCacheIdentity::from_parts(program_a, policy_a, device_b).digest,
            "Fix: shared pipeline cache identity must include device/runtime fingerprint facts."
        );
    }

    #[test]
    fn miss_reason_classifies_adjacent_identity_evidence() {
        assert_eq!(
            PipelineCacheMissReason::classify(PipelineCacheMissEvidence {
                total_entries: 0,
                same_program_entries: 0,
                same_program_and_policy_entries: 0,
                same_program_policy_and_device_entries: 0,
            }),
            PipelineCacheMissReason::EmptyCache
        );
        assert_eq!(
            PipelineCacheMissReason::classify(PipelineCacheMissEvidence {
                total_entries: 3,
                same_program_entries: 0,
                same_program_and_policy_entries: 0,
                same_program_policy_and_device_entries: 0,
            }),
            PipelineCacheMissReason::ProgramChanged
        );
        assert_eq!(
            PipelineCacheMissReason::classify(PipelineCacheMissEvidence {
                total_entries: 3,
                same_program_entries: 2,
                same_program_and_policy_entries: 0,
                same_program_policy_and_device_entries: 0,
            }),
            PipelineCacheMissReason::DispatchPolicyChanged
        );
        assert_eq!(
            PipelineCacheMissReason::classify(PipelineCacheMissEvidence {
                total_entries: 3,
                same_program_entries: 2,
                same_program_and_policy_entries: 1,
                same_program_policy_and_device_entries: 0,
            }),
            PipelineCacheMissReason::DeviceOrRuntimeChanged
        );
        assert_eq!(
            PipelineCacheMissReason::classify(PipelineCacheMissEvidence {
                total_entries: 3,
                same_program_entries: 2,
                same_program_and_policy_entries: 1,
                same_program_policy_and_device_entries: 1,
            }),
            PipelineCacheMissReason::KeyAbsent
        );
    }

    #[test]
    fn miss_reason_metric_suffixes_are_stable_snake_case() {
        assert_eq!(
            PipelineCacheMissReason::EmptyCache.metric_suffix(),
            "empty_cache"
        );
        assert_eq!(
            PipelineCacheMissReason::ProgramChanged.metric_suffix(),
            "program_changed"
        );
        assert_eq!(
            PipelineCacheMissReason::DispatchPolicyChanged.metric_suffix(),
            "dispatch_policy_changed"
        );
        assert_eq!(
            PipelineCacheMissReason::DeviceOrRuntimeChanged.metric_suffix(),
            "device_or_runtime_changed"
        );
        assert_eq!(
            PipelineCacheMissReason::KeyAbsent.metric_suffix(),
            "key_absent"
        );
    }

    #[test]
    fn miss_reason_classifies_cached_shared_identities() {
        let program = hash32(1);
        let other_program = hash32(2);
        let policy = hash32(3);
        let other_policy = hash32(4);
        let device = PipelineDeviceFingerprint::from_parts(1, 2, "driver-a", "runtime-a");
        let other_device = PipelineDeviceFingerprint::from_parts(1, 2, "driver-a", "runtime-b");
        let requested = PipelineCacheIdentity::from_parts(program, policy, device);

        assert_eq!(
            PipelineCacheMissReason::classify_identities([].iter(), &requested),
            PipelineCacheMissReason::EmptyCache
        );
        assert_eq!(
            PipelineCacheMissReason::classify_identities(
                [PipelineCacheIdentity::from_parts(
                    other_program,
                    policy,
                    device
                )]
                .iter(),
                &requested,
            ),
            PipelineCacheMissReason::ProgramChanged
        );
        assert_eq!(
            PipelineCacheMissReason::classify_identities(
                [PipelineCacheIdentity::from_parts(
                    program,
                    other_policy,
                    device
                )]
                .iter(),
                &requested,
            ),
            PipelineCacheMissReason::DispatchPolicyChanged
        );
        assert_eq!(
            PipelineCacheMissReason::classify_identities(
                [PipelineCacheIdentity::from_parts(
                    program,
                    policy,
                    other_device
                )]
                .iter(),
                &requested,
            ),
            PipelineCacheMissReason::DeviceOrRuntimeChanged
        );
        assert_eq!(
            PipelineCacheMissReason::classify_identities(
                [PipelineCacheIdentity::from_parts(program, policy, device)].iter(),
                &requested,
            ),
            PipelineCacheMissReason::KeyAbsent
        );
    }

    #[test]
    fn poisoned_pending_flush_lock_returns_structured_error() {
        let cache = Arc::new(DiskPipelineCache {
            root: std::env::temp_dir(),
            pending_flushes: std::sync::Mutex::new(Vec::new()),
        });
        let poisoned = Arc::clone(&cache);
        let _ = std::thread::spawn(move || {
            let _guard = poisoned
                .try_lock_pending_flushes()
                .expect("Fix: first pending-flush lock acquisition should succeed");
            panic!("poison disk pipeline cache pending flushes");
        })
        .join();

        let error = cache
            .flush()
            .expect_err("poisoned disk pipeline cache must return an io error");
        let message = error.to_string();
        assert!(
            message.contains("pending-flush lock was poisoned"),
            "{message}"
        );
    }
}
