//! WGPU-owned megakernel dispatch wrapper.

use crate::numeric::usize_to_u64;
use smallvec::SmallVec;
use std::cell::RefCell;
use std::sync::Arc;
use std::time::Instant;
use vyre_driver::{
    BackendError, CompiledPipeline, DispatchConfig, OutputBuffers, Resource, VyreBackend,
};
use vyre_foundation::ir::Program;

use vyre_runtime::megakernel::io::{
    try_encode_empty_io_queue_into, validate_io_queue_bytes, IO_SLOT_COUNT,
};
use vyre_runtime::megakernel::{
    build_program_sharded_once_slots_control_report_shared, build_scallop_lineage_with_scratch,
    plan_compact_fusion_into, try_prune_redundant_work_items_with_scratch_into,
    CompactFusionPlanningScratch, CrossArmRedundancy, Megakernel, MegakernelConfig,
    MegakernelDispatch, MegakernelLaunchRecommendation, MegakernelReport, MegakernelTelemetry,
    MegakernelWorkItem, RedundantWorkItemPruneScratch, IO_SLOT_WORDS,
};

#[cfg(feature = "megakernel-batch")]
#[path = "megakernel/batch.rs"]
pub mod batch;
#[cfg(feature = "megakernel-batch")]
#[path = "megakernel/dispatch_plan.rs"]
pub mod dispatch_plan;
#[cfg(feature = "megakernel-batch")]
#[path = "megakernel/dispatcher.rs"]
pub mod dispatcher;
#[cfg(feature = "megakernel-batch")]
#[path = "megakernel/pipeline_cache.rs"]
pub(crate) mod pipeline_cache;
#[cfg(feature = "megakernel-batch")]
#[path = "megakernel/segmentation.rs"]
pub mod segmentation;

#[cfg(feature = "megakernel-batch")]
pub use batch::{
    queue_state_word, BatchFile, CombinedBatch, FileBatch, FileBatchRefreshReport, FileMetadata,
    HitRecord, WorkTriple, FILE_METADATA_WORDS, HIT_RECORD_WORDS, QUEUE_STATE_WORDS,
    WORK_TRIPLE_WORDS,
};
#[cfg(feature = "megakernel-batch")]
pub use dispatch_plan::BatchDispatchPlan;
#[cfg(feature = "megakernel-batch")]
pub use dispatcher::{
    BatchDispatchConfig, BatchDispatchReport, BatchDispatchSummary, BatchDispatchTelemetry,
    BatchDispatcher, BatchHitWriter, CombinedDispatchSummary, CombinedDispatcher,
    SegLenCalibration, SegLenMeasurement, TransitionWidth, DEFAULT_SEG_LEN_CANDIDATES,
    WgpuScanBatchSegmentationError, WgpuScanBatchSegmentationEvidence,
    WgpuScanBatchSegmentationRequest, WGPU_SCAN_BATCH_SEGMENTATION_SCHEMA_VERSION,
    wgpu_scan_batch_segmentation_evidence,
};

thread_local! {
    static DISPATCH_SCRATCH: RefCell<DispatchScratch> = RefCell::new(DispatchScratch::default());
}

const MAX_INLINE_LINEAGE_ITEMS: usize = 256;

#[derive(Default)]
struct DispatchScratch {
    io_queue_bytes: Vec<u8>,
    control_bytes: Vec<u8>,
    ring_words: Vec<u32>,
    debug_log_bytes: Vec<u8>,
    fusion: CompactFusionPlanningScratch,
    lineage_state: Vec<u32>,
    lineage_next: Vec<u32>,
    lineage_changed: [u32; 1],
    deduped_items: Vec<MegakernelWorkItem>,
    dedupe: RedundantWorkItemPruneScratch,
    compiled: Option<CompiledMegakernelPipeline>,
    resident: Option<ResidentMegakernelBuffers>,
    outputs: OutputBuffers,
}

struct CompiledMegakernelPipeline {
    backend_id: &'static str,
    backend_version: &'static str,
    workgroup_size_x: u32,
    slot_count: u32,
    dispatch_config: DispatchConfig,
    program: Arc<Program>,
    pipeline: Arc<dyn CompiledPipeline>,
}

struct ResidentMegakernelBuffers {
    backend_id: &'static str,
    backend_version: &'static str,
    workgroup_size_x: u32,
    slot_count: u32,
    input_lens: [usize; 4],
    resources: Vec<Resource>,
}

enum IoQueueInput<'a> {
    Scratch,
    Borrowed(&'a [u8]),
}

/// Runtime wrapper for persistent megakernel dispatch.
pub struct WgpuMegakernelDispatcher<'a> {
    backend: &'a dyn VyreBackend,
}

impl<'a> WgpuMegakernelDispatcher<'a> {
    /// Create a new dispatcher.
    #[must_use]
    pub fn new(backend: &'a dyn VyreBackend) -> Self {
        Self { backend }
    }

    /// Decode a raw little-endian `MegakernelWorkItem` queue and launch the megakernel.
    ///
    /// # Errors
    ///
    /// Returns a backend error when `work_queue_bytes` is not exactly aligned to
    /// [`MegakernelWorkItem`] records or when backend dispatch fails.
    pub fn dispatch_megakernel_bytes(
        &self,
        work_queue_bytes: &[u8],
        config: &MegakernelConfig,
    ) -> Result<MegakernelReport, BackendError> {
        if work_queue_bytes.len() % std::mem::size_of::<MegakernelWorkItem>() != 0 {
            return Err(BackendError::new(format!(
                "megakernel work queue has {} bytes, which is not a multiple of sizeof(MegakernelWorkItem)={}. Fix: encode whole MegakernelWorkItem records before dispatch.",
                work_queue_bytes.len(),
                std::mem::size_of::<MegakernelWorkItem>()
            )));
        }
        let work_items = bytemuck::try_cast_slice::<u8, MegakernelWorkItem>(work_queue_bytes).map_err(|err| {
            BackendError::new(format!(
                "megakernel work queue bytes are not aligned as MegakernelWorkItem records: {err}. Fix: allocate or copy the queue into aligned MegakernelWorkItem storage before dispatch."
            ))
        })?;
        self.dispatch_megakernel(work_items, config)
    }

    /// Launch the megakernel.
    pub fn dispatch_megakernel(
        &self,
        work_items: &[MegakernelWorkItem],
        config: &MegakernelConfig,
    ) -> Result<MegakernelReport, BackendError> {
        config.validate()?;

        if work_items.is_empty() {
            return Ok(MegakernelReport::default());
        }

        DISPATCH_SCRATCH.with(|scratch| {
            let mut scratch = scratch.borrow_mut();
            ensure_empty_io_queue_bytes(&mut scratch.io_queue_bytes)?;
            self.dispatch_megakernel_with_io_queue_ref(
                work_items,
                config,
                IoQueueInput::Scratch,
                &mut scratch,
            )
        })
    }

