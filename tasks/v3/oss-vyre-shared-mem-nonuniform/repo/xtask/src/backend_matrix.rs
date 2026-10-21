//! Backend release-policy evidence for CUDA-first / WGPU-fallback.

use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Serialize;
use vyre_driver::backend::{
    acquire, acquire_preferred_dispatch_backend, backend_dispatches, backend_precedence,
    registered_backends_by_precedence_slice,
};

use vyre_driver_cuda as _;
use vyre_driver_wgpu as _;

const MAX_BACKEND_EVIDENCE_TEXT_BYTES: u64 = 4_194_304;

#[derive(Debug, Serialize)]
struct BackendMatrix {
    schema_version: u32,
    cuda_first: bool,
    wgpu_fallback_present: bool,
    preferred_backend_id: Option<String>,
    preferred_backend_gpu_only: bool,
    gpu_probe: GpuProbe,
    cuda_feature_markers: Vec<BackendFeatureMarker>,
    wgpu_feature_markers: Vec<BackendFeatureMarker>,
    capability_rows: Vec<BackendCapabilityRow>,
    hidden_fallback_findings: Vec<BackendSourceFinding>,
    hidden_fallback_scan_errors: Vec<String>,
    backends: Vec<BackendEntry>,
    blockers: Vec<String>,
}

#[derive(Debug, Serialize)]
struct GpuProbe {
    nvidia_smi_ok: bool,
    nvidia_smi_devices: Vec<String>,
    nvidia_smi_device_details: Vec<GpuProbeDevice>,
    nvidia_driver_version: Option<String>,
    nvidia_cuda_version: Option<String>,
    nvidia_smi_error: Option<String>,
}

#[derive(Debug, Serialize)]
struct GpuProbeDevice {
    name: String,
    driver_version: String,
    memory_total_mib: Option<u64>,
    compute_capability_major: Option<u32>,
    compute_capability_minor: Option<u32>,
}

#[derive(Debug, Serialize)]
struct BackendEntry {
    id: String,
    precedence: u32,
    dispatches: bool,
    acquire_ok: bool,
    acquire_error: Option<String>,
}

#[derive(Debug, Serialize)]
struct BackendFeatureMarker {
    id: &'static str,
    path: String,
    exists: bool,
    read_error: Option<String>,
    source_bytes: usize,
    implementation_tokens: Vec<&'static str>,
    missing_tokens: Vec<&'static str>,
    unresolved_markers: Vec<&'static str>,
    role: &'static str,
}

#[derive(Debug, Serialize)]
struct BackendSourceFinding {
    path: String,
    line: usize,
    pattern: &'static str,
}

#[derive(Debug, Clone, Serialize)]
struct BackendCapabilityRow {
    backend_id: String,
    capability_id: String,
    probe_source: String,
    probed_value: Option<String>,
    supported: bool,
    unsupported_reason: Option<String>,
    fix: String,
}

struct BackendFeatureRequirement {
    id: &'static str,
    relative: &'static str,
    role: &'static str,
    tokens: &'static [&'static str],
}

