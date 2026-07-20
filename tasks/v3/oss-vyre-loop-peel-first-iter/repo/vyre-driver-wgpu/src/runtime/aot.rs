//! AOT WGSL specialization cache for pipeline-mode dispatch.
//!
//! The cache persists lowered WGSL under `~/.cache/vyre/aot/`, keyed by the
//! canonical IR wire hash plus an observed backend fingerprint. This lets a
//! second process skip IR lowering and driver-specific specialization checks.

use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use vyre_driver::{BackendError, DispatchConfig};
use vyre_foundation::ir::Program;

const CACHE_VERSION: u32 = 1;
const MAX_AOT_WGSL_BYTES: u64 = 16 * 1024 * 1024;
const MAX_AOT_METADATA_BYTES: u64 = 64 * 1024;

/// Result of reading or populating the AOT specialization cache.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AotArtifact {
    /// Lowered WGSL shader source.
    pub wgsl: String,
    /// Cache key derived from program wire bytes and backend fingerprint.
    pub key: String,
    /// True when the artifact came from disk.
    pub cache_hit: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct AotMetadata {
    version: u32,
    spec_hash: String,
    backend_fingerprint: String,
    created_unix_ms: u64,
    wgsl_bytes: usize,
}

/// Return the backend fingerprint used by the wgpu AOT cache.
#[must_use]
pub fn backend_fingerprint() -> String {
    env::var("VYRE_BACKEND_FINGERPRINT_OVERRIDE").unwrap_or_else(|_| {
        let backend = env::var("WGPU_BACKEND").unwrap_or_else(|_| "auto".to_string());
        let adapter = env::var("VYRE_WGPU_ADAPTER").unwrap_or_else(|_| "cached-device".to_string());
        format!("wgpu:{backend}:{adapter}:{}", env!("CARGO_PKG_VERSION"))
    })
}

/// Load WGSL from the on-disk AOT cache, or lower and persist it.
///
/// # Errors
///
/// Returns a backend error when program serialization, lowering, or durable
/// cache writes fail.
pub fn load_or_compile(program: &Program, fingerprint: &str) -> Result<AotArtifact, BackendError> {
    load_or_compile_with_config(program, fingerprint, &DispatchConfig::default())
}

/// Load WGSL from the on-disk AOT cache, or lower and persist it with policy.
///
/// The cache key includes policy that affects shader text, such as P-20
/// approximate transcendental ULP budgets.
///
/// # Errors
///
/// Returns a backend error when program serialization, lowering, or durable
/// cache writes fail.
pub fn load_or_compile_with_config(
    program: &Program,
    fingerprint: &str,
    config: &DispatchConfig,
) -> Result<AotArtifact, BackendError> {
    let wire = program.to_wire().map_err(|error| {
        BackendError::new(format!(
            "failed to serialize Program for AOT cache key: {error}. Fix: validate the Program before pipeline compilation."
        ))
    })?;
    let spec_hash = blake3::hash(&wire).to_hex().to_string();
    let policy = format!("ulp={:?}", config.ulp_budget);
    let key = cache_key(&format!("{spec_hash}:{policy}"), fingerprint);
    let dir = cache_dir();
    let wgsl_path = dir.join(format!("{key}.wgsl"));
    let meta_path = dir.join(format!("{key}.toml"));

    if let Ok(wgsl) = read_aot_text_bounded(&wgsl_path, MAX_AOT_WGSL_BYTES) {
        if metadata_matches(&meta_path, &spec_hash, fingerprint, wgsl.len()) {
            return Ok(AotArtifact {
                wgsl,
                key,
                cache_hit: true,
            });
        }
    }

    let wgsl = crate::emit::lower_with_config(program, config).map_err(|error| {
        BackendError::new(format!(
            "failed to lower vyre IR to WGSL: {error}. Fix: provide a valid Program accepted by the WGSL lowering pipeline."
        ))
    })?;
    fs::create_dir_all(&dir).map_err(|error| {
        BackendError::new(format!(
            "failed to create AOT cache dir `{}`: {error}. Fix: ensure the cache directory is writable.",
            dir.display()
        ))
    })?;
    let metadata = AotMetadata {
        version: CACHE_VERSION,
        spec_hash,
        backend_fingerprint: fingerprint.to_string(),
        created_unix_ms: now_unix_ms(),
        wgsl_bytes: wgsl.len(),
    };
    let toml = toml::to_string(&metadata).map_err(|error| {
        BackendError::new(format!(
            "failed to encode AOT metadata: {error}. Fix: report this vyre-wgpu cache bug."
        ))
    })?;
    fs::write(&wgsl_path, &wgsl).map_err(|error| {
        BackendError::new(format!(
            "failed to write AOT WGSL cache `{}`: {error}. Fix: ensure the cache directory is writable.",
            wgsl_path.display()
        ))
    })?;
    fs::write(&meta_path, toml).map_err(|error| {
        BackendError::new(format!(
            "failed to write AOT metadata `{}`: {error}. Fix: ensure the cache directory is writable.",
            meta_path.display()
        ))
    })?;

    Ok(AotArtifact {
        wgsl,
        key,
        cache_hit: false,
    })
}

/// Deterministic cache key for one specialization.
#[must_use]
pub fn cache_key(spec_hash: &str, backend_fingerprint: &str) -> String {
    vyre_driver::specialization::versioned_specialization_artifact_key(
        CACHE_VERSION,
        spec_hash,
        backend_fingerprint,
    )
}

/// Directory used by the AOT cache.
#[must_use]
pub fn cache_dir() -> PathBuf {
    if let Ok(dir) = env::var("VYRE_AOT_CACHE_DIR") {
        return PathBuf::from(dir);
    }
    let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".cache").join("vyre").join("aot")
}

fn metadata_matches(
    path: &std::path::Path,
    spec_hash: &str,
    backend_fingerprint: &str,
    wgsl_bytes: usize,
) -> bool {
    let Ok(raw) = read_aot_text_bounded(path, MAX_AOT_METADATA_BYTES) else {
        return false;
    };
    let Ok(metadata) = toml::from_str::<AotMetadata>(&raw) else {
        return false;
    };
    metadata.version == CACHE_VERSION
        && metadata.spec_hash == spec_hash
        && metadata.backend_fingerprint == backend_fingerprint
        && metadata.wgsl_bytes == wgsl_bytes
}

fn read_aot_text_bounded(path: &std::path::Path, max_bytes: u64) -> std::io::Result<String> {
    use std::io::Read as _;

    let mut file = fs::File::open(path)?;
    let metadata = file.metadata()?;
    if metadata.len() > max_bytes {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("AOT cache file exceeds {max_bytes} byte limit"),
        ));
    }
    let capacity = usize::try_from(metadata.len()).map_err(|source| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("AOT cache file length cannot fit usize: {source}"),
        )
    })?;
    let bounded_read_limit = max_bytes.checked_add(1).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "AOT cache max_bytes cannot add sentinel byte without overflowing u64",
        )
    })?;
    let mut text = String::with_capacity(capacity);
    file.by_ref()
        .take(bounded_read_limit)
        .read_to_string(&mut text)?;
    if u64::try_from(text.len()).map_or(true, |len| len > max_bytes) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "AOT cache file exceeded bounded read limit",
        ));
    }
    Ok(text)
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            u64::try_from(duration.as_millis().min(u128::from(u64::MAX))).unwrap_or(u64::MAX)
        })
}
