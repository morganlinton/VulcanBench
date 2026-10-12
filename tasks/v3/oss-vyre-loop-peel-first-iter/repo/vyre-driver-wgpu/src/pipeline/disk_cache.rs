//! Disk-backed WGSL + compiled-pipeline cache for compiled pipeline mode.

use fs2::FileExt;
use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use vyre_driver::{BackendError, DispatchConfig};
use vyre_foundation::ir::Program;
use vyre_foundation::serial::wire::framing::WIRE_FORMAT_VERSION;

use crate::staging_reserve::reserve_backend_vec;

#[cfg(test)]
pub(crate) use super::disk_cache_invalidation::set_test_disk_pipeline_cache_root;
pub(crate) use super::disk_cache_invalidation::{
    cache_entry_path, disk_pipeline_cache_dir, invalidate_impacted, CompiledPipelineMetadata,
    DiskPipelineMetadata,
};

const DISK_PIPELINE_CACHE_VERSION: u32 = 5;
const NAGA_VERSION: &str = env!("VYRE_NAGA_VERSION");
const WGSL_LOWERING_CONTRACT: &str =
    "vyre-wgpu-lowering-contract:v17:region-phi-named-carrier+ssa-carrier-snapshots+block-shadowed-carriers+carrier-rebind-invalidates-stale-blocks+restored-loop-and-block-carrier-scope+nonfinite-f32-bitcast+per-word-byte-compact+no-mutable-loop-unroll+licm-keeps-reassigned-loop-locals+runtime-storage-buffer-lengths+saturating-f32-to-int-cast";
const MAX_WGSL_CACHE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_COMPILED_PIPELINE_CACHE_BLOB_BYTES: u64 = 64 * 1024 * 1024;
const MAX_PIPELINE_CACHE_METADATA_BYTES: u64 = 64 * 1024;
const MAX_PENDING_DURABLE_CACHE_FILES: usize = 4096;

static PENDING_DURABLE_CACHE_FILES: OnceLock<Mutex<BTreeSet<PathBuf>>> = OnceLock::new();

pub(crate) struct CompiledPipelineCacheKey {
    pub(crate) hash: [u8; 32],
    pub(crate) adapter_fingerprint: String,
    pub(crate) cache_key: String,
    pub(crate) wgsl_blake3: String,
}

pub(crate) struct PipelineCacheHandle {
    pub(crate) cache: wgpu::PipelineCache,
}

pub(crate) fn load_or_compile_disk_wgsl(
    program: &Program,
    adapter_info: &wgpu::AdapterInfo,
    config: &DispatchConfig,
    enabled_features: &crate::runtime::device::EnabledFeatures,
) -> Result<String, BackendError> {
    let fingerprint = adapter_fingerprint(adapter_info);

    let norm_digest =
        vyre_driver::pipeline::try_normalized_program_cache_digest(program).map_err(|error| {
            BackendError::new(format!("WGSL disk pipeline cache digest failed: {error}"))
        })?;
    let cache_key = wgsl_cache_key(&norm_digest, &fingerprint, config);
    let cache_key_hex = hex_hash(&cache_key);
    let dir = disk_pipeline_cache_dir();
    let wgsl_path = cache_entry_path(&dir, &cache_key_hex, ".wgsl");
    let meta_path = cache_entry_path(&dir, &cache_key_hex, ".wgsl.toml");
    if let Ok(wgsl) = read_bounded_utf8(&wgsl_path, MAX_WGSL_CACHE_BYTES) {
        if wgsl_metadata_matches(&meta_path, &cache_key, &wgsl, &fingerprint, config) {
            return Ok(wgsl);
        }
    }
    let start = std::time::Instant::now();
    let wgsl = lower_wgsl(program, config, enabled_features)?;
    let elapsed = start.elapsed();
    tracing::info!(
        program_fingerprint = %cache_key_hex,
        elapsed_ms = elapsed.as_secs_f64() * 1000.0,
        "WGSL cache miss  -  cold cache or program shape changed"
    );
    persist_disk_wgsl(
        &dir,
        &wgsl_path,
        &meta_path,
        &cache_key,
        &wgsl,
        &fingerprint,
        config,
    )?;
    Ok(wgsl)
}

