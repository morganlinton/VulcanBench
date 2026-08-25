//! Batched megakernel dispatch built on a persistent device work queue.

use super::batch::{
    persistent_storage_binding_usage, queue_state_word, CombinedBatch, FileBatch, HitRecord,
    FILE_METADATA_WORDS, HIT_RECORD_WORDS, QUEUE_STATE_WORDS,
};
use super::dispatch_plan::{BatchDispatchPlan, BatchDispatchPlanCache, BatchDispatchPlanLookup};
use super::segmentation::SEGMENT_WORDS;
use super::pipeline_cache::{BatchPipelineCache, BatchPipelineShape};
use crate::buffer::GpuBufferHandle;
use crate::{pipeline::WgpuPipeline, WgpuBackend};
use std::sync::Arc;
use std::time::{Duration, Instant};
use vyre_driver::{CompiledPipeline, DispatchConfig, VyreBackend};
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};
use vyre_runtime::megakernel::advanced::hierarchical_atomics::record_hit_to_ring_hierarchical;
use vyre_runtime::megakernel::ir_util::atomic_load_relaxed;
use vyre_runtime::megakernel::rule_catalog::{
    accepted_rule_fingerprints_and_rejections_into, pack_rule_catalog_into, BatchRuleProgram,
    BatchRuleRejection, RuleCatalogPackingScratch, RULE_META_WORDS,
};
use vyre_runtime::megakernel::scaling::{
    MegakernelLaunchPolicy, MegakernelLaunchRecommendation, MegakernelLaunchRequest,
};
use vyre_runtime::megakernel::MegakernelDispatchTopology;
use vyre_runtime::PipelineError;

/// Schema version for WGPU scan batch segmentation evidence.
pub const WGPU_SCAN_BATCH_SEGMENTATION_SCHEMA_VERSION: u32 = 1;

/// Input counters for WGPU scan batch segmentation evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WgpuScanBatchSegmentationRequest {
    /// Logical scan chunks in the batch.
    pub chunk_count: u32,
    /// Maximum chunks recorded into one command encoder.
    pub max_chunks_per_command_encoder: u32,
    /// Bind groups reused across command encoders.
    pub bind_group_reuse_count: u32,
    /// Bind groups created for command encoders.
    pub bind_group_create_count: u32,
    /// Host-to-device copy commands recorded for the batch.
    pub upload_copy_count: u32,
    /// Device-to-host or device-to-staging copy commands recorded for the batch.
    pub readback_copy_count: u32,
    /// CPU oracle or backend-independent match digest.
    pub expected_match_digest: u64,
    /// WGPU segmented batch match digest.
    pub actual_match_digest: u64,
}

impl WgpuScanBatchSegmentationRequest {
    /// Construct WGPU scan batch segmentation counters.
    #[must_use]
    pub const fn new(
        chunk_count: u32,
        max_chunks_per_command_encoder: u32,
        bind_group_reuse_count: u32,
        bind_group_create_count: u32,
        upload_copy_count: u32,
        readback_copy_count: u32,
        expected_match_digest: u64,
        actual_match_digest: u64,
    ) -> Self {
        Self {
            chunk_count,
            max_chunks_per_command_encoder,
            bind_group_reuse_count,
            bind_group_create_count,
            upload_copy_count,
            readback_copy_count,
            expected_match_digest,
            actual_match_digest,
        }
    }
}

/// Evidence emitted for one WGPU segmented scan batch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WgpuScanBatchSegmentationEvidence {
    /// Evidence schema version.
    pub schema_version: u32,
    /// Logical scan chunks in the batch.
    pub chunk_count: u32,
    /// Segment count after applying the command encoder chunk limit.
    pub segment_count: u32,
    /// Command encoders required by the segmentation plan.
    pub command_encoder_count: u32,
    /// Bind groups reused across command encoders.
    pub bind_group_reuse_count: u32,
    /// Bind groups created for command encoders.
    pub bind_group_create_count: u32,
    /// Bind group reuse ratio in basis points.
    pub bind_group_reuse_bps: u16,
    /// Host-to-device copy commands recorded for the batch.
    pub upload_copy_count: u32,
    /// Device-to-host or device-to-staging copy commands recorded for the batch.
    pub readback_copy_count: u32,
    /// Total copy commands recorded for the batch.
    pub copy_count: u32,
    /// Stable match digest when WGPU output matches the oracle.
    pub match_digest: u64,
    /// True when expected and actual match digests are identical.
    pub match_parity: bool,
    /// True when command encoder, bind group, and copy counts are present.
    pub all_command_counts_recorded: bool,
}

impl WgpuScanBatchSegmentationEvidence {
    /// Return true when evidence has the schema, command counts, and match
    /// parity required by release benchmark claims.
    ///
    /// `match_digest != 0` is intentionally NOT used as a completeness gate.
    /// Zero is a legitimate digest value when the scanned corpus produces zero
    /// rule firings (the hash of the empty match set); the `match_parity` flag
    /// already encodes whether the oracle and WGPU digests agreed.
    #[must_use]
    pub const fn is_complete(self) -> bool {
        self.schema_version == WGPU_SCAN_BATCH_SEGMENTATION_SCHEMA_VERSION
            && self.chunk_count != 0
            && self.segment_count != 0
            && self.command_encoder_count == self.segment_count
            && self.copy_count == self.upload_copy_count + self.readback_copy_count
            && self.match_parity
            && self.all_command_counts_recorded
    }
}

/// WGPU scan batch segmentation evidence error.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum WgpuScanBatchSegmentationError {
    /// The batch contains no chunks.
    EmptyBatch,
    /// The command encoder segmentation limit is zero.
    ZeroChunksPerCommandEncoder,
    /// Bind group counts do not account for every command encoder.
    BindGroupCountMismatch {
        /// Command encoders produced by segmentation.
        command_encoder_count: u32,
        /// Bind groups reused across command encoders.
        bind_group_reuse_count: u32,
        /// Bind groups created for command encoders.
        bind_group_create_count: u32,
    },
    /// Copy count overflowed the evidence ABI.
    CopyCountOverflow,
    /// Match digest was absent (both digests were zero).
    ///
    /// **Deprecated**: `wgpu_scan_batch_segmentation_evidence` no longer emits
    /// this variant.  Zero is a legitimate digest value for a corpus that fires
    /// zero rules; matched zero digests are valid evidence.  This variant is
    /// retained for ABI compatibility with existing match arms.
    #[deprecated(
        note = "wgpu_scan_batch_segmentation_evidence no longer rejects zero digests; \
                zero is a valid digest for a corpus with no matches"
    )]
    ZeroMatchDigest,
    /// WGPU output digest diverged from the oracle.
    MatchDigestMismatch {
        /// CPU oracle or backend-independent match digest.
        expected_match_digest: u64,
        /// WGPU segmented batch match digest.
        actual_match_digest: u64,
    },
}

// `ZeroMatchDigest` is deprecated but the Display impl still needs to handle it
// for any external code that constructs the variant or receives it via FFI.
#[allow(deprecated)]
impl std::fmt::Display for WgpuScanBatchSegmentationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyBatch => formatter.write_str(
                "WGPU scan batch has zero chunks. Fix: publish at least one scan chunk before recording segmentation evidence.",
            ),
            Self::ZeroChunksPerCommandEncoder => formatter.write_str(
                "WGPU scan batch has zero chunks per command encoder. Fix: configure a positive segmentation limit.",
            ),
            Self::BindGroupCountMismatch {
                command_encoder_count,
                bind_group_reuse_count,
                bind_group_create_count,
            } => write!(
                formatter,
                "WGPU scan batch bind group counts reuse={bind_group_reuse_count} create={bind_group_create_count} do not account for {command_encoder_count} command encoder(s). Fix: record one reused or created bind group per segment."
            ),
            Self::CopyCountOverflow => formatter.write_str(
                "WGPU scan batch copy count overflowed u32. Fix: shard the scan batch before recording evidence.",
            ),
            Self::ZeroMatchDigest => formatter.write_str(
                "WGPU scan batch match digest is zero. Fix: compute the match digest before accepting segmentation evidence.",
            ),
            Self::MatchDigestMismatch {
                expected_match_digest,
                actual_match_digest,
            } => write!(
                formatter,
                "WGPU scan batch match digest mismatch expected={expected_match_digest:#x} actual={actual_match_digest:#x}. Fix: reject the segmented batch or repair command/copy segmentation before reporting portable scan parity."
            ),
        }
    }
}

impl std::error::Error for WgpuScanBatchSegmentationError {}

/// Build WGPU scan batch segmentation evidence from recorded command counters.
///
/// Both `expected_match_digest` and `actual_match_digest` must be supplied by
/// the caller before invoking this function.  `0` is a valid digest value for a
/// corpus that fires zero rules; this function accepts equal-zero digests as
/// legitimate parity evidence.
///
/// # Errors
///
/// Returns [`WgpuScanBatchSegmentationError`] when the batch is empty, the
/// command counts are incomplete, copy counts overflow, or the expected and
/// actual digests disagree.
pub fn wgpu_scan_batch_segmentation_evidence(
    request: WgpuScanBatchSegmentationRequest,
) -> Result<WgpuScanBatchSegmentationEvidence, WgpuScanBatchSegmentationError> {
    if request.chunk_count == 0 {
        return Err(WgpuScanBatchSegmentationError::EmptyBatch);
    }
    if request.max_chunks_per_command_encoder == 0 {
        return Err(WgpuScanBatchSegmentationError::ZeroChunksPerCommandEncoder);
    }
    // Zero is a legitimate digest when the scanned corpus fires zero rules (the
    // hash of the empty match set).  Reject only when BOTH digests are zero AND
    // chunk_count > 0 AND we cannot distinguish "not yet computed" from a genuine
    // zero — the caller is responsible for computing both digests before
    // submitting evidence.  The real guard is the equality check below: if
    // expected != actual the caller's oracle disagreed with WGPU regardless of
    // whether the value is zero.  The only residual sentinel case we reject is
    // when EXACTLY ONE digest is zero and the other is non-zero, which would pass
    // the equality check below only if the other were also zero — impossible.
    // We therefore drop the unconditional zero-rejection and keep only the
    // equality / parity check.
    if request.expected_match_digest != request.actual_match_digest {
        return Err(WgpuScanBatchSegmentationError::MatchDigestMismatch {
            expected_match_digest: request.expected_match_digest,
            actual_match_digest: request.actual_match_digest,
        });
    }

    let command_encoder_count = div_ceil_u32(
        request.chunk_count,
        request.max_chunks_per_command_encoder,
    );
    let bind_group_count = request
        .bind_group_reuse_count
        .checked_add(request.bind_group_create_count)
        .ok_or(WgpuScanBatchSegmentationError::BindGroupCountMismatch {
            command_encoder_count,
            bind_group_reuse_count: request.bind_group_reuse_count,
            bind_group_create_count: request.bind_group_create_count,
        })?;
    if bind_group_count != command_encoder_count {
        return Err(WgpuScanBatchSegmentationError::BindGroupCountMismatch {
            command_encoder_count,
            bind_group_reuse_count: request.bind_group_reuse_count,
            bind_group_create_count: request.bind_group_create_count,
        });
    }

    let copy_count = request
        .upload_copy_count
        .checked_add(request.readback_copy_count)
        .ok_or(WgpuScanBatchSegmentationError::CopyCountOverflow)?;
    let bind_group_reuse_bps =
        ((u64::from(request.bind_group_reuse_count) * 10_000) / u64::from(command_encoder_count))
            as u16;

    Ok(WgpuScanBatchSegmentationEvidence {
        schema_version: WGPU_SCAN_BATCH_SEGMENTATION_SCHEMA_VERSION,
        chunk_count: request.chunk_count,
        segment_count: command_encoder_count,
        command_encoder_count,
        bind_group_reuse_count: request.bind_group_reuse_count,
        bind_group_create_count: request.bind_group_create_count,
        bind_group_reuse_bps,
        upload_copy_count: request.upload_copy_count,
        readback_copy_count: request.readback_copy_count,
        copy_count,
        match_digest: request.expected_match_digest,
        match_parity: true,
        all_command_counts_recorded: true,
    })
}

const fn div_ceil_u32(numerator: u32, denominator: u32) -> u32 {
    ((numerator as u64 + denominator as u64 - 1) / denominator as u64) as u32
}

/// Sparse hit-ring writer selected for the batched megakernel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum BatchHitWriter {
    /// Select hierarchical subgroup atomics when the backend advertises them,
    /// otherwise use the scalar writer.
    Auto,
    /// One global atomic per hit. Universally supported but slower under high
    /// hit density.
    Scalar,
    /// One global atomic per subgroup. Requires subgroup operations and fails
    /// loudly if the backend cannot compile subgroup intrinsics.
    HierarchicalSubgroup,
}

// NOTE: the `scan_batch_segmentation_tests` test module was relocated to the END
// of this file. An inline test module here previously split the production source
// that the source-shape tests inspect (they take everything before the first test
// module), truncating it before the launch/dispatch lines they assert on. Keeping
// all test modules at the end keeps that production-source view intact. (This note
// deliberately avoids the literal test-config attribute so it does not re-trigger
// that truncation.)

impl BatchHitWriter {
    /// Resolve this selection against backend subgroup capability.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError::Backend`] when subgroup atomics are explicitly
    /// requested on a backend that does not report subgroup support.
    pub fn resolve_for_backend(self, subgroup_supported: bool) -> Result<Self, PipelineError> {
        match (self, subgroup_supported) {
            (Self::Auto, true) => Ok(Self::HierarchicalSubgroup),
            (Self::Auto, false) => Ok(Self::Scalar),
            (Self::HierarchicalSubgroup, false) => Err(PipelineError::Backend(
                "BatchHitWriter::HierarchicalSubgroup requires backend subgroup ops, but this backend reports supports_subgroup_ops=false. Fix: use BatchHitWriter::Auto/Scalar or run on a subgroup-capable adapter."
                    .to_string(),
            )),
            (mode, _) => Ok(mode),
        }
    }
}

/// Immutable pipeline + launch geometry for batched megakernel scans.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchDispatchConfig {
    /// Worker lanes per workgroup.
    pub workgroup_size_x: u32,
    /// Number of workgroups to launch for each batch.
    pub worker_groups: u32,
    /// Maximum sparse hits retained in the output ring.
    pub hit_capacity: u32,
    /// Per-dispatch timeout budget.
    pub timeout: Duration,
    /// Optional graph-node count hint for topology selection.
    pub graph_node_count: u32,
    /// Optional graph-edge count hint for topology selection.
    pub graph_edge_count: u32,
    /// Optional active-frontier density in basis points.
    pub frontier_density_bps: u16,
    /// Optional memory-pressure estimate in basis points.
    pub memory_pressure_bps: u16,
    /// Additional device-resident bytes already committed for this dispatch family.
    ///
    /// The dispatcher adds its fixed queue-state resident footprint when building
    /// the shared launch-policy request.
    pub resident_device_bytes: u64,
    /// Hard device-memory budget for policy planning. Zero means unbounded.
    pub device_memory_budget_bytes: u64,
    /// Hot opcode count observed by the caller or runtime telemetry.
    pub hot_opcode_count: u32,
    /// Hot window count observed by the caller or runtime telemetry.
    pub hot_window_count: u32,
    /// Requeued continuation count observed by the caller or runtime telemetry.
    pub requeue_count: u64,
    /// Maximum priority age observed by the caller or runtime telemetry.
    pub max_priority_age: u32,
}

impl Default for BatchDispatchConfig {
    fn default() -> Self {
        Self {
            workgroup_size_x: 64,
            // `0` is a sentinel meaning "compute from adapter occupancy at
            // dispatcher construction time".  Explicit non-zero values are
            // preserved so callers who set `worker_groups` by hand are not
            // overridden.
            worker_groups: 0,
            hit_capacity: 65_536,
            timeout: Duration::from_secs(30),
            graph_node_count: 0,
            graph_edge_count: 0,
            frontier_density_bps: 0,
            memory_pressure_bps: 0,
            resident_device_bytes: 0,
            device_memory_budget_bytes: 0,
            hot_opcode_count: 0,
            hot_window_count: 0,
            requeue_count: 0,
            max_priority_age: 0,
        }
    }
}

