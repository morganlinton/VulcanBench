use std::io::Read as _;
use std::path::PathBuf;

use super::classified_memory::{ClassifiedCacheKey, PREPROCESS_CACHE_SEMANTIC_VERSION};

// ---------------------------------------------------------------
// Disk-backed persistence for classified_cache (T030 first half).
//
// Default location is `${XDG_CACHE_HOME:-$HOME/.cache}/vyre/parsed-ast/`. Each entry is a
// length-prefixed binary file named by the (path, source_len, source_
// hash) cache key  -  a cache miss in process memory consults disk
// before paying for the GPU lex+classify dispatches; a successful
// disk hit also warms the in-memory cache so subsequent hits in the
// same process pay no I/O.
// ---------------------------------------------------------------

/// Magic header so a stale or unrelated file in the cache directory
/// can be rejected immediately rather than mis-decoded.
pub(crate) const CLASSIFIED_DISK_MAGIC: &[u8; 8] = b"VYRECTS1";

pub(crate) static DISK_CACHE_TMP_COUNTER: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

const MAX_PREPROCESS_DISK_CACHE_ENTRY_BYTES: u64 = 512 * 1024 * 1024;

pub(crate) fn parsed_ast_cache_dir() -> PathBuf {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))
        .unwrap_or_else(|| std::env::temp_dir().join("vyre-cache"));
    let dir = base.join("vyre").join("parsed-ast");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

pub(crate) fn disk_cache_tmp_path(path: &std::path::Path, extension: &str) -> PathBuf {
    let seq = DISK_CACHE_TMP_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    path.with_extension(format!("{extension}.{}.{}.tmp", std::process::id(), seq))
}

pub(crate) fn remove_disk_cache_file(path: &std::path::Path, context: &str) {
    match std::fs::remove_file(path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => {}
    }
}

pub(crate) fn read_disk_cache_file_bounded(
    path: &std::path::Path,
    context: &str,
) -> Result<Option<Vec<u8>>, String> {
    read_disk_cache_file_bounded_with_limit(path, context, MAX_PREPROCESS_DISK_CACHE_ENTRY_BYTES)
}

pub(crate) fn read_disk_cache_file_bounded_with_limit(
    path: &std::path::Path,
    context: &str,
    max_bytes: u64,
) -> Result<Option<Vec<u8>>, String> {
    let metadata = match std::fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!(
                "vyre C GPU preprocessor disk cache could not inspect {context} entry {}: {error}. Fix: repair cache directory permissions or delete the cache root.",
                path.display()
            ));
        }
    };
    if metadata.len() > max_bytes {
        remove_disk_cache_file(path, context);
        return Ok(None);
    }
    let capacity = usize::try_from(metadata.len()).map_err(|_| {
        format!(
            "vyre C GPU preprocessor disk cache {context} entry {} is {} bytes and exceeds host addressable memory. Fix: delete the cache root or lower cache entry size.",
            path.display(),
            metadata.len()
        )
    })?;
    let mut file = std::fs::File::open(path).map_err(|error| {
        format!(
            "vyre C GPU preprocessor disk cache could not read {context} entry {}: {error}. Fix: repair cache directory permissions or delete the cache root.",
            path.display()
        )
    })?;
    let mut bytes = Vec::with_capacity(capacity);
    file.by_ref()
        .take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| {
            format!(
                "vyre C GPU preprocessor disk cache could not read {context} entry {}: {error}. Fix: repair cache directory permissions or delete the cache root.",
                path.display()
            )
        })?;
    if bytes.len() as u64 > max_bytes {
        remove_disk_cache_file(path, context);
        return Ok(None);
    }
    Ok(Some(bytes))
}

pub(crate) fn publish_disk_cache_file(
    tmp: &std::path::Path,
    path: &std::path::Path,
    context: &str,
) {
    if let Err(rename_error) = std::fs::rename(tmp, path) {
        match std::fs::remove_file(tmp) {
            Ok(()) => {}
            Err(cleanup_error) if cleanup_error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => {}
        }
    }
}

pub(crate) fn source_hash128(source: &[u8]) -> [u8; 16] {
    let digest = blake3::hash(source);
    let mut out = [0u8; 16];
    out.copy_from_slice(&digest.as_bytes()[..16]);
    out
}

pub(crate) fn cache_key_stem(
    path_bytes: &[u8],
    source_len: usize,
    source_hash: [u8; 16],
    macro_fingerprint: Option<[u8; 16]>,
) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&(PREPROCESS_CACHE_SEMANTIC_VERSION.len() as u64).to_le_bytes());
    hasher.update(PREPROCESS_CACHE_SEMANTIC_VERSION);
    hasher.update(&(path_bytes.len() as u64).to_le_bytes());
    hasher.update(path_bytes);
    hasher.update(&(source_len as u64).to_le_bytes());
    hasher.update(&source_hash);
    if let Some(macro_fingerprint) = macro_fingerprint {
        hasher.update(&macro_fingerprint);
    }
    let digest = hasher.finalize();
    let mut stem = String::with_capacity(32);
    for byte in &digest.as_bytes()[..16] {
        use std::fmt::Write as _;

        let _ = write!(&mut stem, "{byte:02x}");
    }
    stem
}

pub(crate) fn classified_disk_path(dir: &std::path::Path, key: &ClassifiedCacheKey) -> PathBuf {
    dir.join(format!(
        "{}.vct",
        cache_key_stem(
            key.path.as_os_str().as_encoded_bytes(),
            key.source_len,
            key.source_hash,
            None,
        )
    ))
}
