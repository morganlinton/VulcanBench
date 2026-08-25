//! Apple Metal.framework runtime implementation.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::c_void;
use std::sync::{
    atomic::{AtomicU32, AtomicU64, Ordering},
    Arc, Mutex, MutexGuard,
};
use std::time::Instant;

use metal::{Buffer, Device, MTLCommandBufferStatus, MTLResourceOptions, MTLSize, NSUInteger};
use vyre_driver::backend::{private, BackendError};
use vyre_driver::pipeline::{PipelineCacheIdentity, PipelineCacheMissReason};
use vyre_driver::resident_transfer_fusion::{
    fuse_resident_transfer_intervals, ResidentTransferInterval, ResidentTransferView,
};
use vyre_driver::{
    dispatch_element_count_for_program, enforce_actual_output_budget,
    infer_dispatch_grid_for_count, output_binding_layouts, BindingPlan, BindingRole,
    CompiledPipeline, DeviceProfile, DeviceTimingQuality, DispatchConfig, OutputBindingLayout,
    OutputBuffers, PipelineCacheSnapshot, Resource, TimedDispatchResult, VyreBackend,
};
use vyre_foundation::ir::{OpId, Program};

use crate::METAL_BACKEND_ID;

/// Native Metal implementation of [`VyreBackend`].
pub struct MetalBackend {
    device: Device,
    queue: metal::CommandQueue,
    resident_buffers: MetalResidentBufferTable,
    next_resident: AtomicU64,
    pipeline_cache: Mutex<BTreeMap<[u8; 32], MetalCompiledPipeline>>,
    metrics: MetalMetricCounters,
    /// SIMD-group width as reported by the first compiled `ComputePipelineState`.
    /// `0` means "not yet probed". Metal does not expose this at the Device level;
    /// it is pipeline-state-dependent and queried via
    /// `MTLComputePipelineState::threadExecutionWidth` after the first compile.
    cached_simd_width: AtomicU32,
}

impl std::fmt::Debug for MetalBackend {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("MetalBackend")
            .field("backend", &METAL_BACKEND_ID)
            .finish_non_exhaustive()
    }
}

impl private::Sealed for MetalBackend {}

// SAFETY: `MTLDevice` and `MTLCommandQueue` are Objective-C Metal objects whose
// public API is designed for cross-thread command creation and submission. This
// backend does not expose interior raw pointers or share command encoders across
// calls; each dispatch creates its own command buffer and encoder. Resident
// handle state is protected by a Mutex and Metal buffer handles are cloned
// Objective-C object references.
unsafe impl Send for MetalBackend {}

// SAFETY: See the `Send` rationale above. Shared access only reaches Metal's
// thread-safe device/queue handles, and per-dispatch mutable state is local
// except for the resident table guarded by `resident_buffers`.
unsafe impl Sync for MetalBackend {}

/// Schema version for Metal resident scan resource table evidence.
pub const METAL_RESIDENT_SCAN_RESOURCE_TABLE_SCHEMA_VERSION: u32 = 1;

const METAL_SCAN_RESOURCE_FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const METAL_SCAN_RESOURCE_FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Lifetime class for a resident scan table bound through Metal resources.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MetalResidentScanResourceLifetime {
    /// The entry is valid only for the current command buffer.
    CommandBuffer,
    /// The entry is valid for a compiled pipeline instance.
    Pipeline,
    /// The entry is valid for a resident scan session until explicitly invalidated.
    ResidentSession,
}

impl MetalResidentScanResourceLifetime {
    /// Stable evidence label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CommandBuffer => "command_buffer",
            Self::Pipeline => "pipeline",
            Self::ResidentSession => "resident_session",
        }
    }

    const fn tag(self) -> u64 {
        match self {
            Self::CommandBuffer => 1,
            Self::Pipeline => 2,
            Self::ResidentSession => 3,
        }
    }
}

/// One Metal argument-buffer entry for a resident scan table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MetalResidentScanResourceEntry {
    /// Resident pattern table id from the shared scan metadata.
    pub table_id: u64,
    /// Resident verifier id from the shared scan metadata.
    pub verifier_id: u64,
    /// Resident output slab id consumed by the scan batch.
    pub output_slab_id: u64,
    /// Metal argument-buffer entry index used for this resident table.
    pub argument_buffer_entry: u32,
    /// Lifetime class expected for this entry.
    pub lifetime: MetalResidentScanResourceLifetime,
    /// Digest of the shared scan table metadata.
    pub shared_scan_metadata_digest: u64,
    /// Digest of the Metal resource metadata derived from the shared metadata.
    pub metal_resource_metadata_digest: u64,
}

impl MetalResidentScanResourceEntry {
    /// Construct one resident scan resource entry.
    #[must_use]
    pub const fn new(
        table_id: u64,
        verifier_id: u64,
        output_slab_id: u64,
        argument_buffer_entry: u32,
        lifetime: MetalResidentScanResourceLifetime,
        shared_scan_metadata_digest: u64,
        metal_resource_metadata_digest: u64,
    ) -> Self {
        Self {
            table_id,
            verifier_id,
            output_slab_id,
            argument_buffer_entry,
            lifetime,
            shared_scan_metadata_digest,
            metal_resource_metadata_digest,
        }
    }
}

/// Evidence for a validated Metal resident scan resource table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MetalResidentScanResourceTableEvidence {
    /// Evidence schema version.
    pub schema_version: u32,
    /// Number of resident scan resource entries.
    pub entry_count: u32,
    /// Lifetime required for every entry in the table.
    pub expected_lifetime: MetalResidentScanResourceLifetime,
    /// Number of unique pattern table ids.
    pub table_id_count: u32,
    /// Number of unique verifier ids.
    pub verifier_id_count: u32,
    /// Number of unique output slab ids.
    pub output_slab_id_count: u32,
    /// Number of unique Metal argument-buffer entries.
    pub argument_buffer_entry_count: u32,
    /// True when every Metal resource metadata digest matches shared scan metadata.
    pub shared_metadata_parity: bool,
    /// Deterministic digest of the table entries.
    pub resource_table_digest: u64,
}

impl MetalResidentScanResourceTableEvidence {
    /// Return true when the evidence is complete enough for resident scan claims.
    #[must_use]
    pub const fn is_complete(self) -> bool {
        self.schema_version == METAL_RESIDENT_SCAN_RESOURCE_TABLE_SCHEMA_VERSION
            && self.entry_count != 0
            && self.argument_buffer_entry_count == self.entry_count
            && self.shared_metadata_parity
            && self.resource_table_digest != 0
    }
}

/// Metal resident scan resource table validation error.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum MetalResidentScanResourceError {
    /// The table contains no entries.
    EmptyTable,
    /// The table has too many entries for the evidence ABI.
    EntryCountOverflow {
        /// Entry count that could not fit u32.
        entry_count: usize,
    },
    /// A required id field is zero.
    ZeroId {
        /// Entry index that failed validation.
        entry_index: usize,
        /// Id field name.
        field: &'static str,
    },
    /// Shared scan metadata digest is absent.
    ZeroSharedMetadataDigest {
        /// Entry index that failed validation.
        entry_index: usize,
    },
    /// Metal resource metadata diverged from shared scan metadata.
    MetadataDigestMismatch {
        /// Entry index that failed validation.
        entry_index: usize,
        /// Digest of shared scan metadata.
        shared_scan_metadata_digest: u64,
        /// Digest of Metal resource metadata.
        metal_resource_metadata_digest: u64,
    },
    /// The entry lifetime does not match the expected table lifetime.
    LifetimeMismatch {
        /// Entry index that failed validation.
        entry_index: usize,
        /// Expected lifetime label.
        expected: MetalResidentScanResourceLifetime,
        /// Actual lifetime label.
        actual: MetalResidentScanResourceLifetime,
    },
    /// A Metal argument-buffer entry is reused by two scan table entries.
    DuplicateArgumentBufferEntry {
        /// Duplicated Metal argument-buffer entry.
        argument_buffer_entry: u32,
    },
}

impl std::fmt::Display for MetalResidentScanResourceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyTable => formatter.write_str(
                "Metal resident scan resource table is empty. Fix: bind at least one resident scan table before dispatch.",
            ),
            Self::EntryCountOverflow { entry_count } => write!(
                formatter,
                "Metal resident scan resource table has {entry_count} entries, which exceeds the u32 evidence ABI. Fix: shard resident scan tables before dispatch."
            ),
            Self::ZeroId { entry_index, field } => write!(
                formatter,
                "Metal resident scan resource table entry {entry_index} has zero {field}. Fix: allocate table, verifier, and output slab ids before building Metal resources."
            ),
            Self::ZeroSharedMetadataDigest { entry_index } => write!(
                formatter,
                "Metal resident scan resource table entry {entry_index} has zero shared metadata digest. Fix: derive Metal resource metadata from the shared scan table metadata."
            ),
            Self::MetadataDigestMismatch {
                entry_index,
                shared_scan_metadata_digest,
                metal_resource_metadata_digest,
            } => write!(
                formatter,
                "Metal resident scan resource table entry {entry_index} metadata digest mismatch shared={shared_scan_metadata_digest:#x} metal={metal_resource_metadata_digest:#x}. Fix: rebuild the Metal argument-buffer entry from shared scan metadata."
            ),
            Self::LifetimeMismatch {
                entry_index,
                expected,
                actual,
            } => write!(
                formatter,
                "Metal resident scan resource table entry {entry_index} has lifetime {} but expected {}. Fix: rebuild the argument-buffer table at one consistent resource lifetime.",
                actual.as_str(),
                expected.as_str()
            ),
            Self::DuplicateArgumentBufferEntry {
                argument_buffer_entry,
            } => write!(
                formatter,
                "Metal resident scan resource table reuses argument-buffer entry {argument_buffer_entry}. Fix: assign one unique Metal argument-buffer entry per resident scan table."
            ),
        }
    }
}

impl std::error::Error for MetalResidentScanResourceError {}

/// Validate resident scan table metadata before binding it through Metal resources.
///
/// # Errors
///
/// Returns [`MetalResidentScanResourceError`] when ids are missing, metadata
/// digests drift, lifetimes are inconsistent, or argument-buffer entries are
/// reused.
pub fn metal_resident_scan_resource_table(
    entries: &[MetalResidentScanResourceEntry],
    expected_lifetime: MetalResidentScanResourceLifetime,
) -> Result<MetalResidentScanResourceTableEvidence, MetalResidentScanResourceError> {
    if entries.is_empty() {
        return Err(MetalResidentScanResourceError::EmptyTable);
    }
    let entry_count =
        u32::try_from(entries.len()).map_err(|_| MetalResidentScanResourceError::EntryCountOverflow {
            entry_count: entries.len(),
        })?;

    let mut table_ids = BTreeSet::new();
    let mut verifier_ids = BTreeSet::new();
    let mut output_slab_ids = BTreeSet::new();
    let mut argument_buffer_entries = BTreeSet::new();
    let mut digest = METAL_SCAN_RESOURCE_FNV_OFFSET;

    for (entry_index, entry) in entries.iter().copied().enumerate() {
        validate_metal_resident_scan_resource_entry(entry_index, entry, expected_lifetime)?;
        if !argument_buffer_entries.insert(entry.argument_buffer_entry) {
            return Err(MetalResidentScanResourceError::DuplicateArgumentBufferEntry {
                argument_buffer_entry: entry.argument_buffer_entry,
            });
        }
        table_ids.insert(entry.table_id);
        verifier_ids.insert(entry.verifier_id);
        output_slab_ids.insert(entry.output_slab_id);
        digest = mix_metal_scan_resource_entry_digest(digest, entry);
    }

    Ok(MetalResidentScanResourceTableEvidence {
        schema_version: METAL_RESIDENT_SCAN_RESOURCE_TABLE_SCHEMA_VERSION,
        entry_count,
        expected_lifetime,
        table_id_count: table_ids.len() as u32,
        verifier_id_count: verifier_ids.len() as u32,
        output_slab_id_count: output_slab_ids.len() as u32,
        argument_buffer_entry_count: argument_buffer_entries.len() as u32,
        shared_metadata_parity: true,
        resource_table_digest: digest,
    })
}

fn validate_metal_resident_scan_resource_entry(
    entry_index: usize,
    entry: MetalResidentScanResourceEntry,
    expected_lifetime: MetalResidentScanResourceLifetime,
) -> Result<(), MetalResidentScanResourceError> {
    if entry.table_id == 0 {
        return Err(MetalResidentScanResourceError::ZeroId {
            entry_index,
            field: "table_id",
        });
    }
    if entry.verifier_id == 0 {
        return Err(MetalResidentScanResourceError::ZeroId {
            entry_index,
            field: "verifier_id",
        });
    }
    if entry.output_slab_id == 0 {
        return Err(MetalResidentScanResourceError::ZeroId {
            entry_index,
            field: "output_slab_id",
        });
    }
    if entry.shared_scan_metadata_digest == 0 {
        return Err(MetalResidentScanResourceError::ZeroSharedMetadataDigest { entry_index });
    }
    if entry.shared_scan_metadata_digest != entry.metal_resource_metadata_digest {
        return Err(MetalResidentScanResourceError::MetadataDigestMismatch {
            entry_index,
            shared_scan_metadata_digest: entry.shared_scan_metadata_digest,
            metal_resource_metadata_digest: entry.metal_resource_metadata_digest,
        });
    }
    if entry.lifetime != expected_lifetime {
        return Err(MetalResidentScanResourceError::LifetimeMismatch {
            entry_index,
            expected: expected_lifetime,
            actual: entry.lifetime,
        });
    }
    Ok(())
}

