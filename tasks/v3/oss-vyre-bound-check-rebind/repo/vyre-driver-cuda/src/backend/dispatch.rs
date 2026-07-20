//! CUDA backend: device lifecycle, buffer management, and kernel dispatch.

use std::sync::{Arc, Mutex};

use cudarc::driver::CudaContext;
use smallvec::SmallVec;
use vyre_driver::binding::{BindingPlan, BindingRole};
use vyre_driver::speculate::SpeculationMode;
use vyre_driver::validation::ValidationCache;
use vyre_driver::{resolve_fixpoint_iterations, BackendError, DispatchConfig, LaunchPlan};
use vyre_foundation::ir::Program;

use super::allocations::{DeviceAllocationPool, PinnedHostAllocationPool};
use super::module_cache::{
    CudaModuleCache, CudaPtxSourceCache, CudaPtxSourceCacheSnapshot, ModuleCacheKey,
    PtxSourceCacheKey,
};
use super::plan::{compute_ordered_output_indices, CudaDispatchPlan};
use super::ptx_target::select_loadable_ptx_target_sm;
use super::resident::{CudaResidentBuffer, CudaResidentStore, ResidentBufferView};
use super::resident_dispatch::next_resident_handle;
use super::staging_reserve::reserve_smallvec;
use super::telemetry::{CudaTelemetry, CudaTelemetrySnapshot};
use crate::device::{CudaDeviceCaps, CudaDeviceHandle};

const TRANSIENT_ALLOCATION_POOL_BYTES: usize = 256 * 1024 * 1024;
const PINNED_HOST_POOL_BYTES: usize = 128 * 1024 * 1024;
const CUDA_LAUNCH_RESOURCE_CACHE: usize = 128;

#[cfg(test)]
mod tests {
    #[test]
    fn resident_dispatch_input_lengths_reserve_fallibly() {
        let source = include_str!("dispatch.rs");
        assert!(
            source.contains("use super::staging_reserve::reserve_smallvec;"),
            "Fix: CUDA resident dispatch staging must use the shared fallible staging reservation contract."
        );
        assert!(
            source.contains("reserve_smallvec(")
                && source.contains("&mut input_lengths")
                && source.contains("static_bindings.input_indices.len()")
                && source.contains("\"resident dispatch input lengths\"")
                && !source.contains(concat!(
                    "SmallVec::<[usize; 8]>::",
                    "with_capacity(static_bindings.input_indices.len())"
                ))
                && !source.contains(concat!("input_lengths", ".resize(")),
            "Fix: CUDA resident dispatch input-length staging must reserve fallibly instead of using infallible SmallVec capacity growth."
        );
        assert!(
            source.contains(
                "input_lengths.extend(std::iter::repeat_n(0, static_bindings.input_indices.len()))"
            ),
            "Fix: CUDA resident dispatch input lengths must extend after fallible reserve without resize-driven growth."
        );
        assert!(
            source.contains("input_lengths.get_mut(input_index)")
                && source.contains("resident dispatch input binding index {input_index}")
                && !source.contains(concat!("input_lengths", "[input_index]")),
            "Fix: CUDA resident dispatch input-length derivation must turn stale binding input indexes into BackendError instead of directly indexing the input length table."
        );
    }
}

/// A live CUDA backend handle bound to a specific device.
#[derive(Debug, Clone)]
pub struct CudaBackend {
    /// Probed device capabilities over the hardware limit.
    pub caps: CudaDeviceCaps,
    pub(crate) ptx_target_sm: u32,
    pub(crate) launch_resources: Arc<crate::stream::CudaLaunchResourcePool>,
    pub(crate) transient_pool: Arc<DeviceAllocationPool>,
    pub(crate) host_pool: Arc<PinnedHostAllocationPool>,
    pub(crate) ptx_source_cache: Arc<CudaPtxSourceCache>,
    module_cache: Arc<CudaModuleCache>,
    pub(crate) resident_store: Arc<CudaResidentStore>,
    pub(crate) validation_cache: Arc<ValidationCache>,
    pub(crate) graph_capture_lock: Arc<Mutex<()>>,
    pub(crate) async_upload_stream: Arc<Mutex<Option<crate::stream::CudaStream>>>,
    pub(crate) telemetry: Arc<CudaTelemetry>,
    pub(crate) ctx: Arc<CudaContext>,
}

