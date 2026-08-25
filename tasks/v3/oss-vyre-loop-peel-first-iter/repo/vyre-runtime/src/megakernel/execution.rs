//! Compiled persistent-megakernel handle and dispatch path.

mod persistent_handles;
mod readback_dispatch;
mod types;

use super::builder::{build_program_jit_slots, build_program_sharded_slots_shared};
use super::handlers::OpcodeHandler;
use super::io;
use super::planner::MegakernelLaunchGeometry;
use super::protocol;
use super::protocol_api::{validate_control_bytes, validate_debug_log_bytes};
use super::recovery::{
    backend_error_indicates_device_loss, recover_compiled_pipeline, MegakernelRecoveryDecision,
    MegakernelRecoveryPolicy,
};
use super::staging_reserve::reserve_vec_capacity;
use crate::PipelineError;
use arc_swap::ArcSwap;
use std::sync::Arc;
use std::time::Instant;
use vyre_driver::backend::{
    CompiledPipeline, DispatchConfig, OutputBuffers, Resource, VyreBackend,
};
use vyre_foundation::ir::Program;

pub use types::{
    MegakernelBatchDispatchOutput, MegakernelDispatchOutput, MegakernelDispatchStats,
    MegakernelResidentBatchScratch, MegakernelResidentHandles,
};

/// Orchestrated persistent-megakernel handle.
///
/// Construct with [`Megakernel::bootstrap`] (default 256 lanes x 1
/// workgroup) or [`Megakernel::bootstrap_sharded`] for multi-tenant fan-in.
/// Feed bytecode with [`Megakernel::dispatch`].
pub struct Megakernel {
    backend: Arc<dyn VyreBackend>,
    pipeline: ArcSwap<PipelineSlot>,
    /// Pipeline identity string, updated atomically on every `recover_after_device_loss`.
    pipeline_id: ArcSwap<String>,
    program: Arc<Program>,
    has_grid_sync: bool,
    empty_io_queue_bytes: Arc<[u8]>,
    slot_count: u32,
    workgroup_size_x: u32,
    recovery_policy: MegakernelRecoveryPolicy,
}

struct PipelineSlot {
    inner: Arc<dyn CompiledPipeline>,
}

impl Megakernel {
    /// Default bootstrap: 256 lanes x 1 workgroup, no custom opcodes.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError::Backend`] if the backend rejects the program.
    pub fn bootstrap(backend: Arc<dyn VyreBackend>) -> Result<Self, PipelineError> {
        Self::bootstrap_sharded(backend, 256, 256, Vec::new())
    }

    /// Bootstrap with custom opcodes but default sharding.
    ///
    /// # Errors
    ///
    /// See [`Megakernel::bootstrap`].
    pub fn bootstrap_with_opcodes(
        backend: Arc<dyn VyreBackend>,
        opcodes: Vec<OpcodeHandler>,
    ) -> Result<Self, PipelineError> {
        Self::bootstrap_sharded(backend, 256, 256, opcodes)
    }

    /// Compute worker groups for a megakernel slot geometry without compiling.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError::QueueFull`] when the geometry cannot map slots
    /// to whole workgroups.
    pub fn worker_groups_for_geometry(
        slot_count: u32,
        workgroup_size_x: u32,
    ) -> Result<u32, PipelineError> {
        validate_bootstrap_geometry(slot_count, workgroup_size_x)?;
        Ok(slot_count / workgroup_size_x)
    }

    /// Full bootstrap with sharding and custom opcodes.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError::QueueFull`] when geometry is invalid or
    /// [`PipelineError::Backend`] from the underlying compile.
    pub fn bootstrap_sharded(
        backend: Arc<dyn VyreBackend>,
        slot_count: u32,
        workgroup_size_x: u32,
        opcodes: Vec<OpcodeHandler>,
    ) -> Result<Self, PipelineError> {
        validate_bootstrap_geometry(slot_count, workgroup_size_x)?;
        let program = build_program_sharded_slots_shared(workgroup_size_x, slot_count, &opcodes);
        Self::compile_bootstrap_shared(backend, slot_count, workgroup_size_x, program)
    }