pub(crate) fn early_pipeline_cache_key(
    program: &Program,
    adapter_info: &wgpu::AdapterInfo,
    config: &DispatchConfig,
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"vyre-early-pipeline-cache-v1\0program\0");
    hasher.update(&program.fingerprint());
    hasher.update(b"\0adapter\0");
    update_adapter_fingerprint(&mut hasher, adapter_info);
    hasher.update(b"\0abi\0");
    hasher.update(&WIRE_FORMAT_VERSION.to_le_bytes());
    hasher.update(b"\0naga\0");
    hasher.update(NAGA_VERSION.as_bytes());
    update_wgsl_lowering_contract(&mut hasher);
    hasher.update(b"\0policy\0");
    vyre_driver::pipeline::update_dispatch_policy_cache_hash(&mut hasher, config);
    hasher.update(b"\0workgroup_override\0");
    if let Some(wg) = config.workgroup_override {
        for axis in wg {
            hasher.update(&axis.to_le_bytes());
        }
    }
    *hasher.finalize().as_bytes()
}

pub(crate) fn compiled_pipeline_cache_key(
    adapter_info: &wgpu::AdapterInfo,
    wgsl_source: &str,
) -> CompiledPipelineCacheKey {
    let adapter_fingerprint = adapter_fingerprint(adapter_info);
    let wgsl_blake3 = blake3_hex(wgsl_source.as_bytes());
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"vyre-compiled-pipeline-cache-v1\0");
    hasher.update(adapter_fingerprint.as_bytes());
    hasher.update(b"\0abi\0");
    hasher.update(&WIRE_FORMAT_VERSION.to_le_bytes());
    hasher.update(b"\0wgsl\0");
    hasher.update(wgsl_blake3.as_bytes());
    hasher.update(b"\0naga\0");
    hasher.update(NAGA_VERSION.as_bytes());
    let hash = *hasher.finalize().as_bytes();
    let cache_key = hex_hash(&hash);
    CompiledPipelineCacheKey {
        hash,
        adapter_fingerprint,
        cache_key,
        wgsl_blake3,
    }
}

pub(crate) fn create_compiled_pipeline_cache(
    device: &wgpu::Device,
    key: &CompiledPipelineCacheKey,
) -> Result<PipelineCacheHandle, BackendError> {
    let data = load_compiled_pipeline_blob(key)?;
    let cache = {
        #[allow(unsafe_code)]
        // SAFETY: FFI to wgpu / wgpu-hal native APIs. Handles + sizes are
        // validated by the surrounding cache layer; fallback=false makes a
        // broken advertised pipeline-cache feature fail loudly instead of
        // silently substituting an uncached driver path.
        unsafe {
            device.create_pipeline_cache(&wgpu::PipelineCacheDescriptor {
                label: Some("vyre persistent compiled pipeline cache"),
                data: data.as_deref(),
                fallback: false,
            })
        }
    };
    Ok(PipelineCacheHandle { cache })
}

pub(crate) fn persist_compiled_pipeline_cache(
    key: &CompiledPipelineCacheKey,
    cache: &wgpu::PipelineCache,
) -> Result<(), BackendError> {
    let Some(bytes) = cache.get_data() else {
        return Ok(());
    };
    let dir = disk_pipeline_cache_dir();
    let blob_path = cache_entry_path(&dir, &key.cache_key, ".pipeline.bin");
    let meta_path = cache_entry_path(&dir, &key.cache_key, ".pipeline.toml");
    let metadata = CompiledPipelineMetadata {
        version: DISK_PIPELINE_CACHE_VERSION,
        cache_key: key.hash,
        adapter_fingerprint: metadata_fingerprint(&key.adapter_fingerprint),
        wgsl_blake3: key.wgsl_blake3.clone(),
        program_abi_version: u32::from(WIRE_FORMAT_VERSION),
        naga_version: std::borrow::Cow::Borrowed(NAGA_VERSION),
        blob_bytes: bytes.len(),
        blob_blake3: blake3_hex(&bytes),
    };
    persist_bytes(&dir, &blob_path, &meta_path, &bytes, &metadata)
}

pub(crate) fn flush_disk_pipeline_cache() -> Result<(), BackendError> {
    let Some(pending) = PENDING_DURABLE_CACHE_FILES.get() else {
        return Ok(());
    };
    let paths = {
        let mut guard = pending.lock().map_err(BackendError::poisoned_lock)?;
        let mut paths = Vec::new();
        reserve_backend_vec(
            &mut paths,
            guard.len(),
            "pipeline cache pending flush path staging",
        )?;
        paths.extend(std::mem::take(&mut *guard));
        paths
    };
    if paths.is_empty() {
        return Ok(());
    }

    if let Err(error) = flush_disk_cache_paths(&paths) {
        let mut guard = pending.lock().map_err(BackendError::poisoned_lock)?;
        guard.extend(paths);
        return Err(error);
    }
    Ok(())
}

