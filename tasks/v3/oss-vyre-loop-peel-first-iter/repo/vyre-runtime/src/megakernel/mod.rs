//! Persistent megakernel  -  the GPU becomes a VIR0 bytecode interpreter.
//!
//! One dispatch compiles the program; the kernel loops forever, pulling
//! packed bytecode slots from a host-fed ring buffer and executing each.
//! The host never re-dispatches  -  it only writes new slots and observes
//! atomic counters in the control buffer.
//!
//! ## Layout
//!
//! - `protocol`  -  ring-buffer slot layout, control words, opcodes.
//! - `handlers`  -  built-in opcode handlers + extension mechanism.
//! - `builder`  -  IR `Program` construction (interpreted + JIT).
//! - `execution`  -  compiled persistent-kernel handle and dispatch path.
//! - `resident`  -  host mirrors for GPU-resident runtime buffers.
//! - `readback`  -  strict output-buffer decoding after dispatch.
//! - `recovery`  -  device-loss classification and pipeline rebuild.
//!
//! ## Coordination protocol
//!
//! 1. Read `control[SHUTDOWN]`; if non-zero, `Node::Return`.
//! 2. Read this slot's `status`; skip idle slots without tenant metadata loads.
//! 3. If PUBLISHED, read `tenant_id`; authorize via tenant-mask table.
//! 4. CAS `ring_buffer[status]` from PUBLISHED → CLAIMED.
//! 5. Dispatch on opcode through If-tree (or JIT fused body).
//! 6. `atomic_add(control[DONE_COUNT], 1)`.
//! 7. Store DONE into the status word.

#[cfg(feature = "megakernel-batch")]
pub mod advanced;
pub mod automata_worklist;
pub mod builder;
pub mod descriptor;
pub mod execution;
pub mod handlers;
pub mod io;
pub mod ir_util;
pub mod mixed_work;
pub mod planner;
pub mod policy;
pub mod protocol;
mod protocol_api;
pub mod readback;
pub mod recovery;
pub mod resident;
pub mod ring;
#[cfg(feature = "megakernel-batch")]
pub mod rule_catalog;
pub mod scaling;
pub mod scheduler;
pub mod speculation;
mod staging_reserve;
pub mod task;
pub mod telemetry;
pub mod workspace_adapter;
pub mod workspace_layout;

use vyre_driver::backend::BackendError;

