//! CUDA dispatch path for borrowed host buffers.

use std::ffi::c_void;
use std::sync::Arc;

use cudarc::driver::sys::CUstream;
use smallvec::SmallVec;
use vyre_driver::accounting::checked_add_usize_lazy;
use vyre_driver::binding::{BindingPlan, BindingRole};
use vyre_driver::transfer_accounting::TransferAccountingPolicy;
use vyre_driver::{BackendError, DispatchConfig, OutputBuffers, PendingDispatch, VyreBackend};
use vyre_foundation::ir::Program;

use crate::numeric::CUDA_NUMERIC;
use crate::CUDA_BACKEND_ID;

use super::allocations::{DispatchAllocations, HostTransferAllocations};
use super::copy::aligned_async_copy_len;
use super::dispatch::CudaBackend;
use super::launch_params::launch_param_byte_len;
use super::module_cache::ModuleCacheKey;
use super::output_range::cuda_output_readback_for_binding;
use super::plan::CudaDispatchPlan;
use super::staging_reserve::{reserve_smallvec, reserved_vec};

#[derive(Clone, Copy)]
struct HostUpload {
    dst: u64,
    src: *const c_void,
    byte_len: usize,
}

#[derive(Clone, Copy)]
struct DeviceClear {
    dst: u64,
    byte_len: usize,
}

struct CudaReadyPending {
    outputs: Vec<Vec<u8>>,
}

const CUDA_HOST_TRANSFER_ACCOUNTING: TransferAccountingPolicy =
    TransferAccountingPolicy::new("CUDA", "split the dispatch into bounded chunks");

impl vyre_driver::backend::private::Sealed for CudaReadyPending {}

impl PendingDispatch for CudaReadyPending {
    fn is_ready(&self) -> bool {
        true
    }

    fn await_result(self: Box<Self>) -> Result<Vec<Vec<u8>>, BackendError> {
        Ok(self.outputs)
    }
}

struct GridSyncSplitCudaBackend<'a>(&'a CudaBackend);

impl vyre_driver::backend::private::Sealed for GridSyncSplitCudaBackend<'_> {}

impl VyreBackend for GridSyncSplitCudaBackend<'_> {
    fn id(&self) -> &'static str {
        CUDA_BACKEND_ID
    }

    fn dispatch(
        &self,
        program: &Program,
        inputs: &[Vec<u8>],
        config: &DispatchConfig,
    ) -> Result<Vec<Vec<u8>>, BackendError> {
        let mut borrowed_inputs = SmallVec::<[&[u8]; 8]>::new();
        reserve_smallvec(&mut borrowed_inputs, inputs.len(), "grid-sync CUDA input")?;
        borrowed_inputs.extend(inputs.iter().map(Vec::as_slice));
        self.0
            .dispatch_borrowed_async(program, &borrowed_inputs, config)?
            .await_result()
    }

    fn dispatch_borrowed(
        &self,
        program: &Program,
        inputs: &[&[u8]],
        config: &DispatchConfig,
    ) -> Result<Vec<Vec<u8>>, BackendError> {
        self.0
            .dispatch_borrowed_async(program, inputs, config)?
            .await_result()
    }

    fn dispatch_borrowed_into(
        &self,
        program: &Program,
        inputs: &[&[u8]],
        config: &DispatchConfig,
        outputs: &mut OutputBuffers,
    ) -> Result<(), BackendError> {
        self.0
            .dispatch_borrowed_async(program, inputs, config)?
            .await_result_into(outputs)
    }

    fn supports_grid_sync(&self) -> bool {
        self.0.supports_grid_sync()
    }

    fn max_workgroup_size(&self) -> [u32; 3] {
        self.0.max_block_dim()
    }

    fn max_compute_invocations_per_workgroup(&self) -> u32 {
        self.0.max_threads_per_block()
    }

    fn max_compute_workgroups_per_dimension(&self) -> u32 {
        self.0.max_grid_dim()[0]
    }
}

fn add_transfer_bytes(total: &mut u64, bytes: usize, label: &str) -> Result<(), BackendError> {
    CUDA_HOST_TRANSFER_ACCOUNTING.add_bytes(total, bytes, label)
}

fn add_transfer_operation(total: &mut u64, label: &str) -> Result<(), BackendError> {
    CUDA_HOST_TRANSFER_ACCOUNTING.add_operation(total, label)
}

fn host_dispatch_input<'a>(
    inputs: &'a [&[u8]],
    input_index: usize,
    binding_name: &str,
    context: &'static str,
) -> Result<&'a [u8], BackendError> {
    inputs
        .get(input_index)
        .copied()
        .ok_or_else(|| BackendError::InvalidProgram {
            fix: format!(
                "Fix: CUDA host dispatch {context} expected input index {input_index} for `{binding_name}` but only {} input(s) were supplied. Rebuild the binding plan or validate inputs before launch.",
                inputs.len()
            ),
        })
}

impl CudaBackend {
    fn dispatch_borrowed_with_grid_sync_split(
        &self,
        program: &Program,
        inputs: &[&[u8]],
        config: &DispatchConfig,
    ) -> Result<Vec<Vec<u8>>, BackendError> {
        let adapter = GridSyncSplitCudaBackend(self);
        vyre_driver::grid_sync::dispatch_with_grid_sync_split(&adapter, program, inputs, config)
    }

