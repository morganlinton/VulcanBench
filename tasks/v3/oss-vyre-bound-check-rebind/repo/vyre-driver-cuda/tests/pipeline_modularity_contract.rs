//! Integration test crate for the containing Vyre package.

use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn parent_cuda_pipeline_module_owns_construction_not_dispatch_trait_logic() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let pipeline = crate_root.join("src/pipeline.rs");
    let contents = fs::read_to_string(&pipeline)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", pipeline.display()));

    let forbidden = [
        "impl CompiledPipeline for CudaCompiledPipeline",
        "fn dispatch_borrowed_into",
        "fn dispatch_persistent_handles_into",
        "fn dispatch_persistent_resource_outputs",
    ];
    let mut findings = Vec::new();
    for pattern in forbidden {
        if contents.contains(pattern) {
            findings.push(pattern);
        }
    }

    let line_count = contents.lines().count();
    assert!(
        line_count <= 300,
        "src/pipeline.rs has {line_count} lines. Fix: keep CUDA pipeline construction in the parent and dispatch mechanics in pipeline/compiled_dispatch.rs."
    );
    assert!(
        findings.is_empty(),
        "src/pipeline.rs reabsorbed CUDA dispatch duties: {}. Fix: keep CompiledPipeline entrypoints in pipeline/compiled_dispatch.rs.",
        findings.join(", ")
    );
}

#[test]
fn cuda_pipeline_dispatch_duties_live_in_compiled_dispatch_module() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let compiled_dispatch = read_source(&crate_root, "src/pipeline/compiled_dispatch.rs");

    for (evidence, needle) in [
        (
            "CompiledPipeline trait implementation",
            "impl CompiledPipeline for CudaCompiledPipeline",
        ),
        ("borrowed dispatch entrypoint", "fn dispatch_borrowed_into"),
        (
            "resident-handle dispatch entrypoint",
            "fn dispatch_persistent_handles_into",
        ),
        (
            "resident resource-output dispatch entrypoint",
            "fn dispatch_persistent_resource_outputs",
        ),
        (
            "CUDA graph replay selection",
            "dispatch_borrowed_batched_via_cuda_graph_lanes",
        ),
        (
            "resident readback routing",
            "download_resident_readbacks_many_into",
        ),
        (
            "resident batch readback routing",
            "download_resident_readback_batches_many_into",
        ),
    ] {
        assert!(
            compiled_dispatch.contains(needle),
            "src/pipeline/compiled_dispatch.rs is missing {evidence}. Fix: keep CUDA pipeline construction in src/pipeline.rs and dispatch entrypoints in pipeline/compiled_dispatch.rs."
        );
    }
}

#[test]
fn cuda_persistent_dynamic_batches_enqueue_before_waiting() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let compiled_dispatch = read_source(&crate_root, "src/pipeline/compiled_dispatch.rs");

    assert!(
        compiled_dispatch.contains("dispatch_dynamic_persistent_batches_concurrently"),
        "Fix: mismatched-config CUDA persistent batches must share a concurrent dynamic resident dispatch path."
    );
    assert!(
        compiled_dispatch.contains("dispatches.push(self.backend.dispatch_resident_async_concrete_with_ptx_key"),
        "Fix: dynamic persistent batch dispatch must enqueue resident CUDA rows before readback waits."
    );
    for forbidden in [
        "for (batch, item_outputs) in batches.iter().zip(outputs.iter_mut())",
        "self.dispatch_persistent_handles_into(batch, config, item_outputs)",
        "for (row, item_outputs) in rows.iter().zip(outputs.iter_mut())",
        "self.dispatch_persistent_handles_into(row.as_slice(), config, item_outputs)",
    ] {
        assert!(
            !compiled_dispatch.contains(forbidden),
            "Fix: CUDA persistent batch config mismatch regressed to serial per-row host orchestration: `{forbidden}`."
        );
    }
}

#[test]
fn cuda_backend_duties_stay_in_one_purpose_modules() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let backend_mod = read_source(&crate_root, "src/backend/mod.rs");

    for (module, role, token) in [
        (
            "allocations",
            "transient device and pinned-host pools",
            "PinnedHostAllocationPool",
        ),
        (
            "capabilities",
            "capability and validation-cache policy",
            "pipeline_feature_flags",
        ),
        ("cuda_graph", "CUDA graph capture", "CachedCudaGraph"),
        (
            "cuda_graph_replay",
            "CUDA graph replay",
            "CudaGraphReplayStats",
        ),
        (
            "dispatch",
            "backend handle and launch orchestration",
            "CudaBackend",
        ),
        (
            "host_dispatch",
            "host-borrowed dispatch",
            "dispatch_borrowed",
        ),
        (
            "launch",
            "raw CUDA kernel launch boundary",
            "cuLaunchKernel",
        ),
        ("module_cache", "loaded PTX module cache", "ModuleCacheKey"),
        (
            "output_range",
            "CUDA output readback ranges",
            "CudaOutputReadback",
        ),
        ("plan", "dispatch-plan assembly", "CudaDispatchPlan"),
        (
            "ptx_target",
            "live PTX target probing",
            "select_loadable_ptx_target_sm",
        ),
        (
            "resident",
            "resident device allocations",
            "CudaResidentBuffer",
        ),
        (
            "resident_dispatch",
            "resident dispatch",
            "dispatch_resident",
        ),
        (
            "resident_io",
            "resident input/output copies",
            "download_resident_readbacks_many",
        ),
        (
            "telemetry",
            "CUDA runtime telemetry",
            "CudaTelemetrySnapshot",
        ),
    ] {
        assert!(
            backend_mod.contains(&format!("mod {module};"))
                || backend_mod.contains(&format!("pub mod {module};"))
                || backend_mod.contains(&format!("pub(crate) mod {module};")),
            "src/backend/mod.rs no longer declares `{module}`. Fix: keep {role} in its one-purpose CUDA backend module."
        );
        let source = read_source(&crate_root, &format!("src/backend/{module}.rs"));
        assert!(
            source.contains(token),
            "src/backend/{module}.rs is missing `{token}` evidence for {role}. Fix: do not move this duty back into src/pipeline.rs or backend/mod.rs."
        );
    }
}

#[test]
fn cuda_pipeline_parent_does_not_absorb_backend_duty_modules() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let pipeline = read_source(&crate_root, "src/pipeline.rs");

    for forbidden in [
        "cuLaunchKernel",
        "cuLaunchCooperativeKernel",
        "cuMemcpyDtoHAsync_v2",
        "cuMemcpyHtoDAsync_v2",
        "DashMap<",
        "CudaResidentStore",
        "CudaTelemetrySnapshot",
        "select_loadable_ptx_target_sm",
    ] {
        assert!(
            !pipeline.contains(forbidden),
            "src/pipeline.rs absorbed backend duty token `{forbidden}`. Fix: pipeline constructs compiled state only; launch, copies, telemetry, target probing, cache, and residency stay in backend/* modules."
        );
    }
}

fn read_source(crate_root: &Path, relative: &str) -> String {
    let path = crate_root.join(relative);
    fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()))
}
