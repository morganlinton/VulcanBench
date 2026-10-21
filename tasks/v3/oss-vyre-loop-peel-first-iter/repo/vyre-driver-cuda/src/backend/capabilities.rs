//! CUDA capability, feature-flag, and validation policy.

use std::sync::Arc;
use vyre_driver::binding::BindingRole;
use vyre_driver::validation::{LaunchGeometryLimits, ProgramValidationCaps};
use vyre_driver::{BackendError, DispatchConfig, LaunchPlan};
use vyre_foundation::ir::Program;
use vyre_foundation::validate::ValidationOptions;

use super::dispatch::CudaBackend;
use super::module_cache::PtxSourceCacheKey;
use super::plan::CudaDispatchPlan;
use crate::kernel_failure_diagnostics::{
    diagnose_cuda_kernel_launch_shape, CudaKernelDeviceEnvelope, CudaKernelLaunchEnvelope,
    CudaKernelLaunchShape,
};
use crate::numeric::CUDA_NUMERIC;
use crate::occupancy::cooperative_thread_residency_block_limit;

const CUDA_TRANSIENT_DISPATCH_BUDGET_NUMERATOR: u64 = 9;
const CUDA_TRANSIENT_DISPATCH_BUDGET_DENOMINATOR: u64 = 10;

impl CudaBackend {
    /// Compute capability as (major, minor).
    #[must_use]
    pub fn compute_capability(&self) -> (u32, u32) {
        self.caps.compute_capability
    }

    /// CUDA SM target number used by PTX emission.
    #[must_use]
    pub fn target_sm(&self) -> u32 {
        self.caps.native_sm()
    }

    /// CUDA SM target used by the current PTX ISA emitter.
    #[must_use]
    pub fn ptx_target_sm(&self) -> u32 {
        self.ptx_target_sm
    }

    /// Total device memory in bytes.
    #[must_use]
    pub fn device_memory_bytes(&self) -> u64 {
        self.caps.total_memory
    }

    /// Maximum number of threads per CUDA block.
    #[must_use]
    pub fn max_threads_per_block(&self) -> u32 {
        self.caps.max_threads_per_block_u32()
    }

    /// Maximum CUDA block dimensions.
    #[must_use]
    pub fn max_block_dim(&self) -> [u32; 3] {
        self.caps.max_block_dim_u32()
    }

    /// Maximum CUDA grid dimensions.
    #[must_use]
    pub fn max_grid_dim(&self) -> [u32; 3] {
        self.caps.max_grid_dim_u32()
    }

    /// Shared memory available per CUDA thread block in bytes.
    #[must_use]
    pub fn max_shared_memory_per_block_bytes(&self) -> u32 {
        self.caps.shared_memory_per_block_bytes()
    }

    /// CUDA warp size used by subgroup-style execution.
    #[must_use]
    pub fn warp_size(&self) -> Option<u32> {
        self.caps.warp_size_u32()
    }

    /// Whether the device has hardware subgroup/warp execution.
    #[must_use]
    pub fn hardware_supports_subgroup_ops(&self) -> bool {
        self.warp_size()
            .map(vyre_driver::SubgroupCaps::native)
            .is_some_and(|caps| caps.supports_subgroup)
    }

    /// Whether the device can execute asynchronous CUDA work concurrently.
    #[must_use]
    pub fn hardware_supports_async_compute(&self) -> bool {
        self.caps.concurrent_kernels || self.caps.async_engine_count > 0
    }

    /// Whether this device can run a cooperative whole-grid barrier.
    #[must_use]
    pub fn hardware_supports_grid_sync(&self) -> bool {
        self.caps.compute_capability >= (6, 0) && self.caps.cooperative_launch
    }

    /// Whether the device generation has native fp16 arithmetic support.
    #[must_use]
    pub fn hardware_supports_f16(&self) -> bool {
        self.caps.hardware_supports_f16()
    }

    /// Whether the device generation has native bf16 arithmetic support.
    #[must_use]
    pub fn hardware_supports_bf16(&self) -> bool {
        self.caps.hardware_supports_bf16()
    }