    /// Whether a grid-sync `program` must be dispatched via the host-split path
    /// (its barriers split into separate regular kernel launches) rather than a
    /// single cooperative launch. True when either the device lacks native
    /// grid-sync support, or the program's launch grid exceeds the device's
    /// cooperative thread-residency limit: a cooperative launch needs every CTA
    /// co-resident, so an over-residency grid would fail with
    /// `CooperativeResidencyExceeded`. Host-split segments are regular launches
    /// with no co-residency requirement, so they run at any grid size (e.g. the
    /// recursive multi-block prefix scan whose pass-B grid can exceed the
    /// cooperative residency limit). Caller has already confirmed
    /// `contains_grid_sync(program)`.
    fn grid_sync_program_needs_host_split(
        &self,
        program: &Program,
        inputs: &[&[u8]],
        config: &DispatchConfig,
    ) -> Result<bool, BackendError> {
        if !self.supports_grid_sync() {
            return Ok(true);
        }
        let bindings = BindingPlan::from_borrowed_inputs(program, inputs)?;
        let launch = self.prepare_launch_plan(program, &bindings, config)?;
        Ok(!self.cooperative_residency_admits(&launch)?)
    }

    /// Dispatch a vyre Program synchronously on this CUDA device with borrowed inputs.
    pub fn dispatch_borrowed(
        &self,
        program: &Program,
        inputs: &[&[u8]],
        config: &DispatchConfig,
    ) -> Result<Vec<Vec<u8>>, BackendError> {
        if vyre_driver::grid_sync::contains_grid_sync(program)
            && self.grid_sync_program_needs_host_split(program, inputs, config)?
        {
            return self.dispatch_borrowed_with_grid_sync_split(program, inputs, config);
        }
        self.dispatch_borrowed_async(program, inputs, config)?
            .await_result()
    }

    /// Dispatch a vyre Program asynchronously on this CUDA device.
    pub fn dispatch_async(
        &self,
        program: &Program,
        inputs: &[Vec<u8>],
        config: &DispatchConfig,
    ) -> Result<Box<dyn PendingDispatch>, BackendError> {
        let mut borrowed_inputs = SmallVec::<[&[u8]; 8]>::new();
        reserve_smallvec(&mut borrowed_inputs, inputs.len(), "borrowed input")?;
        for input in inputs {
            borrowed_inputs.push(input.as_slice());
        }
        self.dispatch_borrowed_async(program, &borrowed_inputs, config)
    }

    /// Dispatch a vyre Program asynchronously on this CUDA device with borrowed inputs.
    pub fn dispatch_borrowed_async(
        &self,
        program: &Program,
        inputs: &[&[u8]],
        config: &DispatchConfig,
    ) -> Result<Box<dyn PendingDispatch>, BackendError> {
        let lowered_program =
            vyre_foundation::transform::collectives::lower_single_rank_collectives(program)
                .map_err(|error| BackendError::InvalidProgram {
                    fix: error.to_string(),
                })?;
        let program = lowered_program.as_ref().unwrap_or(program);
        if vyre_driver::grid_sync::contains_grid_sync(program) && !self.supports_grid_sync() {
            let outputs = self.dispatch_borrowed_with_grid_sync_split(program, inputs, config)?;
            return Ok(Box::new(CudaReadyPending { outputs }));
        }
        let trace = crate::instrumentation::cuda_stage_trace_enabled();
        let start = std::time::Instant::now();
        if trace {
            tracing::debug!(
                "[cuda-trace] dispatch_borrowed_async start buffers={} inputs={}",
                program.buffers().len(),
                inputs.len()
            );
        }
        let prepared = self.prepare_host_dispatch(program, inputs, config)?;
        if trace {
            tracing::debug!(
                "[cuda-trace] +{}ms prepare_host_dispatch",
                start.elapsed().as_millis()
            );
        }
        let (ptx_src, ptx_source_key) = self.ptx_for_program_cached_with_key(program, config)?;
        if trace {
            tracing::debug!(
                "[cuda-trace] +{}ms ptx_for_program_cached bytes={}",
                start.elapsed().as_millis(),
                ptx_src.len()
            );
        }
        let module_key = self.module_cache_key_for_ptx_source_key(ptx_source_key)?;

        self.dispatch_prepared_borrowed_async_with_ptx_key(
            program, inputs, &ptx_src, module_key, &prepared,
        )
    }

    /// Dispatch with backend-owned wall and CUDA event timing.
    pub fn dispatch_borrowed_timed(
        &self,
        program: &Program,
        inputs: &[&[u8]],
        config: &DispatchConfig,
    ) -> Result<vyre_driver::TimedDispatchResult, BackendError> {
        let lowered_program =
            vyre_foundation::transform::collectives::lower_single_rank_collectives(program)
                .map_err(|error| BackendError::InvalidProgram {
                    fix: error.to_string(),
                })?;
        let program = lowered_program.as_ref().unwrap_or(program);
        let prepared = self.prepare_host_dispatch(program, inputs, config)?;
        let (ptx_src, ptx_source_key) = self.ptx_for_program_cached_with_key(program, config)?;
        let module_key = self.module_cache_key_for_ptx_source_key(ptx_source_key)?;
        self.dispatch_prepared_borrowed_timed_with_ptx_key(
            program, inputs, config, &ptx_src, module_key, &prepared,
        )
    }

