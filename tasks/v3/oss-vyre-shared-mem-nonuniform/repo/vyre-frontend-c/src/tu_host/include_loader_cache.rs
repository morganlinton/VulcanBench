use std::collections::{hash_map::DefaultHasher, HashMap};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::resident_cache::ResidentPrepDep;

#[derive(Clone, Eq, PartialEq)]
pub(super) struct ResolveCacheKey {
    dir: PathBuf,
    is_system: bool,
    is_next: bool,
    name: Vec<u8>,
}

impl ResolveCacheKey {
    pub(super) fn new(dir: PathBuf, is_system: bool, is_next: bool, name: &[u8]) -> Self {
        Self {
            dir,
            is_system,
            is_next,
            name: name.to_vec(),
        }
    }

    fn hash(&self) -> u64 {
        resolve_cache_hash(self.dir.as_path(), self.is_system, self.is_next, &self.name)
    }

    fn matches(&self, dir: &Path, is_system: bool, is_next: bool, name: &[u8]) -> bool {
        self.is_system == is_system
            && self.is_next == is_next
            && self.dir.as_path() == dir
            && self.name.as_slice() == name
    }
}

fn resolve_cache_hash(dir: &Path, is_system: bool, is_next: bool, name: &[u8]) -> u64 {
    let mut hasher = DefaultHasher::new();
    dir.hash(&mut hasher);
    is_system.hash(&mut hasher);
    is_next.hash(&mut hasher);
    name.hash(&mut hasher);
    hasher.finish()
}

const INCLUDE_RESOLVE_CACHE_MAX_ENTRIES: usize = 65_536;
const INCLUDE_FILE_CACHE_MAX_ENTRIES: usize = 8_192;
const INCLUDE_FILE_CACHE_MAX_BYTES: usize = 512 * 1024 * 1024;
const INCLUDE_DEPENDENCY_CACHE_MAX_ENTRIES: usize = 65_536;

pub(super) struct ResidentIncludeResolveCache {
    entries: HashMap<u64, Vec<ResolveCacheEntry>>,
    len: usize,
    max_entries: usize,
    epoch: u64,
}

struct ResolveCacheEntry {
    key: ResolveCacheKey,
    value: Option<PathBuf>,
    last_access: u64,
}

impl ResidentIncludeResolveCache {
    pub(super) fn new() -> Self {
        Self {
            entries: HashMap::new(),
            len: 0,
            max_entries: INCLUDE_RESOLVE_CACHE_MAX_ENTRIES,
            epoch: 0,
        }
    }

    #[cfg(test)]
    pub(super) fn with_limit(max_entries: usize) -> Self {
        Self {
            entries: HashMap::new(),
            len: 0,
            max_entries,
            epoch: 0,
        }
    }

    pub(super) fn lookup(
        &mut self,
        dir: &Path,
        is_system: bool,
        is_next: bool,
        name: &[u8],
    ) -> Option<Option<PathBuf>> {
        let hash = resolve_cache_hash(dir, is_system, is_next, name);
        let index = self
            .entries
            .get(&hash)?
            .iter()
            .position(|entry| entry.key.matches(dir, is_system, is_next, name))?;
        let next_epoch = self.next_epoch();
        let Some(entry) = self
            .entries
            .get_mut(&hash)
            .and_then(|bucket| bucket.get_mut(index))
        else {
            return None;
        };
        entry.last_access = next_epoch;
        Some(entry.value.clone())
    }

    pub(super) fn insert(&mut self, key: ResolveCacheKey, value: Option<PathBuf>) {
        if self.max_entries == 0 {
            self.remove(&key);
            return;
        }
        self.remove(&key);
        while self.len >= self.max_entries {
            let Some(evict_key) = self.least_recently_used_key() else {
                break;
            };
            self.remove(&evict_key);
        }
        let last_access = self.next_epoch();
        let hash = key.hash();
        self.entries
            .entry(hash)
            .or_default()
            .push(ResolveCacheEntry {
                key,
                value,
                last_access,
            });
        self.len = self.len.saturating_add(1);
    }

    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.len
    }

    #[cfg(test)]
    pub(super) fn contains_key(&self, key: &ResolveCacheKey) -> bool {
        self.entries
            .get(&key.hash())
            .map(|bucket| {
                bucket.iter().any(|entry| {
                    entry
                        .key
                        .matches(key.dir.as_path(), key.is_system, key.is_next, &key.name)
                })
            })
            .unwrap_or(false)
    }

    fn remove(&mut self, key: &ResolveCacheKey) -> Option<ResolveCacheEntry> {
        let hash = key.hash();
        let (entry, remove_bucket) = {
            let bucket = self.entries.get_mut(&hash)?;
            let index = bucket.iter().position(|entry| {
                entry
                    .key
                    .matches(key.dir.as_path(), key.is_system, key.is_next, &key.name)
            })?;
            let entry = bucket.swap_remove(index);
            (entry, bucket.is_empty())
        };
        if remove_bucket {
            self.entries.remove(&hash);
        }
        self.len = self.len.saturating_sub(1);
        Some(entry)
    }

    fn next_epoch(&mut self) -> u64 {
        self.epoch = self.epoch.saturating_add(1);
        self.epoch
    }

    fn least_recently_used_key(&self) -> Option<ResolveCacheKey> {
        self.entries
            .values()
            .flat_map(|bucket| bucket.iter())
            .min_by_key(|entry| entry.last_access)
            .map(|entry| entry.key.clone())
    }
}

