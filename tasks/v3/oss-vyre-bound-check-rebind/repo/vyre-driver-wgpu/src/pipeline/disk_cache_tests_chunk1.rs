// Tests for `pipeline_disk_cache.rs`. Split out per audit item #85 to keep the
// parent file focused on production code.
// (allow(missing_docs) moved to enclosing tests module in disk_cache.rs)

use super::*;
use std::sync::Arc;

static ENV_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn cache_key_isolates_wire_from_adapter() {
    // Two different (wire, fingerprint) pairs whose concatenation would
    // collide under a naïve concat hash must still produce different
    // cache keys because the domain separators intervene.
    let cfg = DispatchConfig::default();
    let k1 = wgsl_cache_key(b"ab", "cd", &cfg);
    let k2 = wgsl_cache_key(b"a", "bcd", &cfg);
    assert_ne!(
        k1, k2,
        "wire/adapter boundaries must not collapse into a single blob"
    );

    // Same (wire, fingerprint) pair must be deterministic across calls.
    let k3 = wgsl_cache_key(b"ab", "cd", &cfg);
    assert_eq!(k1, k3);
}

#[test]
fn adapter_change_invalidates_cache_match() {
    // Given the same wire, a different adapter fingerprint must miss.
    let wire = b"some-wire-bytes".as_slice();
    let cfg = DispatchConfig::default();
    let k_a = wgsl_cache_key(wire, "adapter-alpha", &cfg);
    let k_b = wgsl_cache_key(wire, "adapter-beta", &cfg);
    assert_ne!(k_a, k_b);
}

#[test]
fn manual_cache_key_strings_preserve_stable_format() {
    let adapter_info = wgpu::AdapterInfo {
        name: "test-adapter".to_string(),
        vendor: 0x1234,
        device: 0x5678,
        device_type: wgpu::DeviceType::Other,
        driver: "driver".to_string(),
        driver_info: "info".to_string(),
        backend: wgpu::Backend::Vulkan,
    };
    assert_eq!(
        adapter_fingerprint(&adapter_info),
        "Vulkan:00001234:00005678:driver:info"
    );

    let mut config = DispatchConfig::default();
    config.ulp_budget = Some(7);
    config.workgroup_override = Some([8, 16, 32]);
    assert_eq!(
        vyre_driver::pipeline::dispatch_policy_cache_string(&config),
        "ulp=Some(7):wg=Some([8, 16, 32])"
    );
}

#[test]
fn content_digest_rejects_corrupted_payload() {
    use std::io::Write;
    let dir = tempfile::tempdir().unwrap();
    let meta_path = dir.path().join("meta.toml");

    let wgsl = "genuine shader content";
    let cache_key = wgsl_cache_key(b"key123", "fingerprint", &DispatchConfig::default());

    let metadata = DiskPipelineMetadata {
        version: DISK_PIPELINE_CACHE_VERSION,
        cache_key,
        wgsl_bytes: wgsl.len(),
        adapter_fingerprint: metadata_fingerprint("fingerprint"),
        program_abi_version: u32::from(WIRE_FORMAT_VERSION),
        naga_version: std::borrow::Cow::Borrowed(NAGA_VERSION),
        wgsl_lowering_contract: std::borrow::Cow::Borrowed(WGSL_LOWERING_CONTRACT),
        policy: vyre_driver::pipeline::dispatch_policy_cache_string(&DispatchConfig::default()),
        wgsl_blake3: blake3_hex(wgsl.as_bytes()),
    };
    let mut file = std::fs::File::create(&meta_path).unwrap();
    file.write_all(toml::to_string(&metadata).unwrap().as_bytes())
        .unwrap();

    // Exact match -> true
    assert!(wgsl_metadata_matches(
        &meta_path,
        &cache_key,
        wgsl,
        "fingerprint",
        &DispatchConfig::default()
    ));

    // Match length, but corrupted bytes -> false
    let corrupted_wgsl = "genuine shader corpent";
    assert_eq!(corrupted_wgsl.len(), wgsl.len());
    assert!(!wgsl_metadata_matches(
        &meta_path,
        &cache_key,
        corrupted_wgsl,
        "fingerprint",
        &DispatchConfig::default()
    ));
}

#[test]
fn wgsl_cache_key_includes_lowering_contract() {
    let cfg = DispatchConfig::default();
    let digest = b"normalized-program-digest";
    let fingerprint = "Vulkan:00000000:00000000:test:driver";
    let real = wgsl_cache_key(digest, fingerprint, &cfg);

    let mut legacy_hasher = blake3::Hasher::new();
    legacy_hasher.update(b"vyre-pipeline-cache-v7\0norm\0");
    legacy_hasher.update(digest);
    legacy_hasher.update(b"\0adapter\0");
    legacy_hasher.update(fingerprint.as_bytes());
    legacy_hasher.update(b"\0abi\0");
    legacy_hasher.update(&WIRE_FORMAT_VERSION.to_le_bytes());
    legacy_hasher.update(b"\0naga\0");
    legacy_hasher.update(NAGA_VERSION.as_bytes());
    legacy_hasher.update(b"\0policy\0");
    vyre_driver::pipeline::update_dispatch_policy_cache_hash(&mut legacy_hasher, &cfg);

    assert_ne!(
        real,
        *legacy_hasher.finalize().as_bytes(),
        "WGSL cache keys must include the lowering contract so stale lowered shaders cannot survive emitter semantic changes"
    );
}