impl CudaBackend {
    /// Acquire the default CUDA device (ordinal 0).
    pub fn acquire() -> Result<Self, String> {
        Self::acquire_ordinal(0)
    }

    /// Acquire a specific CUDA device by ordinal.
    ///
    /// # Errors
    ///
    /// Returns an error when the CUDA driver cannot initialize, the ordinal is
    /// out of range, or required device attributes cannot be queried.
    pub fn acquire_ordinal(ordinal: usize) -> Result<Self, String> {
        // E4 + E5: enable the CUDA driver's persistent disk JIT cache
        // before any module load so the first dispatch this process
        // does on a previously-seen kernel hits the on-disk cuBIN
        // instead of re-JITing. Idempotent and respectful of operator
        // overrides via the CUDA_CACHE_* env vars.
        crate::jit_cache::configure_jit_cache_default()?;
        let device = CudaDeviceHandle::acquire_ordinal(ordinal)?;
        let caps = device.caps;
        let ptx_target_sm = select_loadable_ptx_target_sm(caps.ptx_target_sm())?;
        Ok(Self {
            caps,
            ptx_target_sm,
            launch_resources: Arc::new(crate::stream::CudaLaunchResourcePool::new(
                CUDA_LAUNCH_RESOURCE_CACHE,
            )),
            transient_pool: Arc::new(DeviceAllocationPool::new(TRANSIENT_ALLOCATION_POOL_BYTES)),
            host_pool: Arc::new(PinnedHostAllocationPool::new(PINNED_HOST_POOL_BYTES)),
            ptx_source_cache: Arc::new(CudaPtxSourceCache::new()),
            module_cache: Arc::new(CudaModuleCache::new()),
            resident_store: Arc::new(CudaResidentStore::new()),
            validation_cache: Arc::new(ValidationCache::default()),
            graph_capture_lock: Arc::new(Mutex::new(())),
            async_upload_stream: Arc::new(Mutex::new(None)),
            telemetry: Arc::new(CudaTelemetry::default()),
            ctx: device.ctx,
        })
    }

    pub(crate) fn prepare_launch_plan(
        &self,
        program: &Program,
        bindings: &BindingPlan,
        config: &DispatchConfig,
    ) -> Result<LaunchPlan, BackendError> {
        self.enforce_config_caps(config)?;
        LaunchPlan::from_bindings(program, &bindings.bindings, config, self.launch_limits())
    }

    pub(crate) fn prepare_host_dispatch(
        &self,
        program: &Program,
        inputs: &[&[u8]],
        config: &DispatchConfig,
    ) -> Result<CudaDispatchPlan, BackendError> {
        let bindings = BindingPlan::from_borrowed_inputs(program, inputs)?;
        let launch = self.prepare_launch_plan(program, &bindings, config)?;
        self.validate_program_cached(program)?;
        let cooperative = self.resolve_host_cooperative_flag(program, config)?;
        let output_binding_indices = compute_ordered_output_indices(&bindings)?;
        let fixpoint_iterations = resolve_fixpoint_iterations(config, "CUDA")?;
        Ok(CudaDispatchPlan {
            bindings,
            output_binding_indices,
            launch,
            cooperative,
            fixpoint_iterations,
        })
    }

