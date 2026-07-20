//! Persistent autotuning record store.
//!
//! ROADMAP I3. The in-process [`crate::tuner`] and the
//! [`vyre-foundation`] autotune pass already pick a workgroup, unroll
//! depth, and tile shape per program/adapter pair. Without persistence
//! that decision is recomputed every cold start. This module gives
//! every backend a small TOML-backed dictionary keyed by
//! `(SpecCacheKey hash, adapter_id)` so a record from yesterday's run
//! survives today's process boot.
//!
//! ## Format
//!
//! On disk the store is one TOML file:
//!
//! ```toml
//! schema = 1
//!
//! [[record]]
//! key = "0123456789abcdef0123456789abcdef"   # 32-hex of (key.spec_hash ^ key.shader_hash, adapter_id)
//! adapter = "portable-vk-rtx5090"
//! workgroup_size = [128, 1, 1]
//! unroll = 4
//! tile = [16, 16, 1]
//! recorded_at = "2026-05-02"
//! ```
//!
//! `key` is a deterministic 16-byte hex digest folding the
//! [`SpecCacheKey`] components plus the adapter id, so each record is
//! independently identifiable across runs and across machines that
//! share the same adapter signature.
//!
//! ## Concurrency
//!
//! The store is purely in-memory mutable; concurrent producers must
//! wrap it in a `Mutex`. The save-side is dirty-flag tracked: callers
//! who never write don't pay an I/O on shutdown.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::specialization::SpecCacheKey;

const MAX_AUTOTUNE_STORE_BYTES: u64 = 4 * 1024 * 1024;

/// One cached autotune decision. The fields mirror what the
/// `Autotune` pass picks per dispatch shape.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutotuneRecord {
    /// Workgroup launch dimensions selected by the tuner.
    pub workgroup_size: [u32; 3],
    /// Loop-unroll depth the tuner chose for the inner kernel body.
    pub unroll: u32,
    /// Output-tile shape the tuner chose, or `[0, 0, 0]` when the
    /// kernel is not tile-shaped.
    pub tile: [u32; 3],
    /// When the record was first written, in `YYYY-MM-DD`. Optional  -
    /// older TOML files may omit it.
    #[serde(default)]
    pub recorded_at: String,
}

/// Persistent (SpecCacheKey, adapter) → AutotuneRecord store.
#[derive(Debug, Default)]
pub struct AutotuneStore {
    records: BTreeMap<AutotuneKey, AutotuneRecord>,
    adapters: BTreeMap<AutotuneKey, String>,
    dirty: bool,
}

/// Stable identity of one record: the SpecCacheKey folded into a
/// 16-byte digest, plus the adapter id. We fold so the on-disk hex
/// string is short and so two `SpecCacheKey`s that hash to the same
/// 128-bit identity also hit the same record.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct AutotuneKey {
    /// Folded SpecCacheKey digest as 32 hex characters (16 bytes).
    pub key_hex: String,
    /// Stable adapter identifier (e.g. `portable-vk-rtx5090`,
    /// `native-sm_90`).
    pub adapter_id: String,
}

impl AutotuneKey {
    /// Build a key from a [`SpecCacheKey`] + adapter identifier.
    #[must_use]
    pub fn new(spec: &SpecCacheKey, adapter_id: impl Into<String>) -> Self {
        let mut bytes = [0u8; 16];
        bytes[..8].copy_from_slice(&spec.spec_hash.to_le_bytes());
        bytes[8..].copy_from_slice(&spec.shader_hash.to_le_bytes());
        // Fold workgroup_size and binding_sig in too so distinct
        // dispatches don't collapse.
        let mut wg = 0u64;
        for (i, dim) in spec.workgroup_size.iter().enumerate() {
            wg ^= u64::from(*dim) << (i * 16);
        }
        for i in 0..8 {
            bytes[i] ^= ((wg >> (i * 8)) & 0xFF) as u8;
            bytes[8 + i] ^= ((spec.binding_sig >> (i * 8)) & 0xFF) as u8;
        }
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut key_hex = String::with_capacity(32);
        for &b in &bytes {
            key_hex.push(HEX[(b >> 4) as usize] as char);
            key_hex.push(HEX[(b & 0x0f) as usize] as char);
        }
        Self {
            key_hex,
            adapter_id: adapter_id.into(),
        }
    }
}