    pub(crate) fn dispatch_borrowed_async_with_ptx_key(
        &self,
        program: &Program,
        inputs: &[&[u8]],
        config: &DispatchConfig,
        ptx_src: &str,
        module_key: ModuleCacheKey,
    ) -> Result<Box<dyn PendingDispatch>, BackendError> {
        let prepared = self.prepare_host_dispatch(program, inputs, config)?;
        self.dispatch_prepared_borrowed_async_with_ptx_key(
            program, inputs, ptx_src, module_key, &prepared,
        )
    }

    pub(crate) fn dispatch_borrowed_timed_with_ptx_key(
        &self,
        program: &Program,
        inputs: &[&[u8]],
        config: &DispatchConfig,
        ptx_src: &str,
        module_key: ModuleCacheKey,
    ) -> Result<vyre_driver::TimedDispatchResult, BackendError> {
        let prepared = self.prepare_host_dispatch(program, inputs, config)?;
        self.dispatch_prepared_borrowed_timed_with_ptx_key(
            program, inputs, config, ptx_src, module_key, &prepared,
        )
    }

    pub(crate) fn dispatch_prepared_borrowed_async_with_ptx_key(
        &self,
        program: &Program,
        inputs: &[&[u8]],
        ptx_src: &str,
        module_key: ModuleCacheKey,
        prepared: &CudaDispatchPlan,
    ) -> Result<Box<dyn PendingDispatch>, BackendError> {
        Ok(Box::new(self.dispatch_borrowed_async_with_ptx_concrete(
            program, inputs, ptx_src, module_key, false, prepared,
        )?))
    }

    pub(crate) fn dispatch_prepared_borrowed_timed_with_ptx_key(
        &self,
        program: &Program,
        inputs: &[&[u8]],
        config: &DispatchConfig,
        ptx_src: &str,
        module_key: ModuleCacheKey,
        prepared: &CudaDispatchPlan,
    ) -> Result<vyre_driver::TimedDispatchResult, BackendError> {
        let started = std::time::Instant::now();
        let enqueue_started = std::time::Instant::now();
        let pending = self.dispatch_borrowed_async_with_ptx_concrete(
            program, inputs, ptx_src, module_key, true, prepared,
        )?;
        let enqueue_ns =
            CUDA_NUMERIC.elapsed_nanos_u64(enqueue_started, "host-dispatch enqueue latency")?;
        let wait_started = std::time::Instant::now();
        let (outputs, device_ns) = pending.await_timed_result()?;
        let wait_ns = CUDA_NUMERIC.elapsed_nanos_u64(wait_started, "host-dispatch wait latency")?;
        if let Some(measured_device_ns) = device_ns {
            let _accepted = vyre_driver::launch::record_launch_measurement(
                program,
                config,
                self.launch_limits(),
                prepared.launch.element_count,
                prepared.launch.workgroup,
                measured_device_ns,
            );
        }
        let wall_ns = CUDA_NUMERIC.elapsed_nanos_u64(started, "host-dispatch wall latency")?;
        self.telemetry
            .record_timed_dispatch(wall_ns, device_ns, Some(enqueue_ns), Some(wait_ns));
        Ok(vyre_driver::TimedDispatchResult {
            outputs,
            wall_ns,
            device_ns,
            enqueue_ns: Some(enqueue_ns),
            wait_ns: Some(wait_ns),
        })
    }

