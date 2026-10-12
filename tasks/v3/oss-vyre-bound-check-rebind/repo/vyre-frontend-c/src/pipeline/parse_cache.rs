//! Process-local + on-disk caches for the C parser pipeline.
//!
//! Two layers. Lexer output is keyed by `(backend_id, source_text)`;
//! full semantic summaries are keyed by `(backend_id, source_text, semantic_options)`.
//!
//! 1. **Lexer output cache**  -  `(types, starts, lens, counts, n_tokens, …)`
//!    after the resident GPU `c11_lexer` dispatch. The dense single-thread
//!    GPU lexer scans 200 KB on one thread (~550 ms), so every repeat of the
//!    same source skips the dispatch entirely without introducing a CPU parse
//!    path.
//! 2. **Full-summary cache (memory + disk)**  -
//!    [`crate::api::CParseSummary`] keyed by source hash plus semantic
//!    option discriminators. The on-disk half persists across process restarts: a re-parse of any
//!    previously-seen translation unit returns the cached summary
//!    without invoking lex / parse / sema at all.
//!
//! Disk root resolves in this order: `VYRE_FRONTEND_C_SUMMARY_CACHE_DIR`,
//! `$XDG_CACHE_HOME/vyre/frontend-c-summary`, then
//! `$HOME/.cache/vyre/frontend-c-summary`. Missing cache roots are surfaced
//! loudly because silently falling back to temporary storage destroys
//! cross-process parse-cache performance.
//!
//! Source-bearing cache keys use BLAKE3-128. Do not reduce them to `u64`:
//! these caches are keyed by attacker-controlled translation-unit bytes, and a
//! collision returns wrong parser/sema evidence without running the GPU path.

use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use super::parse_memory_cache::{LexerOutputCache, SummaryCache};
use crate::api::CParseSummary;
use crate::hash::{blake3_128_from_hasher, blake3_128_update_len_prefixed, StableHash128};

static SUMMARY_CACHE_TMP_COUNTER: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

/// Cached output of the lex stage for one (backend, source) pair.
#[derive(Clone)]
pub(crate) struct CachedLexerOutputs {
    pub types: Arc<[u8]>,
    pub starts: Arc<[u8]>,
    pub lens: Arc<[u8]>,
    pub counts: Arc<[u8]>,
    pub n_tokens: u32,
    pub keyword_promoted: bool,
    pub cuda_keyword_haystack: Option<(Arc<[u8]>, u32)>,
}

pub(crate) type CacheKey = StableHash128;

pub(crate) fn lexer_output_cache() -> &'static Mutex<LexerOutputCache> {
    static CACHE: OnceLock<Mutex<LexerOutputCache>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(LexerOutputCache::new()))
}

pub(crate) fn summary_cache() -> &'static Mutex<SummaryCache> {
    static CACHE: OnceLock<Mutex<SummaryCache>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(SummaryCache::new()))
}

pub(crate) fn insert_lexer_output_cache(
    cache: &mut LexerOutputCache,
    key: CacheKey,
    entry: CachedLexerOutputs,
) {
    cache.insert(key, entry);
}

pub(crate) fn insert_summary_cache(
    cache: &mut SummaryCache,
    key: CacheKey,
    summary: CParseSummary,
) {
    cache.insert(key, summary);
}

/// Stable hash of `(backend_id, source_text)` for lexer-cache layers.
pub(crate) fn cache_key(backend_id: &str, source: &str) -> CacheKey {
    let mut hash = blake3::Hasher::new();
    blake3_128_update_len_prefixed(&mut hash, backend_id.as_bytes());
    blake3_128_update_len_prefixed(&mut hash, source.as_bytes());
    blake3_128_from_hasher(&hash)
}