fn mix_metal_scan_resource_entry_digest(
    mut digest: u64,
    entry: MetalResidentScanResourceEntry,
) -> u64 {
    digest = mix_metal_scan_resource_digest(digest, entry.table_id);
    digest = mix_metal_scan_resource_digest(digest, entry.verifier_id);
    digest = mix_metal_scan_resource_digest(digest, entry.output_slab_id);
    digest = mix_metal_scan_resource_digest(digest, u64::from(entry.argument_buffer_entry));
    digest = mix_metal_scan_resource_digest(digest, entry.lifetime.tag());
    digest = mix_metal_scan_resource_digest(digest, entry.shared_scan_metadata_digest);
    mix_metal_scan_resource_digest(digest, entry.metal_resource_metadata_digest)
}

fn mix_metal_scan_resource_digest(mut digest: u64, value: u64) -> u64 {
    for byte in value.to_le_bytes() {
        digest ^= u64::from(byte);
        digest = digest.wrapping_mul(METAL_SCAN_RESOURCE_FNV_PRIME);
    }
    digest
}

impl MetalBackend {
    /// Acquire the system default Metal device and command queue.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when no Metal device is available.
    pub fn acquire() -> Result<Self, BackendError> {
        let device = Device::system_default().ok_or_else(|| BackendError::UnsupportedFeature {
            name: "system default Metal device".to_string(),
            backend: METAL_BACKEND_ID.to_string(),
        })?;
        let queue = device.new_command_queue();
        Ok(Self {
            device,
            queue,
            resident_buffers: Arc::new(Mutex::new(BTreeMap::new())),
            next_resident: AtomicU64::new(1),
            pipeline_cache: Mutex::new(BTreeMap::new()),
            metrics: Arc::new(MetalMetrics::default()),
            cached_simd_width: AtomicU32::new(0),
        })
    }

    fn compile_pipeline(
        &self,
        program: &Program,
        config: &DispatchConfig,
    ) -> Result<
        (
            PipelineCacheIdentity,
            vyre_emit_metal::MetalArtifact,
            metal::ComputePipelineState,
        ),
        BackendError,
    > {
        let cache_identity = metal_pipeline_cache_key(program, config, &self.device)?;
        let miss_reason = {
            let cache = self.lock_pipeline_cache("pipeline cache lookup")?;
            if let Some(hit) = cache.get(&cache_identity.digest).cloned() {
                self.metrics
                    .pipeline_cache_hits
                    .fetch_add(1, Ordering::Relaxed);
                return Ok((hit.identity, hit.artifact, hit.pipeline));
            }
            PipelineCacheMissReason::classify_identities(
                cache.values().map(|entry| &entry.identity),
                &cache_identity,
            )
        };
        self.metrics
            .pipeline_cache_misses
            .fetch_add(1, Ordering::Relaxed);
        self.record_pipeline_cache_miss_reason(miss_reason);
        let lowered = vyre_lower::pre_emit::lower_for_emit(program).map_err(|error| {
            BackendError::KernelCompileFailed {
                backend: METAL_BACKEND_ID.to_string(),
                compiler_message: format!(
                    "pre-emission lowering failed before Metal compilation: {error}"
                ),
            }
        })?;
        let artifact = vyre_emit_metal::emit_artifact(&lowered.descriptor).map_err(|error| {
            BackendError::KernelCompileFailed {
                backend: METAL_BACKEND_ID.to_string(),
                compiler_message: format!("MSL artifact emission failed: {error}"),
            }
        })?;
        let options = metal::CompileOptions::new();
        let library = self
            .device
            .new_library_with_source(&artifact.msl, &options)
            .map_err(|error| BackendError::KernelCompileFailed {
                backend: METAL_BACKEND_ID.to_string(),
                compiler_message: format!("Metal library compilation failed: {error}"),
            })?;
        let function = library
            .get_function(&artifact.entry_point, None)
            .map_err(|error| BackendError::KernelCompileFailed {
                backend: METAL_BACKEND_ID.to_string(),
                compiler_message: format!(
                    "Metal entry point `{}` lookup failed: {error}",
                    artifact.entry_point
                ),
            })?;
        let pipeline = self
            .device
            .new_compute_pipeline_state_with_function(&function)
            .map_err(|error| BackendError::KernelCompileFailed {
                backend: METAL_BACKEND_ID.to_string(),
                compiler_message: format!(
                    "Metal compute pipeline creation failed for `{}`: {error}",
                    artifact.entry_point
                ),
            })?;
        // Cache the SIMD-group width from the first successfully compiled pipeline.
        // Metal exposes this per-pipeline-state (not per-device), so we store it
        // on first compile. If concurrent compiles race here we may call
        // compare_exchange twice; both are racing to write the same value for a
        // given device family, so the last writer wins and no correctness is lost.
        let thread_width = ns_uint_to_u32_saturating(pipeline.thread_execution_width());
        if thread_width > 0 {
            let _ = self.cached_simd_width.compare_exchange(
                0,
                thread_width,
                Ordering::Relaxed,
                Ordering::Relaxed,
            );
        }
        let compiled = MetalCompiledPipeline {
            identity: cache_identity,
            artifact,
            pipeline,
        };
        let mut cache = self.lock_pipeline_cache("pipeline cache insert")?;
        let cached = cache
            .entry(cache_identity.digest)
            .or_insert_with(|| compiled.clone());
        Ok((
            cached.identity,
            cached.artifact.clone(),
            cached.pipeline.clone(),
        ))
    }

    fn record_pipeline_cache_miss_reason(&self, reason: PipelineCacheMissReason) {
        match reason {
            PipelineCacheMissReason::EmptyCache => {
                self.metrics
                    .pipeline_cache_miss_empty_cache
                    .fetch_add(1, Ordering::Relaxed);
            }
            PipelineCacheMissReason::ProgramChanged => {
                self.metrics
                    .pipeline_cache_miss_program_changed
                    .fetch_add(1, Ordering::Relaxed);
            }
            PipelineCacheMissReason::DispatchPolicyChanged => {
                self.metrics
                    .pipeline_cache_miss_dispatch_policy_changed
                    .fetch_add(1, Ordering::Relaxed);
            }
            PipelineCacheMissReason::DeviceOrRuntimeChanged => {
                self.metrics
                    .pipeline_cache_miss_device_or_runtime_changed
                    .fetch_add(1, Ordering::Relaxed);
            }
            PipelineCacheMissReason::KeyAbsent => {
                self.metrics
                    .pipeline_cache_miss_key_absent
                    .fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    fn dispatch_planned_buffers(
        &self,
        program: &Program,
        binding_plan: &BindingPlan,
        config: &DispatchConfig,
        artifact: &vyre_emit_metal::MetalArtifact,
        pipeline: &metal::ComputePipelineState,
        buffers: Vec<PlannedBuffer>,
    ) -> Result<MetalDispatchResult, BackendError> {
        self.record_planned_buffer_metrics(&buffers);
        let result = dispatch_planned_buffers_with_queue(
            &self.device,
            &self.queue,
            program,
            binding_plan,
            config,
            artifact,
            pipeline,
            buffers,
        )?;
        self.record_output_readback_metrics(&result.outputs);
        Ok(result)
    }

    fn record_planned_buffer_metrics(&self, buffers: &[PlannedBuffer]) {
        record_planned_buffer_metrics(&self.metrics, buffers);
    }

    fn record_output_readback_metrics(&self, outputs: &[Vec<u8>]) {
        record_output_readback_metrics(&self.metrics, outputs);
    }

    fn record_host_to_device_copy(&self, byte_len: usize) {
        record_host_to_device_copy(&self.metrics, byte_len);
    }

    fn record_device_to_host_copy(&self, byte_len: usize) {
        record_device_to_host_copy(&self.metrics, byte_len);
    }

    fn record_buffer_allocation(&self, byte_len: usize) {
        record_buffer_allocation(&self.metrics, byte_len);
    }
}

fn dispatch_planned_buffers_with_queue(
    device: &Device,
    queue: &metal::CommandQueue,
    program: &Program,
    binding_plan: &BindingPlan,
    config: &DispatchConfig,
    artifact: &vyre_emit_metal::MetalArtifact,
    pipeline: &metal::ComputePipelineState,
    buffers: Vec<PlannedBuffer>,
) -> Result<MetalDispatchResult, BackendError> {
    let timing = submit_planned_buffers_with_queue(
        device,
        queue,
        program,
        binding_plan,
        config,
        artifact,
        pipeline,
        &buffers,
    )?;
    let outputs = collect_outputs(
        &buffers,
        &output_layout_map(output_binding_layouts(program)?)?,
    )?;
    enforce_actual_output_budget(config, &outputs)?;
    Ok(MetalDispatchResult {
        outputs,
        enqueue_ns: timing.enqueue_ns,
        wait_ns: timing.wait_ns,
    })
}

fn submit_planned_buffers_with_queue(
    device: &Device,
    queue: &metal::CommandQueue,
    program: &Program,
    binding_plan: &BindingPlan,
    config: &DispatchConfig,
    artifact: &vyre_emit_metal::MetalArtifact,
    pipeline: &metal::ComputePipelineState,
    buffers: &[PlannedBuffer],
) -> Result<MetalCommandTiming, BackendError> {
    let sizes_buffer = artifact
        .sizes_buffer_index
        .map(|slot| new_buffer_sizes_buffer(device, slot, &artifact.bindings, buffers))
        .transpose()?;
    let mut threadgroup_memory_lengths = Vec::new();
    threadgroup_memory_lengths
            .try_reserve(artifact.threadgroup_memories.len())
            .map_err(|error| BackendError::InvalidProgram {
                fix: format!(
                    "Fix: Metal threadgroup memory length list could not reserve {} entries: {error}. Split the workgroup allocations.",
                    artifact.threadgroup_memories.len()
                ),
            })?;
    for memory in &artifact.threadgroup_memories {
        threadgroup_memory_lengths.push((
                memory.threadgroup_index,
                checked_ns_uint(
                    usize::try_from(memory.aligned_byte_length).map_err(|error| {
                        BackendError::InvalidProgram {
                            fix: format!(
                                "Fix: Metal threadgroup memory `{}` length {} cannot fit usize: {error}. Split the workgroup allocation.",
                                memory.name, memory.aligned_byte_length
                            ),
                        }
                    })?,
                    "Metal threadgroup memory length",
                )?,
            ));
    }
    let workgroup_size = config.workgroup_override.unwrap_or(artifact.workgroup_size);
    let threads_per_group = metal_threadgroup_size(workgroup_size)?;
    let workgroups = match config.grid_override {
        Some(grid) => grid,
        None => infer_dispatch_grid_for_count(
            dispatch_element_count_for_program(program, &binding_plan.bindings),
            workgroup_size,
        )?,
    };
    let threadgroups = metal_grid_size(workgroups)?;

    let enqueue_start = Instant::now();
    let command_buffer = queue.new_command_buffer();
    let encoder = command_buffer.new_compute_command_encoder();
    encoder.set_compute_pipeline_state(pipeline);
    for planned in buffers {
        encoder.set_buffer(planned.metal_slot.into(), Some(&planned.buffer), 0);
    }
    if let Some((slot, buffer)) = sizes_buffer.as_ref() {
        encoder.set_buffer((*slot).into(), Some(buffer), 0);
    }
    for (index, length) in threadgroup_memory_lengths {
        encoder.set_threadgroup_memory_length(index.into(), length);
    }

    encoder.dispatch_thread_groups(threadgroups, threads_per_group);
    encoder.end_encoding();
    command_buffer.commit();
    let enqueue_ns = elapsed_ns(enqueue_start, "Metal command buffer enqueue")?;
    let wait_start = Instant::now();
    command_buffer.wait_until_completed();
    let wait_ns = elapsed_ns(wait_start, "Metal command buffer wait")?;
    let status = command_buffer.status();
    if status != MTLCommandBufferStatus::Completed {
        return Err(BackendError::DispatchFailed {
            code: Some(status as i32),
            message: format!(
                "Metal command buffer finished with status {status:?} after {wait_ns} ns"
            ),
        });
    }

    Ok(MetalCommandTiming {
        enqueue_ns,
        wait_ns,
    })
}

impl MetalBackend {
    fn lock_resident_buffers(
        &self,
        operation: &'static str,
    ) -> Result<MutexGuard<'_, BTreeMap<u64, MetalResidentBuffer>>, BackendError> {
        lock_resident_buffer_table(&self.resident_buffers, operation)
    }

    fn lock_pipeline_cache(
        &self,
        operation: &'static str,
    ) -> Result<MutexGuard<'_, BTreeMap<[u8; 32], MetalCompiledPipeline>>, BackendError> {
        self.pipeline_cache
            .lock()
            .map_err(|error| BackendError::InvalidProgram {
                fix: format!(
                    "Fix: Metal pipeline cache was poisoned during {operation}: {error}. Drop and reacquire the Metal backend before dispatch."
                ),
            })
    }

    fn resident_buffer(
        &self,
        resource: &Resource,
        operation: &'static str,
    ) -> Result<(u64, MetalResidentBuffer), BackendError> {
        let Resource::Resident(id) = resource else {
            return Err(BackendError::InvalidProgram {
                fix: format!(
                    "Fix: Metal {operation} expected a resident resource handle, but received a borrowed host buffer. Allocate with allocate_resident first."
                ),
            });
        };
        let table = self.lock_resident_buffers(operation)?;
        let resident = table.get(id).cloned().ok_or_else(|| BackendError::InvalidProgram {
            fix: format!(
                "Fix: Metal {operation} received stale resident handle {id}. Keep the resource allocated until all resident operations finish and free each handle exactly once."
            ),
        })?;
        Ok((*id, resident))
    }

    fn resolve_resident_resources<'a>(
        &self,
        binding_plan: &BindingPlan,
        resources: &'a [Resource],
    ) -> Result<Vec<ResolvedMetalResource<'a>>, BackendError> {
        resolve_resident_resources_from_table(&self.resident_buffers, binding_plan, resources)
    }
}