    /// JIT compiler bootstrap for fused payload processors.
    ///
    /// # Errors
    ///
    /// See [`Megakernel::bootstrap_sharded`].
    pub fn bootstrap_jit(
        backend: Arc<dyn VyreBackend>,
        slot_count: u32,
        workgroup_size_x: u32,
        payload_processor: &[vyre_foundation::ir::Node],
    ) -> Result<Self, PipelineError> {
        validate_bootstrap_geometry(slot_count, workgroup_size_x)?;
        let program = build_program_jit_slots(workgroup_size_x, slot_count, payload_processor);
        Self::compile_bootstrap(backend, slot_count, workgroup_size_x, program)
    }

    fn compile_bootstrap(
        backend: Arc<dyn VyreBackend>,
        slot_count: u32,
        workgroup_size_x: u32,
        program: Program,
    ) -> Result<Self, PipelineError> {
        Self::compile_bootstrap_shared(backend, slot_count, workgroup_size_x, Arc::new(program))
    }

    fn compile_bootstrap_shared(
        backend: Arc<dyn VyreBackend>,
        slot_count: u32,
        workgroup_size_x: u32,
        program: Arc<Program>,
    ) -> Result<Self, PipelineError> {
        validate_bootstrap_geometry(slot_count, workgroup_size_x)?;
        let config = DispatchConfig::default();
        let pipeline = vyre_driver::pipeline::compile_shared(
            Arc::clone(&backend),
            Arc::clone(&program),
            &config,
        )?;
        let pipeline_id = pipeline.id().to_string();
        let has_grid_sync = vyre_driver::grid_sync::contains_grid_sync(&program);
        let empty_io_queue_bytes =
            Arc::<[u8]>::from(io::try_encode_empty_io_queue(io::IO_SLOT_COUNT)?.into_boxed_slice());
        Ok(Self {
            backend,
            pipeline: ArcSwap::from(Arc::new(PipelineSlot { inner: pipeline })),
            pipeline_id: ArcSwap::from(Arc::new(pipeline_id)),
            program,
            has_grid_sync,
            empty_io_queue_bytes,
            slot_count,
            workgroup_size_x,
            recovery_policy: MegakernelRecoveryPolicy::default(),
        })
    }

    /// Dispatch a full storage buffer set with an empty IO queue.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError`] when protocol buffers are malformed, dispatch
    /// fails, or device-loss recovery cannot rebuild the compiled pipeline.
    pub fn dispatch(
        &self,
        control_bytes: Vec<u8>,
        ring_bytes: Vec<u8>,
        debug_log_bytes: Vec<u8>,
    ) -> Result<Vec<Vec<u8>>, PipelineError> {
        self.dispatch_borrowed(&control_bytes, &ring_bytes, &debug_log_bytes)
    }

    /// Dispatch a borrowed storage buffer set with an empty IO queue.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError`] when protocol buffers are malformed, dispatch
    /// fails, or device-loss recovery cannot rebuild the compiled pipeline.
    pub fn dispatch_borrowed(
        &self,
        control_bytes: &[u8],
        ring_bytes: &[u8],
        debug_log_bytes: &[u8],
    ) -> Result<Vec<Vec<u8>>, PipelineError> {
        Ok(self
            .dispatch_borrowed_observed(control_bytes, ring_bytes, debug_log_bytes)?
            .buffers)
    }

    /// Dispatch a full storage buffer set and return runtime instrumentation.
    ///
    /// # Errors
    ///
    /// See [`Megakernel::dispatch`].
    pub fn dispatch_observed(
        &self,
        control_bytes: Vec<u8>,
        ring_bytes: Vec<u8>,
        debug_log_bytes: Vec<u8>,
    ) -> Result<MegakernelDispatchOutput, PipelineError> {
        self.dispatch_with_io_queue_borrowed_observed(
            &control_bytes,
            &ring_bytes,
            &debug_log_bytes,
            &self.empty_io_queue_bytes,
        )
    }

