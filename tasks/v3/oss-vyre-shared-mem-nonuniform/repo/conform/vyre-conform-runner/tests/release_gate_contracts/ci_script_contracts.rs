use super::*;

#[test]
fn nightly_ci_runs_backend_gates_and_real_conform_subcommands() {
    let script = repo_file("scripts/nightly_ci.sh");
    assert!(
        script.contains("source scripts/lib/cargo_runner.sh") && script.contains("vyre_select_cargo_runner"),
        "Fix: nightly_ci.sh must fall back to cargo under CARGO_BUILD_JOBS-gated execution when cargo_full is absent."
    );

    for required in [
        "nvidia-smi",
        "scripts/check_test_coverage_per_crate.sh",
        "scripts/check_roadmap_status_split.sh",
        "scripts/check_ownership_boundaries.sh",
        "scripts/check_cuda_parity_perf_gate.sh",
        "dispatch --backend",
    ] {
        assert!(
            script.contains(required),
            "Fix: nightly_ci.sh must contain `{required}` so backend gates cannot be silently unchecked."
        );
    }
    assert!(
        !script.contains(" -- run --backend "),
        "Fix: nightly_ci.sh must call the implemented `dispatch` subcommand, not the stale `run` spelling."
    );
    assert!(
        !script.contains("\"\" test"),
        "Fix: nightly_ci.sh must invoke the selected cargo runner, not an empty command string."
    );
    for required_test in [
        "CARGO_BUILD_JOBS=\"${CARGO_BUILD_JOBS:-1}\" \"$CARGO_RUNNER\" test -q -p vyre-reference --test oracle_program_edges",
        "CARGO_BUILD_JOBS=\"${CARGO_BUILD_JOBS:-1}\" \"$CARGO_RUNNER\" test -q -p vyre-reference --test quantized_buffer_contract",
        "CARGO_BUILD_JOBS=\"${CARGO_BUILD_JOBS:-1}\" \"$CARGO_RUNNER\" test -q -p vyre-spec --test invariant_catalog_surface",
        "CARGO_BUILD_JOBS=\"${CARGO_BUILD_JOBS:-1}\" \"$CARGO_RUNNER\" test -q -p vyre-spec --test data_type_layout_matrix",
        "CARGO_BUILD_JOBS=\"${CARGO_BUILD_JOBS:-1}\" \"$CARGO_RUNNER\" test -q -p vyre-spec --test collective_op_contracts",
        "CARGO_BUILD_JOBS=\"${CARGO_BUILD_JOBS:-1}\" \"$CARGO_RUNNER\" test -q -p vyre-macros --test adversarial",
        "CARGO_BUILD_JOBS=\"${CARGO_BUILD_JOBS:-1}\" \"$CARGO_RUNNER\" test -q -p vyre-foundation --test wire_fuzz_infra_contracts",
        "CARGO_BUILD_JOBS=\"${CARGO_BUILD_JOBS:-1}\" \"$CARGO_RUNNER\" test -q -p vyre-foundation --test autodiff_transform_contracts",
        "CARGO_BUILD_JOBS=\"${CARGO_BUILD_JOBS:-1}\" \"$CARGO_RUNNER\" test -q -p vyre-foundation --test collective_ir_contracts",
        "CARGO_BUILD_JOBS=\"${CARGO_BUILD_JOBS:-1}\" \"$CARGO_RUNNER\" test -q -p vyre-libs --test hash_single_source_contracts",
        "CARGO_BUILD_JOBS=\"${CARGO_BUILD_JOBS:-1}\" \"$CARGO_RUNNER\" test -q -p vyre-bench --test release_matrix_contracts",
        "CARGO_BUILD_JOBS=\"${CARGO_BUILD_JOBS:-1}\" \"$CARGO_RUNNER\" test -q -p vyre --test wire_malformed_adversarial",
        "CARGO_BUILD_JOBS=\"${CARGO_BUILD_JOBS:-1}\" \"$CARGO_RUNNER\" test -q -p vyre-self-substrate --test organization_contracts",
        "CARGO_BUILD_JOBS=\"${CARGO_BUILD_JOBS:-1}\" \"$CARGO_RUNNER\" test -q -p vyre-self-substrate --test graph_single_source_contracts",
        "CARGO_BUILD_JOBS=\"${CARGO_BUILD_JOBS:-1}\" \"$CARGO_RUNNER\" test -q -p vyre-self-substrate --test platform_doc_consumer_boundary",
    ] {
        assert!(
            script.contains(required_test),
            "Fix: nightly_ci.sh must run focused release-blocker test `{required_test}`."
        );
    }
}