    /// Cooperative-launch flag for the borrowed-host path, accounting for the
    /// program's grid-sync content.
    ///
    /// A program that still contains `MemoryOrdering::GridSync` barriers when it
    /// reaches host dispatch has been lowered to PTX with native in-kernel grid
    /// barriers (the resident-fixpoint and host-split paths split the barriers
    /// out before lowering, so they never arrive here). Such a kernel MUST be
    /// launched cooperatively — every CTA co-resident — or the in-kernel grid
    /// barrier deadlocks. Force cooperative and fail closed when the device
    /// cannot run cooperative launch, rather than silently launching a kernel
    /// that would hang.
    fn resolve_host_cooperative_flag(
        &self,
        program: &Program,
        config: &DispatchConfig,
    ) -> Result<bool, BackendError> {
        if vyre_driver::grid_sync::contains_grid_sync(program) {
            if !self.hardware_supports_grid_sync() {
                return Err(BackendError::UnsupportedFeature {
                    name: format!(
                        "cuda_native_grid_sync (compute_capability={:?}, cooperative_launch={})",
                        self.caps.compute_capability, self.caps.cooperative_launch
                    ),
                    backend: crate::CUDA_BACKEND_ID.to_string(),
                });
            }
            return Ok(true);
        }
        self.resolve_cooperative_flag(config)
    }

    pub(crate) fn prepare_static_dispatch(
        &self,
        program: &Program,
        config: &DispatchConfig,
    ) -> Result<CudaDispatchPlan, BackendError> {
        let bindings = BindingPlan::build(program)?;
        let launch = self.prepare_launch_plan(program, &bindings, config)?;
        self.validate_program_cached(program)?;
        let cooperative = self.resolve_cooperative_flag(config)?;
        let output_binding_indices = compute_ordered_output_indices(&bindings)?;
        let fixpoint_iterations = resolve_fixpoint_iterations(config, "CUDA")?;
        Ok(CudaDispatchPlan {
            bindings,
            output_binding_indices,
            launch,
            cooperative,
            fixpoint_iterations,
        })
    }

    pub(crate) fn prepare_resident_dispatch(
        &self,
        program: &Program,
        handles: &[CudaResidentBuffer],
        config: &DispatchConfig,
    ) -> Result<CudaDispatchPlan, BackendError> {
        let static_bindings = BindingPlan::build(program)?;
        let required_handles = static_bindings
            .bindings
            .len()
            .checked_sub(static_bindings.shared_indices.len())
            .ok_or_else(|| BackendError::InvalidProgram {
                fix: format!(
                    "Fix: CUDA resident binding plan has {} binding(s) but {} shared binding index(es). Rebuild the dispatch plan before launching.",
                    static_bindings.bindings.len(),
                    static_bindings.shared_indices.len()
                ),
            })?;
        if handles.len() != required_handles {
            return Err(BackendError::InvalidProgram {
                fix: format!(
                    "Fix: CUDA resident dispatch expected {required_handles} resident buffer handle(s) but received {}.",
                    handles.len()
                ),
            });
        }

        let mut input_lengths = SmallVec::<[usize; 8]>::new();
        reserve_smallvec(
            &mut input_lengths,
            static_bindings.input_indices.len(),
            "resident dispatch input lengths",
        )?;
        input_lengths.extend(std::iter::repeat_n(0, static_bindings.input_indices.len()));
        let mut next_handle = 0usize;
        for binding in &static_bindings.bindings {
            if binding.role == BindingRole::Shared {
                continue;
            }
            let handle = next_resident_handle(
                handles,
                &mut next_handle,
                "resident dispatch input-length derivation",
            )?;
            let resident = self.resident_store.view(handle)?;
            if let Some(input_index) = binding.input_index {
                let Some(input_len) = input_lengths.get_mut(input_index) else {
                    return Err(BackendError::InvalidProgram {
                        fix: format!(
                            "Fix: CUDA resident dispatch input binding index {input_index} has no matching input-length slot after deriving {} resident input length(s). Rebuild the binding plan before resident launch.",
                            input_lengths.len()
                        ),
                    });
                };
                *input_len = resident.byte_len;
            }
        }

        let bindings = BindingPlan::from_input_lengths(program, &input_lengths)?;
        let launch = self.prepare_launch_plan(program, &bindings, config)?;
        self.validate_program_cached(program)?;
        let cooperative = self.resolve_cooperative_flag(config)?;
        let output_binding_indices = compute_ordered_output_indices(&bindings)?;
        let fixpoint_iterations = resolve_fixpoint_iterations(config, "CUDA")?;
        Ok(CudaDispatchPlan {
            bindings,
            output_binding_indices,
            launch,
            cooperative,
            fixpoint_iterations,
        })
    }

