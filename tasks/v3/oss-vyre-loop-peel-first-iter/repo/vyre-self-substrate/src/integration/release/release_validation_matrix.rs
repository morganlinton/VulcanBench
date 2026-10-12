//! Release validation command matrix for the platform, dataflow consumer, and compiler frontend.

/// One command required by the release validation matrix.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReleaseValidationCommand {
    /// Stable command identifier.
    pub id: &'static str,
    /// Owning crate or release area.
    pub area: &'static str,
    /// Exact command line to run from [`ReleaseValidationCommand::working_dir`].
    pub command: &'static str,
    /// Directory where the command must run, relative to the Vyre workspace root.
    pub working_dir: &'static str,
    /// Whether this command must run after a loud GPU probe.
    pub requires_gpu_probe: bool,
    /// Evidence artifact expected from the command output.
    pub evidence: &'static str,
}

/// One executable release-validation step after GPU probes are expanded.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReleaseValidationStep {
    /// Gate id that owns this step.
    pub gate_id: &'static str,
    /// Directory where the command must run, relative to the Vyre workspace root.
    pub working_dir: &'static str,
    /// Exact command to execute.
    pub command: &'static str,
    /// True when this step is the hardware probe preceding a GPU-required gate.
    pub is_gpu_probe: bool,
}

/// GPU probe command that must run before GPU-required release gates.
pub const GPU_PROBE_COMMAND: &str = "nvidia-smi";
const DATAFLOW_MANIFEST_ALIAS: &str = "@dataflow-manifest";
const DATAFLOW_MANIFEST_ENV: &str = "VYRE_DATAFLOW_MANIFEST";