    /// Launch the megakernel with a caller-supplied IO queue.
    ///
    /// The queue is validated against the megakernel ABI before any backend
    /// work starts, so malformed queue views fail before compilation or GPU
    /// submission.
    pub fn dispatch_megakernel_with_io_queue(
        &self,
        work_items: &[MegakernelWorkItem],
        config: &MegakernelConfig,
        io_queue_bytes: Vec<u8>,
    ) -> Result<MegakernelReport, BackendError> {
        DISPATCH_SCRATCH.with(|scratch| {
            let mut scratch = scratch.borrow_mut();
            self.dispatch_megakernel_with_io_queue_ref(
                work_items,
                config,
                IoQueueInput::Borrowed(io_queue_bytes.as_slice()),
                &mut scratch,
            )
        })
    }

    fn dispatch_megakernel_with_io_queue_ref(
        &self,
        work_items: &[MegakernelWorkItem],
        config: &MegakernelConfig,
        io_queue: IoQueueInput<'_>,
        scratch: &mut DispatchScratch,
    ) -> Result<MegakernelReport, BackendError> {
        config.validate()?;
        let io_queue_bytes = match io_queue {
            IoQueueInput::Scratch => scratch.io_queue_bytes.as_slice(),
            IoQueueInput::Borrowed(bytes) => bytes,
        };
        validate_io_queue_bytes(io_queue_bytes).map_err(|e| BackendError::new(e.to_string()))?;

        let initial_item_count = work_items.len();
        if initial_item_count == 0 {
            return Ok(MegakernelReport::default());
        }

        let plan_start = Instant::now();
        let redundancy = try_prune_redundant_work_items_with_scratch_into(
            work_items,
            &mut scratch.deduped_items,
            &mut scratch.dedupe,
        )
        .map_err(|error| BackendError::new(error.to_string()))?;
        let planning_items = if redundancy.is_empty() {
            work_items
        } else {
            scratch.deduped_items.as_slice()
        };

        let track_lineage = should_track_lineage(planning_items.len());
        if track_lineage {
            let _fusion_plan = plan_compact_fusion_into(planning_items, &mut scratch.fusion);
        } else {
            let empty_fusion_plan = plan_compact_fusion_into(&[], &mut scratch.fusion);
            debug_assert!(
                empty_fusion_plan.is_empty(),
                "empty megakernel fusion planning input must produce an empty plan"
            );
        }
        let dispatch_items = planning_items;
        let item_count = dispatch_items.len();
        let queue_plan_ns = nanos_u64(plan_start.elapsed().as_nanos())?;

        let queue_len = u32::try_from(item_count).map_err(|_| {
            BackendError::new(
                "megakernel work queue length exceeds u32::MAX. Fix: shard the queue before dispatch.",
            )
        })?;
        let max_workgroup_size_x = self.backend.max_workgroup_size()[0];
        if max_workgroup_size_x == 0 {
            return Err(BackendError::new(format!(
                "backend `{}` reported max_workgroup_size.x=0. Fix: use a backend that exposes real adapter limits before megakernel dispatch.",
                self.backend.id()
            )));
        }
        let launch = config.launch_recommendation(
            queue_len,
            max_workgroup_size_x,
            self.backend.max_compute_workgroups_per_dimension(),
            self.backend.max_compute_invocations_per_workgroup(),
        )?;
        let geometry = launch.geometry;

        let publish_start = Instant::now();
        let dispatch_config = geometry.dispatch_config(Some(config.max_wall_time));
        let compiled_cache_hit = compiled_pipeline_cache_matches(
            self.backend,
            geometry.workgroup_size_x,
            geometry.slot_count,
            &dispatch_config,
            &scratch.compiled,
        );
        let program = if compiled_cache_hit {
            None
        } else {
            Some(build_program_sharded_once_slots_control_report_shared(
                geometry.workgroup_size_x,
                geometry.slot_count,
                &[],
            ))
        };
        let compiled = if compiled_cache_hit {
            scratch
                .compiled
                .as_ref()
                .map(|cached| cached.pipeline.as_ref())
        } else {
            let program = program.as_ref().ok_or_else(|| {
                BackendError::new(
                    "megakernel cache miss had no Program to compile. Fix: build the sharded megakernel Program before compiling a new geometry."
                        .to_string(),
                )
            })?;
            compiled_pipeline_for_geometry(
                self.backend,
                program.clone(),
                geometry.workgroup_size_x,
                geometry.slot_count,
                &dispatch_config,
                &mut scratch.compiled,
            )?
        };
        Megakernel::encode_work_items_ring_words_into(
            geometry.slot_count,
            0,
            dispatch_items,
            &mut scratch.ring_words,
        )
        .map_err(|e| BackendError::new(e.to_string()))?;
        ensure_control_bytes(&mut scratch.control_bytes)?;
        ensure_empty_debug_log_bytes(&mut scratch.debug_log_bytes)?;
        let queue_publish_ns = nanos_u64(publish_start.elapsed().as_nanos())?;

        let start = Instant::now();
        let inputs = [
            scratch.control_bytes.as_slice(),
            bytemuck::cast_slice(scratch.ring_words.as_slice()),
            scratch.debug_log_bytes.as_slice(),
            io_queue_bytes,
        ];
        let estimated_peak_device_bytes = megakernel_dispatch_peak_device_bytes(&inputs, launch)?;
        enforce_megakernel_device_memory_budget(
            estimated_peak_device_bytes,
            launch.device_memory_budget_bytes,
        )?;
        let mut resident_allocations = 0;
        let mut resident_input_cache_hit = false;
        if let Some(compiled) = compiled {
            resident_allocations = resident_megakernel_allocation_events(
                self.backend,
                geometry.workgroup_size_x,
                geometry.slot_count,
                &inputs,
                &scratch.resident,
            );
            let input_lens = [
                inputs[0].len(),
                inputs[1].len(),
                inputs[2].len(),
                inputs[3].len(),
            ];
            resident_input_cache_hit = resident_megakernel_cache_matches(
                self.backend,
                geometry.workgroup_size_x,
                geometry.slot_count,
                input_lens,
                &scratch.resident,
            );
            if let Some(resources) = ensure_resident_megakernel_buffers(
                self.backend,
                geometry.workgroup_size_x,
                geometry.slot_count,
                &inputs,
                &mut scratch.resident,
            )? {
                compiled.dispatch_persistent_handles_into(
                    resources,
                    &dispatch_config,
                    &mut scratch.outputs,
                )?;
            } else {
                compiled.dispatch_borrowed_into(&inputs, &dispatch_config, &mut scratch.outputs)?;
            }
        } else {
            let program = program.ok_or_else(|| {
                BackendError::new(
                    "megakernel cache-miss dispatch had no compiled pipeline and no Program. Fix: build the megakernel Program on every non-native-cache path.".to_string(),
                )
            })?;
            self.backend.dispatch_borrowed_into(
                program.as_ref(),
                &inputs,
                &dispatch_config,
                &mut scratch.outputs,
            )?;
        }
        let wall_time = start.elapsed();

        let control_done_count = scratch.outputs.first().ok_or_else(|| {
            BackendError::new(
                "megakernel dispatch returned no control output buffer. Fix: backend must return the control buffer as output 0.",
            )
        })?;
        let control_done_count = u64::from(
            Megakernel::try_read_done_count(control_done_count)
                .map_err(|error| BackendError::new(error.to_string()))?,
        );
        let slot_done_count = strict_done_ring_slots_from_outputs(&scratch.outputs, item_count)?;
        let done_count = control_done_count.max(slot_done_count);

        // P-RUNTIME-1: attach scallop-provenance lineage per dispatched
        // region so observability collectors can attribute outputs back
        // to the source rules that derived them. We seed the lineage
        // bitset from work_items[i].op_handle (each op contributes its
        // own bit, capped at 32 distinct ops per dispatch  -  the u32
        // word width) and run the substrate provenance closure across
        // the same exchange_adj that the matroid scheduler used. The
        // closure propagates lineage through any fused-region edges,
        // so a fused region's lineage bitset = union of contributing
        // ops' bits.
        let lineage_start = Instant::now();
        let region_lineage = if track_lineage {
            build_scallop_lineage_with_scratch(
                self.backend,
                planning_items,
                scratch.fusion.exchange_adj(),
                planning_items.len(),
                &mut scratch.lineage_state,
                &mut scratch.lineage_next,
                &mut scratch.lineage_changed,
                config.max_wall_time,
            )?
        } else {
            Vec::new()
        };
        let lineage_ns = nanos_u64(lineage_start.elapsed().as_nanos())?;

        let redundant_items = retained_redundant_done_count(
            work_items,
            dispatch_items,
            done_count,
            item_count,
            &redundancy,
        );
        let initial_item_count_u64 = u64::try_from(initial_item_count).map_err(|source| {
            BackendError::new(format!(
                "megakernel initial item count cannot fit u64: {source}. Fix: shard work items before dispatch."
            ))
        })?;
        let logical_done_count = done_count.checked_add(redundant_items).ok_or_else(|| {
            BackendError::new(
                "megakernel logical done count overflowed u64. Fix: shard work items before dispatch.",
            )
        })?;
        let bounded_done_count = logical_done_count.min(initial_item_count_u64);
        let telemetry = megakernel_report_telemetry(
            &inputs,
            &scratch.outputs,
            resident_allocations,
            item_count,
            geometry.slot_count,
            geometry.covering_worker_groups(),
            geometry.workgroup_size_x,
            launch,
            estimated_peak_device_bytes,
            compiled_cache_hit,
            resident_input_cache_hit,
        )?;
        Ok(MegakernelReport {
            items_processed: bounded_done_count,
            items_remaining: initial_item_count_u64 - bounded_done_count,
            wall_time,
            queue_plan_ns,
            queue_publish_ns,
            backend_dispatch_ns: nanos_u64(wall_time.as_nanos())?,
            lineage_ns,
            deduped_items: usize_to_u64(
                redundancy.total_redundant_ops,
                "megakernel redundant operation count",
            )?,
            published_items: usize_to_u64(item_count, "megakernel published item count")?,
            lineage_items: if track_lineage {
                usize_to_u64(item_count, "megakernel lineage item count")?
            } else {
                0
            },
            telemetry,
            region_lineage,
        })
    }
}

