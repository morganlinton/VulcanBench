use super::{
    nanos_u64, nested_output_bytes, nested_output_count_u32, output_bytes, output_count_u32,
    reserve_output_shell, resident_handle_count_u32, resident_row_count_u32, Megakernel,
    MegakernelBatchDispatchOutput, MegakernelDispatchOutput, MegakernelDispatchStats,
    MegakernelResidentBatchScratch, MegakernelResidentHandles,
};
use crate::PipelineError;
use smallvec::SmallVec;
use std::time::Instant;
use vyre_driver::backend::{OutputBuffers, Resource};

impl Megakernel {
    /// Dispatch using backend-resident handles for all megakernel ABI buffers.
    ///
    /// This path never falls back to host byte buffers. If the compiled backend
    /// pipeline does not implement resident handles, the backend's structured
    /// unsupported-feature error is returned.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError`] when the backend rejects persistent handles,
    /// dispatch fails, or device-loss recovery cannot rebuild the pipeline.
    pub fn dispatch_persistent_handles(
        &self,
        handles: MegakernelResidentHandles,
    ) -> Result<Vec<Vec<u8>>, PipelineError> {
        Ok(self.dispatch_persistent_handles_observed(handles)?.buffers)
    }

    /// Dispatch using backend-resident handles and return instrumentation.
    ///
    /// # Errors
    ///
    /// See [`Megakernel::dispatch_persistent_handles`].
    pub fn dispatch_persistent_handles_observed(
        &self,
        handles: MegakernelResidentHandles,
    ) -> Result<MegakernelDispatchOutput, PipelineError> {
        let mut buffers = Vec::new();
        reserve_output_shell(
            &mut buffers,
            MegakernelResidentHandles::ABI_RESOURCE_COUNT,
            "persistent-handle output slots",
        )?;
        let stats = self.dispatch_persistent_handles_into(handles, &mut buffers)?;
        Ok(MegakernelDispatchOutput { buffers, stats })
    }

    /// Dispatch using backend-resident handles into caller-owned output storage.
    ///
    /// This keeps the persistent ABI buffers resident and lets callers retain
    /// host readback allocation across repeated megakernel launches.
    ///
    /// # Errors
    ///
    /// See [`Megakernel::dispatch_persistent_handles`].
    pub fn dispatch_persistent_handles_into(
        &self,
        handles: MegakernelResidentHandles,
        outputs: &mut OutputBuffers,
    ) -> Result<MegakernelDispatchStats, PipelineError> {
        if self.has_grid_sync && !self.backend.supports_grid_sync() {
            return Err(PipelineError::Backend(
                "persistent-handle dispatch cannot split GridSync barriers without backend-resident segment threading. Fix: use a backend with native grid sync or dispatch borrowed buffers through the grid-sync splitter."
                    .to_string(),
            ));
        }
        let resources = handles.resources();
        let config = self.launch_geometry().dispatch_config(None);
        let started = Instant::now();
        let mut recovered = false;
        match self.dispatch_persistent_handles_once_into(&resources, &config, outputs) {
            Ok(()) => {}
            Err(error) if self.recovery_policy.allows_retry(&error) => {
                self.recover_after_device_loss()?;
                recovered = true;
                self.dispatch_persistent_handles_once_into(&resources, &config, outputs)?
            }
            Err(error) => return Err(error.into()),
        }
        let latency_ns = nanos_u64(started.elapsed().as_nanos())?;
        let output_bytes = output_bytes(outputs)?;
        let output_buffers = output_count_u32(outputs)?;
        Ok(MegakernelDispatchStats {
            input_bytes: 0,
            output_bytes,
            readback_bytes: output_bytes,
            bytes_moved: output_bytes,
            device_allocation_bytes: 0,
            device_allocation_events: 0,
            latency_ns,
            output_buffers,
            resident_resource_rows: 1,
            resident_resource_handles: resident_handle_count_u32(1)?,
            kernel_launches: if recovered { 2 } else { 1 },
            sync_points: 1,
            recovered_after_device_loss: recovered,
        })
    }