    /// Whether the device generation has NVIDIA tensor-core instructions.
    #[must_use]
    pub fn hardware_supports_tensor_cores(&self) -> bool {
        self.caps.hardware_supports_tensor_cores()
    }

    /// Whether this backend launches grid-sync kernels through the cooperative ABI.
    ///
    /// The PTX emitter lowers each `MemoryOrdering::GridSync` barrier to a
    /// monotonic-counter cooperative grid barrier
    /// (`vyre_emit_ptx` `emit_grid_sync_barrier`) backed by the module-scope
    /// `_vyre_grid_barrier` counter, and the host dispatch path launches such
    /// kernels with `cuLaunchCooperativeKernel`, zeroing the counter before
    /// each launch. `supports_grid_sync()` additionally gates on
    /// `hardware_supports_grid_sync()` so non-cooperative-capable devices still
    /// route to the kernel-split path; when the cooperative grid cannot be made
    /// fully resident the launch returns `CooperativeResidencyExceeded` and the
    /// orchestrator falls back to the resident-fixpoint path.
    #[must_use]
    pub fn lowers_grid_sync(&self) -> bool {
        true
    }

    /// Whether CUDA can execute `MemoryOrdering::GridSync` inside one dispatch.
    pub fn supports_grid_sync(&self) -> bool {
        self.hardware_supports_grid_sync() && self.lowers_grid_sync()
    }

    /// Whether CUDA PTX lowering emits tensor-core instructions.
    #[must_use]
    pub fn lowers_tensor_core_ops(&self) -> bool {
        true
    }

    /// Pipeline feature flags that participate in shared cache identity.
    #[must_use]
    pub fn pipeline_feature_flags(&self) -> vyre_driver::pipeline::PipelineFeatureFlags {
        let mut flags = vyre_driver::pipeline::PipelineFeatureFlags::empty();
        if self.hardware_supports_subgroup_ops() {
            flags = flags.union(vyre_driver::pipeline::PipelineFeatureFlags::SUBGROUP_OPS);
        }
        if self.hardware_supports_f16() {
            flags = flags.union(vyre_driver::pipeline::PipelineFeatureFlags::F16);
        }
        if self.hardware_supports_bf16() {
            flags = flags.union(vyre_driver::pipeline::PipelineFeatureFlags::BF16);
        }
        if self.hardware_supports_tensor_cores() && self.lowers_tensor_core_ops() {
            flags = flags.union(vyre_driver::pipeline::PipelineFeatureFlags::TENSOR_CORES);
        }
        if self.hardware_supports_async_compute() {
            flags = flags.union(vyre_driver::pipeline::PipelineFeatureFlags::ASYNC_COMPUTE);
        }
        flags
    }

    pub(crate) fn ptx_for_program_cached(
        &self,
        program: &Program,
        config: &DispatchConfig,
    ) -> Result<Arc<str>, BackendError> {
        self.ptx_for_program_cached_with_key(program, config)
            .map(|(ptx, _)| ptx)
    }

    pub(crate) fn ptx_for_program_cached_with_key(
        &self,
        program: &Program,
        config: &DispatchConfig,
    ) -> Result<(Arc<str>, PtxSourceCacheKey), BackendError> {
        let subgroup_size = self.warp_size().ok_or_else(|| BackendError::InvalidProgram {
            fix: "Fix: CUDA device probe reported no hardware warp size on a GPU-required host; fix the CUDA capability probe before lowering."
                .to_string(),
        })?;
        let lowered_program = vyre_foundation::lower::lower_subgroup_reductions(
            program.clone(),
            &self.caps.to_adapter_caps(),
        );
        let key = self.ptx_source_cache.key_for_program(
            &lowered_program,
            config,
            self.ptx_target_sm(),
            subgroup_size,
            self.pipeline_feature_flags(),
        )?;
        let ptx = self.ptx_source_cache.get_or_lower(key, || {
            crate::codegen::program_to_ptx_for_sm_and_subgroup(
                &lowered_program,
                config,
                self.ptx_target_sm(),
                subgroup_size,
            )
            .map_err(|compiler_message| BackendError::KernelCompileFailed {
                backend: crate::CUDA_BACKEND_ID.to_string(),
                compiler_message,
            })
        })?;
        Ok((ptx, key))
    }

