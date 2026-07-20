#!/usr/bin/env bash
set -euo pipefail

usage() {
    printf '%s\n' \
        'Usage: scripts/check_metal_macbook.sh [driver|conformance|benchmark|all]' \
        '' \
        'Required environment:' \
        '  VYRE_MACBOOK_SSH       SSH target for the Apple GPU host.' \
        '  VYRE_MACBOOK_VYRE_ROOT Vyre workspace path on the Apple GPU host.' \
        '' \
        'Optional environment:' \
        '  VYRE_MACBOOK_CARGO_TARGET_DIR Remote cargo target directory.' \
        '  VYRE_MACBOOK_BENCH_OUTPUT_DIR Remote directory for benchmark smoke JSON reports.' \
        '  VYRE_MACBOOK_CONNECT_TIMEOUT  SSH connect timeout in seconds, default 8.' \
        '  VYRE_CARGO_RUNNER             Remote cargo runner override consumed by scripts/lib/cargo_runner.sh.' \
        '' \
        'Examples:' \
        '  VYRE_MACBOOK_SSH=tt-macbook \' \
        '  VYRE_MACBOOK_VYRE_ROOT=/Users/thiruthangarathinam/Santh/libs/performance/matching/vyre \' \
        '  scripts/check_metal_macbook.sh conformance' >&2
}

mode="${1:-all}"
if [[ "${mode}" == "-h" || "${mode}" == "--help" ]]; then
    usage
    exit 0
fi

: "${VYRE_MACBOOK_SSH:?Fix: set VYRE_MACBOOK_SSH to the Apple GPU SSH target.}"
: "${VYRE_MACBOOK_VYRE_ROOT:?Fix: set VYRE_MACBOOK_VYRE_ROOT to the Vyre workspace path on the Apple GPU host.}"

remote_quote() {
    printf '%q' "$1"
}

run_remote() {
    local command="$1"
    local root
    root="$(remote_quote "${VYRE_MACBOOK_VYRE_ROOT}")"
    local setup="set -euo pipefail; cd ${root}; source scripts/lib/cargo_runner.sh; vyre_select_cargo_runner; export CARGO_BUILD_JOBS=\"\${CARGO_BUILD_JOBS:-1}\";"
    if [[ -n "${VYRE_MACBOOK_CARGO_TARGET_DIR:-}" ]]; then
        local target
        target="$(remote_quote "${VYRE_MACBOOK_CARGO_TARGET_DIR}")"
        setup="${setup} export CARGO_TARGET_DIR=${target};"
    fi
    if [[ -n "${VYRE_MACBOOK_BENCH_OUTPUT_DIR:-}" ]]; then
        local bench_output
        bench_output="$(remote_quote "${VYRE_MACBOOK_BENCH_OUTPUT_DIR}")"
        setup="${setup} export VYRE_MACBOOK_BENCH_OUTPUT_DIR=${bench_output};"
    fi
    ssh -o BatchMode=yes -o ConnectTimeout="${VYRE_MACBOOK_CONNECT_TIMEOUT:-8}" \
        "${VYRE_MACBOOK_SSH}" \
        "${setup} ${command}"
}

run_driver() {
    echo "metal-macbook: running native driver gate" >&2
    run_remote '"$CARGO_RUNNER" test -p vyre-driver-metal'
}

run_conformance() {
    echo "metal-macbook: running Metal conformance gate" >&2
    run_remote 'VYRE_BACKEND=metal "$CARGO_RUNNER" test -p vyre-conform-runner --features gpu'
}