impl VyreBackend for MetalBackend {
    fn id(&self) -> &'static str {
        METAL_BACKEND_ID
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn supported_ops(&self) -> &std::collections::HashSet<OpId> {
        vyre_driver::backend::core_supported_ops()
    }

    fn max_workgroup_size(&self) -> [u32; 3] {
        let size = self.device.max_threads_per_threadgroup();
        [
            ns_uint_to_u32_saturating(size.width),
            ns_uint_to_u32_saturating(size.height).max(1),
            ns_uint_to_u32_saturating(size.depth).max(1),
        ]
    }

    fn supports_subgroup_ops(&self) -> bool {
        true
    }

    fn subgroup_size(&self) -> Option<u32> {
        // Metal's SIMD-group width is pipeline-state-dependent, not device-level.
        // We probe it from `ComputePipelineState::threadExecutionWidth` on the
        // first successful `compile_pipeline` call and cache the result here.
        // `0` means "not yet probed" — return `None` so callers can handle
        // the unknown case rather than receiving a potentially-wrong constant.
        // In practice the first dispatch fills the cache so `subgroup_size()`
        // always returns `Some` after the first kernel is compiled on this backend.
        let cached = self.cached_simd_width.load(Ordering::Relaxed);
        if cached > 0 {
            Some(cached)
        } else {
            None
        }
    }

    fn max_compute_workgroups_per_dimension(&self) -> u32 {
        u32::MAX
    }

    fn max_compute_invocations_per_workgroup(&self) -> u32 {
        ns_uint_to_u32_saturating(self.device.max_threads_per_threadgroup().width).max(1)
    }

    fn max_storage_buffer_bytes(&self) -> u64 {
        self.device
            .max_buffer_length()
            .try_into()
            .unwrap_or(u64::MAX)
    }

    fn device_profile(&self) -> DeviceProfile {
        let max_workgroup_size = self.max_workgroup_size();
        let max_shared_memory_bytes =
            ns_uint_to_u32_saturating(self.device.max_threadgroup_memory_length());
        DeviceProfile {
            backend: self.id(),
            supports_subgroup_ops: self.supports_subgroup_ops(),
            supports_indirect_dispatch: self.supports_indirect_dispatch(),
            supports_distributed_collectives: self.supports_distributed_collectives(),
            supports_specialization_constants: false,
            supports_f16: self.supports_f16(),
            supports_bf16: self.supports_bf16(),
            supports_trap_propagation: false,
            supports_tensor_cores: self.supports_tensor_cores(),
            has_mul_high: false,
            has_dual_issue_fp32_int32: false,
            has_subgroup_shuffle: self.supports_subgroup_ops(),
            has_shared_memory: max_shared_memory_bytes > 0,
            max_native_int_width: 32,
            max_workgroup_size,
            max_invocations_per_workgroup: self.max_compute_invocations_per_workgroup(),
            max_shared_memory_bytes,
            max_storage_buffer_binding_size: self.max_storage_buffer_bytes(),
            subgroup_size: self.subgroup_size().unwrap_or(0),
            compute_units: 0,
            regs_per_thread_max: 0,
            l1_cache_bytes: 0,
            l2_cache_bytes: 0,
            mem_bw_gbps: bytes_per_second_to_gbps(self.device.max_transfer_rate()),
            timing_quality: DeviceTimingQuality::HostEnqueueWait,
            supports_device_timestamps: false,
            supports_hardware_counters: false,
            ideal_unroll_depth: 0,
            ideal_vector_pack_bits: 0,
            ideal_workgroup_tile: [0, 0, 0],
            shared_memory_bank_count: 0,
            shared_memory_bank_width_bytes: 0,
        }
    }

    fn dispatch(
        &self,
        program: &Program,
        inputs: &[Vec<u8>],
        config: &DispatchConfig,
    ) -> Result<Vec<Vec<u8>>, BackendError> {
        let borrowed = vyre_driver::backend::borrowed_input_slices(inputs, "metal inputs")?;
        self.dispatch_borrowed(program, &borrowed, config)
    }

    fn dispatch_borrowed(
        &self,
        program: &Program,
        inputs: &[&[u8]],
        config: &DispatchConfig,
    ) -> Result<Vec<Vec<u8>>, BackendError> {
        self.dispatch_borrowed_timed(program, inputs, config)
            .map(|timed| timed.outputs)
    }

    fn dispatch_borrowed_timed(
        &self,
        program: &Program,
        inputs: &[&[u8]],
        config: &DispatchConfig,
    ) -> Result<TimedDispatchResult, BackendError> {
        let started = Instant::now();
        validate_metal_dispatch_config(
            config,
            "Metal cooperative grid dispatch",
            "Metal non-resident repeated dispatch",
            "Metal dispatch",
        )?;
        let binding_plan = BindingPlan::from_borrowed_inputs(program, inputs)?;
        let output_layouts = output_binding_layouts(program)?;
        let output_by_binding = output_layout_map(output_layouts)?;
        let (_, artifact, pipeline) = self.compile_pipeline(program, config)?;
        let metal_slots = metal_slot_map(&artifact)?;
        let buffers = plan_buffers(
            &self.device,
            &binding_plan,
            inputs,
            &output_by_binding,
            &metal_slots,
            &artifact.bindings,
        )?;
        let result = self.dispatch_planned_buffers(
            program,
            &binding_plan,
            config,
            &artifact,
            &pipeline,
            buffers,
        )?;
        Ok(TimedDispatchResult {
            outputs: result.outputs,
            wall_ns: elapsed_ns(started, "Metal borrowed timed dispatch")?,
            device_ns: None,
            enqueue_ns: Some(result.enqueue_ns),
            wait_ns: Some(result.wait_ns),
        })
    }

    fn allocate_resident(&self, byte_len: usize) -> Result<Resource, BackendError> {
        let buffer = new_zero_buffer(&self.device, byte_len)?;
        let id = next_resident_id(&self.next_resident)?;
        let mut table = self.lock_resident_buffers("resident allocation")?;
        if table
            .insert(id, MetalResidentBuffer { buffer, byte_len })
            .is_some()
        {
            return Err(BackendError::InvalidProgram {
                fix: format!(
                    "Fix: Metal resident allocation generated duplicate handle {id}. Drop and reacquire the backend before allocating more resident buffers."
                ),
            });
        }
        self.record_buffer_allocation(byte_len);
        Ok(Resource::Resident(id))
    }

    fn upload_resident(&self, resource: &Resource, bytes: &[u8]) -> Result<(), BackendError> {
        self.upload_resident_many(&[(resource, bytes)])
    }

    fn upload_resident_many(&self, uploads: &[(&Resource, &[u8])]) -> Result<(), BackendError> {
        let mut resolved = Vec::new();
        resolved
            .try_reserve(uploads.len())
            .map_err(|error| BackendError::InvalidProgram {
                fix: format!(
                    "Fix: Metal resident batch upload could not reserve {} upload descriptor(s): {error}. Split the resident upload batch.",
                    uploads.len()
                ),
            })?;
        for &(resource, bytes) in uploads {
            let (id, resident) = self.resident_buffer(resource, "resident batch upload")?;
            validate_resident_range(
                id,
                resident.byte_len,
                0,
                bytes.len(),
                "resident batch upload",
            )?;
            resolved.push((resident, bytes));
        }
        for (resident, bytes) in resolved {
            copy_to_shared_buffer_range(&resident.buffer, 0, bytes, "resident batch upload")?;
            self.record_host_to_device_copy(bytes.len());
            if bytes.len() < resident.byte_len {
                zero_shared_buffer_range(
                    &resident.buffer,
                    bytes.len(),
                    resident.byte_len - bytes.len(),
                    "resident batch upload padding",
                )?;
            }
        }
        Ok(())
    }

    fn upload_resident_at(
        &self,
        resource: &Resource,
        dst_offset_bytes: usize,
        bytes: &[u8],
    ) -> Result<(), BackendError> {
        self.upload_resident_at_many(&[(resource, dst_offset_bytes, bytes)])
    }

    fn upload_resident_at_many(
        &self,
        uploads: &[(&Resource, usize, &[u8])],
    ) -> Result<(), BackendError> {
        let mut resolved = Vec::new();
        resolved
            .try_reserve(uploads.len())
            .map_err(|error| BackendError::InvalidProgram {
                fix: format!(
                    "Fix: Metal resident ranged batch upload could not reserve {} upload descriptor(s): {error}. Split the resident upload batch.",
                    uploads.len()
                ),
            })?;
        for &(resource, dst_offset_bytes, bytes) in uploads {
            let (id, resident) = self.resident_buffer(resource, "resident ranged batch upload")?;
            validate_resident_range(
                id,
                resident.byte_len,
                dst_offset_bytes,
                bytes.len(),
                "resident ranged batch upload",
            )?;
            resolved.push((resident, dst_offset_bytes, bytes));
        }
        for (resident, dst_offset_bytes, bytes) in resolved {
            copy_to_shared_buffer_range(
                &resident.buffer,
                dst_offset_bytes,
                bytes,
                "resident ranged batch upload",
            )?;
            self.record_host_to_device_copy(bytes.len());
        }
        Ok(())
    }

    fn download_resident_into(
        &self,
        resource: &Resource,
        out: &mut Vec<u8>,
    ) -> Result<(), BackendError> {
        let (id, resident) = self.resident_buffer(resource, "resident download")?;
        validate_resident_range(
            id,
            resident.byte_len,
            0,
            resident.byte_len,
            "resident download",
        )?;
        copy_shared_buffer_range_into(
            &resident.buffer,
            0,
            resident.byte_len,
            out,
            "resident download",
        )?;
        self.record_device_to_host_copy(resident.byte_len);
        Ok(())
    }

    fn download_resident_range_into(
        &self,
        resource: &Resource,
        byte_offset: usize,
        byte_len: usize,
        out: &mut Vec<u8>,
    ) -> Result<(), BackendError> {
        let (id, resident) = self.resident_buffer(resource, "resident ranged download")?;
        validate_resident_range(
            id,
            resident.byte_len,
            byte_offset,
            byte_len,
            "resident ranged download",
        )?;
        copy_shared_buffer_range_into(
            &resident.buffer,
            byte_offset,
            byte_len,
            out,
            "resident ranged download",
        )?;
        self.record_device_to_host_copy(byte_len);
        Ok(())
    }