fn strict_done_ring_slots_from_outputs(
    outputs: &[Vec<u8>],
    item_count: usize,
) -> Result<u64, BackendError> {
    if item_count == 0 {
        return Ok(0);
    }
    let item_count_u32 = u32::try_from(item_count).map_err(|source| {
        BackendError::new(format!(
            "megakernel item_count {item_count} cannot fit u32 for ring decode: {source}. Fix: shard megakernel dispatches before protocol ring sizing."
        ))
    })?;
    let ring_bytes = Megakernel::ring_byte_len(item_count_u32).ok_or_else(|| {
        BackendError::new(
            "megakernel item_count ring byte length overflowed. Fix: shard megakernel dispatches before protocol ring sizing.".to_string(),
        )
    })?;
    let mut saw_ring_output = false;
    let mut max_done = 0_u64;
    for bytes in outputs {
        if bytes.len() < ring_bytes {
            continue;
        }
        saw_ring_output = true;
        let done = Megakernel::try_count_done_ring_slots(bytes, item_count)
            .map_err(|source| BackendError::new(source.to_string()))?;
        max_done = max_done.max(done);
    }
    if !saw_ring_output {
        return Err(BackendError::new(format!(
            "megakernel dispatch returned no output buffer large enough for {item_count} ring slot(s). Fix: backend output 0 must include control and at least one ring-sized status readback."
        )));
    }
    Ok(max_done)
}

fn ensure_resident_megakernel_buffers<'a>(
    backend: &dyn VyreBackend,
    workgroup_size_x: u32,
    slot_count: u32,
    inputs: &[&[u8]; 4],
    cache: &'a mut Option<ResidentMegakernelBuffers>,
) -> Result<Option<&'a [Resource]>, BackendError> {
    if backend.id() != "cuda" {
        return Ok(None);
    }

    let input_lens = [
        inputs[0].len(),
        inputs[1].len(),
        inputs[2].len(),
        inputs[3].len(),
    ];
    let matches_cache =
        resident_megakernel_cache_matches(backend, workgroup_size_x, slot_count, input_lens, cache);

    if !matches_cache {
        if let Some(old) = cache.take() {
            for resource in old.resources {
                backend.free_resident(resource)?;
            }
        }
        let mut resources = Vec::with_capacity(inputs.len());
        for input in inputs {
            match backend.allocate_resident(input.len()) {
                Ok(resource) => resources.push(resource),
                Err(BackendError::UnsupportedFeature { name, backend }) => {
                    return Err(BackendError::UnsupportedFeature {
                        name: format!(
                            "CUDA resident megakernel input allocation required `{name}`"
                        ),
                        backend,
                    });
                }
                Err(error) => return Err(error),
            }
        }
        if let Err(error) = refresh_resident_megakernel_inputs(backend, &resources, &inputs) {
            return match release_resident_megakernel_resources(backend, resources) {
                Ok(()) => Err(error),
                Err(cleanup) => Err(BackendError::new(format!(
                    "megakernel resident input upload failed, and cleanup of newly allocated resident slots also failed. Upload error: {error}. Cleanup error: {cleanup}. Fix: inspect CUDA resident buffer ownership and stream state before retrying."
                ))),
            };
        }
        *cache = Some(ResidentMegakernelBuffers {
            backend_id: backend.id(),
            backend_version: backend.version(),
            workgroup_size_x,
            slot_count,
            input_lens,
            resources,
        });
    } else if let Some(resident) = cache.as_ref() {
        refresh_resident_megakernel_inputs(backend, &resident.resources, &inputs)?;
    }

    Ok(cache.as_ref().map(|resident| resident.resources.as_slice()))
}

