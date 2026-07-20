use super::*;

#[test]
fn write_failure_still_returns_engine() {
    let dir = tempfile::tempdir().expect("tempdir");
    let key = "write-fails";
    let path = engine_cache_path(dir.path(), key).expect("cache_path");

    // Pre-create the expected temp path as a directory so `fs::write` fails.
    let tmp_path = path.with_extension(format!("tmp.{}", std::process::id()));
    std::fs::create_dir(&tmp_path).expect("create tmp dir");

    let mut compiles = 0;
    let engine: GpuLiteralSet = cached_load_or_compile(dir.path(), key, || {
        compiles += 1;
        GpuLiteralSet::compile(&[b"test".as_slice()])
    });
    assert_eq!(compiles, 1, "must compile when write fails");
    assert_eq!(engine.reference_scan(b"test"), vec![Match::new(0, 0, 4)]);
    assert!(
        !path.exists(),
        "cache file must not exist when temp write failed"
    );
}

#[test]
fn tempfile_rename_in_tmp() {
    let dir = tempfile::tempdir_in("/tmp").expect("tempdir in /tmp");
    let key = "tmp-rename";

    let engine: GpuLiteralSet = cached_load_or_compile(dir.path(), key, || {
        GpuLiteralSet::compile(&[b"test".as_slice()])
    });
    assert_eq!(engine.reference_scan(b"test"), vec![Match::new(0, 0, 4)]);

    let mut compiles = 0;
    let _: GpuLiteralSet = cached_load_or_compile(dir.path(), key, || {
        compiles += 1;
        GpuLiteralSet::compile(&[b"test".as_slice()])
    });
    assert_eq!(compiles, 0, "must hit cache when stored in /tmp");
}

#[test]
fn unicode_cache_key_and_dir() {
    let dir = tempfile::tempdir().expect("tempdir");
    let cache_dir = dir.path().join("缓存目录🚀");
    let key = "キー_מפתח_مفتاح";

    let engine: GpuLiteralSet = cached_load_or_compile(&cache_dir, key, || {
        GpuLiteralSet::compile(&[b"unicode".as_slice()])
    });
    assert_eq!(engine.reference_scan(b"unicode"), vec![Match::new(0, 0, 7)]);
    assert!(
        engine_cache_path(&cache_dir, key).unwrap().is_file(),
        "unicode cache file must exist"
    );
}

#[test]
fn cache_dir_is_file_not_directory() {
    let dir = tempfile::tempdir().expect("tempdir");
    let cache_dir = dir.path().join("is-a-file");
    std::fs::write(&cache_dir, b"not a dir").expect("write file");

    let key = "file-dir";
    let mut compiles = 0;
    let engine: GpuLiteralSet = cached_load_or_compile(&cache_dir, key, || {
        compiles += 1;
        GpuLiteralSet::compile(&[b"test".as_slice()])
    });
    assert_eq!(
        compiles, 1,
        "must compile when cache_dir is a file (create_dir_all fails)"
    );
    assert_eq!(engine.reference_scan(b"test"), vec![Match::new(0, 0, 4)]);
}

#[test]
fn cache_key_with_path_separator_does_not_panic() {
    let dir = tempfile::tempdir().expect("tempdir");
    let key = "a/b/c";

    // The helper must not panic even if the cache key contains path
    // separators.  It may or may not successfully write the cache.
    let engine: GpuLiteralSet = cached_load_or_compile(dir.path(), key, || {
        GpuLiteralSet::compile(&[b"test".as_slice()])
    });
    assert_eq!(engine.reference_scan(b"test"), vec![Match::new(0, 0, 4)]);
}

// ---------------------------------------------------------------------------
// 4. Cache-key contract (7 tests)
// ---------------------------------------------------------------------------

#[test]
fn same_patterns_same_cache_key() {
    let a = GpuLiteralSet::compile(&[b"AKIA".as_slice(), b"ghp_".as_slice()]);
    let b = GpuLiteralSet::compile(&[b"AKIA".as_slice(), b"ghp_".as_slice()]);
    assert_eq!(
        MatchScan::cache_key(&a),
        MatchScan::cache_key(&b),
        "identical patterns must yield identical cache keys"
    );
}

#[test]
fn reordering_changes_cache_key() {
    let a = GpuLiteralSet::compile(&[b"first".as_slice(), b"second".as_slice()]);
    let b = GpuLiteralSet::compile(&[b"second".as_slice(), b"first".as_slice()]);
    assert_ne!(
        MatchScan::cache_key(&a),
        MatchScan::cache_key(&b),
        "reordering patterns must change cache key"
    );
}

#[test]
fn removing_pattern_changes_cache_key() {
    let full = GpuLiteralSet::compile(&[b"a".as_slice(), b"b".as_slice(), b"c".as_slice()]);
    let partial = GpuLiteralSet::compile(&[b"a".as_slice(), b"b".as_slice()]);
    assert_ne!(
        MatchScan::cache_key(&full),
        MatchScan::cache_key(&partial),
        "removing a pattern must change cache key"
    );
}

#[test]
fn single_byte_mutation_changes_cache_key() {
    let a = GpuLiteralSet::compile(&[b"AKIA".as_slice()]);
    let b = GpuLiteralSet::compile(&[b"AKIB".as_slice()]);
    assert_ne!(
        MatchScan::cache_key(&a),
        MatchScan::cache_key(&b),
        "single-byte mutation must change cache key"
    );
}