const CUDA_FEATURE_MARKERS: &[BackendFeatureRequirement] = &[
    BackendFeatureRequirement {
        id: "tensor-core-fragment",
        relative: "vyre-emit-ptx/src/patterns/tensor_core_fragment/mod.rs",
        role: "Tensor-core/MMA lowering pattern",
        tokens: &["mma", "fragment"],
    },
    BackendFeatureRequirement {
        id: "ldmatrix-cp-async",
        relative: "vyre-emit-ptx/src/patterns/ldmatrix_cp_async/mod.rs",
        role: "Ampere+ async global-to-shared staging pattern",
        tokens: &["ldmatrix", "cp.async"],
    },
    BackendFeatureRequirement {
        id: "predicated-execution",
        relative: "vyre-emit-ptx/src/patterns/predicated_execution/mod.rs",
        role: "Predicated execution pattern",
        tokens: &["predicate", "predicated"],
    },
    BackendFeatureRequirement {
        id: "instruction-scheduling",
        relative: "vyre-emit-ptx/src/patterns/instruction_scheduling/mod.rs",
        role: "PTX instruction scheduling pattern",
        tokens: &["schedule", "latency"],
    },
    BackendFeatureRequirement {
        id: "ptx-vector-load-gap-scheduling",
        relative: "vyre-emit-ptx/src/emitter/body.rs",
        role: "PTX fused vector-load latency-gap scheduling",
        tokens: &["vector-load gap", "find_latency_filler_avoiding_results"],
    },
    BackendFeatureRequirement {
        id: "ptx-compute-load-gap-scheduling",
        relative: "vyre-emit-ptx/src/emitter/schedule.rs",
        role: "PTX load-use latency-gap scheduling with independent compute fillers",
        tokens: &[
            "KernelOpKind::Fma",
            "KernelOpKind::MatrixMma",
            "KernelOpKind::SubgroupReduce",
        ],
    },
    BackendFeatureRequirement {
        id: "ptx-vector-load-fusion",
        relative: "vyre-emit-ptx/src/emitter/vector.rs",
        role: "PTX vector load fusion pattern",
        tokens: &["ld.global", "v4"],
    },
    BackendFeatureRequirement {
        id: "ptx-vector-store-fusion",
        relative: "vyre-emit-ptx/src/emitter/vector.rs",
        role: "PTX vector store fusion pattern",
        tokens: &["st.global", "v4"],
    },
    BackendFeatureRequirement {
        id: "async-copy-emitter",
        relative: "vyre-emit-ptx/src/emitter/async_copy.rs",
        role: "PTX async copy emitter",
        tokens: &["cp.async", "commit_group"],
    },
    BackendFeatureRequirement {
        id: "mma-emitter",
        relative: "vyre-emit-ptx/src/emitter/mma.rs",
        role: "PTX MMA emitter",
        tokens: &["mma", "sync"],
    },
    BackendFeatureRequirement {
        id: "cuda-resident-dispatch",
        relative: "vyre-driver-cuda/src/backend/resident_dispatch.rs",
        role: "CUDA resident dispatch release path",
        tokens: &["dispatch_resident", "ptx"],
    },
    BackendFeatureRequirement {
        id: "cuda-resident-io",
        relative: "vyre-driver-cuda/src/backend/resident_io.rs",
        role: "CUDA resident input/output buffers and sparse readback batching",
        tokens: &[
            "download_resident_readbacks_many",
            "upload_resident_at_many",
            "resident_device_ptr",
        ],
    },
    BackendFeatureRequirement {
        id: "cuda-graph-launch",
        relative: "vyre-driver-cuda/src/backend/cuda_graph.rs",
        role: "CUDA graph launch path",
        tokens: &["record_cuda_graph", "cugraph"],
    },
    BackendFeatureRequirement {
        id: "cuda-module-cache",
        relative: "vyre-driver-cuda/src/backend/module_cache.rs",
        role: "CUDA PTX module cache",
        tokens: &["function_for_ptx", "ptx_target_sm"],
    },
    BackendFeatureRequirement {
        id: "cuda-ptx-source-cache",
        relative: "vyre-driver-cuda/src/backend/module_cache.rs",
        role: "CUDA PTX source cache before module load",
        tokens: &[
            "CudaPtxSourceCache",
            "CudaPtxSourceCacheSnapshot",
            "get_or_lower",
            "snapshot",
            "PTX_SOURCE_CACHE_SOFT_CAP",
            "evict_submodular",
        ],
    },
    BackendFeatureRequirement {
        id: "cuda-ptx-target-probe",
        relative: "vyre-driver-cuda/src/backend/ptx_target.rs",
        role: "CUDA loadable PTX target probing",
        tokens: &["select_loadable_ptx_target_sm", "cumoduleloaddata"],
    },
    BackendFeatureRequirement {
        id: "megakernel-paired-speculation",
        relative: "vyre-runtime/src/megakernel/speculation.rs",
        role: "Megakernel paired speculative execution adoption policy",
        tokens: &[
            "PairedSpeculationWindow",
            "record_sample",
            "side_compile_cost_ns",
            "decide_speculation",
        ],
    },
];

const WGPU_FEATURE_MARKERS: &[BackendFeatureRequirement] = &[
    BackendFeatureRequirement {
        id: "wgpu-persistent-engine",
        relative: "vyre-driver-wgpu/src/engine/persistent.rs",
        role: "WGPU persistent execution engine",
        tokens: &["persistent", "dispatch"],
    },
    BackendFeatureRequirement {
        id: "wgpu-megakernel-dispatcher",
        relative: "vyre-driver-wgpu/src/megakernel/dispatcher.rs",
        role: "WGPU megakernel dispatcher",
        tokens: &["megakernel", "dispatch"],
    },
    BackendFeatureRequirement {
        id: "wgpu-readback-ring",
        relative: "vyre-driver-wgpu/src/runtime/readback_ring.rs",
        role: "WGPU sparse/readback ring",
        tokens: &["ring", "readback"],
    },
    BackendFeatureRequirement {
        id: "wgpu-async-dispatch-prefetch",
        relative: "vyre-driver-wgpu/src/async_dispatch.rs",
        role: "WGPU non-blocking dispatch with predicted pipeline prefetch",
        tokens: &["dispatch_borrowed_async", "PipelinePrefetch"],
    },
    BackendFeatureRequirement {
        id: "wgpu-dispatch-scratch-reuse",
        relative: "vyre-driver-wgpu/src/engine/dispatch_scratch.rs",
        role: "WGPU dispatch hot-path scratch arena reuse",
        tokens: &["thread_local", "reset"],
    },
    BackendFeatureRequirement {
        id: "wgpu-disk-cache",
        relative: "vyre-driver-wgpu/src/pipeline/disk_cache.rs",
        role: "WGPU pipeline disk cache",
        tokens: &["cache", "pipeline", "MAX_PENDING_DURABLE_CACHE_FILES"],
    },
    BackendFeatureRequirement {
        id: "wgpu-no-cpu-fallback-test",
        relative: "vyre-driver-wgpu/tests/dispatch_never_cpu_fallback.rs",
        role: "WGPU no-hidden-CPU-fallback contract",
        tokens: &["never", "cpu", "fallback"],
    },
    BackendFeatureRequirement {
        id: "megakernel-paired-speculation",
        relative: "vyre-runtime/src/megakernel/speculation.rs",
        role: "Megakernel paired speculative execution adoption policy",
        tokens: &[
            "PairedSpeculationWindow",
            "record_sample",
            "side_compile_cost_ns",
            "decide_speculation",
        ],
    },
];

