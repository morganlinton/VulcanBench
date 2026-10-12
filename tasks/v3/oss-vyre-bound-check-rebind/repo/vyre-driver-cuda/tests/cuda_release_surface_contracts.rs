//! CUDA release-surface executable contracts.

use serde_json::Value;
use std::fs;

#[test]
fn cuda_release_surface_exposes_megakernel_speedup_csv_verifier() {
    let source = include_str!("../examples/cuda_release_surface.rs");
    let speedup_gate = include_str!("../src/megakernel_speedup_gate.rs");
    assert!(
        source.contains("--verify-megakernel-speedup-csv")
            && source.contains("--format-resident-graph-speedup-csv")
            && source.contains("--print-megakernel-speedup-csv-header")
            && source.contains("--print-cuda-device-evidence-prefix")
            && source.contains("validate_cuda_megakernel_speedup_evidence_csv")
            && source.contains("format_validated_cuda_resident_graph_session_evidence_csv")
            && source.contains("CudaDeviceHandle::acquire_ordinal")
            && source.contains("MEGAKERNEL_SPEEDUP_EVIDENCE_CSV_HEADER"),
        "Fix: CUDA release surface must expose command-level megakernel speedup CSV verifier/producer/header/device-provenance commands, not only library helpers."
    );
    assert!(
        speedup_gate.contains("resident_borrowed_fallback_dispatches")
            && speedup_gate.contains("borrowed_fallback_dispatches")
            && source.contains("--verify-megakernel-speedup-csv"),
        "Fix: CUDA release speedup evidence must expose resident borrowed-fallback telemetry so host-buffer escape paths cannot pass as native megakernel speedup."
    );
    assert!(
        source.contains("std::process::exit(1)"),
        "Fix: CUDA release verifier failures must exit non-zero so release automation cannot ignore invalid speedup evidence."
    );
}

#[test]
fn cuda_parity_perf_gate_runs_release_path_contracts() {
    let script = include_str!("../../scripts/check_cuda_parity_perf_gate.sh");
    let helper = include_str!("../../scripts/lib/cargo_runner.sh");
    assert!(
        script.contains("nvidia-smi >/dev/null 2>&1")
            && script.contains("do not skip CUDA parity")
            && script.contains("exit 1"),
        "Fix: CUDA parity/perf gate must fail loudly when the NVIDIA GPU probe is misconfigured."
    );
    assert!(
        script.contains("--test \"$test\""),
        "Fix: CUDA parity/perf gate must execute each named contract test through cargo's `--test` integration-test selector."
    );
    for required_test in [
        "capability_contracts",
        "cuda_device_contract",
        "cuda_release_surface_contracts",
        "gpu_elementwise_conformance",
        "megakernel_scale_scheduler_contracts",
        "module_cache_contracts",
    ] {
        assert!(
            script.contains(required_test),
            "Fix: CUDA parity/perf gate must run `{required_test}` on the CUDA release path."
        );
    }
    assert!(
        helper.contains("CARGO_BUILD_JOBS=\"${CARGO_BUILD_JOBS:-1}\""),
        "Fix: CUDA parity/perf gate must default to single-job cargo execution to avoid build OOM."
    );
    assert!(
        script.contains("source scripts/lib/cargo_runner.sh") && script.contains("vyre_select_cargo_runner"),
        "Fix: CUDA parity/perf gate must fall back to cargo under CARGO_BUILD_JOBS=1 when the local cargo_full wrapper is absent."
    );
    assert!(
        script.contains("*gpu_parity*") && script.contains("int4_quantized_gpu_parity"),
        "Fix: CUDA parity/perf gate must document and auto-discover INT4 gpu_parity coverage."
    );
}