    /// Dispatch borrowed buffers with an empty IO queue and return runtime
    /// instrumentation.
    ///
    /// # Errors
    ///
    /// See [`Megakernel::dispatch_borrowed`].
    pub fn dispatch_borrowed_observed(
        &self,
        control_bytes: &[u8],
        ring_bytes: &[u8],
        debug_log_bytes: &[u8],
    ) -> Result<MegakernelDispatchOutput, PipelineError> {
        self.dispatch_with_io_queue_borrowed_observed(
            control_bytes,
            ring_bytes,
            debug_log_bytes,
            &self.empty_io_queue_bytes,
        )
    }

    /// Dispatch a full storage buffer set with a caller-supplied `io_queue`.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError`] when any protocol buffer is malformed, backend
    /// dispatch fails, or device-loss recovery cannot rebuild the pipeline.
    pub fn dispatch_with_io_queue(
        &self,
        control_bytes: Vec<u8>,
        ring_bytes: Vec<u8>,
        debug_log_bytes: Vec<u8>,
        io_queue_bytes: Vec<u8>,
    ) -> Result<Vec<Vec<u8>>, PipelineError> {
        self.dispatch_with_io_queue_borrowed(
            &control_bytes,
            &ring_bytes,
            &debug_log_bytes,
            &io_queue_bytes,
        )
    }

    /// Dispatch borrowed buffers with a caller-supplied `io_queue`.
    ///
    /// # Errors
    ///
    /// See [`Megakernel::dispatch_with_io_queue`].
    pub fn dispatch_with_io_queue_borrowed(
        &self,
        control_bytes: &[u8],
        ring_bytes: &[u8],
        debug_log_bytes: &[u8],
        io_queue_bytes: &[u8],
    ) -> Result<Vec<Vec<u8>>, PipelineError> {
        Ok(self
            .dispatch_with_io_queue_borrowed_observed(
                control_bytes,
                ring_bytes,
                debug_log_bytes,
                io_queue_bytes,
            )?
            .buffers)
    }

    /// Dispatch with a caller-supplied `io_queue` and return instrumentation.
    ///
    /// # Errors
    ///
    /// See [`Megakernel::dispatch_with_io_queue`].
    pub fn dispatch_with_io_queue_observed(
        &self,
        control_bytes: Vec<u8>,
        ring_bytes: Vec<u8>,
        debug_log_bytes: Vec<u8>,
        io_queue_bytes: Vec<u8>,
    ) -> Result<MegakernelDispatchOutput, PipelineError> {
        self.dispatch_with_io_queue_borrowed_observed(
            &control_bytes,
            &ring_bytes,
            &debug_log_bytes,
            &io_queue_bytes,
        )
    }

    /// Dispatch borrowed buffers with a caller-supplied `io_queue` and return
    /// instrumentation.
    ///
    /// # Errors
    ///
    /// See [`Megakernel::dispatch_with_io_queue`].
    pub fn dispatch_with_io_queue_borrowed_observed(
        &self,
        control_bytes: &[u8],
        ring_bytes: &[u8],
        debug_log_bytes: &[u8],
        io_queue_bytes: &[u8],
    ) -> Result<MegakernelDispatchOutput, PipelineError> {
        let mut buffers = Vec::new();
        reserve_output_shell(
            &mut buffers,
            MegakernelResidentHandles::ABI_RESOURCE_COUNT,
            "borrowed megakernel output shell",
        )?;
        let stats = self.dispatch_with_io_queue_borrowed_into(
            control_bytes,
            ring_bytes,
            debug_log_bytes,
            io_queue_bytes,
            &mut buffers,
        )?;
        Ok(MegakernelDispatchOutput { buffers, stats })
    }