/// Deep release validation matrix.
pub const RELEASE_VALIDATION_MATRIX: &[ReleaseValidationCommand] = &[
    ReleaseValidationCommand {
        id: "vyre-core-gpu-boundary",
        area: "vyre",
        command: "./cargo_full test -j1 -p vyre --test gpu_boundary_contracts",
        working_dir: ".",
        requires_gpu_probe: true,
        evidence: "production sources reject hidden CPU or host escape paths",
    },
    ReleaseValidationCommand {
        id: "vyre-production-cpu-fallback-lint",
        area: "vyre",
        command: "./cargo_full run -j1 -p vyre-lints -- --check-production-cpu-fallbacks",
        working_dir: ".",
        requires_gpu_probe: false,
        evidence: "default production roots reject CPU reference helpers and public CPU reexports",
    },
    ReleaseValidationCommand {
        id: "vyre-gpu-probe-contract",
        area: "vyre",
        command: "./cargo_full test -j1 -p vyre-self-substrate gpu_probe_contract",
        working_dir: ".",
        requires_gpu_probe: true,
        evidence: "GPU tests fail loudly with NVIDIA adapter and probe details instead of skip-on-no-GPU behavior",
    },
    ReleaseValidationCommand {
        id: "vyre-memory-ownership-contract",
        area: "vyre",
        command: "./cargo_full test -j1 -p vyre-self-substrate memory_ownership_contract",
        working_dir: ".",
        requires_gpu_probe: false,
        evidence: "host/device buffer ownership and staging boundaries are centralized instead of per-subsystem conventions",
    },
    ReleaseValidationCommand {
        id: "vyre-architecture-boundary-map",
        area: "vyre",
        command: "./cargo_full test -j1 -p vyre-self-substrate architecture_boundary_map",
        working_dir: ".",
        requires_gpu_probe: false,
        evidence: "platform, dataflow, and compiler frontend duties have exactly one declared owner for parsing, graph formation, lowering, scheduling, dispatch, and validation",
    },
    ReleaseValidationCommand {
        id: "vyre-contributor-module-map",
        area: "vyre",
        command: "./cargo_full test -j1 -p vyre-self-substrate contributor_module_map",
        working_dir: ".",
        requires_gpu_probe: false,
        evidence: "contributor-facing module map directs new kernels, analyses, parser phases, diagnostics, benchmarks, and validation to one-duty module families",
    },
    ReleaseValidationCommand {
        id: "vyre-public-api-boundary",
        area: "vyre",
        command: "./cargo_full test -j1 -p vyre-self-substrate public_api_boundary",
        working_dir: ".",
        requires_gpu_probe: false,
        evidence: "public APIs expose contract-level capabilities without staging buffers, temporary graph encodings, or pipeline internals",
    },
    ReleaseValidationCommand {
        id: "vyre-cuda-resident-dispatch",
        area: "vyre-cuda",
        command: "./cargo_full test -j1 -p vyre-driver-cuda --test resident_dispatch_contracts",
        working_dir: ".",
        requires_gpu_probe: true,
        evidence: "resident replay keeps readback and sync points sublinear",
    },
    ReleaseValidationCommand {
        id: "vyre-cuda-capability-contracts",
        area: "vyre-cuda",
        command: "./cargo_full test -j1 -p vyre-driver-cuda --test capability_contracts",
        working_dir: ".",
        requires_gpu_probe: true,
        evidence: "CUDA device capability probes produce actionable missing-feature diagnostics instead of silent fallback",
    },
    ReleaseValidationCommand {
        id: "vyre-cuda-cooperative-launch",
        area: "vyre-cuda",
        command: "./cargo_full test -j1 -p vyre-driver-cuda --test cooperative_launch_contracts",
        working_dir: ".",
        requires_gpu_probe: true,
        evidence: "cooperative launch prerequisites for resident megakernel execution are probed and enforced",
    },
    ReleaseValidationCommand {
        id: "vyre-cuda-module-cache",
        area: "vyre-cuda",
        command: "./cargo_full test -j1 -p vyre-driver-cuda --test module_cache_contracts",
        working_dir: ".",
        requires_gpu_probe: true,
        evidence: "CUDA module and PTX cache paths are bounded, keyed by device features, and avoid repeated compile/load churn",
    },
    ReleaseValidationCommand {
        id: "vyre-cuda-pipeline-modularity",
        area: "vyre-cuda",
        command: "./cargo_full test -j1 -p vyre-driver-cuda --test pipeline_modularity_contract",
        working_dir: ".",
        requires_gpu_probe: true,
        evidence: "CUDA pipeline files orchestrate only while cache, launch, diagnostics, residency, and dispatch duties live in separate modules",
    },
    ReleaseValidationCommand {
        id: "vyre-cuda-frontier-queue",
        area: "vyre-cuda",
        command: "./cargo_full test -j1 -p vyre-driver-cuda --test csr_frontier_queue_gpu_parity",
        working_dir: ".",
        requires_gpu_probe: true,
        evidence: "resident sparse CSR queue avoids selector readback and batched queries share one host fence",
    },
    ReleaseValidationCommand {
        id: "vyre-cuda-device-work-queue",
        area: "vyre-cuda",
        command: "./cargo_full test -j1 -p vyre-driver-cuda device_work_queue",
        working_dir: ".",
        requires_gpu_probe: true,
        evidence: "dependent dataflow work drains through resident device queues with final-only host synchronization",
    },
    ReleaseValidationCommand {
        id: "vyre-cuda-launch-fusion",
        area: "vyre-cuda",
        command: "./cargo_full test -j1 -p vyre-driver-cuda launch_fusion",
        working_dir: ".",
        requires_gpu_probe: true,
        evidence: "adjacent CUDA dataflow stages fuse when layouts and budgets allow without host-visible intermediates",
    },
    ReleaseValidationCommand {
        id: "vyre-cuda-multi-query-execution",
        area: "vyre-cuda",
        command: "./cargo_full test -j1 -p vyre-driver-cuda multi_query_execution",
        working_dir: ".",
        requires_gpu_probe: true,
        evidence: "many compatible analyses over one resident graph share traversal and host fence cost",
    },
    ReleaseValidationCommand {
        id: "vyre-cuda-result-compaction",
        area: "vyre-cuda",
        command: "./cargo_full test -j1 -p vyre-driver-cuda result_compaction",
        working_dir: ".",
        requires_gpu_probe: true,
        evidence: "small CUDA outputs compact to meaningful bytes before readback while large outputs stay direct",
    },
    ReleaseValidationCommand {
        id: "vyre-cuda-device-diagnostic-aggregation",
        area: "vyre-cuda",
        command: "./cargo_full test -j1 -p vyre-driver-cuda device_diagnostic_aggregation",
        working_dir: ".",
        requires_gpu_probe: true,
        evidence: "device diagnostics aggregate and cap on GPU instead of host-filtering raw diagnostic streams",
    },
    ReleaseValidationCommand {
        id: "vyre-cuda-kernel-failure-diagnostics",
        area: "vyre-cuda",
        command: "./cargo_full test -j1 -p vyre-driver-cuda kernel_failure_diagnostics",
        working_dir: ".",
        requires_gpu_probe: true,
        evidence: "kernel launch failures report every missing CUDA capability with actionable diagnostics",
    },
    ReleaseValidationCommand {
        id: "vyre-cuda-benchmark-pass-selection",
        area: "vyre-cuda",
        command: "./cargo_full test -j1 -p vyre-driver-cuda benchmark_pass_selection",
        working_dir: ".",
        requires_gpu_probe: true,
        evidence: "CUDA benchmark-driven pass selection ranks profitable passes without unbounded candidate copies",
    },
    ReleaseValidationCommand {
        id: "vyre-cuda-self-optimizer-e2e",
        area: "vyre-cuda",
        command: "./cargo_full test -j1 -p vyre-driver-cuda --test self_optimizer_e2e",
        working_dir: ".",
        requires_gpu_probe: true,
        evidence: "CUDA self-optimizer executes end-to-end optimizer passes instead of only registering pass metadata",
    },
    ReleaseValidationCommand {
        id: "vyre-cuda-self-optimizer-const-prop",
        area: "vyre-cuda",
        command: "./cargo_full test -j1 -p vyre-driver-cuda --test self_optimizer_const_prop_e2e",
        working_dir: ".",
        requires_gpu_probe: true,
        evidence: "CUDA self-optimizer constant propagation preserves behavior end to end",
    },
    ReleaseValidationCommand {
        id: "vyre-cuda-self-optimizer-cse",
        area: "vyre-cuda",
        command: "./cargo_full test -j1 -p vyre-driver-cuda --test self_optimizer_cse_e2e",
        working_dir: ".",
        requires_gpu_probe: true,
        evidence: "CUDA self-optimizer common-subexpression elimination preserves behavior end to end",
    },
    ReleaseValidationCommand {
        id: "vyre-cuda-self-optimizer-licm",
        area: "vyre-cuda",
        command: "./cargo_full test -j1 -p vyre-driver-cuda --test self_optimizer_licm_e2e",
        working_dir: ".",
        requires_gpu_probe: true,
        evidence: "CUDA self-optimizer loop-invariant code motion preserves behavior end to end",
    },
    ReleaseValidationCommand {
        id: "vyre-cuda-self-optimizer-dead-branch",
        area: "vyre-cuda",
        command: "./cargo_full test -j1 -p vyre-driver-cuda --test self_optimizer_dead_branch_e2e",
        working_dir: ".",
        requires_gpu_probe: true,
        evidence: "CUDA self-optimizer dead-branch elimination preserves behavior end to end",
    },
    ReleaseValidationCommand {
        id: "vyre-cuda-self-optimizer-pattern-match",
        area: "vyre-cuda",
        command: "./cargo_full test -j1 -p vyre-driver-cuda --test self_optimizer_pattern_match_e2e",
        working_dir: ".",
        requires_gpu_probe: true,
        evidence: "CUDA self-optimizer pattern matching rewrites preserve behavior end to end",
    },
    ReleaseValidationCommand {
        id: "vyre-cuda-self-optimizer-pipeline-resident",
        area: "vyre-cuda",
        command: "./cargo_full test -j1 -p vyre-driver-cuda --test self_optimizer_pipeline_resident_e2e",
        working_dir: ".",
        requires_gpu_probe: true,
        evidence: "CUDA self-optimizer resident pipeline keeps optimizer execution on resident buffers",
    },
    ReleaseValidationCommand {
        id: "vyre-cuda-megakernel-scheduler",
        area: "vyre-cuda",
        command: "./cargo_full test -j1 -p vyre-driver-cuda --test megakernel_scale_scheduler_contracts",
        working_dir: ".",
        requires_gpu_probe: true,
        evidence: "scheduler uses telemetry, barriers, memory budgets, and plan cache",
    },
    ReleaseValidationCommand {
        id: "vyre-cuda-convergence",
        area: "vyre-cuda",
        command: "./cargo_full test -j1 -p vyre-driver-cuda megakernel_convergence",
        working_dir: ".",
        requires_gpu_probe: true,
        evidence: "convergence planner rejects host-polled iteration",
    },
    ReleaseValidationCommand {
        id: "vyre-cuda-megakernel-speedup",
        area: "vyre-cuda",
        command: "./cargo_full test -j1 -p vyre-driver-cuda megakernel_speedup_gate",
        working_dir: ".",
        requires_gpu_probe: true,
        evidence: "steady-state resident megakernel samples enforce a 100x plus CUDA speedup floor without setup, upload, allocation, or host-sync pollution",
    },
    ReleaseValidationCommand {
        id: "resident-fixed-point",
        area: "dataflow",
        command: "./cargo_full test -j1 --manifest-path @dataflow-manifest --features cpu-parity fixed_point_resident",
        working_dir: ".",
        requires_gpu_probe: true,
        evidence: "resident analyses reject wrong graph families and reuse buffers",
    },
    ReleaseValidationCommand {
        id: "ifds-resident",
        area: "dataflow",
        command: "./cargo_full test -j1 --manifest-path @dataflow-manifest --features cpu-parity cuda_vyre_resident_ifds",
        working_dir: ".",
        requires_gpu_probe: true,
        evidence: "IFDS solve uses CUDA resident sequence and final flag readback",
    },
    ReleaseValidationCommand {
        id: "adversarial-oracles",
        area: "dataflow",
        command: "./cargo_full test -j1 --manifest-path @dataflow-manifest --features cpu-parity --test df_adversarial_oracles",
        working_dir: ".",
        requires_gpu_probe: false,
        evidence: "Dataflow analysis consumer adversarial graph families cover empty, single-node, fan-in, fan-out, cycles, disconnected, and degenerate inputs",
    },
    ReleaseValidationCommand {
        id: "fuzz-bitset-oracles",
        area: "dataflow",
        command: "./cargo_full test -j1 --manifest-path @dataflow-manifest --features cpu-parity --test df_fuzz_bitset_oracles",
        working_dir: ".",
        requires_gpu_probe: false,
        evidence: "Dataflow analysis consumer bitset oracle fuzzing covers arbitrary dataflow fact shapes under explicit parity feature",
    },
    ReleaseValidationCommand {
        id: "gap-bitset-oracle-edges",
        area: "dataflow",
        command: "./cargo_full test -j1 --manifest-path @dataflow-manifest --features cpu-parity --test df_gap_bitset_oracle_edges",
        working_dir: ".",
        requires_gpu_probe: false,
        evidence: "Dataflow analysis consumer tracked gap tests make missing edge cases reproducible instead of silently green",
    },
    ReleaseValidationCommand {
        id: "property-points-to",
        area: "dataflow",
        command: "./cargo_full test -j1 --manifest-path @dataflow-manifest --features cpu-parity --test df_property_points_to",
        working_dir: ".",
        requires_gpu_probe: false,
        evidence: "points-to property tests cover monotonicity and closure invariants",
    },
    ReleaseValidationCommand {
        id: "property-ifds",
        area: "dataflow",
        command: "./cargo_full test -j1 --manifest-path @dataflow-manifest --features cpu-parity --test df_property_ifds",
        working_dir: ".",
        requires_gpu_probe: false,
        evidence: "IFDS property tests cover reachability and lattice invariants",
    },
    ReleaseValidationCommand {
        id: "property-slice",
        area: "dataflow",
        command: "./cargo_full test -j1 --manifest-path @dataflow-manifest --features cpu-parity --test df_property_slice_construction",
        working_dir: ".",
        requires_gpu_probe: false,
        evidence: "slicing property tests cover traversal and dependency closure invariants",
    },
    ReleaseValidationCommand {
        id: "property-reaching-escapes",
        area: "dataflow",
        command: "./cargo_full test -j1 --manifest-path @dataflow-manifest --features cpu-parity --test df_property_reaching_def_escapes",
        working_dir: ".",
        requires_gpu_probe: false,
        evidence: "reaching definitions and escape properties cover monotone dataflow invariants",
    },
    ReleaseValidationCommand {
        id: "exact-primitive-parity",
        area: "dataflow",
        command: "./cargo_full test -j1 --manifest-path @dataflow-manifest --features cpu-parity --test df_parity_exact_primitives",
        working_dir: ".",
        requires_gpu_probe: true,
        evidence: "Dataflow analysis consumer exact primitive parity runs on GPU against explicit parity-only oracles",
    },
    ReleaseValidationCommand {
        id: "scale-oracle-no-oom",
        area: "dataflow",
        command: "./cargo_full test -j1 --manifest-path @dataflow-manifest --features cpu-parity --test df_scale_oracle_no_oom",
        working_dir: ".",
        requires_gpu_probe: false,
        evidence: "Dataflow analysis consumer scale and mass-fan inputs remain bounded and do not OOM",
    },
    ReleaseValidationCommand {
        id: "resident-fixed-point-benchmark",
        area: "dataflow",
        command: "./cargo_full bench -j1 --manifest-path @dataflow-manifest --features cpu-parity --bench resident_fixed_point_hot_path",
        working_dir: ".",
        requires_gpu_probe: true,
        evidence: "resident fixed-point benchmark isolates hot-path repeated graph execution",
    },
    ReleaseValidationCommand {
        id: "ifds-direct-resident-benchmark",
        area: "dataflow",
        command: "./cargo_full bench -j1 --manifest-path @dataflow-manifest --features cpu-parity --bench ifds_direct_resident_hot_path",
        working_dir: ".",
        requires_gpu_probe: true,
        evidence: "IFDS direct resident benchmark isolates hot-path repeated resident solves",
    },
    ReleaseValidationCommand {
        id: "vyrec-beta-contract",
        area: "vyrec",
        command: "./cargo_full test -j1 --manifest-path ../../../../tools/vyrec/Cargo.toml",
        working_dir: ".",
        requires_gpu_probe: true,
        evidence: "C frontend beta scope and clang parity gaps are explicit",
    },
    ReleaseValidationCommand {
        id: "vyrec-c-parser-production-build",
        area: "vyrec",
        command: "./cargo_full check -j1 -p vyre-libs --no-default-features --features c-parser",
        working_dir: ".",
        requires_gpu_probe: false,
        evidence: "C parser production build does not require CPU parity surfaces",
    },
    ReleaseValidationCommand {
        id: "vyrec-c-parser-parity-build",
        area: "vyrec",
        command: "./cargo_full check -j1 -p vyre-libs --no-default-features --features c-parser,cpu-parity",
        working_dir: ".",
        requires_gpu_probe: false,
        evidence: "C parser parity build keeps explicit CPU oracle surfaces available only under cpu-parity",
    },
    ReleaseValidationCommand {
        id: "vyrec-c-frontend-cpu-oracle-boundary",
        area: "vyrec",
        command: "./cargo_full test -j1 -p vyre-libs --no-default-features --features c-parser --test c_frontend_cpu_oracle_boundary",
        working_dir: ".",
        requires_gpu_probe: false,
        evidence: "C frontend production source rejects ungated reference and CPU oracle modules",
    },
    ReleaseValidationCommand {
        id: "vyrec-c-preprocess-gpu-resident-state",
        area: "vyrec",
        command: "./cargo_full test -j1 -p vyre-libs --no-default-features --features c-parser,cpu-parity --test c_preprocess_gpu_resident_state_contracts",
        working_dir: ".",
        requires_gpu_probe: true,
        evidence: "C preprocessor preserves resident GPU state and rejects host-resident preprocessing drift",
    },
    ReleaseValidationCommand {
        id: "vyrec-c-preprocess-cpu-api-boundary",
        area: "vyrec",
        command: "./cargo_full test -j1 -p vyre-libs --no-default-features --features c-parser --test preprocess_cpu_api_boundary",
        working_dir: ".",
        requires_gpu_probe: false,
        evidence: "C preprocessor CPU API surface remains explicit boundary code, not production fallback",
    },
    ReleaseValidationCommand {
        id: "vyrec-c-dialect-matrix",
        area: "vyrec",
        command: "./cargo_full test -j1 -p vyre-self-substrate c_dialect_matrix",
        working_dir: ".",
        requires_gpu_probe: false,
        evidence: "C dialect coverage records required parser, preprocessor, and semantic gates",
    },
    ReleaseValidationCommand {
        id: "vyrec-parser-semantic-safety",
        area: "vyrec",
        command: "./cargo_full test -j1 -p vyre-self-substrate parser_semantic_safety",
        working_dir: ".",
        requires_gpu_probe: false,
        evidence: "parser semantic safety gates reject silent semantic drift and raw cargo reproductions",
    },
    ReleaseValidationCommand {
        id: "vyrec-gpu-preprocessing-coverage",
        area: "vyrec",
        command: "./cargo_full test -j1 -p vyre-self-substrate gpu_preprocessing_coverage",
        working_dir: ".",
        requires_gpu_probe: true,
        evidence: "GPU preprocessing coverage records resident, macro, conditional, and payload stages",
    },
    ReleaseValidationCommand {
        id: "vyrec-diagnostic-comparison",
        area: "vyrec",
        command: "./cargo_full test -j1 -p vyre-self-substrate diagnostic_comparison",
        working_dir: ".",
        requires_gpu_probe: false,
        evidence: "diagnostic comparison coverage records actionable parser diagnostics against expected baselines",
    },
    ReleaseValidationCommand {
        id: "vyrec-clang-parity-dashboard",
        area: "vyrec",
        command: "./cargo_full test -j1 -p vyre-self-substrate clang_parity_dashboard",
        working_dir: ".",
        requires_gpu_probe: false,
        evidence: "clang-compatible, partial, and failing frontend feature classes remain explicit with cargo_full evidence",
    },
    ReleaseValidationCommand {
        id: "vyrec-linux-corpus-parity",
        area: "vyrec",
        command: "./cargo_full test -j1 -p vyre-self-substrate linux_corpus_parity",
        working_dir: ".",
        requires_gpu_probe: true,
        evidence: "Linux subsystem C corpus slices carry clang command evidence, commit provenance, cargo_full reproduction, and parser parity status",
    },
    ReleaseValidationCommand {
        id: "vyrec-c-parser-throughput-evidence",
        area: "vyrec",
        command: "./cargo_full test -j1 -p vyre-self-substrate c_parser_benchmark_evidence",
        working_dir: ".",
        requires_gpu_probe: true,
        evidence: "Linux subsystem C parser throughput artifact proves CUDA raw syntax parsing, resident cache use, zero host token upload, full token coverage, and 100x scaled baseline floor",
    },
    ReleaseValidationCommand {
        id: "structural-token-fact-graph",
        area: "vyre-self",
        command: "./cargo_full test -j1 -p vyre-self-substrate device_resident_token_fact_graph",
        working_dir: ".",
        requires_gpu_probe: true,
        evidence: "parser tokens and dataflow facts share a deterministic device-resident CSR graph layout",
    },
    ReleaseValidationCommand {
        id: "structural-incremental-invalidation",
        area: "vyre-self",
        command: "./cargo_full test -j1 -p vyre-self-substrate incremental_invalidation",
        working_dir: ".",
        requires_gpu_probe: false,
        evidence: "source edits recompute only overlapping token spans, macro regions, semantic scopes, and dependent facts",
    },
    ReleaseValidationCommand {
        id: "structural-multi-corpus-batching",
        area: "vyre-self",
        command: "./cargo_full test -j1 -p vyre-self-substrate multi_corpus_batching",
        working_dir: ".",
        requires_gpu_probe: true,
        evidence: "translation units batch by shared preprocessing, include residency, semantic graph, and device feature keys",
    },
    ReleaseValidationCommand {
        id: "structural-frontier-typed-ir",
        area: "vyre-self",
        command: "./cargo_full test -j1 -p vyre-self-substrate frontier_typed_ir",
        working_dir: ".",
        requires_gpu_probe: true,
        evidence: "parser, semantic, and dataflow work are represented as dependency-typed execution waves",
    },
    ReleaseValidationCommand {
        id: "structural-frontier-partitioning",
        area: "vyre-self",
        command: "./cargo_full test -j1 -p vyre-self-substrate frontier_partitioning",
        working_dir: ".",
        requires_gpu_probe: false,
        evidence: "contentious frontier fact updates are colored into conflict-free execution partitions",
    },
    ReleaseValidationCommand {
        id: "structural-diagnostic-aggregation",
        area: "vyre-self",
        command: "./cargo_full test -j1 -p vyre-self-substrate diagnostic_aggregation",
        working_dir: ".",
        requires_gpu_probe: true,
        evidence: "diagnostics are compacted before host readback with bounded device-to-host transfer size",
    },
    ReleaseValidationCommand {
        id: "structural-optimization-composition",
        area: "vyre-self",
        command: "./cargo_full test -j1 -p vyre-self-substrate optimization_composition_contracts",
        working_dir: ".",
        requires_gpu_probe: false,
        evidence: "optimization pass composition preserves required idempotence and commutativity laws",
    },
    ReleaseValidationCommand {
        id: "structural-benchmark-pass-selection",
        area: "vyre-self",
        command: "./cargo_full test -j1 -p vyre-self-substrate optimization_pass_selection",
        working_dir: ".",
        requires_gpu_probe: false,
        evidence: "expensive optimization passes fire only when benchmark-derived workload statistics justify them",
    },
    ReleaseValidationCommand {
        id: "optimization-control-plane",
        area: "vyre-self",
        command: "./cargo_full test -j1 -p vyre-self-substrate optimization_registry",
        working_dir: ".",
        requires_gpu_probe: false,
        evidence: "100 plus optimizations remain discoverable and phase ordered",
    },
    ReleaseValidationCommand {
        id: "optimization-release-evidence",
        area: "vyre-self",
        command: "./cargo_full test -j1 -p vyre-self-substrate optimization_release_evidence",
        working_dir: ".",
        requires_gpu_probe: true,
        evidence: "100 plus optimization registry entries are backed by CUDA artifacts, 14 families, 12k plus cases, and concrete integration sources",
    },
    ReleaseValidationCommand {
        id: "cross-crate-perf-contracts",
        area: "vyre-self",
        command: "./cargo_full test -j1 -p vyre-self-substrate cross_crate_perf_contracts",
        working_dir: ".",
        requires_gpu_probe: true,
        evidence: "Vyrec and Dataflow analysis consumer cannot disable required CUDA optimizations",
    },
    ReleaseValidationCommand {
        id: "analysis-coverage",
        area: "dataflow",
        command: "./cargo_full test -j1 -p vyre-self-substrate analysis_coverage",
        working_dir: ".",
        requires_gpu_probe: false,
        evidence: "Dataflow analysis coverage records required dataflow analyses and cargo_full evidence",
    },
    ReleaseValidationCommand {
        id: "graph-layout-coverage",
        area: "dataflow",
        command: "./cargo_full test -j1 -p vyre-self-substrate graph_layout_coverage",
        working_dir: ".",
        requires_gpu_probe: false,
        evidence: "Dataflow graph layout coverage records resident graph layout and cache gates",
    },
    ReleaseValidationCommand {
        id: "semantic-parity-coverage",
        area: "vyre-self",
        command: "./cargo_full test -j1 -p vyre-self-substrate semantic_parity_coverage",
        working_dir: ".",
        requires_gpu_probe: false,
        evidence: "semantic parity coverage records exactness, gap, and cargo_full reproduction evidence",
    },
    ReleaseValidationCommand {
        id: "release-test-taxonomy-coverage",
        area: "release",
        command: "./cargo_full test -j1 -p vyre-self-substrate test_taxonomy_coverage",
        working_dir: ".",
        requires_gpu_probe: false,
        evidence: "major modules carry unit adversarial property benchmark fuzz and gap evidence",
    },
    ReleaseValidationCommand {
        id: "release-hostile-input-coverage",
        area: "release",
        command: "./cargo_full test -j1 -p vyre-self-substrate hostile_input_coverage",
        working_dir: ".",
        requires_gpu_probe: false,
        evidence: "hostile malformed and oversized inputs are covered by explicit tests",
    },
    ReleaseValidationCommand {
        id: "release-benchmark-baselines",
        area: "release",
        command: "./cargo_full test -j1 -p vyre-self-substrate benchmark_baselines",
        working_dir: ".",
        requires_gpu_probe: true,
        evidence: "performance claims map to committed CUDA thresholds and cargo_full commands",
    },
    ReleaseValidationCommand {
        id: "release-allocation-regression",
        area: "release",
        command: "./cargo_full test -j1 -p vyre-self-substrate allocation_regression",
        working_dir: ".",
        requires_gpu_probe: true,
        evidence: "hot loops and repeated dispatch reject host allocations, device allocations, and output capacity growth",
    },
    ReleaseValidationCommand {
        id: "release-public-api-doctests",
        area: "release",
        command: "./cargo_full test -j1 -p vyre-self-substrate public_api_doctest_gate",
        working_dir: ".",
        requires_gpu_probe: false,
        evidence: "public examples compile against real APIs and avoid internals",
    },
    ReleaseValidationCommand {
        id: "release-gpu-evidence",
        area: "release",
        command: "./cargo_full test -j1 -p vyre-self-substrate release_gpu_evidence",
        working_dir: ".",
        requires_gpu_probe: true,
        evidence: "release artifacts record nvidia-smi CUDA hardware driver and cargo_full commands",
    },
    ReleaseValidationCommand {
        id: "release-cuda-ptx-pattern-evidence",
        area: "release",
        command: "./cargo_full test -j1 -p vyre-self-substrate cuda_ptx_pattern_evidence",
        working_dir: ".",
        requires_gpu_probe: true,
        evidence: "CUDA PTX release artifact proves vectorized memory ops, predication, tensor-core emission, async copy, and PTX source-cache evidence",
    },
    ReleaseValidationCommand {
        id: "release-gap-findings",
        area: "release",
        command: "./cargo_full test -j1 -p vyre-self-substrate release_gap_findings",
        working_dir: ".",
        requires_gpu_probe: false,
        evidence: "missing clang or dataflow parity is tracked as reproducible gap findings",
    },
    ReleaseValidationCommand {
        id: "release-crate-metadata-readiness",
        area: "release",
        command: "./cargo_full test -j1 -p vyre-self-substrate crate_metadata_readiness",
        working_dir: ".",
        requires_gpu_probe: false,
        evidence: "public crates have complete metadata and exact publish commands",
    },
    ReleaseValidationCommand {
        id: "release-launch-sequence",
        area: "release",
        command: "./cargo_full test -j1 -p vyre-self-substrate release_launch_sequence",
        working_dir: ".",
        requires_gpu_probe: false,
        evidence: "final release order is cargo_full green then cargo_full publish then public repos then git push and tags",
    },
    ReleaseValidationCommand {
        id: "release-checklist-gate",
        area: "release",
        command: "./cargo_full test -j1 -p vyre-self-substrate release_checklist_gate",
        working_dir: ".",
        requires_gpu_probe: false,
        evidence: "release checklist requires cargo_full GPU fuzz gap benchmarks docs metadata and public API evidence",
    },
    ReleaseValidationCommand {
        id: "release-completion-audit-honesty",
        area: "release",
        command: "./cargo_full test -j1 -p vyre-self-substrate release_completion_audit",
        working_dir: ".",
        requires_gpu_probe: false,
        evidence: "completion audit evidence must target the active paradigm-shift plan and include cargo_full publish, public repo, branch push, and tag push proof",
    },
    ReleaseValidationCommand {
        id: "release-paradigm-shift-plan-audit",
        area: "release",
        command: "./cargo_full test -j1 -p vyre-self-substrate paradigm_shift_plan_audit",
        working_dir: ".",
        requires_gpu_probe: false,
        evidence: "active paradigm-shift 100-item plan maps every numbered item to current release validation gates and rejects stale completion artifacts",
    },
    ReleaseValidationCommand {
        id: "release-deep-review-gate",
        area: "release",
        command: "./cargo_full test -j1 -p vyre-self-substrate deep_review_gate",
        working_dir: ".",
        requires_gpu_probe: false,
        evidence: "deep personal review evidence covers touched public crate files and fixed findings",
    },
    ReleaseValidationCommand {
        id: "release-scope-docs",
        area: "release",
        command: "./cargo_full test -j1 -p vyre-self-substrate release_scope_docs",
        working_dir: ".",
        requires_gpu_probe: false,
        evidence: "release docs state Vyrec beta scope and active C compiler development honestly",
    },
];

