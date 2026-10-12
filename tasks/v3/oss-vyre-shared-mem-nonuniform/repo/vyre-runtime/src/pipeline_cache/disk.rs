//! Disk-backed pipeline cache. Writes one file per fingerprint under
//! `<root>/<hex>.bin` with a blake3 footer that the reader verifies
//! before returning the payload (covers torn writes, bit-rot, and
//! deliberate tampering).

use std::fs::{self, File};
use std::io::Read;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;

use dashmap::DashMap;

use super::fingerprint::PipelineFingerprint;
use super::metrics::{PipelineCacheCounters, PipelineCacheMetrics};
use super::store::PipelineCacheStore;

/// Disk-backed pipeline cache. Writes one file per fingerprint
/// under `<root>/<hex>.bin`. Reads are stateless; writes are
/// `write + rename` for atomicity. No eviction policy today
/// (user decides)  -  the footprint is bounded by
/// sum(artifact_size × unique_canonical_programs).
#[derive(Debug)]
pub struct DiskCache {
    root: PathBuf,
    pending_flushes: DashMap<PathBuf, ()>,
    metrics: PipelineCacheCounters,
}

/// Crash-durability evidence for disk cache artifacts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DiskCacheDurabilityReport {
    /// Entries installed by rename but not yet explicitly flushed.
    pub pending_flushes: u64,
    /// True when no installed artifacts are waiting on file and parent-dir
    /// fsync evidence.
    pub durable: bool,
}

/// Persistent process-crossing pipeline-cache store.
///
/// This is the default disk-backed store for callers that need compiled
/// pipeline artifacts to survive process restarts.
static DISK_CACHE_TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Alias for the disk-backed pipeline cache store.
pub type PersistentPipelineCacheStore = DiskCache;

// On-disk layout:
//   <payload bytes..>  <32-byte blake3 footer>
// Total file size = payload.len() + 32. Get verifies the footer
// before returning the payload; mismatches or truncated files
// return None so the caller recompiles. Covers torn writes +
// bit-rot + deliberate tampering.
pub(super) const CHECKSUM_LEN: usize = 32;
pub(super) const CHECKSUM_LEN_U64: u64 = 32;
pub(super) const MAX_PIPELINE_BLOB_BYTES: u64 = 64 * 1024 * 1024;
pub(super) const MAX_ENCODED_PIPELINE_BLOB_BYTES: u64 = MAX_PIPELINE_BLOB_BYTES + CHECKSUM_LEN_U64;

impl DiskCache {
    /// Construct a cache rooted at `root`. Creates the directory if
    /// it doesn't exist.
    ///
    /// # Errors
    ///
    /// Returns [`DiskCacheError::Io`] when the directory can't be
    /// created.
    pub fn new(root: impl Into<PathBuf>) -> Result<Self, DiskCacheError> {
        let root = root.into();
        fs::create_dir_all(&root).map_err(DiskCacheError::Io)?;
        Ok(Self {
            root,
            pending_flushes: DashMap::new(),
            metrics: PipelineCacheCounters::default(),
        })
    }

