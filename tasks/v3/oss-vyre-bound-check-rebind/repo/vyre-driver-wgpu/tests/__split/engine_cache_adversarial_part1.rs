use super::*;

#[test]
fn cache_recovers_from_truncated_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let key = "truncated";
    let path = engine_cache_path(dir.path(), key).expect("cache_path");

    let mut blob = GpuLiteralSet::compile(&[b"test".as_slice()])
        .to_bytes()
        .expect("encode");
    blob.truncate(12); // magic + version + partial section length
    std::fs::write(&path, &blob).expect("write truncated blob");

    let mut compiles = 0;
    let engine: GpuLiteralSet = cached_load_or_compile(dir.path(), key, || {
        compiles += 1;
        GpuLiteralSet::compile(&[b"test".as_slice()])
    });
    assert_eq!(compiles, 1, "must recompile when cache is truncated");
    assert_eq!(engine.reference_scan(b"test"), vec![Match::new(0, 0, 4)]);
}

#[test]
fn cache_recovers_from_wrong_magic() {
    let dir = tempfile::tempdir().expect("tempdir");
    let key = "bad-magic";
    let path = engine_cache_path(dir.path(), key).expect("cache_path");

    let mut blob = GpuLiteralSet::compile(&[b"test".as_slice()])
        .to_bytes()
        .expect("encode");
    blob[0] = 0xDE;
    blob[1] = 0xAD;
    blob[2] = 0xBE;
    blob[3] = 0xEF;
    std::fs::write(&path, &blob).expect("write bad magic");

    let mut compiles = 0;
    let _: GpuLiteralSet = cached_load_or_compile(dir.path(), key, || {
        compiles += 1;
        GpuLiteralSet::compile(&[b"test".as_slice()])
    });
    assert_eq!(compiles, 1, "must recompile on wrong magic");
}

#[test]
fn cache_recovers_from_wrong_version() {
    let dir = tempfile::tempdir().expect("tempdir");
    let key = "bad-version";
    let path = engine_cache_path(dir.path(), key).expect("cache_path");

    let mut blob = GpuLiteralSet::compile(&[b"test".as_slice()])
        .to_bytes()
        .expect("encode");
    let version = u32::from_le_bytes(blob[4..8].try_into().unwrap());
    blob[4..8].copy_from_slice(&version.wrapping_add(1).to_le_bytes());
    std::fs::write(&path, &blob).expect("write bad version");

    let mut compiles = 0;
    let _: GpuLiteralSet = cached_load_or_compile(dir.path(), key, || {
        compiles += 1;
        GpuLiteralSet::compile(&[b"test".as_slice()])
    });
    assert_eq!(compiles, 1, "must recompile on version mismatch");
}

#[test]
fn cache_recovers_from_valid_magic_garbage_section() {
    let dir = tempfile::tempdir().expect("tempdir");
    let key = "garbage-section";
    let path = engine_cache_path(dir.path(), key).expect("cache_path");

    let mut blob = GpuLiteralSet::compile(&[b"test".as_slice()])
        .to_bytes()
        .expect("encode");
    // Corrupt deep inside the first nested section (after header + length prefix).
    if blob.len() > 20 {
        blob[20] ^= 0xFF;
    }
    std::fs::write(&path, &blob).expect("write garbage section");

    let mut compiles = 0;
    let _: GpuLiteralSet = cached_load_or_compile(dir.path(), key, || {
        compiles += 1;
        GpuLiteralSet::compile(&[b"test".as_slice()])
    });
    assert_eq!(compiles, 1, "must recompile when inner section is corrupt");
}

#[test]
fn cache_recovers_from_directory_instead_of_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let key = "is-a-dir";
    let path = engine_cache_path(dir.path(), key).expect("cache_path");

    std::fs::create_dir(&path).expect("create dir at cache path");

    let mut compiles = 0;
    let _: GpuLiteralSet = cached_load_or_compile(dir.path(), key, || {
        compiles += 1;
        GpuLiteralSet::compile(&[b"test".as_slice()])
    });
    assert_eq!(compiles, 1, "must compile when cache path is a directory");
    assert!(path.is_dir(), "cache path must remain a directory");
}

#[test]
fn cache_recovers_from_zero_length_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let key = "zero-length";
    let path = engine_cache_path(dir.path(), key).expect("cache_path");

    std::fs::write(&path, b"").expect("write empty file");

    let mut compiles = 0;
    let _: GpuLiteralSet = cached_load_or_compile(dir.path(), key, || {
        compiles += 1;
        GpuLiteralSet::compile(&[b"test".as_slice()])
    });
    assert_eq!(compiles, 1, "must recompile on empty cache file");
}