/// Stable hash of `(backend_id, source_text, semantic_options)` for full summaries.
pub(crate) fn semantic_summary_cache_key(
    backend_id: &str,
    source: &str,
    target_options_tag: u64,
) -> CacheKey {
    let mut hash = blake3::Hasher::new();
    blake3_128_update_len_prefixed(&mut hash, backend_id.as_bytes());
    blake3_128_update_len_prefixed(&mut hash, source.as_bytes());
    blake3_128_update_len_prefixed(&mut hash, &target_options_tag.to_le_bytes());
    blake3_128_from_hasher(&hash)
}

fn summary_disk_cache_root() -> Result<PathBuf, String> {
    if let Some(p) = std::env::var_os("VYRE_FRONTEND_C_SUMMARY_CACHE_DIR") {
        let path = PathBuf::from(p);
        if path.as_os_str().is_empty() {
            return Err(
                "VYRE_FRONTEND_C_SUMMARY_CACHE_DIR is empty. Fix: set it to a writable directory or unset it so XDG/HOME cache discovery can run."
                    .to_string(),
            );
        }
        return Ok(path);
    }
    if let Some(xdg) = std::env::var_os("XDG_CACHE_HOME") {
        return Ok(PathBuf::from(xdg).join("vyre").join("frontend-c-summary"));
    }
    if let Some(home) = std::env::var_os("HOME") {
        return Ok(PathBuf::from(home)
            .join(".cache")
            .join("vyre")
            .join("frontend-c-summary"));
    }
    Err(
        "vyre-frontend-c summary cache has no VYRE_FRONTEND_C_SUMMARY_CACHE_DIR, XDG_CACHE_HOME, or HOME. Fix: configure a writable persistent cache root; temporary fallback is forbidden for production parser performance."
            .to_string(),
    )
}

fn summary_disk_cache_path(key: CacheKey) -> Result<PathBuf, String> {
    let stem = cache_key_hex(key);
    Ok(summary_disk_cache_root()?
        .join(&stem[..2])
        .join(format!("{stem}.bin")))
}

fn cache_key_hex(key: CacheKey) -> String {
    let mut stem = String::with_capacity(32);
    for byte in key {
        use std::fmt::Write as _;

        let _ = write!(&mut stem, "{byte:02x}");
    }
    stem
}

/// Wire format version for the summary disk cache. Bump when
/// `CParseSummary` field layout changes so stale entries are ignored
/// instead of decoded as garbage.
const SUMMARY_WIRE_VERSION: u32 = 1;
const SUMMARY_MAGIC: &[u8; 8] = b"VYREFCPS";
/// Magic + version + 13 fields × 8 bytes (each field encoded as u64 LE).
const SUMMARY_WIRE_BYTES: usize = 8 + 4 + 13 * 8;

fn encode_summary(summary: &CParseSummary) -> [u8; SUMMARY_WIRE_BYTES] {
    let mut out = [0u8; SUMMARY_WIRE_BYTES];
    out[..8].copy_from_slice(SUMMARY_MAGIC);
    out[8..12].copy_from_slice(&SUMMARY_WIRE_VERSION.to_le_bytes());
    let fields: [u64; 13] = [
        summary.source_bytes,
        summary.token_count as u64,
        summary.ast_bytes,
        summary.ast_node_count as u64,
        summary.vast_bytes,
        summary.abi_layout_bytes,
        summary.expression_shape_bytes,
        summary.program_graph_bytes,
        summary.semantic_node_bytes,
        summary.semantic_edge_bytes,
        summary.sema_scope_bytes,
        summary.function_record_bytes,
        summary.call_record_bytes,
    ];
    let mut offset = 12;
    for word in fields {
        out[offset..offset + 8].copy_from_slice(&word.to_le_bytes());
        offset += 8;
    }
    out
}

