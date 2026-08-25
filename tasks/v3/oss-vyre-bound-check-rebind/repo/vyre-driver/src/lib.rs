#![forbid(unsafe_code)]
#![allow(unused_imports)]
#![allow(
    clippy::only_used_in_recursion,
    clippy::result_unit_err,
    clippy::module_inception
)]
//! vyre-driver  -  substrate-agnostic backend machinery.
//!
//! Registry, runtime, pipeline, routing, diagnostics, and the VyreBackend
//! trait. Concrete backend crates depend on this crate and contribute
//! lowerings via the inventory collection mechanism.

// missing_docs is enforced workspace-wide via [workspace.lints.rust].
// vyre-driver inherits that floor; do not re-allow it here.

/// Backend-neutral checked arithmetic and atomic accounting primitives.
pub mod accounting;
/// Backend-neutral fallible allocation reservation helpers.
pub mod allocation;
/// Backend-neutral ahead-of-time emission registry.
pub mod aot;
/// Independent-arm detection for queue-parallel dispatch (ROADMAP D2).
/// Pure set arithmetic over (reads, writes) summaries; the dispatcher
/// uses `can_dispatch_concurrently` to decide whether two megakernel
/// arms can launch on independent backend queues or streams.
pub mod arm_independence;
/// Async-copy / kernel-overlap decision policy (ROADMAP D3). Pure
/// per-slot read/write conflict check that decides whether an H2D
/// copy can run on a side stream concurrently with a downstream
/// kernel.
pub mod async_copy_overlap;
/// Persistent autotuning record store (ROADMAP I3).
pub mod autotune_store;
/// VyreBackend trait, BackendError, capability records, validation.
pub mod backend;
/// Backend-neutral benchmark-driven optimization pass selection.
pub mod benchmark_pass_selection;
/// Backend-neutral program binding plans.
pub mod binding;
/// Bindless buffers / textures decision policy (ROADMAP D9). Decides
/// whether to use a bindless descriptor array or traditional per-
/// resource bindings, given the kernel's resource count and the
/// backend's bindless support level (Full / Static / Unsupported).
pub mod bindless_policy;
/// Backend-neutral cache eviction policy.
pub mod cache_eviction;
/// N5 substrate: spec-cache eviction with frequency × recency heat
/// decay. Used by F1/F3 cache layers when capacity pressure
/// triggers  -  `entries_to_evict(stats, capacity, now)` returns the
/// evictable IDs in eviction order (lowest heat first).
pub mod cache_eviction_heat;
/// Backend-neutral cache invalidation policy.
pub mod cache_invalidation;
/// Pre-recorded command reuse decision policy (ROADMAP D4). Decides
/// whether to record a native command sequence once and replay it for
/// repeated identical dispatches, based on per-launch overhead vs
/// record + replay overhead.
pub mod command_reuse_policy;
/// Device-conditioned e-graph extraction helpers.
/// Backend-neutral device-side convergence planning.
pub mod device_convergence;
/// Backend-neutral device diagnostic aggregation planning.
pub mod device_diagnostic_aggregation;
pub mod device_extraction;
/// Backend-neutral device capability profile and projections.
pub mod device_profile;
/// Tier-B device signature TOML loader.
pub mod device_signature;
/// Backend-neutral device-side work queue planning.
pub mod device_work_queue;
/// Structured, machine-readable diagnostic rendering.
pub mod diagnostics;
/// Bundled D-series + I2 policy invocation. One-shot eval of every
/// dispatch-side decision substrate so the runtime threads a single
/// `DispatchPolicyVerdict` instead of six per-substrate verdicts.
pub mod dispatch_policy;
/// Backend-neutral dispatch-shape comparison helpers.
pub mod dispatch_shape;
/// Backend-neutral evidence bundles and source provenance.
pub mod evidence;
/// Device-profile-aware extraction cost helpers (ROADMAP A7).
pub mod extraction_cost;
/// Backend-neutral fixpoint-iteration resolution.
pub mod fixpoint_iterations;
/// Cross-dispatch fusion decision types and pure analysis.
pub mod fusion;
/// Backend-neutral replayable graph-capture binding planning.
pub mod graph_capture;
/// Backend-neutral exact-input identity keys for replay caches.
pub mod input_identity;
/// Backend-neutral monotonic ordering helpers for staging hot paths.
pub mod ordering;
/// Backend-neutral fallible output-slot vector management.
pub mod output_slots;
/// Push-constant / tiny-param inlining decision policy (ROADMAP D7).
/// Backends consume `decide_param_inlining` to choose between inlined
/// launch metadata and a uniform buffer upload, based on a per-backend
/// [`crate::param_inlining::ParamInliningPolicy`].
pub mod param_inlining;
/// Persistent-kernel-mode decision policy (ROADMAP D1). Decides
/// whether to replace N small kernel launches with one persistent
/// kernel that polls a device-side work queue, based on measured
/// per-launch overhead and persistent-setup cost. Pure decision,
/// no Program walk.
pub mod persistent_kernel_policy;
/// Compiled-pipeline cache, dispatch config, batched dispatch.
pub mod pipeline;
/// N4 substrate: cross-pipeline disjoint-binding fusion analysis.
/// Lifts D2's in-megakernel-arm independence check to the
/// cross-dispatch boundary so consecutive pipelines with disjoint
/// reads/writes can fuse into one launch with a workgroup-bounded
/// fence instead of a full grid-sync.
pub mod pipeline_fusion;
/// Dialect registry, OpDef registration, lowering tables, and interner.
pub mod registry;
/// Backend-neutral reservation policy adapters.
pub mod reservation_policy;
/// Backend-neutral resident-resource reuse telemetry.
pub mod residency;
/// Backend-neutral resident transfer interval fusion.
pub mod resident_transfer_fusion;
/// Backend-neutral compact result readback planning.
pub mod result_compaction;
/// Runtime routing: profile-guided variant selection, algorithm heuristics.
pub mod routing;
/// Sampled CPU-reference shadow execution of live dispatches.
pub mod shadow;
/// N8 substrate: predicted-next-shape fingerprint API. Records
/// recent dispatch fingerprints and predicts the next via repeat /
/// short-cycle detection so the async dispatch path can prefetch
/// the predicted pipeline cache key during the GPU wait window.
pub mod shape_prediction;
/// Backend-neutral shader specialization values and cache key inputs.
pub mod specialization;
/// N2 substrate (foundation half): per-rewrite speculation-as-substrate
/// decision policy. Given baseline + speculative dispatch observations
/// + side-compile cost, returns Adopt / Reject / KeepRacing.
pub mod speculation_substrate;
/// Canonical subgroup operation taxonomy and capability records.
pub mod subgroup;
/// Trace-based JIT specialization decision policy (ROADMAP I2).
/// Decides whether the dispatcher should fire a speculative
/// pre-spec on a predicted shape, weighted by recent hit count and
/// prediction confidence vs the speculative spec cost.
pub mod trace_jit_policy;
/// Backend-neutral checked transfer accounting policy.
pub mod transfer_accounting;
/// Backend-neutral autotuner framework.
pub mod tuner;
/// Shared validation caches and launch-geometry contracts.
pub mod validation;