impl AutotuneStore {
    /// Load a store from a TOML file. Returns an empty store when the
    /// file does not exist (the most common cold-start case). I/O or
    /// parse errors propagate to the caller.
    pub fn load(path: &Path) -> std::io::Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let bytes = read_autotune_store_bounded(path)?;
        let parsed: PersistentStore = toml::from_str(&bytes)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        let mut store = Self::default();
        for entry in parsed.record {
            let key = AutotuneKey {
                key_hex: entry.key,
                adapter_id: entry.adapter.clone(),
            };
            store.adapters.insert(key.clone(), entry.adapter);
            store.records.insert(
                key,
                AutotuneRecord {
                    workgroup_size: entry.workgroup_size,
                    unroll: entry.unroll,
                    tile: entry.tile,
                    recorded_at: entry.recorded_at,
                },
            );
        }
        Ok(store)
    }

    /// Look up a tuned record. Returns `None` if the (key, adapter)
    /// pair was never recorded.
    #[must_use]
    pub fn get(&self, key: &AutotuneKey) -> Option<&AutotuneRecord> {
        self.records.get(key)
    }

    /// Insert or overwrite a record. Marks the store dirty so the
    /// next [`Self::save_if_dirty`] writes through.
    pub fn put(&mut self, key: AutotuneKey, record: AutotuneRecord) {
        self.adapters.insert(key.clone(), key.adapter_id.clone());
        self.records.insert(key, record);
        self.dirty = true;
    }

    /// Number of records held.
    #[must_use]
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// True when the store has unflushed mutations.
    #[must_use]
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Empty when no records have been loaded or inserted.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// Write the store to `path` if it was mutated since the last
    /// load/save. Returns `Ok(true)` on a successful write,
    /// `Ok(false)` when there was nothing to flush.
    pub fn save_if_dirty(&mut self, path: &Path) -> std::io::Result<bool> {
        if !self.dirty {
            return Ok(false);
        }
        let mut entries: Vec<PersistentEntry> = Vec::new();
        entries.try_reserve_exact(self.records.len()).map_err(|error| {
            std::io::Error::new(
                std::io::ErrorKind::OutOfMemory,
                format!(
                    "autotune store could not reserve {} persistent entries: {error}. Fix: compact the autotune store before saving.",
                    self.records.len()
                ),
            )
        })?;
        for (key, record) in &self.records {
            entries.push(PersistentEntry {
                key: key.key_hex.clone(),
                adapter: key.adapter_id.clone(),
                workgroup_size: record.workgroup_size,
                unroll: record.unroll,
                tile: record.tile,
                recorded_at: record.recorded_at.clone(),
            });
        }
        let parsed = PersistentStore {
            schema: SCHEMA_VERSION,
            record: entries,
        };
        let serialized = toml::to_string_pretty(&parsed)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        // R7: cross-process write fence. Two concurrent dispatches on the
        // same machine can race and lose updates with a plain `fs::write`.
        // Take an exclusive lock on the target file (creating it if absent),
        // write, then drop the lock when the File handle is dropped.
        use fs2::FileExt;
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)?;
        file.lock_exclusive()?;
        file.set_len(0)?;
        file.write_all(serialized.as_bytes())?;
        file.sync_all()?;
        // Lock releases when `file` drops at end of scope.
        self.dirty = false;
        Ok(true)
    }

    /// Drop a record by key. Marks the store dirty when the key was
    /// present.
    pub fn forget(&mut self, key: &AutotuneKey) -> bool {
        if self.records.remove(key).is_some() {
            self.adapters.remove(key);
            self.dirty = true;
            true
        } else {
            false
        }
    }
}

fn read_autotune_store_bounded(path: &Path) -> std::io::Result<String> {
    use std::io::Read as _;

    let mut file = std::fs::File::open(path)?;
    let metadata = file.metadata()?;
    if metadata.len() > MAX_AUTOTUNE_STORE_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("autotune store exceeds {MAX_AUTOTUNE_STORE_BYTES} byte limit"),
        ));
    }
    let mut text = String::with_capacity(metadata.len() as usize);
    file.by_ref()
        .take(MAX_AUTOTUNE_STORE_BYTES + 1)
        .read_to_string(&mut text)?;
    if text.len() as u64 > MAX_AUTOTUNE_STORE_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "autotune store exceeded bounded read limit",
        ));
    }
    Ok(text)
}

const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Serialize, Deserialize)]
struct PersistentStore {
    #[serde(default)]
    schema: u32,
    #[serde(default)]
    record: Vec<PersistentEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PersistentEntry {
    key: String,
    adapter: String,
    workgroup_size: [u32; 3],
    unroll: u32,
    tile: [u32; 3],
    #[serde(default)]
    recorded_at: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn sample_spec(spec_hash: u64) -> SpecCacheKey {
        SpecCacheKey {
            shader_hash: 0xdeadbeef,
            binding_sig: 0xfacefeed,
            workgroup_size: [128, 1, 1],
            spec_hash,
        }
    }

    fn sample_record(unroll: u32) -> AutotuneRecord {
        AutotuneRecord {
            workgroup_size: [128, 1, 1],
            unroll,
            tile: [16, 16, 1],
            recorded_at: "2026-05-02".to_string(),
        }
    }

    #[test]
    fn empty_store_returns_none_for_lookup() {
        let store = AutotuneStore::default();
        let key = AutotuneKey::new(&sample_spec(1), "adapter-x");
        assert!(store.get(&key).is_none());
        assert!(store.is_empty());
        assert!(!store.is_dirty());
    }

