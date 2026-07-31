use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use super::gpu_pipeline::IncludeLoader;
use super::include_loader_cache::{
    ResidentIncludeDependencyCache, ResidentIncludeFileCache, ResidentIncludeResolveCache,
    ResolveCacheKey,
};
use super::include_search::IncludeSearchDirs;
use super::resident_cache::ResidentPrepDep;
use super::{
    expanded_include_search_dirs, read_include_bounded, resident_dep_from_metadata,
    resident_prep_deps_valid, search_include_file, search_include_next_file,
    search_system_include_file, stable_hash_bytes,
};

pub(super) struct ResidentIncludeLoader {
    include_dirs: IncludeSearchDirs,
    resolve_cache: Mutex<ResidentIncludeResolveCache>,
    file_cache: Arc<Mutex<ResidentIncludeFileCache>>,
    dependency_cache: Mutex<ResidentIncludeDependencyCache>,
}

fn resident_include_file_cache() -> Arc<Mutex<ResidentIncludeFileCache>> {
    static CACHE: OnceLock<Arc<Mutex<ResidentIncludeFileCache>>> = OnceLock::new();
    Arc::clone(CACHE.get_or_init(|| Arc::new(Mutex::new(ResidentIncludeFileCache::new()))))
}

impl ResidentIncludeLoader {
    pub(super) fn new(
        include_dirs: &[PathBuf],
        quote_include_dirs: &[PathBuf],
        system_include_dirs: &[PathBuf],
        after_include_dirs: &[PathBuf],
        use_system_include_dirs: bool,
        system_include_sysroot: Option<&Path>,
    ) -> Result<Self, String> {
        Ok(Self {
            include_dirs: expanded_include_search_dirs(
                include_dirs,
                quote_include_dirs,
                system_include_dirs,
                after_include_dirs,
                use_system_include_dirs,
                system_include_sysroot,
            )?,
            resolve_cache: Mutex::new(ResidentIncludeResolveCache::new()),
            file_cache: resident_include_file_cache(),
            dependency_cache: Mutex::new(ResidentIncludeDependencyCache::new()),
        })
    }

    fn cached_include_bytes(&self, path: &Path) -> Result<Arc<[u8]>, String> {
        let cached = self
            .file_cache
            .lock()
            .map_err(|_| "vyre-frontend-c: include file cache poisoned".to_string())?
            .lookup(path);
        if let Some((bytes, dep)) = cached {
            if resident_prep_deps_valid(std::slice::from_ref(&dep))? {
                self.dependency_cache
                    .lock()
                    .map_err(|_| "vyre-frontend-c: include dependency cache poisoned".to_string())?
                    .insert(path.to_path_buf(), dep)?;
                return Ok(bytes);
            }
            self.file_cache
                .lock()
                .map_err(|_| "vyre-frontend-c: include file cache poisoned".to_string())?
                .discard(path);
        }
        let (bytes, dep) = read_include_record(path)?;
        let bytes = Arc::<[u8]>::from(bytes);
        let mut deps = self
            .dependency_cache
            .lock()
            .map_err(|_| "vyre-frontend-c: include dependency cache poisoned".to_string())?;
        deps.insert(path.to_path_buf(), dep.clone())?;
        let mut guard = self
            .file_cache
            .lock()
            .map_err(|_| "vyre-frontend-c: include file cache poisoned".to_string())?;
        guard.insert(path.to_path_buf(), Arc::clone(&bytes), dep);
        Ok(bytes)
    }

    pub(super) fn dependency_signature(&self) -> Result<Vec<ResidentPrepDep>, String> {
        Ok(self
            .dependency_cache
            .lock()
            .map_err(|_| "vyre-frontend-c: include dependency cache poisoned".to_string())?
            .signature())
    }
}