const UNRESOLVED_MARKERS: &[&str] = &[
    "todo",
    "fixme",
    "placeholder",
    "stub",
    "todo!",
    "unimplemented!",
    "panic!(\"not implemented",
    "tbd",
];

const HIDDEN_FALLBACK_PATTERNS: &[&str] = &[
    "cpu fallback",
    "software fallback",
    "fallback dispatch",
    "falling back to cpu",
    "fallback to cpu",
];

const BACKEND_PRODUCTION_SCAN_ROOTS: &[&str] = &[
    "vyre-driver/src",
    "vyre-driver-cuda/src",
    "vyre-driver-wgpu/src",
    "vyre-runtime/src",
];

pub(crate) fn run(args: &[String]) {
    let output = match parse_output(args) {
        Ok(output) => output,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(2);
        }
    };
    let mut backends = Vec::new();
    for registration in registered_backends_by_precedence_slice() {
        let dispatches = backend_dispatches(registration.id);
        let acquire_result = acquire(registration.id);
        let (acquire_ok, acquire_error) = match acquire_result {
            Ok(_) => (true, None),
            Err(error) => (false, Some(error.to_string())),
        };
        backends.push(BackendEntry {
            id: registration.id.to_string(),
            precedence: backend_precedence(registration.id),
            dispatches,
            acquire_ok,
            acquire_error,
        });
    }

    let cuda = backends.iter().find(|backend| backend.id == "cuda");
    let wgpu = backends.iter().find(|backend| backend.id == "wgpu");
    let preferred_backend = acquire_preferred_dispatch_backend();
    let preferred_backend_id = preferred_backend
        .as_ref()
        .ok()
        .map(|backend| backend.id().to_string());
    let preferred_backend_gpu_only = preferred_backend_id
        .as_deref()
        .is_some_and(|id| matches!(id, "cuda" | "wgpu"));
    let cuda_first = match (cuda, wgpu) {
        (Some(cuda), Some(wgpu)) => {
            cuda.dispatches && cuda.acquire_ok && cuda.precedence < wgpu.precedence
        }
        (Some(cuda), None) => cuda.dispatches && cuda.acquire_ok,
        _ => false,
    };
    let wgpu_fallback_present =
        wgpu.is_some_and(|backend| backend.dispatches && backend.acquire_ok);
    let mut blockers = Vec::new();
    if !cuda_first {
        blockers.push(
            "CUDA is not the first acquired dispatch backend. Fix: link/configure CUDA and give it higher precedence than WGPU.".to_string(),
        );
    }
    if !wgpu_fallback_present {
        blockers.push(
            "WGPU fallback is not present and acquireable. Fix: link/configure vyre-driver-wgpu."
                .to_string(),
        );
    }
    if !preferred_backend_gpu_only {
        let detail = preferred_backend_id.as_deref().map_or_else(
            || {
                preferred_backend
                    .as_ref()
                    .err()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| {
                        "preferred backend acquisition returned no backend".to_string()
                    })
            },
            |id| format!("preferred backend was `{id}`"),
        );
        blockers.push(format!(
            "preferred runtime backend is not GPU-only ({detail}). Fix: acquire_preferred_dispatch_backend must never select cpu-ref/reference as an implicit fallback."
        ));
    }
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let cuda_feature_markers = collect_cuda_feature_markers(&workspace_root, &mut blockers);
    let wgpu_feature_markers =
        collect_feature_markers(&workspace_root, WGPU_FEATURE_MARKERS, &mut blockers);
    let (hidden_fallback_findings, hidden_fallback_scan_errors) =
        scan_hidden_fallback_language(&workspace_root, &mut blockers);
    for finding in &hidden_fallback_findings {
        blockers.push(format!(
            "backend production source `{}`:{} contains hidden fallback language `{}`",
            finding.path, finding.line, finding.pattern
        ));
    }
    let gpu_probe = probe_nvidia_smi();
    if !gpu_probe.nvidia_smi_ok {
        blockers.push(
            "nvidia-smi -L did not report a GPU. Fix: repair CUDA/NVIDIA driver visibility before release benchmarking."
                .to_string(),
        );
    }
    if !gpu_probe.nvidia_smi_device_details.iter().any(|device| {
        device.memory_total_mib.is_some_and(|mib| mib >= 16 * 1024)
            && matches!(
                (device.compute_capability_major, device.compute_capability_minor),
                (Some(major), Some(minor)) if (major, minor) >= (8, 0)
            )
    }) {
        blockers.push(
            "nvidia-smi did not report a CUDA GPU meeting the release floor: >=16384 MiB VRAM and compute capability >=8.0"
                .to_string(),
        );
    }
    let capability_rows = collect_backend_capability_rows(&backends, &gpu_probe);
    blockers.extend(capability_contract_blockers(&capability_rows));
    let matrix = BackendMatrix {
        schema_version: 3,
        cuda_first,
        wgpu_fallback_present,
        preferred_backend_id,
        preferred_backend_gpu_only,
        gpu_probe,
        cuda_feature_markers,
        wgpu_feature_markers,
        capability_rows,
        hidden_fallback_findings,
        hidden_fallback_scan_errors,
        backends,
        blockers,
    };
    let json = match serde_json::to_string_pretty(&matrix) {
        Ok(json) => json,
        Err(error) => {
            eprintln!("Fix: failed to serialize backend matrix: {error}");
            std::process::exit(1);
        }
    };
    if let Some(parent) = output.parent() {
        if let Err(error) = fs::create_dir_all(parent) {
            eprintln!("Fix: failed to create `{}`: {error}", parent.display());
            std::process::exit(1);
        }
    }
    if let Err(error) = fs::write(&output, format!("{json}\n")) {
        eprintln!("Fix: failed to write `{}`: {error}", output.display());
        std::process::exit(1);
    }
    println!("backend-matrix: wrote {}", output.display());
    if !matrix.blockers.is_empty() {
        std::process::exit(1);
    }
}