    /// Dispatch borrowed buffers with a caller-supplied IO queue, writing
    /// backend outputs into caller-owned storage.
    ///
    /// # Errors
    ///
    /// See [`Megakernel::dispatch_with_io_queue_borrowed`].
    pub fn dispatch_with_io_queue_borrowed_into(
        &self,
        control_bytes: &[u8],
        ring_bytes: &[u8],
        debug_log_bytes: &[u8],
        io_queue_bytes: &[u8],
        outputs: &mut OutputBuffers,
    ) -> Result<MegakernelDispatchStats, PipelineError> {
        validate_control_bytes(control_bytes)?;
        validate_debug_log_bytes(debug_log_bytes)?;
        io::validate_io_queue_bytes(io_queue_bytes)?;
        self.validate_ring_bytes(ring_bytes)?;

        let input_bytes = total_len([control_bytes, ring_bytes, debug_log_bytes, io_queue_bytes])?;
        let inputs = [control_bytes, ring_bytes, debug_log_bytes, io_queue_bytes];
        let config = self.launch_geometry().dispatch_config(None);
        let started = Instant::now();
        let mut recovered = false;
        match self.dispatch_once_into(&inputs, &config, outputs) {
            Ok(()) => {}
            Err(error) if self.recovery_policy.allows_retry(&error) => {
                self.recover_after_device_loss()?;
                recovered = true;
                self.dispatch_once_into(&inputs, &config, outputs)?
            }
            Err(error) => return Err(error.into()),
        }
        let latency_ns = nanos_u64(started.elapsed().as_nanos())?;
        let output_bytes = output_bytes(outputs)?;
        let readback_bytes = output_bytes;
        let bytes_moved = checked_add_u64(input_bytes, readback_bytes, "megakernel bytes moved")?;
        let device_allocation_bytes = checked_add_u64(
            input_bytes,
            output_bytes,
            "megakernel host-visible device allocation bytes",
        )?;
        let output_buffers = count_u32(outputs.len(), "megakernel output buffer count")?;
        let device_allocation_events =
            checked_add_u32(4, output_buffers, "megakernel allocation event count")?;
        Ok(MegakernelDispatchStats {
            input_bytes,
            output_bytes,
            readback_bytes,
            bytes_moved,
            device_allocation_bytes,
            device_allocation_events,
            latency_ns,
            output_buffers,
            resident_resource_rows: 0,
            resident_resource_handles: 0,
            kernel_launches: if recovered { 2 } else { 1 },
            sync_points: 1,
            recovered_after_device_loss: recovered,
        })
    }

    /// Rebuild the compiled pipeline after device-loss symptoms.
    ///
    /// This does not mask the failure: if recompilation fails, the structured
    /// backend error is returned with the original remediation text intact.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError::Backend`] when the backend cannot recompile.
    pub fn recover_after_device_loss(&self) -> Result<MegakernelRecoveryDecision, PipelineError> {
        let config = self.launch_geometry().dispatch_config(None);
        let rebuilt = recover_compiled_pipeline(&self.backend, Arc::clone(&self.program), &config)?;
        let new_id = rebuilt.id().to_string();
        self.pipeline
            .store(Arc::new(PipelineSlot { inner: rebuilt }));
        self.pipeline_id.store(Arc::new(new_id));
        Ok(MegakernelRecoveryDecision::RecompiledPipeline)
    }

    /// Pipeline id from the backend.
    ///
    /// Returns the id of the currently loaded compiled pipeline. After a call
    /// to [`Megakernel::recover_after_device_loss`] this reflects the rebuilt
    /// pipeline's id rather than the original one.
    #[must_use]
    pub fn pipeline_id(&self) -> String {
        (**self.pipeline_id.load()).clone()
    }