    pub(crate) fn launch_limits(&self) -> LaunchGeometryLimits {
        LaunchGeometryLimits {
            backend: "CUDA",
            max_threads_per_block: self.max_threads_per_block(),
            max_block_dim: self.max_block_dim(),
            max_grid_dim: self.max_grid_dim(),
        }
    }

    /// Device capability envelope used by release launch diagnostics.
    #[must_use]
    pub fn kernel_device_envelope(&self) -> CudaKernelDeviceEnvelope {
        let sm_major = if self.caps.compute_capability.0 > u32::from(u16::MAX) {
            tracing::error!(
                "CUDA compute capability major value {} cannot fit u16. Fix: widen CudaKernelDeviceEnvelope before release diagnostics.",
                self.caps.compute_capability.0
            );
            u16::MAX
        } else {
            self.caps.compute_capability.0 as u16
        };
        let sm_minor = if self.caps.compute_capability.1 > u32::from(u16::MAX) {
            tracing::error!(
                "CUDA compute capability minor value {} cannot fit u16. Fix: widen CudaKernelDeviceEnvelope before release diagnostics.",
                self.caps.compute_capability.1
            );
            u16::MAX
        } else {
            self.caps.compute_capability.1 as u16
        };
        CudaKernelDeviceEnvelope {
            sm_major,
            sm_minor,
            max_threads_per_block: self.max_threads_per_block(),
            shared_memory_per_block_bytes: u64::from(self.max_shared_memory_per_block_bytes()),
            supports_cooperative_launch: self.hardware_supports_grid_sync(),
            supports_tensor_cores: self.hardware_supports_tensor_cores()
                && self.lowers_tensor_core_ops(),
        }
    }

    /// Build the release launch envelope for a prepared CUDA launch plan.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when launch shape products overflow the
    /// diagnostic envelope.
    pub fn diagnose_launch_plan(
        &self,
        kernel: &'static str,
        launch: &LaunchPlan,
        cooperative: bool,
        requires_tensor_cores: bool,
    ) -> Result<CudaKernelLaunchEnvelope, BackendError> {
        let threads_per_block = CUDA_NUMERIC
            .checked_dim_product_u64(launch.workgroup)
            .ok_or_else(|| BackendError::InvalidProgram {
                fix: format!(
                    "Fix: CUDA launch diagnostic workgroup product overflowed u64 for {:?}. Lower the workgroup before dispatch.",
                    launch.workgroup
                ),
            })?;
        let cooperative_resident_block_limit = if cooperative {
            let threads_per_block = u32::try_from(threads_per_block).map_err(|source| {
                BackendError::InvalidProgram {
                    fix: format!(
                        "Fix: CUDA launch diagnostic workgroup {:?} has {threads_per_block} thread slots, which does not fit u32: {source}. Lower the workgroup before dispatch.",
                        launch.workgroup
                    ),
                }
            })?;
            Some(cooperative_thread_residency_block_limit(
                &self.caps,
                threads_per_block,
            ))
        } else {
            None
        };
        diagnose_cuda_kernel_launch_shape(
            kernel,
            self.kernel_device_envelope(),
            CudaKernelLaunchShape {
                grid: launch.grid,
                block: launch.workgroup,
                dynamic_shared_memory_bytes: 0,
                cooperative,
                requires_tensor_cores,
            },
            cooperative_resident_block_limit,
        )
        .map_err(|error| BackendError::InvalidProgram { fix: error.fix })
    }

    pub(crate) fn program_validation_caps(&self) -> ProgramValidationCaps {
        ProgramValidationCaps {
            backend_id: crate::CUDA_BACKEND_ID,
            supports_subgroup_ops: self.hardware_supports_subgroup_ops(),
            supports_f16: self.hardware_supports_f16(),
            supports_bf16: self.hardware_supports_bf16(),
            supports_indirect_dispatch: false,
            supports_trap_propagation: true,
            supports_distributed_collectives: false,
            max_workgroup_size: self.max_block_dim(),
        }
    }

