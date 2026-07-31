use super::*;

#[test]
fn concrete_driver_coverage_floors_are_nonzero_release_gates() {
    let script = repo_file("scripts/check_test_coverage_per_crate.sh");

    for crate_name in concrete_driver_crates() {
        assert!(
            floor(&script, &crate_name) > 0,
            "Fix: concrete driver `{crate_name}` must not be exempt from per-crate test coverage."
        );
    }
}

#[test]
fn cuda_parity_gate_documents_int4_gpu_parity_coverage() {
    let script = repo_file("scripts/check_cuda_parity_perf_gate.sh");
    assert!(
        script.contains("check_cuda_parity_perf_gate.sh")
            && script.contains("*gpu_parity*")
            && script.contains("int4_quantized_gpu_parity"),
        "Fix: CUDA parity gate must auto-discover INT4 gpu_parity integration tests."
    );

    let evidence: serde_json::Value =
        serde_json::from_str(&repo_file("release/evidence/tests/cuda-release-gate.json"))
            .expect("Fix: CUDA release gate evidence must be valid JSON.");
    let int4_ops = evidence["int4_conformance_ops"]
        .as_array()
        .expect("Fix: cuda-release-gate.json must list int4_conformance_ops.");
    assert_eq!(
        int4_ops.len(),
        6,
        "Fix: INT4 release gate must enumerate all six harness-backed quant.int4 ops."
    );
    assert!(
        evidence["gpu_parity_integration_tests"]
            .as_array()
            .is_some_and(|tests| tests.iter().any(|test| test == "int4_quantized_gpu_parity")),
        "Fix: cuda-release-gate.json must name int4_quantized_gpu_parity as a gpu_parity integration test."
    );
}