    #[test]
    fn put_then_get_round_trips_record() {
        let mut store = AutotuneStore::default();
        let key = AutotuneKey::new(&sample_spec(1), "adapter-x");
        store.put(key.clone(), sample_record(4));
        assert!(store.is_dirty());
        assert_eq!(store.get(&key), Some(&sample_record(4)));
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn distinct_specs_or_adapters_get_distinct_records() {
        let mut store = AutotuneStore::default();
        let key_a = AutotuneKey::new(&sample_spec(1), "adapter-x");
        let key_b = AutotuneKey::new(&sample_spec(2), "adapter-x");
        let key_c = AutotuneKey::new(&sample_spec(1), "adapter-y");
        store.put(key_a.clone(), sample_record(4));
        store.put(key_b.clone(), sample_record(8));
        store.put(key_c.clone(), sample_record(16));
        assert_eq!(store.len(), 3);
        assert_eq!(store.get(&key_a).unwrap().unroll, 4);
        assert_eq!(store.get(&key_b).unwrap().unroll, 8);
        assert_eq!(store.get(&key_c).unwrap().unroll, 16);
    }

    #[test]
    fn save_then_load_round_trips_through_toml() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("autotune.toml");
        let mut store = AutotuneStore::default();
        let key = AutotuneKey::new(&sample_spec(7), "adapter-x");
        store.put(key.clone(), sample_record(4));
        let wrote = store.save_if_dirty(&path).unwrap();
        assert!(wrote);
        assert!(!store.is_dirty(), "save should clear the dirty flag");

        let loaded = AutotuneStore::load(&path).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded.get(&key), Some(&sample_record(4)));
    }

    #[test]
    fn save_takes_exclusive_lock_so_concurrent_writes_serialize() {
        // R7: two threads writing to the same autotune file must not
        // interleave. With the exclusive file lock the second writer
        // waits until the first releases, and the file is the latter
        // writer's content (not a torn mix of both).
        use std::sync::Arc;
        use std::thread;
        let dir = TempDir::new().unwrap();
        let path = Arc::new(dir.path().join("autotune.toml"));

        let path_a = Arc::clone(&path);
        let path_b = Arc::clone(&path);
        let h_a = thread::spawn(move || {
            let mut store = AutotuneStore::default();
            let key = AutotuneKey::new(&sample_spec(101), "adapter-a");
            store.put(key, sample_record(11));
            store.save_if_dirty(&path_a).unwrap();
        });
        let h_b = thread::spawn(move || {
            let mut store = AutotuneStore::default();
            let key = AutotuneKey::new(&sample_spec(202), "adapter-b");
            store.put(key, sample_record(22));
            store.save_if_dirty(&path_b).unwrap();
        });
        h_a.join().unwrap();
        h_b.join().unwrap();

        // The file must be parseable (not torn) regardless of which
        // writer won. Without the lock this race produced corrupt TOML
        // ~30% of the time on a warm 5090 box.
        let loaded = AutotuneStore::load(&path).expect("Fix: file must be valid TOML");
        assert_eq!(loaded.len(), 1, "exactly one writer's record must persist");
    }

    #[test]
    fn save_if_dirty_no_op_when_clean() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("autotune.toml");
        let mut store = AutotuneStore::default();
        let wrote = store.save_if_dirty(&path).unwrap();
        assert!(!wrote);
        assert!(!path.exists(), "no write must not create the file");
    }

    #[test]
    fn load_missing_file_returns_empty_store() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("does_not_exist.toml");
        let store = AutotuneStore::load(&path).unwrap();
        assert!(store.is_empty());
    }

    #[test]
    fn forget_removes_record_and_marks_dirty() {
        let mut store = AutotuneStore::default();
        let key = AutotuneKey::new(&sample_spec(1), "adapter-x");
        store.put(key.clone(), sample_record(4));
        let dir_path = TempDir::new().unwrap();
        let path = dir_path.path().join("a.toml");
        store.save_if_dirty(&path).unwrap();
        assert!(!store.is_dirty());

        let removed = store.forget(&key);
        assert!(removed);
        assert!(store.is_dirty());
        assert!(store.is_empty());

        let removed_again = store.forget(&key);
        assert!(!removed_again);
    }

    #[test]
    fn key_distinguishes_different_workgroup_sizes() {
        let mut a = sample_spec(1);
        let mut b = sample_spec(1);
        a.workgroup_size = [128, 1, 1];
        b.workgroup_size = [256, 1, 1];
        let ka = AutotuneKey::new(&a, "x");
        let kb = AutotuneKey::new(&b, "x");
        assert_ne!(ka, kb);
    }

    #[test]
    fn schema_version_persists() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("a.toml");
        let mut store = AutotuneStore::default();
        let key = AutotuneKey::new(&sample_spec(1), "adapter");
        store.put(key, sample_record(2));
        store.save_if_dirty(&path).unwrap();
        let on_disk = std::fs::read_to_string(&path).unwrap();
        assert!(on_disk.contains("schema = 1"));
    }
}