#[test]
fn cuda_release_gate_evidence_matches_executable_gate() {
    let evidence: Value = serde_json::from_str(include_str!(
        "../../release/evidence/tests/cuda-release-gate.json"
    ))
    .expect("Fix: CUDA release gate evidence must be valid JSON.");
    let script = include_str!("../../scripts/check_cuda_parity_perf_gate.sh");

    assert_eq!(
        evidence["schema_version"], 1,
        "Fix: CUDA release gate evidence schema drift must be explicit."
    );
    assert_eq!(
        evidence["gate"], "scripts/check_cuda_parity_perf_gate.sh",
        "Fix: CUDA release gate evidence must point at the executable gate."
    );
    assert_eq!(
        evidence["validated_command"], "CARGO_BUILD_JOBS=1 scripts/check_cuda_parity_perf_gate.sh",
        "Fix: CUDA release gate evidence must record the exact validation command."
    );
    assert_eq!(
        evidence["gpu_probe_command"], "nvidia-smi",
        "Fix: CUDA release evidence must require the live NVIDIA probe."
    );
    assert_eq!(
        evidence["gpu_probe_policy"], "fail_loudly",
        "Fix: CUDA release evidence must not encode a silent no-GPU skip path."
    );

    for required_test in evidence["required_cuda_tests"]
        .as_array()
        .expect("Fix: CUDA release evidence must list required_cuda_tests.")
    {
        let test_name = required_test
            .as_str()
            .expect("Fix: required_cuda_tests entries must be strings.");
        assert!(
            script.contains(test_name) && script.contains("--test \"$test\""),
            "Fix: CUDA release gate evidence names `{test_name}`, but the executable gate does not run it through the contract-test loop."
        );
    }

    for gpu_parity_test in evidence["gpu_parity_integration_tests"]
        .as_array()
        .expect("Fix: CUDA release evidence must list gpu_parity_integration_tests.")
    {
        let test_name = gpu_parity_test
            .as_str()
            .expect("Fix: gpu_parity_integration_tests entries must be strings.");
        assert!(
            script.contains("*gpu_parity*") && script.contains("--test"),
            "Fix: CUDA release gate must auto-discover gpu_parity integration tests including `{test_name}`."
        );
        assert!(
            script.contains("int4_quantized_gpu_parity")
                || script.contains("*gpu_parity*"),
            "Fix: INT4 CUDA parity test `{test_name}` must be exercised by the gpu_parity discovery loop."
        );
    }
    let mut evidence_gpu_parity_tests = evidence["gpu_parity_integration_tests"]
        .as_array()
        .expect("Fix: CUDA release evidence must list gpu_parity_integration_tests.")
        .iter()
        .map(|test| {
            test.as_str()
                .expect("Fix: gpu_parity_integration_tests entries must be strings.")
                .to_string()
        })
        .collect::<Vec<_>>();
    evidence_gpu_parity_tests.sort();
    let mut actual_gpu_parity_tests = fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/tests"))
        .expect("Fix: CUDA release evidence contract must be able to enumerate integration tests.")
        .filter_map(|entry| {
            let entry = entry.expect(
                "Fix: CUDA release evidence contract must read every integration test entry.",
            );
            let path = entry.path();
            let file_name = path.file_name()?.to_str()?;
            if !file_name.ends_with(".rs") || !file_name.contains("gpu_parity") {
                return None;
            }
            Some(
                path.file_stem()
                    .expect("Fix: CUDA gpu_parity test files must have a file stem.")
                    .to_str()
                    .expect("Fix: CUDA gpu_parity test names must be UTF-8.")
                    .to_string(),
            )
        })
        .collect::<Vec<_>>();
    actual_gpu_parity_tests.sort();
    assert_eq!(
        evidence_gpu_parity_tests, actual_gpu_parity_tests,
        "Fix: CUDA release gate evidence must list exactly the gpu_parity tests auto-discovered by scripts/check_cuda_parity_perf_gate.sh."
    );

    let assertions = evidence["release_path_assertions"]
        .as_array()
        .expect("Fix: CUDA release evidence must list release_path_assertions.");
    assert!(
        assertions.len() >= 6,
        "Fix: CUDA release evidence must cover device, dispatch, execution, async, megakernel, and cache assertions."
    );
    assert_eq!(
        evidence["last_local_validation"]["result"], "passed",
        "Fix: CUDA release evidence must record the latest local gate result."
    );
}

#[test]
fn cuda_release_path_tracks_resident_borrowed_fallback_telemetry_gate() {
    let instrumentation = include_str!("../src/instrumentation.rs");
    let telemetry = include_str!("../src/backend/telemetry.rs");
    let borrowed = include_str!("../src/backend/resident_dispatch/borrowed.rs");
    let contracts = include_str!("resident_dispatch_contracts.rs");

    assert!(
        instrumentation.contains("VYRE_CUDA_ALLOW_BORROWED_FALLBACK")
            && instrumentation.contains("VYRE_CUDA_RESIDENT_BORROWED_FALLBACK")
            && instrumentation.contains("#[cfg(not(debug_assertions))]"),
        "Fix: CUDA resident borrowed fallback must be refused on release builds unless an explicit allow env is set."
    );
    assert!(
        telemetry.contains("resident_borrowed_fallback_dispatches")
            && telemetry.contains("record_resident_borrowed_fallback_dispatch")
            && telemetry.contains("vyre_cuda_resident_borrowed_fallback_dispatches_total"),
        "Fix: CUDA release telemetry must expose a resident borrowed-fallback counter for perf gates."
    );
    assert!(
        borrowed.contains("record_resident_borrowed_fallback_dispatch"),
        "Fix: resident borrowed fallback must increment telemetry at the single borrowed dispatch entrypoint."
    );
    assert!(
        contracts.contains("release_path_resident_dispatch_keeps_borrowed_fallback_counter_at_zero")
            && contracts.contains("resident_borrowed_fallback_dispatches, 0"),
        "Fix: CUDA release gate contracts must assert the resident borrowed-fallback counter stays zero on native dispatch."
    );
}