/// Backend-specific lowering strategies (Layer 2 of the two-layer
/// optimization architecture). Target-dependent emission decisions
/// that don't change what a program computes but change how it's
/// emitted for a specific chip/API.
///
/// See the [module docs](strategy/index.html) for the full architecture.
pub mod strategy;

/// Pure [`vyre_foundation::ir::Program`] analysis shared by all backends.
pub mod program_walks;

/// Driver-tier observability surface (P-OBS-1). Substrate-call
/// counters, cache hit rates, and a Prometheus exposition format.
pub mod observability;

/// G6: speculative rule evaluation with commit/rollback. Runs the
/// expensive confirmer on every tile, commits only tiles whose
/// pre-filter passed. Hides gather latency + improves subgroup
/// uniformity. Scaffold.
pub mod speculate;

/// Cross-grid synchronization: kernel-split fallback for backends
/// that lack a native cooperative-launch grid barrier. Splits a
/// `Program` at every `Node::Barrier { ordering: GridSync }` and
/// dispatches the segments in sequence  -  the kernel-launch boundary
/// itself is the grid-level fence.
pub mod grid_sync;
/// Backend-neutral launch preparation and program fingerprint wrappers.
pub mod launch;
/// Backend-neutral adjacent-stage launch fusion planning.
pub mod launch_fusion;
/// Backend-neutral megakernel wave barrier planning.
pub mod megakernel_barrier;
/// Backend-neutral persistent megakernel execution planning.
pub mod megakernel_execution;
/// Backend-neutral megakernel frontier memory planning.
pub mod megakernel_frontier;
/// Backend-neutral resident-graph multi-query execution planning.
pub mod multi_query_execution;
/// Backend-neutral numeric boundary conversions.
pub mod numeric;
/// G7: persistent-thread engine + device-side work queue.
/// Eliminates per-file kernel-launch overhead for streams of
/// many small scan jobs.
pub mod persistent;
/// Re-exports the unified vyre error type from `vyre-foundation`.
pub use vyre_foundation::error;