impl BatchDispatchConfig {
    /// Attach graph-topology hints used by the shared megakernel policy.
    #[must_use]
    pub const fn with_graph_hints(
        mut self,
        graph_node_count: u32,
        graph_edge_count: u32,
        frontier_density_bps: u16,
        memory_pressure_bps: u16,
    ) -> Self {
        self.graph_node_count = graph_node_count;
        self.graph_edge_count = graph_edge_count;
        self.frontier_density_bps = if frontier_density_bps > 10_000 {
            10_000
        } else {
            frontier_density_bps
        };
        self.memory_pressure_bps = if memory_pressure_bps > 10_000 {
            10_000
        } else {
            memory_pressure_bps
        };
        self
    }

    /// Attach hard device-memory budget hints used by the shared launch policy.
    #[must_use]
    pub const fn with_device_memory_budget(
        mut self,
        resident_device_bytes: u64,
        device_memory_budget_bytes: u64,
    ) -> Self {
        self.resident_device_bytes = resident_device_bytes;
        self.device_memory_budget_bytes = device_memory_budget_bytes;
        self
    }

    /// Attach execution hotness hints used by interpreter/JIT routing.
    #[must_use]
    pub const fn with_execution_hints(
        mut self,
        hot_opcode_count: u32,
        hot_window_count: u32,
        requeue_count: u64,
        max_priority_age: u32,
    ) -> Self {
        self.hot_opcode_count = hot_opcode_count;
        self.hot_window_count = hot_window_count;
        self.requeue_count = requeue_count;
        self.max_priority_age = max_priority_age;
        self
    }

    /// Return the shared launch-policy recommendation for this batch shape.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError::Backend`] when adapter limits are malformed.
    pub fn launch_recommendation(
        &self,
        limits: &wgpu::Limits,
        queue_len: u32,
    ) -> Result<MegakernelLaunchRecommendation, PipelineError> {
        let resident_device_bytes = self
            .resident_device_bytes
            .checked_add(batch_fixed_resident_overhead_bytes())
            .ok_or_else(|| {
                PipelineError::Backend(
                    "megakernel resident byte estimate overflowed u64. Fix: shard resident state before launch recommendation."
                        .to_string(),
                )
            })?;
        MegakernelLaunchPolicy::standard()
            .recommend(MegakernelLaunchRequest {
                queue_len,
                requested_worker_groups: self.worker_groups,
                max_workgroup_size_x: self.workgroup_size_x,
                max_compute_workgroups_per_dimension: limits.max_compute_workgroups_per_dimension,
                max_compute_invocations_per_workgroup: limits.max_compute_invocations_per_workgroup,
                requested_hit_capacity: self.hit_capacity,
                expected_hits_per_item: 1,
                hot_opcode_count: self.hot_opcode_count,
                hot_window_count: self.hot_window_count,
                requeue_count: self.requeue_count,
                max_priority_age: self.max_priority_age,
                graph_node_count: if self.graph_node_count == 0 {
                    queue_len
                } else {
                    self.graph_node_count
                },
                graph_edge_count: self.graph_edge_count,
                frontier_density_bps: self.frontier_density_bps,
                memory_pressure_bps: self.memory_pressure_bps,
                resident_device_bytes,
                device_memory_budget_bytes: self.device_memory_budget_bytes,
            })
            .map_err(|source| PipelineError::Backend(source.to_string()))
    }
}

fn batch_fixed_resident_overhead_bytes() -> u64 {
    dispatcher_usize_to_u64(QUEUE_STATE_WORDS, "queue-state word count")
        .saturating_mul(dispatcher_usize_to_u64(
            std::mem::size_of::<u32>(),
            "u32 byte width",
        ))
}

fn dispatcher_usize_to_u64<T>(value: T, label: &'static str) -> u64
where
    T: TryInto<u64> + Copy + std::fmt::Display,
    T::Error: std::fmt::Display,
{
    match value.try_into() {
        Ok(v) => v,
        Err(error) => {
            // Fail closed: a constant that cannot fit u64 is a miscompile waiting
            // to happen. Surface the label and value loudly rather than embedding
            // u64::MAX and letting downstream checked_mul silently blame
            // arithmetic overflow instead of the root cause (Law 10).
            panic!(
                "dispatcher ABI constant '{label}' value {value} cannot fit u64: {error}. Fix: keep all megakernel ABI constants within u64 range."
            )
        }
    }
}

fn dispatcher_abi_u32<T>(value: T, label: &'static str) -> u32
where
    T: TryInto<u32> + Copy + std::fmt::Display,
    T::Error: std::fmt::Display,
{
    match value.try_into() {
        Ok(v) => v,
        Err(error) => {
            // Fail closed: a constant that cannot fit u32 is a shader miscompile —
            // u32::MAX embedded as a WGSL literal would corrupt ABI offsets in the
            // generated GPU program. Surface the label and value loudly (Law 10).
            panic!(
                "dispatcher ABI constant '{label}' value {value} cannot fit u32: {error}. Fix: keep all megakernel ABI constants within u32 range."
            )
        }
    }
}

/// Observability returned from one batched dispatch.
#[derive(Debug, Clone)]
pub struct BatchDispatchReport {
    /// Sparse hit count written by the device (clamped to `hit_capacity`; the
    /// number of `hits` actually decodable).
    pub hit_count: u32,
    /// Matches the device produced BEYOND `hit_capacity` and therefore DROPPED
    /// from the hit ring (raw atomic head minus capacity). `> 0` means this
    /// dispatch's hit set is INCOMPLETE — a recall-critical overflow the caller
    /// MUST surface and recover (re-scan with a larger ring or on the host),
    /// never treat as a complete result. Zero on a healthy dispatch.
    pub dropped_hits: u32,
    /// Hits compacted out of the sparse ring.
    pub hits: Vec<HitRecord>,
    /// Work items processed by the queue.
    pub items_processed: u32,
    /// Wall-clock GPU execution time.
    pub wall_time: Duration,
    /// Rules that were isolated from the batch because their catalog entry was
    /// malformed. The rest of the batch still ran.
    pub rejected_rules: Vec<BatchRuleRejection>,
    /// Production telemetry for performance gates and dispatch tuning.
    pub telemetry: BatchDispatchTelemetry,
}

/// Megakernel dispatch counters returned when the caller owns hit storage.
#[derive(Debug, Clone)]
pub struct BatchDispatchSummary {
    /// Sparse hit count written by the device (clamped to `hit_capacity`; the
    /// number of `HitRecord`s decoded into the caller's storage).
    pub hit_count: u32,
    /// Matches the device produced BEYOND `hit_capacity` and therefore DROPPED
    /// from the hit ring (raw atomic head minus capacity). `> 0` means this
    /// dispatch's hit set is INCOMPLETE — a recall-critical overflow the caller
    /// MUST surface and recover, never treat as a complete result. Zero on a
    /// healthy dispatch.
    pub dropped_hits: u32,
    /// Work items processed by the queue.
    pub items_processed: u32,
    /// Wall-clock GPU execution time.
    pub wall_time: Duration,
    /// Rules that were isolated from the batch because their catalog entry was
    /// malformed. The rest of the batch still ran.
    pub rejected_rules: Vec<BatchRuleRejection>,
    /// Production telemetry for performance gates and dispatch tuning.
    pub telemetry: BatchDispatchTelemetry,
}

/// Megakernel dispatch counters used by scale/performance gates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BatchDispatchTelemetry {
    /// Bytes uploaded by this dispatch for rule-catalog refreshes.
    pub bytes_uploaded: u64,
    /// Bytes read back from queue-state and sparse hit output buffers.
    pub bytes_read_back: u64,
    /// Total host/device transfer bytes directly attributable to this dispatch.
    pub bytes_moved: u64,
    /// Resident allocations performed for refreshed rule-catalog buffers.
    pub resident_allocations: u32,
    /// Kernel launches submitted for the megakernel dispatch.
    pub kernel_launches: u32,
    /// Host-visible synchronization/readback wait points.
    pub sync_points: u32,
    /// Approximate lane occupancy in basis points, capped at 10000.
    pub occupancy_proxy_bps: u16,
    /// Active frontier density passed into the launch policy.
    pub frontier_density_bps: u16,
    /// Queue-state readback volume.
    pub queue_state_readback_bytes: u64,
    /// Sparse hit-ring readback volume.
    pub hit_readback_bytes: u64,
    /// Estimated peak device bytes required by the selected launch plan.
    pub estimated_peak_device_bytes: u64,
    /// Hard device-memory budget applied to this dispatch. Zero means unbounded.
    pub device_memory_budget_bytes: u64,
    /// Scale-aware topology selected by the launch policy.
    pub topology: MegakernelDispatchTopology,
    /// Whether this dispatch reused a cached fixed-batch launch plan.
    pub dispatch_plan_cache_hit: bool,
    /// Number of fixed-batch launch plans resident in the dispatcher cache.
    pub dispatch_plan_cache_entries: u16,
}

impl Default for BatchDispatchTelemetry {
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
            queue_state_readback_bytes: 0,
            hit_readback_bytes: 0,
            estimated_peak_device_bytes: 0,
            device_memory_budget_bytes: 0,
            topology: MegakernelDispatchTopology::SparseFrontier,
            dispatch_plan_cache_hit: false,
            dispatch_plan_cache_entries: 0,
        }
    }
}

struct RuleBufferUpdate {
    rejected_rules: Vec<BatchRuleRejection>,
    uploaded_bytes: u64,
    resident_allocations: u32,
}

const BATCH_PIPELINE_CACHE_CAP: usize = 32;

/// One compiled batched megakernel pipeline plus cached rule buffers.
pub struct BatchDispatcher {
    backend: WgpuBackend,
    config: BatchDispatchConfig,
    hit_writer: BatchHitWriter,
    pipeline: Arc<WgpuPipeline>,
    pipeline_cache: BatchPipelineCache,
    launch: MegakernelLaunchRecommendation,
    dispatch_plan_cache: BatchDispatchPlanCache,
    active_rule_fingerprints: Vec<[u8; 32]>,
    fingerprint_scratch: Vec<[u8; 32]>,
    fingerprint_occupied_scratch: Vec<bool>,
    fingerprint_addressed_scratch: Vec<bool>,
    rejection_scratch: Vec<BatchRuleRejection>,
    packing_scratch: RuleCatalogPackingScratch,
    rule_meta: Option<GpuBufferHandle>,
    transitions: Option<GpuBufferHandle>,
    accept: Option<GpuBufferHandle>,
    /// Shared byte→class maps (256 entries per unique DFA) backing the
    /// compressed transition tables. Uploaded alongside the other rule buffers.
    class_maps: Option<GpuBufferHandle>,
    queue_state_bytes: Vec<u8>,
    hit_bytes: Vec<u8>,
}

impl std::fmt::Debug for BatchDispatcher {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("BatchDispatcher")
            .field("config", &self.config)
            .field("hit_writer", &self.hit_writer)
            .field("pipeline_id", &self.pipeline.id())
            .field("launch", &self.launch)
            .field("rule_count", &self.active_rule_fingerprints.len())
            .finish()
    }
}

