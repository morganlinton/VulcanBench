//! Integration test crate for the containing Vyre package.

/// G8: content-hash on-disk pipeline cache.
///
/// Keyed by `blake3(program.to_wire() || driver_version || device_gen
/// || CURRENT_PIPELINE_CACHE_KEY_VERSION || feature_flags)`. A hit
/// lets a backend skip target compilation and load the bytes
/// straight into a pipeline handle  -  single-digit ms cold start
/// after the first run.
///
/// This module owns the **pure** key derivation + blob I/O. The
/// backend supplies its native blob bytes and calls [`store`] after a successful compile;
/// subsequent runs call [`load`] before compiling. The key
/// versioning means a `CURRENT_PIPELINE_CACHE_KEY_VERSION` bump
/// invalidates every existing file on disk, the same way it
use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::{PipelineFeatureFlags, CURRENT_PIPELINE_CACHE_KEY_VERSION};
use blake3::Hasher;

/// Cache-file extension. Binary blob.
pub(super) const CACHE_EXTENSION: &str = "bin";

/// Compute the 32-byte blake3 cache key for `program` on the
/// named backend.
///
/// `driver_version` is the backend's own build identifier;
/// `device_gen` is a caller-chosen generation bucket for the
/// target device family. Mixing them makes a pipeline compiled
/// for one generation miss on another, even though the Program
/// bytes match.
#[must_use]
pub(super) fn compute_cache_key(
    program_wire: &[u8],
    backend_id: &str,
    driver_version: &str,
    device_gen: &str,
    feature_flags: PipelineFeatureFlags,
) -> [u8; 32] {
    let mut hasher = Hasher::new();
    hasher.update(&CURRENT_PIPELINE_CACHE_KEY_VERSION.to_le_bytes());
    hasher.update(&(backend_id.len() as u32).to_le_bytes());
    hasher.update(backend_id.as_bytes());
    hasher.update(&(driver_version.len() as u32).to_le_bytes());
    hasher.update(driver_version.as_bytes());
    hasher.update(&(device_gen.len() as u32).to_le_bytes());
    hasher.update(device_gen.as_bytes());
    hasher.update(&feature_flags.0.to_le_bytes());
    hasher.update(&(program_wire.len() as u64).to_le_bytes());
    hasher.update(program_wire);
    let mut out = [0_u8; 32];
    out.copy_from_slice(hasher.finalize().as_bytes());
    out
}

/// Filename inside `cache_dir` for `key`  -  lowercase hex +
/// `.bin` extension. Deterministic; no salt.
#[must_use]
pub(super) fn cache_path(cache_dir: &Path, key: &[u8; 32]) -> PathBuf {
    // Writes to a String never fail; ignore the Result per the
    // stdlib convention for `fmt::Write` on owned buffers.
    let mut name = String::with_capacity(64 + 1 + CACHE_EXTENSION.len());
    for b in key {
        let _ = write!(&mut name, "{b:02x}");
    }
    name.push('.');
    name.push_str(CACHE_EXTENSION);
    cache_dir.join(name)
}

/// Load a cached blob by key. Returns `Ok(None)` on a miss
/// (file doesn't exist) and `Err` on I/O errors.
pub(super) fn load(cache_dir: &Path, key: &[u8; 32]) -> Result<Option<Vec<u8>>, CacheError> {
    let path = cache_path(cache_dir, key);
    match fs::read(&path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(CacheError::Io { path, source: e }),
    }
}

/// Write a cached blob for `key`. Creates `cache_dir` if
/// missing. Writes via a temp file + atomic rename so a
/// concurrent reader either sees the old blob or the new one,
/// never a torn write.
pub(super) fn store(cache_dir: &Path, key: &[u8; 32], bytes: &[u8]) -> Result<(), CacheError> {
    fs::create_dir_all(cache_dir).map_err(|e| CacheError::Io {
        path: cache_dir.to_path_buf(),
        source: e,
    })?;
    let final_path = cache_path(cache_dir, key);
    let tmp_path = final_path.with_extension("bin.tmp");
    fs::write(&tmp_path, bytes).map_err(|e| CacheError::Io {
        path: tmp_path.clone(),
        source: e,
    })?;
    fs::rename(&tmp_path, &final_path).map_err(|e| CacheError::Io {
        path: final_path,
        source: e,
    })
}