    fn download_resident_ranges_into(
        &self,
        ranges: &[(&Resource, usize, usize)],
        outputs: &mut [&mut Vec<u8>],
    ) -> Result<(), BackendError> {
        if ranges.len() != outputs.len() {
            return Err(BackendError::InvalidProgram {
                fix: format!(
                    "Fix: Metal resident ranged batch download expected matching range/output counts but got {} range(s) and {} output buffer(s).",
                    ranges.len(),
                    outputs.len()
                ),
            });
        }

        let mut copies = Vec::new();
        copies
            .try_reserve(ranges.len())
            .map_err(|error| BackendError::InvalidProgram {
                fix: format!(
                    "Fix: Metal resident ranged batch download could not reserve {} validated transfer interval(s): {error}. Split the resident readback batch.",
                    ranges.len()
                ),
            })?;
        let mut buffers = BTreeMap::new();
        for &(resource, byte_offset, byte_len) in ranges {
            let (id, resident) =
                self.resident_buffer(resource, "resident ranged batch download")?;
            validate_resident_range(
                id,
                resident.byte_len,
                byte_offset,
                byte_len,
                "resident ranged batch download",
            )?;
            let src = u64::try_from(byte_offset).map_err(|error| BackendError::InvalidProgram {
                fix: format!(
                    "Fix: Metal resident ranged batch download offset {byte_offset} for handle {id} cannot fit u64 transfer fusion coordinates: {error}. Split the readback range."
                ),
            })?;
            buffers.entry(id).or_insert(resident.buffer);
            copies.push(ResidentTransferInterval {
                handle_id: id,
                src,
                byte_len,
            });
        }

        let fused = fuse_resident_transfer_intervals(&copies)?;
        reserve_fused_resident_view_outputs(&fused.copies, &fused.views, outputs)?;
        let mut fused_outputs = Vec::new();
        fused_outputs
            .try_reserve(fused.copies.len())
            .map_err(|error| BackendError::InvalidProgram {
                fix: format!(
                    "Fix: Metal resident ranged batch download could not reserve {} fused readback output slot(s): {error}. Split the resident readback batch.",
                    fused.copies.len()
                ),
            })?;
        for copy in fused.copies.iter().copied() {
            let buffer = buffers.get(&copy.handle_id).ok_or_else(|| BackendError::InvalidProgram {
                fix: format!(
                    "Fix: Metal resident ranged batch download fused copy references unknown handle {}. Rebuild the resident transfer fusion plan after validation.",
                    copy.handle_id
                ),
            })?;
            let src = usize::try_from(copy.src).map_err(|error| BackendError::InvalidProgram {
                fix: format!(
                    "Fix: Metal resident fused ranged batch download offset {} for handle {} cannot fit usize readback coordinates: {error}. Split the readback range.",
                    copy.src, copy.handle_id
                ),
            })?;
            let mut fused_output = Vec::new();
            copy_shared_buffer_range_into(
                buffer,
                src,
                copy.byte_len,
                &mut fused_output,
                "resident fused ranged batch download",
            )?;
            self.record_device_to_host_copy(copy.byte_len);
            fused_outputs.push(fused_output);
        }
        for (view, output) in fused.views.iter().copied().zip(outputs.iter_mut()) {
            copy_fused_resident_view_into(&fused_outputs, view, output)?;
        }
        Ok(())
    }

    fn free_resident(&self, resource: Resource) -> Result<(), BackendError> {
        let Resource::Resident(id) = resource else {
            return Err(BackendError::InvalidProgram {
                fix: "Fix: Metal resident free expected a handle returned by allocate_resident, but received a borrowed host buffer.".to_string(),
            });
        };
        let mut table = self.lock_resident_buffers("resident free")?;
        table.remove(&id).ok_or_else(|| BackendError::InvalidProgram {
            fix: format!(
                "Fix: Metal resident free received stale handle {id}. Free each resident resource exactly once."
            ),
        })?;
        Ok(())
    }

    fn shutdown(&self) -> Result<(), BackendError> {
        self.lock_resident_buffers("shutdown")?.clear();
        self.lock_pipeline_cache("shutdown")?.clear();
        Ok(())
    }

    fn pipeline_cache_snapshot(&self) -> Option<PipelineCacheSnapshot> {
        Some(PipelineCacheSnapshot {
            hits: self.metrics.pipeline_cache_hits.load(Ordering::Relaxed),
            misses: self.metrics.pipeline_cache_misses.load(Ordering::Relaxed),
        })
    }

    fn backend_metric_snapshot(&self) -> Vec<(&'static str, u64)> {
        let mut metrics = Vec::with_capacity(16);
        metrics.push((
            "metal_pipeline_cache_hits",
            self.metrics.pipeline_cache_hits.load(Ordering::Relaxed),
        ));
        metrics.push((
            "metal_pipeline_cache_misses",
            self.metrics.pipeline_cache_misses.load(Ordering::Relaxed),
        ));
        metrics.push((
            "metal_pipeline_cache_miss_empty_cache",
            self.metrics
                .pipeline_cache_miss_empty_cache
                .load(Ordering::Relaxed),
        ));
        metrics.push((
            "metal_pipeline_cache_miss_program_changed",
            self.metrics
                .pipeline_cache_miss_program_changed
                .load(Ordering::Relaxed),
        ));
        metrics.push((
            "metal_pipeline_cache_miss_dispatch_policy_changed",
            self.metrics
                .pipeline_cache_miss_dispatch_policy_changed
                .load(Ordering::Relaxed),
        ));
        metrics.push((
            "metal_pipeline_cache_miss_device_or_runtime_changed",
            self.metrics
                .pipeline_cache_miss_device_or_runtime_changed
                .load(Ordering::Relaxed),
        ));
        metrics.push((
            "metal_pipeline_cache_miss_key_absent",
            self.metrics
                .pipeline_cache_miss_key_absent
                .load(Ordering::Relaxed),
        ));
        metrics.push((
            "metal_buffer_allocation_count",
            self.metrics.buffer_allocation_count.load(Ordering::Relaxed),
        ));
        metrics.push((
            "metal_buffer_allocation_bytes",
            self.metrics.buffer_allocation_bytes.load(Ordering::Relaxed),
        ));
        metrics.push((
            "metal_host_to_device_copy_count",
            self.metrics
                .host_to_device_copy_count
                .load(Ordering::Relaxed),
        ));
        metrics.push((
            "metal_host_to_device_bytes",
            self.metrics.host_to_device_bytes.load(Ordering::Relaxed),
        ));
        metrics.push((
            "metal_device_to_host_copy_count",
            self.metrics
                .device_to_host_copy_count
                .load(Ordering::Relaxed),
        ));
        metrics.push((
            "metal_device_to_host_bytes",
            self.metrics.device_to_host_bytes.load(Ordering::Relaxed),
        ));
        metrics.push((
            "metal_output_readback_bytes",
            self.metrics.output_readback_bytes.load(Ordering::Relaxed),
        ));
        // Law 10: do NOT silently discard Mutex-poisoned state. If a background
        // thread panicked while holding `resident_buffers`, callers cannot
        // distinguish "zero resident buffers" from "poisoned backend". We push
        // u64::MAX as an unambiguous sentinel so any downstream metric gate or
        // benchmark comparison fires immediately rather than observing a silent
        // gap.
        match self.resident_buffers.lock() {
            Ok(table) => {
                metrics.push(("metal_resident_buffer_count", table.len() as u64));
                let resident_bytes = table
                    .values()
                    .try_fold(0_u64, |total, resident| {
                        u64::try_from(resident.byte_len)
                            .ok()
                            .and_then(|byte_len| total.checked_add(byte_len))
                    })
                    .unwrap_or(u64::MAX);
                metrics.push(("metal_resident_bytes", resident_bytes));
            }
            Err(_poison) => {
                // Sentinel values: u64::MAX is unambiguously wrong and will
                // trigger any downstream alert that compares against normal
                // buffer counts or byte totals.
                metrics.push(("metal_resident_buffer_count", u64::MAX));
                metrics.push(("metal_resident_bytes", u64::MAX));
                metrics.push(("metal_resident_buffer_error", 1_u64));
                tracing::error!(
                    "metal resident_buffers Mutex is poisoned; \
                     resident buffer metrics are sentinel values (u64::MAX). \
                     Fix: a background dispatch thread panicked while holding \
                     the resident buffer table lock."
                );
            }
        }
        metrics
    }

    fn dispatch_resident_timed(
        &self,
        program: &Program,
        resources: &[Resource],
        config: &DispatchConfig,
    ) -> Result<TimedDispatchResult, BackendError> {
        let started = Instant::now();
        validate_metal_dispatch_config(
            config,
            "Metal cooperative grid resident dispatch",
            "Metal repeated resident dispatch",
            "Metal resident dispatch",
        )?;

        let base_plan = BindingPlan::build(program)?;
        let resolved = self.resolve_resident_resources(&base_plan, resources)?;
        let input_lengths = resident_input_lengths(&base_plan, &resolved)?;
        let binding_plan = BindingPlan::from_input_lengths(program, &input_lengths)?;
        let output_layouts = output_binding_layouts(program)?;
        let output_by_binding = output_layout_map(output_layouts)?;
        let (_, artifact, pipeline) = self.compile_pipeline(program, config)?;
        let metal_slots = metal_slot_map(&artifact)?;
        let buffers = plan_resident_buffers(
            &self.device,
            &binding_plan,
            &resolved,
            &output_by_binding,
            &metal_slots,
            &artifact.bindings,
        )?;
        let result = self.dispatch_planned_buffers(
            program,
            &binding_plan,
            config,
            &artifact,
            &pipeline,
            buffers,
        )?;
        Ok(TimedDispatchResult {
            outputs: result.outputs,
            wall_ns: elapsed_ns(started, "Metal resident timed dispatch")?,
            device_ns: None,
            enqueue_ns: Some(result.enqueue_ns),
            wait_ns: Some(result.wait_ns),
        })
    }

    fn compile_native(
        &self,
        program: &Program,
        config: &DispatchConfig,
    ) -> Result<Option<Arc<dyn CompiledPipeline>>, BackendError> {
        self.compile_native_shared(Arc::new(program.clone()), config)
    }

    fn compile_native_shared(
        &self,
        program: Arc<Program>,
        config: &DispatchConfig,
    ) -> Result<Option<Arc<dyn CompiledPipeline>>, BackendError> {
        validate_metal_dispatch_config(
            config,
            "Metal cooperative grid compiled dispatch",
            "Metal compiled repeated dispatch",
            "Metal compiled dispatch",
        )?;
        let (cache_identity, artifact, pipeline) =
            self.compile_pipeline(program.as_ref(), config)?;
        let id = format!(
            "metal:{}",
            vyre_driver::pipeline::hex_encode(&cache_identity.digest)
        );
        Ok(Some(Arc::new(MetalPersistentPipeline {
            id,
            program,
            artifact,
            pipeline,
            device: self.device.clone(),
            queue: self.queue.clone(),
            resident_buffers: Arc::clone(&self.resident_buffers),
            metrics: Arc::clone(&self.metrics),
        })))
    }
}

type MetalResidentBufferTable = Arc<Mutex<BTreeMap<u64, MetalResidentBuffer>>>;
type MetalMetricCounters = Arc<MetalMetrics>;

#[derive(Default)]
struct MetalMetrics {
    pipeline_cache_hits: AtomicU64,
    pipeline_cache_misses: AtomicU64,
    pipeline_cache_miss_empty_cache: AtomicU64,
    pipeline_cache_miss_program_changed: AtomicU64,
    pipeline_cache_miss_dispatch_policy_changed: AtomicU64,
    pipeline_cache_miss_device_or_runtime_changed: AtomicU64,
    pipeline_cache_miss_key_absent: AtomicU64,
    buffer_allocation_count: AtomicU64,
    buffer_allocation_bytes: AtomicU64,
    host_to_device_copy_count: AtomicU64,
    host_to_device_bytes: AtomicU64,
    device_to_host_copy_count: AtomicU64,
    device_to_host_bytes: AtomicU64,
    output_readback_bytes: AtomicU64,
}

struct MetalDispatchResult {
    outputs: Vec<Vec<u8>>,
    enqueue_ns: u64,
    wait_ns: u64,
}

struct MetalCommandTiming {
    enqueue_ns: u64,
    wait_ns: u64,
}

#[derive(Clone)]
struct MetalResidentBuffer {
    buffer: Buffer,
    byte_len: usize,
}

#[derive(Clone)]
struct MetalCompiledPipeline {
    identity: PipelineCacheIdentity,
    artifact: vyre_emit_metal::MetalArtifact,
    pipeline: metal::ComputePipelineState,
}

struct MetalPersistentPipeline {
    id: String,
    program: Arc<Program>,
    artifact: vyre_emit_metal::MetalArtifact,
    pipeline: metal::ComputePipelineState,
    device: Device,
    queue: metal::CommandQueue,
    resident_buffers: MetalResidentBufferTable,
    metrics: MetalMetricCounters,
}

impl private::Sealed for MetalPersistentPipeline {}

impl CompiledPipeline for MetalPersistentPipeline {
    fn id(&self) -> &str {
        &self.id
    }

    fn dispatch(
        &self,
        inputs: &[Vec<u8>],
        config: &DispatchConfig,
    ) -> Result<Vec<Vec<u8>>, BackendError> {
        let borrowed =
            vyre_driver::backend::borrowed_input_slices(inputs, "metal compiled inputs")?;
        self.dispatch_borrowed(&borrowed, config)
    }

    fn dispatch_borrowed(
        &self,
        inputs: &[&[u8]],
        config: &DispatchConfig,
    ) -> Result<Vec<Vec<u8>>, BackendError> {
        self.dispatch_borrowed_timed(inputs, config)
            .map(|timed| timed.outputs)
    }