/// Validate the release matrix shape before publishing.

pub fn validate_release_validation_matrix() -> Result<(), String> {
    if GPU_PROBE_COMMAND != "nvidia-smi" {
        return Err(
            "GPU probe must be nvidia-smi. Fix: release validation must fail loudly when NVIDIA GPUs are hidden."
                .to_string(),
        );
    }

    for command in RELEASE_VALIDATION_MATRIX {
        validate_command(*command)?;
    }

    require_area("vyre")?;
    require_area("vyre-cuda")?;
    require_area("dataflow")?;
    require_area("vyrec")?;
    require_area("release")?;
    require_unique_ids()?;
    require_cuda_gates_probe_gpu()?;
    Ok(())
}

/// Expand release gates into the exact command sequence a release runner must execute.
pub fn release_validation_execution_plan() -> Result<Vec<ReleaseValidationStep>, String> {
    validate_release_validation_matrix()?;
    let mut steps = Vec::new();
    for gate in RELEASE_VALIDATION_MATRIX {
        if gate.requires_gpu_probe {
            steps.push(ReleaseValidationStep {
                gate_id: gate.id,
                working_dir: gate.working_dir,
                command: GPU_PROBE_COMMAND,
                is_gpu_probe: true,
            });
        }
        steps.push(ReleaseValidationStep {
            gate_id: gate.id,
            working_dir: gate.working_dir,
            command: gate.command,
            is_gpu_probe: false,
        });
    }
    Ok(steps)
}