    /// Validate that the caller's cooperative-launch request is consistent
    /// with the device's reported capabilities. Returns the resolved flag
    /// (always `false` when the caller didn't ask) or an `UnsupportedFeature`
    /// error when the caller asked for cooperative launch on a device that
    /// can't run it.
    ///
    /// This method gates *only* the host-side launch API, NOT the codegen
    /// emission of in-kernel grid-sync barriers. The barrier emission is
    /// still controlled by `lowers_grid_sync()`. Callers that opt into
    /// cooperative launch but whose program does not contain any GridSync
    /// barriers get the cooperative API call (resident grid) but no
    /// in-kernel sync sequence  -  the launcher still runs faster on programs
    /// that benefit from a resident grid even without explicit grid-sync.
    fn resolve_cooperative_flag(&self, config: &DispatchConfig) -> Result<bool, BackendError> {
        if !config.cooperative {
            return Ok(false);
        }
        if !self.hardware_supports_grid_sync() {
            return Err(BackendError::UnsupportedFeature {
                name: format!(
                    "cuda_cooperative_launch (compute_capability={:?}, cooperative_launch={})",
                    self.caps.compute_capability, self.caps.cooperative_launch
                ),
                backend: crate::CUDA_BACKEND_ID.to_string(),
            });
        }
        Ok(true)
    }

    fn enforce_config_caps(&self, config: &DispatchConfig) -> Result<(), BackendError> {
        if matches!(config.speculation, Some(SpeculationMode::Force)) {
            return Err(BackendError::UnsupportedFeature {
                name: "speculative dispatch".to_string(),
                backend: crate::CUDA_BACKEND_ID.to_string(),
            });
        }
        Ok(())
    }

    /// Pre-warmup: ensures the CUDA context is active.
    pub fn warmup(&self) -> Result<(), BackendError> {
        self.ctx
            .bind_to_thread()
            .map_err(|e| BackendError::DispatchFailed {
                code: None,
                message: format!("CUDA context bind failed: {e}"),
            })
    }

    /// Cleanup: sync and release cached modules.
    pub fn cleanup(&self) -> Result<(), BackendError> {
        self.warmup()?;
        self.ptx_source_cache.clear();
        self.module_cache.clear();
        self.resident_store.clear()?;
        self.transient_pool.clear()?;
        self.host_pool.clear()?;
        self.launch_resources.clear()?;
        Ok(())
    }

    pub(crate) fn with_resident<T>(
        &self,
        handle: CudaResidentBuffer,
        f: impl FnOnce(ResidentBufferView) -> Result<T, BackendError>,
    ) -> Result<T, BackendError> {
        self.warmup()?;
        let buffer = self.resident_store.view(handle)?;
        f(buffer)
    }

    pub(crate) fn resident_handles_from_resources(
        &self,
        resources: &[vyre_driver::Resource],
    ) -> Result<SmallVec<[CudaResidentBuffer; 8]>, BackendError> {
        self.resident_store.handles_from_resources(resources)
    }

    pub(crate) fn resident_handle_from_resource(
        &self,
        resource: &vyre_driver::Resource,
    ) -> Result<CudaResidentBuffer, BackendError> {
        self.resident_store.handle_from_resource(resource)
    }

    pub(crate) fn module_cache_key_for_ptx_source_key(
        &self,
        ptx_source_key: PtxSourceCacheKey,
    ) -> Result<ModuleCacheKey, BackendError> {
        self.module_cache
            .key_for_ptx_source_key(ptx_source_key, self.caps.compute_capability)
    }

    pub(crate) fn module_cache_key_for_raw_ptx_artifact(
        &self,
        raw_ptx_source: &str,
    ) -> Result<ModuleCacheKey, BackendError> {
        self.module_cache
            .key_for_raw_ptx_artifact(raw_ptx_source, self.caps.compute_capability)
    }