    /// Construct a cache rooted at `~/.cache/vyre/pipelines/` (or
    /// `$XDG_CACHE_HOME/vyre/pipelines/` if set).
    ///
    /// # Errors
    ///
    /// Returns [`DiskCacheError::CacheDirUnknown`] when neither env
    /// var resolves, or [`DiskCacheError::Io`] on mkdir failure.
    pub fn in_user_cache() -> Result<Self, DiskCacheError> {
        let base = std::env::var_os("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| Path::new(&h).join(".cache")))
            .ok_or(DiskCacheError::CacheDirUnknown)?;
        Self::new(base.join("vyre").join("pipelines"))
    }

    /// Root directory this cache operates on.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Snapshot whether installed artifacts have crossed the explicit
    /// durability boundary.
    #[must_use]
    pub fn durability_report(&self) -> DiskCacheDurabilityReport {
        let pending_flushes = match u64::try_from(self.pending_flushes.len()) {
            Ok(pending_flushes) => pending_flushes,
            Err(_) => u64::MAX,
        };
        DiskCacheDurabilityReport {
            pending_flushes,
            durable: pending_flushes == 0,
        }
    }

    fn path_for(&self, fp: &PipelineFingerprint) -> PathBuf {
        self.root.join(cache_file_name(fp))
    }
}

fn cache_file_name(fp: &PipelineFingerprint) -> String {
    let mut file_name = String::with_capacity(68);
    fp.push_hex(&mut file_name);
    file_name.push_str(".bin");
    file_name
}

impl PipelineCacheStore for DiskCache {
    fn get(&self, fp: &PipelineFingerprint) -> Option<Vec<u8>> {
        self.metrics.lookups.fetch_add(1, Ordering::Relaxed);
        let path = self.path_for(fp);
        // FINDING-CACHE-1: reject symlinks before reading. `symlink_metadata`
        // does NOT follow the symlink; regular-file check is strict.
        let Some(meta) = fs::symlink_metadata(&path).ok() else {
            self.metrics.misses.fetch_add(1, Ordering::Relaxed);
            return None;
        };
        if !meta.file_type().is_file() {
            self.metrics.misses.fetch_add(1, Ordering::Relaxed);
            return None;
        }
        if meta.len() > MAX_ENCODED_PIPELINE_BLOB_BYTES {
            self.metrics.misses.fetch_add(1, Ordering::Relaxed);
            return None;
        }
        let Some(file) = File::open(&path).ok() else {
            self.metrics.misses.fetch_add(1, Ordering::Relaxed);
            return None;
        };
        let capacity = usize::try_from(meta.len()).ok()?;
        let result = read_verified_cache_blob_with_capacity(file, capacity);
        if result.is_some() {
            self.metrics.hits.fetch_add(1, Ordering::Relaxed);
        } else {
            self.metrics.misses.fetch_add(1, Ordering::Relaxed);
        }
        result
    }

    fn put(&self, fp: PipelineFingerprint, artifact: Vec<u8>) {
        let tmp_id = DISK_CACHE_TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let mut tmp_name = String::with_capacity(85);
        tmp_name.push('.');
        fp.push_hex(&mut tmp_name);
        tmp_name.push('-');
        append_u64_decimal(&mut tmp_name, tmp_id);
        tmp_name.push_str(".bin.tmp");
        let tmp_path = self.root.join(&tmp_name);

        let mut final_name = String::with_capacity(68);
        fp.push_hex(&mut final_name);
        final_name.push_str(".bin");
        let final_path = self.root.join(&final_name);

        // Write payload + blake3 footer in one shot and install by rename so
        // readers see either the prior complete file or the new complete file.
        // Durability is batched through `flush`; fsyncing every insertion
        // turns steady-state cache population into a storage latency bottleneck.
        let write_rename = || -> io::Result<()> {
            let checksum = ::blake3::hash(&artifact);
            let mut f = File::create(&tmp_path)?;
            f.write_all(&artifact)?;
            f.write_all(checksum.as_bytes())?;
            drop(f);
            // FINDING-CACHE-1: if the final path is a symlink, unlink it
            // first so rename replaces the symlink (not its target).
            if let Ok(meta) = fs::symlink_metadata(&final_path) {
                if meta.file_type().is_symlink() {
                    fs::remove_file(&final_path)?;
                }
            }
            fs::rename(&tmp_path, &final_path)?;
            self.pending_flushes.insert(final_path, ());
            Ok(())
        };
        if write_rename().is_err() {
            self.metrics.rejected_puts.fetch_add(1, Ordering::Relaxed);
            // Best-effort; caller falls back to recompile. Clean up
            // the tmp file so it doesn't accumulate on failure.
            match fs::remove_file(&tmp_path) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => tracing::warn!(
                    tmp_path = %tmp_path.display(),
                    error = %error,
                    "failed to remove temporary disk-cache artifact after rejected put"
                ),
            }
        } else {
            self.metrics.puts.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn flush(&self) -> io::Result<()> {
        self.metrics.flushes.fetch_add(1, Ordering::Relaxed);
        let paths: Vec<PathBuf> = self
            .pending_flushes
            .iter()
            .map(|entry| entry.key().clone())
            .collect();
        self.pending_flushes.clear();
        if let Err(error) = flush_paths(&paths) {
            self.metrics.flush_errors.fetch_add(1, Ordering::Relaxed);
            for path in paths {
                self.pending_flushes.insert(path, ());
            }
            return Err(error);
        }
        Ok(())
    }

    fn metrics(&self) -> PipelineCacheMetrics {
        self.metrics.snapshot(0, 0)
    }
}

fn flush_paths(paths: &[PathBuf]) -> io::Result<()> {
    let mut parents = Vec::with_capacity(paths.len());
    sync_paths_bounded(
        paths,
        File::sync_data,
        "pipeline cache file sync worker panicked",
    )?;
    for path in paths {
        if let Some(parent) = path.parent() {
            parents.push(parent.to_path_buf());
        }
    }
    parents.sort();
    parents.dedup();
    sync_parent_dirs(&parents)?;
    Ok(())
}

#[cfg(unix)]
fn sync_parent_dirs(parents: &[PathBuf]) -> io::Result<()> {
    sync_paths_bounded(
        parents,
        File::sync_all,
        "pipeline cache directory sync worker panicked",
    )
}

#[cfg(not(unix))]
fn sync_parent_dirs(_parents: &[PathBuf]) -> io::Result<()> {
    Ok(())
}

fn sync_paths_bounded(
    paths: &[PathBuf],
    sync: fn(&File) -> io::Result<()>,
    panic_message: &'static str,
) -> io::Result<()> {
    if paths.is_empty() {
        return Ok(());
    }
    let workers = sync_worker_count();
    for chunk in paths.chunks(workers) {
        std::thread::scope(|scope| {
            let mut handles = Vec::with_capacity(chunk.len());
            for path in chunk {
                handles.push(scope.spawn(move || {
                    let file = File::open(path)?;
                    sync(&file)
                }));
            }
            for handle in handles {
                handle
                    .join()
                    .map_err(|_| io::Error::other(panic_message))??;
            }
            Ok::<(), io::Error>(())
        })?;
    }
    Ok(())
}

fn sync_worker_count() -> usize {
    static WORKERS: OnceLock<usize> = OnceLock::new();
    *WORKERS.get_or_init(|| {
        std::thread::available_parallelism()
            .map(usize::from)
            .unwrap_or(1)
            .clamp(1, 16)
    })
}

/// Errors from disk-backed pipeline cache construction / use.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum DiskCacheError {
    /// Neither `$XDG_CACHE_HOME` nor `$HOME` is set.
    #[error(
        "could not resolve a user cache directory  -  set XDG_CACHE_HOME or HOME, or call DiskCache::new() with an explicit path"
    )]
    CacheDirUnknown,
    /// `std::io` failure (mkdir, read, write).
    #[error("disk-cache I/O error: {0}")]
    Io(#[from] io::Error),
}

