//! Megakernel backend capability and report types.

use std::time::Duration;

use super::super::policy::{
    MegakernelDispatchTopology, MegakernelExecutionMode, MegakernelQueuePressure,
};

/// Capabilities surfaced by megakernel-aware backends.
#[derive(Debug, Clone, Copy)]
pub struct MegakernelCaps {
    /// Whether the backend implements a megakernel path.
    pub supported: bool,
    /// Maximum worker-count ceiling the backend accepts.
    pub max_worker_count: u32,
}

impl MegakernelCaps {
    /// Unsupported  -  every method returns an explicit error.
    #[must_use]
    pub const fn unsupported() -> Self {
        Self {
            supported: false,
            max_worker_count: 0,
        }
    }

    /// Declare supported with the given worker ceiling.
    #[must_use]
    pub const fn supported(max_worker_count: u32) -> Self {
        Self {
            supported: true,
            max_worker_count,
        }
    }
}

/// One work-queue item the megakernel worker consumes.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MegakernelWorkItem {
    /// Stable op id index into the dialect registry.
    pub op_handle: u32,
    /// Input-buffer handle.
    pub input_handle: u32,
    /// Output-buffer handle.
    pub output_handle: u32,
    /// Optional per-item parameter word.
    pub param: u32,
}

/// Production counters from one megakernel dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MegakernelTelemetry {
    /// Bytes uploaded across control, ring, debug, and IO inputs.
    pub bytes_uploaded: u64,
    /// Bytes read back across all megakernel output buffers.
    pub bytes_read_back: u64,
    /// Total host/device transfer bytes attributable to this dispatch.
    pub bytes_moved: u64,
    /// Resident input allocations performed before dispatch.
    pub resident_allocations: u32,
    /// Kernel launches issued for this logical dispatch.
    pub kernel_launches: u32,
    /// Host-visible synchronization/readback wait points.
    pub sync_points: u32,
    /// Approximate lane occupancy in basis points, capped at 10000.
    pub occupancy_proxy_bps: u16,
    /// Active queue/frontier density in basis points, capped at 10000.
    pub frontier_density_bps: u16,
    /// Number of output buffers read back from the backend.
    pub readback_buffers: u32,
    /// True when the direct dispatch reused a compiled megakernel pipeline.
    pub compiled_pipeline_cache_hit: bool,
    /// True when the direct dispatch reused resident input resources.
    pub resident_input_cache_hit: bool,
    /// Scale-aware topology selected by the launch policy.
    pub topology: MegakernelDispatchTopology,
    /// Queue pressure classification selected by the launch policy.
    pub pressure: MegakernelQueuePressure,
    /// Interpreter or JIT route selected by launch policy telemetry.
    pub execution_mode: MegakernelExecutionMode,
    /// Sparse-hit capacity selected by the launch policy.
    pub hit_capacity: u32,
    /// Estimated peak device bytes for the selected launch plan.
    pub estimated_peak_device_bytes: u64,
    /// Hard device-memory budget applied to the launch. Zero means unbounded.
    pub device_memory_budget_bytes: u64,
}

impl Default for MegakernelTelemetry {
    fn default() -> Self {
        Self {
            bytes_uploaded: 0,
            bytes_read_back: 0,
            bytes_moved: 0,
            resident_allocations: 0,
            kernel_launches: 0,
            sync_points: 0,
            occupancy_proxy_bps: 0,
            frontier_density_bps: 0,
            readback_buffers: 0,
            compiled_pipeline_cache_hit: false,
            resident_input_cache_hit: false,
            topology: MegakernelDispatchTopology::Empty,
            pressure: MegakernelQueuePressure::Empty,
            execution_mode: MegakernelExecutionMode::Interpreter,
            hit_capacity: 0,
            estimated_peak_device_bytes: 0,
            device_memory_budget_bytes: 0,
        }
    }
}

/// Summary stats from one megakernel run.
#[derive(Debug, Clone, Default)]
pub struct MegakernelReport {
    /// Items the workers processed before exiting.
    pub items_processed: u64,
    /// Items still queued when `max_wall_time` fired.
    pub items_remaining: u64,
    /// Wall-clock time spent.
    pub wall_time: Duration,
    /// Host-side time spent shaping the queue before publication:
    /// dedupe, fusion planning, and launch-geometry preparation.
    pub queue_plan_ns: u64,
    /// Host-side time spent encoding protocol buffers and publishing
    /// queued work into ring slots.
    pub queue_publish_ns: u64,
    /// Host-observed backend dispatch latency after queue publication.
    pub backend_dispatch_ns: u64,
    /// Host-observed time spent computing optional region lineage after
    /// dispatch. Zero when lineage tracking is skipped.
    pub lineage_ns: u64,
    /// Logical work items removed by queue dedupe before publication.
    pub deduped_items: u64,
    /// Work items actually published into megakernel ring slots.
    pub published_items: u64,
    /// Number of work items included in region lineage tracking.
    pub lineage_items: u64,
    /// Production counters for performance gates and launch tuning.
    pub telemetry: MegakernelTelemetry,
    /// Per-output provenance lineage bitsets, one entry per fused
    /// region in dispatch order. `lineage[i]` is a 32-bit set of
    /// source-rule IDs that contributed to fused-region `i`'s output,
    /// computed via the substrate
    /// `vyre_self_substrate::scallop_provenance` Datalog
    /// closure on the rule-derivation graph. Empty `Vec` when
    /// provenance tracking was disabled for the dispatch.
    ///
    /// Lets observability collectors (Tempo, Honeycomb, Prometheus)
    /// attribute every megakernel output back to the source rules
    /// that derived it  -  without this, fused-region outputs lose
    /// their lineage.
    pub region_lineage: Vec<u32>,
}
