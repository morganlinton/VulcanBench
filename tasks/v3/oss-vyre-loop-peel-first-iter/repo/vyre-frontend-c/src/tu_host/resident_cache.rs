use super::*;
use std::sync::Arc;

#[derive(Clone, Hash, PartialEq, Eq)]
pub(super) struct ResidentPrepKey {
    pub(super) tu_path: PathBuf,
    pub(super) source_hash: StableHash128,
    pub(super) options_hash: StableHash128,
}

#[derive(Clone)]
pub(super) struct ResidentPrepEntry {
    pub(super) source: String,
    pub(super) deps: Arc<[ResidentPrepDep]>,
}

#[derive(Clone)]
pub(super) struct ResidentPrepDep {
    pub(super) path: PathBuf,
    pub(super) len: u64,
    pub(super) modified_ns: u128,
    pub(super) change_ns: i128,
    pub(super) dev: u64,
    pub(super) ino: u64,
    pub(super) content_hash: StableHash128,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct ResidentPrepCacheStats {
    pub(super) hits: u64,
    pub(super) misses: u64,
    pub(super) stale_removals: u64,
    pub(super) inserts: u64,
    pub(super) evictions: u64,
    pub(super) rejected_oversized: u64,
    pub(super) entries: usize,
    pub(super) bytes: usize,
}

pub(super) struct ResidentPrepCache {
    entries: HashMap<ResidentPrepKey, ResidentPrepCacheEntry>,
    epoch: u64,
    bytes: usize,
    stats: ResidentPrepCacheStats,
}

struct ResidentPrepCacheEntry {
    entry: ResidentPrepEntry,
    last_access: u64,
}

impl Default for ResidentPrepCache {
    fn default() -> Self {
        Self {
            entries: HashMap::new(),
            epoch: 0,
            bytes: 0,
            stats: ResidentPrepCacheStats::default(),
        }
    }
}

impl ResidentPrepCache {
    pub(super) fn new() -> Self {
        Self::default()
    }

    pub(super) fn len(&self) -> usize {
        self.entries.len()
    }

    pub(super) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub(super) fn contains_key(&self, key: &ResidentPrepKey) -> bool {
        self.entries.contains_key(key)
    }

    pub(super) fn keys(&self) -> impl Iterator<Item = &ResidentPrepKey> {
        self.entries.keys()
    }