pub(super) struct ResidentIncludeFileCache {
    entries: HashMap<PathBuf, ResidentIncludeFileEntry>,
    bytes: usize,
    max_entries: usize,
    max_bytes: usize,
    epoch: u64,
    stats: ResidentIncludeFileCacheStats,
}

struct ResidentIncludeFileEntry {
    bytes: Arc<[u8]>,
    dep: ResidentPrepDep,
    last_access: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct ResidentIncludeFileCacheStats {
    pub(super) hits: u64,
    pub(super) misses: u64,
    pub(super) inserts: u64,
    pub(super) evictions: u64,
    pub(super) stale_discards: u64,
    pub(super) entries: usize,
    pub(super) bytes: usize,
}

impl ResidentIncludeFileCache {
    pub(super) fn new() -> Self {
        Self {
            entries: HashMap::new(),
            bytes: 0,
            max_entries: INCLUDE_FILE_CACHE_MAX_ENTRIES,
            max_bytes: INCLUDE_FILE_CACHE_MAX_BYTES,
            epoch: 0,
            stats: ResidentIncludeFileCacheStats::default(),
        }
    }

    #[cfg(test)]
    pub(super) fn with_limits(max_entries: usize, max_bytes: usize) -> Self {
        Self {
            entries: HashMap::new(),
            bytes: 0,
            max_entries,
            max_bytes,
            epoch: 0,
            stats: ResidentIncludeFileCacheStats::default(),
        }
    }

    pub(super) fn lookup(&mut self, path: &Path) -> Option<(Arc<[u8]>, ResidentPrepDep)> {
        let next_epoch = self.next_epoch();
        let Some(entry) = self.entries.get_mut(path) else {
            self.stats.misses = self.stats.misses.saturating_add(1);
            return None;
        };
        entry.last_access = next_epoch;
        self.stats.hits = self.stats.hits.saturating_add(1);
        Some((Arc::clone(&entry.bytes), entry.dep.clone()))
    }

    pub(super) fn insert(
        &mut self,
        path: PathBuf,
        bytes: impl Into<Arc<[u8]>>,
        dep: ResidentPrepDep,
    ) {
        let bytes = bytes.into();
        if self.max_entries == 0 || bytes.len() > self.max_bytes {
            self.remove(&path);
            return;
        }
        self.remove(&path);
        while self.entries.len() >= self.max_entries
            || self.bytes.checked_add(bytes.len()).unwrap_or(usize::MAX) > self.max_bytes
        {
            let Some(evict_key) = self.least_recently_used_key() else {
                break;
            };
            self.remove(&evict_key);
        }
        let last_access = self.next_epoch();
        self.bytes = self.bytes.saturating_add(bytes.len());
        self.entries.insert(
            path,
            ResidentIncludeFileEntry {
                bytes,
                dep,
                last_access,
            },
        );
        self.stats.inserts = self.stats.inserts.saturating_add(1);
    }

    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.entries.len()
    }

    #[cfg(test)]
    pub(super) fn byte_len(&self) -> usize {
        self.bytes
    }

    pub(super) fn stats(&self) -> ResidentIncludeFileCacheStats {
        ResidentIncludeFileCacheStats {
            entries: self.entries.len(),
            bytes: self.bytes,
            ..self.stats
        }
    }

    #[cfg(test)]
    pub(super) fn contains_key(&self, path: &Path) -> bool {
        self.entries.contains_key(path)
    }

    pub(super) fn discard(&mut self, path: &Path) {
        if self.remove(path).is_some() {
            self.stats.stale_discards = self.stats.stale_discards.saturating_add(1);
        }
    }