    fn dispatch_borrowed_async_with_ptx_concrete(
        &self,
        program: &Program,
        inputs: &[&[u8]],
        ptx_src: &str,
        module_key: ModuleCacheKey,
        capture_timing: bool,
        prepared: &CudaDispatchPlan,
    ) -> Result<crate::stream::CudaPendingDispatch, BackendError> {
        let _profiler_range =
            crate::profiler::cuda_profiler_range(crate::profiler::CUDA_HOST_DISPATCH_RANGE);
        if prepared
            .bindings
            .bindings
            .iter()
            .any(|binding| binding.role == BindingRole::Persistent)
        {
            return Err(BackendError::UnsupportedFeature {
                name: "cuda_persistent_memory_binding".to_string(),
                backend: crate::CUDA_BACKEND_ID.to_string(),
            });
        }

        let trace = crate::instrumentation::cuda_stage_trace_enabled();
        let start = std::time::Instant::now();
        self.warmup()?;
        if trace {
            tracing::debug!("[cuda-trace] +{}ms warmup", start.elapsed().as_millis());
        }
        self.validate_transient_dispatch_memory_budget(prepared, inputs, "CUDA host dispatch")?;

        let buffers = program.buffers();
        let mut allocations =
            DispatchAllocations::new(buffers.len(), Arc::clone(&self.transient_pool))?;
        let (transfer_capacity, output_capacity) = host_transfer_capacities(prepared)?;
        let mut host_transfers = HostTransferAllocations::with_capacity(
            Arc::clone(&self.host_pool),
            transfer_capacity,
            output_capacity,
        )?;
        let mut host_uploads = SmallVec::<[HostUpload; 8]>::new();
        reserve_smallvec(
            &mut host_uploads,
            host_upload_batch_capacity(prepared)?,
            "host upload",
        )?;
        let mut device_clears = SmallVec::<[DeviceClear; 8]>::new();
        reserve_smallvec(
            &mut device_clears,
            prepared.bindings.bindings.len(),
            "device clear",
        )?;
        let mut upload_bytes = 0_u64;
        let mut upload_operations = 0_u64;

        for binding in &prepared.bindings.bindings {
            if binding.role == BindingRole::Shared {
                continue;
            }

            let byte_len = match binding.input_index {
                Some(input_index) => {
                    host_dispatch_input(inputs, input_index, &binding.name, "allocation sizing")?
                        .len()
                }
                None => binding.static_byte_len.ok_or_else(|| BackendError::InvalidProgram {
                    fix: format!(
                        "Fix: CUDA output `{}` needs a static byte length before launch; set BufferDecl::with_count or output_byte_range.",
                        binding.name
                    ),
                })?,
            };

            let allocation_byte_len = aligned_async_copy_len(byte_len)?;
            let allocation = self.transient_pool.acquire(allocation_byte_len)?;
            self.telemetry.record_transient_allocation_bytes(
                CUDA_NUMERIC
                    .usize_to_u64(allocation.byte_len, "transient allocation byte count")?,
            );
            let dev_ptr = allocation.ptr;
            allocations.set_ptr(binding.buffer_index, allocation, &binding.name)?;

            if let Some(input_index) = binding.input_index {
                let input =
                    host_dispatch_input(inputs, input_index, &binding.name, "upload staging")?;
                let copy_byte_len = aligned_async_copy_len(input.len())?;
                let host_ptr = host_transfers.push_upload_padded(input, copy_byte_len)?;
                add_transfer_bytes(&mut upload_bytes, input.len(), "host upload")?;
                if !input.is_empty() {
                    add_transfer_operation(&mut upload_operations, "host upload")?;
                }
                host_uploads.push(HostUpload {
                    dst: dev_ptr,
                    src: host_ptr,
                    byte_len: copy_byte_len,
                });
            } else if byte_len != 0 {
                device_clears.push(DeviceClear {
                    dst: dev_ptr,
                    byte_len: allocation.byte_len,
                });
            }
        }

        let param_bytes = launch_param_byte_len(&prepared.launch.param_words, "host dispatch")?;
        let params_buf_ptr = if param_bytes == 0 {
            0
        } else {
            let param_copy_bytes = aligned_async_copy_len(param_bytes)?;
            let params_allocation = self.transient_pool.acquire(param_copy_bytes)?;
            self.telemetry
                .record_transient_allocation_bytes(CUDA_NUMERIC.usize_to_u64(
                    params_allocation.byte_len,
                    "parameter allocation byte count",
                )?);
            let params_buf_ptr = params_allocation.ptr;
            let param_host_ptr = host_transfers
                .push_u32_words_padded(&prepared.launch.param_words, param_copy_bytes)?;
            host_uploads.push(HostUpload {
                dst: params_buf_ptr,
                src: param_host_ptr,
                byte_len: param_copy_bytes,
            });
            add_transfer_bytes(&mut upload_bytes, param_bytes, "parameter upload")?;
            add_transfer_operation(&mut upload_operations, "parameter upload")?;
            self.telemetry.record_param_upload_bytes(
                CUDA_NUMERIC.usize_to_u64(param_bytes, "parameter upload byte count")?,
            );
            allocations.set_params(params_allocation);
            params_buf_ptr
        };

        let launch_resources = crate::stream::CudaLaunchResourceLease::acquire(
            Arc::clone(&self.launch_resources),
            capture_timing,
        )?;
        let mut launch_resources = Some(launch_resources);
        let mut allocations = Some(allocations);
        let mut host_transfers = Some(host_transfers);
        let stream_raw = launch_resources
            .as_ref()
            .ok_or_else(|| BackendError::InvalidProgram {
                fix: "Fix: CUDA host dispatch launch resources were consumed before enqueue; rebuild pending dispatch ownership before launching.".to_string(),
            })?
            .stream_raw()?;
        if trace {
            tracing::debug!(
                "[cuda-trace] +{}ms stream/events",
                start.elapsed().as_millis()
            );
        }
        let pending = (|| {
            let allocations_ref = allocations.as_ref().ok_or_else(|| BackendError::InvalidProgram {
                fix: "Fix: CUDA host dispatch allocations were consumed before enqueue finished; rebuild pending dispatch ownership before launching.".to_string(),
            })?;
            let host_transfers_ref = host_transfers
                .as_mut()
                .ok_or_else(|| BackendError::InvalidProgram {
                    fix: "Fix: CUDA host dispatch host staging was consumed before enqueue finished; rebuild pending dispatch ownership before launching.".to_string(),
                })?;
            let launch_resources_ref =
                launch_resources
                    .as_ref()
                    .ok_or_else(|| BackendError::InvalidProgram {
                        fix: "Fix: CUDA host dispatch launch resources were consumed before enqueue finished; rebuild pending dispatch ownership before launching.".to_string(),
                    })?;

            enqueue_host_uploads_async(&host_uploads, stream_raw)?;
            self.telemetry.record_host_to_device_bytes(upload_bytes);
            self.telemetry
                .record_host_upload_operations(upload_operations);
            enqueue_device_clears_async(&device_clears, stream_raw)?;
            if trace {
                tracing::debug!(
                    "[cuda-trace] +{}ms alloc/upload/clear",
                    start.elapsed().as_millis()
                );
            }

            if let Some((start_event, _)) = launch_resources_ref.timing_events()? {
                start_event.record(stream_raw)?;
            }
            // Fixpoint loop: launch the kernel `fixpoint_iterations` times
            // on the same stream. CUDA serialises kernels within a single
            // stream so each iteration observes the previous iteration's
            // writes  -  the persistent-state contract that dataflow BFS-on-CSR
            // primitives rely on to converge multi-hop reachability.
            // `allocations` stays device-resident across iterations, so the
            // pointer vector is materialized once and borrowed by each launch.
            let func = self.resolve_launch_function(
                ptx_src,
                module_key,
                &prepared.launch,
                prepared.cooperative,
            )?;
            if trace {
                tracing::debug!(
                    "[cuda-trace] +{}ms resolve_launch_function",
                    start.elapsed().as_millis()
                );
            }
            let mut ptr_values = SmallVec::<[u64; 8]>::new();
            reserve_smallvec(
                &mut ptr_values,
                prepared.bindings.bindings.len(),
                "kernel pointer argument",
            )?;
            for binding in &prepared.bindings.bindings {
                if binding.role == BindingRole::Shared {
                    continue;
                }
                let ptr = allocations_ref.ptr(binding.buffer_index, &binding.name)?;
                if ptr == 0 {
                    return Err(BackendError::InvalidProgram {
                        fix: format!(
                            "Fix: CUDA launch binding `{}` has no device allocation; argument order must match the lowered kernel descriptor.",
                            binding.name
                        ),
                    });
                }
                ptr_values.push(ptr);
            }
            if trace {
                tracing::debug!(
                    "[cuda-trace] +{}ms host args ptr_values={:x?} params=0x{params_buf_ptr:x} words={:?} grid={:?} workgroup={:?} element_count={}",
                    start.elapsed().as_millis(),
                    ptr_values,
                    prepared.launch.param_words,
                    prepared.launch.grid,
                    prepared.launch.workgroup,
                    prepared.launch.element_count
                );
            }
            let mut params_ref = params_buf_ptr;
            let mut kernel_args = Self::kernel_args(&mut ptr_values, &mut params_ref)?;
            // A native grid-sync kernel reads/writes a module-scope arrival
            // counter (`_vyre_grid_barrier`). Within one cooperative launch the
            // in-kernel barriers drive it up to N*gridSize for N barriers, so it
            // MUST start each launch at zero; a stale value would release the
            // next launch's first barrier before every CTA arrives. Resolve the
            // counter once, then zero it on the dispatch stream before each
            // launch (the memset is stream-ordered ahead of the kernel).
            let grid_barrier_counter = if prepared.cooperative
                && vyre_driver::grid_sync::contains_grid_sync(program)
            {
                match self.grid_barrier_global_with_key(ptx_src, module_key)? {
                    Some(global) => Some(global),
                    None => {
                        return Err(BackendError::InvalidProgram {
                            fix:
                                "Fix: CUDA cooperative grid-sync launch found no `_vyre_grid_barrier` counter in the loaded module although the program contains grid-sync barriers. Ensure the PTX emitter declares the module-scope counter for grid-sync kernels."
                                    .to_string(),
                        });
                    }
                }
            } else {
                None
            };
            for _ in 0..prepared.fixpoint_iterations {
                if let Some((counter_ptr, counter_len)) = grid_barrier_counter {
                    // SAFETY: counter_ptr/len come from cuModuleGetGlobal on the
                    // loaded module; stream_raw is the dispatch stream; the
                    // memset is enqueued before the launch on the same stream.
                    unsafe {
                        super::copy::memset_d8_async_checked(
                            counter_ptr,
                            0,
                            counter_len,
                            stream_raw,
                        )?;
                    }
                }
                self.launch_prevalidated_function(
                    func,
                    &mut kernel_args,
                    &prepared.launch,
                    stream_raw,
                    false,
                    prepared.cooperative,
                )?;
            }
            if trace {
                tracing::debug!("[cuda-trace] +{}ms launch", start.elapsed().as_millis());
            }

            let mut readback_bytes = 0_u64;
            let mut readback_operations = 0_u64;
            for &binding_index in &prepared.output_binding_indices {
                let binding =
                    prepared.output_binding(binding_index, "host dispatch output readback")?;
                let full_byte_len = match binding.static_byte_len {
                    Some(len) => len,
                    None => match binding.input_index {
                        Some(input_index) => host_dispatch_input(
                            inputs,
                            input_index,
                            &binding.name,
                            "output readback sizing",
                        )?
                        .len(),
                        None => {
                            return Err(BackendError::InvalidProgram {
                                fix: format!(
                                    "Fix: CUDA output `{}` needs a static byte length before readback.",
                                    binding.name
                                ),
                            });
                        }
                    },
                };
                let readback = cuda_output_readback_for_binding(
                    buffers,
                    binding.buffer_index,
                    &binding.name,
                    full_byte_len,
                    "output readback",
                )?;
                let allocation_byte_len =
                    allocations_ref.byte_len(binding.buffer_index, &binding.name)?;
                let padded_readback_len = aligned_async_copy_len(readback.byte_len)?;
                let readback_end = readback
                    .device_offset
                    .checked_add(padded_readback_len)
                    .ok_or_else(|| BackendError::InvalidProgram {
                        fix: format!(
                            "Fix: CUDA host dispatch readback for output `{}` overflowed while checking capacity at device offset {} with padded length {}. Rebuild the program with a valid output byte range or split the output buffer.",
                            binding.name, readback.device_offset, padded_readback_len
                        ),
                    })?;
                let copy_byte_len = if readback_end <= allocation_byte_len {
                    padded_readback_len
                } else {
                    readback.byte_len
                };
                let out_ptr =
                    host_transfers_ref.push_output_padded(readback.byte_len, copy_byte_len)?;
                if readback.byte_len != 0 {
                    add_transfer_bytes(&mut readback_bytes, readback.byte_len, "output readback")?;
                    add_transfer_operation(&mut readback_operations, "output readback")?;
                    let base_ptr = allocations_ref.ptr(binding.buffer_index, &binding.name)?;
                    let device_ptr = vyre_driver::accounting::checked_add_u64_usize_offset_lazy(
                        base_ptr,
                        readback.device_offset,
                        || {
                            BackendError::InvalidProgram {
                            fix: format!(
                                "Fix: CUDA host dispatch readback device offset {} for output `{}` does not fit CUdeviceptr arithmetic.",
                                readback.device_offset, binding.name
                            ),
                        }
                        },
                        || {
                            BackendError::InvalidProgram {
                            fix: format!(
                                "Fix: CUDA host dispatch readback pointer overflowed for output `{}` at device_ptr={base_ptr} offset={}. Rebuild the program with a valid output byte range or split the output buffer.",
                                binding.name, readback.device_offset
                            ),
                        }
                        },
                    )?;
                    // SAFETY: FFI to libcuda.so. Pointer args were validated by
                    // the matching alloc / store API; lifetimes are documented in
                    // the surrounding function. cuda_check (or matching CUresult
                    // guard) propagates non-success codes as BackendError.
                    unsafe {
                        super::copy::d2h_async_checked(
                            out_ptr,
                            device_ptr,
                            copy_byte_len,
                            stream_raw,
                        )?;
                    }
                }
            }
            self.telemetry
                .record_device_to_host_readback(readback_bytes);
            self.telemetry
                .record_device_readback_operations(readback_operations);
            if let Some((_, end_event)) = launch_resources_ref.timing_events()? {
                end_event.record(stream_raw)?;
            }

            let output_storage =
                reserved_vec(prepared.output_binding_indices.len(), "pending output")?;
            let event = self.launch_resources.acquire_event()?;
            if let Err(error) = event.record(stream_raw) {
                self.launch_resources.release_event(event);
                return Err(error);
            }
            if trace {
                tracing::debug!(
                    "[cuda-trace] +{}ms readback/event",
                    start.elapsed().as_millis()
                );
            }
            let (stream, timing_events) =
                launch_resources
                    .take()
                    .ok_or_else(|| BackendError::InvalidProgram {
                        fix: "Fix: CUDA host dispatch launch resources were consumed before pending dispatch ownership transfer.".to_string(),
                    })?
                    .into_parts()?;
            let allocations = allocations
                .take()
                .ok_or_else(|| BackendError::InvalidProgram {
                    fix: "Fix: CUDA host dispatch allocations were consumed before pending dispatch ownership transfer.".to_string(),
                })?;
            let host_transfers =
                host_transfers
                    .take()
                    .ok_or_else(|| BackendError::InvalidProgram {
                    fix: "Fix: CUDA host dispatch host staging was consumed before pending dispatch ownership transfer.".to_string(),
                })?;
            if let Some((start_event, end_event)) = timing_events {
                Ok(crate::stream::CudaPendingDispatch::new_with_timing(
                    Arc::clone(&self.ctx),
                    Arc::clone(&self.launch_resources),
                    event,
                    stream,
                    allocations,
                    None,
                    Some(host_transfers),
                    output_storage,
                    start_event,
                    end_event,
                    Arc::clone(&self.telemetry),
                ))
            } else {
                Ok(crate::stream::CudaPendingDispatch::new(
                    Arc::clone(&self.ctx),
                    Arc::clone(&self.launch_resources),
                    event,
                    stream,
                    allocations,
                    None,
                    Some(host_transfers),
                    output_storage,
                    Arc::clone(&self.telemetry),
                ))
            }
        })();
        if let Err(error) = pending {
            let Some(launch_resources) = launch_resources.take() else {
                return Err(error);
            };
            match crate::stream::synchronize_raw_stream(
                stream_raw,
                "cuStreamSynchronize (host dispatch error cleanup)",
            ) {
                Ok(()) => {
                    self.telemetry.record_sync_point();
                    return Err(error);
                }
                Err(sync_error) => {
                    tracing::error!(
                        "Fix: failed to synchronize CUDA host dispatch stream after enqueue error: {sync_error}. In-flight host dispatch resources will not be recycled."
                    );
                    std::mem::forget(launch_resources);
                    if let Some(allocations) = allocations.take() {
                        std::mem::forget(allocations);
                    }
                    if let Some(host_transfers) = host_transfers.take() {
                        std::mem::forget(host_transfers);
                    }
                    return Err(error);
                }
            }
        }
        pending
    }