    fn dispatch_borrowed_timed(
        &self,
        inputs: &[&[u8]],
        config: &DispatchConfig,
    ) -> Result<TimedDispatchResult, BackendError> {
        let started = Instant::now();
        validate_metal_dispatch_config(
            config,
            "Metal cooperative grid compiled dispatch",
            "Metal compiled repeated dispatch",
            "Metal compiled dispatch",
        )?;
        let binding_plan = BindingPlan::from_borrowed_inputs(&self.program, inputs)?;
        let output_layouts = output_binding_layouts(&self.program)?;
        let output_by_binding = output_layout_map(output_layouts)?;
        let metal_slots = metal_slot_map(&self.artifact)?;
        let buffers = plan_buffers(
            &self.device,
            &binding_plan,
            inputs,
            &output_by_binding,
            &metal_slots,
            &self.artifact.bindings,
        )?;
        record_planned_buffer_metrics(&self.metrics, &buffers);
        let result = dispatch_planned_buffers_with_queue(
            &self.device,
            &self.queue,
            &self.program,
            &binding_plan,
            config,
            &self.artifact,
            &self.pipeline,
            buffers,
        )?;
        record_output_readback_metrics(&self.metrics, &result.outputs);
        Ok(TimedDispatchResult {
            outputs: result.outputs,
            wall_ns: elapsed_ns(started, "Metal compiled borrowed timed dispatch")?,
            device_ns: None,
            enqueue_ns: Some(result.enqueue_ns),
            wait_ns: Some(result.wait_ns),
        })
    }

    fn dispatch_borrowed_into(
        &self,
        inputs: &[&[u8]],
        config: &DispatchConfig,
        outputs: &mut OutputBuffers,
    ) -> Result<(), BackendError> {
        let result = self.dispatch_borrowed(inputs, config)?;
        let stats = vyre_driver::backend::replace_output_buffers_preserving_slots_with_memory_stats(
            result, outputs,
        );
        vyre_driver::observability::record_output_replacement_stats(stats);
        Ok(())
    }

    fn dispatch_persistent_handles(
        &self,
        inputs: &[Resource],
        config: &DispatchConfig,
    ) -> Result<OutputBuffers, BackendError> {
        self.dispatch_persistent_handles_timed(inputs, config)
            .map(|timed| timed.outputs)
    }

    fn dispatch_persistent_handles_timed(
        &self,
        inputs: &[Resource],
        config: &DispatchConfig,
    ) -> Result<TimedDispatchResult, BackendError> {
        let started = Instant::now();
        validate_metal_dispatch_config(
            config,
            "Metal cooperative grid compiled resident dispatch",
            "Metal compiled resident repeated dispatch",
            "Metal compiled resident dispatch",
        )?;
        let base_plan = BindingPlan::build(&self.program)?;
        let resolved =
            resolve_resident_resources_from_table(&self.resident_buffers, &base_plan, inputs)?;
        let input_lengths = resident_input_lengths(&base_plan, &resolved)?;
        let binding_plan = BindingPlan::from_input_lengths(&self.program, &input_lengths)?;
        let output_layouts = output_binding_layouts(&self.program)?;
        let output_by_binding = output_layout_map(output_layouts)?;
        let metal_slots = metal_slot_map(&self.artifact)?;
        let buffers = plan_resident_buffers(
            &self.device,
            &binding_plan,
            &resolved,
            &output_by_binding,
            &metal_slots,
            &self.artifact.bindings,
        )?;
        record_planned_buffer_metrics(&self.metrics, &buffers);
        let result = dispatch_planned_buffers_with_queue(
            &self.device,
            &self.queue,
            &self.program,
            &binding_plan,
            config,
            &self.artifact,
            &self.pipeline,
            buffers,
        )?;
        record_output_readback_metrics(&self.metrics, &result.outputs);
        Ok(TimedDispatchResult {
            outputs: result.outputs,
            wall_ns: elapsed_ns(started, "Metal compiled resident timed dispatch")?,
            device_ns: None,
            enqueue_ns: Some(result.enqueue_ns),
            wait_ns: Some(result.wait_ns),
        })
    }

    fn dispatch_persistent_handles_into(
        &self,
        inputs: &[Resource],
        config: &DispatchConfig,
        outputs: &mut OutputBuffers,
    ) -> Result<(), BackendError> {
        let result = self.dispatch_persistent_handles(inputs, config)?;
        let stats = vyre_driver::backend::replace_output_buffers_preserving_slots_with_memory_stats(
            result, outputs,
        );
        vyre_driver::observability::record_output_replacement_stats(stats);
        Ok(())
    }

    fn dispatch_persistent_resource_outputs(
        &self,
        inputs: &[Resource],
        config: &DispatchConfig,
    ) -> Result<Vec<Resource>, BackendError> {
        validate_metal_dispatch_config(
            config,
            "Metal cooperative grid compiled resident resource-output dispatch",
            "Metal compiled resident resource-output repeated dispatch",
            "Metal compiled resident resource-output dispatch",
        )?;
        let base_plan = BindingPlan::build(&self.program)?;
        let output_resources = resident_output_resources(&base_plan, inputs)?;
        let resolved =
            resolve_resident_resources_from_table(&self.resident_buffers, &base_plan, inputs)?;
        let input_lengths = resident_input_lengths(&base_plan, &resolved)?;
        let binding_plan = BindingPlan::from_input_lengths(&self.program, &input_lengths)?;
        let output_layouts = output_binding_layouts(&self.program)?;
        let output_by_binding = output_layout_map(output_layouts)?;
        let metal_slots = metal_slot_map(&self.artifact)?;
        let buffers = plan_resident_buffers(
            &self.device,
            &binding_plan,
            &resolved,
            &output_by_binding,
            &metal_slots,
            &self.artifact.bindings,
        )?;
        record_planned_buffer_metrics(&self.metrics, &buffers);
        submit_planned_buffers_with_queue(
            &self.device,
            &self.queue,
            &self.program,
            &binding_plan,
            config,
            &self.artifact,
            &self.pipeline,
            &buffers,
        )?;
        Ok(output_resources)
    }
}

fn lock_resident_buffer_table<'a>(
    resident_buffers: &'a MetalResidentBufferTable,
    operation: &'static str,
) -> Result<MutexGuard<'a, BTreeMap<u64, MetalResidentBuffer>>, BackendError> {
    resident_buffers
        .lock()
        .map_err(|error| BackendError::InvalidProgram {
            fix: format!(
                "Fix: Metal resident buffer table was poisoned during {operation}: {error}. Drop and reacquire the Metal backend before reusing resident resources."
            ),
        })
}

fn resolve_resident_resources_from_table<'a>(
    resident_buffers: &MetalResidentBufferTable,
    binding_plan: &BindingPlan,
    resources: &'a [Resource],
) -> Result<Vec<ResolvedMetalResource<'a>>, BackendError> {
    let expected = resident_resource_count(binding_plan)?;
    if resources.len() != expected {
        return Err(BackendError::InvalidProgram {
            fix: format!(
                "Fix: Metal resident dispatch expected {expected} resource(s) in binding order but received {}.",
                resources.len()
            ),
        });
    }
    let table =
        lock_resident_buffer_table(resident_buffers, "resident dispatch resource resolution")?;
    let mut resolved = Vec::new();
    resolved
        .try_reserve(expected)
        .map_err(|error| BackendError::InvalidProgram {
            fix: format!(
                "Fix: Metal resident dispatch could not reserve {expected} resolved resource descriptor(s): {error}. Split the dispatch bindings."
            ),
        })?;
    let mut resource_index = 0usize;
    for binding in &binding_plan.bindings {
        if binding.role == BindingRole::Shared {
            continue;
        }
        if binding.role == BindingRole::Persistent {
            return Err(BackendError::UnsupportedFeature {
                name: format!(
                    "Metal persistent buffer binding `{}` in resident dispatch",
                    binding.name
                ),
                backend: METAL_BACKEND_ID.to_string(),
            });
        }
        let resource = resources.get(resource_index).ok_or_else(|| BackendError::InvalidProgram {
            fix: format!(
                "Fix: Metal resident dispatch missing resource slot {resource_index} for binding {} (`{}`).",
                binding.binding, binding.name
            ),
        })?;
        match resource {
            Resource::Borrowed(bytes) => resolved.push(ResolvedMetalResource::Borrowed(bytes)),
            Resource::Resident(id) => {
                let resident = table.get(id).cloned().ok_or_else(|| {
                    BackendError::InvalidProgram {
                        fix: format!(
                            "Fix: Metal resident dispatch received stale handle {id} for binding {} (`{}`). Keep resident resources allocated until dispatch completes.",
                            binding.binding, binding.name
                        ),
                    }
                })?;
                resolved.push(ResolvedMetalResource::Resident {
                    id: *id,
                    buffer: resident.buffer,
                    byte_len: resident.byte_len,
                });
            }
        }
        resource_index += 1;
    }
    Ok(resolved)
}

fn resident_output_resources(
    binding_plan: &BindingPlan,
    resources: &[Resource],
) -> Result<Vec<Resource>, BackendError> {
    let mut outputs = Vec::new();
    outputs
        .try_reserve(binding_plan.output_indices.len())
        .map_err(|error| BackendError::InvalidProgram {
            fix: format!(
                "Fix: Metal compiled resident resource-output dispatch could not reserve {} output resource slot(s): {error}. Split the resident dispatch.",
                binding_plan.output_indices.len()
            ),
        })?;
    outputs.resize_with(binding_plan.output_indices.len(), || None);

    let mut resource_index = 0usize;
    for binding in &binding_plan.bindings {
        if binding.role == BindingRole::Shared {
            continue;
        }
        let resource = resources.get(resource_index).ok_or_else(|| BackendError::InvalidProgram {
            fix: format!(
                "Fix: Metal compiled resident resource-output dispatch missing resource slot {resource_index} for binding {} (`{}`).",
                binding.binding, binding.name
            ),
        })?;
        if let Some(output_index) = binding.output_index {
            let Resource::Resident(id) = resource else {
                return Err(BackendError::InvalidProgram {
                    fix: format!(
                        "Fix: Metal compiled resident resource-output dispatch cannot return borrowed output binding {} (`{}`). Allocate a resident output buffer and pass Resource::Resident so the backend can skip host readback.",
                        binding.binding, binding.name
                    ),
                });
            };
            let slot = outputs.get_mut(output_index).ok_or_else(|| BackendError::InvalidProgram {
                fix: format!(
                    "Fix: Metal compiled resident resource-output dispatch output index {output_index} for binding {} (`{}`) is outside {} output slot(s). Rebuild BindingPlan before dispatch.",
                    binding.binding,
                    binding.name,
                    binding_plan.output_indices.len()
                ),
            })?;
            *slot = Some(Resource::Resident(*id));
        }
        resource_index += 1;
    }
    if resource_index != resources.len() {
        return Err(BackendError::InvalidProgram {
            fix: format!(
                "Fix: Metal compiled resident resource-output dispatch received {} resource(s) but consumed {resource_index}. Pass one resource per public non-shared binding with no extras.",
                resources.len()
            ),
        });
    }
    outputs
        .into_iter()
        .enumerate()
        .map(|(output_index, resource)| {
            resource.ok_or_else(|| BackendError::InvalidProgram {
                fix: format!(
                    "Fix: Metal compiled resident resource-output dispatch did not resolve output slot {output_index}. Rebuild BindingPlan output indices before dispatch."
                ),
            })
        })
        .collect()
}

enum ResolvedMetalResource<'a> {
    Borrowed(&'a [u8]),
    Resident {
        id: u64,
        buffer: Buffer,
        byte_len: usize,
    },
}

impl ResolvedMetalResource<'_> {
    fn byte_len(&self) -> usize {
        match self {
            Self::Borrowed(bytes) => bytes.len(),
            Self::Resident { byte_len, .. } => *byte_len,
        }
    }
}

struct PlannedBuffer {
    binding: u32,
    metal_slot: u8,
    buffer: Buffer,
    allocated_bytes: usize,
    host_to_device_bytes: usize,
}

fn output_layout_map(
    output_layouts: Vec<OutputBindingLayout>,
) -> Result<BTreeMap<u32, OutputBindingLayout>, BackendError> {
    let mut by_binding = BTreeMap::new();
    for layout in output_layouts {
        if by_binding.insert(layout.binding, layout).is_some() {
            return Err(BackendError::InvalidProgram {
                fix: "Fix: output layout planning produced duplicate output bindings; rebuild the Program with unique buffer bindings.".to_string(),
            });
        }
    }
    Ok(by_binding)
}

fn metal_slot_map(
    artifact: &vyre_emit_metal::MetalArtifact,
) -> Result<BTreeMap<u32, u8>, BackendError> {
    let mut slots = BTreeMap::new();
    for binding in &artifact.bindings {
        if slots
            .insert(binding.slot, binding.metal_buffer_index)
            .is_some()
        {
            return Err(BackendError::KernelCompileFailed {
                backend: METAL_BACKEND_ID.to_string(),
                compiler_message: format!(
                    "Metal artifact contains duplicate binding metadata for slot {}",
                    binding.slot
                ),
            });
        }
    }
    Ok(slots)
}