    pub(super) fn stats(&self) -> ResidentPrepCacheStats {
        ResidentPrepCacheStats {
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

pub(super) fn resident_prep_cache() -> &'static Mutex<ResidentPrepCache> {
    static CACHE: OnceLock<Mutex<ResidentPrepCache>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(ResidentPrepCache::new()))
}

pub(super) fn resident_prep_key(
    tu_path: &Path,
    prefixed_source: &[u8],
    options: &VyreCompileOptions,
) -> Result<ResidentPrepKey, String> {
    let canonical_tu_path = fs::canonicalize(tu_path).map_err(|error| {
        format!(
            "vyre-frontend-c: translation unit {} could not be canonicalized for resident preprocessor cache key: {error}. Fix: validate input path before preprocessing.",
            tu_path.display()
        )
    })?;
    Ok(ResidentPrepKey {
        tu_path: canonical_tu_path,
        source_hash: stable_hash_bytes(prefixed_source),
        options_hash: resident_options_hash(options),
    })
}

pub(super) fn resident_options_hash(options: &VyreCompileOptions) -> StableHash128 {
    let mut hash = blake3::Hasher::new();
    hash_path_class(&mut hash, b"include", &options.include_dirs);
    hash_path_class(&mut hash, b"quote-include", &options.quote_include_dirs);
    hash_path_class(&mut hash, b"system-include", &options.system_include_dirs);
    hash_path_class(&mut hash, b"after-include", &options.after_include_dirs);
    hash_path_class(&mut hash, b"imacros", &options.imacro_files);
    hash_path_class(&mut hash, b"forced-include", &options.forced_include_files);
    if let Some(sysroot) = &options.system_include_sysroot {
        blake3_128_update_len_prefixed(&mut hash, sysroot.as_os_str().as_encoded_bytes());
    } else {
        blake3_128_update_len_prefixed(&mut hash, &[]);
    }
    blake3_128_update_len_prefixed(&mut hash, &[u8::from(options.disable_system_include_dirs)]);
    blake3_128_update_len_prefixed(&mut hash, &options.target.cache_tag().to_le_bytes());
    hash_macro_actions(&mut hash, &cli_macro_actions(options));
    blake3_128_from_hasher(&hash)
}

pub(super) fn stable_hash_bytes(bytes: &[u8]) -> StableHash128 {
    blake3_128(bytes)
}

pub(super) fn hash_paths(hash: &mut blake3::Hasher, paths: &[PathBuf]) {
    blake3_128_update_len_prefixed(hash, &(paths.len() as u64).to_le_bytes());
    for path in paths {
        blake3_128_update_len_prefixed(hash, path.as_os_str().as_encoded_bytes());
    }
}

pub(super) fn hash_path_class(hash: &mut blake3::Hasher, class: &[u8], paths: &[PathBuf]) {
    blake3_128_update_len_prefixed(hash, class);
    hash_paths(hash, paths);
}

pub(super) fn hash_macro_actions(hash: &mut blake3::Hasher, actions: &[CliMacroAction]) {
    blake3_128_update_len_prefixed(hash, &(actions.len() as u64).to_le_bytes());
    for action in actions {
        match action {
            CliMacroAction::Define { name, value } => {
                blake3_128_update_len_prefixed(hash, b"D");
                blake3_128_update_len_prefixed(hash, name.as_bytes());
                match value {
                    Some(value) => {
                        blake3_128_update_len_prefixed(hash, b"1");
                        blake3_128_update_len_prefixed(hash, value.as_bytes());
                    }
                    None => blake3_128_update_len_prefixed(hash, b"0"),
                }
            }
            CliMacroAction::DefineFunction {
                name,
                params,
                value,
            } => {
                blake3_128_update_len_prefixed(hash, b"F");
                blake3_128_update_len_prefixed(hash, name.as_bytes());
                blake3_128_update_len_prefixed(hash, &(params.len() as u64).to_le_bytes());
                for param in params {
                    blake3_128_update_len_prefixed(hash, param.as_bytes());
                }
                match value {
                    Some(value) => {
                        blake3_128_update_len_prefixed(hash, b"1");
                        blake3_128_update_len_prefixed(hash, value.as_bytes());
                    }
                    None => blake3_128_update_len_prefixed(hash, b"0"),
                }
            }
            CliMacroAction::Undef { name } => {
                blake3_128_update_len_prefixed(hash, b"U");
                blake3_128_update_len_prefixed(hash, name.as_bytes());
            }
        }
    }
}

pub(super) fn resident_prep_deps_valid(deps: &[ResidentPrepDep]) -> Result<bool, String> {
    for dep in deps {
        let metadata = match fs::metadata(&dep.path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(error) => {
                return Err(format!(
                    "vyre-frontend-c: stat cached include dependency {} during resident prep cache validation: {error}. Fix: repair filesystem metadata access before relying on cache invalidation.",
                    dep.path.display()
                ));
            }
        };
        let modified = metadata.modified().map_err(|error| {
            format!(
                "vyre-frontend-c: read mtime for cached include dependency {} during resident prep cache validation: {error}. Fix: repair filesystem metadata access before relying on cache invalidation.",
                dep.path.display()
            )
        })?;
        let duration = modified.duration_since(std::time::UNIX_EPOCH).map_err(|error| {
            format!(
                "vyre-frontend-c: cached include dependency {} has pre-UNIX_EPOCH mtime during resident prep cache validation: {error}. Fix: repair filesystem timestamps before relying on cache invalidation.",
                dep.path.display()
            )
        })?;
        if metadata.len() != dep.len || duration.as_nanos() != dep.modified_ns {
            return Ok(false);
        }
        if resident_dep_metadata_identity_available() {
            if resident_dep_dev(&metadata) != dep.dev
                || resident_dep_ino(&metadata) != dep.ino
                || resident_dep_change_ns(&metadata) != dep.change_ns
            {
                return Ok(false);
            }
            continue;
        }
        let bytes = read_include_bounded(&dep.path)?;
        if stable_hash_bytes(&bytes) != dep.content_hash {
            return Ok(false);
        }
    }
    Ok(true)
}

pub(super) fn resident_dep_from_metadata(
    path: PathBuf,
    metadata: &fs::Metadata,
    modified_ns: u128,
    content_hash: StableHash128,
) -> ResidentPrepDep {
    ResidentPrepDep {
        path,
        len: metadata.len(),
        modified_ns,
        change_ns: resident_dep_change_ns(metadata),
        dev: resident_dep_dev(metadata),
        ino: resident_dep_ino(metadata),
        content_hash,
    }
}

#[cfg(unix)]
pub(super) fn resident_dep_metadata_identity_available() -> bool {
    true
}

#[cfg(not(unix))]
pub(super) fn resident_dep_metadata_identity_available() -> bool {
    false
}

#[cfg(unix)]
pub(super) fn resident_dep_dev(metadata: &fs::Metadata) -> u64 {
    use std::os::unix::fs::MetadataExt as _;

    metadata.dev()
}

#[cfg(not(unix))]
pub(super) fn resident_dep_dev(_metadata: &fs::Metadata) -> u64 {
    0
}

#[cfg(unix)]
pub(super) fn resident_dep_ino(metadata: &fs::Metadata) -> u64 {
    use std::os::unix::fs::MetadataExt as _;

    metadata.ino()
}

#[cfg(not(unix))]
pub(super) fn resident_dep_ino(_metadata: &fs::Metadata) -> u64 {
    0
}

#[cfg(unix)]
pub(super) fn resident_dep_change_ns(metadata: &fs::Metadata) -> i128 {
    use std::os::unix::fs::MetadataExt as _;

    i128::from(metadata.ctime())
        .saturating_mul(1_000_000_000)
        .saturating_add(i128::from(metadata.ctime_nsec()))
}

#[cfg(not(unix))]
pub(super) fn resident_dep_change_ns(_metadata: &fs::Metadata) -> i128 {
    0
}

pub(super) fn insert_resident_prep_cache(
    cache: &mut ResidentPrepCache,
    key: ResidentPrepKey,
    entry: ResidentPrepEntry,
) {
    insert_resident_prep_cache_with_limits(
        cache,
        key,
        entry,
        RESIDENT_PREP_CACHE_MAX_ENTRIES,
        RESIDENT_PREP_CACHE_MAX_BYTES,
    );
}

#[cfg(test)]
pub(super) fn lookup_resident_prep_cache(
    cache: &mut ResidentPrepCache,
    key: &ResidentPrepKey,
) -> Option<ResidentPrepEntry> {
    let epoch = cache.next_epoch();
    match cache.entries.get_mut(key) {
        Some(entry) => {
            entry.last_access = epoch;
            cache.stats.hits = cache.stats.hits.saturating_add(1);
            Some(entry.entry.clone())
        }
        None => {
            cache.stats.misses = cache.stats.misses.saturating_add(1);
            None
        }
    }
}

pub(super) fn lookup_resident_prep_cache_deps(
    cache: &mut ResidentPrepCache,
    key: &ResidentPrepKey,
) -> Option<Arc<[ResidentPrepDep]>> {
    let epoch = cache.next_epoch();
    match cache.entries.get_mut(key) {
        Some(entry) => {
            entry.last_access = epoch;
            cache.stats.hits = cache.stats.hits.saturating_add(1);
            Some(entry.entry.deps.clone())
        }
        None => {
            cache.stats.misses = cache.stats.misses.saturating_add(1);
            None
        }
    }
}

pub(super) fn clone_resident_prep_cache_source(
    cache: &ResidentPrepCache,
    key: &ResidentPrepKey,
) -> Option<String> {
    cache
        .entries
        .get(key)
        .map(|entry| entry.entry.source.clone())
}

pub(super) fn remove_stale_resident_prep_cache_entry(
    cache: &mut ResidentPrepCache,
    key: &ResidentPrepKey,
) {
    if let Some(entry) = cache.entries.remove(key) {
        cache.bytes = cache.bytes.saturating_sub(entry.entry.source.len());
        cache.stats.stale_removals = cache.stats.stale_removals.saturating_add(1);
    }
}

pub(super) fn insert_resident_prep_cache_with_limits(
    cache: &mut ResidentPrepCache,
    key: ResidentPrepKey,
    entry: ResidentPrepEntry,
    max_entries: usize,
    max_bytes: usize,
) {
    let entry_bytes = entry.source.len();
    if entry_bytes > max_bytes {
        cache.stats.rejected_oversized = cache.stats.rejected_oversized.saturating_add(1);
        return;
    }
    if let Some(old_entry) = cache.entries.remove(&key) {
        cache.bytes = cache.bytes.saturating_sub(old_entry.entry.source.len());
    }
    while cache.len() >= max_entries
        || cache.bytes.checked_add(entry_bytes).unwrap_or(usize::MAX) > max_bytes
    {
        let Some(evict_key) = cache
            .entries
            .iter()
            .min_by_key(|(_, entry)| entry.last_access)
            .map(|(key, _)| key.clone())
        else {
            break;
        };
        if let Some(evicted) = cache.entries.remove(&evict_key) {
            cache.bytes = cache.bytes.saturating_sub(evicted.entry.source.len());
            cache.stats.evictions = cache.stats.evictions.saturating_add(1);
        }
    }
    if max_entries == 0 {
        return;
    }
    let last_access = cache.next_epoch();
    cache.bytes = cache.bytes.saturating_add(entry_bytes);
    cache
        .entries
        .insert(key, ResidentPrepCacheEntry { entry, last_access });
    cache.stats.inserts = cache.stats.inserts.saturating_add(1);
}

pub(super) fn resident_prep_cache_bytes(cache: &ResidentPrepCache) -> usize {
    cache.bytes
}

// Reference-only prep lives behind the `cpu-oracle` feature; production uses
// GPU resident preprocessing.