// Re-export protocol constants at the megakernel level for back-compat.
pub use automata_worklist::{
    AutomataStateIndex, AutomataWorklistEvidence, AutomataWorklistMode, AutomataWorklistPolicy,
    AutomataWorklistRecommendation, AutomataWorklistRequest,
    AUTOMATA_WORKLIST_EVIDENCE_SCHEMA_VERSION,
};
pub use builder::{
    build_program, build_program_jit, build_program_jit_slots, build_program_priority,
    build_program_priority_slots, build_program_sharded, build_program_sharded_no_io,
    build_program_sharded_once_slots, build_program_sharded_once_slots_control_report_shared,
    build_program_sharded_once_slots_shared, build_program_sharded_slots,
    build_program_sharded_slots_shared, build_program_sharded_with_io_polling,
    build_program_sharded_with_workspace_adapter, persistent_body, persistent_body_jit,
    persistent_body_priority, persistent_body_priority_slots,
    try_build_program_with_self_loading_miss_handler,
};
#[cfg(any(test, feature = "legacy-infallible"))]
pub use builder::build_program_with_self_loading_miss_handler;
pub use descriptor::{
    BatchDescriptor, BuiltinOpcode, PackedOpDescriptor, SlotDescriptor, SlotOpcode, WindowClass,
    WindowDescriptor,
};
pub use execution::{
    Megakernel, MegakernelDispatchOutput, MegakernelDispatchStats, MegakernelResidentHandles,
};
pub use handlers::OpcodeHandler;
pub use io::{IoCompletion, IoRequest, MegakernelIoQueue, IO_SLOT_COUNT, IO_SLOT_WORDS};
pub use mixed_work::{
    mixed_work_protocol_evidence, validate_mixed_work_protocol, MixedWorkProtocolError,
    MixedWorkProtocolEvidence, MixedWorkProtocolPlan, MixedWorkQueueClass, MixedWorkUnit,
    MixedWorkUnitType, OutputSlabId, ResidentArtifactId, MIXED_WORK_PROTOCOL_SCHEMA_VERSION,
};
#[cfg(feature = "self-substrate-adapters")]
pub use planner::{
    build_bellman_tn_order_program, build_kfac_autotune_step_program,
    build_persistent_fixpoint_program, build_scallop_lineage_with_scratch,
    build_scallop_provenance_wide_program, build_sinkhorn_clustering_program,
    build_sinkhorn_full_clustering_program,
};
pub use planner::{
    build_scallop_lineage_with_program_and_scratch, default_worker_groups_from_limits,
    dispatch_grid_for, padded_slot_count, plan_compact_fusion_into,
    select_fused_subset, select_fused_subset_compact, select_fused_subset_compact_into,
    select_fused_subset_into, select_fused_subset_with_rate, select_optimal_fused_subset,
    try_detect_cross_arm_redundancy, try_prune_redundant_work_items_into,
    try_prune_redundant_work_items_with_scratch_into, worker_workgroup_size,
    CompactFusionPlanningScratch, CrossArmRedundancy, FusionSelectionScratch, MegakernelCaps,
    MegakernelConfig, MegakernelGridLimits, MegakernelGridPlan, MegakernelGridRequest,
    MegakernelLaunchGeometry, MegakernelReport, MegakernelSizingPolicy, MegakernelTelemetry,
    MegakernelWorkItem, MegakernelWorkloadHints, RedundantWorkItemPruneScratch,
};
#[cfg(any(test, feature = "legacy-infallible"))]
pub use planner::{prune_redundant_work_items_into, prune_redundant_work_items_with_scratch_into};
#[cfg(any(test, feature = "legacy-infallible"))]
pub use policy::{diffuse_priority_across_siblings, diffuse_priority_across_siblings_into};
pub use policy::{
    try_diffuse_priority_across_siblings, try_diffuse_priority_across_siblings_into,
    MegakernelDispatchTopology, MegakernelExecutionMode, MegakernelGraphBlasSwitchClass,
    MegakernelLaunchCacheStats, MegakernelLaunchPolicy, MegakernelLaunchRecommendation,
    MegakernelLaunchRequest, MegakernelPromotionEvidence, MegakernelPromotionRoute,
    MegakernelQueuePressure, MegakernelTopologyEvidence, PriorityDrainReason,
    PriorityDrainRecommendation, PriorityRequeueAccounting, HOT_WINDOW_PROMOTION_EVIDENCE_SCHEMA_VERSION,
    PRIORITY_COUNTER_DRAIN_FIX, PRIORITY_COUNTER_DRAIN_HEADROOM, TOPOLOGY_EVIDENCE_SCHEMA_VERSION,
};
pub use protocol::{
    control, control_byte_len, count_done_ring_slots, debug, debug_log_byte_len, encode_control,
    encode_empty_debug_log, encode_empty_ring, opcode, read_debug_log, read_done_count, read_epoch,
    read_metrics, read_observable, ring_byte_len, slot, try_count_done_ring_slots,
    try_encode_control, try_encode_control_into, try_encode_empty_debug_log,
    try_encode_empty_debug_log_into, try_encode_empty_ring, try_encode_empty_ring_into,
    try_read_debug_log, try_read_done_count, try_read_epoch, try_read_metrics, try_read_observable,
    DebugRecord, ProtocolError, ARG0_WORD, ARGS_PER_SLOT, CONTROL_MIN_WORDS, OPCODE_WORD,
    PRIORITY_WORD, SLOT_WORDS, STATUS_WORD, TENANT_WORD,
};
pub use protocol_api::RingSlotTransition;
pub use readback::{MegakernelReadback, MegakernelReadbackCounters};
pub use telemetry::{
    MegakernelRuntimeEvidence, RuntimeEvidenceMetricCoverage, RuntimeEvidenceMetricFamily,
    TelemetryDecodeCapacityEvidence, TelemetryDecodeScratch, RUNTIME_IO_EVIDENCE_SCHEMA_VERSION,
    TELEMETRY_DECODE_CAPACITY_SCHEMA_VERSION,
};
pub use recovery::{
    backend_error_indicates_device_loss, MegakernelRecoveryDecision, MegakernelRecoveryPolicy,
};
pub use resident::{MegakernelResidentBuffers, MegakernelResidentDispatchScratch};
#[cfg(feature = "megakernel-batch")]
pub use rule_catalog::{BatchRuleProgram, BatchRuleRejection};
pub use scheduler::{
    default_priority_offsets, priority_partition_active_lane_count,
    priority_partition_probe_budget, priority_partition_probe_count, priority_scan_body,
    priority_scan_body_with_stride, try_default_priority_offsets, write_default_priority_offsets,
};
pub use speculation::{PairedSpeculationSample, PairedSpeculationUpdate, PairedSpeculationWindow};
pub use task::{TaskPriority, TaskQueueSnapshot, TaskState, TaskWorkItem};
pub use telemetry::{
    ControlSnapshot, CountMinSketch, MegakernelRuntimeCounters, RingOccupancy, RingSlotSnapshot,
    RingStatus, RingTelemetry, SketchTelemetry, WindowTelemetry,
};
pub use workspace_adapter::MegakernelWorkspaceAdapter;
pub use workspace_layout::{
    build_workspace_regions, first_workspace_region, next_record_workspace_region,
    next_workspace_region, workspace_record_words, MegakernelWorkspaceLayoutError,
    MegakernelWorkspaceRegion, MegakernelWorkspaceRegionSpec,
};
/// Backend-neutral megakernel dispatch contract.
pub trait MegakernelDispatch {
    /// Drain the requested megakernel dispatch.
    fn dispatch_megakernel(
        &self,
        work_queue: &[MegakernelWorkItem],
        config: &MegakernelConfig,
    ) -> Result<MegakernelReport, BackendError>;
}