/// Cache I/O errors.
#[derive(Debug, thiserror::Error)]
pub(super) enum CacheError {
    /// Disk-side I/O failure while reading or writing a cache entry.
    #[error(
        "Fix: pipeline-cache I/O failed at {path:?}. \
         Ensure the cache directory is writable: {source}"
    )]
    Io {
        /// Cache directory or file the operation targeted.
        path: PathBuf,
        /// Underlying `std::io::Error` that triggered the failure.
        #[source]
        source: io::Error,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key1() -> [u8; 32] {
        [1_u8; 32]
    }

    fn key2() -> [u8; 32] {
        [2_u8; 32]
    }

    #[test]
    fn compute_cache_key_is_deterministic() {
        let a = compute_cache_key(
            b"bytes",
            "backend-a",
            "v24",
            "ada",
            PipelineFeatureFlags::SUBGROUP_OPS,
        );
        let b = compute_cache_key(
            b"bytes",
            "backend-a",
            "v24",
            "ada",
            PipelineFeatureFlags::SUBGROUP_OPS,
        );
        assert_eq!(a, b);
    }

    #[test]
    fn compute_cache_key_changes_with_driver_version() {
        let a = compute_cache_key(
            b"x",
            "backend-a",
            "v24",
            "gen-a",
            PipelineFeatureFlags::empty(),
        );
        let b = compute_cache_key(
            b"x",
            "backend-a",
            "v25",
            "gen-a",
            PipelineFeatureFlags::empty(),
        );
        assert_ne!(a, b);
    }

    #[test]
    fn compute_cache_key_changes_with_device_gen() {
        let a = compute_cache_key(
            b"x",
            "backend-a",
            "v24",
            "gen-a",
            PipelineFeatureFlags::empty(),
        );
        let b = compute_cache_key(
            b"x",
            "backend-a",
            "v24",
            "gen-b",
            PipelineFeatureFlags::empty(),
        );
        assert_ne!(a, b);
    }

    #[test]
    fn compute_cache_key_changes_with_feature_flags() {
        let a = compute_cache_key(
            b"x",
            "backend-a",
            "v24",
            "gen-a",
            PipelineFeatureFlags::empty(),
        );
        let b = compute_cache_key(
            b"x",
            "backend-a",
            "v24",
            "gen-a",
            PipelineFeatureFlags::SUBGROUP_OPS,
        );
        assert_ne!(a, b);
    }

    #[test]
    fn compute_cache_key_changes_with_program_bytes() {
        let a = compute_cache_key(
            b"prog-a",
            "backend-a",
            "v24",
            "gen-a",
            PipelineFeatureFlags::empty(),
        );
        let b = compute_cache_key(
            b"prog-b",
            "backend-a",
            "v24",
            "gen-a",
            PipelineFeatureFlags::empty(),
        );
        assert_ne!(a, b);
    }

    #[test]
    fn compute_cache_key_not_vulnerable_to_length_extension() {
        // A naive concatenation of two variable-length fields
        // without separating them would let `("ab", "cd")`
        // collide with `("abc", "d")`. Our format prefixes each
        // field with its length, so these must differ.
        let a = compute_cache_key(b"", "ab", "cd", "gen-a", PipelineFeatureFlags::empty());
        let b = compute_cache_key(b"", "abc", "d", "gen-a", PipelineFeatureFlags::empty());
        assert_ne!(a, b);
    }

    #[test]
    fn cache_path_is_hex_and_bin_extension() {
        let d = Path::new("/tmp");
        let p = cache_path(d, &[0xAB_u8; 32]);
        let fname = p.file_name().unwrap().to_string_lossy().to_string();
        assert!(fname.ends_with(".bin"));
        assert!(fname.contains("abababab"));
        assert_eq!(fname.len(), 64 + 4); // 64 hex + ".bin"
    }

    #[test]
    fn load_miss_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let r = load(dir.path(), &key1()).unwrap();
        assert!(r.is_none());
    }

    #[test]
    fn store_then_load_roundtrips() {
        let dir = tempfile::tempdir().unwrap();
        let payload = b"compiled-target-bytes".to_vec();
        store(dir.path(), &key1(), &payload).unwrap();
        let loaded = load(dir.path(), &key1()).unwrap();
        assert_eq!(loaded.as_deref(), Some(payload.as_slice()));
    }

    #[test]
    fn store_creates_missing_cache_dir() {
        let parent = tempfile::tempdir().unwrap();
        let nested = parent.path().join("a").join("b").join("c");
        assert!(!nested.exists());
        store(&nested, &key1(), b"blob").unwrap();
        let loaded = load(&nested, &key1()).unwrap();
        assert_eq!(loaded.as_deref(), Some(b"blob".as_slice()));
    }

    #[test]
    fn different_keys_do_not_overlap() {
        let dir = tempfile::tempdir().unwrap();
        store(dir.path(), &key1(), b"one").unwrap();
        store(dir.path(), &key2(), b"two").unwrap();
        assert_eq!(
            load(dir.path(), &key1()).unwrap().as_deref(),
            Some(b"one".as_slice())
        );
        assert_eq!(
            load(dir.path(), &key2()).unwrap().as_deref(),
            Some(b"two".as_slice())
        );
    }

    #[test]
    fn overwriting_same_key_preserves_atomicity() {
        let dir = tempfile::tempdir().unwrap();
        store(dir.path(), &key1(), b"first").unwrap();
        store(dir.path(), &key1(), b"second").unwrap();
        assert_eq!(
            load(dir.path(), &key1()).unwrap().as_deref(),
            Some(b"second".as_slice())
        );
    }
}