#[test]
fn cross_process_determinism_literal_set_known_constant() {
    // FNV-1a64 over the wire buffer for [b"VYRE"] is deterministic.
    // Computed independently: 6fbbc5c22cb738b9.
    let engine = GpuLiteralSet::compile(&[b"VYRE".as_slice()]);
    let key = MatchScan::cache_key(&engine);
    assert_eq!(
        key, "lit-6fbbc5c22cb738b9",
        "cache key must match known cross-process constant"
    );
}

#[cfg(feature = "matching-nfa")]
#[test]
fn cross_process_determinism_rule_pipeline_stable() {
    let pipe = build_rule_pipeline(&["abc", "de"], "input", "hits", 8);
    let key = MatchScan::cache_key(&pipe);
    let pipe2 = build_rule_pipeline(&["abc", "de"], "input", "hits", 8);
    assert_eq!(
        key,
        MatchScan::cache_key(&pipe2),
        "RulePipeline cache key must be stable across recomputations"
    );
}

#[test]
fn different_engines_different_keys() {
    let literal = GpuLiteralSet::compile(&[b"abc".as_slice()]);
    let literal_key = MatchScan::cache_key(&literal);
    assert!(
        literal_key.starts_with("lit-"),
        "literal key must use lit- prefix"
    );

    #[cfg(feature = "matching-nfa")]
    {
        let pipe = build_rule_pipeline(&["abc"], "input", "hits", 8);
        let pipe_key = MatchScan::cache_key(&pipe);
        assert!(
            pipe_key.starts_with("pipe-"),
            "pipeline key must use pipe- prefix"
        );
        assert_ne!(
            literal_key, pipe_key,
            "different engines must not share cache keys"
        );
    }
}

// ---------------------------------------------------------------------------
// 5. Trait object dispatch (7 tests)
// ---------------------------------------------------------------------------

#[test]
fn box_dyn_match_scan_gpu_literal_set_reference_scan() {
    let engine: Box<dyn MatchScan> = Box::new(GpuLiteralSet::compile(&[b"abc".as_slice()]));
    assert_eq!(engine.reference_scan(b"zabc"), vec![Match::new(0, 1, 4)]);
}

#[cfg(feature = "matching-nfa")]
#[test]
fn box_dyn_match_scan_rule_pipeline_reference_scan() {
    let engine: Box<dyn MatchScan> =
        Box::new(build_rule_pipeline(&["abc", "bc"], "input", "hits", 4));
    let matches = engine.reference_scan(b"zabc");
    assert!(matches.contains(&Match::new(0, 1, 4)));
    assert!(matches.contains(&Match::new(1, 2, 4)));
}

#[test]
fn vec_mixed_engines_reference_scan() {
    let mut engines: Vec<Box<dyn MatchScan>> =
        vec![Box::new(GpuLiteralSet::compile(&[b"abc".as_slice()]))];

    #[cfg(feature = "matching-nfa")]
    {
        engines.push(Box::new(build_rule_pipeline(&["abc"], "input", "hits", 8)));
    }

    for engine in &engines {
        let matches = engine.reference_scan(b"zabc");
        assert!(
            matches
                .iter()
                .any(|m| m.pattern_id == 0 && m.start == 1 && m.end == 4),
            "each engine in Vec<Box<dyn MatchScan>> must find 'abc' in 'zabc': got {matches:?}"
        );
    }
}

#[test]
fn scan_through_dyn_ref() {
    let engine = GpuLiteralSet::compile(&[b"abc".as_slice()]);
    let dyn_ref: &dyn MatchScan = &engine;

    let backend = vyre_driver_wgpu::WgpuBackend::new()
        .expect("Fix: scan_through_dyn_ref requires a live GPU");
    let matches = dyn_ref
        .scan(&backend, b"zabc", 10_000)
        .expect("scan through &dyn MatchScan must succeed");
    assert_eq!(matches, vec![Match::new(0, 1, 4)]);
}

#[test]
fn reference_scan_through_dyn_ref() {
    let engine = GpuLiteralSet::compile(&[b"abc".as_slice()]);
    let dyn_ref: &dyn MatchScan = &engine;
    assert_eq!(
        dyn_ref.reference_scan(b"zabc"),
        vec![Match::new(0, 1, 4)],
        "reference_scan through &dyn MatchScan must work"
    );
}

#[test]
fn cache_key_through_dyn_ref() {
    let engine = GpuLiteralSet::compile(&[b"abc".as_slice()]);
    let dyn_ref: &dyn MatchScan = &engine;
    let key = dyn_ref.cache_key();
    assert!(
        key.starts_with("lit-"),
        "cache_key through &dyn MatchScan must return expected prefix"
    );
}

#[cfg(feature = "matching-nfa")]
#[test]
fn rule_pipeline_scan_through_dyn_ref() {
    let engine = build_rule_pipeline(&["abc", "bc"], "input", "hits", 4);
    let dyn_ref: &dyn MatchScan = &engine;

    let backend = vyre_driver_wgpu::WgpuBackend::new()
        .expect("Fix: rule_pipeline_scan_through_dyn_ref requires a live GPU");
    let matches = dyn_ref
        .scan(&backend, b"zabc", 10_000)
        .expect("scan through &dyn MatchScan must succeed");
    assert!(
        matches.contains(&Match::new(0, 1, 4)),
        "expected abc match at (1,4), got {:?}",
        matches
    );
    assert!(
        matches.contains(&Match::new(1, 2, 4)),
        "expected bc match at (2,4), got {:?}",
        matches
    );
}