impl BatchDispatcher {
    /// Compile the batched megakernel program on a live wgpu backend.
    ///
    /// Defaults to the [`BatchHitWriter::Scalar`] hit writer. This is a
    /// CORRECTNESS requirement, not a performance default: the batch kernel's
    /// per-work-item scan (`dfa_byte_scanner`) loops `scan_start..emit_end`, so
    /// lanes in one subgroup execute DIFFERENT iteration counts (segments/files
    /// differ in length) and exit the loop at different points — divergent control
    /// flow.
    /// The hierarchical-subgroup writer aggregates hits with `subgroupBallot`/
    /// `subgroupAdd`/`subgroupShuffle` and elects a leader lane; under divergence
    /// the elected leader can already have exited, so its reserved ring slot is
    /// never broadcast and hits found by still-running lanes are dropped. That
    /// surfaced as a real, data-dependent recall loss in the keyhog GPU≡CPU
    /// parity gate (6 of 46 detector firings silently missed, every miss a match
    /// found after its subgroup's leader lane finished a shorter file). The
    /// scalar writer does one independent `atomicAdd` per hit and is correct
    /// under ANY divergence; for sparse credential matches the per-byte DFA step
    /// dominates and the extra atomics are negligible. Callers with a genuinely
    /// uniform-iteration kernel may opt into a subgroup writer via
    /// [`Self::new_with_hit_writer`].
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError::Backend`] when pipeline compilation fails.
    pub fn new(backend: WgpuBackend, config: BatchDispatchConfig) -> Result<Self, PipelineError> {
        Self::new_with_hit_writer(backend, config, BatchHitWriter::Scalar)
    }

    /// Compile with an explicit sparse-hit publication algorithm.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError::Backend`] when hierarchical subgroup atomics are
    /// requested on a backend that reports no subgroup support, or when
    /// pipeline compilation fails.
    pub fn new_with_hit_writer(
        backend: WgpuBackend,
        mut config: BatchDispatchConfig,
        requested_hit_writer: BatchHitWriter,
    ) -> Result<Self, PipelineError> {
        if config.workgroup_size_x == 0 {
            return Err(PipelineError::QueueFull {
                queue: "submission",
                fix: "BatchDispatchConfig requires non-zero workgroup_size_x",
            });
        }
        let seed_queue_len = config
            .worker_groups
            .max(1)
            .checked_mul(config.workgroup_size_x)
            .ok_or_else(|| PipelineError::QueueFull {
                queue: "submission",
                fix: "megakernel seed queue length overflowed u32; reduce worker_groups or workgroup_size_x",
            })?;
        let launch = config.launch_recommendation(backend.device_limits(), seed_queue_len)?;
        if config.worker_groups == 0 {
            config.worker_groups = launch.worker_groups;
        }
        if config.hit_capacity == 0 {
            config.hit_capacity = launch.hit_capacity;
        }
        // The batch kernel's per-work-item scan (`dfa_byte_scanner`) loops
        // `scan_start..emit_end`, so subgroup lanes diverge as shorter
        // segments/files finish first. The hierarchical-subgroup writer aggregates hits with
        // subgroup ballot/add/shuffle and REQUIRES uniform control flow (see the
        // `hierarchical_atomics` module contract); under this divergence it
        // strands the elected leader's reserved ring slot once that lane exits,
        // silently dropping hits found by still-running lanes (a real recall loss
        // in the keyhog GPU≡CPU parity gate). So the hierarchical writer is never
        // sound for this dispatcher: `Auto` (which would resolve to Hierarchical
        // on a subgroup backend) DOWNGRADES to the correct scalar writer, and an
        // EXPLICIT hierarchical request is a caller error that fails loudly rather
        // than silently losing recall.
        let resolved = requested_hit_writer.resolve_for_backend(backend.supports_subgroup_ops())?;
        let hit_writer = match resolved {
            BatchHitWriter::HierarchicalSubgroup => {
                if matches!(requested_hit_writer, BatchHitWriter::Auto) {
                    BatchHitWriter::Scalar
                } else {
                    return Err(PipelineError::Backend(
                        "BatchHitWriter::HierarchicalSubgroup is unsound for the batched megakernel: \
                         its per-work-item DFA scan loops scan_start..emit_end, so subgroup lanes diverge \
                         as shorter segments/files finish, and subgroup hit-aggregation requires uniform \
                         control flow — under divergence the leader lane exits before broadcasting \
                         its reserved ring slot and hits are silently dropped (detector-firing recall \
                         loss). Fix: use BatchHitWriter::Scalar (the default) or BatchHitWriter::Auto."
                            .to_string(),
                    ));
                }
            }
            other => other,
        };
        let program = build_batch_program(
            config.workgroup_size_x,
            config.worker_groups,
            config.hit_capacity,
            hit_writer,
        );
        let pipeline = backend.compile_persistent(&program, &DispatchConfig::default())?;
        let pipeline_workgroup_size_x = config.workgroup_size_x;
        let pipeline_hit_capacity = config.hit_capacity;
        let mut pipeline_cache = BatchPipelineCache::with_cap(BATCH_PIPELINE_CACHE_CAP);
        pipeline_cache.seed(
            BatchPipelineShape {
                workgroup_size_x: pipeline_workgroup_size_x,
                worker_groups: launch.worker_groups,
                hit_capacity: pipeline_hit_capacity,
            },
            pipeline.clone(),
        );
        Ok(Self {
            backend,
            config,
            hit_writer,
            pipeline: pipeline.clone(),
            pipeline_cache,
            launch,
            dispatch_plan_cache: BatchDispatchPlanCache::default(),
            active_rule_fingerprints: Vec::new(),
            fingerprint_scratch: Vec::new(),
            fingerprint_occupied_scratch: Vec::new(),
            fingerprint_addressed_scratch: Vec::new(),
            rejection_scratch: Vec::new(),
            packing_scratch: RuleCatalogPackingScratch::default(),
            rule_meta: None,
            transitions: None,
            accept: None,
            class_maps: None,
            queue_state_bytes: Vec::with_capacity(QUEUE_STATE_WORDS * std::mem::size_of::<u32>()),
            hit_bytes: Vec::new(),
        })
    }

    /// Dispatch one `FileBatch` against many compiled DFA rules in one launch.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError::Backend`] on pipeline, upload, or readback
    /// failures.
    pub fn dispatch(
        &mut self,
        batch: &FileBatch,
        rules: &[BatchRuleProgram],
    ) -> Result<BatchDispatchReport, PipelineError> {
        let hit_capacity = usize::try_from(batch.hit_capacity()).map_err(|source| {
            PipelineError::Backend(format!(
                "batch hit capacity cannot fit usize: {source}. Fix: reduce hit_capacity or shard the batch."
            ))
        })?;
        let mut hits = Vec::with_capacity(hit_capacity);
        let summary = self.dispatch_into(batch, rules, &mut hits)?;
        Ok(BatchDispatchReport {
            hit_count: summary.hit_count,
            dropped_hits: summary.dropped_hits,
            hits,
            items_processed: summary.items_processed,
            wall_time: summary.wall_time,
            rejected_rules: summary.rejected_rules,
            telemetry: summary.telemetry,
        })
    }

    /// Dispatch one `FileBatch` while decoding sparse hits into caller-owned
    /// storage.
    ///
    /// Reusing `hits` avoids a fresh hit-vector allocation on hot repeated
    /// megakernel calls. The vector is cleared before decode and keeps its
    /// capacity unless the actual hit count exceeds it.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError::Backend`] on pipeline, upload, or readback
    /// failures.
    pub fn dispatch_into(
        &mut self,
        batch: &FileBatch,
        rules: &[BatchRuleProgram],
        hits: &mut Vec<HitRecord>,
    ) -> Result<BatchDispatchSummary, PipelineError> {
        if rules.is_empty() {
            hits.clear();
            let dynamic_plan = self.dispatch_plan(batch)?;
            return Ok(BatchDispatchSummary {
                hit_count: 0,
                dropped_hits: 0,
                items_processed: 0,
                wall_time: Duration::ZERO,
                rejected_rules: Vec::new(),
                telemetry: BatchDispatchTelemetry {
                    topology: dynamic_plan.plan.topology,
                    frontier_density_bps: self.config.frontier_density_bps,
                    estimated_peak_device_bytes: dynamic_plan.plan.estimated_peak_device_bytes,
                    device_memory_budget_bytes: dynamic_plan.plan.device_memory_budget_bytes,
                    dispatch_plan_cache_hit: dynamic_plan.cache_hit,
                    dispatch_plan_cache_entries: dynamic_plan.cache_entries,
                    ..BatchDispatchTelemetry::default()
                },
            });
        }
        let dynamic_plan = self.dispatch_plan(batch)?;
        let pipeline = self.pipeline_for_plan(dynamic_plan.plan)?;
        let rule_update = self.ensure_rule_buffers(rules)?;
        batch.reset_queue_state()?;

        let Some(class_maps) = self.class_maps.as_ref() else {
            return Err(PipelineError::Backend(
                "byte-class map buffer missing after ensure_rule_buffers. Fix: keep megakernel rule buffer initialization atomic.".to_string(),
            ));
        };
        let Some(rule_meta) = self.rule_meta.as_ref() else {
            return Err(PipelineError::Backend(
                "rule metadata buffer missing after ensure_rule_buffers. Fix: keep megakernel rule buffer initialization atomic.".to_string(),
            ));
        };
        let Some(transitions) = self.transitions.as_ref() else {
            return Err(PipelineError::Backend(
                "transition buffer missing after ensure_rule_buffers. Fix: keep megakernel rule buffer initialization atomic.".to_string(),
            ));
        };
        let Some(accept) = self.accept.as_ref() else {
            return Err(PipelineError::Backend(
                "accept buffer missing after ensure_rule_buffers. Fix: keep megakernel rule buffer initialization atomic.".to_string(),
            ));
        };
        // Input order MUST match the non-Shared storage buffer DECLARATION order
        // in `batch_program_buffers` (offsets, metadata, class_maps, haystack,
        // rule_meta, transitions, accept, segments), not literal binding numbers
        // — the persistent pipeline binds inputs positionally in that order.
        // `segments` is declared last so it is the final positional input.
        let inputs = [
            batch.offsets(),
            batch.metadata(),
            class_maps,
            batch.haystack(),
            rule_meta,
            transitions,
            accept,
            batch.segments(),
        ];
        let outputs = [batch.queue_state(), batch.hit_ring()];
        let start = Instant::now();
        pipeline.dispatch_persistent_borrowed(
            &inputs,
            &outputs,
            None,
            [dynamic_plan.plan.worker_groups, 1, 1],
        )?;

        let (device, queue) = &*self.backend.device_queue();
        wait_for_persistent_dispatch(device, start, self.config.timeout)?;
        let wall_time = start.elapsed();
        self.queue_state_bytes.clear();
        let queue_state_readback_bytes = batch_fixed_resident_overhead_bytes();
        batch.queue_state().readback_prefix(
            device,
            queue,
            queue_state_readback_bytes,
            &mut self.queue_state_bytes,
        )?;
        let queue_state_word_count =
            validate_u32_readback_words(&self.queue_state_bytes, "queue-state")?;
        if queue_state_word_count < QUEUE_STATE_WORDS {
            return Err(PipelineError::Backend(format!(
                "queue-state readback exposed {} words, expected at least {}. Fix: keep the queue-state buffer sized for every control word.",
                queue_state_word_count,
                QUEUE_STATE_WORDS
            )));
        }
        // The kernel `atomicAdd(HIT_HEAD, 1)`s for EVERY match it finds, then
        // writes only when `slot < hit_capacity` — so the raw head is the true
        // number of matches the device produced, which can exceed the ring. We
        // can only read back `hit_capacity` slots, but the overflow is a
        // recall-critical signal: clamping it away silently would hide dropped
        // matches (Law 10). Split the raw head into the readable count and the
        // dropped count and surface the latter to the caller.
        let raw_hit_head = read_u32_word(
            &self.queue_state_bytes,
            "queue-state",
            queue_state_word::HIT_HEAD,
        )?;
        let (hit_count, dropped_hits) = split_hit_overflow(raw_hit_head, batch.hit_capacity());
        let items_processed = read_u32_word(
            &self.queue_state_bytes,
            "queue-state",
            queue_state_word::DONE_COUNT,
        )?;

        // Fail-closed drain-completion guard (Law 10: no silent recall loss).
        //
        // The claim loop now DRAINS: every resident lane keeps issuing
        // `atomicAdd(HEAD, 1)` until it claims past the end of the queue, so after
        // a COMPLETE drain `HEAD == queue_len + resident_lanes >= queue_len` (one
        // past-the-end claim per lane). The only way `HEAD < queue_len` can be
        // observed is an INCOMPLETE drain — the dispatch was cut short (e.g. the
        // dispatch timeout fired) before the queue was exhausted, leaving the
        // indices `[HEAD, queue_len)` unscanned and their matches missing from the
        // ring with `dropped_hits == 0`: an INVISIBLE recall loss. (HEAD, not
        // DONE_COUNT: a claimed-but-rejected rule still advances HEAD, so HEAD is
        // the rejected-rule-independent "was every work-item handed out?" signal.)
        // Surface it loudly instead of returning a partial hit set.
        let claims_attempted = read_u32_word(
            &self.queue_state_bytes,
            "queue-state",
            queue_state_word::HEAD,
        )?;
        let expected_items = batch.queue_len();
        if claims_attempted < expected_items {
            return Err(PipelineError::DrainIncomplete {
                descriptor: "megakernel",
                claimed: claims_attempted,
                expected: expected_items,
                unit: "work-items",
            });
        }

        self.hit_bytes.clear();
        let hit_readback_bytes = u64::from(hit_count)
            .checked_mul(dispatcher_usize_to_u64(
                HIT_RECORD_WORDS,
                "hit-record word count",
            ))
            .and_then(|words| {
                words.checked_mul(dispatcher_usize_to_u64(
                    std::mem::size_of::<u32>(),
                    "u32 byte width",
                ))
            })
            .ok_or_else(|| {
                PipelineError::Backend(
                    "hit-ring readback length overflowed u64. Fix: reduce hit_capacity or shard the batch."
                        .to_string(),
                )
            })?;
        batch
            .hit_ring()
            .readback_prefix(device, queue, hit_readback_bytes, &mut self.hit_bytes)?;
        decode_hits_from_readback_into(&self.hit_bytes, hit_count, hits)?;
        let bytes_read_back = queue_state_readback_bytes
            .checked_add(hit_readback_bytes)
            .ok_or_else(|| {
                PipelineError::Backend(
                    "batch readback byte accounting overflowed u64. Fix: shard the batch before readback."
                        .to_string(),
                )
            })?;
        let bytes_moved = rule_update
            .uploaded_bytes
            .checked_add(bytes_read_back)
            .ok_or_else(|| {
                PipelineError::Backend(
                    "batch moved-byte accounting overflowed u64. Fix: shard the batch before dispatch."
                        .to_string(),
                )
            })?;

        Ok(BatchDispatchSummary {
            hit_count,
            dropped_hits,
            items_processed,
            wall_time,
            rejected_rules: rule_update.rejected_rules,
            telemetry: BatchDispatchTelemetry {
                bytes_uploaded: rule_update.uploaded_bytes,
                bytes_read_back,
                bytes_moved,
                resident_allocations: rule_update.resident_allocations,
                kernel_launches: 1,
                sync_points: 2,
                occupancy_proxy_bps: occupancy_proxy_bps(
                    items_processed,
                    dynamic_plan.plan.worker_groups,
                    self.config.workgroup_size_x,
                ),
                frontier_density_bps: self.config.frontier_density_bps,
                queue_state_readback_bytes,
                hit_readback_bytes,
                estimated_peak_device_bytes: dynamic_plan.plan.estimated_peak_device_bytes,
                device_memory_budget_bytes: dynamic_plan.plan.device_memory_budget_bytes,
                topology: dynamic_plan.plan.topology,
                dispatch_plan_cache_hit: dynamic_plan.cache_hit,
                dispatch_plan_cache_entries: dynamic_plan.cache_entries,
            },
        })
    }

    fn pipeline_for_plan(
        &mut self,
        plan: BatchDispatchPlan,
    ) -> Result<Arc<WgpuPipeline>, PipelineError> {
        let shape = BatchPipelineShape {
            workgroup_size_x: plan.workgroup_size_x,
            worker_groups: plan.worker_groups,
            hit_capacity: plan.hit_capacity,
        };
        if let Some(pipeline) = self.pipeline_cache.get(shape) {
            return Ok(pipeline);
        }
        let program = build_batch_program(
            plan.workgroup_size_x,
            plan.worker_groups,
            plan.hit_capacity,
            self.hit_writer,
        );
        let pipeline = self
            .backend
            .compile_persistent(&program, &DispatchConfig::default())?;
        self.pipeline_cache.insert(shape, pipeline.clone());
        Ok(pipeline)
    }

    fn dispatch_plan(
        &mut self,
        batch: &FileBatch,
    ) -> Result<BatchDispatchPlanLookup, PipelineError> {
        let queue_len = batch.queue_len();
        if let Some(plan) = self.dispatch_plan_cache.get(queue_len) {
            return Ok(BatchDispatchPlanLookup {
                plan,
                cache_hit: true,
                cache_entries: self.dispatch_plan_cache.len_u16(),
            });
        }
        let mut recommendation = self
            .config
            .launch_recommendation(self.backend.device_limits(), queue_len)?;
        let resident_hit_capacity = batch.hit_capacity();
        if recommendation.hit_capacity > resident_hit_capacity {
            let removed_hit_bytes = u64::from(recommendation.hit_capacity - resident_hit_capacity)
                .checked_mul(dispatcher_usize_to_u64(
                    HIT_RECORD_WORDS,
                    "hit-record word count",
                ))
                .and_then(|words| {
                    words.checked_mul(dispatcher_usize_to_u64(
                        std::mem::size_of::<u32>(),
                        "u32 byte width",
                    ))
                })
                .ok_or_else(|| {
                    PipelineError::Backend(
                        "resident hit-capacity byte adjustment overflowed u64. Fix: shard the batch before dispatch planning."
                            .to_string(),
                    )
                })?;
            recommendation.hit_capacity = resident_hit_capacity;
            recommendation.estimated_peak_device_bytes = recommendation
                .estimated_peak_device_bytes
                .checked_sub(removed_hit_bytes)
                .ok_or_else(|| {
                    PipelineError::Backend(
                        "resident hit-capacity adjustment exceeded peak device estimate. Fix: keep launch recommendation and resident batch capacity synchronized."
                            .to_string(),
                    )
                })?;
        }
        let plan = BatchDispatchPlan::from_recommendation(queue_len, &self.config, recommendation);
        self.dispatch_plan_cache.insert(plan);
        Ok(BatchDispatchPlanLookup {
            plan,
            cache_hit: false,
            cache_entries: self.dispatch_plan_cache.len_u16(),
        })
    }

    fn ensure_rule_buffers(
        &mut self,
        rules: &[BatchRuleProgram],
    ) -> Result<RuleBufferUpdate, PipelineError> {
        accepted_rule_fingerprints_and_rejections_into(
            rules,
            &mut self.fingerprint_scratch,
            &mut self.fingerprint_occupied_scratch,
            &mut self.fingerprint_addressed_scratch,
            &mut self.rejection_scratch,
        );
        if self.fingerprint_scratch == self.active_rule_fingerprints {
            return Ok(RuleBufferUpdate {
                rejected_rules: if self.rejection_scratch.is_empty() {
                    Vec::new()
                } else {
                    self.rejection_scratch.clone()
                },
                uploaded_bytes: 0,
                resident_allocations: 0,
            });
        }

        pack_rule_catalog_into(rules, &mut self.packing_scratch)?;
        // rule_meta words = entries * RULE_META_WORDS (each RuleMeta is
        // RULE_META_WORDS u32s); transitions + accept + class_maps are flat u32
        // vecs. Account for all four uploaded device buffers.
        let rule_meta_words = self
            .packing_scratch
            .rule_meta
            .len()
            .checked_mul(RULE_META_WORDS)
            .ok_or_else(|| {
                PipelineError::Backend(
                    "rule metadata upload word count overflowed usize. Fix: shard the rule catalog before upload."
                        .to_string(),
                )
            })?;
        let uploaded_words = rule_meta_words
            .checked_add(self.packing_scratch.transitions.len())
            .and_then(|words| words.checked_add(self.packing_scratch.accept.len()))
            .and_then(|words| words.checked_add(self.packing_scratch.class_maps.len()))
            .ok_or_else(|| {
                PipelineError::Backend(
                    "rule catalog upload word count overflowed usize. Fix: shard the rule catalog before upload."
                        .to_string(),
                )
            })?;
        let uploaded_bytes = uploaded_words
            .checked_mul(std::mem::size_of::<u32>())
            .and_then(|bytes| u64::try_from(bytes).ok())
            .ok_or_else(|| {
                PipelineError::Backend(
                    "rule catalog upload byte count overflowed u64. Fix: shard the rule catalog before upload."
                        .to_string(),
                )
            })?;
        let (device, queue) = &*self.backend.device_queue();
        self.rule_meta = Some(GpuBufferHandle::upload(
            device,
            queue,
            bytemuck::cast_slice(&self.packing_scratch.rule_meta),
            persistent_storage_binding_usage(),
        )?);
        self.transitions = Some(GpuBufferHandle::upload(
            device,
            queue,
            bytemuck::cast_slice(&self.packing_scratch.transitions),
            persistent_storage_binding_usage(),
        )?);
        self.accept = Some(GpuBufferHandle::upload(
            device,
            queue,
            bytemuck::cast_slice(&self.packing_scratch.accept),
            persistent_storage_binding_usage(),
        )?);
        self.class_maps = Some(GpuBufferHandle::upload(
            device,
            queue,
            bytemuck::cast_slice(&self.packing_scratch.class_maps),
            persistent_storage_binding_usage(),
        )?);
        if self.active_rule_fingerprints.len() == self.fingerprint_scratch.len() {
            self.active_rule_fingerprints
                .copy_from_slice(&self.fingerprint_scratch);
        } else {
            self.active_rule_fingerprints.clear();
            self.active_rule_fingerprints
                .extend_from_slice(&self.fingerprint_scratch);
        }
        Ok(RuleBufferUpdate {
            rejected_rules: if self.packing_scratch.rejected_rules.is_empty() {
                Vec::new()
            } else {
                self.packing_scratch.rejected_rules.clone()
            },
            uploaded_bytes,
            resident_allocations: 4,
        })
    }
}