fn validate_command(command: ReleaseValidationCommand) -> Result<(), String> {
    for (field, value) in [
        ("id", command.id),
        ("area", command.area),
        ("command", command.command),
        ("working_dir", command.working_dir),
        ("evidence", command.evidence),
    ] {
        if value.trim().is_empty() {
            return Err(format!(
                "release validation command `{}` has empty {field}. Fix: every gate needs an id, area, cargo_full command, and evidence contract.",
                command.id
            ));
        }
    }

    if !command.command.starts_with("./cargo_full ") {
        return Err(format!(
            "release validation command `{}` does not use ./cargo_full. Fix: avoid raw cargo in release gates.",
            command.id
        ));
    }
    if command.working_dir != "." && !vyre_workspace_root().join(command.working_dir).is_dir() {
        return Err(format!(
            "release validation command `{}` has missing working_dir `{}`. Fix: adjacent crate gates must name a real directory relative to the Vyre workspace.",
            command.id, command.working_dir
        ));
    }
    validate_manifest_path(command)?;
    if command.command.contains("cargo test") || command.command.contains("cargo +") {
        return Err(format!(
            "release validation command `{}` contains raw cargo. Fix: use ./cargo_full.",
            command.id
        ));
    }
    validate_features(command)?;
    validate_test_target(command)?;
    validate_bench_target(command)?;
    let filter_count = positional_test_filter_count(command.command);
    if filter_count > 1 {
        return Err(format!(
            "release validation command `{}` has {filter_count} positional test filters. Fix: cargo test accepts at most one TESTNAME filter; split this gate into separate commands.",
            command.id
        ));
    }
    validate_positional_test_filter_anchor(command)?;
    Ok(())
}