pub use aot::{emit_aot_target, registered_aot_emitters, AotEmitter, AotTargetId};
pub use backend::{
    borrowed_input_slices, default_dispatch_with_device_buffers,
    replace_output_buffers_preserving_slots, validate_buffer_ownership,
    validate_program_for_backend, BackendError, BackendRegistration, CompiledPipeline,
    DeviceBuffer, DispatchConfig, Executable, HostShimBuffer, Memory, MemoryRef, OutputBuffers,
    PendingDispatch, ResidentDispatchStep, ResidentReadRange, ResidentSequenceTiming, Resource,
    TimedDispatchResult, TypedDispatchExt, VyreBackend, DEVICE_BUFFER_FEATURE,
};
pub use binding::{
    binding_plans_share_layout, BackendLayoutClass, BackendLayoutFingerprint, BackendLayoutSlot,
    Binding, BindingPlan, BindingRole, BindingSetFingerprint,
};
pub use device_extraction::{
    extract_best_for_device, extract_best_for_devices, DeviceExtraction, ExtractionDevice,
};
pub use device_profile::{DeviceProfile, DeviceTimingQuality};
pub use device_signature::{DeviceSignature, DeviceSignatureTable};
pub use diagnostics::{Diagnostic, DiagnosticCode, OpLocation, Severity};
pub use dispatch_shape::{
    borrowed_input_batch_shapes_match, borrowed_input_shapes_match,
    dispatch_configs_share_launch_shape,
};
pub use error::Error;
pub use evidence::{
    capture_git_info, capture_git_info_at, source_fingerprint, source_tree_fingerprint,
    source_tree_fingerprint_at, DispatchTimingEvidence, EvidenceArtifact, EvidenceBundle,
    ReplayEvidence, SourceProvenance,
};
pub use fixpoint_iterations::{resolve_fixpoint_iterations, resolve_fixpoint_iterations_usize};
pub use launch::{program_vsa_fingerprint, program_vsa_fingerprint_words, LaunchPlan};
pub use pipeline::{
    compile, compile_owned, compile_owned_with_telemetry, compile_shared,
    compile_shared_with_telemetry, compile_with_telemetry, hex_encode, hex_short,
    CompiledPipelineBuild, DiskPipelineCache, PipelineCacheIdentity, PipelineCacheKey,
    PipelineCacheMissEvidence, PipelineCacheMissReason, PipelineCacheSnapshot,
    PipelineDeviceFingerprint, PipelineFeatureFlags, CURRENT_PIPELINE_CACHE_KEY_VERSION,
};
pub use program_walks::{
    coerce_to_pow2_with_tail_mask, dispatch_element_count, dispatch_element_count_for_program,
    dispatch_param_words_into, element_size_bytes, enforce_actual_output_budget,
    find_indirect_dispatch, infer_dispatch_grid, infer_dispatch_grid_for_count,
    output_binding_layout, output_binding_layouts, output_layout_from_program,
    try_coerce_to_pow2_with_tail_mask, try_dispatch_param_words, try_dispatch_param_words_into,
    IndirectDispatch, OutputBindingLayout, OutputLayout, TailMaskPolicy,
};
pub use registry::{
    default_validator, intern_string, AttrSchema, AttrType, Category, Chain, Dialect,
    DialectRegistration, DialectRegistry, DuplicateOpIdError, EnforceGate, EnforceVerdict,
    InternedOpId, LoweringCtx, LoweringTable, MutationClass, NativeModule, NativeModuleBuilder,
    OpBackendTarget, OpDef, OpDefRegistration, PrimaryBinaryBuilder, PrimaryTextBuilder,
    ReferenceKind, SecondaryTextBuilder, Signature, Target, TextModule, TypedParam,
};
pub use residency::{ResidentGraphReuseTelemetry, ResidentGraphReuseTelemetryError};
pub use routing::{select_sort_backend, Distribution, RoutingTable, SortBackend};
pub use specialization::{SpecCacheKey, SpecMap, SpecValue};
pub use speculate::{
    record_speculative_variant_race, SpeculativeVariantDecision, SpeculativeVariantKeys,
    SpeculativeVariantKind, SpeculativeVariantRace,
};
pub use subgroup::{SubgroupCaps, SubgroupOp};