#[test]
fn cache_recovers_from_extra_trailing_garbage() {
    let dir = tempfile::tempdir().expect("tempdir");
    let key = "trailing-garbage";
    let path = engine_cache_path(dir.path(), key).expect("cache_path");

    let mut blob = GpuLiteralSet::compile(&[b"test".as_slice()])
        .to_bytes()
        .expect("encode");
    blob.extend_from_slice(b"EXTRA GARBAGE THAT MUST BE IGNORED OR REJECTED");
    std::fs::write(&path, &blob).expect("write trailing garbage");

    let engine: GpuLiteralSet = cached_load_or_compile(dir.path(), key, || {
        GpuLiteralSet::compile(&[b"test".as_slice()])
    });
    // If the decoder accepted the blob, the engine works. If it rejected,
    // the helper compiled a fresh one. Either way reference_scan must succeed.
    assert_eq!(engine.reference_scan(b"test"), vec![Match::new(0, 0, 4)]);
}

// ---------------------------------------------------------------------------
// 2. Concurrent access (6 tests)
// ---------------------------------------------------------------------------

#[test]
fn concurrent_compile_and_save_one_wins_neither_corrupts() {
    let dir = tempfile::tempdir().expect("tempdir");
    let key = "concurrent-save";
    let path = engine_cache_path(dir.path(), key).expect("cache_path");

    let barrier = Arc::new(Barrier::new(2));
    let dir_path = dir.path().to_path_buf();

    let handles: Vec<_> = (0..2)
        .map(|thread_id| {
            let barrier = Arc::clone(&barrier);
            let dir_path = dir_path.clone();
            thread::spawn(move || {
                barrier.wait();
                let engine: GpuLiteralSet = cached_load_or_compile(&dir_path, key, || {
                    GpuLiteralSet::compile(&[b"concurrent".as_slice()])
                });
                (thread_id, engine)
            })
        })
        .collect();

    let results: Vec<_> = handles
        .into_iter()
        .map(|h| h.join().expect("thread must not panic"))
        .collect();

    for (_, engine) in &results {
        assert_eq!(engine.reference_scan(b"concurrent"), vec![Match::new(0, 0, 10)]);
    }

    assert!(path.is_file(), "final cache must exist as a regular file");
    let bytes = std::fs::read(&path).expect("read final cache");
    let loaded = GpuLiteralSet::from_bytes(&bytes).expect("final cache must not be corrupt");
    assert_eq!(loaded.reference_scan(b"concurrent"), vec![Match::new(0, 0, 10)]);
}

#[test]
fn concurrent_read_while_writing_never_partial() {
    let dir = tempfile::tempdir().expect("tempdir");
    let key = "concurrent-read-write";
    let path = engine_cache_path(dir.path(), key).expect("cache_path");

    // Seed an old valid cache that the writer will replace.
    let old = GpuLiteralSet::compile(&[b"old".as_slice()]);
    std::fs::write(&path, old.to_bytes().expect("encode old")).expect("write old cache");

    let barrier = Arc::new(Barrier::new(2));
    let dir_path = dir.path().to_path_buf();
    let path_for_writer = path.clone();

    let writer = {
        let barrier = Arc::clone(&barrier);
        let dir_path = dir_path.clone();
        thread::spawn(move || {
            barrier.wait();
            // Force a recompile+save by deleting the old cache first.
            let _ = std::fs::remove_file(&path_for_writer);
            let engine: GpuLiteralSet = cached_load_or_compile(&dir_path, key, || {
                GpuLiteralSet::compile(&[b"new_pattern".as_slice()])
            });
            engine
        })
    };

    let reader = {
        let barrier = Arc::clone(&barrier);
        let dir_path = dir_path.clone();
        thread::spawn(move || {
            barrier.wait();
            for _ in 0..50 {
                let engine: GpuLiteralSet = cached_load_or_compile(&dir_path, key, || {
                    GpuLiteralSet::compile(&[b"new_pattern".as_slice()])
                });
                let old_matches = engine.reference_scan(b"old");
                let new_matches = engine.reference_scan(b"new_pattern");
                let is_old = old_matches == vec![Match::new(0, 0, 3)];
                let is_new = new_matches == vec![Match::new(0, 0, 11)];
                assert!(
                    is_old || is_new,
                    "reader got partial or corrupt engine: old={old_matches:?} new={new_matches:?}"
                );
            }
        })
    };

    writer.join().expect("writer must not panic");
    reader.join().expect("reader must not panic");
}

#[test]
fn rapid_recompile_loop_100_iters() {
    let dir = tempfile::tempdir().expect("tempdir");
    let base_key = "rapid-recompile";

    for i in 0..100 {
        let key = format!("{base_key}-{i}");
        let engine: GpuLiteralSet = cached_load_or_compile(dir.path(), &key, || {
            GpuLiteralSet::compile(&[b"rapid".as_slice()])
        });
        assert_eq!(
            engine.reference_scan(b"rapid"),
            vec![Match::new(0, 0, 5)],
            "iteration {i} produced broken engine"
        );
    }
}