fn decode_summary(bytes: &[u8]) -> Option<CParseSummary> {
    if bytes.len() != SUMMARY_WIRE_BYTES {
        return None;
    }
    if &bytes[..8] != SUMMARY_MAGIC {
        return None;
    }
    let version = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
    if version != SUMMARY_WIRE_VERSION {
        return None;
    }
    let mut words = [0u64; 13];
    for (i, word) in words.iter_mut().enumerate() {
        let off = 12 + i * 8;
        *word = u64::from_le_bytes([
            bytes[off],
            bytes[off + 1],
            bytes[off + 2],
            bytes[off + 3],
            bytes[off + 4],
            bytes[off + 5],
            bytes[off + 6],
            bytes[off + 7],
        ]);
    }
    Some(CParseSummary {
        source_bytes: words[0],
        token_count: u32::try_from(words[1]).ok()?,
        ast_bytes: words[2],
        ast_node_count: u32::try_from(words[3]).ok()?,
        vast_bytes: words[4],
        abi_layout_bytes: words[5],
        expression_shape_bytes: words[6],
        program_graph_bytes: words[7],
        semantic_node_bytes: words[8],
        semantic_edge_bytes: words[9],
        sema_scope_bytes: words[10],
        function_record_bytes: words[11],
        call_record_bytes: words[12],
    })
}

pub(crate) fn load_summary_from_disk(key: CacheKey) -> Result<Option<CParseSummary>, String> {
    let path = summary_disk_cache_path(key)?;
    let Some(bytes) = read_summary_cache_entry_bounded(&path)? else {
        return Ok(None);
    };
    match decode_summary(&bytes) {
        Some(summary) => Ok(Some(summary)),
        None => {
            remove_summary_cache_entry(&path)?;
            Ok(None)
        }
    }
}

fn read_summary_cache_entry_bounded(path: &Path) -> Result<Option<Vec<u8>>, String> {
    let metadata = match std::fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!(
                "vyre-frontend-c summary cache metadata read failed at {}: {error}. Fix: repair cache directory permissions.",
                path.display()
            ));
        }
    };
    if metadata.len() > SUMMARY_WIRE_BYTES as u64 {
        remove_summary_cache_entry(path)?;
        return Ok(None);
    }
    let capacity = usize::try_from(metadata.len()).map_err(|_| {
        format!(
            "vyre-frontend-c summary cache entry {} is {} bytes and exceeds host addressable memory. Fix: delete the cache root.",
            path.display(),
            metadata.len()
        )
    })?;
    let mut file = std::fs::File::open(path).map_err(|error| {
        format!(
            "vyre-frontend-c summary cache open failed at {}: {error}. Fix: repair cache directory permissions.",
            path.display()
        )
    })?;
    let mut bytes = Vec::with_capacity(capacity);
    file.by_ref()
        .take(SUMMARY_WIRE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| {
            format!(
                "vyre-frontend-c summary cache read failed at {}: {error}. Fix: repair cache directory permissions.",
                path.display()
            )
        })?;
    if bytes.len() > SUMMARY_WIRE_BYTES {
        remove_summary_cache_entry(path)?;
        return Ok(None);
    }
    Ok(Some(bytes))
}

fn remove_summary_cache_entry(path: &Path) -> Result<(), String> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "vyre-frontend-c summary cache entry {} is invalid and could not be removed: {error}. Fix: repair cache directory permissions or delete the cache root.",
            path.display()
        )),
    }
}