fn resident_megakernel_allocation_events(
    backend: &dyn VyreBackend,
    workgroup_size_x: u32,
    slot_count: u32,
    inputs: &[&[u8]; 4],
    cache: &Option<ResidentMegakernelBuffers>,
) -> u32 {
    if backend.id() != "cuda" {
        return 0;
    }
    let input_lens = [
        inputs[0].len(),
        inputs[1].len(),
        inputs[2].len(),
        inputs[3].len(),
    ];
    if resident_megakernel_cache_matches(backend, workgroup_size_x, slot_count, input_lens, cache) {
        0
    } else {
        4
    }
}

fn resident_megakernel_cache_matches(
    backend: &dyn VyreBackend,
    workgroup_size_x: u32,
    slot_count: u32,
    input_lens: [usize; 4],
    cache: &Option<ResidentMegakernelBuffers>,
) -> bool {
    cache.as_ref().is_some_and(|resident| {
        resident.backend_id == backend.id()
            && resident.backend_version == backend.version()
            && resident.workgroup_size_x == workgroup_size_x
            && resident.slot_count == slot_count
            && resident.input_lens == input_lens
    })
}

fn refresh_resident_megakernel_inputs(
    backend: &dyn VyreBackend,
    resources: &[Resource],
    inputs: &[&[u8]; 4],
) -> Result<(), BackendError> {
    let uploads = resident_input_upload_plan(resources, inputs)?;
    backend.upload_resident_many(uploads.as_slice())
}

fn resident_input_upload_plan<'a>(
    resources: &'a [Resource],
    inputs: &'a [&[u8]; 4],
) -> Result<SmallVec<[(&'a Resource, &'a [u8]); 4]>, BackendError> {
    if resources.len() != inputs.len() {
        return Err(BackendError::new(format!(
            "megakernel resident input refresh expected {} resident slot(s) for {} input buffer(s). Fix: rebuild resident megakernel resources when the ABI input count changes.",
            inputs.len(),
            resources.len()
        )));
    }
    let mut uploads = SmallVec::with_capacity(inputs.len());
    for (resource, input) in resources.iter().zip(inputs.iter()) {
        uploads.push((resource, *input));
    }
    Ok(uploads)
}

fn release_resident_megakernel_resources(
    backend: &dyn VyreBackend,
    resources: Vec<Resource>,
) -> Result<(), BackendError> {
    let mut first_error = None;
    for resource in resources {
        if let Err(error) = backend.free_resident(resource) {
            if first_error.is_none() {
                first_error = Some(error);
            }
        }
    }
    if let Some(error) = first_error {
        Err(error)
    } else {
        Ok(())
    }
}

fn megakernel_report_telemetry(
    inputs: &[&[u8]; 4],
    outputs: &OutputBuffers,
    resident_allocations: u32,
    item_count: usize,
    slot_count: u32,
    worker_groups: u32,
    workgroup_size_x: u32,
    launch: MegakernelLaunchRecommendation,
    estimated_peak_device_bytes: u64,
    compiled_pipeline_cache_hit: bool,
    resident_input_cache_hit: bool,
) -> Result<MegakernelTelemetry, BackendError> {
    let mut bytes_uploaded = 0u64;
    for input in inputs {
        let input_len = u64::try_from(input.len()).map_err(|source| {
            BackendError::new(format!(
                "megakernel telemetry input length cannot fit u64: {source}. Fix: shard input buffers before dispatch."
            ))
        })?;
        bytes_uploaded = bytes_uploaded.checked_add(input_len).ok_or_else(|| {
            BackendError::new(
                "megakernel telemetry uploaded-byte total overflowed u64. Fix: shard input buffers before dispatch.",
            )
        })?;
    }
    let mut bytes_read_back = 0u64;
    for output in outputs {
        let output_len = u64::try_from(output.len()).map_err(|source| {
            BackendError::new(format!(
                "megakernel telemetry output length cannot fit u64: {source}. Fix: shard output buffers before dispatch."
            ))
        })?;
        bytes_read_back = bytes_read_back.checked_add(output_len).ok_or_else(|| {
            BackendError::new(
                "megakernel telemetry readback-byte total overflowed u64. Fix: shard output buffers before dispatch.",
            )
        })?;
    }
    let bytes_moved = bytes_uploaded.checked_add(bytes_read_back).ok_or_else(|| {
        BackendError::new(
            "megakernel telemetry moved-byte total overflowed u64. Fix: shard dispatch buffers before dispatch.",
        )
    })?;
    Ok(MegakernelTelemetry {
        bytes_uploaded,
        bytes_read_back,
        bytes_moved,
        resident_allocations,
        kernel_launches: 1,
        sync_points: 1,
        occupancy_proxy_bps: occupancy_proxy_bps(item_count, worker_groups, workgroup_size_x)?,
        frontier_density_bps: density_bps(
            usize_to_u64(item_count, "megakernel frontier item count")?,
            u64::from(slot_count.max(1)),
        ),
        readback_buffers: u32::try_from(outputs.len()).map_err(|source| {
            BackendError::new(format!(
                "megakernel readback buffer count cannot fit u32: {source}. Fix: shard output buffers before telemetry reporting."
            ))
        })?,
        compiled_pipeline_cache_hit,
        resident_input_cache_hit,
        topology: launch.topology,
        pressure: launch.pressure,
        execution_mode: launch.execution_mode,
        hit_capacity: launch.hit_capacity,
        estimated_peak_device_bytes,
        device_memory_budget_bytes: launch.device_memory_budget_bytes,
    })
}

fn megakernel_dispatch_peak_device_bytes(
    inputs: &[&[u8]; 4],
    launch: MegakernelLaunchRecommendation,
) -> Result<u64, BackendError> {
    let mut abi_input_bytes = 0u64;
    for input in inputs {
        let input_len = u64::try_from(input.len()).map_err(|source| {
            BackendError::new(format!(
                "megakernel ABI input length cannot fit u64: {source}. Fix: shard ABI input buffers before dispatch."
            ))
        })?;
        abi_input_bytes = abi_input_bytes.checked_add(input_len).ok_or_else(|| {
            BackendError::new(
                "megakernel ABI input byte total overflowed u64. Fix: shard ABI input buffers before dispatch.",
            )
        })?;
    }
    launch
        .estimated_peak_device_bytes
        .checked_add(abi_input_bytes)
        .and_then(|bytes| bytes.checked_add(abi_input_bytes))
        .ok_or_else(|| {
            BackendError::new(
                "megakernel peak device byte estimate overflowed u64. Fix: shard ABI buffers before dispatch.",
            )
        })
}