fn flush_disk_cache_paths(paths: &[PathBuf]) -> Result<(), BackendError> {
    sync_cache_files_bounded(paths, File::sync_data, "pipeline cache explicit flush")?;
    let mut parents = Vec::new();
    reserve_backend_vec(
        &mut parents,
        paths.len(),
        "pipeline cache parent directory staging",
    )?;
    for path in paths {
        if let Some(parent) = path.parent() {
            parents.push(parent.to_path_buf());
        }
    }
    parents.sort();
    parents.dedup();
    sync_parent_dirs_bounded(&parents)?;
    Ok(())
}

fn persist_disk_wgsl(
    dir: &Path,
    wgsl_path: &Path,
    meta_path: &Path,
    cache_key: &[u8; 32],
    wgsl: &str,
    fingerprint: &str,
    config: &DispatchConfig,
) -> Result<(), BackendError> {
    let metadata = DiskPipelineMetadata {
        version: DISK_PIPELINE_CACHE_VERSION,
        cache_key: *cache_key,
        wgsl_bytes: wgsl.len(),
        adapter_fingerprint: metadata_fingerprint(fingerprint),
        program_abi_version: u32::from(WIRE_FORMAT_VERSION),
        naga_version: std::borrow::Cow::Borrowed(NAGA_VERSION),
        wgsl_lowering_contract: std::borrow::Cow::Borrowed(WGSL_LOWERING_CONTRACT),
        policy: vyre_driver::pipeline::dispatch_policy_cache_string(config),
        wgsl_blake3: blake3_hex(wgsl.as_bytes()),
    };
    persist_bytes(dir, wgsl_path, meta_path, wgsl.as_bytes(), &metadata)
}

fn wgsl_metadata_matches(
    meta_path: &Path,
    cache_key: &[u8; 32],
    wgsl: &str,
    fingerprint: &str,
    config: &DispatchConfig,
) -> bool {
    let Ok(metadata) = read_metadata::<DiskPipelineMetadata>(meta_path) else {
        return false;
    };
    metadata.version == DISK_PIPELINE_CACHE_VERSION
        && metadata.cache_key == *cache_key
        && metadata.wgsl_bytes == wgsl.len()
        && metadata.adapter_fingerprint == metadata_fingerprint(fingerprint)
        && metadata.program_abi_version == u32::from(WIRE_FORMAT_VERSION)
        && metadata.naga_version == NAGA_VERSION
        && metadata.wgsl_lowering_contract == WGSL_LOWERING_CONTRACT
        && metadata.policy == vyre_driver::pipeline::dispatch_policy_cache_string(config)
        && metadata.wgsl_blake3 == blake3_hex(wgsl.as_bytes())
}

fn load_compiled_pipeline_blob(
    key: &CompiledPipelineCacheKey,
) -> Result<Option<Vec<u8>>, BackendError> {
    let dir = disk_pipeline_cache_dir();
    let blob_path = cache_entry_path(&dir, &key.cache_key, ".pipeline.bin");
    let meta_path = cache_entry_path(&dir, &key.cache_key, ".pipeline.toml");
    let Ok(metadata) = read_metadata::<CompiledPipelineMetadata>(&meta_path) else {
        tracing::warn!(
            cache_key = %key.cache_key,
            "compiled-pipeline cache miss  -  metadata missing or unreadable"
        );
        return Ok(None);
    };
    if metadata.version != DISK_PIPELINE_CACHE_VERSION
        || metadata.cache_key != key.hash
        || metadata.adapter_fingerprint != metadata_fingerprint(&key.adapter_fingerprint)
        || metadata.wgsl_blake3 != key.wgsl_blake3
        || metadata.program_abi_version != u32::from(WIRE_FORMAT_VERSION)
        || metadata.naga_version != NAGA_VERSION
    {
        tracing::warn!(
            cache_key = %key.cache_key,
            "compiled-pipeline cache miss  -  metadata does not match current adapter or compiler contract"
        );
        return Ok(None);
    }
    let metadata_blob_bytes = u64::try_from(metadata.blob_bytes).map_err(|source| {
        BackendError::new(format!(
            "compiled pipeline blob metadata length cannot fit u64: {source}. Fix: delete the corrupt cache entry."
        ))
    })?;
    if metadata_blob_bytes > MAX_COMPILED_PIPELINE_CACHE_BLOB_BYTES {
        tracing::warn!(
            cache_key = %key.cache_key,
            blob_bytes = metadata.blob_bytes,
            max_bytes = MAX_COMPILED_PIPELINE_CACHE_BLOB_BYTES,
            "compiled-pipeline cache miss  -  blob exceeds bounded cache read budget"
        );
        return Ok(None);
    }
    let bytes = read_bounded_bytes(&blob_path, MAX_COMPILED_PIPELINE_CACHE_BLOB_BYTES).map_err(
        |error| {
            BackendError::new(format!(
                "compiled pipeline cache blob `{}` could not be read: {error}. Fix: delete the corrupt cache entry or repair filesystem permissions.",
                blob_path.display()
            ))
        },
    )?;
    if bytes.len() != metadata.blob_bytes || blake3_hex(&bytes) != metadata.blob_blake3 {
        tracing::warn!(
            cache_key = %key.cache_key,
            "compiled-pipeline cache miss  -  blob length or digest mismatch"
        );
        return Ok(None);
    }
    Ok(Some(bytes))
}