    /// Dispatch a vyre Program on this CUDA device.
    pub fn dispatch(
        &self,
        program: &Program,
        inputs: &[Vec<u8>],
        config: &DispatchConfig,
    ) -> Result<Vec<Vec<u8>>, BackendError> {
        let lowered_program =
            vyre_foundation::transform::collectives::lower_single_rank_collectives(program)
                .map_err(|error| BackendError::InvalidProgram {
                    fix: error.to_string(),
                })?;
        let program = lowered_program.as_ref().unwrap_or(program);
        // Reject programs that ask for capabilities the live CUDA
        // backend doesn't expose BEFORE we attempt PTX emit. Without
        // this gate, indirect_dispatch / f16 / bf16 IR falls all the
        // way down to vyre-emit-ptx and surfaces a generic
        // "unsupported KernelOp kind" message that hides the
        // missing-capability contract the dispatch layer is supposed
        // to enforce.
        let required = vyre_foundation::program_caps::scan(program);
        let validation_caps = self.program_validation_caps();
        vyre_foundation::program_caps::check_backend_capabilities(
            validation_caps.backend_id,
            validation_caps.supports_subgroup_ops,
            validation_caps.supports_f16,
            validation_caps.supports_bf16,
            validation_caps.supports_indirect_dispatch,
            validation_caps.supports_trap_propagation,
            validation_caps.supports_distributed_collectives,
            validation_caps.max_workgroup_size,
            &required,
        )
        .map_err(|error| BackendError::InvalidProgram {
            fix: error.to_string(),
        })?;
        if vyre_driver::grid_sync::contains_grid_sync(program) {
            let mut borrowed_inputs = SmallVec::<[&[u8]; 8]>::new();
            reserve_smallvec(
                &mut borrowed_inputs,
                inputs.len(),
                "grid-sync CUDA dispatch input",
            )?;
            borrowed_inputs.extend(inputs.iter().map(Vec::as_slice));
            if self.grid_sync_program_needs_host_split(program, &borrowed_inputs, config)? {
                return self
                    .dispatch_borrowed_with_grid_sync_split(program, &borrowed_inputs, config);
            }
        }
        self.dispatch_async(program, inputs, config)?.await_result()
    }
}