#[cfg_attr(not(any(test, feature = "remote")), allow(dead_code))]
pub(super) fn read_verified_cache_blob(mut reader: impl Read) -> Option<Vec<u8>> {
    read_verified_cache_blob_with_capacity(&mut reader, 0)
}

fn read_verified_cache_blob_with_capacity(
    mut reader: impl Read,
    capacity: usize,
) -> Option<Vec<u8>> {
    let max_encoded_capacity = usize::try_from(MAX_ENCODED_PIPELINE_BLOB_BYTES).ok()?;
    let mut bytes = Vec::with_capacity(capacity.min(max_encoded_capacity));
    reader
        .by_ref()
        .take(MAX_ENCODED_PIPELINE_BLOB_BYTES + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    verify_cache_blob(bytes)
}

pub(super) fn verify_cache_blob(mut bytes: Vec<u8>) -> Option<Vec<u8>> {
    let byte_len = u64::try_from(bytes.len()).ok()?;
    if byte_len > MAX_ENCODED_PIPELINE_BLOB_BYTES || bytes.len() < CHECKSUM_LEN {
        return None;
    }
    let payload_len = bytes.len() - CHECKSUM_LEN;
    if u64::try_from(payload_len).ok()? > MAX_PIPELINE_BLOB_BYTES {
        return None;
    }
    let (payload, footer) = bytes.split_at(payload_len);
    let expected = ::blake3::hash(payload);
    if footer != expected.as_bytes() {
        return None;
    }
    bytes.truncate(payload_len);
    Some(bytes)
}

fn append_u64_decimal(out: &mut String, mut value: u64) {
    let mut digits = [0u8; 20];
    let mut len = 0usize;
    loop {
        digits[len] = b'0' + (value % 10) as u8;
        len += 1;
        value /= 10;
        if value == 0 {
            break;
        }
    }
    for digit in digits[..len].iter().rev() {
        out.push(char::from(*digit));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline_cache::test_helpers::{tiny_program, unique_u64};

    #[test]
    fn persistent_alias_disk_cache_persists_across_store_reopen() {
        let root = std::env::temp_dir().join(format!(
            "vyre-pipeline-cache-test-{}-{}",
            std::process::id(),
            unique_u64()
        ));
        let fp = PipelineFingerprint::of(&tiny_program());

        let first = DiskCache::new(&root)
            .expect("Fix: test must create disk cache directory; restore temp-dir access.");
        first.put(fp, b"compiled-pipeline".to_vec());
        drop(first);

        let reopened =
            PersistentPipelineCacheStore::new(&root).expect("Fix: disk cache must reopen.");
        assert_eq!(
            reopened.get(&fp).as_deref(),
            Some(&b"compiled-pipeline"[..]),
            "Fix: disk pipeline cache must persist artifacts across process-local store reconstruction"
        );

        std::fs::remove_dir_all(root).expect("Fix: disk cache test root cleanup must succeed");
    }

    #[test]
    fn disk_cache_persists_across_store_reopen() {
        let temp = tempfile::TempDir::new().expect("Fix: tempdir required for disk cache test");
        let fp = PipelineFingerprint::of(&tiny_program());
        {
            let cache = DiskCache::new(temp.path())
                .expect("Fix: disk cache test must create isolated cache root");
            cache.put(fp, b"driver-pipeline-blob".to_vec());
        }
        let reopened =
            DiskCache::new(temp.path()).expect("Fix: disk cache must reopen an existing root");
        assert_eq!(
            reopened.get(&fp),
            Some(b"driver-pipeline-blob".to_vec()),
            "Fix: disk PipelineCacheStore must survive process/backend reconstruction"
        );
    }

    #[test]
    fn disk_cache_flush_is_explicit_durability_boundary() {
        let temp = tempfile::TempDir::new().expect("Fix: tempdir required for disk cache test");
        let fp = PipelineFingerprint::of(&tiny_program());
        let cache = DiskCache::new(temp.path())
            .expect("Fix: disk cache test must create isolated cache root");
        cache.put(fp, b"driver-pipeline-blob".to_vec());
        assert!(
            !cache.pending_flushes.is_empty(),
            "Fix: DiskCache::put must defer fsync work until explicit flush."
        );
        cache
            .flush()
            .expect("Fix: explicit disk cache flush must fsync pending entries.");
        assert!(
            cache.pending_flushes.is_empty(),
            "Fix: explicit disk cache flush must drain pending entries."
        );
        assert_eq!(
            cache.get(&fp),
            Some(b"driver-pipeline-blob".to_vec()),
            "Fix: explicit flush must preserve the installed cache artifact."
        );
    }

    #[test]
    fn cache_blob_verifier_accepts_checksum_footer() {
        let payload = b"compiled-artifact".to_vec();
        let mut encoded = payload.clone();
        encoded.extend_from_slice(::blake3::hash(&payload).as_bytes());

        assert_eq!(verify_cache_blob(encoded), Some(payload));
    }

    #[test]
    fn cache_blob_verifier_rejects_corrupted_footer() {
        let payload = b"compiled-artifact".to_vec();
        let mut encoded = payload;
        encoded.extend_from_slice(&[0xA5; CHECKSUM_LEN]);

        assert!(
            verify_cache_blob(encoded).is_none(),
            "Fix: disk and remote cache readers must reject artifacts whose checksum footer does not match"
        );
    }

    #[test]
    fn cache_blob_reader_rejects_oversized_encoded_blob() {
        let oversized = std::io::repeat(0).take(MAX_ENCODED_PIPELINE_BLOB_BYTES + 1);

        assert!(
            read_verified_cache_blob(oversized).is_none(),
            "Fix: disk and remote cache readers must cap encoded blob bytes before allocation"
        );
    }

    #[test]
    fn disk_cache_durability_report_tracks_pending_flush_boundary() {
        let temp = tempfile::tempdir().expect("Fix: create temp disk cache root");
        let fp = PipelineFingerprint::of(&tiny_program());
        let cache = DiskCache::new(temp.path()).expect("Fix: create disk cache");

        assert_eq!(
            cache.durability_report(),
            DiskCacheDurabilityReport {
                pending_flushes: 0,
                durable: true,
            }
        );

        cache.put(fp, b"driver-pipeline-blob".to_vec());
        let after_put = cache.durability_report();
        assert_eq!(after_put.pending_flushes, 1);
        assert!(
            !after_put.durable,
            "Fix: installed artifacts must not be reported durable before explicit flush"
        );

        cache
            .flush()
            .expect("Fix: disk cache flush must fsync pending file and parent dir");
        assert_eq!(
            cache.durability_report(),
            DiskCacheDurabilityReport {
                pending_flushes: 0,
                durable: true,
            }
        );
    }
}