fn validate_manifest_path(command: ReleaseValidationCommand) -> Result<(), String> {
    let tokens = command.command.split_whitespace().collect::<Vec<_>>();
    for (index, token) in tokens.iter().enumerate() {
        if *token != "--manifest-path" {
            continue;
        }
        let Some(path) = tokens.get(index + 1) else {
            return Err(format!(
                "release validation command `{}` has --manifest-path without a value. Fix: point it at the adjacent crate Cargo.toml.",
                command.id
            ));
        };
        let full_path = resolve_manifest_alias(command, path);
        if !full_path.is_file() {
            return Err(format!(
                "release validation command `{}` has missing manifest path `{path}`. Fix: update the adjacent crate path before trusting this release gate.",
                command.id
            ));
        }
    }
    Ok(())
}

fn validate_test_target(command: ReleaseValidationCommand) -> Result<(), String> {
    let tokens = command.command.split_whitespace().collect::<Vec<_>>();
    for (index, token) in tokens.iter().enumerate() {
        if *token != "--test" {
            continue;
        }
        let Some(test_name) = tokens.get(index + 1) else {
            return Err(format!(
                "release validation command `{}` has --test without a value. Fix: name the integration test target explicitly.",
                command.id
            ));
        };
        let root = source_root_for_command(command)?;
        let test_path = root.join("tests").join(format!("{test_name}.rs"));
        if !test_path.is_file() {
            return Err(format!(
                "release validation command `{}` references missing integration test `{test_name}` at {}. Fix: update the --test target before trusting this release gate.",
                command.id,
                test_path.display()
            ));
        }
    }
    Ok(())
}