#[inline]
fn host_transfer_capacities(prepared: &CudaDispatchPlan) -> Result<(usize, usize), BackendError> {
    let output_capacity = prepared.output_binding_indices.len();
    let upload_capacity = host_upload_batch_capacity(prepared)?;
    let transfer_capacity = checked_add_usize_lazy(upload_capacity, output_capacity, || {
        BackendError::InvalidProgram {
                fix: format!(
                    "Fix: CUDA host transfer capacity overflowed usize for {upload_capacity} upload slot(s) plus {output_capacity} output slot(s); split the dispatch."
                ),
            }
    })?;
    Ok((transfer_capacity, output_capacity))
}

#[inline]
fn host_upload_batch_capacity(prepared: &CudaDispatchPlan) -> Result<usize, BackendError> {
    let input_slots = prepared.bindings.input_indices.len();
    checked_add_usize_lazy(
        input_slots,
        usize::from(!prepared.launch.param_words.is_empty()),
        || {
            BackendError::InvalidProgram {
            fix: "Fix: CUDA host upload batch capacity overflowed usize while adding the params upload slot; split the dispatch."
                .to_string(),
        }
        },
    )
}

#[inline]
fn enqueue_host_uploads_async(
    uploads: &[HostUpload],
    stream: CUstream,
) -> Result<(), BackendError> {
    for upload in uploads {
        if upload.byte_len == 0 {
            continue;
        }
        // SAFETY: FFI to libcuda.so. Pointer args were validated by the
        // matching alloc / store API; lifetimes are documented in the
        // surrounding function. cuda_check (or matching CUresult guard)
        // propagates non-success codes as BackendError.
        unsafe {
            super::copy::h2d_async_checked(upload.dst, upload.src, upload.byte_len, stream)?;
        }
    }
    Ok(())
}

