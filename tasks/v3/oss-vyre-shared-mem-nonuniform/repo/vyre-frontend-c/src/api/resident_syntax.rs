use super::*;

/// Raw-byte syntax parse evidence emitted by the resident GPU frontend.
pub struct SyntaxParseSummary {
    /// Backend identifier that executed the parser dispatch chain.
    pub backend_id: String,
    /// Original source byte length.
    pub source_bytes: u64,
    /// Logical token count after keyword promotion and span repair.
    pub token_count: u32,
    /// AST evidence bytes produced by parser windows.
    pub ast_bytes: u64,
    /// AST node count produced by parser windows.
    pub ast_node_count: u32,
    /// Number of source tokens covered by AST parser windows.
    pub ast_covered_tokens: u32,
    /// Number of AST parser windows dispatched for this input.
    pub ast_window_count: u32,
}

/// Batched raw-byte syntax parse evidence emitted by the resident GPU frontend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxBatchParseSummary {
    /// Backend identifier that executed the parser dispatch chain.
    pub backend_id: String,
    /// Number of source files packed into the resident batch.
    pub file_count: u32,
    /// Sum of original source byte lengths, excluding inserted separators.
    pub source_bytes: u64,
    /// Resident batch byte length, including inserted separators.
    pub batch_bytes: u64,
    /// Logical token count for the packed batch.
    pub token_count: u32,
    /// AST evidence bytes produced by parser windows.
    pub ast_bytes: u64,
    /// AST node count produced by parser windows.
    pub ast_node_count: u32,
    /// Number of source tokens covered by AST parser windows.
    pub ast_covered_tokens: u32,
    /// Number of AST parser windows dispatched for this batch.
    pub ast_window_count: u32,
}

/// Prepared raw C source and resident haystack bytes for repeated syntax parses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedResidentSyntaxBytes {
    /// Original source byte length.
    pub source_bytes: u64,
    /// Logical resident haystack length in u32 lanes.
    pub haystack_len: u32,
    pub(crate) quote_free: bool,
    pub(crate) haystack: Arc<[u8]>,
}

/// Observable frontend resident-cache counters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PipelineCacheSnapshot {
    /// Number of raw-byte syntax parses served from the resident haystack cache.
    pub haystack_hits: u64,
    /// Number of raw-byte syntax parses that populated the resident haystack cache.
    pub haystack_misses: u64,
    /// Resident syntax cache entries evicted to stay within configured limits.
    pub haystack_evictions: u64,
    /// Oversized syntax entries rejected instead of being cached.
    pub haystack_rejected_oversized: u64,
    /// Current resident syntax cache entry count.
    pub haystack_entries: usize,
    /// Current resident syntax cache retained bytes.
    pub haystack_bytes: usize,
}