fn enforce_megakernel_device_memory_budget(
    requested: u64,
    available: u64,
) -> Result<(), BackendError> {
    if available != 0 && requested > available {
        Err(BackendError::DeviceOutOfMemory {
            requested,
            available,
        })
    } else {
        Ok(())
    }
}

fn occupancy_proxy_bps(
    item_count: usize,
    worker_groups: u32,
    workgroup_size_x: u32,
) -> Result<u16, BackendError> {
    let lanes = u64::from(worker_groups.max(1))
        .checked_mul(u64::from(workgroup_size_x.max(1)))
        .ok_or_else(|| {
            BackendError::new(
                "megakernel occupancy lane count overflowed u64. Fix: reduce worker groups or workgroup size.",
            )
        })?;
    Ok(density_bps(
        usize_to_u64(item_count, "megakernel occupancy item count")?,
        lanes,
    ))
}

fn density_bps(numerator: u64, denominator: u64) -> u16 {
    crate::numeric::ratio_basis_points_u64_wide(
        numerator,
        denominator.max(1),
        0,
        "megakernel occupancy density",
    )
    .min(10_000) as u16
}

fn ensure_empty_io_queue_bytes(bytes: &mut Vec<u8>) -> Result<(), BackendError> {
    let expected = usize::try_from(IO_SLOT_COUNT)
        .map_err(|source| {
            BackendError::new(format!(
                "IO_SLOT_COUNT cannot fit usize: {source}. Fix: keep IO_SLOT_COUNT within the host index ABI."
            ))
        })?
        .checked_mul(usize::try_from(IO_SLOT_WORDS).map_err(|source| {
            BackendError::new(format!(
                "IO_SLOT_WORDS cannot fit usize: {source}. Fix: keep IO_SLOT_WORDS within the host index ABI."
            ))
        })?)
        .and_then(|words| words.checked_mul(std::mem::size_of::<u32>()))
        .ok_or_else(|| {
            BackendError::new(
                "megakernel IO queue byte length overflowed usize. Fix: shard IO queue slots before dispatch.".to_string(),
            )
        })?;
    if bytes.len() != expected {
        try_encode_empty_io_queue_into(IO_SLOT_COUNT, bytes)
            .map_err(|error| BackendError::new(error.to_string()))?;
    }
    Ok(())
}

fn ensure_control_bytes(bytes: &mut Vec<u8>) -> Result<(), BackendError> {
    let expected = Megakernel::control_byte_len(0).ok_or_else(|| {
        BackendError::new(
            "megakernel control byte length overflowed usize. Fix: reduce observable slot count."
                .to_string(),
        )
    })?;
    if bytes.len() != expected {
        Megakernel::try_encode_control_into(false, 1, 0, bytes)
            .map_err(|error| BackendError::new(error.to_string()))?;
    }
    Ok(())
}

fn ensure_empty_debug_log_bytes(bytes: &mut Vec<u8>) -> Result<(), BackendError> {
    let record_capacity = Megakernel::debug_record_capacity();
    let expected = Megakernel::debug_log_byte_len(record_capacity).ok_or_else(|| {
        BackendError::new(
            "megakernel debug-log byte length overflowed usize. Fix: reduce debug record capacity.".to_string(),
        )
    })?;
    if bytes.len() != expected {
        Megakernel::try_encode_empty_debug_log_into(record_capacity, bytes)
            .map_err(|error| BackendError::new(error.to_string()))?;
    }
    Ok(())
}

fn compiled_pipeline_cache_matches(
    backend: &dyn VyreBackend,
    workgroup_size_x: u32,
    slot_count: u32,
    dispatch_config: &DispatchConfig,
    cache: &Option<CompiledMegakernelPipeline>,
) -> bool {
    cache.as_ref().is_some_and(|cached| {
        cached.backend_id == backend.id()
            && cached.backend_version == backend.version()
            && cached.workgroup_size_x == workgroup_size_x
            && cached.slot_count == slot_count
            && same_dispatch_shape(&cached.dispatch_config, dispatch_config)
    })
}

fn compiled_pipeline_for_geometry<'a>(
    backend: &dyn VyreBackend,
    program: Arc<Program>,
    workgroup_size_x: u32,
    slot_count: u32,
    dispatch_config: &DispatchConfig,
    cache: &'a mut Option<CompiledMegakernelPipeline>,
) -> Result<Option<&'a dyn CompiledPipeline>, BackendError> {
    if compiled_pipeline_cache_matches(
        backend,
        workgroup_size_x,
        slot_count,
        dispatch_config,
        cache,
    ) && cache
        .as_ref()
        .is_some_and(|cached| Arc::ptr_eq(&cached.program, &program))
    {
        return Ok(cache.as_ref().map(|cached| cached.pipeline.as_ref()));
    }

    match backend.compile_native_shared(program.clone(), dispatch_config)? {
        Some(pipeline) => {
            *cache = Some(CompiledMegakernelPipeline {
                backend_id: backend.id(),
                backend_version: backend.version(),
                workgroup_size_x,
                slot_count,
                dispatch_config: dispatch_config.clone(),
                program,
                pipeline,
            });
            Ok(cache.as_ref().map(|cached| cached.pipeline.as_ref()))
        }
        None => {
            *cache = None;
            Ok(None)
        }
    }
}

fn same_dispatch_shape(left: &DispatchConfig, right: &DispatchConfig) -> bool {
    left.profile == right.profile
        && left.ulp_budget == right.ulp_budget
        && left.max_output_bytes == right.max_output_bytes
        && left.workgroup_override == right.workgroup_override
        && left.grid_override == right.grid_override
        && left.fixpoint_iterations == right.fixpoint_iterations
        && left.speculation == right.speculation
        && left.persistent_thread == right.persistent_thread
        && left.cooperative == right.cooperative
        && left.timeout == right.timeout
}

fn retained_redundant_done_count(
    work_items: &[MegakernelWorkItem],
    dispatch_items: &[MegakernelWorkItem],
    done_count: u64,
    dispatch_item_count: usize,
    redundancy: &CrossArmRedundancy,
) -> u64 {
    let dispatch_item_count = u64::try_from(dispatch_item_count).unwrap_or(u64::MAX);
    if done_count < dispatch_item_count {
        return 0;
    }
    redundancy
        .redundant_pairs
        .iter()
        .filter(|(early_idx, _, _)| {
            work_items
                .get(*early_idx)
                .is_some_and(|item| dispatch_items.iter().any(|queued| queued == item))
        })
        .count()
        .try_into()
        .unwrap_or(u64::MAX)
}

