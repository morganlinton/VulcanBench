//! B-5  -  Result schema validation.
//!
//! Verifies that every case's output JSON contains the required
//! field set: `id`, `workload_fingerprint`, `status`, `wall_ns`,
//! `correctness`, `metrics`, and `optimization_passes_applied`.

#![allow(clippy::field_reassign_with_default)]
use vyre_bench::api::suite::SuiteKind;
use vyre_bench::runner::{execute_suite, RunConfig};

#[test]
fn test_result_schema_fields() {
    let mut config = RunConfig::default();
    config.warmup_samples = 1;
    config.measured_samples = Some(30);
    config.determinism_runs = 1;
    config.case_ids = vec!["foundation.elementwise.add.1m".to_string()];

    let registry = vyre_bench::registry::collect_all();
    let report = execute_suite(&registry, SuiteKind::Smoke, &config);

    // Serialize and parse back via serde_json to validate schema
    let json =
        vyre_bench::report::json::generate_json_report(&report).expect("Must serialize to JSON");
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

    // Top-level schema fields
    assert!(parsed["schema"].is_string(), "schema field must be present");
    assert!(parsed["run_id"].is_string(), "run_id field must be present");
    assert!(parsed["suite"].is_string(), "suite field must be present");
    assert!(
        parsed["backend_profile"].is_object(),
        "backend_profile field must be populated by benchmark execution"
    );
    assert!(
        parsed["backend_profile"]["backend"].is_string(),
        "backend_profile.backend must be present"
    );
    assert!(
        parsed["backend_profile"]["timing_quality"].is_string(),
        "backend_profile.timing_quality must be present"
    );
    assert!(
        parsed["backend_profile"]["supports_device_timestamps"].is_boolean(),
        "backend_profile.supports_device_timestamps must be present"
    );
    assert!(
        parsed["backend_profile"]["supports_hardware_counters"].is_boolean(),
        "backend_profile.supports_hardware_counters must be present"
    );
    assert!(
        parsed["backend_profile"]["max_workgroup_size"].is_array(),
        "backend_profile.max_workgroup_size must be present"
    );
    assert!(parsed["git"].is_object(), "git field must be present");
    assert!(
        parsed["source_fingerprint"].is_string(),
        "source_fingerprint field must be present"
    );
    assert!(
        parsed["source_tree_fingerprint"].is_string(),
        "source_tree_fingerprint field must be present"
    );
    assert!(
        parsed["environment"].is_object(),
        "environment field must be present"
    );
    assert!(parsed["cases"].is_array(), "cases field must be present");
    assert!(
        parsed["summary"].is_object(),
        "summary field must be present"
    );

    // Per-case schema fields
    let cases = parsed["cases"].as_array().unwrap();
    assert!(!cases.is_empty(), "Should have at least one case");

    for case in cases {
        let obj = case.as_object().unwrap();
        assert!(obj.contains_key("id"), "case must have id");
        assert!(
            obj.contains_key("workload_fingerprint"),
            "case must have workload_fingerprint"
        );
        assert!(obj.contains_key("status"), "case must have status");
        assert!(obj.contains_key("wall_ns"), "case must have wall_ns");
        assert!(
            obj.contains_key("correctness"),
            "case must have correctness"
        );
        assert!(obj.contains_key("metrics"), "case must have metrics");
        assert!(
            obj.contains_key("optimization_passes_applied"),
            "case must have optimization_passes_applied"
        );
        assert!(obj.contains_key("artifacts"), "case must have artifacts");

        // Metrics must include wall_ns as a metric
        if obj["status"].as_str() != Some("failed") {
            let metrics = obj["metrics"].as_object().unwrap();
            assert!(
                metrics.contains_key("wall_ns"),
                "case metrics must include wall_ns"
            );
        }
    }

    // Summary schema
    let summary = parsed["summary"].as_object().unwrap();
    assert!(
        summary.contains_key("total_cases"),
        "summary must have total_cases"
    );
    assert!(summary.contains_key("passed"), "summary must have passed");
    assert!(summary.contains_key("failed"), "summary must have failed");
    assert!(
        summary.contains_key("total_time_ns"),
        "summary must have total_time_ns"
    );
}