fn validate_bench_target(command: ReleaseValidationCommand) -> Result<(), String> {
    let tokens = command.command.split_whitespace().collect::<Vec<_>>();
    for (index, token) in tokens.iter().enumerate() {
        if *token != "--bench" {
            continue;
        }
        let Some(bench_name) = tokens.get(index + 1) else {
            return Err(format!(
                "release validation command `{}` has --bench without a value. Fix: name the benchmark target explicitly.",
                command.id
            ));
        };
        let root = source_root_for_command(command)?;
        let bench_path = root.join("benches").join(format!("{bench_name}.rs"));
        if !bench_path.is_file() {
            return Err(format!(
                "release validation command `{}` references missing benchmark `{bench_name}` at {}. Fix: update the --bench target before trusting this release gate.",
                command.id,
                bench_path.display()
            ));
        }
    }
    Ok(())
}

fn validate_features(command: ReleaseValidationCommand) -> Result<(), String> {
    let tokens = command.command.split_whitespace().collect::<Vec<_>>();
    for (index, token) in tokens.iter().enumerate() {
        if *token != "--features" {
            continue;
        }
        let Some(features) = tokens.get(index + 1) else {
            return Err(format!(
                "release validation command `{}` has --features without a value. Fix: list real Cargo features or remove the flag.",
                command.id
            ));
        };
        let manifest = manifest_path_for_command(command)?;
        let manifest_text = std::fs::read_to_string(&manifest).map_err(|error| {
            format!(
                "release validation command `{}` could not read manifest {}: {error}",
                command.id,
                manifest.display()
            )
        })?;
        for feature in features.split(',').filter(|feature| !feature.is_empty()) {
            if !manifest_defines_feature(&manifest_text, feature) {
                return Err(format!(
                    "release validation command `{}` references missing feature `{feature}` in {}. Fix: update the feature list before trusting this gate.",
                    command.id,
                    manifest.display()
                ));
            }
        }
    }
    Ok(())
}

fn manifest_path_for_command(
    command: ReleaseValidationCommand,
) -> Result<std::path::PathBuf, String> {
    let tokens = command.command.split_whitespace().collect::<Vec<_>>();
    if let Some(index) = tokens.iter().position(|token| *token == "--manifest-path") {
        let Some(path) = tokens.get(index + 1) else {
            return Err(format!(
                "release validation command `{}` has --manifest-path without a value.",
                command.id
            ));
        };
        return Ok(resolve_manifest_alias(command, path));
    }
    if let Some(index) = tokens
        .iter()
        .position(|token| *token == "-p" || *token == "--package")
    {
        let Some(package) = tokens.get(index + 1) else {
            return Err(format!(
                "release validation command `{}` has package flag without a value.",
                command.id
            ));
        };
        let root = package_root(package).ok_or_else(|| {
            format!(
                "release validation command `{}` references unknown package `{package}`.",
                command.id
            )
        })?;
        return Ok(root.join("Cargo.toml"));
    }
    Ok(vyre_workspace_root().join("Cargo.toml"))
}

fn manifest_defines_feature(manifest: &str, feature: &str) -> bool {
    let mut in_features = false;
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_features = trimmed == "[features]";
            continue;
        }
        if !in_features || trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some((name, _)) = trimmed.split_once('=') {
            let name = name.trim().trim_matches('"');
            if name == feature {
                return true;
            }
        }
    }
    false
}

fn validate_positional_test_filter_anchor(command: ReleaseValidationCommand) -> Result<(), String> {
    let Some(filter) = positional_test_filter(command.command) else {
        return Ok(());
    };
    let source_root = source_root_for_command(command)?;
    if source_tree_contains(&source_root, filter) {
        Ok(())
    } else {
        Err(format!(
            "release validation command `{}` uses positional test filter `{filter}` but no matching source anchor was found under {}. Fix: update the filter or use an explicit --test target so Cargo cannot silently run zero tests.",
            command.id,
            source_root.display()
        ))
    }
}

fn source_root_for_command(
    command: ReleaseValidationCommand,
) -> Result<std::path::PathBuf, String> {
    let tokens = command.command.split_whitespace().collect::<Vec<_>>();
    if let Some(index) = tokens.iter().position(|token| *token == "--manifest-path") {
        let Some(path) = tokens.get(index + 1) else {
            return Err(format!(
                "release validation command `{}` has --manifest-path without a value.",
                command.id
            ));
        };
        let manifest = resolve_manifest_alias(command, path);
        return Ok(manifest
            .parent()
            .ok_or_else(|| {
                format!(
                    "release validation command `{}` has manifest path without parent.",
                    command.id
                )
            })?
            .to_path_buf());
    }
    if let Some(index) = tokens
        .iter()
        .position(|token| *token == "-p" || *token == "--package")
    {
        let Some(package) = tokens.get(index + 1) else {
            return Err(format!(
                "release validation command `{}` has package flag without a value.",
                command.id
            ));
        };
        return package_root(package).ok_or_else(|| {
            format!(
                "release validation command `{}` references unknown package `{package}`. Fix: add an explicit package root mapping before trusting this release gate.",
                command.id
            )
        });
    }
    Ok(vyre_workspace_root().to_path_buf())
}

fn package_root(package: &str) -> Option<std::path::PathBuf> {
    let relative = match package {
        "vyre" => "vyre-core",
        "vyre-driver-cuda" => "vyre-driver-cuda",
        "vyre-libs" => "vyre-libs",
        "vyre-lints" => "vyre-lints",
        "vyre-self-substrate" => "vyre-self-substrate",
        _ => package,
    };
    let root = vyre_workspace_root().join(relative);
    root.is_dir().then_some(root)
}

fn resolve_manifest_alias(command: ReleaseValidationCommand, path: &str) -> std::path::PathBuf {
    let requested = vyre_workspace_root().join(command.working_dir).join(path);
    if path != DATAFLOW_MANIFEST_ALIAS {
        return requested;
    }
    if let Some(manifest) = dataflow_manifest_from_env() {
        return manifest;
    }
    discover_dataflow_consumer_manifest().unwrap_or(requested)
}

fn dataflow_manifest_from_env() -> Option<std::path::PathBuf> {
    let raw = std::env::var_os(DATAFLOW_MANIFEST_ENV)?;
    if raw.is_empty() {
        return None;
    }
    Some(std::path::PathBuf::from(raw))
}

