use super::*;

#[test]
fn cuda_release_suite_artifact_proves_real_gpu_macro_workloads() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("Fix: vyre-bench must live under the workspace root");
    let suite_path = workspace.join("release/evidence/benchmarks/cuda-release-suite.json");
    let suite = read_json(&suite_path);
    let matrix =
        read_json(&workspace.join("release/evidence/benchmarks/release-workload-matrix.json"));
    let matrix_families = matrix["families"]
        .as_array()
        .expect("Fix: release workload matrix must list families.")
        .iter()
        .map(|family| json_str(family, "id").to_owned())
        .collect::<std::collections::BTreeSet<_>>();
    let matrix_family_speedups = matrix["families"]
        .as_array()
        .expect("Fix: release workload matrix must list families.")
        .iter()
        .map(|family| {
            (
                json_str(family, "id").to_owned(),
                family["max_cpu_sota_min_speedup_x"].as_f64().unwrap_or(0.0),
            )
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    assert_eq!(
        suite["schema_version"], 2,
        "Fix: CUDA release benchmark suite evidence must use schema v2."
    );
    assert_eq!(
        suite["backend"], "cuda",
        "Fix: CUDA release benchmark suite must be CUDA-bound evidence."
    );
    assert_eq!(
        json_usize(&suite, "family_count"),
        matrix_families.len(),
        "Fix: CUDA release benchmark suite must cover every release workload matrix family."
    );

    let artifacts = suite["artifacts"]
        .as_array()
        .expect("Fix: CUDA release suite must list artifacts.");
    let statuses = suite["artifact_statuses"]
        .as_array()
        .expect("Fix: CUDA release suite must list artifact_statuses.");
    assert_eq!(
        artifacts.len(),
        statuses.len(),
        "Fix: CUDA release suite artifacts and statuses must have one row per workload."
    );

    let mut covered_families = std::collections::BTreeSet::new();
    for status in statuses {
        let path = json_str(status, "path");
        let family_id = json_str(status, "family_id");
        let family_matrix_speedup = *matrix_family_speedups.get(family_id).unwrap_or_else(|| {
            panic!("Fix: CUDA suite family `{family_id}` is absent from release-workload-matrix.")
        });
        assert!(
            artifacts
                .iter()
                .any(|artifact| artifact.as_str() == Some(path)),
            "Fix: CUDA release suite status references `{path}` but artifacts[] does not."
        );
        assert_eq!(
            status["exists"], true,
            "Fix: CUDA workload artifact `{path}` must exist."
        );
        assert!(
            json_usize(status, "bytes") > 16_000,
            "Fix: CUDA workload artifact `{path}` is too small to be real benchmark evidence."
        );
        assert!(
            status["read_error"].is_null(),
            "Fix: CUDA workload artifact `{path}` must be readable."
        );
        assert_eq!(
            json_str(status, "selected_backend"),
            "cuda",
            "Fix: CUDA workload artifact `{path}` status must be CUDA-selected."
        );
        assert!(
            json_str(status, "gpu_model").contains("NVIDIA"),
            "Fix: CUDA workload artifact `{path}` must record NVIDIA GPU provenance."
        );
        assert!(
            json_usize(status, "gpu_memory_total_mib") >= 24 * 1024,
            "Fix: CUDA workload artifact `{path}` must record release-class GPU memory."
        );
        assert!(
            json_usize(status, "min_wall_samples") >= 30
                && json_usize(status, "min_baseline_wall_samples") >= 30,
            "Fix: CUDA workload artifact `{path}` must record at least 30 GPU and baseline timing samples."
        );
        assert!(
            json_usize(status, "case_count") >= 1 && json_usize(status, "failed_count") == 0,
            "Fix: CUDA workload artifact `{path}` must contain at least one passing benchmark case."
        );
        let requires_cpu_sota_100x = status["cpu_sota_100x_required"].as_bool().expect(
            "Fix: CUDA suite status must state whether the 100x CPU-SOTA contract is required.",
        );
        if requires_cpu_sota_100x {
            assert!(
                json_usize(status, "cpu_sota_100x_contract_cases") >= 1
                    && json_usize(status, "cpu_sota_100x_passing_cases")
                        == json_usize(status, "cpu_sota_100x_contract_cases"),
                "Fix: CUDA workload artifact `{path}` must pass every required CPU-SOTA 100x contract case."
            );
        } else {
            assert!(
                family_matrix_speedup >= 10.0,
                "Fix: CUDA workload artifact `{path}` must map to a matrix CPU-SOTA contract of at least 10x."
            );
        }
        assert!(
            status["blockers"].as_array().is_some_and(Vec::is_empty),
            "Fix: CUDA workload artifact `{path}` must not carry blockers."
        );

        let artifact = read_json(&workspace.join(path));
        assert_eq!(
            artifact["schema"], "vyre-bench.result.v1",
            "Fix: `{path}` must be a vyre-bench result artifact."
        );
        assert_eq!(
            artifact["suite"], "release",
            "Fix: `{path}` must be release-suite evidence."
        );
        assert_eq!(
            artifact["selected_backend"], "cuda",
            "Fix: `{path}` must be CUDA evidence."
        );
        assert_eq!(
            artifact["environment"]["has_gpu"], true,
            "Fix: `{path}` must record a live GPU environment."
        );
        assert!(
            artifact["environment"]["features"]
                .as_array()
                .expect("Fix: benchmark environment features must be an array.")
                .iter()
                .any(|feature| feature.as_str() == Some("backend.usable.cuda")),
            "Fix: `{path}` must prove CUDA was usable, not merely linked."
        );
        let cases = artifact["cases"]
            .as_array()
            .expect("Fix: benchmark artifact cases must be an array.");
        assert!(
            !cases.is_empty(),
            "Fix: `{path}` must include benchmark cases."
        );
        for case in cases {
            assert_eq!(
                case["status"], "pass",
                "Fix: `{path}` has a non-passing benchmark case."
            );
            assert_eq!(
                case["backend_id"], "cuda",
                "Fix: `{path}` contains a non-CUDA case."
            );
            assert_eq!(
                case["workload_class"], "Macro",
                "Fix: `{path}` must prove macro workloads, not primitive-only microbenchmarks."
            );
            assert_eq!(
                case["needs_gpu"], true,
                "Fix: `{path}` release cases must require GPU execution."
            );
            assert!(
                case["min_input_bytes"].as_u64().unwrap_or(0) >= 512 * 1024,
                "Fix: `{path}` release cases must use at least 512 KiB input."
            );
            assert!(
                case["performance"]["contract_passed"]
                    .as_bool()
                    .unwrap_or(false),
                "Fix: `{path}` benchmark case failed its performance contract."
            );
            let min_cuda_cpu_sota_speedup = cuda_cpu_sota_min_speedup(case);
            assert!(
                min_cuda_cpu_sota_speedup >= family_matrix_speedup,
                "Fix: `{path}` case contract must be at least as strong as release-workload-matrix family `{family_id}`."
            );
            assert!(
                case["performance"]["speedup_x"].as_f64().unwrap_or(0.0)
                    >= min_cuda_cpu_sota_speedup,
                "Fix: `{path}` benchmark case must prove its CUDA CPU-SOTA speedup contract."
            );
            if requires_cpu_sota_100x {
                assert!(
                    min_cuda_cpu_sota_speedup >= 100.0,
                    "Fix: `{path}` is marked 100x-required but its CUDA CPU-SOTA contract is weaker."
                );
            } else {
                assert!(
                    min_cuda_cpu_sota_speedup >= family_matrix_speedup,
                    "Fix: `{path}` non-required release contract is weaker than release-workload-matrix family `{family_id}`."
                );
            }
            assert!(
                case["performance"]["speedup_x"].as_f64().unwrap_or(0.0) >= 25.0,
                "Fix: `{path}` benchmark case must prove at least the non-100x release floor."
            );
            assert!(
                case["metrics"]["wall_ns"]["samples"].as_u64().unwrap_or(0) >= 30,
                "Fix: `{path}` benchmark case must contain at least 30 wall-clock samples."
            );
        }
        covered_families.insert(json_str(status, "family_id").to_owned());
    }

    assert_eq!(
        covered_families, matrix_families,
        "Fix: CUDA release suite family coverage must match release-workload-matrix exactly."
    );
}

#[test]
fn wgpu_fallback_suite_covers_release_workload_matrix_families() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("Fix: vyre-bench must live under the workspace root");
    let matrix =
        read_json(&workspace.join("release/evidence/benchmarks/release-workload-matrix.json"));
    let matrix_families = matrix["families"]
        .as_array()
        .expect("Fix: release workload matrix must list families.")
        .iter()
        .map(|family| json_str(family, "id").to_owned())
        .collect::<BTreeSet<_>>();
    let suite = read_json(&workspace.join("release/evidence/benchmarks/wgpu-fallback-suite.json"));
    assert_eq!(
        suite["schema_version"], 2,
        "Fix: WGPU fallback suite evidence must use schema v2."
    );
    assert_eq!(
        suite["backend"], "wgpu",
        "Fix: WGPU fallback suite must be WGPU-bound evidence."
    );
    assert_eq!(
        json_usize(&suite, "family_count"),
        matrix_families.len(),
        "Fix: WGPU fallback suite must cover every release workload matrix family."
    );

    let artifacts = suite["artifacts"]
        .as_array()
        .expect("Fix: WGPU fallback suite must list artifacts.");
    let statuses = suite["artifact_statuses"]
        .as_array()
        .expect("Fix: WGPU fallback suite must list artifact_statuses.");
    assert_eq!(
        artifacts.len(),
        statuses.len(),
        "Fix: WGPU fallback suite artifacts and statuses must have one row per workload."
    );

    let mut covered_families = BTreeSet::new();
    for status in statuses {
        let path = json_str(status, "path");
        assert!(
            artifacts
                .iter()
                .any(|artifact| artifact.as_str() == Some(path)),
            "Fix: WGPU fallback suite status references `{path}` but artifacts[] does not."
        );
        assert_eq!(
            status["exists"], true,
            "Fix: WGPU workload artifact `{path}` must exist."
        );
        assert!(
            status["blockers"].as_array().is_some(),
            "Fix: WGPU workload artifact `{path}` status must carry an explicit blockers array."
        );
        let artifact = read_json(&workspace.join(path));
        assert_eq!(
            artifact["schema"], "vyre-bench.result.v1",
            "Fix: `{path}` must be a vyre-bench result artifact."
        );
        assert_eq!(
            artifact["suite"], "release",
            "Fix: `{path}` must be release-suite evidence."
        );
        assert_eq!(
            artifact["selected_backend"], "wgpu",
            "Fix: `{path}` must be WGPU evidence."
        );
        assert!(
            artifact["environment"]["features"]
                .as_array()
                .expect("Fix: benchmark environment features must be an array.")
                .iter()
                .any(|feature| feature.as_str() == Some("backend.usable.wgpu")),
            "Fix: `{path}` must prove WGPU was usable, not merely linked."
        );
        covered_families.insert(json_str(status, "family_id").to_owned());
    }

    assert_eq!(
        covered_families, matrix_families,
        "Fix: WGPU fallback suite family coverage must match release-workload-matrix exactly."
    );
}