    /// Dispatch several resident megakernel submissions through the compiled
    /// backend batch contract.
    ///
    /// This is the many-small-launch path: callers keep every ABI buffer
    /// resident, then submit a slice of handle tuples so native backends can
    /// record one command buffer or replay one graph batch instead of paying a
    /// host submission per item.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError`] when the backend rejects persistent handles,
    /// any item fails, or device-loss recovery cannot rebuild the pipeline.
    pub fn dispatch_persistent_handles_many_observed(
        &self,
        handles: &[MegakernelResidentHandles],
    ) -> Result<MegakernelBatchDispatchOutput, PipelineError> {
        let mut batches = Vec::new();
        reserve_output_shell(&mut batches, handles.len(), "persistent-handle batch rows")?;
        let stats = self.dispatch_persistent_handles_many_into(handles, &mut batches)?;
        Ok(MegakernelBatchDispatchOutput { batches, stats })
    }

    /// Dispatch several resident megakernel submissions into caller-owned
    /// nested output storage.
    ///
    /// Existing batch rows and output slots are reused when the backend returns
    /// the same shape, avoiding nested result-vector churn in many-small-launch
    /// hot paths.
    ///
    /// # Errors
    ///
    /// See [`Megakernel::dispatch_persistent_handles_many_observed`].
    pub fn dispatch_persistent_handles_many_into(
        &self,
        handles: &[MegakernelResidentHandles],
        batches: &mut Vec<OutputBuffers>,
    ) -> Result<MegakernelDispatchStats, PipelineError> {
        if handles.is_empty() {
            batches.clear();
            return Ok(MegakernelDispatchStats {
                input_bytes: 0,
                output_bytes: 0,
                readback_bytes: 0,
                bytes_moved: 0,
                device_allocation_bytes: 0,
                device_allocation_events: 0,
                latency_ns: 0,
                output_buffers: 0,
                resident_resource_rows: 0,
                resident_resource_handles: 0,
                kernel_launches: 0,
                sync_points: 0,
                recovered_after_device_loss: false,
            });
        }
        if self.has_grid_sync && !self.backend.supports_grid_sync() {
            return Err(PipelineError::Backend(
                "batched persistent-handle dispatch cannot split GridSync barriers without backend-resident segment threading. Fix: use a backend with native grid sync or dispatch borrowed buffers through the grid-sync splitter."
                    .to_string(),
            ));
        }

        let mut resources: SmallVec<[[Resource; 4]; 16]> = SmallVec::new();
        reserve_resource_rows_small(&mut resources, handles.len())?;
        resources.extend(handles.iter().map(|handles| handles.resources()));
        let config = self.launch_geometry().dispatch_config(None);
        let started = Instant::now();
        let mut recovered = false;
        match self.dispatch_persistent_handle_rows_once_into(&resources, &config, batches) {
            Ok(()) => {}
            Err(error) if self.recovery_policy.allows_retry(&error) => {
                self.recover_after_device_loss()?;
                recovered = true;
                self.dispatch_persistent_handle_rows_once_into(&resources, &config, batches)?
            }
            Err(error) => return Err(error.into()),
        }
        let latency_ns = nanos_u64(started.elapsed().as_nanos())?;
        let output_bytes = nested_output_bytes(batches)?;
        let output_buffers = nested_output_count_u32(batches)?;
        let resident_resource_rows = resident_row_count_u32(handles.len())?;
        let resident_resource_handles = resident_handle_count_u32(handles.len())?;
        Ok(MegakernelDispatchStats {
            input_bytes: 0,
            output_bytes,
            readback_bytes: output_bytes,
            bytes_moved: output_bytes,
            device_allocation_bytes: 0,
            device_allocation_events: 0,
            latency_ns,
            output_buffers,
            resident_resource_rows,
            resident_resource_handles,
            kernel_launches: if recovered { 2 } else { 1 },
            sync_points: 1,
            recovered_after_device_loss: recovered,
        })
    }