    /// Slot count this kernel was sharded for.
    #[must_use]
    pub const fn slot_count(&self) -> u32 {
        self.slot_count
    }

    /// Workgroup size this kernel was compiled for.
    #[must_use]
    pub const fn workgroup_size_x(&self) -> u32 {
        self.workgroup_size_x
    }

    /// Workgroup count needed to cover every ring slot.
    #[must_use]
    pub fn worker_groups(&self) -> u32 {
        self.slot_count / self.workgroup_size_x
    }

    pub(super) fn validate_ring_bytes(&self, ring_bytes: &[u8]) -> Result<(), PipelineError> {
        let expected_ring_bytes = protocol::ring_byte_len(self.slot_count).ok_or_else(|| {
            PipelineError::Backend(
                "megakernel ring byte length overflowed usize. Fix: split the ring into smaller dispatch shards."
                    .to_string(),
            )
        })?;
        if ring_bytes.len() != expected_ring_bytes {
            return Err(PipelineError::Backend(format!(
                "megakernel ring buffer has {} bytes, expected {expected_ring_bytes} for {} slots. Fix: build ring bytes with Megakernel::encode_empty_ring(slot_count) for this handle.",
                ring_bytes.len(),
                self.slot_count
            )));
        }
        Ok(())
    }

    pub(super) fn launch_geometry(&self) -> MegakernelLaunchGeometry {
        MegakernelLaunchGeometry {
            workgroup_size_x: self.workgroup_size_x,
            slot_count: self.slot_count,
            dispatch_grid: [self.slot_count / self.workgroup_size_x, 1, 1],
        }
    }

    fn dispatch_once_into(
        &self,
        inputs: &[&[u8]; 4],
        config: &DispatchConfig,
        outputs: &mut OutputBuffers,
    ) -> Result<(), vyre_driver::BackendError> {
        if self.has_grid_sync && !self.backend.supports_grid_sync() {
            return vyre_driver::grid_sync::dispatch_with_grid_sync_split_into(
                self.backend.as_ref(),
                &self.program,
                inputs,
                config,
                outputs,
            );
        }
        let pipeline = self.pipeline.load();
        pipeline
            .inner
            .dispatch_borrowed_into(inputs, config, outputs)
    }

    fn dispatch_persistent_handles_once_into(
        &self,
        inputs: &[Resource; 4],
        config: &DispatchConfig,
        outputs: &mut OutputBuffers,
    ) -> Result<(), vyre_driver::BackendError> {
        let pipeline = self.pipeline.load();
        pipeline
            .inner
            .dispatch_persistent_handles_into(inputs, config, outputs)
    }

    fn dispatch_persistent_handle_rows_once_into(
        &self,
        rows: &[[Resource; 4]],
        config: &DispatchConfig,
        outputs: &mut Vec<OutputBuffers>,
    ) -> Result<(), vyre_driver::BackendError> {
        let pipeline = self.pipeline.load();
        pipeline
            .inner
            .dispatch_persistent_handle_rows_into(rows, config, outputs)
    }
}

impl MegakernelRecoveryPolicy {
    fn allows_retry(self, error: &vyre_driver::BackendError) -> bool {
        self.retry_device_loss_once && backend_error_indicates_device_loss(error)
    }
}

fn validate_bootstrap_geometry(
    slot_count: u32,
    workgroup_size_x: u32,
) -> Result<(), PipelineError> {
    if slot_count == 0 || workgroup_size_x == 0 || slot_count % workgroup_size_x != 0 {
        return Err(PipelineError::QueueFull {
            queue: "submission",
            fix: "slot_count must be a non-zero multiple of workgroup_size_x",
        });
    }
    Ok(())
}

pub(super) fn total_len<const N: usize>(buffers: [&[u8]; N]) -> Result<u64, PipelineError> {
    let mut total = 0u64;
    for buffer in buffers {
        total = checked_add_u64(
            total,
            usize_to_u64(buffer.len(), "megakernel input buffer length")?,
            "megakernel input byte total",
        )?;
    }
    Ok(total)
}