fn discover_dataflow_consumer_manifest() -> Option<std::path::PathBuf> {
    let dataflow_root = vyre_workspace_root().join("../../../dataflow");
    let entries = std::fs::read_dir(dataflow_root).ok()?;
    for entry in entries.flatten() {
        let crate_root = entry.path();
        let manifest = crate_root.join("Cargo.toml");
        if !manifest.is_file() {
            continue;
        }
        let tests = crate_root.join("tests");
        if tests.join("df_property_ifds.rs").is_file()
            && tests.join("df_parity_exact_primitives.rs").is_file()
            && tests.join("df_scale_oracle_no_oom.rs").is_file()
        {
            return Some(manifest);
        }
    }
    None
}

fn source_tree_contains(root: &std::path::Path, needle: &str) -> bool {
    let mut stack = vec![root.to_path_buf()];
    while let Some(path) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&path) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().and_then(std::ffi::OsStr::to_str) != Some("rs") {
                continue;
            }
            if path
                .file_stem()
                .and_then(std::ffi::OsStr::to_str)
                .is_some_and(|stem| stem == needle)
            {
                return true;
            }
            if std::fs::read_to_string(&path)
                .map(|source| source.contains(needle))
                .unwrap_or(false)
            {
                return true;
            }
        }
    }
    false
}

fn vyre_workspace_root() -> &'static std::path::Path {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap_or_else(|| std::path::Path::new(env!("CARGO_MANIFEST_DIR")))
}

fn positional_test_filter_count(command: &str) -> usize {
    positional_test_filters(command).len()
}

fn positional_test_filter(command: &str) -> Option<&str> {
    let filters = positional_test_filters(command);
    if filters.len() == 1 {
        filters.first().copied()
    } else {
        None
    }
}

fn positional_test_filters(command: &str) -> Vec<&str> {
    let mut filters = Vec::new();
    let mut skip_next = false;
    for (index, token) in command.split_whitespace().enumerate() {
        if index < 2 || skip_next {
            skip_next = false;
            continue;
        }
        match token {
            "-p" | "--package" | "--test" | "--bench" | "--manifest-path" | "--features" | "-j" => {
                skip_next = true;
            }
            "--" => break,
            token if token.starts_with('-') => {}
            _ => filters.push(token),
        }
    }
    filters
}

fn require_area(area: &str) -> Result<(), String> {
    if RELEASE_VALIDATION_MATRIX
        .iter()
        .any(|command| command.area == area)
    {
        Ok(())
    } else {
        Err(format!(
            "release validation matrix has no `{area}` gate. Fix: every release area needs at least one command."
        ))
    }
}

fn require_cuda_gates_probe_gpu() -> Result<(), String> {
    let missing = RELEASE_VALIDATION_MATRIX
        .iter()
        .filter(|command| cuda_release_gate(command) && !command.requires_gpu_probe)
        .map(|command| command.id)
        .collect::<Vec<_>>();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "CUDA release gates missing GPU probe: {}. Fix: CUDA is the release path; run nvidia-smi before every CUDA-identified gate, even when the owning area is a higher-level release or self-substrate gate.",
            missing.join(", ")
        ))
    }
}

fn cuda_release_gate(command: &ReleaseValidationCommand) -> bool {
    command.area == "vyre-cuda"
        || command.id.contains("cuda")
        || command.command.contains("cuda")
        || command.evidence.contains("CUDA")
}

