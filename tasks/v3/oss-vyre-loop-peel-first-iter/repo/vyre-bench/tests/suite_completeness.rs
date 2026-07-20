//! Suite completeness test.
use vyre_bench::api::suite::SuiteKind;

#[test]
fn test_suite_completeness() {
    let registry = vyre_bench::registry::collect_all();

    // SuiteKind::Smoke
    let smoke_cases: Vec<_> = registry
        .iter()
        .filter(|c| c.active_in_suite(SuiteKind::Smoke))
        .collect();
    // Verify adversarial case is NOT in Smoke
    assert!(!smoke_cases
        .iter()
        .any(|c| c.id().0.starts_with("adversarial.")));
    // Verify foundation is in Smoke
    assert!(smoke_cases
        .iter()
        .any(|c| c.id().0.starts_with("foundation.")));

    // SuiteKind::Adversarial
    let adv_cases: Vec<_> = registry
        .iter()
        .filter(|c| c.active_in_suite(SuiteKind::Adversarial))
        .collect();
    assert!(adv_cases
        .iter()
        .any(|c| c.id().0.starts_with("adversarial.")));

    // SuiteKind::Release
    let release_cases: Vec<_> = registry
        .iter()
        .filter(|c| c.active_in_suite(SuiteKind::Release))
        .collect();
    assert!(release_cases
        .iter()
        .any(|c| c.id().0.starts_with("adversarial.")));
    assert!(release_cases
        .iter()
        .any(|c| c.id().0.starts_with("foundation.")));

    // Verify JSON Report Schema top-level wall_ns (B-5)
    let dummy_report = vyre_bench::report::json::ReportSchema {
        schema: "1.0".to_string(),
        run_id: "test".to_string(),
        suite: "smoke".to_string(),
        selected_backend: Some("test".to_string()),
        backend_profile: None,
        git: std::collections::BTreeMap::new(),
        source_fingerprint: "test-source".to_string(),
        source_tree_fingerprint: "test-source-tree".to_string(),
        environment: vyre_bench::probes::environment::EnvironmentData {
            os: "linux".to_string(),
            architecture: "x86_64".to_string(),
            cpu_model: Some("test-cpu".to_string()),
            cpu_cores: 8,
            has_gpu: true,
            gpu_devices: vec![vyre_bench::probes::environment::GpuDeviceInfo {
                name: "NVIDIA GeForce RTX 5090".to_string(),
                driver_version: "test-driver".to_string(),
                memory_total_mib: Some(32_768),
                compute_capability_major: Some(12),
                compute_capability_minor: Some(0),
            }],
            nvidia_driver_version: Some("test-driver".to_string()),
            nvidia_cuda_version: Some("test-cuda".to_string()),
            features: vec!["gpu.nvidia_smi".to_string()],
        },
        features: vec![],
        summary: vyre_bench::report::json::ReportSummary {
            total_cases: 1,
            passed: 1,
            failed: 0,
            total_time_ns: 0,
            cache_hit_rate: None,
        },
        cases: vec![vyre_bench::report::json::CaseReport {
            id: "test".to_string(),
            workload_fingerprint: "bench-case:test".to_string(),
            name: "test".to_string(),
            owner_crate: "vyre-bench-test".to_string(),
            workload_class: "Micro".to_string(),
            tags: vec![],
            backend_id: Some("test".to_string()),
            device_signature: Some("device-profile-v1:test".to_string()),
            held_out_corpus_id: Some("heldout:bench-case:test".to_string()),
            needs_gpu: false,
            min_vram_bytes: None,
            min_input_bytes: None,
            required_features: vec![],
            status: "passed".to_string(),
            wall_ns: Some(100.0),
            correctness: vyre_bench::api::case::Correctness::Exact,
            contract: None,
            performance: None,
            metrics: std::collections::BTreeMap::new(),
            optimization_passes_applied: vec![],
            artifacts: vec![],
        }],
        blockers: vec![],
    };
    let json_str = vyre_bench::report::json::generate_json_report(&dummy_report).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
    assert!(
        parsed["blockers"].as_array().is_some_and(Vec::is_empty),
        "Top-level JSON must carry an explicit blockers array"
    );
    let cases = parsed["cases"].as_array().unwrap();
    let case_obj = cases[0].as_object().unwrap();
    assert!(
        case_obj.contains_key("wall_ns"),
        "Top-level JSON must contain wall_ns"
    );
    assert!(case_obj.contains_key("id"));
    assert!(case_obj.contains_key("status"));
}