fn collect_cuda_feature_markers(
    workspace_root: &Path,
    blockers: &mut Vec<String>,
) -> Vec<BackendFeatureMarker> {
    collect_feature_markers(workspace_root, CUDA_FEATURE_MARKERS, blockers)
}

fn collect_feature_markers(
    workspace_root: &Path,
    requirements: &'static [BackendFeatureRequirement],
    blockers: &mut Vec<String>,
) -> Vec<BackendFeatureMarker> {
    let mut markers = Vec::new();
    for requirement in requirements {
        let path = workspace_root.join(requirement.relative);
        let exists = path.is_file();
        let (text, read_error) = if exists {
            match read_text_bounded(&path) {
                Ok(text) => (text, None),
                Err(error) => {
                    blockers.push(format!(
                        "backend feature marker `{}` could not be read at {}: {error}",
                        requirement.id,
                        path.display()
                    ));
                    (String::new(), Some(error.to_string()))
                }
            }
        } else {
            (String::new(), None)
        };
        let lowered = text.to_ascii_lowercase();
        let code_lowered = non_comment_source(&text).to_ascii_lowercase();
        let missing_tokens = requirement
            .tokens
            .iter()
            .copied()
            .filter(|token| !code_lowered.contains(&token.to_ascii_lowercase()))
            .collect::<Vec<_>>();
        let unresolved_markers = UNRESOLVED_MARKERS
            .iter()
            .copied()
            .filter(|marker| lowered.contains(marker))
            .collect::<Vec<_>>();
        if !exists {
            blockers.push(format!(
                "backend feature marker `{}` is missing at {}",
                requirement.id,
                path.display()
            ));
        } else if text.trim().is_empty() {
            blockers.push(format!(
                "backend feature marker `{}` is empty",
                requirement.id
            ));
        }
        for token in &missing_tokens {
            blockers.push(format!(
                "backend feature marker `{}` does not contain implementation token `{token}`",
                requirement.id
            ));
        }
        for marker in &unresolved_markers {
            blockers.push(format!(
                "backend feature marker `{}` contains unresolved marker `{marker}`",
                requirement.id
            ));
        }
        markers.push(BackendFeatureMarker {
            id: requirement.id,
            path: path.display().to_string(),
            exists,
            read_error,
            source_bytes: text.len(),
            implementation_tokens: requirement.tokens.to_vec(),
            missing_tokens,
            unresolved_markers,
            role: requirement.role,
        });
    }
    markers
}