fn trace_io_err(path: &Path, error: &std::io::Error, context: &str) {
    tracing::error!(path_id = %path_fingerprint(path), error_kind = ?error.kind(), "{context}");
}

fn persist_bytes<T: serde::Serialize>(
    dir: &Path,
    data_path: &Path,
    meta_path: &Path,
    bytes: &[u8],
    metadata: &T,
) -> Result<(), BackendError> {
    fs::create_dir_all(dir).map_err(|error| {
        trace_io_err(dir, &error, "pipeline cache directory is unwritable");
        BackendError::new(format!("failed to create pipeline cache dir: {error}"))
    })?;
    write_atomic(data_path, bytes, "pipeline cache data")?;
    let encoded = toml::to_string(metadata).map_err(|error| {
        BackendError::new(format!("failed to encode pipeline cache metadata: {error}"))
    })?;
    write_atomic(meta_path, encoded.as_bytes(), "pipeline cache metadata")
}

fn write_atomic(path: &Path, bytes: &[u8], label: &str) -> Result<(), BackendError> {
    static TMP_COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let tmp_id = TMP_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tmp_path = path.with_extension(format!("tmp.{}_{}", std::process::id(), tmp_id));
    let mut file = File::create(&tmp_path)
        .map_err(|error| BackendError::new(format!("failed to create {label}: {error}")))?;
    file.lock_exclusive()
        .map_err(|error| BackendError::new(error.to_string()))?;
    file.write_all(bytes)
        .map_err(|error| BackendError::new(error.to_string()))?;
    file.unlock()
        .map_err(|error| BackendError::new(error.to_string()))?;
    fs::rename(&tmp_path, path)
        .map_err(|error| BackendError::new(format!("failed to install {label}: {error}")))?;
    register_pending_durable_cache_file(path)?;
    Ok(())
}

fn register_pending_durable_cache_file(path: &Path) -> Result<(), BackendError> {
    let pending = PENDING_DURABLE_CACHE_FILES.get_or_init(|| Mutex::new(BTreeSet::new()));
    let should_flush = {
        let mut guard = pending.lock().map_err(BackendError::poisoned_lock)?;
        guard.insert(path.to_path_buf());
        guard.len() >= MAX_PENDING_DURABLE_CACHE_FILES
    };
    if should_flush {
        flush_disk_pipeline_cache()?;
    }
    Ok(())
}

#[cfg(unix)]
fn sync_parent_dirs_bounded(parents: &[PathBuf]) -> Result<(), BackendError> {
    sync_cache_files_bounded(parents, File::sync_all, "pipeline cache directory flush")
}

#[cfg(not(unix))]
fn sync_parent_dirs_bounded(_parents: &[PathBuf]) -> Result<(), BackendError> {
    Ok(())
}

fn sync_cache_files_bounded(
    paths: &[PathBuf],
    sync: fn(&File) -> std::io::Result<()>,
    context: &'static str,
) -> Result<(), BackendError> {
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
            reserve_backend_vec(
                &mut handles,
                chunk.len(),
                "pipeline cache flush worker staging",
            )?;
            for path in chunk {
                handles.push(scope.spawn(move || -> Result<(), BackendError> {
                    let file = File::open(path).map_err(|error| {
                        trace_io_err(path, &error, "pipeline cache flush open failed");
                        BackendError::new(format!(
                            "{context} failed to open {}: {error}. Fix: remove the corrupted cache entry and retry.",
                            path_fingerprint(path)
                        ))
                    })?;
                    sync(&file).map_err(|error| {
                        trace_io_err(path, &error, "pipeline cache flush fsync failed");
                        BackendError::new(format!(
                            "{context} failed for {}: {error}. Fix: check cache storage health and retry.",
                            path_fingerprint(path)
                        ))
                    })
                }));
            }
            for handle in handles {
                handle.join().map_err(|_| {
                    BackendError::new(format!("{context} worker panicked. Fix: retry the flush."))
                })??;
            }
            Ok::<(), BackendError>(())
        })?;
    }
    Ok(())
}

