//! Adversarial break-it tests for the runtime pipeline cache (disk + remote).
//!
//! Every assertion here is the strongest reasonable invariant.  If a test
//! passes trivially the invariant is too weak.  Several tests are
//! intentionally designed to **fail** against the current implementation  -
//! they document bugs that must be fixed before the cache is safe at
//! internet scale.
//!
//! Invariants enforced:
//!   - No torn reads under concurrent `put()` pressure.
//!   - Corrupted / truncated on-disk artifacts are rejected (`None`).
//!   - Disk-full conditions are handled gracefully (no panic).
//!   - Symlinks in the cache root are never followed (anti-traversal).
//!   - Concurrent `put` + `get` interleaving is race-free.
//!   - `LayeredPipelineCache` routes writes to layer-0 and reads fall through.
//!   - `RemoteCache` is feature-gated; this file compiles without `remote`.

#![forbid(unsafe_code)]

use std::path::Path;
use std::process::Command;
use std::sync::{Arc, Barrier};
use std::thread;

use proptest::prelude::*;
use tempfile::TempDir;
use vyre_runtime::pipeline_cache::DiskCache;
use vyre_runtime::{
    InMemoryPipelineCache, LayeredPipelineCache, PipelineCacheStore, PipelineFingerprint,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Deterministic fingerprint from a single seed byte.
fn fp_from_seed(seed: u8) -> PipelineFingerprint {
    let mut bytes = [seed; 32];
    bytes[0] = seed;
    bytes[31] = seed.wrapping_mul(7);
    PipelineFingerprint(bytes)
}

/// Fixed fingerprint for tests that need a stable path name.
fn dead_fp() -> PipelineFingerprint {
    PipelineFingerprint([0u8; 32])
}

// ---------------------------------------------------------------------------
// 1. Atomic rename race  -  100 writers, same fp, different 1 MiB payloads
// ---------------------------------------------------------------------------

#[test]
fn disk_cache_atomic_rename_race_100_threads() {
    let dir = TempDir::new().unwrap();
    let cache = Arc::new(DiskCache::new(dir.path()).unwrap());
    let fp = fp_from_seed(42);

    // 64 KiB payloads  -  large enough that any torn read would have a
    // mismatched length, small enough that the test finishes quickly.
    let payloads: Vec<Vec<u8>> = (0..100).map(|i| vec![i as u8; 64 * 1024]).collect();

    let barrier = Arc::new(Barrier::new(101)); // 100 writers + 1 reader
    let mut handles = vec![];

    // 100 writers race to put different payloads for the same fingerprint.
    for payload in payloads.clone() {
        let cache: Arc<DiskCache> = Arc::clone(&cache);
        let barrier = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            barrier.wait();
            cache.put(fp, payload);
        }));
    }

    // Reader continuously samples the cache while writers are active.
    let cache_reader: Arc<DiskCache> = Arc::clone(&cache);
    let reader = thread::spawn(move || {
        barrier.wait();
        let mut observations = vec![];
        for _ in 0..10_000 {
            if let Some(bytes) = cache_reader.get(&fp) {
                observations.push(bytes);
            }
            // Yield occasionally to increase scheduler interleaving.
            if observations.len() % 1000 == 0 {
                thread::yield_now();
            }
        }
        observations
    });

    for h in handles {
        h.join().expect("writer thread must not panic");
    }
    let observations = reader.join().expect("reader thread must not panic");

    // Strongest invariant: every single observed value is a *complete*
    // payload.  A torn state would be a prefix of one payload concatenated
    // with a suffix of another, giving a length or content mismatch.
    for (idx, obs) in observations.iter().enumerate() {
        assert!(
            payloads.contains(obs),
            "torn read at observation #{idx}: {} bytes do not match any complete payload",
            obs.len()
        );
    }

    // Eventual consistency: after all writers finish, the final value must
    // also be one of the complete payloads (or None if every writer lost
    // its race  -  impossible here, but we allow it).
    if let Some(bytes) = cache.get(&fp) {
        assert!(
            payloads.contains(&bytes),
            "final state is {} bytes, not a complete payload",
            bytes.len()
        );
    }
}

// ---------------------------------------------------------------------------
// 2. Corrupted .bin file rejection (property-based)
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 256,
        failure_persistence: Some(Box::new(
            proptest::test_runner::FileFailurePersistence::Off
        )),
        ..ProptestConfig::default()
    })]

    #[test]
    fn corrupted_bin_file_rejected(
        seed in 0u8..=255,
        partial in prop::collection::vec(0u8..=255, 1..1024)
    ) {
        let dir = TempDir::new().unwrap();
        let cache = DiskCache::new(dir.path()).unwrap();
        let fp = fp_from_seed(seed);
        let path = cache.root().join(format!("{}.bin", fp.hex()));

        // Manually write partial bytes to simulate a crash mid-write or a
        // truncated download.
        std::fs::write(&path, &partial).unwrap();

        // The cache must detect corruption / truncation and return None.
        // Returning the raw bytes would pass a truncated target binary
        // artifact to the driver → undefined behaviour at internet scale.
        let result = cache.get(&fp);
        prop_assert!(
            result.is_none(),
            "corrupted file returned Some({} bytes) instead of None",
            result.map(|v| v.len()).unwrap_or(0)
        );
    }
}