run_benchmark() {
    echo "metal-macbook: running native Metal/WGPU/reference benchmark gate" >&2
    run_remote '
        "$CARGO_RUNNER" build -p vyre-bench
        bench_bin="${CARGO_TARGET_DIR:-target}/debug/vyre-bench"
        bench_output_dir="${VYRE_MACBOOK_BENCH_OUTPUT_DIR:-${CARGO_TARGET_DIR:-target}/vyre-metal-benchmark-smoke}"
        mkdir -p "$bench_output_dir"
        "$bench_bin" list --format json >/dev/null
        for backend in cpu-ref wgpu metal; do
            output="$bench_output_dir/${backend}.json"
            VYRE_ALLOW_FEW_SAMPLES=1 "$bench_bin" run \
                --suite smoke \
                --format json \
                --backend "$backend" \
                --case foundation.elementwise.add.1m \
                --warmup-samples 0 \
                --measured-samples 3 \
                --sample-timeout-secs 30 \
                --determinism-runs 1 \
                --output "$output" >/dev/null
            test -s "$output"
            "$bench_bin" validate-report \
                --path "$output" \
                --backend "$backend" \
                --total-cases 1 \
                --failed 0 >/dev/null
        done
        grep -q "\"metal_pipeline_cache_hits\"" "$bench_output_dir/metal.json"
        grep -q "\"metal_pipeline_cache_misses\"" "$bench_output_dir/metal.json"
        grep -q "\"metal_pipeline_cache_miss_empty_cache\"" "$bench_output_dir/metal.json"
        grep -q "\"metal_pipeline_cache_miss_program_changed\"" "$bench_output_dir/metal.json"
        grep -q "\"metal_pipeline_cache_miss_dispatch_policy_changed\"" "$bench_output_dir/metal.json"
        grep -q "\"metal_pipeline_cache_miss_device_or_runtime_changed\"" "$bench_output_dir/metal.json"
        grep -q "\"metal_pipeline_cache_miss_key_absent\"" "$bench_output_dir/metal.json"
        grep -q "\"metal_buffer_allocation_count\"" "$bench_output_dir/metal.json"
        grep -q "\"metal_buffer_allocation_bytes\"" "$bench_output_dir/metal.json"
        grep -q "\"metal_host_to_device_copy_count\"" "$bench_output_dir/metal.json"
        grep -q "\"metal_host_to_device_bytes\"" "$bench_output_dir/metal.json"
        grep -q "\"metal_device_to_host_copy_count\"" "$bench_output_dir/metal.json"
        grep -q "\"metal_device_to_host_bytes\"" "$bench_output_dir/metal.json"
        grep -q "\"metal_output_readback_bytes\"" "$bench_output_dir/metal.json"
        grep -q "\"metal_resident_buffer_count\"" "$bench_output_dir/metal.json"
        grep -q "\"metal_resident_bytes\"" "$bench_output_dir/metal.json"
        resident_output="$bench_output_dir/metal-resident-queue-closure.json"
        VYRE_ALLOW_FEW_SAMPLES=1 "$bench_bin" run \
            --suite smoke \
            --format json \
            --backend metal \
            --case dataflow.ifds.skewed.queue_closure.1m \
            --warmup-samples 0 \
            --measured-samples 1 \
            --sample-timeout-secs 60 \
            --determinism-runs 1 \
            --output "$resident_output" >/dev/null
        test -s "$resident_output"
        "$bench_bin" validate-report \
            --path "$resident_output" \
            --backend metal \
            --total-cases 1 \
            --failed 0 >/dev/null
        grep -q "dataflow.ifds.skewed.queue_closure.1m" "$resident_output"
        grep -q "\"dataflow_ifds_closure_resident_buffers\"" "$resident_output"
        grep -q "\"dataflow_ifds_closure_resident_reset_bytes\"" "$resident_output"
        grep -q "\"metal_pipeline_cache_hits\"" "$resident_output"
        grep -q "\"metal_pipeline_cache_misses\"" "$resident_output"
        grep -q "\"metal_pipeline_cache_miss_empty_cache\"" "$resident_output"
        grep -q "\"metal_pipeline_cache_miss_program_changed\"" "$resident_output"
        grep -q "\"metal_pipeline_cache_miss_dispatch_policy_changed\"" "$resident_output"
        grep -q "\"metal_pipeline_cache_miss_device_or_runtime_changed\"" "$resident_output"
        grep -q "\"metal_pipeline_cache_miss_key_absent\"" "$resident_output"
        grep -q "\"metal_buffer_allocation_count\"" "$resident_output"
        grep -q "\"metal_buffer_allocation_bytes\"" "$resident_output"
        grep -q "\"metal_host_to_device_copy_count\"" "$resident_output"
        grep -q "\"metal_host_to_device_bytes\"" "$resident_output"
        grep -q "\"metal_device_to_host_copy_count\"" "$resident_output"
        grep -q "\"metal_device_to_host_bytes\"" "$resident_output"
        grep -q "\"metal_output_readback_bytes\"" "$resident_output"
        grep -q "\"metal_resident_buffer_count\"" "$resident_output"
        grep -q "\"metal_resident_bytes\"" "$resident_output"
        comparison="$bench_output_dir/wgpu-vs-metal.txt"
        comparison_json="$bench_output_dir/wgpu-vs-metal.json"
        ref_comparison="$bench_output_dir/cpu-ref-vs-metal.txt"
        ref_comparison_json="$bench_output_dir/cpu-ref-vs-metal.json"
        {
            echo "baseline_backend=wgpu"
            echo "candidate_backend=metal"
            compare_status=0
            "$bench_bin" compare \
                --baseline "$bench_output_dir/wgpu.json" \
                --candidate "$bench_output_dir/metal.json" \
                --output "$comparison_json" || compare_status=$?
            echo "compare_exit_code=$compare_status"
        } > "$comparison"
        {
            echo "baseline_backend=cpu-ref"
            echo "candidate_backend=metal"
            compare_status=0
            "$bench_bin" compare \
                --baseline "$bench_output_dir/cpu-ref.json" \
                --candidate "$bench_output_dir/metal.json" \
                --output "$ref_comparison_json" || compare_status=$?
            echo "compare_exit_code=$compare_status"
        } > "$ref_comparison"
        test -s "$comparison"
        test -s "$comparison_json"
        test -s "$ref_comparison"
        test -s "$ref_comparison_json"
        bundle_manifest="$bench_output_dir/bundle-manifest.json"
        "$bench_bin" validate-comparison \
            --path "$comparison_json" \
            --baseline-backend wgpu \
            --candidate-backend metal \
            --case foundation.elementwise.add.1m >/dev/null
        "$bench_bin" validate-comparison \
            --path "$ref_comparison_json" \
            --baseline-backend cpu-ref \
            --candidate-backend metal \
            --case foundation.elementwise.add.1m >/dev/null
        grep -q "baseline_backend=wgpu" "$comparison"
        grep -q "candidate_backend=metal" "$comparison"
        grep -q "baseline_backend=cpu-ref" "$ref_comparison"
        grep -q "candidate_backend=metal" "$ref_comparison"
        grep -q "baseline_profile_backend=wgpu" "$comparison"
        grep -q "candidate_profile_backend=metal" "$comparison"
        grep -q "baseline_profile_backend=cpu-ref" "$ref_comparison"
        grep -q "candidate_profile_backend=metal" "$ref_comparison"
        grep -q "baseline_timing_quality=" "$comparison"
        grep -q "candidate_timing_quality=" "$comparison"
        grep -q "baseline_timing_quality=" "$ref_comparison"
        grep -q "candidate_timing_quality=" "$ref_comparison"
        grep -q "compare_exit_code=" "$comparison"
        grep -q "compare_exit_code=" "$ref_comparison"
        grep -q "foundation.elementwise.add.1m" "$comparison"
        grep -q "foundation.elementwise.add.1m" "$ref_comparison"
        "$bench_bin" validate-benchmark-bundle \
            --dir "$bench_output_dir" \
            --manifest-output "$bundle_manifest" >/dev/null
        "$bench_bin" validate-benchmark-bundle \
            --dir "$bench_output_dir" \
            --manifest-input "$bundle_manifest" >/dev/null
        test -s "$bundle_manifest"
        grep -q "\"schema\": \"vyre-bench.bundle.v1\"" "$bundle_manifest"
        grep -q "\"validator\": \"vyre-bench validate-benchmark-bundle\"" "$bundle_manifest"
        grep -q "\"suite\": \"smoke\"" "$bundle_manifest"
        grep -q "\"case_id\": \"foundation.elementwise.add.1m\"" "$bundle_manifest"
        grep -q "\"baseline_backend\": \"wgpu\"" "$bundle_manifest"
        grep -q "\"candidate_backend\": \"metal\"" "$bundle_manifest"
        grep -q "\"comparison_pairs\"" "$bundle_manifest"
        grep -q "\"cpu-ref->metal\"" "$bundle_manifest"
        grep -q "\"wgpu->metal\"" "$bundle_manifest"
        grep -q "\"source_fingerprint\"" "$bundle_manifest"
        grep -q "\"source_tree_fingerprint\"" "$bundle_manifest"
        grep -q "\"artifact_count\": 7" "$bundle_manifest"
        grep -q "\"bundle_blake3\"" "$bundle_manifest"
        grep -q "\"path\": \"metal.json\"" "$bundle_manifest"
        grep -q "\"path\": \"cpu-ref-vs-metal.json\"" "$bundle_manifest"
        echo "metal-macbook: benchmark reports written to $bench_output_dir" >&2
    '
}

case "${mode}" in
    driver|correctness)
        run_driver
        ;;
    conformance)
        run_conformance
        ;;
    benchmark)
        run_benchmark
        ;;
    all)
        run_driver
        run_conformance
        run_benchmark
        ;;
    *)
        usage
        echo "Fix: unknown Metal MacBook gate mode '${mode}'." >&2
        exit 2
        ;;
esac