    pub(crate) fn validation_options(&self) -> ValidationOptions<'_> {
        ValidationOptions::default()
            .with_backend_capabilities(self.caps.to_device_profile().validation_capabilities())
            .with_shadowing(true)
    }

    pub(crate) fn validate_transient_dispatch_memory_budget(
        &self,
        prepared: &CudaDispatchPlan,
        inputs: &[&[u8]],
        context: &'static str,
    ) -> Result<(), BackendError> {
        let required_bytes = cuda_transient_dispatch_required_bytes(prepared, inputs)?;
        let budget_bytes = cuda_transient_dispatch_live_available_budget_bytes(
            self.caps.total_memory,
            cuda_live_free_memory_bytes()?,
            self.resident_store.allocated_bytes(),
            cuda_usize_bytes_to_u64(
                self.transient_pool.allocated_bytes()?,
                "transient pool allocated bytes",
            )?,
        );
        let budget_bytes = self
            .reclaim_cached_transient_allocations_when_over_budget(required_bytes, budget_bytes)?;
        validate_cuda_transient_dispatch_budget(required_bytes, budget_bytes, context)
    }

    pub(crate) fn validate_transient_allocation_memory_budget(
        &self,
        byte_len: usize,
        label: &'static str,
        context: &'static str,
    ) -> Result<(), BackendError> {
        let required_bytes = cuda_dispatch_allocation_bucket(byte_len, label)?;
        let budget_bytes = cuda_transient_dispatch_live_available_budget_bytes(
            self.caps.total_memory,
            cuda_live_free_memory_bytes()?,
            self.resident_store.allocated_bytes(),
            cuda_usize_bytes_to_u64(
                self.transient_pool.allocated_bytes()?,
                "transient pool allocated bytes",
            )?,
        );
        let budget_bytes = self
            .reclaim_cached_transient_allocations_when_over_budget(required_bytes, budget_bytes)?;
        validate_cuda_transient_dispatch_budget(required_bytes, budget_bytes, context)
    }

    fn reclaim_cached_transient_allocations_when_over_budget(
        &self,
        required_bytes: u64,
        budget_bytes: u64,
    ) -> Result<u64, BackendError> {
        if required_bytes <= budget_bytes {
            return Ok(budget_bytes);
        }
        self.transient_pool.clear()?;
        Ok(cuda_transient_dispatch_live_available_budget_bytes(
            self.caps.total_memory,
            cuda_live_free_memory_bytes()?,
            self.resident_store.allocated_bytes(),
            cuda_usize_bytes_to_u64(
                self.transient_pool.allocated_bytes()?,
                "transient pool allocated bytes after reclaim",
            )?,
        ))
    }

    pub(crate) fn validate_program_cached(&self, program: &Program) -> Result<(), BackendError> {
        if !crate::instrumentation::cuda_dispatch_validation_enabled() {
            return Ok(());
        }
        self.validation_cache.get_or_validate(
            program,
            self.validation_options(),
            crate::cuda_supported_ops(),
            self.program_validation_caps(),
        )
    }
}

pub(crate) fn cuda_transient_dispatch_budget_bytes(total_memory: u64) -> u64 {
    let numerator = u128::from(total_memory) * u128::from(CUDA_TRANSIENT_DISPATCH_BUDGET_NUMERATOR);
    (numerator / u128::from(CUDA_TRANSIENT_DISPATCH_BUDGET_DENOMINATOR)) as u64
}

pub(crate) fn cuda_live_free_memory_bytes() -> Result<u64, BackendError> {
    let (free, _total) = cudarc::driver::result::mem_get_info().map_err(|error| {
        BackendError::DispatchFailed {
            code: None,
            message: format!(
                "CUDA live-memory query failed: {error}. Fix: keep the CUDA context bound before memory preflight and treat query failure as a GPU release-path configuration error, not a CPU escape."
            ),
        }
    })?;
    cuda_usize_bytes_to_u64(free, "CUDA live free memory bytes")
}