    fn remove(&mut self, path: &Path) -> Option<ResidentIncludeFileEntry> {
        let entry = self.entries.remove(path)?;
        self.bytes = self.bytes.saturating_sub(entry.bytes.len());
        self.stats.evictions = self.stats.evictions.saturating_add(1);
        Some(entry)
    }

    fn next_epoch(&mut self) -> u64 {
        self.epoch = self.epoch.saturating_add(1);
        self.epoch
    }

    fn least_recently_used_key(&self) -> Option<PathBuf> {
        self.entries
            .iter()
            .min_by_key(|(_, entry)| entry.last_access)
            .map(|(key, _)| key.clone())
    }
}

pub(super) struct ResidentIncludeDependencyCache {
    entries: HashMap<PathBuf, ResidentPrepDep>,
    max_entries: usize,
}

impl ResidentIncludeDependencyCache {
    pub(super) fn new() -> Self {
        Self {
            entries: HashMap::new(),
            max_entries: INCLUDE_DEPENDENCY_CACHE_MAX_ENTRIES,
        }
    }

    #[cfg(test)]
    pub(super) fn with_limit(max_entries: usize) -> Self {
        Self {
            entries: HashMap::new(),
            max_entries,
        }
    }

    pub(super) fn insert(&mut self, path: PathBuf, dep: ResidentPrepDep) -> Result<(), String> {
        if !self.entries.contains_key(&path) && self.entries.len() >= self.max_entries {
            return Err(format!(
                "vyre-frontend-c: resident include dependency set exceeded {} files while reading {}. Fix: reduce generated include fanout or split the translation unit; dependency records cannot be evicted without weakening cache invalidation.",
                self.max_entries,
                path.display()
            ));
        }
        self.entries.insert(path, dep);
        Ok(())
    }

    pub(super) fn signature(&self) -> Vec<ResidentPrepDep> {
        let mut deps = self.entries.values().cloned().collect::<Vec<_>>();
        deps.sort_by(|left, right| left.path.cmp(&right.path));
        deps
    }

    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.entries.len()
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use super::super::include_loader::include_modified_ns;
    use super::super::{resident_dep_from_metadata, stable_hash_bytes};
    use super::{
        ResidentIncludeDependencyCache, ResidentIncludeFileCache, ResidentIncludeResolveCache,
        ResolveCacheKey,
    };
    use crate::tu_host::resident_cache::ResidentPrepDep;

    fn resolve_key(name: &[u8]) -> ResolveCacheKey {
        ResolveCacheKey::new(PathBuf::from("."), false, false, name)
    }

    fn lookup_key(
        cache: &mut ResidentIncludeResolveCache,
        key: &ResolveCacheKey,
    ) -> Option<Option<PathBuf>> {
        cache.lookup(key.dir.as_path(), key.is_system, key.is_next, &key.name)
    }