fn read_metadata<T: serde::de::DeserializeOwned>(meta_path: &Path) -> Result<T, ()> {
    let Ok(mut file) = File::open(meta_path) else {
        return Err(());
    };
    let Ok(metadata) = file.metadata() else {
        return Err(());
    };
    if metadata.len() > MAX_PIPELINE_CACHE_METADATA_BYTES {
        return Err(());
    }
    if file.lock_shared().is_err() {
        return Err(());
    }
    let capacity = usize::try_from(metadata.len()).map_err(|_| ())?;
    let bounded_read_limit = MAX_PIPELINE_CACHE_METADATA_BYTES.checked_add(1).ok_or(())?;
    let mut text = String::new();
    text.try_reserve_exact(capacity).map_err(|_| ())?;
    let res = Read::by_ref(&mut file)
        .take(bounded_read_limit)
        .read_to_string(&mut text);
    if file.unlock().is_err() {
        return Err(());
    }
    if res.is_err()
        || u64::try_from(text.len()).map_or(true, |len| len > MAX_PIPELINE_CACHE_METADATA_BYTES)
    {
        return Err(());
    }
    toml::from_str::<T>(&text).map_err(|_| ())
}

fn read_bounded_utf8(path: &Path, max_bytes: u64) -> std::io::Result<String> {
    let bytes = read_bounded_bytes(path, max_bytes)?;
    String::from_utf8(bytes)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))
}

fn read_bounded_bytes(path: &Path, max_bytes: u64) -> std::io::Result<Vec<u8>> {
    let mut file = File::open(path)?;
    let metadata = file.metadata()?;
    if metadata.len() > max_bytes {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "cache entry too large",
        ));
    }
    let capacity = usize::try_from(metadata.len()).map_err(|source| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("cache entry length cannot fit usize: {source}"),
        )
    })?;
    let bounded_read_limit = max_bytes.checked_add(1).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "cache entry max_bytes cannot add sentinel byte without overflowing u64",
        )
    })?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(capacity).map_err(|source| {
        std::io::Error::new(
            std::io::ErrorKind::OutOfMemory,
            format!("cache entry buffer could not reserve {capacity} bytes: {source}"),
        )
    })?;
    Read::by_ref(&mut file)
        .take(bounded_read_limit)
        .read_to_end(&mut bytes)?;
    if u64::try_from(bytes.len()).map_or(true, |len| len > max_bytes) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "cache entry exceeded bounded read limit",
        ));
    }
    Ok(bytes)
}

fn wgsl_cache_key(norm_digest: &[u8], fingerprint: &str, config: &DispatchConfig) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"vyre-pipeline-cache-v7\0norm\0");
    hasher.update(norm_digest);
    hasher.update(b"\0adapter\0");
    hasher.update(fingerprint.as_bytes());
    hasher.update(b"\0abi\0");
    hasher.update(&WIRE_FORMAT_VERSION.to_le_bytes());
    hasher.update(b"\0naga\0");
    hasher.update(NAGA_VERSION.as_bytes());
    update_wgsl_lowering_contract(&mut hasher);
    hasher.update(b"\0policy\0");
    vyre_driver::pipeline::update_dispatch_policy_cache_hash(&mut hasher, config);
    *hasher.finalize().as_bytes()
}

fn update_wgsl_lowering_contract(hasher: &mut blake3::Hasher) {
    hasher.update(b"\0wgsl_lowering_contract\0");
    hasher.update(WGSL_LOWERING_CONTRACT.as_bytes());
}

fn lower_wgsl(
    program: &Program,
    config: &DispatchConfig,
    enabled_features: &crate::runtime::device::EnabledFeatures,
) -> Result<String, BackendError> {
    crate::emit::lower_with_features(program, config, enabled_features)
        .map_err(|error| BackendError::new(error.to_string()))
}