fn cuda_cpu_sota_min_speedup(case: &Value) -> f64 {
    case["contract"]["baselines"]
        .as_array()
        .expect("Fix: benchmark case contract baselines must be an array.")
        .iter()
        .filter(|baseline| {
            baseline["class"].as_str() == Some("CpuSota")
                && baseline["backend_ids"]
                    .as_array()
                    .expect("Fix: CPU-SOTA baseline backend_ids must be an array.")
                    .iter()
                    .any(|backend| backend.as_str() == Some("cuda"))
        })
        .filter_map(|baseline| baseline["min_speedup_x"].as_f64())
        .fold(0.0, f64::max)
}

fn read_json(path: &Path) -> Value {
    serde_json::from_str(
        &std::fs::read_to_string(path)
            .unwrap_or_else(|error| panic!("Fix: `{}` must be readable: {error}", path.display())),
    )
    .unwrap_or_else(|error| panic!("Fix: `{}` must be valid JSON: {error}", path.display()))
}

fn json_str<'a>(json: &'a Value, key: &str) -> &'a str {
    json[key]
        .as_str()
        .unwrap_or_else(|| panic!("Fix: JSON field `{key}` must be a string."))
}

fn json_usize(json: &Value, key: &str) -> usize {
    json[key]
        .as_u64()
        .unwrap_or_else(|| panic!("Fix: JSON field `{key}` must be an unsigned integer."))
        .try_into()
        .unwrap_or_else(|_| panic!("Fix: JSON field `{key}` must fit usize."))
}