pub(super) fn output_bytes(outputs: &[Vec<u8>]) -> Result<u64, PipelineError> {
    let mut total = 0u64;
    for output in outputs {
        total = checked_add_u64(
            total,
            usize_to_u64(output.len(), "megakernel output buffer length")?,
            "megakernel output byte total",
        )?;
    }
    Ok(total)
}

pub(super) fn nested_output_bytes(outputs: &[Vec<Vec<u8>>]) -> Result<u64, PipelineError> {
    let mut total = 0u64;
    for row in outputs {
        total = checked_add_u64(
            total,
            output_bytes(row)?,
            "megakernel nested output byte total",
        )?;
    }
    Ok(total)
}

pub(super) fn output_count_u32(outputs: &[Vec<u8>]) -> Result<u32, PipelineError> {
    count_u32(outputs.len(), "megakernel output buffer count")
}

pub(super) fn nested_output_count_u32(outputs: &[Vec<Vec<u8>>]) -> Result<u32, PipelineError> {
    let mut total = 0usize;
    for row in outputs {
        total = total.checked_add(row.len()).ok_or_else(|| {
            PipelineError::Backend(
                "megakernel nested output buffer count overflowed usize. Fix: split resident rows before dispatch.".to_string(),
            )
        })?;
    }
    count_u32(total, "megakernel nested output buffer count")
}

pub(super) fn resident_row_count_u32(rows: usize) -> Result<u32, PipelineError> {
    count_u32(rows, "megakernel resident resource row count")
}

pub(super) fn resident_handle_count_u32(rows: usize) -> Result<u32, PipelineError> {
    let handles = rows
        .checked_mul(MegakernelResidentHandles::ABI_RESOURCE_COUNT)
        .ok_or_else(|| {
            PipelineError::Backend(
                "megakernel resident resource handle count overflowed usize. Fix: split resident rows before dispatch.".to_string(),
            )
        })?;
    count_u32(handles, "megakernel resident resource handle count")
}

pub(super) fn reserve_output_shell<T>(
    out: &mut Vec<T>,
    capacity: usize,
    label: &'static str,
) -> Result<(), PipelineError> {
    reserve_vec_capacity(out, capacity, label)
}

pub(super) fn nanos_u64(nanos: u128) -> Result<u64, PipelineError> {
    u64::try_from(nanos).map_err(|source| {
        PipelineError::Backend(format!(
            "megakernel latency cannot fit u64 nanoseconds: {source}. Fix: timeout or split the dispatch before telemetry overflows."
        ))
    })
}

fn usize_to_u64(value: usize, label: &str) -> Result<u64, PipelineError> {
    u64::try_from(value).map_err(|source| {
        PipelineError::Backend(format!(
            "{label} cannot fit u64: {source}. Fix: split the megakernel dispatch before telemetry/accounting."
        ))
    })
}

fn count_u32(value: usize, label: &str) -> Result<u32, PipelineError> {
    u32::try_from(value).map_err(|source| {
        PipelineError::Backend(format!(
            "{label} cannot fit u32: {source}. Fix: split the megakernel dispatch before telemetry/accounting."
        ))
    })
}

fn checked_add_u64(left: u64, right: u64, label: &str) -> Result<u64, PipelineError> {
    left.checked_add(right).ok_or_else(|| {
        PipelineError::Backend(format!(
            "{label} overflowed u64. Fix: split the megakernel dispatch before telemetry/accounting."
        ))
    })
}

fn checked_add_u32(left: u32, right: u32, label: &str) -> Result<u32, PipelineError> {
    left.checked_add(right).ok_or_else(|| {
        PipelineError::Backend(format!(
            "{label} overflowed u32. Fix: split the megakernel dispatch before telemetry/accounting."
        ))
    })
}

#[cfg(test)]
mod tests;