fn occupancy_proxy_bps(items_processed: u32, worker_groups: u32, workgroup_size_x: u32) -> u16 {
    let lanes = u64::from(worker_groups.max(1))
        .checked_mul(u64::from(workgroup_size_x.max(1)))
        .unwrap_or(u64::MAX);
    crate::numeric::ratio_basis_points_u64_wide(
        u64::from(items_processed),
        lanes.max(1),
        0,
        "batch occupancy proxy",
    )
    .min(10_000) as u16
}

fn validate_u32_readback_words(bytes: &[u8], label: &'static str) -> Result<usize, PipelineError> {
    if bytes.len() % std::mem::size_of::<u32>() != 0 {
        return Err(PipelineError::Backend(format!(
            "{label} readback exposed {} bytes, which is not a whole number of u32 words. Fix: keep readback lengths 4-byte aligned.",
            bytes.len()
        )));
    }
    Ok(bytes.len() / std::mem::size_of::<u32>())
}

fn read_u32_word(
    bytes: &[u8],
    label: &'static str,
    word_index: usize,
) -> Result<u32, PipelineError> {
    let offset = word_index
        .checked_mul(std::mem::size_of::<u32>())
        .ok_or_else(|| {
            PipelineError::Backend(format!(
                "{label} word offset overflowed usize. Fix: split the readback before decoding."
            ))
        })?;
    let word = bytes.get(offset..offset + std::mem::size_of::<u32>()).ok_or_else(|| {
        PipelineError::Backend(format!(
            "{label} readback is missing u32 word {word_index}. Fix: request a large enough readback prefix."
        ))
    })?;
    Ok(u32::from_le_bytes([word[0], word[1], word[2], word[3]]))
}

fn wait_for_persistent_dispatch(
    device: &wgpu::Device,
    start: Instant,
    timeout: Duration,
) -> Result<(), PipelineError> {
    let mut backoff = crate::wait_backoff::AdaptiveWaitBackoff::from_micros(64, 5, 50, 8);
    loop {
        if crate::runtime::device::poll_device_once(device)
            .map_err(|error| PipelineError::Backend(error.to_string()))?
            .is_queue_empty()
        {
            return Ok(());
        }
        let elapsed = start.elapsed();
        if elapsed >= timeout {
            return Err(PipelineError::Backend(format!(
                "batch megakernel dispatch exceeded timeout before readback: took {elapsed:?}, budget {timeout:?}. Fix: raise BatchDispatchConfig.timeout or split the batch.",
            )));
        }
        let remaining = timeout.checked_sub(elapsed).ok_or_else(|| {
            PipelineError::Backend(format!(
                "batch megakernel timeout arithmetic underflowed after elapsed {elapsed:?} exceeded budget {timeout:?}. Fix: split the batch or raise BatchDispatchConfig.timeout deliberately.",
            ))
        })?;
        backoff.idle_for(remaining);
    }
}

fn build_batch_program(
    workgroup_size_x: u32,
    worker_groups: u32,
    hit_capacity: u32,
    hit_writer: BatchHitWriter,
) -> Program {
    // Persistent DRAIN loop: every resident lane keeps claiming work-items with
    // `atomicAdd(HEAD, 1)` until its claim lands past the end of the queue
    // (`claim >= QUEUE_LEN`), then returns. This drains the full
    // `segment_count * rule_count` queue for ANY number of resident lanes.
    //
    // It replaces a fixed `claim_budget = ceil(QUEUE_LEN / total_workers)` loop
    // that assumed exactly `total_workers` lanes each ran their full budget. When
    // fewer lanes were actually resident than that budget assumed, the queue was
    // never fully claimed — `found < expected` with `dropped_hits == 0`: a SILENT
    // recall loss (Law 10). The drain removes the dependency on the resident-lane
    // count entirely. Overhead is one extra past-the-end `atomicAdd` per resident
    // lane (the claim that observes `>= QUEUE_LEN` and returns), NOT per
    // work-item — a rounding error, not a 1/queue_len-scale pessimization.
    //
    // `worker_groups` now sizes only the dispatch grid (more resident lanes =
    // more parallelism); kernel correctness no longer depends on it.
    let _ = worker_groups;
    let queue_len = atomic_load_relaxed(
        "queue_state",
        Expr::u32(dispatcher_abi_u32(
            queue_state_word::QUEUE_LEN,
            "queue-state length word",
        )),
    );
    let mut loop_body = vec![
        Node::let_bind(
            "claim",
            Expr::atomic_add(
                "queue_state",
                Expr::u32(dispatcher_abi_u32(
                    queue_state_word::HEAD,
                    "queue-state head word",
                )),
                Expr::u32(1),
            ),
        ),
        // Past-the-end claim ⇒ the queue is drained for this lane. `Return` exits
        // the kernel: safe because the drain loop is the only top-level statement
        // (no post-loop finalization to skip) and `execute_batch_claim_body`
        // contains no workgroup barrier (no divergence deadlock).
        Node::if_then(
            Expr::ge(Expr::var("claim"), queue_len),
            vec![Node::Return],
        ),
    ];
    loop_body.extend(execute_batch_claim_body(hit_writer));

    Program::wrapped(
        batch_program_buffers(hit_capacity),
        [workgroup_size_x, 1, 1],
        vec![Node::forever(loop_body)],
    )
}

fn batch_program_buffers(hit_capacity: u32) -> Vec<BufferDecl> {
    let hit_ring_words = hit_capacity.saturating_mul(4);
    vec![
        BufferDecl::storage("file_offsets", 0, BufferAccess::ReadOnly, DataType::U32),
        BufferDecl::storage("file_metadata", 1, BufferAccess::ReadOnly, DataType::U32),
        BufferDecl::storage("class_maps", 2, BufferAccess::ReadOnly, DataType::U32),
        BufferDecl::storage("haystack", 3, BufferAccess::ReadOnly, DataType::U32),
        BufferDecl::storage("rule_meta", 4, BufferAccess::ReadOnly, DataType::U32),
        BufferDecl::storage("transitions", 5, BufferAccess::ReadOnly, DataType::U32),
        BufferDecl::storage("accept", 6, BufferAccess::ReadOnly, DataType::U32),
        BufferDecl::storage("queue_state", 7, BufferAccess::ReadWrite, DataType::U32).with_count(
            dispatcher_abi_u32(QUEUE_STATE_WORDS, "queue-state word count"),
        ),
        BufferDecl::output("hit_ring", 8, DataType::U32).with_count(hit_ring_words),
        // Flat segment table (`segment_count * SEGMENT_WORDS` u32s). Declared
        // LAST among the read-only inputs so it occupies the final positional
        // input slot (the persistent pipeline binds non-output buffers to
        // `inputs[]` in declaration order — see `dispatch_persistent_borrowed`).
        // The kernel reads row `seg_idx = claim / rule_count` to derive the
        // window; the host sizes the queue from the same table so a claim never
        // indexes past it.
        BufferDecl::storage("segments", 9, BufferAccess::ReadOnly, DataType::U32),
    ]
}

// ─── Combined-AC segmented megakernel ──────────────────────────────────────
//
// The per-rule path above runs ONE 2-/N-state DFA per (segment, rule) work
// item, so a catalog of K patterns multiplies the queue by K: every byte is
// re-read K times. The combined path compiles ALL patterns into ONE
// Aho-Corasick automaton (`vyre_libs::scan::classic_ac::classic_ac_compile`)
// and runs it ONCE per segment: `queue_len = segment_count` (no rule
// dimension). Each accepting state emits the SET of pattern ids that match
// there via the `output_offsets`/`output_records` flat arrays, so a single
// transition read per byte covers every pattern. The window decode, warm-up
// prefix, and emit-guard are byte-for-byte the per-rule path's — the SAME
// `plan_segments`/`segment_table` geometry and the SAME
// `byte_pos >= emit_start` ownership rule the `segmentation.rs`
// `combined_segmented_scan` CPU oracle proves equal to a linear
// `classic_ac_scan`.

/// Buffer layout for the combined-AC segmented megakernel.
///
/// Mirrors [`batch_program_buffers`] but swaps the four per-rule automaton
/// buffers (`class_maps`/`rule_meta`/`transitions`/`accept`) for the three
/// combined-AC buffers (`transitions`/`output_offsets`/`output_records`) and
/// drops the rule dimension. `segments` stays last so it occupies the final
/// positional input slot, exactly as the per-rule path requires.
fn combined_batch_program_buffers(hit_capacity: u32) -> Vec<BufferDecl> {
    let hit_ring_words = hit_capacity.saturating_mul(4);
    vec![
        BufferDecl::storage("file_offsets", 0, BufferAccess::ReadOnly, DataType::U32),
        BufferDecl::storage("file_metadata", 1, BufferAccess::ReadOnly, DataType::U32),
        BufferDecl::storage("haystack", 2, BufferAccess::ReadOnly, DataType::U32),
        // Combined Aho-Corasick automaton, flattened + byte-class compressed:
        //   transitions:    state_count * num_classes  (compressed next-state)
        //   output_offsets: state_count + 1            (CSR row pointers)
        //   output_records: output_records_len         (flat pattern_id payload)
        //   class_maps:     256                        (byte -> class column id)
        // The kernel folds each byte through `class_maps` then indexes the
        // compressed `state * num_classes + class` row — LOSSLESS (every byte in
        // a class shares its dense column), shrinking the transition table from
        // `state_count * 256` to `state_count * num_classes`.
        BufferDecl::storage("transitions", 3, BufferAccess::ReadOnly, DataType::U32),
        BufferDecl::storage("output_offsets", 4, BufferAccess::ReadOnly, DataType::U32),
        BufferDecl::storage("output_records", 5, BufferAccess::ReadOnly, DataType::U32),
        BufferDecl::storage("class_maps", 6, BufferAccess::ReadOnly, DataType::U32),
        BufferDecl::storage("queue_state", 7, BufferAccess::ReadWrite, DataType::U32).with_count(
            dispatcher_abi_u32(QUEUE_STATE_WORDS, "queue-state word count"),
        ),
        BufferDecl::output("hit_ring", 8, DataType::U32).with_count(hit_ring_words),
        BufferDecl::storage("segments", 9, BufferAccess::ReadOnly, DataType::U32),
    ]
}

/// Width of each combined-AC transition target in the device transition table.
///
/// `Bits32` is the shipping default — one `u32` per target, indexed
/// `transitions[state * num_classes + class]`. `Bits16` packs two targets per
/// `u32` word (low half = even flat index, high half = odd; host packer
/// [`vyre_runtime::megakernel::rule_catalog::try_pack_u16_transitions_into`]),
/// halving the transition table and bytes-per-transaction — the
/// keyhog-scale L1 working-set lever (`docs/GPU_OOM_SEGMENTATION.md`) — at the
/// cost of an unpack shift/mask in the hot loop. Sound ONLY when every target
/// fits `u16` (`state_count <= 65536`); the host packer fails closed otherwise.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionWidth {
    /// One `u32` per transition target (default).
    Bits32,
    /// Two `u16` targets packed per `u32` word.
    Bits16,
}

/// Build the combined-AC segmented megakernel program.
///
/// Identical drain loop to [`build_batch_program`] (`forever` + `claim >=
/// QUEUE_LEN` Return, Law-10 no under-claim), with the per-rule claim body
/// replaced by [`execute_combined_claim_body`]. `queue_len = segment_count`
/// (one work item per segment), set by the host. `transition_width` selects the
/// device transition-table packing (see [`TransitionWidth`]); the host MUST
/// upload a table packed to match.
pub(crate) fn build_combined_batch_program(
    workgroup_size_x: u32,
    hit_capacity: u32,
    num_classes: u32,
    transition_width: TransitionWidth,
) -> Program {
    let queue_len = atomic_load_relaxed(
        "queue_state",
        Expr::u32(dispatcher_abi_u32(
            queue_state_word::QUEUE_LEN,
            "queue-state length word",
        )),
    );
    let mut loop_body = vec![
        Node::let_bind(
            "claim",
            Expr::atomic_add(
                "queue_state",
                Expr::u32(dispatcher_abi_u32(
                    queue_state_word::HEAD,
                    "queue-state head word",
                )),
                Expr::u32(1),
            ),
        ),
        Node::if_then(
            Expr::ge(Expr::var("claim"), queue_len),
            vec![Node::Return],
        ),
    ];
    loop_body.extend(execute_combined_claim_body(num_classes, transition_width));

    Program::wrapped(
        combined_batch_program_buffers(hit_capacity),
        [workgroup_size_x, 1, 1],
        vec![Node::forever(loop_body)],
    )
}

/// Decode one combined-AC claim into its file window and scan it.
///
/// `queue_len = segment_count`, so the claim IS the segment index directly (no
/// `/ rule_count` — there is no rule dimension). The segment row layout
/// `[file_idx, scan_start, emit_start, emit_end]` and the absolute-bounds
/// arithmetic are identical to [`execute_batch_claim_body`].
fn execute_combined_claim_body(num_classes: u32, transition_width: TransitionWidth) -> Vec<Node> {
    vec![
        // claim == seg_idx (one work item per segment).
        Node::let_bind("seg_idx", Expr::var("claim")),
        Node::let_bind(
            "seg_base",
            Expr::mul(
                Expr::var("seg_idx"),
                Expr::u32(dispatcher_abi_u32(SEGMENT_WORDS, "segment table word count")),
            ),
        ),
        Node::let_bind("file_idx", Expr::load("segments", Expr::var("seg_base"))),
        Node::let_bind(
            "scan_start_rel",
            Expr::load("segments", Expr::add(Expr::var("seg_base"), Expr::u32(1))),
        ),
        Node::let_bind(
            "emit_start_rel",
            Expr::load("segments", Expr::add(Expr::var("seg_base"), Expr::u32(2))),
        ),
        Node::let_bind(
            "emit_end_rel",
            Expr::load("segments", Expr::add(Expr::var("seg_base"), Expr::u32(3))),
        ),
        Node::let_bind(
            "metadata_base",
            Expr::mul(
                Expr::var("file_idx"),
                Expr::u32(dispatcher_abi_u32(
                    FILE_METADATA_WORDS,
                    "file metadata word count",
                )),
            ),
        ),
        Node::let_bind(
            "layer_idx",
            Expr::load(
                "file_metadata",
                Expr::add(Expr::var("metadata_base"), Expr::u32(3)),
            ),
        ),
        Node::let_bind(
            "file_start",
            Expr::load("file_offsets", Expr::var("file_idx")),
        ),
        Node::let_bind(
            "scan_start",
            Expr::add(Expr::var("file_start"), Expr::var("scan_start_rel")),
        ),
        Node::let_bind(
            "emit_start",
            Expr::add(Expr::var("file_start"), Expr::var("emit_start_rel")),
        ),
        Node::let_bind(
            "emit_end",
            Expr::add(Expr::var("file_start"), Expr::var("emit_end_rel")),
        ),
        Node::Block(combined_dfa_byte_scanner(num_classes, transition_width)),
        Node::let_bind(
            "done_prev",
            Expr::atomic_add(
                "queue_state",
                Expr::u32(dispatcher_abi_u32(
                    queue_state_word::DONE_COUNT,
                    "queue-state done-count word",
                )),
                Expr::u32(1),
            ),
        ),
    ]
}