    pub(crate) fn module_for_ptx_with_key(
        &self,
        ptx_src: &str,
        key: ModuleCacheKey,
    ) -> Result<cudarc::driver::sys::CUfunction, BackendError> {
        self.module_cache
            .function_for_ptx(ptx_src, key, self.ptx_target_sm())
    }

    /// Device pointer + byte size of the cooperative grid-barrier counter for
    /// this PTX module, or `None` when the kernel declares no grid barrier.
    pub(crate) fn grid_barrier_global_with_key(
        &self,
        ptx_src: &str,
        key: ModuleCacheKey,
    ) -> Result<Option<(u64, usize)>, BackendError> {
        self.module_cache
            .grid_barrier_global_for_ptx(ptx_src, key, self.ptx_target_sm())
    }

    /// Number of loaded CUDA modules currently held in the warm cache.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] if the cache lock is poisoned.
    pub fn cached_module_count(&self) -> Result<usize, BackendError> {
        Ok(self.module_cache.len())
    }

    /// Compiled module cache counters for honest compile telemetry.
    #[must_use]
    pub fn pipeline_cache_snapshot(&self) -> vyre_driver::pipeline::PipelineCacheSnapshot {
        self.module_cache.snapshot()
    }

    /// PTX source cache counters for pre-module-load lowering telemetry.
    #[must_use]
    pub fn ptx_source_cache_snapshot(&self) -> CudaPtxSourceCacheSnapshot {
        self.ptx_source_cache.snapshot()
    }

    /// Runtime CUDA telemetry counters for launches, copies, readbacks, and syncs.
    #[must_use]
    pub fn telemetry_snapshot(&self) -> CudaTelemetrySnapshot {
        self.telemetry.snapshot()
    }

    /// Reset runtime CUDA telemetry counters without clearing caches or resident buffers.
    pub fn reset_telemetry(&self) {
        self.telemetry.reset();
    }

    /// Bytes of transient CUDA device memory retained for dispatch reuse.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] if the allocation-pool lock is poisoned.
    pub fn cached_transient_allocation_bytes(&self) -> Result<usize, BackendError> {
        self.transient_pool.cached_bytes()
    }

    /// Bytes of transient CUDA device memory currently owned by the transient pool.
    ///
    /// This includes both checked-out allocations and cached allocations retained for reuse.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] if allocation accounting cannot be read.
    pub fn allocated_transient_allocation_bytes(&self) -> Result<usize, BackendError> {
        self.transient_pool.allocated_bytes()
    }

    /// Cached CUDA streams/events retained for dispatch reuse.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] if a launch-resource pool lock is poisoned.
    pub fn cached_launch_resource_counts(&self) -> Result<(usize, usize), BackendError> {
        self.launch_resources.cached_counts()
    }

    /// Detailed cached CUDA launch resources retained for dispatch reuse,
    /// including timing-enabled events used by CUDA graph replay telemetry.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] if launch-resource accounting cannot be read.
    pub fn cached_launch_resource_counts_detailed(
        &self,
    ) -> Result<crate::CudaLaunchResourceCounts, BackendError> {
        self.launch_resources.cached_counts_detailed()
    }

    /// Snapshot the driver-tier observability surface
    /// ([`vyre_driver::observability::DriverObservability`]) plus the
    /// cuda module-cache count as a single backend metric.
    ///
    /// Operators scrape this in addition to per-substrate Prometheus
    /// counters when correlating substrate activity with backend
    /// resource usage.
    #[must_use]
    pub fn observability_snapshot(&self) -> vyre_driver::observability::DriverObservability {
        vyre_driver::observability::DriverObservability::snapshot()
    }