impl IncludeLoader for ResidentIncludeLoader {
    fn load(
        &self,
        path: &[u8],
        is_system: bool,
        is_next: bool,
        from: &Path,
    ) -> Result<Option<(PathBuf, Arc<[u8]>)>, String> {
        let name = std::str::from_utf8(path)
            .map_err(|error| format!("include path is not UTF-8: {error}"))?;
        let from_dir = from.parent().unwrap_or_else(|| Path::new("."));
        let cache_key_dir = if is_system { Path::new("") } else { from_dir };
        if let Some(cached) = self
            .resolve_cache
            .lock()
            .map_err(|_| "vyre-frontend-c: include resolve cache poisoned".to_string())?
            .lookup(cache_key_dir, is_system, is_next, path)
        {
            let Some(canon) = cached else {
                return Err(format!(
                    "vyre-frontend-c: include `{name}` was previously unresolved from {}. Fix: pass the required -I directory or generated header path.",
                    from.display()
                ));
            };
            let bytes = self.cached_include_bytes(&canon)?;
            return Ok(Some((canon, bytes)));
        }
        let resolved = if is_next {
            search_include_next_file(name, from, &self.include_dirs)?
        } else if is_system {
            search_system_include_file(name, &self.include_dirs)
        } else {
            search_include_file(name, from_dir, &self.include_dirs)
        };
        let Some(resolved) = resolved else {
            let mut guard = self
                .resolve_cache
                .lock()
                .map_err(|_| "vyre-frontend-c: include resolve cache poisoned".to_string())?;
            let cache_key =
                ResolveCacheKey::new(cache_key_dir.to_path_buf(), is_system, is_next, path);
            guard.insert(cache_key, None);
            return Err(format!(
                "vyre-frontend-c: include `{name}` not found from {}. Fix: pass the required -I directory or generated header path.",
                from.display()
            ));
        };
        let canon = fs::canonicalize(&resolved).map_err(|error| {
            format!(
                "vyre-frontend-c: include `{name}` resolved to {} but canonicalization failed: {error}. Fix: repair include path permissions or remove broken symlinks before dispatch.",
                resolved.display()
            )
        })?;
        let mut guard = self
            .resolve_cache
            .lock()
            .map_err(|_| "vyre-frontend-c: include resolve cache poisoned".to_string())?;
        let cache_key = ResolveCacheKey::new(cache_key_dir.to_path_buf(), is_system, is_next, path);
        guard.insert(cache_key, Some(canon.clone()));
        let bytes = self.cached_include_bytes(&canon)?;
        Ok(Some((canon, bytes)))
    }
}

fn read_include_bytes(path: &Path) -> Result<Vec<u8>, String> {
    read_include_bounded(path)
}

fn read_include_record(path: &Path) -> Result<(Vec<u8>, ResidentPrepDep), String> {
    let before = fs::metadata(path).map_err(|error| {
        format!(
            "vyre-frontend-c: stat include dependency {} before read: {error}",
            path.display()
        )
    })?;
    let before_modified_ns = include_modified_ns(path, &before)?;
    let bytes = read_include_bytes(path)?;
    let after = fs::metadata(path).map_err(|error| {
        format!(
            "vyre-frontend-c: stat include dependency {} after read: {error}",
            path.display()
        )
    })?;
    let after_modified_ns = include_modified_ns(path, &after)?;
    let content_hash = stable_hash_bytes(&bytes);
    let before_dep = resident_dep_from_metadata(
        path.to_path_buf(),
        &before,
        before_modified_ns,
        content_hash,
    );
    let after_dep =
        resident_dep_from_metadata(path.to_path_buf(), &after, after_modified_ns, content_hash);
    if !same_dependency_metadata(&before_dep, &after_dep) {
        return Err(format!(
            "vyre-frontend-c: include dependency {} changed while being read. Fix: regenerate the header before compiling so resident-prep cache invalidation records a stable dependency.",
            path.display()
        ));
    }
    Ok((bytes, after_dep))
}

pub(super) fn include_modified_ns(path: &Path, metadata: &fs::Metadata) -> Result<u128, String> {
    let modified = metadata.modified().map_err(|error| {
        format!(
            "vyre-frontend-c: read mtime for include dependency {}: {error}. Fix: repair filesystem metadata access before relying on include-cache invalidation.",
            path.display()
        )
    })?;
    modified
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| {
            format!(
                "vyre-frontend-c: include dependency {} has pre-UNIX_EPOCH mtime: {error}. Fix: repair filesystem timestamps before relying on include-cache invalidation.",
                path.display()
            )
        })
        .map(|duration| duration.as_nanos())
}