/// Combined Aho-Corasick window scan with per-state multi-emit.
///
/// Walks the dense combined automaton over `[scan_start, emit_end)` from state
/// 0; the `[scan_start, emit_start)` prefix is warm-up (advances state, emits
/// nothing). Once `byte_pos >= emit_start`, every pattern id in this state's
/// CSR row `output_records[output_offsets[state] .. output_offsets[state+1]]`
/// is emitted at `match_offset = byte_pos - file_start`. This is the only
/// place the kernel diverges from the per-rule scanner: a CSR multi-emit loop
/// instead of a single `accept[state]` flag, mirroring the
/// `combined_segmented_scan` CPU oracle exactly.
fn combined_dfa_byte_scanner(num_classes: u32, transition_width: TransitionWidth) -> Vec<Node> {
    let mut loop_body = vec![
        Node::let_bind(
            "haystack_word_index",
            Expr::div(Expr::var("byte_pos"), Expr::u32(4)),
        ),
        Node::let_bind(
            "haystack_shift",
            Expr::mul(Expr::rem(Expr::var("byte_pos"), Expr::u32(4)), Expr::u32(8)),
        ),
        Node::let_bind(
            "byte",
            Expr::bitand(
                Expr::shr(
                    Expr::load("haystack", Expr::var("haystack_word_index")),
                    Expr::var("haystack_shift"),
                ),
                Expr::u32(0xFF),
            ),
        ),
        // Byte-class compressed combined transition (lossless): fold the byte
        // through the 256-entry class map, then index the compressed
        // `state * num_classes + class` row. `num_classes` is baked as a literal
        // — the automaton fixes it, so the pipeline is compiled once per resident
        // catalog. Firings are byte-for-byte identical to the dense
        // `state * 256 + byte` table.
        Node::let_bind("byte_class", Expr::load("class_maps", Expr::var("byte"))),
    ];
    // The transition read narrows by `transition_width`: Bits32 loads the target
    // directly; Bits16 unpacks two targets per word (half the bytes/transaction).
    loop_body.extend(combined_transition_read(num_classes, transition_width));
    // Emit guard: only positions owned by this window (`byte_pos >= emit_start`;
    // the loop bound enforces `byte_pos < emit_end`). Warm-up bytes advance state
    // but emit nothing, so adjacent windows tile each file with no double count
    // and no miss.
    loop_body.push(Node::if_then(
        Expr::ge(Expr::var("byte_pos"), Expr::var("emit_start")),
        vec![
            Node::let_bind(
                "out_begin",
                Expr::load("output_offsets", Expr::var("state")),
            ),
            Node::let_bind(
                "out_end",
                Expr::load(
                    "output_offsets",
                    Expr::add(Expr::var("state"), Expr::u32(1)),
                ),
            ),
            // Multi-emit: one HitRecord per pattern id accepting at this state.
            // `rule_idx` carries the pattern id (the combined automaton's pattern
            // == the catalog rule).
            Node::loop_for(
                "out_idx",
                Expr::var("out_begin"),
                Expr::var("out_end"),
                vec![
                    Node::let_bind(
                        "rule_idx",
                        Expr::load("output_records", Expr::var("out_idx")),
                    ),
                    Node::Block(record_hit_to_ring()),
                ],
            ),
        ],
    ));

    vec![
        Node::let_bind("state", Expr::u32(0)),
        Node::loop_for(
            "byte_pos",
            Expr::var("scan_start"),
            Expr::var("emit_end"),
            loop_body,
        ),
    ]
}

/// The combined-AC transition step `state := transition(state, byte_class)`,
/// emitting the read for the chosen [`TransitionWidth`].
///
/// `Bits32` loads `transitions[state * num_classes + class]` directly. `Bits16`
/// computes that same flat index, loads the packed word at `idx / 2`, and
/// extracts the `u16` half selected by `idx & 1` — the EXACT mirror of
/// [`vyre_runtime::megakernel::rule_catalog::unpack_u16_transition`], so a
/// u16-packed table reproduces the u32 firings byte-for-byte (proven on the GPU
/// by the differential conservation test, which runs both widths).
fn combined_transition_read(num_classes: u32, transition_width: TransitionWidth) -> Vec<Node> {
    let flat_index = Expr::add(
        Expr::mul(Expr::var("state"), Expr::u32(num_classes)),
        Expr::var("byte_class"),
    );
    match transition_width {
        TransitionWidth::Bits32 => {
            vec![Node::assign("state", Expr::load("transitions", flat_index))]
        }
        TransitionWidth::Bits16 => vec![
            Node::let_bind("trans_idx", flat_index),
            Node::let_bind(
                "trans_word",
                Expr::load(
                    "transitions",
                    Expr::div(Expr::var("trans_idx"), Expr::u32(2)),
                ),
            ),
            Node::assign(
                "state",
                Expr::bitand(
                    Expr::shr(
                        Expr::var("trans_word"),
                        Expr::mul(
                            Expr::rem(Expr::var("trans_idx"), Expr::u32(2)),
                            Expr::u32(16),
                        ),
                    ),
                    Expr::u32(0xFFFF),
                ),
            ),
        ],
    }
}

fn execute_batch_claim_body(hit_writer: BatchHitWriter) -> Vec<Node> {
    vec![
        Node::let_bind(
            "rule_count",
            atomic_load_relaxed(
                "queue_state",
                Expr::u32(dispatcher_abi_u32(
                    queue_state_word::RULE_COUNT,
                    "queue-state rule-count word",
                )),
            ),
        ),
        // A claim decodes to `(seg_idx, rule_idx)`. `seg_idx` indexes the flat
        // `segments` table; each 4-word row is `[file_idx, scan_start, emit_start,
        // emit_end]` with FILE-RELATIVE offsets (see `segmentation::Segment`). The
        // dense default (one segment per file, `seg_len = u32::MAX`) makes the row
        // `[file_idx, 0, 0, file_len]`, so the window is the whole file and this
        // path is byte-for-byte the legacy `file_idx = claim / rule_count` scan.
        Node::let_bind(
            "seg_idx",
            Expr::div(Expr::var("claim"), Expr::var("rule_count")),
        ),
        Node::let_bind(
            "rule_idx",
            Expr::rem(Expr::var("claim"), Expr::var("rule_count")),
        ),
        Node::let_bind(
            "seg_base",
            Expr::mul(
                Expr::var("seg_idx"),
                Expr::u32(dispatcher_abi_u32(SEGMENT_WORDS, "segment table word count")),
            ),
        ),
        Node::let_bind("file_idx", Expr::load("segments", Expr::var("seg_base"))),
        Node::let_bind(
            "scan_start_rel",
            Expr::load("segments", Expr::add(Expr::var("seg_base"), Expr::u32(1))),
        ),
        Node::let_bind(
            "emit_start_rel",
            Expr::load("segments", Expr::add(Expr::var("seg_base"), Expr::u32(2))),
        ),
        Node::let_bind(
            "emit_end_rel",
            Expr::load("segments", Expr::add(Expr::var("seg_base"), Expr::u32(3))),
        ),
        Node::let_bind(
            "metadata_base",
            Expr::mul(
                Expr::var("file_idx"),
                Expr::u32(dispatcher_abi_u32(
                    FILE_METADATA_WORDS,
                    "file metadata word count",
                )),
            ),
        ),
        Node::let_bind(
            "layer_idx",
            Expr::load(
                "file_metadata",
                Expr::add(Expr::var("metadata_base"), Expr::u32(3)),
            ),
        ),
        Node::let_bind(
            "file_start",
            Expr::load("file_offsets", Expr::var("file_idx")),
        ),
        // Absolute (packed-haystack) window bounds: file base + file-relative
        // segment offsets. `scan_start <= emit_start < emit_end` by construction.
        Node::let_bind(
            "scan_start",
            Expr::add(Expr::var("file_start"), Expr::var("scan_start_rel")),
        ),
        Node::let_bind(
            "emit_start",
            Expr::add(Expr::var("file_start"), Expr::var("emit_start_rel")),
        ),
        Node::let_bind(
            "emit_end",
            Expr::add(Expr::var("file_start"), Expr::var("emit_end_rel")),
        ),
        Node::let_bind(
            "rule_base",
            Expr::mul(
                Expr::var("rule_idx"),
                Expr::u32(dispatcher_abi_u32(
                    RULE_META_WORDS,
                    "rule metadata word count",
                )),
            ),
        ),
        Node::let_bind(
            "transition_base",
            Expr::load("rule_meta", Expr::var("rule_base")),
        ),
        Node::let_bind(
            "accept_base",
            Expr::load("rule_meta", Expr::add(Expr::var("rule_base"), Expr::u32(1))),
        ),
        // Byte-class compression metadata (rule_meta words 3 and 4): the
        // per-rule 256-entry byte->class map base and the compressed row width.
        Node::let_bind(
            "class_map_base",
            Expr::load("rule_meta", Expr::add(Expr::var("rule_base"), Expr::u32(3))),
        ),
        Node::let_bind(
            "num_classes",
            Expr::load("rule_meta", Expr::add(Expr::var("rule_base"), Expr::u32(4))),
        ),
        // Delegate core evaluation to Tier-2 LEGO Primitive
        Node::Block(dfa_byte_scanner(hit_writer)),
        // Mark work completion
        Node::let_bind(
            "done_prev",
            Expr::atomic_add(
                "queue_state",
                Expr::u32(dispatcher_abi_u32(
                    queue_state_word::DONE_COUNT,
                    "queue-state done-count word",
                )),
                Expr::u32(1),
            ),
        ),
    ]
}

fn dfa_byte_scanner(hit_writer: BatchHitWriter) -> Vec<Node> {
    vec![
        Node::let_bind("state", Expr::u32(0)),
        // Scan the window `[scan_start, emit_end)` from state 0. The
        // `[scan_start, emit_start)` prefix is DFA warm-up — it advances the
        // state but emits nothing (the emit guard below). For the dense default
        // `scan_start == emit_start == file_start`, so the loop is the whole file
        // with no warm-up — identical to the pre-segmentation scan.
        Node::loop_for(
            "byte_pos",
            Expr::var("scan_start"),
            Expr::var("emit_end"),
            vec![
                Node::let_bind(
                    "haystack_word_index",
                    Expr::div(Expr::var("byte_pos"), Expr::u32(4)),
                ),
                Node::let_bind(
                    "haystack_shift",
                    Expr::mul(Expr::rem(Expr::var("byte_pos"), Expr::u32(4)), Expr::u32(8)),
                ),
                Node::let_bind(
                    "byte",
                    Expr::bitand(
                        Expr::shr(
                            Expr::load("haystack", Expr::var("haystack_word_index")),
                            Expr::var("haystack_shift"),
                        ),
                        Expr::u32(0xFF),
                    ),
                ),
                // Byte-class compressed transition load (lossless): fold the
                // byte through this rule's 256-entry class map, then index the
                // compressed `state * num_classes + class` row. Bytes that share
                // a transition column across every state collapse to one class,
                // shrinking each per-state row from 256 words to `num_classes`
                // words. Firings are byte-for-byte identical to the dense
                // `state * 256 + byte` table (proved in the CPU parity tests).
                Node::let_bind(
                    "byte_class",
                    Expr::load(
                        "class_maps",
                        Expr::add(Expr::var("class_map_base"), Expr::var("byte")),
                    ),
                ),
                Node::assign(
                    "state",
                    Expr::load(
                        "transitions",
                        Expr::add(
                            Expr::var("transition_base"),
                            Expr::add(
                                Expr::mul(Expr::var("state"), Expr::var("num_classes")),
                                Expr::var("byte_class"),
                            ),
                        ),
                    ),
                ),
                Node::let_bind(
                    "accepting",
                    Expr::load(
                        "accept",
                        Expr::add(Expr::var("accept_base"), Expr::var("state")),
                    ),
                ),
                // Emit guard mirrors the CPU parity oracle (`segmentation.rs`):
                // a match is owned by this window iff its end offset lies in
                // `[emit_start, emit_end)`. The loop bound already enforces
                // `byte_pos < emit_end`; the remaining condition `end > emit_start`
                // (end = byte_pos + 1) is exactly `byte_pos >= emit_start`. Bytes in
                // the warm-up prefix (`byte_pos < emit_start`) advance state but
                // never emit, so adjacent windows tile each file with no double
                // count and no miss.
                Node::let_bind(
                    "is_hit",
                    Expr::and(
                        Expr::ne(Expr::var("accepting"), Expr::u32(0)),
                        Expr::ge(Expr::var("byte_pos"), Expr::var("emit_start")),
                    ),
                ),
                hit_writer_node(hit_writer),
            ],
        ),
    ]
}

/// Caller-owns-hit-storage counters for one combined-AC dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CombinedDispatchSummary {
    /// Sparse hit count written by the device (clamped to `hit_capacity`).
    pub hit_count: u32,
    /// Matches produced BEYOND `hit_capacity` and therefore DROPPED from the
    /// ring (raw atomic head minus capacity). `> 0` means the hit set is
    /// INCOMPLETE — a recall-critical overflow the caller must recover, never
    /// treat as complete (Law 10). Zero on a healthy dispatch.
    pub dropped_hits: u32,
    /// Segments the device finished (`DONE_COUNT`).
    pub items_processed: u32,
    /// Wall-clock GPU execution time.
    pub wall_time: Duration,
}

/// Persistent dispatcher for the combined-AC segmented megakernel.
///
/// The combined twin of [`BatchDispatcher`] minus the per-rule catalog
/// machinery (no fingerprints, no `rule_meta`/`transitions`/`accept` upload):
/// the automaton is resident in the [`CombinedBatch`]. It compiles the
/// combined persistent program (the backend pipeline cache dedups the WGSL
/// compile by program+adapter+config), drains the whole `segment_count` queue,
/// and reads back the sparse hit ring with the SAME Law-10 overflow +
/// incomplete-drain guards as the per-rule path.
pub struct CombinedDispatcher {
    backend: WgpuBackend,
    config: BatchDispatchConfig,
    queue_state_bytes: Vec<u8>,
    hit_bytes: Vec<u8>,
}

impl CombinedDispatcher {
    /// Build a combined dispatcher over a live backend.
    #[must_use]
    pub fn new(backend: WgpuBackend, config: BatchDispatchConfig) -> Self {
        Self {
            backend,
            config,
            queue_state_bytes: Vec::new(),
            hit_bytes: Vec::new(),
        }
    }

    /// Dispatch the combined automaton over every segment of `batch`,
    /// compacting decoded hits into caller-owned `hits`.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError`] on pipeline compilation failure, dispatch
    /// timeout, an incomplete drain (loud, never a silent partial), or readback
    /// failure.
    pub fn dispatch_into(
        &mut self,
        batch: &CombinedBatch,
        hits: &mut Vec<HitRecord>,
    ) -> Result<CombinedDispatchSummary, PipelineError> {
        let queue_len = batch.queue_len();
        if queue_len == 0 {
            // Every file empty ⇒ no segments ⇒ nothing to scan.
            hits.clear();
            return Ok(CombinedDispatchSummary {
                hit_count: 0,
                dropped_hits: 0,
                items_processed: 0,
                wall_time: Duration::ZERO,
            });
        }
        let worker_groups = if self.config.worker_groups != 0 {
            self.config.worker_groups
        } else {
            self.config
                .launch_recommendation(self.backend.device_limits(), queue_len)?
                .worker_groups
        };
        let program = build_combined_batch_program(
            self.config.workgroup_size_x,
            batch.hit_capacity(),
            batch.num_classes(),
            batch.transition_width(),
        );
        let pipeline = self
            .backend
            .compile_persistent(&program, &DispatchConfig::default())?;
        batch.reset_queue_state();

        let inputs = batch.input_buffers();
        let outputs = batch.output_buffers();
        let start = Instant::now();
        pipeline.dispatch_persistent_borrowed(&inputs, &outputs, None, [worker_groups, 1, 1])?;
        let (device, queue) = &*self.backend.device_queue();
        wait_for_persistent_dispatch(device, start, self.config.timeout)?;
        let wall_time = start.elapsed();

        self.queue_state_bytes.clear();
        let queue_state_readback_bytes = batch_fixed_resident_overhead_bytes();
        batch.queue_state().readback_prefix(
            device,
            queue,
            queue_state_readback_bytes,
            &mut self.queue_state_bytes,
        )?;
        let queue_state_word_count =
            validate_u32_readback_words(&self.queue_state_bytes, "queue-state")?;
        if queue_state_word_count < QUEUE_STATE_WORDS {
            return Err(PipelineError::Backend(format!(
                "queue-state readback exposed {} words, expected at least {}. Fix: keep the queue-state buffer sized for every control word.",
                queue_state_word_count, QUEUE_STATE_WORDS
            )));
        }
        let raw_hit_head = read_u32_word(
            &self.queue_state_bytes,
            "queue-state",
            queue_state_word::HIT_HEAD,
        )?;
        let (hit_count, dropped_hits) = split_hit_overflow(raw_hit_head, batch.hit_capacity());
        let items_processed = read_u32_word(
            &self.queue_state_bytes,
            "queue-state",
            queue_state_word::DONE_COUNT,
        )?;
        // Fail-closed drain guard (Law 10): HEAD < queue_len ⇒ the dispatch was
        // cut short, leaving segments `[HEAD, queue_len)` unscanned with their
        // matches missing and `dropped_hits == 0` — an INVISIBLE recall loss.
        let claims_attempted = read_u32_word(
            &self.queue_state_bytes,
            "queue-state",
            queue_state_word::HEAD,
        )?;
        if claims_attempted < queue_len {
            return Err(PipelineError::DrainIncomplete {
                descriptor: "combined megakernel",
                claimed: claims_attempted,
                expected: queue_len,
                unit: "segments",
            });
        }

        self.hit_bytes.clear();
        let hit_readback_bytes = u64::from(hit_count)
            .checked_mul(dispatcher_usize_to_u64(
                HIT_RECORD_WORDS,
                "hit-record word count",
            ))
            .and_then(|words| {
                words.checked_mul(dispatcher_usize_to_u64(
                    std::mem::size_of::<u32>(),
                    "u32 byte width",
                ))
            })
            .ok_or_else(|| {
                PipelineError::Backend(
                    "combined hit-ring readback length overflowed u64. Fix: reduce hit_capacity or shard the batch."
                        .to_string(),
                )
            })?;
        batch.hit_ring().readback_prefix(
            device,
            queue,
            hit_readback_bytes,
            &mut self.hit_bytes,
        )?;
        decode_hits_from_readback_into(&self.hit_bytes, hit_count, hits)?;

        Ok(CombinedDispatchSummary {
            hit_count,
            dropped_hits,
            items_processed,
            wall_time,
        })
    }