    /// Dispatch several resident megakernel submissions through reusable
    /// resident-batch scratch.
    ///
    /// This is the allocation-stable many-small-launch path: resource rows and
    /// returned output batches stay owned by `scratch` across calls.
    ///
    /// # Errors
    ///
    /// See [`Megakernel::dispatch_persistent_handles_many_observed`].
    pub fn dispatch_persistent_handles_many_with_scratch(
        &self,
        handles: &[MegakernelResidentHandles],
        scratch: &mut MegakernelResidentBatchScratch,
    ) -> Result<MegakernelDispatchStats, PipelineError> {
        if handles.is_empty() {
            scratch.clear();
            return Ok(MegakernelDispatchStats {
                input_bytes: 0,
                output_bytes: 0,
                readback_bytes: 0,
                bytes_moved: 0,
                device_allocation_bytes: 0,
                device_allocation_events: 0,
                latency_ns: 0,
                output_buffers: 0,
                resident_resource_rows: 0,
                resident_resource_handles: 0,
                kernel_launches: 0,
                sync_points: 0,
                recovered_after_device_loss: false,
            });
        }
        if self.has_grid_sync && !self.backend.supports_grid_sync() {
            return Err(PipelineError::Backend(
                "batched persistent-handle dispatch cannot split GridSync barriers without backend-resident segment threading. Fix: use a backend with native grid sync or dispatch borrowed buffers through the grid-sync splitter."
                    .to_string(),
            ));
        }

        prepare_resource_rows_into(handles, &mut scratch.resources)?;
        scratch.active_batches = 0;
        let config = self.launch_geometry().dispatch_config(None);
        let started = Instant::now();
        let mut recovered = false;
        match self.dispatch_persistent_handle_rows_once_into(
            &scratch.resources,
            &config,
            &mut scratch.batches,
        ) {
            Ok(()) => {}
            Err(error) if self.recovery_policy.allows_retry(&error) => {
                self.recover_after_device_loss()?;
                recovered = true;
                self.dispatch_persistent_handle_rows_once_into(
                    &scratch.resources,
                    &config,
                    &mut scratch.batches,
                )?
            }
            Err(error) => return Err(error.into()),
        }
        scratch.active_batches = handles.len();
        let latency_ns = nanos_u64(started.elapsed().as_nanos())?;
        let output_bytes = nested_output_bytes(&scratch.batches)?;
        let output_buffers = nested_output_count_u32(&scratch.batches)?;
        let resident_resource_rows = resident_row_count_u32(handles.len())?;
        let resident_resource_handles = resident_handle_count_u32(handles.len())?;
        Ok(MegakernelDispatchStats {
            input_bytes: 0,
            output_bytes,
            readback_bytes: output_bytes,
            bytes_moved: output_bytes,
            device_allocation_bytes: 0,
            device_allocation_events: 0,
            latency_ns,
            output_buffers,
            resident_resource_rows,
            resident_resource_handles,
            kernel_launches: if recovered { 2 } else { 1 },
            sync_points: 1,
            recovered_after_device_loss: recovered,
        })
    }
}

fn prepare_resource_rows_into(
    handles: &[MegakernelResidentHandles],
    resources: &mut Vec<[Resource; 4]>,
) -> Result<(), PipelineError> {
    resources.clear();
    reserve_resource_rows(resources, handles.len())?;
    resources.extend(handles.iter().map(|handles| handles.resources()));
    Ok(())
}

fn reserve_resource_rows(
    rows: &mut Vec<[Resource; 4]>,
    capacity: usize,
) -> Result<(), PipelineError> {
    vyre_foundation::allocation::try_reserve_vec_to_capacity(rows, capacity).map_err(|error| {
        PipelineError::Backend(format!(
            "megakernel resident resource-row reservation failed for {capacity} row(s): {error}. Fix: split persistent-handle dispatch batches before launch."
        ))
    })
}

fn reserve_resource_rows_small(
    rows: &mut SmallVec<[[Resource; 4]; 16]>,
    capacity: usize,
) -> Result<(), PipelineError> {
    vyre_foundation::allocation::try_reserve_smallvec_to_capacity(rows, capacity).map_err(
        |error| {
            PipelineError::Backend(format!(
                "megakernel resident inline resource-row reservation failed for {capacity} row(s): {error}. Fix: split persistent-handle dispatch batches before launch."
            ))
        },
    )
}