pub(crate) fn cuda_transient_dispatch_available_budget_bytes(
    total_memory: u64,
    resident_bytes: u64,
    transient_pool_bytes: u64,
) -> u64 {
    let budget = u128::from(cuda_transient_dispatch_budget_bytes(total_memory));
    let used = u128::from(resident_bytes) + u128::from(transient_pool_bytes);
    if used >= budget {
        0
    } else {
        (budget - used) as u64
    }
}

pub(crate) fn cuda_transient_dispatch_live_available_budget_bytes(
    total_memory: u64,
    live_free_memory: u64,
    resident_bytes: u64,
    transient_pool_bytes: u64,
) -> u64 {
    let accounted = cuda_transient_dispatch_available_budget_bytes(
        total_memory,
        resident_bytes,
        transient_pool_bytes,
    );
    let live = cuda_transient_dispatch_budget_bytes(live_free_memory);
    accounted.min(live)
}

pub(crate) fn cuda_transient_dispatch_required_bytes(
    prepared: &CudaDispatchPlan,
    inputs: &[&[u8]],
) -> Result<u64, BackendError> {
    let mut required_bytes = 0u64;
    for binding in &prepared.bindings.bindings {
        if binding.role == BindingRole::Shared {
            continue;
        }
        let byte_len = match binding.input_index {
            Some(input_index) => inputs
                .get(input_index)
                .map(|input| input.len())
                .ok_or_else(|| BackendError::InvalidProgram {
                    fix: format!(
                        "Fix: CUDA dispatch memory preflight expected input index {input_index} for `{}` but only {} input(s) were supplied.",
                        binding.name,
                        inputs.len()
                    ),
                })?,
            None => binding.static_byte_len.ok_or_else(|| BackendError::InvalidProgram {
                fix: format!(
                    "Fix: CUDA dispatch memory preflight needs a static byte length for output `{}`; set BufferDecl::with_count or output_byte_range before launch.",
                    binding.name
                ),
            })?,
        };
        required_bytes = checked_dispatch_bytes_add(
            required_bytes,
            cuda_dispatch_allocation_bucket(byte_len, "CUDA dispatch buffer bytes")?,
            "CUDA dispatch buffer bytes",
        )?;
    }
    let param_bytes = super::launch_params::launch_param_byte_len(
        &prepared.launch.param_words,
        "dispatch memory preflight",
    )?;
    let param_allocation_bytes = if param_bytes == 0 {
        0
    } else {
        cuda_dispatch_allocation_bucket(param_bytes, "CUDA dispatch parameter bytes")?
    };
    checked_dispatch_bytes_add(
        required_bytes,
        param_allocation_bytes,
        "CUDA dispatch parameter bytes",
    )
}

pub(crate) fn validate_cuda_transient_dispatch_budget(
    required_bytes: u64,
    budget_bytes: u64,
    context: &'static str,
) -> Result<(), BackendError> {
    if required_bytes > budget_bytes {
        return Err(BackendError::InvalidProgram {
            fix: format!(
                "Fix: {context} requires {required_bytes} transient CUDA device bytes but the live-device preflight budget is {budget_bytes} bytes. Reduce input/output size, shard the dispatch, use resident handles with explicit reuse, or raise the CUDA memory budget deliberately."
            ),
        });
    }
    Ok(())
}

fn checked_dispatch_bytes_add(
    left: u64,
    right: u64,
    field: &'static str,
) -> Result<u64, BackendError> {
    left.checked_add(right)
        .ok_or_else(|| BackendError::InvalidProgram {
            fix: format!(
                "Fix: {field} overflowed u64 during CUDA memory preflight. Shard the dispatch before CUDA allocation."
            ),
        })
}