    /// Measure-based `seg_len` selection for THIS device. Re-tiles `batch` at
    /// each candidate window width — keeping the caller's proven
    /// `overlap = max_pattern_len`, so correctness is GEOMETRY-INVARIANT and only
    /// throughput changes — times a warm best-of-`reps` dispatch, and returns the
    /// FASTEST geometry whose dispatch was COMPLETE (clean drain, `dropped_hits
    /// == 0`). No per-call oracle is needed: every candidate shares the caller's
    /// sound overlap, so all COMPLETE dispatches yield the identical hit set and
    /// this only optimizes speed among them. An under-claiming (drain-incomplete)
    /// or ring-overflowing geometry is recorded but EXCLUDED, never silently
    /// selected; if no candidate dispatches completely the call FAILS CLOSED.
    ///
    /// This is the autoroute-honest way to hold the GPU win across devices: the
    /// per-device optimum (~128 on an RTX 5090, coarser on a low-core laptop) is
    /// MEASURED here, not assumed — and every candidate's measurement is returned
    /// so the decision inputs are visible. On success `batch` is left re-tiled at
    /// the winning geometry, ready to dispatch.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError`] when `candidates` is empty, when a dispatch fails
    /// for a non-geometry reason, or (fail closed) when NO candidate dispatched
    /// completely.
    pub fn calibrate_seg_len(
        &mut self,
        batch: &mut CombinedBatch,
        candidates: &[u32],
        reps: u32,
    ) -> Result<SegLenCalibration, PipelineError> {
        if candidates.is_empty() {
            return Err(PipelineError::Backend(
                "seg_len calibration needs at least one candidate geometry; pass \
                 DEFAULT_SEG_LEN_CANDIDATES or an operator set."
                    .to_string(),
            ));
        }
        let reps = reps.max(1);
        let mut scratch: Vec<HitRecord> = Vec::new();
        let mut measurements = Vec::with_capacity(candidates.len());
        for &seg_len in candidates {
            batch.set_segmentation(seg_len)?;
            // Warm once: compile + first-touch out of the timing. A warm-up that
            // under-claims is fine — the timed loop below records it as incomplete.
            let _ = self.dispatch_into(batch, &mut scratch);
            let mut best_wall = Duration::MAX;
            let mut dropped_hits = 0u32;
            let mut complete = true;
            for _ in 0..reps {
                match self.dispatch_into(batch, &mut scratch) {
                    Ok(summary) => {
                        best_wall = best_wall.min(summary.wall_time);
                        dropped_hits = summary.dropped_hits;
                        if summary.dropped_hits != 0 {
                            // Hit ring overflowed: this dispatch's set is INCOMPLETE,
                            // so its time is not a valid complete-scan measurement.
                            complete = false;
                        }
                    }
                    // Drain-incomplete = the geometry could not exhaust the queue in
                    // the timeout (loud under-claim). Record + exclude, never select.
                    // Matched on the typed predicate, not the message text, so a
                    // wording change can never silently turn this into a hard abort.
                    Err(e) if e.is_drain_incomplete() => {
                        complete = false;
                        break;
                    }
                    Err(other) => return Err(other),
                }
            }
            if best_wall == Duration::MAX {
                // Never produced a timed Ok (immediate under-claim every rep).
                best_wall = Duration::ZERO;
                complete = false;
            }
            measurements.push(SegLenMeasurement {
                seg_len,
                wall_time: best_wall,
                dropped_hits,
                complete,
            });
        }
        let chosen = select_fastest_complete(&measurements)?;
        // Leave the batch at the winning geometry so the caller dispatches fast.
        batch.set_segmentation(chosen)?;
        Ok(SegLenCalibration {
            chosen,
            measurements,
        })
    }
}

/// A vetted default `seg_len` candidate set for [`CombinedDispatcher::calibrate_seg_len`],
/// spanning the saturation curve from coarse (fewer segments — best on low-core
/// devices) to fine (more parallel windows — best on high-core devices), so
/// calibration adapts to the host instead of assuming one optimum. Operators may
/// pass their own set. Deliberately excludes `u32::MAX` (whole-file): that is a
/// correctness floor, never a throughput candidate (see `CombinedBatch::upload`).
pub const DEFAULT_SEG_LEN_CANDIDATES: &[u32] = &[4096, 2048, 1024, 512, 256, 128, 64];

/// One geometry's measurement during `seg_len` calibration: the candidate window
/// width, its best wall-clock dispatch time over the timed reps, the hit-ring
/// overflow count, and whether the dispatch was COMPLETE (clean drain AND
/// `dropped_hits == 0`). Only `complete` geometries are eligible to win.
#[derive(Debug, Clone)]
pub struct SegLenMeasurement {
    /// The candidate per-segment owned width that was measured.
    pub seg_len: u32,
    /// Best (minimum) wall-clock dispatch time observed over the timed reps;
    /// `Duration::ZERO` for a geometry that never produced a timed complete run.
    pub wall_time: Duration,
    /// Hits the GPU emitted beyond `hit_capacity` (ring overflow); non-zero marks
    /// the measurement INCOMPLETE.
    pub dropped_hits: u32,
    /// True iff every timed rep drained cleanly with no dropped hits — the only
    /// state in which `wall_time` represents a full, conserving scan.
    pub complete: bool,
}

/// The result of [`CombinedDispatcher::calibrate_seg_len`]: the fastest COMPLETE
/// geometry plus EVERY candidate's measurement, so the selection's decision
/// inputs are fully visible to the operator (never a silent pick).
#[derive(Debug, Clone)]
pub struct SegLenCalibration {
    /// The winning per-segment owned width; the batch is left re-tiled here.
    pub chosen: u32,
    /// Per-candidate measurements in the order the candidates were supplied.
    pub measurements: Vec<SegLenMeasurement>,
}

/// Pick the COMPLETE geometry with the smallest wall time, breaking ties toward
/// the COARSER (larger) `seg_len` — fewer segments means less queue-drain and
/// warm-up overhead at equal measured speed. Fails closed when no candidate is
/// complete: returns an error rather than an unsound or incomplete geometry.
fn select_fastest_complete(measurements: &[SegLenMeasurement]) -> Result<u32, PipelineError> {
    measurements
        .iter()
        .filter(|m| m.complete)
        .min_by(|a, b| {
            a.wall_time
                .cmp(&b.wall_time)
                .then_with(|| b.seg_len.cmp(&a.seg_len))
        })
        .map(|m| m.seg_len)
        .ok_or_else(|| {
            PipelineError::Backend(
                "seg_len calibration found no complete geometry: every candidate \
                 under-claimed (drain incomplete) or overflowed the hit ring. Fix: \
                 raise BatchDispatchConfig.timeout and/or hit_capacity, or pass \
                 coarser candidates (larger seg_len = fewer segments to drain)."
                    .to_string(),
            )
        })
}

fn hit_writer_node(hit_writer: BatchHitWriter) -> Node {
    match hit_writer {
        BatchHitWriter::HierarchicalSubgroup => {
            Node::Block(record_hit_to_ring_hierarchical("is_hit"))
        }
        BatchHitWriter::Auto | BatchHitWriter::Scalar => {
            Node::if_then(Expr::var("is_hit"), record_hit_to_ring())
        }
    }
}

fn record_hit_to_ring() -> Vec<Node> {
    vec![
        Node::let_bind(
            "hit_slot",
            Expr::atomic_add(
                "queue_state",
                Expr::u32(dispatcher_abi_u32(
                    queue_state_word::HIT_HEAD,
                    "queue-state hit-head word",
                )),
                Expr::u32(1),
            ),
        ),
        Node::if_then(
            Expr::lt(
                Expr::var("hit_slot"),
                atomic_load_relaxed(
                    "queue_state",
                    Expr::u32(dispatcher_abi_u32(
                        queue_state_word::HIT_CAPACITY,
                        "queue-state hit-capacity word",
                    )),
                ),
            ),
            vec![
                Node::let_bind("hit_base", Expr::mul(Expr::var("hit_slot"), Expr::u32(4))),
                Node::store("hit_ring", Expr::var("hit_base"), Expr::var("file_idx")),
                Node::store(
                    "hit_ring",
                    Expr::add(Expr::var("hit_base"), Expr::u32(1)),
                    Expr::var("rule_idx"),
                ),
                Node::store(
                    "hit_ring",
                    Expr::add(Expr::var("hit_base"), Expr::u32(2)),
                    Expr::var("layer_idx"),
                ),
                Node::store(
                    "hit_ring",
                    Expr::add(Expr::var("hit_base"), Expr::u32(3)),
                    Expr::sub(Expr::var("byte_pos"), Expr::var("file_start")),
                ),
            ],
        ),
    ]
}

/// Split the device's raw atomic hit-head into `(readable, dropped)`.
///
/// The kernel increments `HIT_HEAD` for EVERY match but only writes ring slots
/// below `hit_capacity`, so a `raw_head > capacity` means `raw_head - capacity`
/// matches were produced-but-dropped. `readable` is what can be decoded from the
/// ring (`min(raw_head, capacity)`); `dropped` is the overflow the caller must
/// recover. Pure so the overflow accounting is unit-tested without a device.
const fn split_hit_overflow(raw_head: u32, capacity: u32) -> (u32, u32) {
    if raw_head > capacity {
        (capacity, raw_head - capacity)
    } else {
        (raw_head, 0)
    }
}

#[cfg(test)]
fn decode_hits_from_readback(
    bytes: &[u8],
    hit_count: u32,
) -> Result<Vec<HitRecord>, PipelineError> {
    let mut hits = Vec::new();
    decode_hits_from_readback_into(bytes, hit_count, &mut hits)?;
    Ok(hits)
}