// ---------------------------------------------------------------------------
// 3. Disk-full simulation via 4 KB tmpfs
// ---------------------------------------------------------------------------

/// Attempt to mount a tmpfs of a given size onto a temporary directory.
/// On drop it lazily unmounts so `TempDir` can clean up the empty folder.
struct TmpfsGuard {
    dir: TempDir,
}

impl TmpfsGuard {
    fn with_size_kb(size_kb: usize) -> Result<Self, String> {
        let dir = TempDir::new().map_err(|e| e.to_string())?;

        // Try `sudo -n` first (typical dev / CI environments).
        let sudo_out = Command::new("sudo")
            .args([
                "-n",
                "mount",
                "-t",
                "tmpfs",
                "-o",
                &format!("size={}k", size_kb),
                "tmpfs",
            ])
            .arg(dir.path())
            .output();

        match sudo_out {
            Ok(out) if out.status.success() => return Ok(Self { dir }),
            _ => {}
        }

        // Fallback: direct mount (works when we are already root or in a
        // mount namespace with privileges).
        let direct_out = Command::new("mount")
            .args(["-t", "tmpfs", "-o", &format!("size={}k", size_kb), "tmpfs"])
            .arg(dir.path())
            .output()
            .map_err(|e| format!("failed to spawn mount: {}", e))?;

        if direct_out.status.success() {
            Ok(Self { dir })
        } else {
            let stderr = String::from_utf8_lossy(&direct_out.stderr);
            Err(format!(
                "tmpfs mount failed (tried sudo and direct mount): {}",
                stderr
            ))
        }
    }

    fn path(&self) -> &Path {
        self.dir.path()
    }
}

impl Drop for TmpfsGuard {
    fn drop(&mut self) {
        // Lazy unmount detaches immediately so `TempDir` can `rmdir` the
        // (now empty) mount point.
        let _ = Command::new("sudo")
            .args(["-n", "umount", "-l"])
            .arg(self.dir.path())
            .stderr(std::process::Stdio::null())
            .status();
        let _ = Command::new("umount")
            .args(["-l"])
            .arg(self.dir.path())
            .stderr(std::process::Stdio::null())
            .status();
    }
}

#[test]
fn disk_full_tmpfs_put_no_panic() {
    let guard = TmpfsGuard::with_size_kb(4).expect(
        "this test requires mount privileges to create a 4 KB tmpfs; \
         configure passwordless sudo or run in a mount-capable environment",
    );

    let cache = DiskCache::new(guard.path()).unwrap();
    let fp = fp_from_seed(1);
    let big_artifact = vec![0xABu8; 100 * 1024]; // 100 KB into 4 KB fs

    // Best-effort `put` must not panic.  The write to the temp file will
    // fail with ENOSPC; the implementation silently drops the error.
    cache.put(fp, big_artifact);

    // Because the temp file could not be written, the final rename never
    // happens.  `get()` must therefore return `None`.
    assert!(
        cache.get(&fp).is_none(),
        "get() returned Some after a disk-full put  -  partial artifact must not be served"
    );
}

// ---------------------------------------------------------------------------
// 4. Symlink attack  -  must refuse to follow, must not read /etc/passwd
// ---------------------------------------------------------------------------

#[cfg(unix)]
#[test]
fn symlink_attack_must_not_read_etc_passwd() {
    let dir = TempDir::new().unwrap();
    let cache = DiskCache::new(dir.path()).unwrap();
    let fp = dead_fp();
    let bin_path = cache.root().join(format!("{}.bin", fp.hex()));

    // Use `/etc/passwd` when available (standard on Unix) so the test
    // matches the real attack surface.  Fallback to a synthetic target
    // on stripped-down environments so the code path is still exercised.
    let target: std::path::PathBuf = if Path::new("/etc/passwd").exists() {
        Path::new("/etc/passwd").into()
    } else {
        let fallback = dir.path().join("synthetic_secret");
        std::fs::write(&fallback, b"SYNTHETIC-SECRET-DATA").unwrap();
        fallback
    };

    std::os::unix::fs::symlink(&target, &bin_path).unwrap();

    // `get()` must refuse to follow the symlink and return `None`.
    // If it follows the symlink, it would return the contents of the
    // target file  -  a directory-traversal / information-disclosure
    // vulnerability.
    let result = cache.get(&fp);
    assert!(
        result.is_none(),
        "symlink attack succeeded: get() read {} bytes from {:?} instead of refusing",
        result.as_ref().map(|v| v.len()).unwrap_or(0),
        target
    );
}