fn plan_buffers(
    device: &Device,
    binding_plan: &BindingPlan,
    inputs: &[&[u8]],
    output_by_binding: &BTreeMap<u32, OutputBindingLayout>,
    metal_slots: &BTreeMap<u32, u8>,
    artifact_bindings: &[vyre_emit_metal::MetalBindingMetadata],
) -> Result<Vec<PlannedBuffer>, BackendError> {
    let mut buffers = Vec::new();
    let reserve_len = binding_plan
        .bindings
        .len()
        .checked_add(artifact_bindings.len())
        .ok_or_else(|| BackendError::InvalidProgram {
            fix: "Fix: Metal buffer planning binding count overflowed usize. Split the Program bindings.".to_string(),
        })?;
    buffers.try_reserve(reserve_len).map_err(|error| {
        BackendError::InvalidProgram {
            fix: format!(
                "Fix: Metal buffer planning could not reserve {reserve_len} binding slot(s): {error}. Split the Program bindings.",
            ),
        }
    })?;

    for binding in &binding_plan.bindings {
        if binding.role == BindingRole::Shared {
            continue;
        }
        if binding.role == BindingRole::Persistent {
            return Err(BackendError::UnsupportedFeature {
                name: format!(
                    "Metal persistent buffer binding `{}` in non-resident dispatch",
                    binding.name
                ),
                backend: METAL_BACKEND_ID.to_string(),
            });
        }
        let metal_slot = metal_slots.get(&binding.binding).copied().ok_or_else(|| {
            BackendError::KernelCompileFailed {
                backend: METAL_BACKEND_ID.to_string(),
                compiler_message: format!(
                    "Metal artifact did not include ABI metadata for binding {} (`{}`)",
                    binding.binding, binding.name
                ),
            }
        })?;
        let (buffer, allocated_bytes, host_to_device_bytes) = match binding.role {
            BindingRole::Input | BindingRole::Uniform => {
                let input_index =
                    binding
                        .input_index
                        .ok_or_else(|| BackendError::InvalidProgram {
                            fix: format!(
                        "Fix: Metal binding `{}` is {:?} but has no input index in BindingPlan.",
                        binding.name, binding.role
                    ),
                        })?;
                let bytes = inputs.get(input_index).ok_or_else(|| BackendError::InvalidProgram {
                    fix: format!(
                        "Fix: Metal input index {input_index} for binding `{}` is missing after BindingPlan validation.",
                        binding.name
                    ),
                })?;
                (
                    new_host_input_buffer(device, bytes)?,
                    metal_physical_buffer_len(bytes.len()),
                    bytes.len(),
                )
            }
            BindingRole::Output => {
                let layout = output_by_binding.get(&binding.binding).ok_or_else(|| {
                    BackendError::InvalidProgram {
                        fix: format!(
                            "Fix: Metal output binding {} (`{}`) has no output readback layout.",
                            binding.binding, binding.name
                        ),
                    }
                })?;
                let byte_len = allocation_len_for_output(layout)?;
                (
                    new_zero_buffer(device, byte_len)?,
                    metal_physical_buffer_len(byte_len),
                    0,
                )
            }
            BindingRole::InputOutput => {
                let input_index =
                    binding
                        .input_index
                        .ok_or_else(|| BackendError::InvalidProgram {
                            fix: format!(
                        "Fix: Metal read-write binding `{}` has no input index in BindingPlan.",
                        binding.name
                    ),
                        })?;
                let bytes = inputs.get(input_index).ok_or_else(|| BackendError::InvalidProgram {
                    fix: format!(
                        "Fix: Metal read-write input index {input_index} for binding `{}` is missing after BindingPlan validation.",
                        binding.name
                    ),
                })?;
                let output_len = output_by_binding
                    .get(&binding.binding)
                    .map(allocation_len_for_output)
                    .transpose()?
                    .unwrap_or(4);
                let byte_len = output_len.max(bytes.len()).max(4);
                let buffer = new_zero_buffer(device, byte_len)?;
                copy_to_shared_buffer(&buffer, bytes)?;
                (buffer, metal_physical_buffer_len(byte_len), bytes.len())
            }
            BindingRole::Shared | BindingRole::Persistent => unreachable!(),
        };
        buffers.push(PlannedBuffer {
            binding: binding.binding,
            metal_slot,
            buffer,
            allocated_bytes,
            host_to_device_bytes,
        });
    }
    for binding in artifact_bindings {
        if buffers
            .iter()
            .any(|planned| planned.binding == binding.slot)
        {
            continue;
        }
        if binding.name == vyre_lower::TRAP_SIDECAR_NAME {
            buffers.push(PlannedBuffer {
                binding: binding.slot,
                metal_slot: binding.metal_buffer_index,
                buffer: new_zero_buffer(device, trap_sidecar_byte_len()?)?,
                allocated_bytes: metal_physical_buffer_len(trap_sidecar_byte_len()?),
                host_to_device_bytes: 0,
            });
            continue;
        }
        return Err(BackendError::InvalidProgram {
            fix: format!(
                "Fix: Metal artifact binding {} (`{}`) was not allocated by the user BindingPlan and is not a recognized backend-owned binding. Keep descriptor artifact bindings synchronized with Program buffers.",
                binding.slot, binding.name
            ),
        });
    }
    Ok(buffers)
}

fn plan_resident_buffers(
    device: &Device,
    binding_plan: &BindingPlan,
    resources: &[ResolvedMetalResource<'_>],
    output_by_binding: &BTreeMap<u32, OutputBindingLayout>,
    metal_slots: &BTreeMap<u32, u8>,
    artifact_bindings: &[vyre_emit_metal::MetalBindingMetadata],
) -> Result<Vec<PlannedBuffer>, BackendError> {
    let mut buffers = Vec::new();
    let reserve_len = binding_plan
        .bindings
        .len()
        .checked_add(artifact_bindings.len())
        .ok_or_else(|| BackendError::InvalidProgram {
            fix: "Fix: Metal resident buffer planning binding count overflowed usize. Split the Program bindings.".to_string(),
        })?;
    buffers
        .try_reserve(reserve_len)
        .map_err(|error| BackendError::InvalidProgram {
            fix: format!(
                "Fix: Metal resident buffer planning could not reserve {reserve_len} binding slot(s): {error}. Split the Program bindings."
            ),
        })?;

    let mut resource_index = 0usize;
    for binding in &binding_plan.bindings {
        if binding.role == BindingRole::Shared {
            continue;
        }
        if binding.role == BindingRole::Persistent {
            return Err(BackendError::UnsupportedFeature {
                name: format!(
                    "Metal persistent buffer binding `{}` in resident dispatch",
                    binding.name
                ),
                backend: METAL_BACKEND_ID.to_string(),
            });
        }
        let metal_slot = metal_slots.get(&binding.binding).copied().ok_or_else(|| {
            BackendError::KernelCompileFailed {
                backend: METAL_BACKEND_ID.to_string(),
                compiler_message: format!(
                    "Metal artifact did not include ABI metadata for binding {} (`{}`)",
                    binding.binding, binding.name
                ),
            }
        })?;
        let resource = resources.get(resource_index).ok_or_else(|| BackendError::InvalidProgram {
            fix: format!(
                "Fix: Metal resident buffer planning missing resource slot {resource_index} for binding {} (`{}`).",
                binding.binding, binding.name
            ),
        })?;
        let (buffer, allocated_bytes, host_to_device_bytes) = match binding.role {
            BindingRole::Input | BindingRole::Uniform => {
                let (allocated_bytes, host_to_device_bytes) =
                    materialized_read_resource_metrics(resource);
                (
                    materialize_read_resource(device, resource)?,
                    allocated_bytes,
                    host_to_device_bytes,
                )
            }
            BindingRole::Output => {
                let layout = output_by_binding.get(&binding.binding).ok_or_else(|| {
                    BackendError::InvalidProgram {
                        fix: format!(
                            "Fix: Metal resident output binding {} (`{}`) has no output readback layout.",
                            binding.binding, binding.name
                        ),
                    }
                })?;
                let required = allocation_len_for_output(layout)?;
                let allocated_bytes = materialized_output_resource_allocation(resource, required);
                (
                    materialize_output_resource(device, resource, layout, binding.binding)?,
                    allocated_bytes,
                    0,
                )
            }
            BindingRole::InputOutput => {
                let output_len = output_by_binding
                    .get(&binding.binding)
                    .map(allocation_len_for_output)
                    .transpose()?
                    .unwrap_or(4);
                let (allocated_bytes, host_to_device_bytes) =
                    materialized_read_write_resource_metrics(resource, output_len);
                (
                    materialize_read_write_resource(device, resource, output_len, binding.binding)?,
                    allocated_bytes,
                    host_to_device_bytes,
                )
            }
            BindingRole::Shared | BindingRole::Persistent => unreachable!(),
        };
        buffers.push(PlannedBuffer {
            binding: binding.binding,
            metal_slot,
            buffer,
            allocated_bytes,
            host_to_device_bytes,
        });
        resource_index += 1;
    }
    for binding in artifact_bindings {
        if buffers
            .iter()
            .any(|planned| planned.binding == binding.slot)
        {
            continue;
        }
        if binding.name == vyre_lower::TRAP_SIDECAR_NAME {
            let byte_len = trap_sidecar_byte_len()?;
            buffers.push(PlannedBuffer {
                binding: binding.slot,
                metal_slot: binding.metal_buffer_index,
                buffer: new_zero_buffer(device, byte_len)?,
                allocated_bytes: metal_physical_buffer_len(byte_len),
                host_to_device_bytes: 0,
            });
            continue;
        }
        return Err(BackendError::InvalidProgram {
            fix: format!(
                "Fix: Metal resident artifact binding {} (`{}`) was not allocated by the user BindingPlan and is not a recognized backend-owned binding. Keep descriptor artifact bindings synchronized with Program buffers.",
                binding.slot, binding.name
            ),
        });
    }
    Ok(buffers)
}

fn materialize_read_resource(
    device: &Device,
    resource: &ResolvedMetalResource<'_>,
) -> Result<Buffer, BackendError> {
    match resource {
        ResolvedMetalResource::Borrowed(bytes) => new_host_input_buffer(device, bytes),
        ResolvedMetalResource::Resident { buffer, .. } => Ok(buffer.clone()),
    }
}

fn materialized_read_resource_metrics(resource: &ResolvedMetalResource<'_>) -> (usize, usize) {
    match resource {
        ResolvedMetalResource::Borrowed(bytes) => {
            (metal_physical_buffer_len(bytes.len()), bytes.len())
        }
        ResolvedMetalResource::Resident { .. } => (0, 0),
    }
}

fn materialize_output_resource(
    device: &Device,
    resource: &ResolvedMetalResource<'_>,
    layout: &OutputBindingLayout,
    binding: u32,
) -> Result<Buffer, BackendError> {
    let required = allocation_len_for_output(layout)?;
    match resource {
        ResolvedMetalResource::Borrowed(_) => new_zero_buffer(device, required),
        ResolvedMetalResource::Resident {
            id,
            buffer,
            byte_len,
        } => {
            if *byte_len < required {
                return Err(BackendError::InvalidProgram {
                    fix: format!(
                        "Fix: Metal resident output binding {binding} (`{}`) requires {required} byte(s), but handle {id} has {byte_len}. Allocate a larger resident output buffer.",
                        layout.name
                    ),
                });
            }
            zero_shared_buffer_range(buffer, 0, required, "resident output clear")?;
            Ok(buffer.clone())
        }
    }
}

fn materialized_output_resource_allocation(
    resource: &ResolvedMetalResource<'_>,
    required: usize,
) -> usize {
    match resource {
        ResolvedMetalResource::Borrowed(_) => metal_physical_buffer_len(required),
        ResolvedMetalResource::Resident { .. } => 0,
    }
}

fn materialize_read_write_resource(
    device: &Device,
    resource: &ResolvedMetalResource<'_>,
    output_len: usize,
    binding: u32,
) -> Result<Buffer, BackendError> {
    match resource {
        ResolvedMetalResource::Borrowed(bytes) => {
            let byte_len = output_len.max(bytes.len()).max(4);
            let buffer = new_zero_buffer(device, byte_len)?;
            copy_to_shared_buffer(&buffer, bytes)?;
            Ok(buffer)
        }
        ResolvedMetalResource::Resident {
            id,
            buffer,
            byte_len,
        } => {
            if *byte_len < output_len {
                return Err(BackendError::InvalidProgram {
                    fix: format!(
                        "Fix: Metal resident read-write binding {binding} requires {output_len} output byte(s), but handle {id} has {byte_len}. Allocate a larger resident buffer."
                    ),
                });
            }
            Ok(buffer.clone())
        }
    }
}

fn materialized_read_write_resource_metrics(
    resource: &ResolvedMetalResource<'_>,
    output_len: usize,
) -> (usize, usize) {
    match resource {
        ResolvedMetalResource::Borrowed(bytes) => (
            metal_physical_buffer_len(output_len.max(bytes.len()).max(4)),
            bytes.len(),
        ),
        ResolvedMetalResource::Resident { .. } => (0, 0),
    }
}

fn resident_resource_count(binding_plan: &BindingPlan) -> Result<usize, BackendError> {
    binding_plan
        .bindings
        .iter()
        .try_fold(0usize, |count, binding| {
            if binding.role == BindingRole::Shared {
                return Ok(count);
            }
            count.checked_add(1).ok_or_else(|| BackendError::InvalidProgram {
                fix: "Fix: Metal resident resource count overflowed usize. Split the Program bindings.".to_string(),
            })
        })
}

fn resident_input_lengths(
    binding_plan: &BindingPlan,
    resources: &[ResolvedMetalResource<'_>],
) -> Result<Vec<usize>, BackendError> {
    let mut input_lengths = vec![0usize; binding_plan.input_indices.len()];
    let mut resource_index = 0usize;
    for binding in &binding_plan.bindings {
        if binding.role == BindingRole::Shared {
            continue;
        }
        let resource = resources.get(resource_index).ok_or_else(|| BackendError::InvalidProgram {
            fix: format!(
                "Fix: Metal resident input length planning missing resource slot {resource_index} for binding {} (`{}`).",
                binding.binding, binding.name
            ),
        })?;
        if let Some(input_index) = binding.input_index {
            let input_slot_count = input_lengths.len();
            let slot = input_lengths.get_mut(input_index).ok_or_else(|| {
                BackendError::InvalidProgram {
                    fix: format!(
                        "Fix: Metal resident binding `{}` references input index {input_index}, but BindingPlan only has {} input slot(s).",
                        binding.name,
                        input_slot_count
                    ),
                }
            })?;
            *slot = resource.byte_len();
        }
        resource_index += 1;
    }
    Ok(input_lengths)
}

fn next_resident_id(counter: &AtomicU64) -> Result<u64, BackendError> {
    counter
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            current.checked_add(1).filter(|next| *next != 0)
        })
        .map_err(|current| BackendError::InvalidProgram {
            fix: format!(
                "Fix: Metal resident handle id counter exhausted at {current}. Drop and reacquire the backend before allocating more resident buffers."
            ),
        })
}