fn same_dependency_metadata(left: &ResidentPrepDep, right: &ResidentPrepDep) -> bool {
    left.len == right.len
        && left.modified_ns == right.modified_ns
        && left.change_ns == right.change_ns
        && left.dev == right.dev
        && left.ino == right.ino
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;
    use std::sync::{Arc, Mutex};

    use super::super::{resident_dep_from_metadata, stable_hash_bytes};
    use super::{
        include_modified_ns, IncludeSearchDirs, ResidentIncludeDependencyCache,
        ResidentIncludeFileCache, ResidentIncludeLoader, ResidentIncludeResolveCache,
    };

    #[test]
    fn dependency_signature_preserves_entries_evicted_from_file_cache() {
        let tmp = std::env::temp_dir().join(format!(
            "vyre_frontend_c_include_dep_cache_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
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
        let mut file_cache = ResidentIncludeFileCache::with_limits(1, 8);
        file_cache.insert(a.clone(), vec![0; 4], dep_for_path(&a));
        file_cache.insert(b.clone(), vec![0; 4], dep_for_path(&b));
        assert_eq!(file_cache.len(), 1);
        let mut dependency_cache = ResidentIncludeDependencyCache::with_limit(2);
        for path in [&a, &b] {
            let metadata = fs::metadata(path).unwrap();
            dependency_cache
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
        let loader = ResidentIncludeLoader {
            include_dirs: IncludeSearchDirs {
                quote_dirs: Vec::new(),
                user_dirs: Vec::new(),
                system_dirs: Vec::new(),
                after_dirs: Vec::new(),
                include_next_dirs: Vec::new(),
            },
            resolve_cache: Mutex::new(ResidentIncludeResolveCache::with_limit(8)),
            file_cache: Arc::new(Mutex::new(file_cache)),
            dependency_cache: Mutex::new(dependency_cache),
        };
        let deps = loader.dependency_signature().unwrap();
        assert_eq!(deps.len(), 2);
    }

    #[test]
    fn include_file_cache_reuses_valid_bytes_across_loader_instances() {
        let tmp = std::env::temp_dir().join(format!(
            "vyre_frontend_c_shared_include_file_cache_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        match fs::remove_dir_all(&tmp) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => panic!(
                "failed to clean shared include cache test directory {}: {error}",
                tmp.display()
            ),
        }
        fs::create_dir_all(&tmp).unwrap();
        let header = tmp.join("shared.h");
        fs::write(&header, b"#define SHARED 1\n").unwrap();
        let shared_file_cache =
            Arc::new(Mutex::new(ResidentIncludeFileCache::with_limits(8, 1024)));
        let first_loader = test_loader_with_file_cache(Arc::clone(&shared_file_cache));
        let second_loader = test_loader_with_file_cache(Arc::clone(&shared_file_cache));

        let first = first_loader
            .cached_include_bytes(&header)
            .expect("Fix: first loader must read include bytes");
        let second = second_loader
            .cached_include_bytes(&header)
            .expect("Fix: second loader must reuse shared include bytes");

        assert!(Arc::ptr_eq(&first, &second));
        assert_eq!(first_loader.dependency_signature().unwrap().len(), 1);
        assert_eq!(second_loader.dependency_signature().unwrap().len(), 1);
        let stats = shared_file_cache.lock().unwrap().stats();
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.entries, 1);
    }

    #[test]
    fn include_file_cache_rejects_stale_shared_bytes_before_reuse() {
        let tmp = std::env::temp_dir().join(format!(
            "vyre_frontend_c_stale_shared_include_file_cache_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        match fs::remove_dir_all(&tmp) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => panic!(
                "failed to clean stale shared include cache test directory {}: {error}",
                tmp.display()
            ),
        }
        fs::create_dir_all(&tmp).unwrap();
        let header = tmp.join("shared.h");
        fs::write(&header, b"#define SHARED 2\n").unwrap();
        let shared_file_cache =
            Arc::new(Mutex::new(ResidentIncludeFileCache::with_limits(8, 1024)));
        let loader = test_loader_with_file_cache(Arc::clone(&shared_file_cache));
        let mut stale_dep = dep_for_path(&header);
        stale_dep.len = stale_dep.len.saturating_add(1);
        let stale_bytes = Arc::<[u8]>::from(&b"#define SHARED 1\n"[..]);
        shared_file_cache.lock().unwrap().insert(
            header.clone(),
            Arc::clone(&stale_bytes),
            stale_dep,
        );

        let loaded = loader
            .cached_include_bytes(&header)
            .expect("Fix: loader must re-read a stale shared include cache entry");

        assert_eq!(loaded.as_ref(), b"#define SHARED 2\n");
        assert!(
            !Arc::ptr_eq(&loaded, &stale_bytes),
            "stale shared include bytes must be discarded before reuse"
        );
        assert_eq!(loader.dependency_signature().unwrap().len(), 1);
        let stats = shared_file_cache.lock().unwrap().stats();
        assert_eq!(stats.stale_discards, 1);
        assert_eq!(stats.entries, 1);
    }

    fn test_loader_with_file_cache(
        file_cache: Arc<Mutex<ResidentIncludeFileCache>>,
    ) -> ResidentIncludeLoader {
        ResidentIncludeLoader {
            include_dirs: IncludeSearchDirs {
                quote_dirs: Vec::new(),
                user_dirs: Vec::new(),
                system_dirs: Vec::new(),
                after_dirs: Vec::new(),
                include_next_dirs: Vec::new(),
            },
            resolve_cache: Mutex::new(ResidentIncludeResolveCache::with_limit(8)),
            file_cache,
            dependency_cache: Mutex::new(ResidentIncludeDependencyCache::with_limit(8)),
        }
    }

    fn dep_for_path(path: &Path) -> super::ResidentPrepDep {
        let metadata = fs::metadata(path).unwrap();
        resident_dep_from_metadata(
            path.to_path_buf(),
            &metadata,
            include_modified_ns(path, &metadata).unwrap(),
            stable_hash_bytes(&fs::read(path).unwrap()),
        )
    }
}