fn collect_backend_capability_rows(
    backends: &[BackendEntry],
    gpu_probe: &GpuProbe,
) -> Vec<BackendCapabilityRow> {
    let cuda = backends.iter().find(|backend| backend.id == "cuda");
    let wgpu = backends.iter().find(|backend| backend.id == "wgpu");
    let mut rows = Vec::new();
    rows.push(registry_capability_row("cuda", cuda));
    rows.push(registry_capability_row("wgpu", wgpu));

    let cuda_sm = highest_cuda_compute_capability(gpu_probe);
    let cuda_sm_supported = cuda_sm.is_some_and(|(major, minor)| (major, minor) >= (8, 0));
    rows.push(BackendCapabilityRow {
        backend_id: "cuda".to_string(),
        capability_id: "live-sm-release-floor".to_string(),
        probe_source: "nvidia-smi --query-gpu=compute_cap".to_string(),
        probed_value: cuda_sm.map(|(major, minor)| format!("sm_{major}{minor}")),
        supported: cuda_sm_supported,
        unsupported_reason: (!cuda_sm_supported).then(|| {
            "no live NVIDIA device reported compute capability >= 8.0".to_string()
        }),
        fix: "Fix: repair CUDA driver/device visibility or route release benchmarks to a supported NVIDIA GPU.".to_string(),
    });

    let max_memory = max_cuda_memory_mib(gpu_probe);
    let memory_supported = max_memory.is_some_and(|mib| mib >= 16 * 1024);
    rows.push(BackendCapabilityRow {
        backend_id: "cuda".to_string(),
        capability_id: "live-memory-release-floor".to_string(),
        probe_source: "nvidia-smi --query-gpu=memory.total".to_string(),
        probed_value: max_memory.map(|mib| format!("{mib} MiB")),
        supported: memory_supported,
        unsupported_reason: (!memory_supported).then(|| {
            "no live NVIDIA device reported >=16384 MiB memory".to_string()
        }),
        fix: "Fix: run release benchmark evidence on a CUDA GPU with at least 16 GiB VRAM.".to_string(),
    });

    let warp_supported = gpu_probe.nvidia_smi_ok && cuda_sm.is_some();
    rows.push(BackendCapabilityRow {
        backend_id: "cuda".to_string(),
        capability_id: "warp-width-contract".to_string(),
        probe_source: "CUDA warp-size contract gated by live NVIDIA device probe".to_string(),
        probed_value: warp_supported.then(|| "32 lanes".to_string()),
        supported: warp_supported,
        unsupported_reason: (!warp_supported).then(|| {
            "CUDA warp-width contract is unavailable without a live NVIDIA GPU probe".to_string()
        }),
        fix: "Fix: expose a live CUDA device before using warp-width-sensitive benchmark claims.".to_string(),
    });

    rows.push(BackendCapabilityRow {
        backend_id: "cuda".to_string(),
        capability_id: "mlir-transform-support".to_string(),
        probe_source: "backend-matrix transform support registry".to_string(),
        probed_value: None,
        supported: false,
        unsupported_reason: Some(
            "CUDA backend does not expose a live MLIR transform-dialect capability probe"
                .to_string(),
        ),
        fix: "Fix: wire transform capability probing before claiming transform-scheduled CUDA lowering.".to_string(),
    });

    rows.push(BackendCapabilityRow {
        backend_id: "wgpu".to_string(),
        capability_id: "adapter-live-acquire".to_string(),
        probe_source: "vyre_driver::backend::acquire(\"wgpu\")".to_string(),
        probed_value: wgpu.map(|backend| {
            format!(
                "dispatches={},acquire_ok={},precedence={}",
                backend.dispatches, backend.acquire_ok, backend.precedence
            )
        }),
        supported: wgpu.is_some_and(|backend| backend.dispatches && backend.acquire_ok),
        unsupported_reason: (!wgpu.is_some_and(|backend| backend.dispatches && backend.acquire_ok))
            .then(|| {
                wgpu.and_then(|backend| backend.acquire_error.clone())
                    .unwrap_or_else(|| "wgpu backend is not registered or acquireable".to_string())
            }),
        fix: "Fix: configure vyre-driver-wgpu so fallback evidence is backed by an acquireable adapter.".to_string(),
    });

    rows.push(BackendCapabilityRow {
        backend_id: "wgpu".to_string(),
        capability_id: "mlir-transform-support".to_string(),
        probe_source: "backend-matrix transform support registry".to_string(),
        probed_value: None,
        supported: false,
        unsupported_reason: Some(
            "WGPU backend does not expose a live MLIR transform-dialect capability probe"
                .to_string(),
        ),
        fix: "Fix: wire transform capability probing before claiming transform-scheduled WGPU lowering.".to_string(),
    });

    rows
}