    /// PTX disk-cache directory path. Reuses the shared on-disk pipeline-cache
    /// layout, keyed by the VSA fingerprint.
    ///
    /// P-CUDA-2: PTX/CUBIN blobs persist across runs in this directory
    /// so first-run compile cost amortizes over the cluster.
    pub fn ptx_disk_cache_dir() -> Result<std::path::PathBuf, BackendError> {
        if let Some(path) = std::env::var_os("VYRE_PTX_CACHE_DIR") {
            let path = std::path::PathBuf::from(path);
            if path.as_os_str().is_empty() {
                return Err(BackendError::InvalidProgram {
                    fix: "Fix: VYRE_PTX_CACHE_DIR is empty. Set it to a writable persistent directory or unset it so XDG/HOME cache discovery can run."
                        .to_string(),
                });
            }
            return Ok(path);
        }
        if let Some(xdg) = std::env::var_os("XDG_CACHE_HOME") {
            return Ok(std::path::PathBuf::from(xdg).join("vyre").join("ptx-cache"));
        }
        if let Some(home) = std::env::var_os("HOME") {
            return Ok(std::path::PathBuf::from(home)
                .join(".cache")
                .join("vyre")
                .join("ptx-cache"));
        }
        Err(BackendError::InvalidProgram {
            fix: "Fix: CUDA PTX disk cache has no VYRE_PTX_CACHE_DIR, XDG_CACHE_HOME, or HOME. Configure a writable persistent cache root; temporary fallback is forbidden for production compile performance."
                .to_string(),
        })
    }

    /// Pre-lower and preload a CUDA pipeline for repeated dispatch.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when PTX lowering or CUDA module loading fails.
    pub fn compile_native(
        &self,
        program: &Program,
        config: &DispatchConfig,
    ) -> Result<std::sync::Arc<dyn vyre_driver::CompiledPipeline>, BackendError> {
        self.compile_native_shared(std::sync::Arc::new(program.clone()), config)
    }

    /// Pre-lower and preload a CUDA pipeline while preserving a caller-owned
    /// shared program allocation.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when PTX lowering or CUDA module loading fails.
    pub fn compile_native_shared(
        &self,
        program: std::sync::Arc<Program>,
        config: &DispatchConfig,
    ) -> Result<std::sync::Arc<dyn vyre_driver::CompiledPipeline>, BackendError> {
        let program = match vyre_foundation::transform::collectives::lower_single_rank_collectives(
            program.as_ref(),
        )
        .map_err(|error| BackendError::InvalidProgram {
            fix: error.to_string(),
        })? {
            Some(lowered) => Arc::new(lowered),
            None => program,
        };
        let trace = crate::instrumentation::cuda_stage_trace_enabled();
        let started = std::time::Instant::now();
        if trace {
            tracing::debug!(
                "[cuda-compile] start entry={}",
                program.entry_op_id.as_deref().unwrap_or("<anonymous>")
            );
        }
        let prepared = self.prepare_static_dispatch(program.as_ref(), config)?;
        if trace {
            tracing::debug!(
                "[cuda-compile] +{}ms prepare_static_dispatch buffers={} outputs={} elements={} grid={:?}",
                started.elapsed().as_millis(),
                prepared.bindings.bindings.len(),
                prepared.output_binding_indices.len(),
                prepared.launch.element_count,
                prepared.launch.grid
            );
        }
        let (ptx_src, ptx_source_key) =
            self.ptx_for_program_cached_with_key(program.as_ref(), config)?;
        if trace {
            tracing::debug!(
                "[cuda-compile] +{}ms ptx_source bytes={}",
                started.elapsed().as_millis(),
                ptx_src.len()
            );
        }
        let module_key = self.module_cache_key_for_ptx_source_key(ptx_source_key)?;
        self.warmup()?;
        if trace {
            tracing::debug!("[cuda-compile] +{}ms warmup", started.elapsed().as_millis());
        }
        self.module_for_ptx_with_key(&ptx_src, module_key)?;
        if trace {
            tracing::debug!(
                "[cuda-compile] +{}ms module ready",
                started.elapsed().as_millis()
            );
        }
        Ok(std::sync::Arc::new(
            crate::pipeline::CudaCompiledPipeline::new(
                self.clone(),
                program,
                ptx_src,
                ptx_source_key,
                module_key,
                config,
                prepared,
            )?,
        ))
    }
}