#[test]
fn wgsl_metadata_rejects_stale_lowering_contract() {
    use std::io::Write;
    let dir = tempfile::tempdir().unwrap();
    let meta_path = dir.path().join("meta.toml");
    let wgsl = "shader";
    let cache_key = wgsl_cache_key(b"key123", "fingerprint", &DispatchConfig::default());
    let metadata = DiskPipelineMetadata {
        version: DISK_PIPELINE_CACHE_VERSION,
        cache_key,
        wgsl_bytes: wgsl.len(),
        adapter_fingerprint: metadata_fingerprint("fingerprint"),
        program_abi_version: u32::from(WIRE_FORMAT_VERSION),
        naga_version: std::borrow::Cow::Borrowed(NAGA_VERSION),
        wgsl_lowering_contract: std::borrow::Cow::Borrowed("old-contract"),
        policy: vyre_driver::pipeline::dispatch_policy_cache_string(&DispatchConfig::default()),
        wgsl_blake3: blake3_hex(wgsl.as_bytes()),
    };
    let mut file = std::fs::File::create(&meta_path).unwrap();
    file.write_all(toml::to_string(&metadata).unwrap().as_bytes())
        .unwrap();

    assert!(
        !wgsl_metadata_matches(
            &meta_path,
            &cache_key,
            wgsl,
            "fingerprint",
            &DispatchConfig::default()
        ),
        "WGSL metadata must reject entries produced under an old lowering contract"
    );
}

#[test]
fn cache_writes_are_durable_on_explicit_flush_not_insert() {
    let _lock = ENV_TEST_LOCK.lock().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("entry.wgsl");
    PENDING_DURABLE_CACHE_FILES
        .get_or_init(|| std::sync::Mutex::new(std::collections::BTreeSet::new()))
        .lock()
        .unwrap()
        .clear();
    write_atomic(&path, b"shader", "test cache data")
        .expect("Fix: cache write must install the entry before explicit flush.");

    let pending = PENDING_DURABLE_CACHE_FILES
        .get()
        .expect("Fix: write_atomic must register cache entries for explicit flush.");
    assert!(
        pending.lock().unwrap().contains(&path),
        "Fix: write_atomic must defer durability work until flush instead of fsyncing every insertion."
    );

    flush_disk_pipeline_cache()
        .expect("Fix: explicit pipeline cache flush must fsync pending writes.");
    assert!(
        !pending.lock().unwrap().contains(&path),
        "Fix: explicit flush must drain successfully fsynced cache entries."
    );
    assert_eq!(
        std::fs::read(&path).unwrap(),
        b"shader",
        "Fix: explicit flush must preserve the installed cache payload."
    );
}

#[test]
fn oversized_pipeline_metadata_is_rejected_before_parse() {
    let dir = tempfile::tempdir().unwrap();
    let meta_path = dir.path().join("oversized.pipeline.toml");
    std::fs::write(
        &meta_path,
        vec![b'a'; MAX_PIPELINE_CACHE_METADATA_BYTES as usize + 1],
    )
    .unwrap();

    assert!(matches!(read_metadata::<CompiledPipelineMetadata>(&meta_path), Err(_)),
        "Fix: oversized compiled-pipeline metadata must be rejected before TOML parsing"
    );
}

#[test]
fn oversized_compiled_pipeline_blob_is_rejected_before_read() {
    let dir = tempfile::tempdir().unwrap();
    let blob_path = dir.path().join("oversized.pipeline.bin");
    let file = File::create(&blob_path).unwrap();
    file.set_len(MAX_COMPILED_PIPELINE_CACHE_BLOB_BYTES + 1)
        .unwrap();
    drop(file);

    let error = read_bounded_bytes(&blob_path, MAX_COMPILED_PIPELINE_CACHE_BLOB_BYTES)
        .expect_err("oversized compiled-pipeline blob must fail before allocation");
    assert_eq!(
        error.kind(),
        std::io::ErrorKind::InvalidData,
        "Fix: oversized compiled-pipeline blobs must return InvalidData, got {error:?}"
    );
}