fn cuda_dispatch_allocation_bucket(
    byte_len: usize,
    field: &'static str,
) -> Result<u64, BackendError> {
    let bucket = byte_len
        .max(1)
        .checked_next_power_of_two()
        .ok_or_else(|| BackendError::InvalidProgram {
            fix: format!(
                "Fix: {field} request of {byte_len} bytes cannot be rounded to the CUDA allocation bucket. Shard the dispatch before CUDA allocation."
            ),
        })?;
    cuda_usize_bytes_to_u64(bucket, field)
}

fn cuda_usize_bytes_to_u64(byte_len: usize, field: &'static str) -> Result<u64, BackendError> {
    u64::try_from(byte_len).map_err(|_| BackendError::InvalidProgram {
        fix: format!(
            "Fix: {field} value of {byte_len} bytes cannot fit u64 CUDA memory telemetry. Shard the dispatch or widen budget accounting."
        ),
    })
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use smallvec::smallvec;
    use vyre_driver::binding::{Binding, BindingPlan, BindingRole};
    use vyre_driver::{BackendError, LaunchPlan};

    use super::{
        cuda_transient_dispatch_available_budget_bytes, cuda_transient_dispatch_budget_bytes,
        cuda_transient_dispatch_live_available_budget_bytes,
        cuda_transient_dispatch_required_bytes, validate_cuda_transient_dispatch_budget,
    };
    use crate::backend::CudaDispatchPlan;

    fn plan(static_output_bytes: usize) -> CudaDispatchPlan {
        CudaDispatchPlan {
            bindings: BindingPlan {
                bindings: vec![
                    Binding {
                        name: Arc::from("input"),
                        binding: 0,
                        buffer_index: 0,
                        role: BindingRole::Input,
                        element_size: 1,
                        preferred_alignment: 1,
                        element_count: 8,
                        static_byte_len: Some(8),
                        input_index: Some(0),
                        output_index: None,
                    },
                    Binding {
                        name: Arc::from("output"),
                        binding: 1,
                        buffer_index: 1,
                        role: BindingRole::Output,
                        element_size: 1,
                        preferred_alignment: 1,
                        element_count: u32::try_from(static_output_bytes).expect("Fix: CUDA parity tests require backend dispatch; skip test if GPU unavailable, do not panic - test CUDA dispatch plan static output bytes must fit u32 element count",
                        ),
                        static_byte_len: Some(static_output_bytes),
                        input_index: None,
                        output_index: Some(0),
                    },
                ],
                input_indices: vec![0],
                output_indices: vec![1],
                shared_indices: vec![],
            },
            output_binding_indices: smallvec![1],
            launch: LaunchPlan {
                grid: [1, 1, 1],
                workgroup: [32, 1, 1],
                element_count: 8,
                param_words: vec![1, 2],
                max_binding_alignment: 1,
            },
            cooperative: false,
            fixpoint_iterations: 1,
        }
    }

    #[test]
    fn transient_dispatch_memory_preflight_sums_buffers_and_params() {
        let input = [0u8; 8];
        let required = cuda_transient_dispatch_required_bytes(&plan(16), &[input.as_slice()])
            .expect("Fix: valid dispatch memory plan should sum");

        assert_eq!(required, 8 + 16 + 8);
    }

    #[test]
    fn transient_dispatch_memory_preflight_does_not_charge_empty_params() {
        let input = [0u8; 8];
        let mut plan = plan(16);
        plan.launch.param_words.clear();
        let required = cuda_transient_dispatch_required_bytes(&plan, &[input.as_slice()])
            .expect("Fix: valid zero-param dispatch memory plan should sum");

        assert_eq!(
            required,
            8 + 16,
            "Fix: CUDA memory preflight must not charge a rounded one-byte allocation for empty launch params."
        );
    }

    #[test]
    fn transient_dispatch_memory_preflight_counts_bucketed_allocation_pressure() {
        let input = [0u8; 9];
        let required = cuda_transient_dispatch_required_bytes(&plan(17), &[input.as_slice()])
            .expect("Fix: valid dispatch memory plan should sum bucketed allocation pressure");

        assert_eq!(required, 16 + 32 + 8);
    }

    #[test]
    fn transient_dispatch_memory_preflight_rejects_over_budget_before_allocation() {
        let error = validate_cuda_transient_dispatch_budget(1025, 1024, "CUDA test dispatch")
            .expect_err("over-budget dispatch must fail before CUDA allocation");

        match error {
            BackendError::InvalidProgram { fix } => {
                assert!(fix.contains("CUDA test dispatch requires 1025"));
                assert!(fix.contains("preflight budget is 1024"));
                assert!(fix.contains("Shard") || fix.contains("shard"));
            }
            other => panic!("expected InvalidProgram, got {other:?}"),
        }
    }

    #[test]
    fn transient_dispatch_budget_uses_conservative_live_vram_fraction() {
        assert_eq!(cuda_transient_dispatch_budget_bytes(1000), 900);
        assert_eq!(
            cuda_transient_dispatch_budget_bytes(u64::MAX),
            16_602_069_666_338_596_453,
            "Fix: CUDA transient budget must widen before multiplying so huge live-memory probes do not saturate before division."
        );
    }

    #[test]
    fn transient_dispatch_available_budget_subtracts_live_resident_and_transient_pool_bytes() {
        assert_eq!(
            cuda_transient_dispatch_available_budget_bytes(1000, 300, 200),
            400
        );
        assert_eq!(
            cuda_transient_dispatch_available_budget_bytes(1000, 1_000, 0),
            0
        );
        assert_eq!(
            cuda_transient_dispatch_available_budget_bytes(1000, 0, 1_000),
            0
        );
        assert_eq!(
            cuda_transient_dispatch_available_budget_bytes(u64::MAX, u64::MAX, u64::MAX),
            0,
            "Fix: CUDA transient available-budget subtraction must use widened arithmetic and clamp only after exact live-usage comparison."
        );
        let source = include_str!("capabilities.rs");
        assert!(source.contains("CUDA_NUMERIC"));
        assert!(source.contains("checked_dim_product_u64"));
        assert!(!source.contains(concat!("vyre_driver::numeric::", "checked_dim_product_u64")));
        assert!(
            !source.contains(concat!(".", "saturating_mul"))
                && !source.contains(concat!(".", "saturating_sub")),
            "Fix: CUDA transient budget math must be exact/widened, not saturating."
        );
    }

    #[test]
    fn transient_dispatch_live_available_budget_caps_against_free_vram() {
        assert_eq!(
            cuda_transient_dispatch_live_available_budget_bytes(10_000, 1_000, 0, 0),
            900,
            "Fix: CUDA preflight must cap dispatch pressure against live free VRAM, not just total board memory."
        );
        assert_eq!(
            cuda_transient_dispatch_live_available_budget_bytes(10_000, 8_000, 2_000, 1_000),
            6_000,
            "Fix: CUDA preflight must still subtract resident and transient allocations from the total-device budget."
        );
        assert_eq!(
            cuda_transient_dispatch_live_available_budget_bytes(10_000, 0, 0, 0),
            0,
            "Fix: zero live free VRAM must produce zero preflight budget instead of allowing optimistic allocation."
        );
    }

    #[test]
    fn cuda_dispatch_validation_is_release_default_not_opt_in() {
        let source = include_str!("../instrumentation.rs");
        let capabilities_source = include_str!("capabilities.rs");

        assert!(
            source.contains("VYRE_CUDA_VALIDATE_DISPATCH")
                && source.contains("cached_enabled_default_true")
                && source.contains("CUDA_VALIDATE_DISPATCH_DISABLED"),
            "Fix: CUDA dispatch validation must be default-on with only an explicit debug disable."
        );
        assert!(
            capabilities_source
                .contains("crate::instrumentation::cuda_dispatch_validation_enabled()")
                && !capabilities_source.contains("std::env::var(\"VYRE_CUDA_VALIDATE_DISPATCH\")")
                && !capabilities_source.contains("var_os(\"VYRE_CUDA_VALIDATE_DISPATCH\")"),
            "Fix: CUDA dispatch validation must not be an opt-in release-path correctness gate."
        );
    }
}