#[test]
fn concurrent_stale_delete_and_recompile() {
    let dir = tempfile::tempdir().expect("tempdir");
    let key = "stale-delete";
    let path = engine_cache_path(dir.path(), key).expect("cache_path");

    std::fs::write(&path, b"corrupt-stale").expect("write stale cache");

    let barrier = Arc::new(Barrier::new(2));
    let dir_path = dir.path().to_path_buf();

    let handles: Vec<_> = (0..2)
        .map(|_| {
            let barrier = Arc::clone(&barrier);
            let dir_path = dir_path.clone();
            thread::spawn(move || {
                barrier.wait();
                let engine: GpuLiteralSet = cached_load_or_compile(&dir_path, key, || {
                    GpuLiteralSet::compile(&[b"fresh".as_slice()])
                });
                engine
            })
        })
        .collect();

    for handle in handles {
        let engine = handle.join().expect("thread must not panic");
        assert_eq!(engine.reference_scan(b"fresh"), vec![Match::new(0, 0, 5)]);
    }
}

#[test]
fn concurrent_multiple_readers_during_write() {
    let dir = tempfile::tempdir().expect("tempdir");
    let key = "multi-reader-write";

    let barrier = Arc::new(Barrier::new(4));
    let dir_path = dir.path().to_path_buf();

    let handles: Vec<_> = (0..4)
        .map(|thread_id| {
            let barrier = Arc::clone(&barrier);
            let dir_path = dir_path.clone();
            thread::spawn(move || {
                barrier.wait();
                let engine: GpuLiteralSet = cached_load_or_compile(&dir_path, key, || {
                    GpuLiteralSet::compile(&[b"pattern".as_slice()])
                });
                assert_eq!(
                    engine.reference_scan(b"pattern"),
                    vec![Match::new(0, 0, 7)],
                    "thread {thread_id} got broken engine"
                );
                thread_id
            })
        })
        .collect();

    for handle in handles {
        handle.join().expect("thread must not panic");
    }
}

#[test]
fn concurrent_stress_no_panic_with_many_threads() {
    let dir = tempfile::tempdir().expect("tempdir");
    let key = "stress";
    let n = 16;

    let barrier = Arc::new(Barrier::new(n));
    let dir_path = dir.path().to_path_buf();

    let handles: Vec<_> = (0..n)
        .map(|_| {
            let barrier = Arc::clone(&barrier);
            let dir_path = dir_path.clone();
            thread::spawn(move || {
                barrier.wait();
                let engine: GpuLiteralSet = cached_load_or_compile(&dir_path, key, || {
                    GpuLiteralSet::compile(&[b"stress".as_slice()])
                });
                engine.reference_scan(b"stress")
            })
        })
        .collect();

    for handle in handles {
        let matches = handle.join().expect("thread must not panic");
        assert_eq!(matches, vec![Match::new(0, 0, 6)]);
    }
}

// ---------------------------------------------------------------------------
// 3. Filesystem edge cases (7 tests)
// ---------------------------------------------------------------------------

#[test]
fn cache_dir_created_if_missing() {
    let parent = tempfile::tempdir().expect("tempdir");
    let cache_dir = parent.path().join("deeply/nested/cache/dir");
    assert!(!cache_dir.exists());

    let key = "missing-dir";
    let engine: GpuLiteralSet = cached_load_or_compile(&cache_dir, key, || {
        GpuLiteralSet::compile(&[b"test".as_slice()])
    });
    assert_eq!(engine.reference_scan(b"test"), vec![Match::new(0, 0, 4)]);
    assert!(cache_dir.is_dir(), "cache_dir must be created");
    assert!(
        engine_cache_path(&cache_dir, key).unwrap().is_file(),
        "cache file must exist"
    );
}

#[test]
fn read_only_cache_dir_falls_through_to_compile() {
    let dir = tempfile::tempdir().expect("tempdir");
    let cache_dir = dir.path().join("readonly");
    std::fs::create_dir(&cache_dir).expect("create dir");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&cache_dir).unwrap().permissions();
        perms.set_mode(0o555);
        std::fs::set_permissions(&cache_dir, perms).unwrap();
    }

    let key = "readonly";
    let mut compiles = 0;
    let engine: GpuLiteralSet = cached_load_or_compile(&cache_dir, key, || {
        compiles += 1;
        GpuLiteralSet::compile(&[b"test".as_slice()])
    });
    assert_eq!(
        compiles, 1,
        "must fall through to compile when cache dir is read-only"
    );
    assert_eq!(engine.reference_scan(b"test"), vec![Match::new(0, 0, 4)]);

    // Restore permissions so TempDir drop can clean up.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&cache_dir).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&cache_dir, perms).unwrap();
    }
}