#[test]
fn stale_compiled_pipeline_adapter_metadata_misses() {
    let _lock = ENV_TEST_LOCK.lock().unwrap();
    let temp = tempfile::tempdir().unwrap();
    let old_cache_root = set_test_disk_pipeline_cache_root(Some(temp.path().to_path_buf()));

    let key = CompiledPipelineCacheKey {
        hash: [7u8; 32],
        adapter_fingerprint: "current-adapter".to_string(),
        cache_key: "stale-adapter-key".to_string(),
        wgsl_blake3: blake3_hex(b"wgsl"),
    };
    let dir = disk_pipeline_cache_dir();
    std::fs::create_dir_all(&dir).unwrap();
    let blob = b"driver-cache-bytes";
    std::fs::write(
        cache_entry_path(&dir, &key.cache_key, ".pipeline.bin"),
        blob,
    )
    .unwrap();
    let metadata = CompiledPipelineMetadata {
        version: DISK_PIPELINE_CACHE_VERSION,
        cache_key: key.hash,
        adapter_fingerprint: metadata_fingerprint("old-adapter"),
        wgsl_blake3: key.wgsl_blake3.clone(),
        program_abi_version: u32::from(WIRE_FORMAT_VERSION),
        naga_version: std::borrow::Cow::Borrowed(NAGA_VERSION),
        blob_bytes: blob.len(),
        blob_blake3: blake3_hex(blob),
    };
    std::fs::write(
        cache_entry_path(&dir, &key.cache_key, ".pipeline.toml"),
        toml::to_string(&metadata).unwrap(),
    )
    .unwrap();

    let result = load_compiled_pipeline_blob(&key)
        .expect("Fix: stale metadata must be a miss; restore this invariant before continuing.");
    assert!(
        result.is_none(),
        "Fix: compiled-pipeline cache must miss when adapter fingerprint metadata is stale"
    );

    set_test_disk_pipeline_cache_root(old_cache_root);
}

#[test]
fn normalized_cache_digest_erases_runtime_storage_lengths() {
    let entry = vec![vyre_foundation::ir::Node::return_()];
    let a = Program::wrapped(
        vec![
            vyre_foundation::ir::BufferDecl::read(
                "haystack",
                0,
                vyre_foundation::ir::DataType::U32,
            )
            .with_count(8),
            vyre_foundation::ir::BufferDecl::output(
                "matches",
                1,
                vyre_foundation::ir::DataType::U32,
            )
            .with_count(8)
            .with_output_byte_range(0..32),
        ],
        [64, 1, 1],
        entry.clone(),
    );
    let b = Program::wrapped(
        vec![
            vyre_foundation::ir::BufferDecl::read(
                "haystack",
                0,
                vyre_foundation::ir::DataType::U32,
            )
            .with_count(1024),
            vyre_foundation::ir::BufferDecl::output(
                "matches",
                1,
                vyre_foundation::ir::DataType::U32,
            )
            .with_count(1024)
            .with_output_byte_range(0..4096),
        ],
        [64, 1, 1],
        entry,
    );

    assert_eq!(
        vyre_driver::pipeline::normalized_program_cache_digest(&a),
        vyre_driver::pipeline::normalized_program_cache_digest(&b),
        "storage buffer lengths must not perturb the compile fingerprint"
    );
}

#[test]
fn early_pipeline_cache_key_preserves_runtime_storage_lengths() {
    let adapter = wgpu::AdapterInfo {
        name: "cache-test".to_string(),
        vendor: 0x10de,
        device: 0x5090,
        device_type: wgpu::DeviceType::DiscreteGpu,
        driver: "driver".to_string(),
        driver_info: "info".to_string(),
        backend: wgpu::Backend::Vulkan,
    };
    let entry = vec![vyre_foundation::ir::Node::return_()];
    let small = Program::wrapped(
        vec![
            vyre_foundation::ir::BufferDecl::read(
                "haystack",
                0,
                vyre_foundation::ir::DataType::U32,
            )
            .with_count(8),
            vyre_foundation::ir::BufferDecl::output(
                "matches",
                1,
                vyre_foundation::ir::DataType::U32,
            )
            .with_count(8)
            .with_output_byte_range(0..32),
        ],
        [64, 1, 1],
        entry.clone(),
    );
    let large = Program::wrapped(
        vec![
            vyre_foundation::ir::BufferDecl::read(
                "haystack",
                0,
                vyre_foundation::ir::DataType::U32,
            )
            .with_count(4096),
            vyre_foundation::ir::BufferDecl::output(
                "matches",
                1,
                vyre_foundation::ir::DataType::U32,
            )
            .with_count(4096)
            .with_output_byte_range(0..16_384),
        ],
        [64, 1, 1],
        entry,
    );

    assert_ne!(
        small.fingerprint(),
        large.fingerprint(),
        "test programs must differ at the raw Program fingerprint layer"
    );
    assert_ne!(
        early_pipeline_cache_key(&small, &adapter, &DispatchConfig::default()),
        early_pipeline_cache_key(&large, &adapter, &DispatchConfig::default()),
        "Fix: in-memory compiled-pipeline artifacts carry binding and output metadata, so shape-distinct Programs must not share an early cache key."
    );
}