pub(crate) fn store_summary_to_disk(key: CacheKey, summary: &CParseSummary) -> Result<(), String> {
    let path = summary_disk_cache_path(key)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
            format!(
                "vyre-frontend-c summary cache directory creation failed at {}: {error}. Fix: repair cache directory permissions.",
                parent.display()
            )
        })?;
    }
    let bytes = encode_summary(summary);
    let seq = SUMMARY_CACHE_TMP_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tmp = path.with_extension(format!("bin.{}.{}.tmp", std::process::id(), seq));
    std::fs::write(&tmp, bytes).map_err(|error| {
        format!(
            "vyre-frontend-c summary cache write failed at {}: {error}. Fix: repair cache directory permissions.",
            tmp.display()
        )
    })?;
    std::fs::rename(&tmp, &path).map_err(|error| {
        let cleanup = match std::fs::remove_file(&tmp) {
            Ok(()) => String::new(),
            Err(cleanup_error) if cleanup_error.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(cleanup_error) => format!(
                " Temp cleanup also failed for `{}`: {cleanup_error}.",
                tmp.display()
            ),
        };
        format!(
            "vyre-frontend-c summary cache commit failed from {} to {}: {error}.{cleanup} Fix: repair cache directory permissions.",
            tmp.display(),
            path.display()
        )
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_summary() -> CParseSummary {
        CParseSummary {
            source_bytes: 1234,
            token_count: 5,
            ast_bytes: 678,
            ast_node_count: 9,
            vast_bytes: 10,
            abi_layout_bytes: 11,
            expression_shape_bytes: 12,
            program_graph_bytes: 13,
            semantic_node_bytes: 14,
            semantic_edge_bytes: 15,
            sema_scope_bytes: 16,
            function_record_bytes: 17,
            call_record_bytes: 18,
        }
    }

    #[test]
    fn summary_round_trip_preserves_every_field() {
        let original = fixture_summary();
        let bytes = encode_summary(&original);
        let decoded = decode_summary(&bytes).expect("Fix: decode round-trip should succeed");
        assert_eq!(original, decoded);
    }

    #[test]
    fn decode_rejects_bad_magic() {
        let mut bytes = encode_summary(&fixture_summary());
        bytes[0] = b'X';
        assert!(decode_summary(&bytes).is_none());
    }

    #[test]
    fn decode_rejects_bad_version() {
        let mut bytes = encode_summary(&fixture_summary());
        bytes[8] = 99;
        assert!(decode_summary(&bytes).is_none());
    }

    #[test]
    fn decode_rejects_wrong_byte_length() {
        let bytes = encode_summary(&fixture_summary());
        assert!(decode_summary(&bytes[..bytes.len() - 1]).is_none());
    }

    #[test]
    fn cache_key_changes_with_source_or_backend() {
        let a = cache_key("cuda", "int main(){}");
        let b = cache_key("cuda", "int other(){}");
        let c = cache_key("wgpu", "int main(){}");
        assert_eq!(a.len(), 16);
        assert_ne!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn semantic_summary_cache_key_changes_with_target_tag() {
        let a = semantic_summary_cache_key("cuda", "int main(){}", 1);
        let b = semantic_summary_cache_key("cuda", "int main(){}", 2);
        assert_ne!(
            a, b,
            "semantic summary cache keys must include target/predefine options so warm parse_source caches cannot cross ABI or predefine boundaries"
        );
    }

    #[test]
    fn cache_key_hex_is_128_bit_hex() {
        assert_eq!(
            cache_key_hex([0xabu8; 16]),
            "abababababababababababababababab"
        );
    }

    fn temp_summary_path(name: &str) -> std::path::PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "vyre_frontend_c_summary_cache_{}_{}_{:?}",
            name,
            std::process::id(),
            std::thread::current().id()
        ));
        path
    }

    #[test]
    fn summary_cache_reader_accepts_exact_wire_size() {
        let path = temp_summary_path("exact");
        let bytes = encode_summary(&fixture_summary());
        std::fs::write(&path, bytes).expect("Fix: temp summary fixture must be writable");

        let loaded = read_summary_cache_entry_bounded(&path)
            .expect("Fix: exact summary cache entry must be readable")
            .expect("Fix: exact summary cache entry must be present");

        assert_eq!(loaded.len(), SUMMARY_WIRE_BYTES);
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn summary_cache_reader_removes_oversized_entry() {
        let path = temp_summary_path("oversized");
        std::fs::write(&path, vec![0x31u8; SUMMARY_WIRE_BYTES + 1])
            .expect("Fix: temp summary fixture must be writable");

        let loaded = read_summary_cache_entry_bounded(&path)
            .expect("Fix: oversized summary cache entry should be removable");

        assert!(loaded.is_none());
        assert!(!path.exists());
    }
}