fn record_planned_buffer_metrics(metrics: &MetalMetrics, buffers: &[PlannedBuffer]) {
    let mut allocation_count = 0_u64;
    let mut allocation_bytes = 0_u64;
    let mut host_to_device_copy_count = 0_u64;
    let mut host_to_device_bytes = 0_u64;
    for buffer in buffers {
        if buffer.allocated_bytes > 0 {
            allocation_count = allocation_count.saturating_add(1);
            allocation_bytes =
                allocation_bytes.saturating_add(usize_to_u64_saturating(buffer.allocated_bytes));
        }
        if buffer.host_to_device_bytes > 0 {
            host_to_device_copy_count = host_to_device_copy_count.saturating_add(1);
            host_to_device_bytes = host_to_device_bytes
                .saturating_add(usize_to_u64_saturating(buffer.host_to_device_bytes));
        }
    }
    add_atomic_metric(&metrics.buffer_allocation_count, allocation_count);
    add_atomic_metric(&metrics.buffer_allocation_bytes, allocation_bytes);
    add_atomic_metric(
        &metrics.host_to_device_copy_count,
        host_to_device_copy_count,
    );
    add_atomic_metric(&metrics.host_to_device_bytes, host_to_device_bytes);
}

fn record_output_readback_metrics(metrics: &MetalMetrics, outputs: &[Vec<u8>]) {
    let mut readback_count = 0_u64;
    let mut readback_bytes = 0_u64;
    for output in outputs {
        if !output.is_empty() {
            readback_count = readback_count.saturating_add(1);
            readback_bytes = readback_bytes.saturating_add(usize_to_u64_saturating(output.len()));
        }
    }
    add_atomic_metric(&metrics.device_to_host_copy_count, readback_count);
    add_atomic_metric(&metrics.device_to_host_bytes, readback_bytes);
    add_atomic_metric(&metrics.output_readback_bytes, readback_bytes);
}

fn record_host_to_device_copy(metrics: &MetalMetrics, byte_len: usize) {
    if byte_len == 0 {
        return;
    }
    add_atomic_metric(&metrics.host_to_device_copy_count, 1);
    add_atomic_metric(
        &metrics.host_to_device_bytes,
        usize_to_u64_saturating(byte_len),
    );
}

fn record_device_to_host_copy(metrics: &MetalMetrics, byte_len: usize) {
    if byte_len == 0 {
        return;
    }
    add_atomic_metric(&metrics.device_to_host_copy_count, 1);
    add_atomic_metric(
        &metrics.device_to_host_bytes,
        usize_to_u64_saturating(byte_len),
    );
}

fn record_buffer_allocation(metrics: &MetalMetrics, byte_len: usize) {
    add_atomic_metric(&metrics.buffer_allocation_count, 1);
    add_atomic_metric(
        &metrics.buffer_allocation_bytes,
        usize_to_u64_saturating(metal_physical_buffer_len(byte_len)),
    );
}

fn add_atomic_metric(counter: &AtomicU64, value: u64) {
    if value == 0 {
        return;
    }
    let _ = counter.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
        Some(current.saturating_add(value))
    });
}

fn usize_to_u64_saturating(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

fn metal_physical_buffer_len(byte_len: usize) -> usize {
    byte_len.max(4)
}

fn metal_pipeline_cache_key(
    program: &Program,
    config: &DispatchConfig,
    device: &Device,
) -> Result<PipelineCacheIdentity, BackendError> {
    let device_name = device.name();
    let revision_extra = format!(
        "artifact_schema={}:msl={}.{}:driver={}:device={}",
        vyre_emit_metal::METAL_ARTIFACT_SCHEMA,
        vyre_emit_metal::DEFAULT_MSL_VERSION.0,
        vyre_emit_metal::DEFAULT_MSL_VERSION.1,
        env!("CARGO_PKG_VERSION"),
        device_name
    );
    let fingerprint = vyre_driver::pipeline::PipelineDeviceFingerprint::from_parts(
        0x106b,
        0,
        "metal",
        &revision_extra,
    );
    PipelineCacheIdentity::try_from_program(program, config, fingerprint).map_err(|error| {
        BackendError::InvalidProgram {
            fix: format!(
                "Fix: Metal pipeline cache could not build shared Program/policy/device identity: {error}"
            ),
        }
    })
}

fn validate_metal_dispatch_config(
    config: &DispatchConfig,
    cooperative_feature: &'static str,
    repeated_feature: &'static str,
    zero_iteration_context: &'static str,
) -> Result<(), BackendError> {
    if config.cooperative {
        return Err(BackendError::UnsupportedFeature {
            name: cooperative_feature.to_string(),
            backend: METAL_BACKEND_ID.to_string(),
        });
    }
    if matches!(config.fixpoint_iterations, Some(0)) {
        return Err(BackendError::InvalidProgram {
            fix: format!(
                "Fix: {zero_iteration_context} received fixpoint_iterations=0; use None or a positive iteration count."
            ),
        });
    }
    if let Some(iterations) = config.fixpoint_iterations {
        if iterations != 1 {
            return Err(BackendError::UnsupportedFeature {
                name: format!("{repeated_feature} with {iterations} iterations"),
                backend: METAL_BACKEND_ID.to_string(),
            });
        }
    }
    Ok(())
}

fn validate_resident_range(
    handle_id: u64,
    allocation_len: usize,
    byte_offset: usize,
    byte_len: usize,
    context: &'static str,
) -> Result<(), BackendError> {
    let end = byte_offset
        .checked_add(byte_len)
        .ok_or_else(|| BackendError::InvalidProgram {
            fix: format!(
                "Fix: Metal {context} overflows usize at offset {byte_offset} len {byte_len} for handle {handle_id}. Split the resident range."
            ),
        })?;
    if end > allocation_len {
        return Err(BackendError::InvalidProgram {
            fix: format!(
                "Fix: Metal {context} requested byte range [{byte_offset}..{end}) from allocation {allocation_len} on handle {handle_id}. Clamp the range or allocate a larger resident buffer."
            ),
        });
    }
    Ok(())
}

fn copy_to_shared_buffer_range(
    buffer: &Buffer,
    dst_offset_bytes: usize,
    bytes: &[u8],
    context: &'static str,
) -> Result<(), BackendError> {
    if bytes.is_empty() {
        return Ok(());
    }
    let capacity = metal_buffer_len(buffer, context)?;
    let end = dst_offset_bytes
        .checked_add(bytes.len())
        .ok_or_else(|| BackendError::InvalidProgram {
            fix: format!(
                "Fix: Metal {context} upload range overflows usize at offset {dst_offset_bytes} len {}.",
                bytes.len()
            ),
        })?;
    if end > capacity {
        return Err(BackendError::InvalidProgram {
            fix: format!(
                "Fix: Metal {context} upload range [{dst_offset_bytes}..{end}) exceeds physical buffer length {capacity}. Rebuild the resident allocation."
            ),
        });
    }
    // SAFETY: `capacity` was read from Metal, the checked range is in bounds,
    // and the source/destination ranges do not overlap.
    unsafe {
        std::ptr::copy_nonoverlapping(
            bytes.as_ptr(),
            buffer.contents().cast::<u8>().add(dst_offset_bytes),
            bytes.len(),
        );
    }
    buffer.did_modify_range(metal::NSRange::new(
        checked_ns_uint(
            dst_offset_bytes,
            "Metal resident upload modified range offset",
        )?,
        checked_ns_uint(bytes.len(), "Metal resident upload modified range length")?,
    ));
    Ok(())
}

fn zero_shared_buffer_range(
    buffer: &Buffer,
    dst_offset_bytes: usize,
    byte_len: usize,
    context: &'static str,
) -> Result<(), BackendError> {
    if byte_len == 0 {
        return Ok(());
    }
    let capacity = metal_buffer_len(buffer, context)?;
    let end = dst_offset_bytes
        .checked_add(byte_len)
        .ok_or_else(|| BackendError::InvalidProgram {
            fix: format!(
                "Fix: Metal {context} zero range overflows usize at offset {dst_offset_bytes} len {byte_len}."
            ),
        })?;
    if end > capacity {
        return Err(BackendError::InvalidProgram {
            fix: format!(
                "Fix: Metal {context} zero range [{dst_offset_bytes}..{end}) exceeds physical buffer length {capacity}. Rebuild the resident allocation."
            ),
        });
    }
    // SAFETY: the checked range is in bounds for a live StorageModeShared
    // buffer allocated by this backend.
    unsafe {
        std::ptr::write_bytes(
            buffer.contents().cast::<u8>().add(dst_offset_bytes),
            0,
            byte_len,
        );
    }
    buffer.did_modify_range(metal::NSRange::new(
        checked_ns_uint(
            dst_offset_bytes,
            "Metal resident zero modified range offset",
        )?,
        checked_ns_uint(byte_len, "Metal resident zero modified range length")?,
    ));
    Ok(())
}

fn copy_shared_buffer_range_into(
    buffer: &Buffer,
    byte_offset: usize,
    byte_len: usize,
    out: &mut Vec<u8>,
    context: &'static str,
) -> Result<(), BackendError> {
    let capacity = metal_buffer_len(buffer, context)?;
    let end = byte_offset
        .checked_add(byte_len)
        .ok_or_else(|| BackendError::InvalidProgram {
            fix: format!(
                "Fix: Metal {context} read range overflows usize at offset {byte_offset} len {byte_len}."
            ),
        })?;
    if end > capacity {
        return Err(BackendError::InvalidProgram {
            fix: format!(
                "Fix: Metal {context} read range [{byte_offset}..{end}) exceeds physical buffer length {capacity}. Rebuild the resident allocation."
            ),
        });
    }
    let additional = byte_len.saturating_sub(out.capacity());
    if additional != 0 {
        out.try_reserve_exact(additional)
            .map_err(|error| BackendError::InvalidProgram {
                fix: format!(
                    "Fix: Metal {context} could not reserve {byte_len} output byte(s): {error}. Split the resident readback."
                ),
            })?;
    }
    out.clear();
    if byte_len == 0 {
        return Ok(());
    }
    // SAFETY: the checked range is in bounds for a live StorageModeShared
    // buffer allocated by this backend.
    let source = unsafe {
        std::slice::from_raw_parts(buffer.contents().cast::<u8>().add(byte_offset), byte_len)
    };
    out.extend_from_slice(source);
    Ok(())
}

fn reserve_fused_resident_view_outputs(
    copies: &[ResidentTransferInterval],
    views: &[ResidentTransferView],
    outputs: &mut [&mut Vec<u8>],
) -> Result<(), BackendError> {
    if views.len() != outputs.len() {
        return Err(BackendError::InvalidProgram {
            fix: format!(
                "Fix: Metal resident ranged batch download fused {} output view(s) for {} requested output slot(s). Keep resident transfer fusion cardinality-preserving before materializing outputs.",
                views.len(),
                outputs.len()
            ),
        });
    }
    for (view_index, (view, output)) in views.iter().copied().zip(outputs.iter_mut()).enumerate() {
        if view.byte_len != 0 {
            let copy = copies.get(view.copy_slot).ok_or_else(|| BackendError::InvalidProgram {
                fix: format!(
                    "Fix: Metal resident ranged batch download view {view_index} references missing fused copy slot {}. Rebuild the resident transfer fusion plan before materializing outputs.",
                    view.copy_slot
                ),
            })?;
            let view_end =
                view.byte_offset
                    .checked_add(view.byte_len)
                    .ok_or_else(|| BackendError::InvalidProgram {
                        fix: format!(
                            "Fix: Metal resident ranged batch download view {view_index} overflows usize at offset {} len {}. Rebuild the resident transfer fusion plan before materializing outputs.",
                            view.byte_offset, view.byte_len
                        ),
                    })?;
            if view_end > copy.byte_len {
                return Err(BackendError::InvalidProgram {
                    fix: format!(
                        "Fix: Metal resident ranged batch download view {view_index} requested bytes [{}..{}) from a {} byte fused output. Rebuild the resident transfer fusion plan before materializing outputs.",
                        view.byte_offset,
                        view_end,
                        copy.byte_len
                    ),
                });
            }
        }
        if view.byte_len > output.capacity() {
            output
                .try_reserve_exact(view.byte_len - output.capacity())
                .map_err(|error| BackendError::InvalidProgram {
                    fix: format!(
                        "Fix: Metal resident ranged batch download could not reserve {} output byte(s) for view {view_index}: {error}. Split the resident readback batch before materializing outputs.",
                        view.byte_len
                    ),
                })?;
        }
    }
    Ok(())
}

fn copy_fused_resident_view_into(
    fused_outputs: &[Vec<u8>],
    view: ResidentTransferView,
    output: &mut Vec<u8>,
) -> Result<(), BackendError> {
    if view.byte_len == 0 {
        output.clear();
        return Ok(());
    }
    let fused_output = fused_outputs
        .get(view.copy_slot)
        .ok_or_else(|| BackendError::InvalidProgram {
            fix: format!(
                "Fix: Metal resident ranged batch download view references missing fused copy slot {}. Rebuild the resident transfer fusion plan before materializing outputs.",
                view.copy_slot
            ),
        })?;
    let view_end =
        view.byte_offset
            .checked_add(view.byte_len)
            .ok_or_else(|| BackendError::InvalidProgram {
                fix: format!(
                    "Fix: Metal resident ranged batch download view overflows usize at offset {} len {}. Rebuild the resident transfer fusion plan before materializing outputs.",
                    view.byte_offset, view.byte_len
                ),
            })?;
    let bytes =
        fused_output
            .get(view.byte_offset..view_end)
            .ok_or_else(|| BackendError::InvalidProgram {
                fix: format!(
                    "Fix: Metal resident ranged batch download view requested bytes [{}..{}) from a {} byte fused output. Rebuild the resident transfer fusion plan before materializing outputs.",
                    view.byte_offset,
                    view_end,
                    fused_output.len()
                ),
            })?;
    if output.len() == bytes.len() {
        output.copy_from_slice(bytes);
        return Ok(());
    }
    if bytes.len() > output.capacity() {
        output
            .try_reserve_exact(bytes.len() - output.capacity())
            .map_err(|error| BackendError::InvalidProgram {
                fix: format!(
                    "Fix: Metal resident ranged batch download could not reserve {} output byte(s): {error}. Split the resident readback batch before materializing outputs.",
                    bytes.len()
                ),
            })?;
    }
    output.clear();
    output.extend_from_slice(bytes);
    Ok(())
}

fn metal_buffer_len(buffer: &Buffer, context: &'static str) -> Result<usize, BackendError> {
    usize::try_from(buffer.length()).map_err(|error| BackendError::InvalidProgram {
        fix: format!(
            "Fix: Metal {context} buffer length cannot fit usize: {error}. Split the resident buffer."
        ),
    })
}

fn trap_sidecar_byte_len() -> Result<usize, BackendError> {
    usize::try_from(vyre_lower::TRAP_SIDECAR_WORDS)
        .map_err(|error| BackendError::InvalidProgram {
            fix: format!(
                "Fix: trap sidecar word count cannot fit usize: {error}. Keep TRAP_SIDECAR_WORDS within the host index ABI."
            ),
        })?
        .checked_mul(std::mem::size_of::<u32>())
        .ok_or_else(|| BackendError::InvalidProgram {
            fix: "Fix: trap sidecar byte length overflowed usize. Keep TRAP_SIDECAR_WORDS within the host index ABI.".to_string(),
        })
}

fn new_host_input_buffer(device: &Device, bytes: &[u8]) -> Result<Buffer, BackendError> {
    if bytes.is_empty() {
        return Ok(new_zero_buffer(device, 4)?);
    }
    Ok(device.new_buffer_with_data(
        bytes.as_ptr().cast::<c_void>(),
        checked_ns_uint(bytes.len(), "Metal input buffer length")?,
        MTLResourceOptions::StorageModeShared,
    ))
}

fn new_zero_buffer(device: &Device, byte_len: usize) -> Result<Buffer, BackendError> {
    Ok(device.new_buffer(
        checked_ns_uint(byte_len.max(4), "Metal zero buffer length")?,
        MTLResourceOptions::StorageModeShared,
    ))
}

fn copy_to_shared_buffer(buffer: &Buffer, bytes: &[u8]) -> Result<(), BackendError> {
    if bytes.is_empty() {
        return Ok(());
    }
    let capacity = usize::try_from(buffer.length()).map_err(|error| BackendError::InvalidProgram {
        fix: format!(
            "Fix: Metal buffer length cannot fit usize during upload: {error}. Split the dispatch buffer."
        ),
    })?;
    if bytes.len() > capacity {
        return Err(BackendError::InvalidProgram {
            fix: format!(
                "Fix: Metal upload length {} exceeds allocated buffer length {capacity}. Rebuild the BindingPlan before dispatch.",
                bytes.len()
            ),
        });
    }
    // SAFETY: the buffer was allocated by this backend with StorageModeShared,
    // `capacity` was read from Metal, and the copy length is checked not to
    // exceed the allocation.
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), buffer.contents().cast::<u8>(), bytes.len());
    }
    buffer.did_modify_range(metal::NSRange::new(
        0,
        checked_ns_uint(bytes.len(), "Metal upload modified range")?,
    ));
    Ok(())
}