fn registry_capability_row(backend_id: &str, backend: Option<&BackendEntry>) -> BackendCapabilityRow {
    let supported = backend.is_some_and(|backend| backend.dispatches && backend.acquire_ok);
    BackendCapabilityRow {
        backend_id: backend_id.to_string(),
        capability_id: "registered-dispatch-backend".to_string(),
        probe_source: "vyre_driver::backend registry plus acquire()".to_string(),
        probed_value: backend.map(|backend| {
            format!(
                "dispatches={},acquire_ok={},precedence={}",
                backend.dispatches, backend.acquire_ok, backend.precedence
            )
        }),
        supported,
        unsupported_reason: (!supported).then(|| {
            backend
                .and_then(|backend| backend.acquire_error.clone())
                .unwrap_or_else(|| format!("{backend_id} backend is not dispatchable/acquireable"))
        }),
        fix: format!(
            "Fix: register and configure `{backend_id}` before publishing backend support claims."
        ),
    }
}

fn highest_cuda_compute_capability(gpu_probe: &GpuProbe) -> Option<(u32, u32)> {
    gpu_probe
        .nvidia_smi_device_details
        .iter()
        .filter_map(|device| {
            Some((
                device.compute_capability_major?,
                device.compute_capability_minor?,
            ))
        })
        .max()
}

fn max_cuda_memory_mib(gpu_probe: &GpuProbe) -> Option<u64> {
    gpu_probe
        .nvidia_smi_device_details
        .iter()
        .filter_map(|device| device.memory_total_mib)
        .max()
}

fn capability_contract_blockers(rows: &[BackendCapabilityRow]) -> Vec<String> {
    let mut blockers = Vec::new();
    if rows.is_empty() {
        blockers.push("backend capability matrix emitted zero capability rows".to_string());
        return blockers;
    }
    let mut seen = std::collections::BTreeSet::new();
    for row in rows {
        if row.backend_id.trim().is_empty() {
            blockers.push("backend capability row has blank backend_id".to_string());
        }
        if row.capability_id.trim().is_empty() {
            blockers.push(format!(
                "backend capability row for `{}` has blank capability_id",
                row.backend_id
            ));
        }
        if !seen.insert((row.backend_id.clone(), row.capability_id.clone())) {
            blockers.push(format!(
                "backend capability row duplicates `{}`/`{}`",
                row.backend_id, row.capability_id
            ));
        }
        if row.probe_source.trim().is_empty() {
            blockers.push(format!(
                "backend capability `{}`/`{}` has no probe_source",
                row.backend_id, row.capability_id
            ));
        }
        if row.supported && row.probed_value.as_deref().is_none_or(str::is_empty) {
            blockers.push(format!(
                "backend capability `{}`/`{}` is supported but has no probed_value",
                row.backend_id, row.capability_id
            ));
        }
        if !row.supported
            && row
                .unsupported_reason
                .as_deref()
                .is_none_or(|reason| reason.trim().is_empty())
        {
            blockers.push(format!(
                "backend capability `{}`/`{}` is unsupported but has no unsupported_reason",
                row.backend_id, row.capability_id
            ));
        }
        if row.fix.trim().is_empty() || !row.fix.starts_with("Fix:") {
            blockers.push(format!(
                "backend capability `{}`/`{}` must include actionable Fix text",
                row.backend_id, row.capability_id
            ));
        }
        let assumption_text = format!(
            "{} {} {}",
            row.probe_source,
            row.probed_value.as_deref().unwrap_or_default(),
            row.fix
        )
        .to_ascii_lowercase();
        if assumption_text.contains("hardcoded") || assumption_text.contains("assume gpu") {
            blockers.push(format!(
                "backend capability `{}`/`{}` contains hardcoded capability language",
                row.backend_id, row.capability_id
            ));
        }
    }
    blockers
}

fn scan_hidden_fallback_language(
    workspace_root: &Path,
    blockers: &mut Vec<String>,
) -> (Vec<BackendSourceFinding>, Vec<String>) {
    let mut findings = Vec::new();
    let mut scan_errors = Vec::new();
    for root in BACKEND_PRODUCTION_SCAN_ROOTS {
        scan_hidden_fallback_dir(
            &workspace_root.join(root),
            &mut findings,
            &mut scan_errors,
            blockers,
        );
    }
    (findings, scan_errors)
}