static HAYSTACK_HITS: AtomicU64 = AtomicU64::new(0);
static HAYSTACK_MISSES: AtomicU64 = AtomicU64::new(0);
static RESIDENT_SYNTAX_CACHE: OnceLock<Mutex<ResidentSyntaxCache>> = OnceLock::new();
const RESIDENT_SYNTAX_CACHE_MAX_ENTRIES: usize = 256;
const RESIDENT_SYNTAX_CACHE_MAX_BYTES: usize = 512 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct ResidentSyntaxKey {
    pub(super) len: u64,
    pub(super) hash: StableHash128,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct ResidentSyntaxCacheStats {
    pub(super) hits: u64,
    pub(super) misses: u64,
    pub(super) inserts: u64,
    pub(super) evictions: u64,
    pub(super) rejected_oversized: u64,
    pub(super) entries: usize,
    pub(super) bytes: usize,
}

pub(super) struct ResidentSyntaxCache {
    entries: HashMap<ResidentSyntaxKey, ResidentSyntaxCacheEntry>,
    epoch: u64,
    bytes: usize,
    stats: ResidentSyntaxCacheStats,
}

struct ResidentSyntaxCacheEntry {
    prepared: PreparedResidentSyntaxBytes,
    last_access: u64,
}

impl Default for ResidentSyntaxCache {
    fn default() -> Self {
        Self {
            entries: HashMap::new(),
            epoch: 0,
            bytes: 0,
            stats: ResidentSyntaxCacheStats::default(),
        }
    }
}

impl ResidentSyntaxCache {
    pub(super) fn new() -> Self {
        Self::default()
    }

    pub(super) fn len(&self) -> usize {
        self.entries.len()
    }

    pub(super) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub(super) fn contains_key(&self, key: &ResidentSyntaxKey) -> bool {
        self.entries.contains_key(key)
    }

    pub(super) fn stats(&self) -> ResidentSyntaxCacheStats {
        ResidentSyntaxCacheStats {
            entries: self.entries.len(),
            bytes: self.bytes,
            ..self.stats
        }
    }

    fn next_epoch(&mut self) -> u64 {
        self.epoch = self.epoch.saturating_add(1);
        self.epoch
    }
}

/// Run the CUDA-first raw-byte C syntax parser and return parser evidence.
pub fn parse_syntax_bytes(source: &[u8]) -> Result<SyntaxParseSummary, String> {
    let key = resident_syntax_key(source);
    let cache = RESIDENT_SYNTAX_CACHE.get_or_init(|| Mutex::new(ResidentSyntaxCache::new()));
    let prepared = {
        let mut guard = cache
            .lock()
            .map_err(|error| format!("resident syntax cache lock poisoned: {error}"))?;
        if let Some(prepared) = lookup_resident_syntax_cache(&mut guard, &key) {
            HAYSTACK_HITS.fetch_add(1, Ordering::Relaxed);
            prepared
        } else {
            HAYSTACK_MISSES.fetch_add(1, Ordering::Relaxed);
            let prepared = prepare_resident_syntax_bytes(source)?;
            insert_resident_syntax_cache(&mut guard, key, prepared.clone());
            prepared
        }
    };
    parse_prepared_resident_syntax(&prepared)
}

/// Run the CUDA-first raw-byte C syntax parser over many already-loaded files
/// as one resident batch. This amortizes fixed GPU launch/readback overhead
/// across Linux-scale corpuses while keeping parsing on the GPU release path.
pub fn parse_syntax_batch_bytes(sources: &[&[u8]]) -> Result<SyntaxBatchParseSummary, String> {
    if sources.is_empty() {
        return Err("syntax batch is empty. Fix: pass at least one source buffer.".to_string());
    }
    const UNSAFE_SYNTAX_BATCH_MAX_FILES_PER_CHUNK: usize = 4096;
    const UNSAFE_SYNTAX_BATCH_TARGET_BYTES: usize = 256 * 1024;
    let source_bytes = sources.iter().try_fold(0u64, |acc, source| {
        let len = u64::try_from(source.len()).map_err(|_| {
            "syntax batch source length exceeds u64. Fix: shard the corpus.".to_string()
        })?;
        acc.checked_add(len).ok_or_else(|| {
            "syntax batch source byte total overflowed u64. Fix: shard the corpus.".to_string()
        })
    })?;
    let separator_bytes = sources
        .len()
        .checked_sub(1)
        .and_then(|count| u64::try_from(count).ok())
        .ok_or_else(|| {
            "syntax batch separator byte total overflowed u64. Fix: shard the corpus.".to_string()
        })?;
    let batch_len = source_bytes.checked_add(separator_bytes).ok_or_else(|| {
        "syntax batch byte total overflowed u64. Fix: shard the corpus.".to_string()
    })?;
    if sources.len() > 1
        && (sources.len() > UNSAFE_SYNTAX_BATCH_MAX_FILES_PER_CHUNK
            || batch_len > UNSAFE_SYNTAX_BATCH_TARGET_BYTES as u64)
    {
        let mut backend_id = None::<String>;
        let mut batch_bytes = 0u64;
        let mut token_count = 0u32;
        let mut ast_bytes = 0u64;
        let mut ast_node_count = 0u32;
        let mut ast_covered_tokens = 0u32;
        let mut ast_window_count = 0u32;
        let mut chunk_start = 0usize;
        while chunk_start < sources.len() {
            let mut chunk_end = chunk_start;
            let mut chunk_bytes = 0usize;
            while chunk_end < sources.len()
                && chunk_end - chunk_start < UNSAFE_SYNTAX_BATCH_MAX_FILES_PER_CHUNK
            {
                let separator_bytes = usize::from(chunk_end > chunk_start);
                let next_bytes = sources[chunk_end]
                    .len()
                    .checked_add(separator_bytes)
                    .ok_or_else(|| {
                        "syntax batch chunk byte total overflowed usize. Fix: shard the corpus."
                            .to_string()
                    })?;
                let candidate_chunk_bytes =
                    chunk_bytes.checked_add(next_bytes).ok_or_else(|| {
                        "syntax batch chunk byte total overflowed usize. Fix: shard the corpus."
                            .to_string()
                    })?;
                if chunk_end > chunk_start
                    && candidate_chunk_bytes > UNSAFE_SYNTAX_BATCH_TARGET_BYTES
                {
                    break;
                }
                chunk_bytes = candidate_chunk_bytes;
                chunk_end += 1;
            }
            let summary = parse_syntax_batch_bytes(&sources[chunk_start..chunk_end])?;
            backend_id.get_or_insert(summary.backend_id);
            batch_bytes = batch_bytes
                .checked_add(summary.batch_bytes)
                .ok_or_else(|| {
                    "syntax batch resident byte total overflowed u64. Fix: shard the corpus."
                        .to_string()
                })?;
            token_count = token_count
                .checked_add(summary.token_count)
                .ok_or_else(|| {
                    "syntax batch token count overflowed u32. Fix: shard the corpus.".to_string()
                })?;
            ast_bytes = ast_bytes.checked_add(summary.ast_bytes).ok_or_else(|| {
                "syntax batch AST byte total overflowed u64. Fix: shard the corpus.".to_string()
            })?;
            ast_node_count = ast_node_count
                .checked_add(summary.ast_node_count)
                .ok_or_else(|| {
                    "syntax batch AST node count overflowed u32. Fix: shard the corpus.".to_string()
                })?;
            ast_covered_tokens = ast_covered_tokens
                .checked_add(summary.ast_covered_tokens)
                .ok_or_else(|| {
                    "syntax batch AST covered token count overflowed u32. Fix: shard the corpus."
                        .to_string()
                })?;
            ast_window_count = ast_window_count
                .checked_add(summary.ast_window_count)
                .ok_or_else(|| {
                    "syntax batch AST window count overflowed u32. Fix: shard the corpus."
                        .to_string()
                })?;
            chunk_start = chunk_end;
        }
        return Ok(SyntaxBatchParseSummary {
            backend_id: backend_id.unwrap_or_else(|| "cuda".to_string()),
            file_count: u32::try_from(sources.len()).map_err(|_| {
                format!(
                    "syntax batch file count {} exceeds u32. Fix: shard the corpus.",
                    sources.len()
                )
            })?,
            source_bytes,
            batch_bytes,
            token_count,
            ast_bytes,
            ast_node_count,
            ast_covered_tokens,
            ast_window_count,
        });
    }
    let mut batch = Vec::with_capacity(usize::try_from(batch_len).map_err(|_| {
        format!(
            "syntax batch length {batch_len} exceeds host addressable memory. Fix: shard the corpus."
        )
    })?);
    for (index, source) in sources.iter().enumerate() {
        if index != 0 {
            batch.push(b'\n');
        }
        batch.extend_from_slice(source);
    }
    let prepared = prepare_resident_syntax_bytes(&batch)?;
    let summary = parse_prepared_resident_syntax(&prepared)?;
    Ok(SyntaxBatchParseSummary {
        backend_id: summary.backend_id,
        file_count: u32::try_from(sources.len()).map_err(|_| {
            format!(
                "syntax batch file count {} exceeds u32. Fix: shard the corpus.",
                sources.len()
            )
        })?,
        source_bytes,
        batch_bytes: prepared.source_bytes,
        token_count: summary.token_count,
        ast_bytes: summary.ast_bytes,
        ast_node_count: summary.ast_node_count,
        ast_covered_tokens: summary.ast_covered_tokens,
        ast_window_count: summary.ast_window_count,
    })
}

/// Pack raw C source bytes into the resident haystack shape used by GPU lexer stages.
pub fn prepare_resident_syntax_bytes(source: &[u8]) -> Result<PreparedResidentSyntaxBytes, String> {
    let haystack_len = u32::try_from(source.len())
        .map_err(|_| {
            format!(
                "syntax source length {} exceeds the current u32 GPU index space. Fix: shard the translation unit before dispatch.",
                source.len()
            )
        })?
        .max(1);
    let haystack_bytes = (haystack_len as usize).checked_mul(4).ok_or_else(|| {
        "syntax haystack byte length overflows usize. Fix: shard the translation unit before dispatch."
            .to_string()
    })?;
    let mut haystack = vec![0u8; haystack_bytes];
    for (index, byte) in source.iter().copied().enumerate() {
        let offset = index.checked_mul(4).ok_or_else(|| {
            "syntax haystack byte offset overflows usize. Fix: shard the translation unit before dispatch."
                .to_string()
        })?;
        haystack[offset] = byte;
    }
    Ok(PreparedResidentSyntaxBytes {
        source_bytes: u64::try_from(source.len()).map_err(|_| {
            "syntax source length exceeds u64. Fix: shard the translation unit before dispatch."
                .to_string()
        })?,
        haystack_len,
        quote_free: !source.contains(&b'"'),
        haystack: Arc::from(haystack),
    })
}

/// Parse prepared resident syntax bytes through the shared GPU frontend backend.
pub fn parse_prepared_resident_syntax(
    prepared: &PreparedResidentSyntaxBytes,
) -> Result<SyntaxParseSummary, String> {
    let backend = crate::pipeline::shared_dispatch_backend()?;
    let backend_id = backend.id().to_string();
    if backend_id != "cuda" {
        return Err(format!(
            "vyre-frontend-c syntax parser requires the CUDA release backend for raw-byte API tests, got {backend_id}. Fix: link/register vyre-driver-cuda before calling parse_syntax_bytes."
        ));
    }
    raw_syntax::parse_regular_sparse_syntax_bytes_gpu(
        prepared,
        backend.as_ref(),
        backend_id.clone(),
        syntax_summary_from_c_summary,
    )
}

/// Return current resident frontend cache counters.
#[must_use]
pub fn pipeline_cache_snapshot() -> PipelineCacheSnapshot {
    let cache_stats = RESIDENT_SYNTAX_CACHE
        .get()
        .and_then(|cache| cache.lock().ok().map(|guard| guard.stats()))
        .unwrap_or_default();
    PipelineCacheSnapshot {
        haystack_hits: HAYSTACK_HITS.load(Ordering::Relaxed),
        haystack_misses: HAYSTACK_MISSES.load(Ordering::Relaxed),
        haystack_evictions: cache_stats.evictions,
        haystack_rejected_oversized: cache_stats.rejected_oversized,
        haystack_entries: cache_stats.entries,
        haystack_bytes: cache_stats.bytes,
    }
}

pub(super) fn resident_syntax_key(source: &[u8]) -> ResidentSyntaxKey {
    ResidentSyntaxKey {
        len: u64::try_from(source.len()).unwrap_or(u64::MAX),
        hash: blake3_128(source),
    }
}

pub(super) fn insert_resident_syntax_cache(
    cache: &mut ResidentSyntaxCache,
    key: ResidentSyntaxKey,
    prepared: PreparedResidentSyntaxBytes,
) {
    insert_resident_syntax_cache_with_limits(
        cache,
        key,
        prepared,
        RESIDENT_SYNTAX_CACHE_MAX_ENTRIES,
        RESIDENT_SYNTAX_CACHE_MAX_BYTES,
    );
}

pub(super) fn lookup_resident_syntax_cache(
    cache: &mut ResidentSyntaxCache,
    key: &ResidentSyntaxKey,
) -> Option<PreparedResidentSyntaxBytes> {
    let epoch = cache.next_epoch();
    match cache.entries.get_mut(key) {
        Some(entry) => {
            entry.last_access = epoch;
            cache.stats.hits = cache.stats.hits.saturating_add(1);
            Some(entry.prepared.clone())
        }
        None => {
            cache.stats.misses = cache.stats.misses.saturating_add(1);
            None
        }
    }
}

pub(super) fn insert_resident_syntax_cache_with_limits(
    cache: &mut ResidentSyntaxCache,
    key: ResidentSyntaxKey,
    prepared: PreparedResidentSyntaxBytes,
    max_entries: usize,
    max_bytes: usize,
) {
    let entry_bytes = resident_syntax_entry_bytes(&prepared);
    if max_entries == 0 || entry_bytes > max_bytes {
        if let Some(old) = cache.entries.remove(&key) {
            cache.bytes = cache
                .bytes
                .saturating_sub(resident_syntax_entry_bytes(&old.prepared));
        }
        cache.stats.rejected_oversized = cache.stats.rejected_oversized.saturating_add(1);
        return;
    }
    if let Some(old) = cache.entries.remove(&key) {
        cache.bytes = cache
            .bytes
            .saturating_sub(resident_syntax_entry_bytes(&old.prepared));
    }
    while cache.len() >= max_entries
        || cache.bytes.checked_add(entry_bytes).unwrap_or(usize::MAX) > max_bytes
    {
        let Some(evict_key) = cache
            .entries
            .iter()
            .min_by_key(|(_, entry)| entry.last_access)
            .map(|(key, _)| *key)
        else {
            break;
        };
        if let Some(evicted) = cache.entries.remove(&evict_key) {
            let evicted_bytes = resident_syntax_entry_bytes(&evicted.prepared);
            cache.bytes = cache.bytes.saturating_sub(evicted_bytes);
            cache.stats.evictions = cache.stats.evictions.saturating_add(1);
        }
    }
    let last_access = cache.next_epoch();
    cache.bytes = cache.bytes.saturating_add(entry_bytes);
    cache.entries.insert(
        key,
        ResidentSyntaxCacheEntry {
            prepared,
            last_access,
        },
    );
    cache.stats.inserts = cache.stats.inserts.saturating_add(1);
}

pub(super) fn resident_syntax_entry_bytes(prepared: &PreparedResidentSyntaxBytes) -> usize {
    prepared.haystack.len()
}

pub(super) fn resident_syntax_cache_bytes(cache: &ResidentSyntaxCache) -> usize {
    cache.bytes
}

pub(super) fn syntax_summary_from_c_summary(
    backend_id: String,
    summary: CParseSummary,
    haystack_len: u32,
    _haystack_bytes: u64,
) -> SyntaxParseSummary {
    let tokens_per_window = vyre_libs::parsing::c::pipeline::stages::C11_AST_MAX_TOK_SCAN.max(1);
    let ast_covered_tokens = summary.token_count;
    let ast_window_count = summary.token_count.max(1).div_ceil(tokens_per_window);
    let ast_node_count = summary
        .ast_node_count
        .max(summary.token_count.min(haystack_len).max(1));
    SyntaxParseSummary {
        backend_id,
        source_bytes: summary.source_bytes,
        token_count: summary.token_count,
        ast_bytes: summary.ast_bytes,
        ast_node_count,
        ast_covered_tokens,
        ast_window_count,
    }
}

#[cfg(test)]

mod tests {
    use std::sync::Arc;

    use super::{
        insert_resident_syntax_cache_with_limits, lookup_resident_syntax_cache,
        resident_syntax_cache_bytes, resident_syntax_entry_bytes, resident_syntax_key,
        PreparedResidentSyntaxBytes, ResidentSyntaxCache,
    };

    #[test]
    fn resident_syntax_key_uses_128_bit_source_identity() {
        let a = resident_syntax_key(b"int main(void) { return 0; }\n");
        let b = resident_syntax_key(b"int main(void) { return 1; }\n");
        assert_eq!(a.hash.len(), 16);
        assert_ne!(a.hash, b.hash);
        assert_eq!(a.len, b.len);
    }

    fn prepared(source: &[u8], haystack: &[u8]) -> PreparedResidentSyntaxBytes {
        PreparedResidentSyntaxBytes {
            source_bytes: source.len() as u64,
            haystack_len: (haystack.len() / 4) as u32,
            quote_free: !source.contains(&b'"'),
            haystack: Arc::<[u8]>::from(haystack),
        }
    }

    #[test]
    fn resident_syntax_entry_bytes_counts_resident_haystack_only() {
        let entry = prepared(b"abc", b"01234567");
        assert_eq!(resident_syntax_entry_bytes(&entry), 8);
    }

    #[test]
    fn resident_syntax_cache_rejects_oversized_entry() {
        let mut cache = ResidentSyntaxCache::new();
        insert_resident_syntax_cache_with_limits(
            &mut cache,
            resident_syntax_key(b"large"),
            prepared(b"large", b"0123456789"),
            4,
            8,
        );
        assert!(cache.is_empty());
        assert_eq!(cache.stats().rejected_oversized, 1);
    }

    #[test]
    fn resident_syntax_cache_evicts_to_byte_budget() {
        let mut cache = ResidentSyntaxCache::new();
        insert_resident_syntax_cache_with_limits(
            &mut cache,
            resident_syntax_key(b"a"),
            prepared(b"a", b"1234"),
            4,
            10,
        );
        insert_resident_syntax_cache_with_limits(
            &mut cache,
            resident_syntax_key(b"b"),
            prepared(b"b", b"5678"),
            4,
            10,
        );
        insert_resident_syntax_cache_with_limits(
            &mut cache,
            resident_syntax_key(b"c"),
            prepared(b"c", b"zzzz"),
            4,
            10,
        );
        assert!(resident_syntax_cache_bytes(&cache) <= 10);
        assert_eq!(cache.stats().evictions, 1);
    }

    #[test]
    fn resident_syntax_cache_replacement_does_not_double_count() {
        let mut cache = ResidentSyntaxCache::new();
        let key = resident_syntax_key(b"same");
        insert_resident_syntax_cache_with_limits(
            &mut cache,
            key,
            prepared(b"same", b"aaaa"),
            4,
            16,
        );
        insert_resident_syntax_cache_with_limits(
            &mut cache,
            key,
            prepared(b"same", b"bbbb"),
            4,
            16,
        );
        assert_eq!(cache.len(), 1);
        assert_eq!(resident_syntax_cache_bytes(&cache), 4);
    }

    #[test]
    fn resident_syntax_cache_evicts_least_recently_used_entry() {
        let mut cache = ResidentSyntaxCache::new();
        let first = resident_syntax_key(b"first");
        let second = resident_syntax_key(b"second");
        let third = resident_syntax_key(b"third");
        insert_resident_syntax_cache_with_limits(&mut cache, first, prepared(b"a", b"1111"), 2, 10);
        insert_resident_syntax_cache_with_limits(
            &mut cache,
            second,
            prepared(b"b", b"2222"),
            2,
            10,
        );
        assert!(lookup_resident_syntax_cache(&mut cache, &first).is_some());
        insert_resident_syntax_cache_with_limits(&mut cache, third, prepared(b"c", b"3333"), 2, 10);

        assert!(cache.contains_key(&first));
        assert!(!cache.contains_key(&second));
        assert!(cache.contains_key(&third));
        assert_eq!(cache.stats().hits, 1);
        assert_eq!(cache.stats().evictions, 1);
    }
}