fn adapter_fingerprint(adapter_info: &wgpu::AdapterInfo) -> String {
    let mut fingerprint = String::new();
    fingerprint.push_str(adapter_backend_name(adapter_info.backend));
    fingerprint.push(':');
    push_hex_u32(&mut fingerprint, adapter_info.vendor);
    fingerprint.push(':');
    push_hex_u32(&mut fingerprint, adapter_info.device);
    fingerprint.push(':');
    fingerprint.push_str(&adapter_info.driver);
    fingerprint.push(':');
    fingerprint.push_str(&adapter_info.driver_info);
    fingerprint
}

fn adapter_backend_name(backend: wgpu::Backend) -> &'static str {
    match backend {
        wgpu::Backend::Noop => "Noop",
        wgpu::Backend::Vulkan => "Vulkan",
        wgpu::Backend::Metal => "Metal",
        wgpu::Backend::Dx12 => "Dx12",
        wgpu::Backend::Gl => "Gl",
        wgpu::Backend::BrowserWebGpu => "BrowserWebGpu",
    }
}

fn update_adapter_fingerprint(hasher: &mut blake3::Hasher, adapter_info: &wgpu::AdapterInfo) {
    hasher.update(adapter_backend_name(adapter_info.backend).as_bytes());
    hasher.update(b"\0");
    hasher.update(&adapter_info.vendor.to_le_bytes());
    hasher.update(b"\0");
    hasher.update(&adapter_info.device.to_le_bytes());
    hasher.update(b"\0");
    hasher.update(adapter_info.driver.as_bytes());
    hasher.update(b"\0");
    hasher.update(adapter_info.driver_info.as_bytes());
}

fn blake3_hex(bytes: &[u8]) -> String {
    hex_hash(blake3::hash(bytes).as_bytes())
}
fn metadata_fingerprint(value: &str) -> [u8; 32] {
    *blake3::hash(value.as_bytes()).as_bytes()
}

fn path_fingerprint(path: &Path) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"vyre-pipeline-cache-path-v1\0");
    hasher.update(path.as_os_str().as_encoded_bytes());
    let hex = hex_hash(hasher.finalize().as_bytes());
    format!("cache-path:{}", &hex[..16])
}

fn hex_hash(bytes: &[u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut hex = [0_u8; 64];
    for (index, byte) in bytes.iter().enumerate() {
        let offset = index * 2;
        hex[offset] = HEX[hex_nibble_index(byte >> 4)];
        hex[offset + 1] = HEX[hex_nibble_index(byte & 0x0f)];
    }
    String::from_utf8_lossy(&hex).into_owned()
}

fn push_hex_u32(out: &mut String, value: u32) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in value.to_be_bytes() {
        out.push(HEX[hex_nibble_index(byte >> 4)] as char);
        out.push(HEX[hex_nibble_index(byte & 0x0f)] as char);
    }
}

fn hex_nibble_index(nibble: u8) -> usize {
    debug_assert!(
        nibble < 16,
        "pipeline disk-cache hex encoding received a non-nibble byte"
    );
    usize::from(nibble)
}

#[cfg(test)]
mod tests {
    #![allow(missing_docs)]
    include!("disk_cache_tests.rs");

    #[test]
    fn disk_cache_production_uses_fallible_cache_staging() {
        let src = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/pipeline/disk_cache.rs"
        ))
        .expect("Fix: disk cache source must be readable");
        let production = src
            .split("\n#[cfg(test)]\nmod tests")
            .next()
            .expect("Fix: meta-test scans production sources; update fixture path if module moved - production section must exist");
        assert!(
            !production.contains("Vec::with_capacity("),
            "disk cache Vec staging must reserve fallibly"
        );
        assert!(
            !production.contains("SmallVec::with_capacity("),
            "disk cache dynamic staging must reserve fallibly"
        );
        assert!(
            !production.contains("String::with_capacity("),
            "disk cache string staging must reserve fallibly"
        );
        assert!(production.contains("reserve_backend_vec"));
        assert!(production.contains("try_reserve_exact"));
    }

    #[test]
    fn fixed_digest_hex_hash_is_lowercase_and_stack_encoded() {
        let mut digest = [0_u8; 32];
        digest[0] = 0xab;
        digest[31] = 0x7f;

        let hex = hex_hash(&digest);

        assert_eq!(hex.len(), 64);
        assert!(hex.starts_with("ab00"));
        assert!(hex.ends_with("007f"));
        assert!(hex.bytes().all(|byte| byte.is_ascii_hexdigit()));
        assert_eq!(hex, hex.to_ascii_lowercase());
    }
}