fn scan_hidden_fallback_dir(
    root: &Path,
    findings: &mut Vec<BackendSourceFinding>,
    scan_errors: &mut Vec<String>,
    blockers: &mut Vec<String>,
) {
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) => {
            let message = format!(
                "hidden fallback scan could not read directory `{}`: {error}",
                root.display()
            );
            blockers.push(message.clone());
            scan_errors.push(message);
            return;
        }
    };
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                let message = format!(
                    "hidden fallback scan could not read entry in `{}`: {error}",
                    root.display()
                );
                blockers.push(message.clone());
                scan_errors.push(message);
                continue;
            }
        };
        let path = entry.path();
        if path.is_dir() {
            scan_hidden_fallback_dir(&path, findings, scan_errors, blockers);
            continue;
        }
        if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }
        scan_hidden_fallback_file(&path, findings, scan_errors, blockers);
    }
}

fn scan_hidden_fallback_file(
    path: &Path,
    findings: &mut Vec<BackendSourceFinding>,
    scan_errors: &mut Vec<String>,
    blockers: &mut Vec<String>,
) {
    let text = match read_text_bounded(path) {
        Ok(text) => text,
        Err(error) => {
            let message = format!(
                "hidden fallback scan could not read source `{}`: {error}",
                path.display()
            );
            blockers.push(message.clone());
            scan_errors.push(message);
            return;
        }
    };
    for (line_index, line) in text.lines().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("//") {
            continue;
        }
        let lowered = line.to_ascii_lowercase();
        for &pattern in HIDDEN_FALLBACK_PATTERNS {
            if lowered.contains(pattern) {
                findings.push(BackendSourceFinding {
                    path: path.display().to_string(),
                    line: line_index + 1,
                    pattern,
                });
            }
        }
    }
}

fn non_comment_source(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for line in text.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("//") {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

fn probe_nvidia_smi() -> GpuProbe {
    match Command::new("nvidia-smi").arg("-L").output() {
        Ok(output) if output.status.success() => {
            let devices = String::from_utf8_lossy(&output.stdout)
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(ToString::to_string)
                .collect::<Vec<_>>();
            let (driver_version, cuda_version) = probe_nvidia_smi_versions();
            let device_details = probe_nvidia_smi_device_details();
            GpuProbe {
                nvidia_smi_ok: !devices.is_empty(),
                nvidia_smi_devices: devices,
                nvidia_smi_device_details: device_details,
                nvidia_driver_version: driver_version,
                nvidia_cuda_version: cuda_version,
                nvidia_smi_error: None,
            }
        }
        Ok(output) => GpuProbe {
            nvidia_smi_ok: false,
            nvidia_smi_devices: Vec::new(),
            nvidia_smi_device_details: Vec::new(),
            nvidia_driver_version: None,
            nvidia_cuda_version: None,
            nvidia_smi_error: Some(String::from_utf8_lossy(&output.stderr).trim().to_string()),
        },
        Err(error) => GpuProbe {
            nvidia_smi_ok: false,
            nvidia_smi_devices: Vec::new(),
            nvidia_smi_device_details: Vec::new(),
            nvidia_driver_version: None,
            nvidia_cuda_version: None,
            nvidia_smi_error: Some(error.to_string()),
        },
    }
}

fn probe_nvidia_smi_versions() -> (Option<String>, Option<String>) {
    let Ok(output) = Command::new("nvidia-smi").output() else {
        return (None, None);
    };
    if !output.status.success() {
        return (None, None);
    }
    parse_nvidia_smi_versions(&String::from_utf8_lossy(&output.stdout))
}

fn probe_nvidia_smi_device_details() -> Vec<GpuProbeDevice> {
    let Ok(output) = Command::new("nvidia-smi")
        .args([
            "--query-gpu=name,driver_version,memory.total,compute_cap",
            "--format=csv,noheader,nounits",
        ])
        .output()
    else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(parse_nvidia_smi_device_detail)
        .collect()
}

fn parse_nvidia_smi_device_detail(line: &str) -> Option<GpuProbeDevice> {
    let mut fields = line.split(',').map(str::trim);
    let name = fields.next()?.to_string();
    let driver_version = fields.next()?.to_string();
    let memory_total_mib = fields.next().and_then(|value| value.parse::<u64>().ok());
    let compute_capability = fields.next().and_then(parse_compute_capability);
    if name.is_empty() {
        return None;
    }
    Some(GpuProbeDevice {
        name,
        driver_version,
        memory_total_mib,
        compute_capability_major: compute_capability.map(|(major, _minor)| major),
        compute_capability_minor: compute_capability.map(|(_major, minor)| minor),
    })
}

fn parse_compute_capability(value: &str) -> Option<(u32, u32)> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    if let Some((major, minor)) = value.split_once('.') {
        Some((major.trim().parse().ok()?, minor.trim().parse().ok()?))
    } else {
        Some((value.parse().ok()?, 0))
    }
}

fn parse_nvidia_smi_versions(text: &str) -> (Option<String>, Option<String>) {
    let mut driver_version = None;
    let mut cuda_version = None;
    for line in text.lines() {
        if let Some(value) = parse_header_value(line, "Driver Version:") {
            driver_version = Some(value);
        }
        if let Some(value) = parse_header_value(line, "CUDA Version:") {
            cuda_version = Some(value);
        }
    }
    (driver_version, cuda_version)
}

fn parse_header_value(line: &str, label: &str) -> Option<String> {
    let start = line.find(label)? + label.len();
    let rest = line.get(start..)?.trim_start();
    let end = [rest.find('|'), rest.find(' ')]
        .into_iter()
        .flatten()
        .min()
        .unwrap_or(rest.len());
    let value = rest.get(..end)?.trim();
    (!value.is_empty()).then(|| value.to_string())
}

fn parse_output(args: &[String]) -> Result<PathBuf, String> {
    let mut output = None;
    let mut index = 2;
    while index < args.len() {
        match args[index].as_str() {
            "--output" => {
                let Some(path) = args.get(index + 1) else {
                    return Err("Fix: --output requires a path.".to_string());
                };
                output = Some(PathBuf::from(path));
                index += 2;
            }
            "--help" | "-h" => {
                println!(
                    "USAGE:\n  cargo_full run --bin xtask -- backend-matrix [--output PATH]\n\n\
                     Probes linked dispatch backends and writes CUDA-first/WGPU-fallback evidence JSON."
                );
                std::process::exit(0);
            }
            other => return Err(format!("Fix: unknown backend-matrix option `{other}`.")),
        }
    }
    Ok(output.unwrap_or_else(default_output))
}

fn default_output() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(|path| path.join("release/evidence/backends/backend-matrix.json"))
        .unwrap_or_else(|| PathBuf::from("release/evidence/backends/backend-matrix.json"))
}