#[cfg(not(unix))]
#[test]
fn symlink_attack_not_applicable_on_non_unix() {
    // On non-Unix platforms the symlink attack vector differs; no symlink probe.
    assert!(!cfg!(unix), "symlink adversarial suite is Unix-only");
}

// ---------------------------------------------------------------------------
// 5. Concurrent put + get  -  50 put threads, 50 get threads, same fp set
// ---------------------------------------------------------------------------

#[test]
fn concurrent_put_get_50_50_same_fp_set() {
    let dir = TempDir::new().unwrap();
    let cache = Arc::new(DiskCache::new(dir.path()).unwrap());

    let fps: Vec<PipelineFingerprint> = (0..5).map(fp_from_seed).collect();
    let payloads: Vec<Vec<u8>> = (0..5).map(|i| vec![i as u8; 128 * 1024]).collect();

    let barrier = Arc::new(Barrier::new(101));
    let mut handles = vec![];

    // 50 put threads
    for i in 0..50 {
        let cache: Arc<DiskCache> = Arc::clone(&cache);
        let fps = fps.clone();
        let payloads = payloads.clone();
        let barrier = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            barrier.wait();
            for _ in 0..20 {
                let fp = fps[i % fps.len()];
                let payload = payloads[i % payloads.len()].clone();
                cache.put(fp, payload);
            }
        }));
    }

    // 50 get threads
    for _ in 0..50 {
        let cache: Arc<DiskCache> = Arc::clone(&cache);
        let fps = fps.clone();
        let payloads = payloads.clone();
        let barrier = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            barrier.wait();
            for _ in 0..200 {
                for fp in &fps {
                    if let Some(bytes) = cache.get(fp) {
                        assert!(
                            payloads.contains(&bytes),
                            "torn read: {} bytes not in known payload set",
                            bytes.len()
                        );
                    }
                }
            }
        }));
    }

    barrier.wait();
    for h in handles {
        h.join().expect("thread must not panic");
    }

    // Eventual consistency: every fp must resolve to one of the known
    // complete payloads (or None if no writer ever succeeded  -  unlikely).
    for fp in &fps {
        if let Some(bytes) = cache.get(fp) {
            assert!(
                payloads.contains(&bytes),
                "final state is {} bytes, not a known complete payload",
                bytes.len()
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 6. LayeredPipelineCache  -  DiskCache layer 0, InMemoryPipelineCache layer 1
// ---------------------------------------------------------------------------

#[test]
fn layered_cache_disk_over_memory_fallthrough_and_put_routing() {
    let dir = TempDir::new().unwrap();
    let disk = Arc::new(DiskCache::new(dir.path()).unwrap());
    let mem = Arc::new(InMemoryPipelineCache::new());

    let fp = fp_from_seed(7);
    let artifact = b"layer-1-artifact".to_vec();

    // Pre-populate the lower layer.
    mem.put(fp, artifact.clone());

    let layered = LayeredPipelineCache::new(vec![disk.clone(), mem.clone()]);

    // Miss in disk, fall through to memory.
    assert_eq!(layered.get(&fp).unwrap(), artifact);

    // `put` routes to layer-0 (disk) only.
    let new_artifact = b"layer-0-artifact".to_vec();
    layered.put(fp, new_artifact.clone());
    assert_eq!(disk.get(&fp).unwrap(), new_artifact);

    // Lower layer is untouched by `put`.
    assert_eq!(mem.get(&fp).unwrap(), artifact);
}

// ---------------------------------------------------------------------------
// 7. RemoteCache feature-gated compilation & layered behaviour
// ---------------------------------------------------------------------------

/// Verify that the crate and this test file compile without the `remote`
/// feature.  All `RemoteCache`-dependent code is behind
/// `#[cfg(feature = "remote")]`.
#[test]
fn compiles_without_remote_feature() {
    // Compilation itself is the test  -  the test binary builds iff
    // the default feature set + this file don't reference `remote`.
    let _ = InMemoryPipelineCache::new();
}

#[cfg(feature = "remote")]
#[test]
fn remote_cache_in_layered_cache_put_and_get() {
    let dir = TempDir::new().unwrap();
    let disk = Arc::new(DiskCache::new(dir.path()).unwrap());
    let remote = Arc::new(vyre_runtime::RemoteCache::new("http://127.0.0.1:1"));

    let fp = fp_from_seed(3);

    let layered = LayeredPipelineCache::new(vec![disk.clone(), remote]);

    // `put` must route to the first layer (disk), not remote.
    layered.put(fp, b"only-on-disk".to_vec());
    assert_eq!(disk.get(&fp).unwrap(), b"only-on-disk".to_vec());

    // `get` must hit disk first; remote is never contacted because disk
    // already has the entry.
    assert_eq!(layered.get(&fp).unwrap(), b"only-on-disk".to_vec());
}