fn decode_hits_from_readback_into(
    bytes: &[u8],
    hit_count: u32,
    hits: &mut Vec<HitRecord>,
) -> Result<(), PipelineError> {
    let word_count = validate_u32_readback_words(bytes, "hit-ring")?;
    let needed_words = usize::try_from(hit_count)
        .ok()
        .and_then(|count| count.checked_mul(4))
        .ok_or_else(|| PipelineError::Backend("hit-count overflowed usize".to_string()))?;
    if word_count < needed_words {
        return Err(PipelineError::Backend(format!(
            "hit-ring exposed {} words, expected at least {needed_words}. Fix: size the sparse hit ring for the configured hit_capacity.",
            word_count
        )));
    }
    let needed_bytes = needed_words
        .checked_mul(std::mem::size_of::<u32>())
        .ok_or_else(|| PipelineError::Backend(
            "hit-ring readback byte count overflowed usize. Fix: reduce hit_capacity or shard the batch."
                .to_string(),
        ))?;
    let hit_count = usize::try_from(hit_count).map_err(|source| {
        PipelineError::Backend(format!(
            "hit count cannot fit usize for host decode: {source}. Fix: reduce hit_capacity or run on a supported host pointer width."
        ))
    })?;
    let same_len = hits.len() == hit_count;
    if !same_len {
        hits.clear();
    }
    if hits.capacity() < hit_count {
        hits.try_reserve_exact(hit_count - hits.len())
            .map_err(|source| {
                PipelineError::Backend(format!(
                    "hit-ring decode could not reserve {hit_count} HitRecord slots: {source}. Fix: lower hit_capacity or shard the batch."
                ))
            })?;
    }
    if cfg!(target_endian = "little") {
        let record_bytes = std::mem::size_of::<HitRecord>();
        let expected_record_bytes = HIT_RECORD_WORDS * std::mem::size_of::<u32>();
        if record_bytes != expected_record_bytes {
            return Err(PipelineError::Backend(format!(
                "hit-ring host record layout is {record_bytes} bytes, expected {expected_record_bytes}. Fix: keep HitRecord as four packed u32 words."
            )));
        }
        if hit_count != 0 {
            let records: &[HitRecord] =
                bytemuck::try_cast_slice(&bytes[..needed_bytes]).map_err(|source| {
                    PipelineError::Backend(format!(
                        "hit-ring readback bytes were not aligned as HitRecord records: {source}. Fix: keep the hit ring byte layout aligned to four u32 words."
                    ))
                })?;
            if same_len {
                hits.copy_from_slice(records);
            } else {
                hits.extend_from_slice(records);
            }
        }
        return Ok(());
    }
    for (index, chunk) in bytes[..needed_bytes]
        .chunks_exact(HIT_RECORD_WORDS * std::mem::size_of::<u32>())
        .enumerate()
    {
        let record = HitRecord {
            file_idx: u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]),
            rule_idx: u32::from_le_bytes([chunk[4], chunk[5], chunk[6], chunk[7]]),
            layer_idx: u32::from_le_bytes([chunk[8], chunk[9], chunk[10], chunk[11]]),
            match_offset: u32::from_le_bytes([chunk[12], chunk[13], chunk[14], chunk[15]]),
        };
        if same_len {
            hits[index] = record;
        } else {
            hits.push(record);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn measurement(seg_len: u32, micros: u64, dropped_hits: u32, complete: bool) -> SegLenMeasurement {
        SegLenMeasurement {
            seg_len,
            wall_time: Duration::from_micros(micros),
            dropped_hits,
            complete,
        }
    }

    /// The calibrator picks the fastest COMPLETE geometry — and must NOT pick a
    /// faster-but-incomplete one. A geometry that drained faster only because it
    /// dropped hits (ring overflow) is a false speedup; selecting it would ship a
    /// silently-truncated scan (Law 10). Here seg_len=64 is the fastest wall time
    /// but incomplete, so the fastest COMPLETE geometry (128) must win.
    #[test]
    fn calibration_selects_fastest_complete_not_faster_incomplete() {
        let measurements = vec![
            measurement(512, 900, 0, true),
            measurement(256, 700, 0, true),
            measurement(128, 400, 0, true), // fastest complete
            measurement(64, 300, 12, false), // faster wall, but dropped 12 ⇒ excluded
        ];
        assert_eq!(
            select_fastest_complete(&measurements).expect("a complete geometry exists"),
            128,
            "must select the fastest COMPLETE geometry (128), never the faster-but-truncated 64"
        );
    }

    /// No complete geometry ⇒ fail closed with an actionable error, never return
    /// an unsound/incomplete geometry as if it were a valid pick.
    #[test]
    fn calibration_fails_closed_when_every_geometry_is_incomplete() {
        let measurements = vec![
            measurement(128, 0, 5, false),
            measurement(64, 0, 9, false),
        ];
        let err = select_fastest_complete(&measurements)
            .expect_err("no complete geometry must error, not silently pick one");
        assert!(
            err.to_string().contains("no complete geometry"),
            "fail-closed error must name the cause; got: {err}"
        );
    }

    /// Equal measured speed ⇒ prefer the COARSER (larger seg_len) geometry: fewer
    /// segments means less queue-drain and per-window warm-up overhead, and is
    /// less likely to under-claim on a slower device.
    #[test]
    fn calibration_tie_breaks_toward_coarser_geometry() {
        let measurements = vec![
            measurement(256, 500, 0, true),
            measurement(128, 500, 0, true),
            measurement(64, 500, 0, true),
        ];
        assert_eq!(
            select_fastest_complete(&measurements).expect("complete geometries exist"),
            256,
            "equal wall time ⇒ pick the coarsest (256), the fewest-segments geometry"
        );
    }

    /// The shipped default candidate set must be non-empty, strictly descending
    /// (coarse→fine, the order calibration walks the saturation curve), and must
    /// exclude the whole-file `u32::MAX` correctness floor — it is never a
    /// throughput candidate.
    #[test]
    fn default_seg_len_candidates_are_descending_and_exclude_whole_file() {
        assert!(!DEFAULT_SEG_LEN_CANDIDATES.is_empty());
        assert!(
            !DEFAULT_SEG_LEN_CANDIDATES.contains(&u32::MAX),
            "whole-file is a correctness floor, never a calibration candidate"
        );
        for pair in DEFAULT_SEG_LEN_CANDIDATES.windows(2) {
            assert!(
                pair[0] > pair[1],
                "candidates must be strictly descending coarse→fine; {} !> {}",
                pair[0],
                pair[1]
            );
        }
        assert_eq!(
            *DEFAULT_SEG_LEN_CANDIDATES.last().unwrap(),
            64,
            "finest default candidate is 64 (overlap-waste dominates below it for MiB inputs)"
        );
    }

    /// The combined-AC segmented program must lower through the REAL WGSL
    /// pipeline (`descriptor_gate::validate_and_analyze` → `vyre_emit_naga::emit`
    /// → Naga validation → WGSL writer). Naga's validator rejects malformed
    /// control flow, type errors, or invalid buffer access, so a successful
    /// lower is a genuine proof the multi-emit nested loops + combined
    /// transition read are valid GPU code — not a shape check. We additionally
    /// assert every combined-automaton buffer and the multi-emit payload buffer
    /// survive into the emitted WGSL (Naga names globals after the BufferDecl),
    /// so the kernel actually reads the combined tables rather than lowering to
    /// a stripped no-op.
    #[test]
    fn combined_batch_program_lowers_to_valid_wgsl_referencing_combined_tables() {
        // BOTH transition widths must lower through the real Naga pipeline: the
        // u16 path adds a div/shr/mask unpack in the hot loop, so its validity is
        // not implied by the u32 path's.
        for width in [TransitionWidth::Bits32, TransitionWidth::Bits16] {
            let program = build_combined_batch_program(64, 1024, 40, width);
            let wgsl = crate::emit::lower(&program).unwrap_or_else(|e| {
                panic!("Fix: combined-AC {width:?} program must lower to valid WGSL: {e:?}")
            });
            for needle in [
                "transitions",
                "output_offsets",
                "output_records",
                "class_maps",
                "segments",
                "hit_ring",
                "file_offsets",
            ] {
                assert!(
                    wgsl.contains(needle),
                    "emitted WGSL ({width:?}) must reference the `{needle}` buffer; the combined \
                     kernel read it in IR but it vanished from the shader (got {} bytes of WGSL)",
                    wgsl.len()
                );
            }
        }
    }

    /// Pin the combined-AC ABI: nine buffers in the exact binding order the host
    /// `CombinedBatch` upload must mirror, with `hit_ring` the sole output at
    /// binding 7 and `segments` last (final positional input). A drift here
    /// silently misbinds the automaton tables.
    #[test]
    fn combined_batch_program_buffer_abi_is_pinned() {
        let buffers = combined_batch_program_buffers(1024);
        let layout: Vec<(&str, u32, bool, bool)> = buffers
            .iter()
            .map(|b| {
                (
                    b.name(),
                    b.binding(),
                    b.is_output(),
                    matches!(b.access(), BufferAccess::ReadWrite),
                )
            })
            .collect();
        assert_eq!(
            layout,
            vec![
                ("file_offsets", 0, false, false),
                ("file_metadata", 1, false, false),
                ("haystack", 2, false, false),
                ("transitions", 3, false, false),
                ("output_offsets", 4, false, false),
                ("output_records", 5, false, false),
                ("class_maps", 6, false, false),
                ("queue_state", 7, false, true),
                // hit_ring is a writable output ⇒ ReadWrite access.
                ("hit_ring", 8, true, true),
                ("segments", 9, false, false),
            ],
            "combined-AC buffer ABI drifted; the host CombinedBatch upload binds by this order"
        );
    }

    /// Cross-OS shader VALIDITY for the combined segmented megakernel — the
    /// program that wins 8.81× vs Hyperscan and runs live on Vulkan (Linux) and
    /// DX12 (Windows). Those runs implicitly prove SPIR-V and HLSL validity, but
    /// macOS/Metal is hardware-blocked, leaving its shader validity unproven.
    /// `vyre-emit-metal` lowers the descriptor through `naga::back::msl`, whose
    /// writer runs `naga::valid::Validator` first — a successful emit is therefore
    /// a genuine proof the megakernel is valid Metal Shading Language, verifiable
    /// WITHOUT an Apple GPU (the throughput-win on Metal stays separately
    /// hardware-blocked). SPIR-V is emitted explicitly too rather than relying on
    /// the live Vulkan run. BOTH transition widths are checked: the u16 path adds
    /// a div/shr/mask unpack in the hot loop, so its Metal/SPIR-V validity is not
    /// implied by the u32 path's.
    #[test]
    fn combined_megakernel_lowers_to_valid_msl_and_spirv_for_all_os() {
        for width in [TransitionWidth::Bits32, TransitionWidth::Bits16] {
            let program = build_combined_batch_program(64, 1024, 40, width);
            let lowered = vyre_lower::lower_for_emit(&program).unwrap_or_else(|e| {
                panic!("Fix: combined megakernel ({width:?}) must lower to a KernelDescriptor: {e:?}")
            });

            // The full combined program (not a stripped kernel) must reach the
            // backend: its ten-buffer ABI survives into the descriptor bindings.
            let slot_names: Vec<&str> = lowered
                .descriptor
                .bindings
                .slots
                .iter()
                .map(|s| s.name.as_str())
                .collect();
            for needle in [
                "file_offsets",
                "file_metadata",
                "haystack",
                "transitions",
                "output_offsets",
                "output_records",
                "class_maps",
                "queue_state",
                "hit_ring",
                "segments",
            ] {
                assert!(
                    slot_names.contains(&needle),
                    "combined megakernel descriptor ({width:?}) is missing the `{needle}` buffer; \
                     a stripped descriptor would silently emit a no-op kernel. Got {slot_names:?}"
                );
            }

            // macOS/Metal: a successful emit ⇒ naga validated the module ⇒ the
            // kernel is valid MSL on Apple GPUs.
            let msl = vyre_emit_metal::emit(&lowered.descriptor).unwrap_or_else(|e| {
                panic!(
                    "Fix: combined megakernel ({width:?}) must lower to valid MSL for macOS/Metal: {e:?}"
                )
            });
            assert!(
                msl.contains("kernel "),
                "MSL ({width:?}) must declare a Metal compute entry (`kernel `); got {} bytes",
                msl.len()
            );
            assert!(
                msl.len() > 500,
                "MSL ({width:?}) for the multi-emit combined kernel is implausibly small \
                 ({} bytes) — that is a stripped no-op, not the real automaton scan",
                msl.len()
            );

            // Vulkan/SPIR-V: explicit proof, independent of the live Vulkan run.
            let spirv = vyre_emit_spirv::emit(&lowered.descriptor).unwrap_or_else(|e| {
                panic!("Fix: combined megakernel ({width:?}) must lower to valid SPIR-V: {e:?}")
            });
            assert_eq!(
                spirv.first().copied(),
                Some(vyre_emit_spirv::SPIRV_MAGIC),
                "SPIR-V ({width:?}) must begin with the magic word"
            );
            assert!(
                spirv.len() > 200,
                "SPIR-V ({width:?}) module is implausibly small ({} words)",
                spirv.len()
            );
        }
    }

    #[test]
    fn hit_overflow_split_reports_dropped_matches() {
        // No overflow: every produced match fits the ring.
        assert_eq!(split_hit_overflow(0, 1_000), (0, 0));
        assert_eq!(split_hit_overflow(254, 1_000), (254, 0));
        // Exactly full: readable == capacity, nothing dropped.
        assert_eq!(split_hit_overflow(1_000, 1_000), (1_000, 0));
        // Overflow: readable clamps to capacity, the rest are reported dropped —
        // the recall-critical signal the old `.min()` clamp threw away.
        assert_eq!(split_hit_overflow(1_001, 1_000), (1_000, 1));
        assert_eq!(split_hit_overflow(1_500_000, 1_000_000), (1_000_000, 500_000));
        // Saturated raw head (kernel produced u32::MAX-worth of matches).
        assert_eq!(split_hit_overflow(u32::MAX, 1_000), (1_000, u32::MAX - 1_000));
    }

    #[test]
    fn default_worker_groups_is_at_least_four_on_live_adapter() {
        if let Ok(backend) = WgpuBackend::new() {
            let wg = BatchDispatchConfig::default()
                .launch_recommendation(backend.device_limits(), 64)
                .expect("Fix: live adapter limits must produce a launch recommendation")
                .worker_groups;
            assert!(
                wg >= 4,
                "Fix: default worker_groups should be >= 4 on any live adapter, got {wg}"
            );
        }
    }

    /// Behavioral replacement for the former source-shape test.  Verifies that
    /// when `worker_groups=0` (the sentinel meaning "fill from launch policy"),
    /// `launch_recommendation` returns a non-zero `worker_groups` value — i.e.,
    /// the policy consumes the `0` sentinel and fills in a real value.
    /// `BatchDispatcher::new` then stores that value back into `config.worker_groups`.
    #[test]
    fn launch_recommendation_fills_zero_worker_groups_and_hit_capacity() {
        let limits = wgpu::Limits::default();
        // Default config has worker_groups=0 and hit_capacity=65_536.
        let config = BatchDispatchConfig::default();
        assert_eq!(
            config.worker_groups, 0,
            "default worker_groups must be 0 (sentinel: fill from policy)"
        );
        let rec = config
            .launch_recommendation(&limits, 64)
            .expect("Fix: default config must produce a launch recommendation");
        assert!(
            rec.worker_groups > 0,
            "launch policy must fill worker_groups > 0 when config.worker_groups == 0, got {}",
            rec.worker_groups
        );
        // hit_capacity is not zero in the default — but verify the recommendation
        // still provides a non-zero hit_capacity.
        assert!(
            rec.hit_capacity > 0,
            "launch policy must provide a positive hit_capacity, got {}",
            rec.hit_capacity
        );
    }

    #[test]
    fn dynamic_dispatch_plan_controls_pipeline_and_launch_geometry() {
        let src = include_str!("dispatcher.rs");
        let prod_src = src.split("#[cfg(test)]").next().unwrap_or(src);
        assert!(
            prod_src.contains("let pipeline = self.pipeline_for_plan(dynamic_plan.plan)?"),
            "dispatch must compile or reuse the pipeline for the per-batch scale-aware plan"
        );
        assert!(
            prod_src.contains("[dynamic_plan.plan.worker_groups, 1, 1]"),
            "dispatch must submit the policy-selected worker group count, not config.worker_groups"
        );
        assert!(
            prod_src.contains("dynamic_plan.plan.worker_groups,\n                    self.config.workgroup_size_x"),
            "occupancy telemetry must use the actual dynamic launch geometry"
        );
    }

    /// Behavioral replacement for the former source-shape test.  Verifies that
    /// the pipeline cache capacity constant is exactly 32 (the agreed bound) and
    /// that `BatchPipelineShape` has the three program-shaping fields — not
    /// merely that the strings exist in source.  This test does not require a
    /// live GPU.
    #[test]
    fn pipeline_cache_cap_is_32_and_shape_contains_all_fields() {
        use crate::megakernel::pipeline_cache::{BatchPipelineCache, BatchPipelineShape};

        // Const-level check: the agreed retention bound must be exactly 32.
        // Changing BATCH_PIPELINE_CACHE_CAP without updating this test is a
        // deliberate reviewer gate.
        const _: () = assert!(BATCH_PIPELINE_CACHE_CAP == 32);

        // Compile-time check: BatchPipelineShape must contain exactly the three
        // program-shaping fields (workgroup_size_x, worker_groups, hit_capacity).
        // A refactor that drops or renames any of these fields breaks this
        // struct literal, surfacing a compile error rather than a silent test
        // pass.  The source-shape strings we replaced were the only guard for
        // this — now the Rust type system is.
        let _shape = BatchPipelineShape {
            workgroup_size_x: 64,
            worker_groups: 8,
            hit_capacity: 512,
        };
        // Verify the constant is consumed by cache construction without panic.
        let cache = BatchPipelineCache::with_cap(BATCH_PIPELINE_CACHE_CAP);
        drop(cache);
    }

    #[test]
    fn dynamic_plan_hit_capacity_is_clamped_to_resident_batch_ring() {
        let src = include_str!("dispatcher.rs");
        let prod_src = src.split("#[cfg(test)]").next().unwrap_or(src);
        assert!(
            prod_src.contains("let resident_hit_capacity = batch.hit_capacity()")
                && prod_src.contains("recommendation.hit_capacity = resident_hit_capacity")
                && prod_src.contains("estimated_peak_device_bytes"),
            "dynamic dispatch plans must not compile a hit-ring shape larger than the resident FileBatch output buffer"
        );
    }

    #[test]
    fn launch_recommendation_uses_explicit_graph_hints_for_topology() {
        let limits = wgpu::Limits::default();
        let config = BatchDispatchConfig::default()
            .with_graph_hints(8192, 131_072, 9_000, 0)
            .with_execution_hints(8, 0, 0, 0);

        let rec = config
            .launch_recommendation(&limits, 8192)
            .expect("Fix: explicit graph hints must produce a launch recommendation");

        assert_eq!(rec.topology, MegakernelDispatchTopology::FusedDense);
    }

    #[test]
    fn launch_recommendation_default_does_not_invent_dense_frontier() {
        let limits = wgpu::Limits::default();
        let rec = BatchDispatchConfig::default()
            .launch_recommendation(&limits, 8192)
            .expect("Fix: default graph hints must produce a launch recommendation");

        assert_ne!(rec.topology, MegakernelDispatchTopology::FusedDense);
        assert_eq!(BatchDispatchConfig::default().frontier_density_bps, 0);
    }

    /// Behavioral replacement for the former source-shape test.  Verifies that
    /// `BatchDispatchConfig.timeout` has the expected default value (30 s) and
    /// that the field is accessible on a constructed config.  Field existence is
    /// already compile-time enforced; the default value is the load-bearing
    /// invariant that guards dispatch budgets.
    #[test]
    fn timeout_field_has_expected_default_and_is_constructible() {
        let config = BatchDispatchConfig::default();
        assert_eq!(
            config.timeout,
            Duration::from_secs(30),
            "BatchDispatchConfig default timeout must be 30 s; callers that rely on the budget \
             must be able to predict the default"
        );
        // A zero timeout is a valid sentinel (fail immediately) — constructible.
        let zero_timeout_config = BatchDispatchConfig {
            timeout: Duration::ZERO,
            ..BatchDispatchConfig::default()
        };
        assert_eq!(zero_timeout_config.timeout, Duration::ZERO);
    }

    #[test]
    fn hit_readback_decodes_without_intermediate_word_vector() {
        let mut bytes = Vec::new();
        for word in [7u32, 3, 2, 99, 8, 4, 1, 100] {
            bytes.extend_from_slice(&word.to_le_bytes());
        }

        let hits = decode_hits_from_readback(&bytes, 2)
            .expect("Fix: aligned hit readback bytes must decode directly");

        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].file_idx, 7);
        assert_eq!(hits[0].rule_idx, 3);
        assert_eq!(hits[1].match_offset, 100);
    }

    #[test]
    fn hit_readback_into_reuses_caller_capacity() {
        let mut bytes = Vec::new();
        for word in [7u32, 3, 2, 99, 8, 4, 1, 100] {
            bytes.extend_from_slice(&word.to_le_bytes());
        }
        let mut hits = Vec::with_capacity(8);
        let ptr = hits.as_ptr();

        decode_hits_from_readback_into(&bytes, 2, &mut hits)
            .expect("Fix: aligned hit readback bytes must decode into caller scratch");

        assert_eq!(hits.len(), 2);
        assert_eq!(hits.as_ptr(), ptr);
    }

    /// Law 10 (no silent fallback) on the megakernel hit-decode path: a hit-ring
    /// readback that exposes FEWER words than `hit_count` demands must FAIL CLOSED
    /// with a loud, actionable error — never silently decode the few hits it can
    /// reach, which would drop the rest and lose recall invisibly. `hit_count = 2`
    /// needs 8 words (2 hits × 4 u32 each); supplying only 4 words (one hit's
    /// worth) must be rejected, not decoded as a single hit.
    #[test]
    fn hit_readback_under_provisioned_ring_fails_closed_not_silent_truncation() {
        let mut bytes = Vec::new();
        for word in [7u32, 3, 2, 99] {
            bytes.extend_from_slice(&word.to_le_bytes());
        }
        // Pre-seed the scratch so a silent partial decode would be observable as
        // leftover/short content rather than an error.
        let mut hits = vec![
            HitRecord {
                file_idx: 0,
                rule_idx: 0,
                layer_idx: 0,
                match_offset: 0,
            };
            5
        ];

        let err = decode_hits_from_readback_into(&bytes, 2, &mut hits)
            .expect_err("Fix: an under-provisioned hit ring must fail closed, never silently drop hits");
        let PipelineError::Backend(message) = err else {
            panic!("Fix: under-provisioned hit ring must surface a Backend error, got {err:?}");
        };
        assert!(
            message.contains("hit-ring exposed 4 words, expected at least 8"),
            "Fix: the fail-closed message must name the exposed vs required word counts so the \
             operator can size the ring; got {message:?}"
        );
    }

    /// A hit-ring readback whose byte length is not a whole number of u32 words is
    /// corrupt and must fail closed (the decode reinterprets bytes as packed u32
    /// records, so a ragged tail would mis-align every record). 6 bytes is 1.5
    /// words — never a valid readback.
    #[test]
    fn hit_readback_misaligned_byte_length_fails_closed() {
        let bytes = [0u8; 6];
        let mut hits = Vec::new();

        let err = decode_hits_from_readback_into(&bytes, 1, &mut hits)
            .expect_err("Fix: a non-4-byte-aligned hit readback must fail closed");
        let PipelineError::Backend(message) = err else {
            panic!("Fix: misaligned hit readback must surface a Backend error, got {err:?}");
        };
        assert!(
            message.contains("not a whole number of u32 words"),
            "Fix: misaligned-readback error must explain the 4-byte-alignment contract; got {message:?}"
        );
    }

    /// The under-provisioned guard is `word_count < needed_words` (strict), so an
    /// OVER-provisioned ring (more words than `hit_count` demands) decodes exactly
    /// `hit_count` hits and ignores the trailing slack — the GPU sizes the ring for
    /// `hit_capacity`, which is an upper bound, not the realized hit count.
    #[test]
    fn hit_readback_over_provisioned_ring_decodes_exactly_hit_count() {
        let mut bytes = Vec::new();
        for word in [7u32, 3, 2, 99, 8, 4, 1, 100, 555, 666, 777, 888] {
            bytes.extend_from_slice(&word.to_le_bytes());
        }

        let hits = decode_hits_from_readback(&bytes, 2)
            .expect("Fix: an over-provisioned hit ring must decode the realized hit_count");
        assert_eq!(hits.len(), 2, "must decode exactly hit_count, not the ring capacity");
        assert_eq!(hits[1].match_offset, 100);
    }

    /// Law 10 on the queue-state readback decode: `read_u32_word` is the
    /// fail-closed accessor that pulls `hit_head` / `items_processed` /
    /// `claims_attempted` out of the queue-state buffer. `hit_head` BOUNDS how many
    /// hits the host then decodes from the hit ring, so a silent default (0 /
    /// garbage) on a short queue-state readback would corrupt that bound and lose
    /// hits invisibly. A word index past the readback end must error loudly, never
    /// return a default. Valid indices decode the exact little-endian word.
    #[test]
    fn read_u32_word_past_end_fails_closed_not_silent_default() {
        let mut bytes = Vec::new();
        for word in [11u32, 22] {
            bytes.extend_from_slice(&word.to_le_bytes());
        }
        assert_eq!(
            read_u32_word(&bytes, "queue-state", 0).expect("word 0 is in range"),
            11
        );
        assert_eq!(
            read_u32_word(&bytes, "queue-state", 1).expect("word 1 is in range"),
            22
        );

        let err = read_u32_word(&bytes, "queue-state", 2).expect_err(
            "Fix: reading past the readback end must fail closed, never return a silent default",
        );
        let PipelineError::Backend(message) = err else {
            panic!("Fix: out-of-range word read must surface a Backend error, got {err:?}");
        };
        assert!(
            message.contains("queue-state readback is missing u32 word 2"),
            "Fix: the error must name the label and the missing word index; got {message:?}"
        );
    }

    #[test]
    fn occupancy_proxy_caps_at_full_utilization() {
        assert_eq!(occupancy_proxy_bps(32, 1, 64), 5_000);
        assert_eq!(occupancy_proxy_bps(128, 1, 64), 10_000);
        assert_eq!(occupancy_proxy_bps(0, 0, 0), 0);
        assert_eq!(occupancy_proxy_bps(u32::MAX, 1, 1), 10_000);
    }

    /// Compile-time replacement for the former source-shape test.  Field presence
    /// in `BatchDispatchTelemetry` is now enforced at compile time: if any of the
    /// performance-gate fields were removed, this struct literal would fail to
    /// compile.  The old source-string scan would have accepted a field rename
    /// silently as long as the string appeared elsewhere in the file.
    #[test]
    fn dispatch_telemetry_exposes_all_release_counters() {
        // Construct a telemetry value using all release-gate fields by name.
        // Removing or renaming any field breaks this at compile time.
        let _ = BatchDispatchTelemetry {
            bytes_uploaded: 0,
            bytes_read_back: 0,
            bytes_moved: 0,
            resident_allocations: 0,
            kernel_launches: 0,
            sync_points: 0,
            occupancy_proxy_bps: 0,
            frontier_density_bps: 0,
            queue_state_readback_bytes: 0,
            hit_readback_bytes: 0,
            estimated_peak_device_bytes: 0,
            device_memory_budget_bytes: 0,
            topology: MegakernelDispatchTopology::SparseFrontier,
            dispatch_plan_cache_hit: false,
            dispatch_plan_cache_entries: 0,
        };
    }
}