#[test]
fn metal_macbook_gate_is_scripted_through_env_and_shared_runner() {
    let script = repo_file("scripts/check_metal_macbook.sh");
    let bench_manifest = repo_file("vyre-bench/Cargo.toml");
    let bench_lib = repo_file("vyre-bench/src/lib.rs");
    let bench_cli = repo_file("vyre-bench/src/cli.rs");
    let rust_frontend_manifest = repo_file("vyre-frontend-rust/Cargo.toml");
    let rust_frontend_lexer_dispatch =
        repo_file("vyre-frontend-rust/src/pipeline/lexer_dispatch.rs");
    assert!(
        script.contains("VYRE_MACBOOK_SSH")
            && script.contains("VYRE_MACBOOK_VYRE_ROOT")
            && script.contains("VYRE_MACBOOK_CARGO_TARGET_DIR")
            && script.contains("VYRE_MACBOOK_BENCH_OUTPUT_DIR")
            && script.contains("VYRE_MACBOOK_CONNECT_TIMEOUT"),
        "Fix: Metal MacBook gate must be driven by documented environment variables."
    );
    assert!(
        script.contains("source scripts/lib/cargo_runner.sh")
            && script.contains("vyre_select_cargo_runner")
            && script.contains("\"$CARGO_RUNNER\" test -p vyre-driver-metal")
            && script.contains("VYRE_BACKEND=metal \"$CARGO_RUNNER\" test -p vyre-conform-runner --features gpu")
            && script.contains("\"$CARGO_RUNNER\" build -p vyre-bench")
            && script.contains("foundation.elementwise.add.1m")
            && script.contains("metal-resident-queue-closure.json")
            && script.contains("dataflow.ifds.skewed.queue_closure.1m")
            && script.contains("dataflow_ifds_closure_resident_buffers")
            && script.contains("dataflow_ifds_closure_resident_reset_bytes")
            && script.contains("metal_pipeline_cache_hits")
            && script.contains("metal_pipeline_cache_misses")
            && script.contains("metal_pipeline_cache_miss_empty_cache")
            && script.contains("metal_pipeline_cache_miss_program_changed")
            && script.contains("metal_pipeline_cache_miss_dispatch_policy_changed")
            && script.contains("metal_pipeline_cache_miss_device_or_runtime_changed")
            && script.contains("metal_pipeline_cache_miss_key_absent")
            && script.contains("metal_buffer_allocation_count")
            && script.contains("metal_buffer_allocation_bytes")
            && script.contains("metal_host_to_device_copy_count")
            && script.contains("metal_host_to_device_bytes")
            && script.contains("metal_device_to_host_copy_count")
            && script.contains("metal_device_to_host_bytes")
            && script.contains("metal_output_readback_bytes")
            && script.contains("metal_resident_buffer_count")
            && script.contains("metal_resident_bytes")
            && script.contains("VYRE_ALLOW_FEW_SAMPLES=1 \"$bench_bin\" run")
            && script.contains("--measured-samples 3")
            && script.contains("--measured-samples 1")
            && script.contains("--sample-timeout-secs 60")
            && script.contains("--output \"$output\"")
            && script.contains("--output \"$resident_output\"")
            && script.contains("\"$bench_bin\" validate-report")
            && script.contains("--path \"$output\"")
            && script.contains("--path \"$resident_output\"")
            && script.contains("--total-cases 1")
            && script.contains("--failed 0")
            && script.contains("wgpu-vs-metal.txt")
            && script.contains("wgpu-vs-metal.json")
            && script.contains("cpu-ref-vs-metal.txt")
            && script.contains("cpu-ref-vs-metal.json")
            && script.contains("\"$bench_bin\" compare")
            && script.contains("--baseline \"$bench_output_dir/wgpu.json\"")
            && script.contains("--candidate \"$bench_output_dir/metal.json\"")
            && script.contains("--output \"$comparison_json\"")
            && script.contains("--baseline \"$bench_output_dir/cpu-ref.json\"")
            && script.contains("--output \"$ref_comparison_json\"")
            && script.contains("\"$bench_bin\" validate-comparison")
            && script.contains("--path \"$comparison_json\"")
            && script.contains("--path \"$ref_comparison_json\"")
            && script.contains("--baseline-backend wgpu")
            && script.contains("--baseline-backend cpu-ref")
            && script.contains("--candidate-backend metal")
            && script.contains("--case foundation.elementwise.add.1m")
            && script.contains("compare_exit_code=$compare_status")
            && script.contains("baseline_profile_backend=wgpu")
            && script.contains("baseline_profile_backend=cpu-ref")
            && script.contains("candidate_profile_backend=metal")
            && script.contains("baseline_timing_quality=")
            && script.contains("candidate_timing_quality=")
            && script.contains("grep -q \"compare_exit_code=\" \"$comparison\"")
            && script.contains("\"$bench_bin\" validate-benchmark-bundle")
            && script.contains("--dir \"$bench_output_dir\"")
            && script.contains("bundle-manifest.json")
            && script.contains("--manifest-output \"$bundle_manifest\"")
            && script.contains("--manifest-input \"$bundle_manifest\"")
            && script.contains("\\\"schema\\\": \\\"vyre-bench.bundle.v1\\\"")
            && script.contains("\\\"validator\\\": \\\"vyre-bench validate-benchmark-bundle\\\"")
            && script.contains("\\\"suite\\\": \\\"smoke\\\"")
            && script.contains("\\\"case_id\\\": \\\"foundation.elementwise.add.1m\\\"")
            && script.contains("\\\"baseline_backend\\\": \\\"wgpu\\\"")
            && script.contains("\\\"candidate_backend\\\": \\\"metal\\\"")
            && script.contains("\\\"comparison_pairs\\\"")
            && script.contains("\\\"cpu-ref->metal\\\"")
            && script.contains("\\\"wgpu->metal\\\"")
            && script.contains("\\\"source_fingerprint\\\"")
            && script.contains("\\\"source_tree_fingerprint\\\"")
            && script.contains("\\\"artifact_count\\\": 7")
            && script.contains("\\\"bundle_blake3\\\"")
            && script.contains("\\\"path\\\": \\\"metal.json\\\"")
            && script.contains("\\\"path\\\": \\\"cpu-ref-vs-metal.json\\\""),
        "Fix: Metal MacBook gate must use the shared cargo runner for driver, conformance, and benchmark gates."
    );
    for backend in ["cpu-ref", "wgpu", "metal"] {
        assert!(
            script.contains(&format!("for backend in cpu-ref wgpu metal; do"))
                && script.contains("--backend \"$backend\""),
            "Fix: Metal MacBook benchmark gate must explicitly run smoke coverage for backend `{backend}`."
        );
    }
    assert!(
        bench_manifest.contains("vyre-driver-metal = { workspace = true }")
            && bench_manifest.contains("[target.'cfg(not(target_os = \"macos\"))'.dependencies]")
            && bench_lib.contains("pub fn link_benchmark_backend_registrations()")
            && bench_cli.contains("crate::link_benchmark_backend_registrations();")
            && bench_lib.contains("use vyre_driver_metal as _;")
            && bench_lib.contains("#[cfg(not(target_os = \"macos\"))]\nuse vyre_driver_cuda as _;"),
        "Fix: vyre-bench must link vyre-driver-metal for Mac benchmarking without unconditionally loading CUDA on macOS."
    );
    assert!(
        rust_frontend_manifest.contains("[target.'cfg(not(target_os = \"macos\"))'.dependencies]")
            && rust_frontend_lexer_dispatch
                .contains("#[cfg(not(target_os = \"macos\"))]\nuse vyre_driver_cuda as _;"),
        "Fix: vyre-frontend-rust must not pull cudarc into the Mac benchmark dependency graph."
    );
    assert!(
        script.contains("ssh -o BatchMode=yes -o ConnectTimeout=")
            && script.contains("driver|correctness")
            && script.contains("conformance")
            && script.contains("benchmark")
            && script.contains("all"),
        "Fix: Metal MacBook gate must expose SSH-backed driver, conformance, benchmark, and complete modes."
    );
    assert!(
        !script.contains("cargo_full(workspace)")
            && !script.contains("ssh tt-macbook")
            && !script.contains("cargo test -p vyre-driver-metal"),
        "Fix: Metal MacBook gate must not hardcode hostnames or bypass scripts/lib/cargo_runner.sh."
    );
}

#[test]

fn release_shell_scripts_use_shared_cargo_runner_selection() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("Fix: vyre-conform-runner must stay under the repository conform directory.");
    let helper = repo_file("scripts/lib/cargo_runner.sh");
    assert!(
        helper.contains("vyre_select_cargo_runner")
            && helper.contains("[[ -x ./cargo_full ]]")
            && helper.contains("CARGO_RUNNER=\"cargo\"")
            && helper.contains("CARGO_BUILD_JOBS=\"${CARGO_BUILD_JOBS:-1}\""),
        "Fix: scripts/lib/cargo_runner.sh must centralize cargo_full/cargo fallback with single-job builds."
    );

    for script in shell_scripts_under(root.join("scripts")) {
        let display = script
            .strip_prefix(root)
            .unwrap_or(&script)
            .display()
            .to_string();
        let contents = std::fs::read_to_string(&script).unwrap_or_else(|error| {
            panic!("Fix: shell script `{display}` must be readable: {error}")
        });
        assert!(
            !contents.contains("VYRE_CARGO_RUNNER:-./cargo_full"),
            "Fix: `{display}` must use scripts/lib/cargo_runner.sh instead of hardcoding a brittle ./cargo_full default."
        );
    }
}