fn read_text_bounded(path: &Path) -> io::Result<String> {
    let mut reader = fs::File::open(path)?.take(MAX_BACKEND_EVIDENCE_TEXT_BYTES.saturating_add(1));
    let mut text = String::new();
    reader.read_to_string(&mut text)?;
    if text.len() as u64 > MAX_BACKEND_EVIDENCE_TEXT_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{} exceeds {MAX_BACKEND_EVIDENCE_TEXT_BYTES} byte backend evidence read cap",
                path.display()
            ),
        ));
    }
    Ok(text)
}

#[cfg(test)]
mod capability_contract_tests {
    use super::*;

    #[test]
    fn unsupported_capability_rows_require_reason_and_fix() {
        let rows = vec![BackendCapabilityRow {
            backend_id: "cuda".to_string(),
            capability_id: "mlir-transform-support".to_string(),
            probe_source: "backend-matrix transform support registry".to_string(),
            probed_value: None,
            supported: false,
            unsupported_reason: None,
            fix: "missing".to_string(),
        }];

        let blockers = capability_contract_blockers(&rows);

        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("unsupported_reason")));
        assert!(blockers.iter().any(|blocker| blocker.contains("Fix")));
    }

    #[test]
    fn cuda_capability_rows_include_live_sm_memory_and_warp_contracts() {
        let backends = vec![BackendEntry {
            id: "cuda".to_string(),
            precedence: 0,
            dispatches: true,
            acquire_ok: true,
            acquire_error: None,
        }];
        let probe = GpuProbe {
            nvidia_smi_ok: true,
            nvidia_smi_devices: vec!["GPU 0: NVIDIA RTX 5090".to_string()],
            nvidia_smi_device_details: vec![GpuProbeDevice {
                name: "NVIDIA RTX 5090".to_string(),
                driver_version: "580.0".to_string(),
                memory_total_mib: Some(32 * 1024),
                compute_capability_major: Some(12),
                compute_capability_minor: Some(0),
            }],
            nvidia_driver_version: Some("580.0".to_string()),
            nvidia_cuda_version: Some("13.0".to_string()),
            nvidia_smi_error: None,
        };

        let rows = collect_backend_capability_rows(&backends, &probe);

        assert!(rows.iter().any(|row| {
            row.backend_id == "cuda"
                && row.capability_id == "live-sm-release-floor"
                && row.supported
                && row.probed_value.as_deref() == Some("sm_120")
        }));
        assert!(rows.iter().any(|row| {
            row.backend_id == "cuda"
                && row.capability_id == "live-memory-release-floor"
                && row.supported
                && row.probed_value.as_deref() == Some("32768 MiB")
        }));
        assert!(rows.iter().any(|row| {
            row.backend_id == "cuda"
                && row.capability_id == "warp-width-contract"
                && row.supported
                && row.probed_value.as_deref() == Some("32 lanes")
        }));
        assert!(capability_contract_blockers(&rows).is_empty());
    }
}