#[inline]
fn enqueue_device_clears_async(
    clears: &[DeviceClear],
    stream: CUstream,
) -> Result<(), BackendError> {
    for clear in clears {
        // SAFETY: FFI to libcuda.so. Pointer args were validated by the
        // matching alloc / store API; lifetimes are documented in the
        // surrounding function. cuda_check (or matching CUresult guard)
        // propagates non-success codes as BackendError.
        unsafe {
            super::copy::memset_d8_async_checked(clear.dst, 0, clear.byte_len, stream)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{host_transfer_capacities, host_upload_batch_capacity};
    use crate::backend::CudaDispatchPlan;
    use smallvec::smallvec;
    use std::sync::Arc;
    use vyre_driver::binding::{Binding, BindingPlan, BindingRole};
    use vyre_driver::LaunchPlan;

    #[test]
    fn host_upload_batch_capacity_counts_inputs_once_plus_params() {
        let plan = CudaDispatchPlan {
            bindings: BindingPlan {
                bindings: vec![
                    Binding {
                        name: Arc::from("a"),
                        binding: 0,
                        buffer_index: 0,
                        role: BindingRole::Input,
                        element_size: 4,
                        preferred_alignment: 4,
                        element_count: 16,
                        static_byte_len: Some(64),
                        input_index: Some(0),
                        output_index: None,
                    },
                    Binding {
                        name: Arc::from("b"),
                        binding: 1,
                        buffer_index: 1,
                        role: BindingRole::InputOutput,
                        element_size: 4,
                        preferred_alignment: 4,
                        element_count: 16,
                        static_byte_len: Some(64),
                        input_index: Some(1),
                        output_index: Some(0),
                    },
                    Binding {
                        name: Arc::from("out"),
                        binding: 2,
                        buffer_index: 2,
                        role: BindingRole::Output,
                        element_size: 4,
                        preferred_alignment: 4,
                        element_count: 16,
                        static_byte_len: Some(64),
                        input_index: None,
                        output_index: Some(1),
                    },
                ],
                input_indices: vec![0, 1],
                output_indices: vec![1, 2],
                shared_indices: Vec::new(),
            },
            output_binding_indices: smallvec![1, 2],
            launch: LaunchPlan::new(),
            cooperative: false,
            fixpoint_iterations: 1,
        };

        assert_eq!(
            host_upload_batch_capacity(&plan).expect("Fix: capacity must fit"),
            2,
            "zero-byte launch params must not reserve a fake H2D upload slot"
        );
        assert_eq!(
            host_transfer_capacities(&plan).expect("Fix: capacity must fit"),
            (4, 2),
            "pinned-host transfer storage must reserve inputs + outputs only when params are empty"
        );

        let mut plan_with_params = plan;
        plan_with_params.launch.param_words.push(7);
        assert_eq!(
            host_upload_batch_capacity(&plan_with_params).expect("Fix: capacity must fit"),
            3,
            "non-empty launch params must reserve one H2D upload slot"
        );
        assert_eq!(
            host_transfer_capacities(&plan_with_params).expect("Fix: capacity must fit"),
            (5, 2),
            "pinned-host transfer storage must reserve inputs + params + outputs when params exist"
        );
    }

    #[test]
    fn host_dispatch_enqueue_errors_leak_resources_when_completion_is_unproven() {
        let source = include_str!("host_dispatch.rs");
        let dispatch = source
            .split("fn dispatch_borrowed_async_with_ptx_concrete")
            .nth(1)
            .expect("Fix: CUDA host dispatch async implementation must exist.")
            .split("    }\n\n    /// Dispatch a vyre Program on this CUDA device.")
            .next()
            .expect("Fix: CUDA host dispatch async implementation must precede sync dispatch API.");
        assert!(
            dispatch.contains("let mut launch_resources = Some(launch_resources);")
                && dispatch.contains("let mut allocations = Some(allocations);")
                && dispatch.contains("let mut host_transfers = Some(host_transfers);")
                && dispatch.contains("let pending = (||"),
            "Fix: CUDA host dispatch must retain launch resources, transient allocations, and pinned host staging in outer cleanup ownership until pending dispatch takes over."
        );
        assert!(
            dispatch.contains("crate::stream::synchronize_raw_stream(\n                stream_raw,\n                \"cuStreamSynchronize (host dispatch error cleanup)\",")
                && dispatch.contains("In-flight host dispatch resources will not be recycled.")
                && dispatch.contains("std::mem::forget(launch_resources);")
                && dispatch.contains("std::mem::forget(allocations);")
                && dispatch.contains("std::mem::forget(host_transfers);"),
            "Fix: CUDA host dispatch enqueue errors must leak stream, transient allocations, and pinned host staging when completion is unproven."
        );
        let cleanup_pos = dispatch
            .find("if let Err(error) = pending")
            .expect("Fix: CUDA host dispatch must classify pending construction errors.");
        let transfer_pos = dispatch.find("CudaPendingDispatch::new(").expect(
            "Fix: CUDA host dispatch must eventually transfer ownership to CudaPendingDispatch.",
        );
        assert!(
            transfer_pos < cleanup_pos,
            "Fix: CUDA host dispatch must install fail-closed cleanup around all fallible enqueue work before returning pending ownership."
        );
        let output_storage_pos = dispatch
            .find("reserved_vec(prepared.output_binding_indices.len(), \"pending output\")")
            .expect("Fix: CUDA host dispatch must reserve pending output storage.");
        let stream_take_pos = dispatch.find(".into_parts()?").expect(
            "Fix: CUDA host dispatch must transfer stream ownership into the pending dispatch.",
        );
        assert!(
            output_storage_pos < stream_take_pos,
            "Fix: CUDA host dispatch must finish fallible output storage reservation before consuming launch-resource ownership."
        );
    }
}