fn nanos_u64(nanos: u128) -> Result<u64, BackendError> {
    u64::try_from(nanos).map_err(|source| {
        BackendError::new(format!(
            "megakernel elapsed time cannot fit u64 nanoseconds: {source}. Fix: split or timeout the dispatch before telemetry overflows."
        ))
    })
}

fn should_track_lineage(item_count: usize) -> bool {
    item_count <= MAX_INLINE_LINEAGE_ITEMS
}

impl MegakernelDispatch for WgpuMegakernelDispatcher<'_> {
    fn dispatch_megakernel(
        &self,
        work_queue: &[MegakernelWorkItem],
        config: &MegakernelConfig,
    ) -> Result<MegakernelReport, BackendError> {
        WgpuMegakernelDispatcher::dispatch_megakernel(self, work_queue, config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(op: u32, input: u32, output: u32, param: u32) -> MegakernelWorkItem {
        MegakernelWorkItem {
            op_handle: op,
            input_handle: input,
            output_handle: output,
            param,
        }
    }

    #[test]
    fn retained_redundant_done_count_is_zero_without_full_dispatch_completion() {
        let a = item(1, 0, 5, 7);
        let work_items = [a, a];
        let redundancy = CrossArmRedundancy {
            redundant_pairs: vec![(0, 1, 0)],
            total_redundant_ops: 1,
        };

        let count = retained_redundant_done_count(&work_items, &[a], 0, 1, &redundancy);

        assert_eq!(count, 0);
    }

    #[test]
    fn retained_redundant_done_count_counts_duplicates_when_producer_finished() {
        let a = item(1, 0, 5, 7);
        let b = item(2, 5, 6, 0);
        let work_items = [a, b, a, a];
        let redundancy = CrossArmRedundancy {
            redundant_pairs: vec![(0, 2, 0), (0, 3, 0)],
            total_redundant_ops: 2,
        };

        let count = retained_redundant_done_count(&work_items, &[a, b], 2, 2, &redundancy);

        assert_eq!(count, 2);
    }

    #[test]
    fn retained_redundant_done_count_ignores_redundancy_without_queued_producer() {
        let a = item(1, 0, 5, 7);
        let b = item(2, 5, 6, 0);
        let work_items = [a, a, b];
        let redundancy = CrossArmRedundancy {
            redundant_pairs: vec![(0, 1, 0)],
            total_redundant_ops: 1,
        };

        let count = retained_redundant_done_count(&work_items, &[b], 1, 1, &redundancy);

        assert_eq!(count, 0);
    }

    #[test]
    fn retained_redundant_done_count_ignores_invalid_indices() {
        let a = item(1, 0, 5, 7);
        let redundancy = CrossArmRedundancy {
            redundant_pairs: vec![(99, 1, 0)],
            total_redundant_ops: 1,
        };

        let count = retained_redundant_done_count(&[a], &[a], 1, 1, &redundancy);

        assert_eq!(count, 0);
    }

    #[test]
    fn lineage_tracking_is_capped_for_large_hot_queues() {
        assert!(should_track_lineage(MAX_INLINE_LINEAGE_ITEMS));
        assert!(!should_track_lineage(MAX_INLINE_LINEAGE_ITEMS + 1));
    }

    #[test]
    fn dispatch_shape_distinguishes_timeout() {
        let mut left = DispatchConfig::default();
        let mut right = DispatchConfig::default();
        left.timeout = Some(std::time::Duration::from_millis(1));
        right.timeout = Some(std::time::Duration::from_millis(2));

        assert!(!same_dispatch_shape(&left, &right));
    }

    #[test]
    fn megakernel_dispatch_uses_caller_owned_output_scratch() {
        let source = include_str!("megakernel.rs");

        assert!(
            !source.contains(concat!("scratch.outputs", ".clear();")),
            "Fix: megakernel dispatch must not drop reusable output slots before dispatch."
        );
        assert!(
            !source.contains(concat!("scratch.outputs", " =")),
            "Fix: megakernel dispatch must not replace caller-owned output scratch with fresh OutputBuffers."
        );
        assert!(
            source.contains("dispatch_persistent_handles_into("),
            "Fix: resident megakernel dispatch must collect into reusable scratch outputs."
        );
        assert!(
            source.contains("dispatch_borrowed_into("),
            "Fix: borrowed megakernel dispatch must collect into reusable scratch outputs."
        );
    }

    #[test]
    fn dispatch_rejects_device_memory_budget_before_backend_work() {
        let backend = FakeCudaResidentBackend::new();
        let dispatcher = WgpuMegakernelDispatcher::new(&backend);
        let config = MegakernelConfig {
            worker_count: 1,
            workload: vyre_runtime::megakernel::MegakernelWorkloadHints {
                resident_device_bytes: 128 * 1024,
                device_memory_budget_bytes: 64 * 1024,
                ..Default::default()
            },
            ..MegakernelConfig::default()
        };

        let error = dispatcher
            .dispatch_megakernel(&[item(1, 0, 1, 0)], &config)
            .expect_err("over-budget megakernel launch must fail before backend work");

        match error {
            BackendError::DeviceOutOfMemory {
                requested,
                available,
            } => {
                assert!(
                    requested > available,
                    "Fix: megakernel budget rejection must report requested bytes above the budget."
                );
                assert_eq!(available, 64 * 1024);
            }
            other => panic!(
                "expected structured DeviceOutOfMemory for over-budget launch, got {other:?}"
            ),
        }
        assert!(
            backend.uploads.lock().unwrap().is_empty(),
            "Fix: over-budget megakernel launch must fail before resident uploads."
        );
        assert!(
            backend.frees.lock().unwrap().is_empty(),
            "Fix: over-budget megakernel launch must not allocate resources that need cleanup."
        );
    }

    #[test]
    fn dispatch_rejects_abi_buffer_budget_before_backend_work() {
        let backend = FakeCudaResidentBackend::new();
        let dispatcher = WgpuMegakernelDispatcher::new(&backend);
        let launch = MegakernelConfig::default()
            .launch_recommendation(1, 1, 1, 1)
            .expect("Fix: test launch recommendation must be valid");
        let budget_above_policy_scratch = launch
            .estimated_peak_device_bytes
            .checked_add(1)
            .expect("Fix: WGPU megakernel peak-device-byte policy sentinel overflowed u64");
        let config = MegakernelConfig {
            worker_count: 1,
            workload: vyre_runtime::megakernel::MegakernelWorkloadHints {
                device_memory_budget_bytes: budget_above_policy_scratch,
                ..Default::default()
            },
            ..MegakernelConfig::default()
        };

        let error = dispatcher
            .dispatch_megakernel(&[item(1, 0, 1, 0)], &config)
            .expect_err("ABI buffers must be included in megakernel device budget");

        match error {
            BackendError::DeviceOutOfMemory {
                requested,
                available,
            } => {
                assert!(
                    requested > available,
                    "Fix: megakernel ABI budget rejection must report actual peak bytes above the budget."
                );
                assert_eq!(available, budget_above_policy_scratch);
            }
            other => panic!(
                "expected structured DeviceOutOfMemory for ABI-buffer budget overflow, got {other:?}"
            ),
        }
        assert!(
            backend.uploads.lock().unwrap().is_empty(),
            "Fix: ABI-budget rejection must happen before resident uploads."
        );
    }

    #[test]
    fn resident_input_upload_plan_covers_every_abi_slot_in_order() {
        let resources = vec![
            Resource::Resident(10),
            Resource::Resident(11),
            Resource::Resident(12),
            Resource::Resident(13),
        ];
        let inputs: [&[u8]; 4] = [
            &b"control"[..],
            &b"ring"[..],
            &b"debug"[..],
            &b"io_queue"[..],
        ];

        let plan = resident_input_upload_plan(&resources, &inputs)
            .expect("Fix: resident ABI upload plan should cover exact megakernel input slots");

        assert_eq!(plan.len(), inputs.len());
        for index in 0..inputs.len() {
            assert!(
                std::ptr::eq(plan[index].0, &resources[index]),
                "Fix: resident megakernel upload slot {index} must target the matching resource."
            );
            assert_eq!(
                plan[index].1, inputs[index],
                "Fix: resident megakernel upload slot {index} must refresh the matching input bytes."
            );
        }
    }

    #[test]
    fn resident_input_upload_plan_rejects_abi_slot_mismatch() {
        let resources = vec![Resource::Resident(10), Resource::Resident(11)];
        let inputs: [&[u8]; 4] = [&b"control"[..], &b"ring"[..], &b"debug"[..], &b"io"[..]];

        let error = resident_input_upload_plan(&resources, &inputs)
            .expect_err("resident upload plan must reject truncated resource lists");

        assert!(
            error
                .to_string()
                .contains("expected 4 resident slot(s) for 2 input buffer(s)"),
            "Fix: resident ABI slot mismatch diagnostics must include expected and actual counts."
        );
    }

    #[test]
    fn megakernel_report_telemetry_counts_transfer_and_density() {
        let inputs: [&[u8]; 4] = [&[0; 4][..], &[1; 8][..], &[2; 12][..], &[3; 16][..]];
        let outputs = vec![vec![0; 5], vec![0; 7], vec![0; 11], vec![0; 13]];

        let launch = vyre_runtime::megakernel::MegakernelLaunchPolicy::standard()
            .recommend(vyre_runtime::megakernel::MegakernelLaunchRequest::direct(
                8, 2, 8,
            ))
            .expect("Fix: test launch policy request must be valid");
        let peak_bytes = megakernel_dispatch_peak_device_bytes(&inputs, launch)
            .expect("Fix: test peak byte estimate must fit u64");
        let telemetry = megakernel_report_telemetry(
            &inputs, &outputs, 4, 8, 16, 2, 8, launch, peak_bytes, true, false,
        )
        .expect("Fix: test telemetry byte accounting must fit u64");

        assert_eq!(telemetry.bytes_uploaded, 40);
        assert_eq!(telemetry.bytes_read_back, 36);
        assert_eq!(telemetry.bytes_moved, 76);
        assert_eq!(telemetry.resident_allocations, 4);
        assert_eq!(telemetry.kernel_launches, 1);
        assert_eq!(telemetry.sync_points, 1);
        assert_eq!(telemetry.occupancy_proxy_bps, 5_000);
        assert_eq!(telemetry.frontier_density_bps, 5_000);
        assert_eq!(density_bps(u64::MAX, 1), 10_000);
        assert_eq!(telemetry.readback_buffers, 4);
        assert!(telemetry.compiled_pipeline_cache_hit);
        assert!(!telemetry.resident_input_cache_hit);
        assert_eq!(telemetry.topology, launch.topology);
        assert_eq!(telemetry.pressure, launch.pressure);
        assert_eq!(telemetry.execution_mode, launch.execution_mode);
        assert_eq!(telemetry.hit_capacity, launch.hit_capacity);
        assert_eq!(telemetry.estimated_peak_device_bytes, peak_bytes);
        assert_eq!(
            telemetry.device_memory_budget_bytes,
            launch.device_memory_budget_bytes
        );
    }

    struct FakeCudaResidentBackend {
        version: &'static str,
        next: std::sync::atomic::AtomicU64,
        uploads: std::sync::Mutex<Vec<(u64, usize)>>,
        frees: std::sync::Mutex<Vec<u64>>,
        supported_ops: std::collections::HashSet<vyre_foundation::ir::OpId>,
    }

    impl FakeCudaResidentBackend {
        fn new() -> Self {
            Self::with_version("test-v1")
        }

        fn with_version(version: &'static str) -> Self {
            Self {
                version,
                next: std::sync::atomic::AtomicU64::new(100),
                uploads: std::sync::Mutex::new(Vec::new()),
                frees: std::sync::Mutex::new(Vec::new()),
                supported_ops: std::collections::HashSet::new(),
            }
        }
    }

    impl vyre_driver::backend::private::Sealed for FakeCudaResidentBackend {}

    impl VyreBackend for FakeCudaResidentBackend {
        fn id(&self) -> &'static str {
            "cuda"
        }

        fn version(&self) -> &'static str {
            self.version
        }

        fn supported_ops(&self) -> &std::collections::HashSet<vyre_foundation::ir::OpId> {
            &self.supported_ops
        }

        fn dispatch(
            &self,
            _program: &Program,
            _inputs: &[Vec<u8>],
            _config: &DispatchConfig,
        ) -> Result<Vec<Vec<u8>>, BackendError> {
            Err(BackendError::new(
                "fake CUDA resident backend must not run host dispatch in resident-cache tests.",
            ))
        }

        fn allocate_resident(&self, _byte_len: usize) -> Result<Resource, BackendError> {
            Ok(Resource::Resident(
                self.next.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            ))
        }

        fn upload_resident_many(&self, uploads: &[(&Resource, &[u8])]) -> Result<(), BackendError> {
            let mut captured = self.uploads.lock().map_err(BackendError::poisoned_lock)?;
            for &(resource, bytes) in uploads {
                let Resource::Resident(handle) = resource else {
                    return Err(BackendError::new(
                        "fake CUDA resident backend expected resident handles.",
                    ));
                };
                captured.push((*handle, bytes.len()));
            }
            Ok(())
        }

        fn free_resident(&self, resource: Resource) -> Result<(), BackendError> {
            let Resource::Resident(handle) = resource else {
                return Err(BackendError::new(
                    "fake CUDA resident backend expected resident handles for free.",
                ));
            };
            self.frees
                .lock()
                .map_err(BackendError::poisoned_lock)?
                .push(handle);
            Ok(())
        }
    }

    struct FakeCompiledPipeline;

    impl vyre_driver::backend::private::Sealed for FakeCompiledPipeline {}

    impl CompiledPipeline for FakeCompiledPipeline {
        fn id(&self) -> &str {
            "fake-compiled-megakernel-pipeline"
        }

        fn dispatch(
            &self,
            _inputs: &[Vec<u8>],
            _config: &DispatchConfig,
        ) -> Result<Vec<Vec<u8>>, BackendError> {
            Err(BackendError::new(
                "fake compiled pipeline must not dispatch in cache identity tests.",
            ))
        }
    }

    struct FakeCudaNoResidentBackend {
        supported_ops: std::collections::HashSet<vyre_foundation::ir::OpId>,
    }

    impl FakeCudaNoResidentBackend {
        fn new() -> Self {
            Self {
                supported_ops: std::collections::HashSet::new(),
            }
        }
    }

    impl vyre_driver::backend::private::Sealed for FakeCudaNoResidentBackend {}

    impl VyreBackend for FakeCudaNoResidentBackend {
        fn id(&self) -> &'static str {
            "cuda"
        }

        fn supported_ops(&self) -> &std::collections::HashSet<vyre_foundation::ir::OpId> {
            &self.supported_ops
        }

        fn dispatch(
            &self,
            _program: &Program,
            _inputs: &[Vec<u8>],
            _config: &DispatchConfig,
        ) -> Result<Vec<Vec<u8>>, BackendError> {
            Err(BackendError::new(
                "fake no-resident CUDA backend must not fall back to host dispatch.",
            ))
        }

        fn allocate_resident(&self, _byte_len: usize) -> Result<Resource, BackendError> {
            Err(BackendError::UnsupportedFeature {
                name: "resident allocation".to_string(),
                backend: self.id().to_string(),
            })
        }
    }

    #[test]
    fn cuda_resident_megakernel_allocation_failure_is_not_borrowed_fallback() {
        let backend = FakeCudaNoResidentBackend::new();
        let inputs: [&[u8]; 4] = [&[1; 4][..], &[2; 8][..], &[3; 12][..], &[4; 16][..]];
        let mut cache = None;

        let error = ensure_resident_megakernel_buffers(&backend, 64, 64, &inputs, &mut cache)
            .expect_err(
                "CUDA megakernel resident setup must fail loudly when residency is unsupported",
            );

        assert!(
            error
                .to_string()
                .contains("CUDA resident megakernel input allocation"),
            "Fix: CUDA megakernel residency failure must not be hidden behind borrowed dispatch fallback: {error}"
        );
        assert!(
            cache.is_none(),
            "Fix: failed CUDA resident setup must not leave a partial cache entry."
        );
    }

    #[test]
    fn compiled_megakernel_cache_separates_backend_versions() {
        let first_backend = FakeCudaResidentBackend::with_version("test-v1");
        let second_backend = FakeCudaResidentBackend::with_version("test-v2");
        let dispatch_config = DispatchConfig::default();
        let cache = Some(CompiledMegakernelPipeline {
            backend_id: first_backend.id(),
            backend_version: first_backend.version(),
            workgroup_size_x: 64,
            slot_count: 64,
            dispatch_config: dispatch_config.clone(),
            program: Arc::new(Program::default()),
            pipeline: Arc::new(FakeCompiledPipeline),
        });

        assert!(
            compiled_pipeline_cache_matches(&first_backend, 64, 64, &dispatch_config, &cache),
            "Fix: identical backend version and megakernel geometry must allow compiled cache reuse."
        );
        assert!(
            !compiled_pipeline_cache_matches(&second_backend, 64, 64, &dispatch_config, &cache),
            "Fix: compiled megakernel cache identity must include backend implementation version."
        );
    }

    #[test]
    fn resident_megakernel_buffers_reuse_resources_and_refresh_all_inputs() {
        let backend = FakeCudaResidentBackend::new();
        let inputs: [&[u8]; 4] = [&[1; 4][..], &[2; 8][..], &[3; 12][..], &[4; 16][..]];
        let mut cache = None;

        let first_resources =
            ensure_resident_megakernel_buffers(&backend, 64, 64, &inputs, &mut cache)
                .expect("Fix: first resident ensure must succeed")
                .expect("Fix: fake CUDA backend supports resident resources")
                .to_vec();
        let first_uploads = backend.uploads.lock().unwrap().clone();
        let second_resources =
            ensure_resident_megakernel_buffers(&backend, 64, 64, &inputs, &mut cache)
                .expect("Fix: second resident ensure must succeed")
                .expect("Fix: fake CUDA backend supports resident resources")
                .to_vec();
        let all_uploads = backend.uploads.lock().unwrap().clone();

        assert_eq!(
            first_resources, second_resources,
            "Fix: identical megakernel resident input shapes must reuse resident resources."
        );
        assert_eq!(
            first_uploads,
            vec![(100, 4), (101, 8), (102, 12), (103, 16)],
            "Fix: first resident publication must upload every megakernel ABI input slot."
        );
        assert_eq!(
            all_uploads,
            vec![
                (100, 4),
                (101, 8),
                (102, 12),
                (103, 16),
                (100, 4),
                (101, 8),
                (102, 12),
                (103, 16)
            ],
            "Fix: cache-hit resident publication must refresh all four volatile ABI input slots."
        );
        assert!(
            backend.frees.lock().unwrap().is_empty(),
            "Fix: cache-hit resident publication must not free and reallocate resources."
        );
    }

    #[test]
    fn resident_megakernel_cache_separates_backend_versions() {
        let first_backend = FakeCudaResidentBackend::with_version("test-v1");
        let second_backend = FakeCudaResidentBackend::with_version("test-v2");
        let inputs: [&[u8]; 4] = [&[1; 4][..], &[2; 8][..], &[3; 12][..], &[4; 16][..]];
        let mut cache = None;

        ensure_resident_megakernel_buffers(&first_backend, 64, 64, &inputs, &mut cache)
            .expect("Fix: first resident ensure must succeed")
            .expect("Fix: first fake CUDA backend supports resident resources");

        assert!(
            !resident_megakernel_cache_matches(
                &second_backend,
                64,
                64,
                [4, 8, 12, 16],
                &cache
            ),
            "Fix: megakernel resident input cache identity must include backend implementation version."
        );
    }
}