    #[test]
    fn resolve_cache_evicts_least_recently_used_entry() {
        let mut cache = ResidentIncludeResolveCache::with_limit(2);
        let a = resolve_key(b"a.h");
        let b = resolve_key(b"b.h");
        let c = resolve_key(b"c.h");
        cache.insert(a.clone(), Some(PathBuf::from("a.h")));
        cache.insert(b.clone(), Some(PathBuf::from("b.h")));
        assert!(lookup_key(&mut cache, &a).is_some());
        cache.insert(c.clone(), Some(PathBuf::from("c.h")));
        assert!(cache.contains_key(&a));
        assert!(!cache.contains_key(&b));
        assert!(cache.contains_key(&c));
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn resolve_cache_lookup_uses_borrowed_key_material() {
        let mut cache = ResidentIncludeResolveCache::with_limit(2);
        let key = ResolveCacheKey::new(PathBuf::from("/tmp/includes"), false, true, b"shared.h");
        cache.insert(key.clone(), Some(PathBuf::from("/tmp/includes/shared.h")));

        let hit = cache
            .lookup(Path::new("/tmp/includes"), false, true, b"shared.h")
            .expect("Fix: borrowed key material must hit the owned cache entry")
            .expect("Fix: cached include must resolve");

        assert_eq!(hit, PathBuf::from("/tmp/includes/shared.h"));
        assert!(cache.contains_key(&key));
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn file_cache_rejects_oversized_entry() {
        let mut cache = ResidentIncludeFileCache::with_limits(4, 8);
        cache.insert(PathBuf::from("huge.h"), vec![0; 9], test_dep("huge.h"));
        assert_eq!(cache.len(), 0);
        assert_eq!(cache.byte_len(), 0);
    }

    #[test]
    fn file_cache_evicts_least_recently_used_entry_to_byte_budget() {
        let mut cache = ResidentIncludeFileCache::with_limits(4, 8);
        let a = PathBuf::from("a.h");
        let b = PathBuf::from("b.h");
        let c = PathBuf::from("c.h");
        cache.insert(a.clone(), vec![0; 4], test_dep("a.h"));
        cache.insert(b.clone(), vec![0; 4], test_dep("b.h"));
        assert!(cache.lookup(&a).is_some());
        cache.insert(c.clone(), vec![0; 4], test_dep("c.h"));
        assert!(cache.contains_key(&a));
        assert!(!cache.contains_key(&b));
        assert!(cache.contains_key(&c));
        assert_eq!(cache.byte_len(), 8);
    }

    #[test]
    fn file_cache_replacement_does_not_double_count() {
        let mut cache = ResidentIncludeFileCache::with_limits(4, 8);
        cache.insert(PathBuf::from("same.h"), vec![1; 6], test_dep("same.h"));
        cache.insert(PathBuf::from("same.h"), vec![2; 6], test_dep("same.h"));
        assert_eq!(cache.len(), 1);
        assert_eq!(cache.byte_len(), 6);
    }

    #[test]
    fn file_cache_lookup_reuses_shared_bytes_without_copy() {
        let mut cache = ResidentIncludeFileCache::with_limits(4, 8);
        let bytes = std::sync::Arc::<[u8]>::from(vec![1, 2, 3]);
        cache.insert(
            PathBuf::from("shared.h"),
            std::sync::Arc::clone(&bytes),
            test_dep("shared.h"),
        );

        let first = cache
            .lookup(PathBuf::from("shared.h").as_path())
            .expect("Fix: cached include must be present");
        let second = cache
            .lookup(PathBuf::from("shared.h").as_path())
            .expect("Fix: cached include must still be present");
        let missing = cache.lookup(PathBuf::from("missing.h").as_path());

        assert!(std::sync::Arc::ptr_eq(&bytes, &first.0));
        assert!(std::sync::Arc::ptr_eq(&first.0, &second.0));
        assert_eq!(first.1.path, PathBuf::from("shared.h"));
        assert_eq!(second.1.path, PathBuf::from("shared.h"));
        assert!(missing.is_none());
        assert_eq!(cache.byte_len(), 3);
        let stats = cache.stats();
        assert_eq!(stats.hits, 2);
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.inserts, 1);
        assert_eq!(stats.entries, 1);
        assert_eq!(stats.bytes, 3);
    }

    #[test]
    fn dependency_cache_rejects_silent_eviction_when_entry_budget_is_full() {
        let dep = resident_dep_from_metadata(
            PathBuf::from("a.h"),
            &fs::metadata(std::env::current_exe().unwrap()).unwrap(),
            0,
            [0; 16],
        );
        let mut cache = ResidentIncludeDependencyCache::with_limit(1);
        cache.insert(PathBuf::from("a.h"), dep.clone()).unwrap();
        let err = cache.insert(PathBuf::from("b.h"), dep).unwrap_err();
        assert!(err.contains("dependency records cannot be evicted"));
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn dependency_signature_is_sorted_and_preserves_all_entries() {
        let tmp = std::env::temp_dir().join("vyre_frontend_c_include_dep_cache");
        match fs::remove_dir_all(&tmp) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => panic!(
                "failed to clean include dependency test directory {}: {error}",
                tmp.display()
            ),
        }
        fs::create_dir_all(&tmp).unwrap();
        let a = tmp.join("a.h");
        let b = tmp.join("b.h");
        fs::write(&a, b"#define A 1\n").unwrap();
        fs::write(&b, b"#define B 2\n").unwrap();
        let mut cache = ResidentIncludeDependencyCache::with_limit(2);
        for path in [&b, &a] {
            let metadata = fs::metadata(path).unwrap();
            cache
                .insert(
                    path.clone(),
                    resident_dep_from_metadata(
                        path.clone(),
                        &metadata,
                        include_modified_ns(path, &metadata).unwrap(),
                        stable_hash_bytes(&fs::read(path).unwrap()),
                    ),
                )
                .unwrap();
        }
        let deps = cache.signature();
        assert_eq!(deps.len(), 2);
        assert_eq!(deps[0].path, a);
        assert_eq!(deps[1].path, b);
    }

    fn test_dep(path: &str) -> ResidentPrepDep {
        ResidentPrepDep {
            path: PathBuf::from(path),
            len: 0,
            modified_ns: 0,
            change_ns: 0,
            dev: 0,
            ino: 0,
            content_hash: [0; 16],
        }
    }
}
