#[test]
fn cache_misses_are_traced_on_fresh_temp_dir() {
    let _lock = ENV_TEST_LOCK.lock().unwrap();

    #[derive(Clone)]
    struct StringWriter(Arc<std::sync::Mutex<String>>);
    impl std::io::Write for StringWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            if let Ok(mut s) = self.0.lock() {
                s.push_str(std::str::from_utf8(buf).unwrap_or_default());
            }
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    let captured = Arc::new(std::sync::Mutex::new(String::new()));
    let writer = StringWriter(captured.clone());
    let subscriber = tracing_subscriber::fmt()
        .with_writer(move || writer.clone())
        .with_level(true)
        .with_target(false)
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);

    let dir = tempfile::tempdir().unwrap();
    let old_cache_root = set_test_disk_pipeline_cache_root(Some(dir.path().to_path_buf()));

    let adapter_info = wgpu::AdapterInfo {
        name: "test-adapter".to_string(),
        vendor: 0x1234,
        device: 0x5678,
        device_type: wgpu::DeviceType::Other,
        driver: "test-driver".to_string(),
        driver_info: "1.0".to_string(),
        backend: wgpu::Backend::Noop,
    };

    let program = Program::wrapped(
        vec![
            vyre_foundation::ir::BufferDecl::output("out", 0, vyre_foundation::ir::DataType::U32)
                .with_count(1),
        ],
        [1, 1, 1],
        vec![vyre_foundation::ir::Node::store(
            "out",
            vyre_foundation::ir::Expr::u32(0),
            vyre_foundation::ir::Expr::u32(42),
        )],
    );

    let enabled_features = crate::runtime::device::EnabledFeatures::default();
    let wgsl = load_or_compile_disk_wgsl(
        &program,
        &adapter_info,
        &DispatchConfig::default(),
        &enabled_features,
    )
    .expect("Fix: lowering must succeed on a trivial program; restore this invariant before continuing.");
    let key = compiled_pipeline_cache_key(&adapter_info, &wgsl);
    let blob = load_compiled_pipeline_blob(&key)
        .expect("Fix: blob load must not error; restore this invariant before continuing.");
    assert!(
        blob.is_none(),
        "fresh temp dir must miss compiled pipeline cache"
    );

    let logs = captured.lock().unwrap();
    assert!(
        logs.contains("WGSL cache miss"),
        "expected WGSL cache miss info log, got:\n{logs}"
    );
    assert!(
        logs.contains("compiled-pipeline cache miss"),
        "expected compiled-pipeline cache miss warn log, got:\n{logs}"
    );

    set_test_disk_pipeline_cache_root(old_cache_root);
}