fn require_unique_ids() -> Result<(), String> {
    let mut ids = std::collections::BTreeSet::new();
    for command in RELEASE_VALIDATION_MATRIX {
        if !ids.insert(command.id) {
            return Err(format!(
                "release validation matrix has duplicate id `{}`. Fix: every release gate needs a unique evidence key.",
                command.id
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_matrix_uses_gpu_probe_and_cargo_full() {
        validate_release_validation_matrix().expect("Fix: release matrix must be valid");
    }

    #[test]
    fn release_matrix_uses_generic_dataflow_manifest_alias() {
        const SOURCE: &str = include_str!("release_validation_matrix.rs");
        let forbidden_consumer_name = concat!("we", "ir");
        assert!(
            !SOURCE
                .to_ascii_lowercase()
                .contains(forbidden_consumer_name),
            "release matrix must discover external dataflow consumers without naming one"
        );

        let mut dataflow_gate_count = 0usize;
        for command in RELEASE_VALIDATION_MATRIX {
            if command.area != "dataflow" || !command.command.contains("--manifest-path") {
                continue;
            }
            dataflow_gate_count += 1;
            assert!(
                command.command.contains(DATAFLOW_MANIFEST_ALIAS),
                "dataflow gate `{}` must use the generic manifest alias: {}",
                command.id,
                command.command
            );
        }
        assert!(
            dataflow_gate_count >= 10,
            "release matrix must keep dataflow validation broad while staying consumer-neutral"
        );
    }

    #[test]
    fn release_matrix_covers_cuda_heavy_gates() {
        for required in [
            "vyre-cuda-resident-dispatch",
            "vyre-cuda-capability-contracts",
            "vyre-cuda-cooperative-launch",
            "vyre-cuda-module-cache",
            "vyre-cuda-pipeline-modularity",
            "vyre-cuda-frontier-queue",
            "vyre-cuda-device-work-queue",
            "vyre-cuda-launch-fusion",
            "vyre-cuda-multi-query-execution",
            "vyre-cuda-result-compaction",
            "vyre-cuda-device-diagnostic-aggregation",
            "vyre-cuda-kernel-failure-diagnostics",
            "vyre-cuda-benchmark-pass-selection",
            "vyre-cuda-self-optimizer-e2e",
            "vyre-cuda-self-optimizer-const-prop",
            "vyre-cuda-self-optimizer-cse",
            "vyre-cuda-self-optimizer-licm",
            "vyre-cuda-self-optimizer-dead-branch",
            "vyre-cuda-self-optimizer-pattern-match",
            "vyre-cuda-self-optimizer-pipeline-resident",
            "optimization-release-evidence",
            "vyre-cuda-megakernel-scheduler",
            "vyre-cuda-convergence",
            "vyre-cuda-megakernel-speedup",
        ] {
            assert!(
                RELEASE_VALIDATION_MATRIX
                    .iter()
                    .any(|command| command.id == required),
                "Fix: CUDA is the release path and needs dedicated validation gate `{required}`."
            );
        }
    }

    #[test]
    fn release_matrix_requires_gpu_probe_for_every_cuda_gate() {
        require_cuda_gates_probe_gpu()
            .expect("Fix: every CUDA release gate must require GPU probe");
    }

    #[test]
    fn release_matrix_has_unique_gate_ids() {
        require_unique_ids().expect("Fix: release matrix gate ids must be unique");
    }

    #[test]
    fn release_matrix_execution_plan_inserts_gpu_probe_before_required_gates() {
        let plan = release_validation_execution_plan().expect("Fix: release execution plan");
        for gate in RELEASE_VALIDATION_MATRIX
            .iter()
            .filter(|gate| gate.requires_gpu_probe)
        {
            let gate_index = plan
                .iter()
                .position(|step| step.gate_id == gate.id && !step.is_gpu_probe)
                .unwrap_or_else(|| panic!("missing gate command for {}", gate.id));
            assert!(
                gate_index > 0,
                "GPU-required gate `{}` must have a preceding probe",
                gate.id
            );
            let probe = plan[gate_index - 1];
            assert!(
                probe.is_gpu_probe
                    && probe.gate_id == gate.id
                    && probe.command == GPU_PROBE_COMMAND,
                "GPU-required gate `{}` must be immediately preceded by nvidia-smi",
                gate.id
            );
        }
    }

    #[test]
    fn release_matrix_runs_cpu_fallback_lint() {
        assert!(
            RELEASE_VALIDATION_MATRIX.iter().any(|command| {
                command.command.contains("--check-production-cpu-fallbacks")
                    && command.command.starts_with("./cargo_full run")
            }),
            "Fix: release matrix must run the production CPU/reference fallback lint."
        );
    }

    #[test]
    fn release_matrix_covers_architecture_invariant_gates() {
        for required in [
            "vyre-gpu-probe-contract",
            "vyre-memory-ownership-contract",
            "vyre-architecture-boundary-map",
            "vyre-contributor-module-map",
            "vyre-public-api-boundary",
        ] {
            assert!(
                RELEASE_VALIDATION_MATRIX
                    .iter()
                    .any(|command| command.id == required),
                "Fix: release matrix missing architecture invariant gate `{required}`."
            );
        }
    }

    #[test]
    fn release_matrix_covers_c_parser_production_and_parity_boundaries() {
        for required in [
            "vyrec-c-parser-production-build",
            "vyrec-c-parser-parity-build",
            "vyrec-c-frontend-cpu-oracle-boundary",
            "vyrec-c-preprocess-gpu-resident-state",
            "vyrec-c-preprocess-cpu-api-boundary",
            "vyrec-c-dialect-matrix",
            "vyrec-parser-semantic-safety",
            "vyrec-gpu-preprocessing-coverage",
            "vyrec-diagnostic-comparison",
            "vyrec-clang-parity-dashboard",
            "vyrec-linux-corpus-parity",
            "vyrec-c-parser-throughput-evidence",
        ] {
            assert!(
                RELEASE_VALIDATION_MATRIX
                    .iter()
                    .any(|command| command.id == required),
                "Fix: release matrix missing C parser gate `{required}`."
            );
        }
    }

    #[test]
    fn release_matrix_covers_release_readiness_gates() {
        for required in [
            "release-test-taxonomy-coverage",
            "release-hostile-input-coverage",
            "release-benchmark-baselines",
            "release-public-api-doctests",
            "release-gpu-evidence",
            "release-cuda-ptx-pattern-evidence",
            "release-gap-findings",
            "release-crate-metadata-readiness",
            "release-launch-sequence",
            "release-checklist-gate",
            "release-completion-audit-honesty",
            "release-paradigm-shift-plan-audit",
            "release-deep-review-gate",
            "release-scope-docs",
        ] {
            assert!(
                RELEASE_VALIDATION_MATRIX
                    .iter()
                    .any(|command| command.id == required),
                "Fix: release matrix missing readiness gate `{required}`."
            );
        }
    }

    #[test]
    fn release_matrix_covers_dataflow_and_semantic_coverage_gates() {
        for required in [
            "adversarial-oracles",
            "fuzz-bitset-oracles",
            "gap-bitset-oracle-edges",
            "property-points-to",
            "property-ifds",
            "property-slice",
            "property-reaching-escapes",
            "exact-primitive-parity",
            "scale-oracle-no-oom",
            "resident-fixed-point-benchmark",
            "ifds-direct-resident-benchmark",
            "analysis-coverage",
            "graph-layout-coverage",
            "semantic-parity-coverage",
        ] {
            assert!(
                RELEASE_VALIDATION_MATRIX
                    .iter()
                    .any(|command| command.id == required),
                "Fix: release matrix missing semantic/dataflow coverage gate `{required}`."
            );
        }
    }

    #[test]
    fn release_matrix_covers_structural_innovation_gates() {
        for required in [
            "structural-token-fact-graph",
            "structural-incremental-invalidation",
            "structural-multi-corpus-batching",
            "structural-frontier-typed-ir",
            "structural-frontier-partitioning",
            "structural-diagnostic-aggregation",
            "structural-optimization-composition",
            "structural-benchmark-pass-selection",
            "release-allocation-regression",
        ] {
            assert!(
                RELEASE_VALIDATION_MATRIX
                    .iter()
                    .any(|command| command.id == required),
                "Fix: release matrix missing structural innovation gate `{required}`."
            );
        }
    }

    #[test]
    fn release_matrix_rejects_raw_cargo_commands() {
        let err = validate_command(ReleaseValidationCommand {
            id: "bad",
            area: "vyre",
            command: "cargo test -p vyre",
            working_dir: ".",
            requires_gpu_probe: true,
            evidence: "bad command",
        })
        .expect_err("raw cargo must be rejected");

        assert!(err.contains("./cargo_full"), "{err}");
    }

    #[test]
    fn release_matrix_rejects_multiple_positional_filters() {
        let err = validate_command(ReleaseValidationCommand {
            id: "bad-filters",
            area: "vyrec",
            command: "./cargo_full test -j1 -p vyrec beta clang parity",
            working_dir: ".",
            requires_gpu_probe: true,
            evidence: "bad filters",
        })
        .expect_err("multiple cargo test filters should be rejected");

        assert!(err.contains("at most one TESTNAME filter"), "{err}");
    }

    #[test]
    fn release_matrix_rejects_unanchored_positional_filter() {
        let err = validate_command(ReleaseValidationCommand {
            id: "stale-filter",
            area: "vyre-self",
            command: concat!(
                "./cargo_full test -j1 -p vyre-self-substrate ",
                "definitely_missing_",
                "release_filter"
            ),
            working_dir: ".",
            requires_gpu_probe: false,
            evidence: "stale filter",
        })
        .expect_err("stale positional filters must be rejected");

        assert!(err.contains("source anchor"), "{err}");
    }

    #[test]
    fn release_matrix_rejects_missing_test_target() {
        let err = validate_command(ReleaseValidationCommand {
            id: "missing-test-target",
            area: "vyre",
            command: "./cargo_full test -j1 -p vyre --test definitely_missing_target",
            working_dir: ".",
            requires_gpu_probe: true,
            evidence: "missing test target",
        })
        .expect_err("missing integration test targets must be rejected");

        assert!(err.contains("missing integration test"), "{err}");
    }

    #[test]
    fn release_matrix_rejects_missing_benchmark_target() {
        let err = validate_command(ReleaseValidationCommand {
            id: "missing-bench-target",
            area: "dataflow",
            command: "./cargo_full bench -j1 --manifest-path @dataflow-manifest --bench definitely_missing_benchmark",
            working_dir: ".",
            requires_gpu_probe: true,
            evidence: "missing benchmark",
        })
        .expect_err("missing benchmark targets must be rejected");

        assert!(err.contains("missing benchmark"), "{err}");
    }

    #[test]
    fn release_matrix_rejects_missing_feature() {
        let err = validate_command(ReleaseValidationCommand {
            id: "missing-feature",
            area: "vyrec",
            command: "./cargo_full check -j1 -p vyre-libs --features c-parser,definitely_missing_feature",
            working_dir: ".",
            requires_gpu_probe: false,
            evidence: "missing feature",
        })
        .expect_err("missing Cargo features must be rejected");

        assert!(err.contains("missing feature"), "{err}");
    }

    #[test]
    fn release_matrix_rejects_missing_manifest_path() {
        let err = validate_command(ReleaseValidationCommand {
            id: "missing-manifest",
            area: "dataflow",
            command: "./cargo_full test -j1 --manifest-path ../../../missing/Cargo.toml",
            working_dir: ".",
            requires_gpu_probe: true,
            evidence: "missing manifest",
        })
        .expect_err("missing manifest paths must be rejected");

        assert!(err.contains("missing manifest path"), "{err}");
    }
}