fn collect_outputs(
    buffers: &[PlannedBuffer],
    output_by_binding: &BTreeMap<u32, OutputBindingLayout>,
) -> Result<Vec<Vec<u8>>, BackendError> {
    let mut outputs = Vec::new();
    outputs
        .try_reserve(output_by_binding.len())
        .map_err(|error| BackendError::InvalidProgram {
            fix: format!(
                "Fix: Metal output collection could not reserve {} output slot(s): {error}. Split the Program outputs.",
                output_by_binding.len()
            ),
        })?;
    for (binding, layout) in output_by_binding {
        let buffer = buffers
            .iter()
            .find(|planned| planned.binding == *binding)
            .ok_or_else(|| BackendError::InvalidProgram {
                fix: format!(
                    "Fix: Metal output binding {binding} (`{}`) has no allocated buffer.",
                    layout.name
                ),
            })?;
        let allocation_len = allocation_len_for_output(layout)?;
        let copy_start = layout.layout.copy_offset;
        let copy_end = copy_start
            .checked_add(layout.layout.copy_size)
            .ok_or_else(|| BackendError::InvalidProgram {
                fix: format!(
                    "Fix: Metal output binding {binding} copy range overflows usize. Narrow output_byte_range."
                ),
            })?;
        if copy_end > allocation_len {
            return Err(BackendError::InvalidProgram {
                fix: format!(
                    "Fix: Metal output binding {binding} copy range {copy_start}..{copy_end} exceeds allocation length {allocation_len}."
                ),
            });
        }
        // SAFETY: the source pointer belongs to a live Metal shared buffer,
        // `allocation_len` is the host allocation length used for the buffer,
        // and range checks above prove the slice window is in bounds.
        let source = unsafe {
            std::slice::from_raw_parts(buffer.buffer.contents().cast::<u8>(), allocation_len)
        };
        let copied = &source[copy_start..copy_end];
        let trim_start = layout.layout.trim_start;
        let trim_end = trim_start
            .checked_add(layout.layout.read_size)
            .ok_or_else(|| BackendError::InvalidProgram {
                fix: format!(
                    "Fix: Metal output binding {binding} trim range overflows usize. Narrow output_byte_range."
                ),
            })?;
        if trim_end > copied.len() {
            return Err(BackendError::InvalidProgram {
                fix: format!(
                    "Fix: Metal output binding {binding} trim range {trim_start}..{trim_end} exceeds copied length {}.",
                    copied.len()
                ),
            });
        }
        outputs.push(copied[trim_start..trim_end].to_vec());
    }
    Ok(outputs)
}

fn new_buffer_sizes_buffer(
    device: &Device,
    slot: u8,
    bindings: &[vyre_emit_metal::MetalBindingMetadata],
    buffers: &[PlannedBuffer],
) -> Result<(u8, Buffer), BackendError> {
    let sidecar_words = bindings
        .iter()
        .map(|binding| usize::from(binding.metal_buffer_index) + 1)
        .max()
        .unwrap_or(1);
    let mut sizes = vec![0u32; sidecar_words];
    sizes.try_reserve(0).map_err(|error| {
        BackendError::InvalidProgram {
            fix: format!(
                "Fix: Metal buffer-size sidecar could not reserve {sidecar_words} length word(s): {error}. Split the Program bindings.",
            ),
        }
    })?;
    for binding in bindings {
        let planned = buffers
            .iter()
            .find(|planned| planned.binding == binding.slot)
            .ok_or_else(|| BackendError::InvalidProgram {
                fix: format!(
                    "Fix: Metal buffer-size sidecar could not find planned binding {} (`{}`). Rebuild BindingPlan before dispatch.",
                    binding.slot, binding.name
                ),
            })?;
        let byte_len =
            u32::try_from(planned.buffer.length()).map_err(|error| BackendError::InvalidProgram {
                fix: format!(
                    "Fix: Metal binding {} (`{}`) length {} cannot fit the u32 _buffer_sizes ABI: {error}. Split the dispatch buffer.",
                    binding.slot,
                    binding.name,
                    planned.buffer.length()
                ),
            })?;
        let Some(size_slot) = sizes.get_mut(usize::from(binding.metal_buffer_index)) else {
            return Err(BackendError::InvalidProgram {
                fix: format!(
                    "Fix: Metal buffer-size sidecar index {} for binding {} (`{}`) exceeds the packed sidecar word count {sidecar_words}. Rebuild the Metal artifact binding map.",
                    binding.metal_buffer_index, binding.slot, binding.name
                ),
            });
        };
        *size_slot = byte_len;
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve(sizes.len() * std::mem::size_of::<u32>())
        .map_err(|error| BackendError::InvalidProgram {
            fix: format!(
                "Fix: Metal buffer-size sidecar byte packing could not reserve {} byte(s): {error}. Split the Program bindings.",
                sizes.len() * std::mem::size_of::<u32>()
            ),
        })?;
    for size in sizes {
        bytes.extend_from_slice(&size.to_le_bytes());
    }
    Ok((slot, new_host_input_buffer(device, &bytes)?))
}

fn allocation_len_for_output(layout: &OutputBindingLayout) -> Result<usize, BackendError> {
    layout
        .layout
        .copy_offset
        .checked_add(layout.layout.copy_size)
        .map(|required| required.max(layout.layout.full_size).max(4))
        .ok_or_else(|| BackendError::InvalidProgram {
            fix: format!(
                "Fix: output layout for `{}` overflows allocation length. Narrow output_byte_range.",
                layout.name
            ),
        })
}

fn metal_threadgroup_size(workgroup_size: [u32; 3]) -> Result<MTLSize, BackendError> {
    Ok(MTLSize::new(
        checked_nonzero_dimension(workgroup_size[0], "workgroup x")?,
        checked_nonzero_dimension(workgroup_size[1], "workgroup y")?,
        checked_nonzero_dimension(workgroup_size[2], "workgroup z")?,
    ))
}

fn metal_grid_size(workgroups: [u32; 3]) -> Result<MTLSize, BackendError> {
    Ok(MTLSize::new(
        checked_nonzero_dimension(workgroups[0], "workgroups x")?,
        checked_nonzero_dimension(workgroups[1], "workgroups y")?,
        checked_nonzero_dimension(workgroups[2], "workgroups z")?,
    ))
}

fn checked_nonzero_dimension(value: u32, field: &'static str) -> Result<NSUInteger, BackendError> {
    if value == 0 {
        return Err(BackendError::InvalidProgram {
            fix: format!("Fix: Metal dispatch {field} dimension must be nonzero."),
        });
    }
    Ok(value.into())
}

fn checked_ns_uint(value: usize, field: &'static str) -> Result<NSUInteger, BackendError> {
    value.try_into().map_err(|error| BackendError::InvalidProgram {
        fix: format!(
            "Fix: {field} value {value} cannot fit Metal NSUInteger: {error}. Split the dispatch buffer."
        ),
    })
}

fn ns_uint_to_u32_saturating(value: NSUInteger) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

fn bytes_per_second_to_gbps(value: u64) -> u32 {
    let gbps = value / 1_000_000_000;
    u32::try_from(gbps).unwrap_or(u32::MAX)
}

fn elapsed_ns(started: Instant, field: &'static str) -> Result<u64, BackendError> {
    u64::try_from(started.elapsed().as_nanos()).map_err(|error| BackendError::InvalidProgram {
        fix: format!(
            "Fix: {field} timing cannot fit u64 nanoseconds: {error}. Split telemetry windows."
        ),
    })
}