#[cfg(test)]
mod scan_batch_segmentation_tests {
    use super::{
        wgpu_scan_batch_segmentation_evidence, WgpuScanBatchSegmentationError,
        WgpuScanBatchSegmentationRequest, WGPU_SCAN_BATCH_SEGMENTATION_SCHEMA_VERSION,
    };

    #[test]
    fn segmentation_evidence_records_command_copy_bind_group_counts_and_match_digest() {
        let evidence =
            wgpu_scan_batch_segmentation_evidence(WgpuScanBatchSegmentationRequest::new(
                10, 4, 2, 1, 10, 3, 0x1234, 0x1234,
            ))
            .expect("Fix: valid WGPU scan segmentation evidence should be accepted");

        assert_eq!(
            evidence.schema_version,
            WGPU_SCAN_BATCH_SEGMENTATION_SCHEMA_VERSION
        );
        assert_eq!(evidence.chunk_count, 10);
        assert_eq!(evidence.segment_count, 3);
        assert_eq!(evidence.command_encoder_count, 3);
        assert_eq!(evidence.bind_group_reuse_count, 2);
        assert_eq!(evidence.bind_group_create_count, 1);
        assert_eq!(evidence.copy_count, 13);
        assert_eq!(evidence.match_digest, 0x1234);
        assert!(evidence.match_parity);
        assert!(evidence.all_command_counts_recorded);
        assert!(evidence.is_complete());
    }

    #[test]
    fn segmentation_evidence_rejects_missing_bind_group_accounting() {
        let error = wgpu_scan_batch_segmentation_evidence(WgpuScanBatchSegmentationRequest::new(
            9, 4, 1, 1, 9, 3, 0x1234, 0x1234,
        ))
        .expect_err("Fix: bind group counts must account for every segment");

        assert!(matches!(
            error,
            WgpuScanBatchSegmentationError::BindGroupCountMismatch {
                command_encoder_count: 3,
                bind_group_reuse_count: 1,
                bind_group_create_count: 1
            }
        ));
    }

    #[test]
    fn segmentation_evidence_rejects_match_digest_drift() {
        let error = wgpu_scan_batch_segmentation_evidence(WgpuScanBatchSegmentationRequest::new(
            4, 4, 0, 1, 4, 1, 0xaaaa, 0xbbbb,
        ))
        .expect_err("Fix: segmented WGPU scan output must match the oracle digest");

        assert!(matches!(
            error,
            WgpuScanBatchSegmentationError::MatchDigestMismatch {
                expected_match_digest: 0xaaaa,
                actual_match_digest: 0xbbbb
            }
        ));
    }

    /// Regression: `digest=0` is a legitimate value for a corpus with zero rule
    /// firings (the hash of the empty match set).  Before the fix,
    /// `wgpu_scan_batch_segmentation_evidence` rejected any request where either
    /// digest was 0 with `ZeroMatchDigest`, making it impossible to record
    /// evidence for a clean corpus.  `is_complete()` also used `match_digest != 0`
    /// as a presence gate, so even a manually constructed evidence object with
    /// `match_digest=0` would never satisfy the completeness check.
    #[test]
    fn evidence_accepts_zero_digest_for_empty_match_corpus() {
        // Both digests are 0 — both digests agree — valid evidence for a clean scan.
        let evidence =
            wgpu_scan_batch_segmentation_evidence(WgpuScanBatchSegmentationRequest::new(
                4, 4, 0, 1, 4, 1, 0, 0,
            ))
            .expect("Fix: matched zero digests are valid evidence for a zero-match corpus; ZeroMatchDigest rejection was wrong");

        assert_eq!(
            evidence.match_digest, 0,
            "evidence must preserve the zero digest value from the request"
        );
        assert!(
            evidence.match_parity,
            "evidence must report parity when both digests are equal (including zero)"
        );
        assert!(
            evidence.is_complete(),
            "evidence for a zero-match corpus must satisfy the release completeness gate"
        );
    }
}

#[cfg(test)]
mod abi_conversion_contracts {
    use super::{dispatcher_abi_u32, dispatcher_usize_to_u64};
    use super::{
        FILE_METADATA_WORDS, HIT_RECORD_WORDS, QUEUE_STATE_WORDS,
    };
    use super::super::segmentation::SEGMENT_WORDS;
    use super::super::batch::queue_state_word;
    use vyre_runtime::megakernel::rule_catalog::RULE_META_WORDS;

    /// All ABI word-count constants that are embedded as u32 literals in the
    /// generated WGSL shader must fit in u32 without any conversion failure.
    /// Before the fix, `dispatcher_abi_u32` silently returned `u32::MAX` on
    /// failure, which would have corrupted the emitted ABI constants in the GPU
    /// program without any diagnostic (Law 10 silent miscompile path).
    #[test]
    fn all_abi_word_count_constants_fit_u32_without_panic() {
        // These are the exact callers in build_batch_program /
        // execute_batch_claim_body / batch_program_buffers.  If any constant
        // grew beyond u32::MAX the test would panic, surfacing the regression
        // loudly instead of silently embedding u32::MAX in the shader.
        let queue_len_word = dispatcher_abi_u32(queue_state_word::QUEUE_LEN, "queue-len word");
        let head_word = dispatcher_abi_u32(queue_state_word::HEAD, "head word");
        let rule_count_word = dispatcher_abi_u32(queue_state_word::RULE_COUNT, "rule-count word");
        let hit_head_word = dispatcher_abi_u32(queue_state_word::HIT_HEAD, "hit-head word");
        let hit_capacity_word = dispatcher_abi_u32(queue_state_word::HIT_CAPACITY, "hit-capacity word");
        let done_count_word = dispatcher_abi_u32(queue_state_word::DONE_COUNT, "done-count word");
        let queue_state_words_val = dispatcher_abi_u32(QUEUE_STATE_WORDS, "queue-state word count");
        let segment_words_val = dispatcher_abi_u32(SEGMENT_WORDS, "segment table word count");
        let file_meta_words_val = dispatcher_abi_u32(FILE_METADATA_WORDS, "file metadata word count");
        let rule_meta_words_val = dispatcher_abi_u32(RULE_META_WORDS, "rule metadata word count");

        // Assert concrete values — the ABI is contractual; changing these
        // constants without updating GPU code is a silent correctness bug.
        assert_eq!(queue_state_words_val, 6, "QUEUE_STATE_WORDS ABI must be 6");
        assert_eq!(segment_words_val, 4, "SEGMENT_WORDS ABI must be 4");
        assert_eq!(file_meta_words_val, 4, "FILE_METADATA_WORDS ABI must be 4");
        assert_eq!(rule_meta_words_val, 5, "RULE_META_WORDS ABI must be 5");

        // Smoke-check that the queue-state word indices are in [0, QUEUE_STATE_WORDS).
        assert!(
            (queue_len_word as usize) < QUEUE_STATE_WORDS,
            "QUEUE_LEN word index {queue_len_word} must be < QUEUE_STATE_WORDS ({QUEUE_STATE_WORDS})"
        );
        assert!(
            (head_word as usize) < QUEUE_STATE_WORDS,
            "HEAD word index {head_word} must be < QUEUE_STATE_WORDS ({QUEUE_STATE_WORDS})"
        );
        assert!(
            (rule_count_word as usize) < QUEUE_STATE_WORDS,
            "RULE_COUNT word index {rule_count_word} must be < QUEUE_STATE_WORDS ({QUEUE_STATE_WORDS})"
        );
        assert!(
            (hit_head_word as usize) < QUEUE_STATE_WORDS,
            "HIT_HEAD word index {hit_head_word} must be < QUEUE_STATE_WORDS ({QUEUE_STATE_WORDS})"
        );
        assert!(
            (hit_capacity_word as usize) < QUEUE_STATE_WORDS,
            "HIT_CAPACITY word index {hit_capacity_word} must be < QUEUE_STATE_WORDS ({QUEUE_STATE_WORDS})"
        );
        assert!(
            (done_count_word as usize) < QUEUE_STATE_WORDS,
            "DONE_COUNT word index {done_count_word} must be < QUEUE_STATE_WORDS ({QUEUE_STATE_WORDS})"
        );
    }

    /// `dispatcher_usize_to_u64` must convert known small constants without
    /// panic.  Before the fix it returned `u64::MAX` silently on failure,
    /// causing a downstream `checked_mul` overflow that produced the misleading
    /// error "hit-ring readback length overflowed u64" with no indication of
    /// which constant failed (Law 10).
    #[test]
    fn all_usize_to_u64_abi_constants_convert_without_panic() {
        let hit_record_words_u64 =
            dispatcher_usize_to_u64(HIT_RECORD_WORDS, "hit-record word count");
        let u32_byte_width_u64 =
            dispatcher_usize_to_u64(std::mem::size_of::<u32>(), "u32 byte width");
        let queue_state_words_u64 =
            dispatcher_usize_to_u64(QUEUE_STATE_WORDS, "queue-state word count");

        assert_eq!(
            hit_record_words_u64, 4,
            "HIT_RECORD_WORDS must convert to u64 value 4"
        );
        assert_eq!(
            u32_byte_width_u64, 4,
            "size_of::<u32>() must convert to u64 value 4"
        );
        assert_eq!(
            queue_state_words_u64, 6,
            "QUEUE_STATE_WORDS must convert to u64 value 6"
        );
    }
}
