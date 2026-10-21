use std::path::PathBuf;

use super::disk_common::cache_key_stem;
#[cfg(test)]
use super::disk_common::source_hash128;

// ---------------------------------------------------------------
// Disk-backed cache for Stage 3 directive payloads (T030 second
// half). Same on-disk format pattern as classified_cache: magic
// header + length-prefixed binary, key verify-on-load, atomic
// publish. Production Stage 3 is macro-independent: it parses directive
// payload spans, while live conditional truth is evaluated during the
// production walk. The key retains a macro-fingerprint field for the
// compatibility extraction API; production callers pass the empty macro set.
// ---------------------------------------------------------------

pub(crate) const PAYLOADS_DISK_MAGIC: &[u8; 8] = b"VYREPL02";

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub(crate) struct PayloadsCacheKey {
    pub(crate) path: PathBuf,
    pub(crate) source_len: usize,
    pub(crate) source_hash: [u8; 16],
    pub(crate) macro_fingerprint: [u8; 16],
}

pub(crate) fn macro_fingerprint(defined_macros: &[&[u8]]) -> [u8; 16] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&(defined_macros.len() as u64).to_le_bytes());
    for name in defined_macros {
        hasher.update(&(name.len() as u64).to_le_bytes());
        hasher.update(name);
    }
    let digest = hasher.finalize();
    let mut out = [0u8; 16];
    out.copy_from_slice(&digest.as_bytes()[..16]);
    out
}

#[cfg(test)]
pub(crate) fn payloads_cache_key(
    file_path: &std::path::Path,
    source: &[u8],
    defined_macros: &[&[u8]],
) -> PayloadsCacheKey {
    payloads_cache_key_from_hash(
        file_path,
        source.len(),
        source_hash128(source),
        defined_macros,
    )
}

pub(crate) fn payloads_cache_key_from_hash(
    file_path: &std::path::Path,
    source_len: usize,
    source_hash: [u8; 16],
    defined_macros: &[&[u8]],
) -> PayloadsCacheKey {
    PayloadsCacheKey {
        path: file_path.to_path_buf(),
        source_len,
        source_hash,
        macro_fingerprint: macro_fingerprint(defined_macros),
    }
}

#[cfg(test)]
pub(crate) fn production_payloads_cache_key(
    file_path: &std::path::Path,
    source: &[u8],
) -> PayloadsCacheKey {
    payloads_cache_key(file_path, source, &[])
}

pub(crate) fn production_payloads_cache_key_from_hash(
    file_path: &std::path::Path,
    source_len: usize,
    source_hash: [u8; 16],
) -> PayloadsCacheKey {
    payloads_cache_key_from_hash(file_path, source_len, source_hash, &[])
}

pub(crate) fn payloads_disk_path(dir: &std::path::Path, key: &PayloadsCacheKey) -> PathBuf {
    dir.join(format!(
        "{}.vpl",
        cache_key_stem(
            key.path.as_os_str().as_encoded_bytes(),
            key.source_len,
            key.source_hash,
            Some(key.macro_fingerprint),
        )
    ))
}
