//! CUDA device probing and capability snapshots.

use std::{fmt, sync::Arc};

use cudarc::driver::{result, sys::CUdevice_attribute, CudaContext};

use crate::backend::staging_reserve::reserved_vec;

fn format_cuda_context_init_error(ordinal: usize, error: impl fmt::Display) -> String {
    format!(
        "CUDA context init failed for ordinal {ordinal}: {error}. Fix: choose a visible `nvidia-smi -L` ordinal and ensure no exclusive-process compute mode blocks context creation. If the error is CUDA_ERROR_OUT_OF_MEMORY, treat it as live VRAM pressure during context creation: run `nvidia-smi --query-compute-apps=pid,process_name,used_memory --format=csv`, free or move the processes holding VRAM, then rerun the CUDA-required validation; do not skip GPU tests or continue on a CPU path."
    )
}

#[cfg(test)]
mod context_init_error_tests {
    use super::format_cuda_context_init_error;

    #[test]
    fn context_init_oom_diagnostic_names_vram_pressure_without_cpu_escape() {
        let diagnostic = format_cuda_context_init_error(0, "CUDA_ERROR_OUT_OF_MEMORY");
        assert!(diagnostic.contains("CUDA_ERROR_OUT_OF_MEMORY"));
        assert!(diagnostic.contains("live VRAM pressure during context creation"));
        assert!(diagnostic
            .contains("nvidia-smi --query-compute-apps=pid,process_name,used_memory --format=csv"));
        assert!(diagnostic.contains("do not skip GPU tests"));
        assert!(diagnostic.contains("continue on a CPU path"));
    }
}

/// Queried physical limits and capabilities of a CUDA GPU.
#[derive(Debug, Clone)]
pub struct CudaDeviceCaps {
    /// The device vendor name.
    pub name: String,
    /// The physical device index.
    pub ordinal: usize,
    /// Hardware compute capability (major, minor).
    pub compute_capability: (u32, u32),
    /// Overall VRAM capacity in bytes.
    pub total_memory: u64,
    /// Maximum number of threads executable in one block.
    pub max_threads_per_block: i32,
    /// Maximum dimensions for a thread block (x, y, z).
    pub max_block_dim: [i32; 3],
    /// Maximum dimensions for a dispatch grid (x, y, z).
    pub max_grid_dim: [i32; 3],
    /// Shared memory available per thread block in bytes.
    pub shared_memory_per_block: i32,
    /// Shared memory available per streaming multiprocessor in bytes.
    pub shared_memory_per_sm: i32,
    /// Number of threads in a hardware warp.
    pub warp_size: i32,
    /// Whether the device supports cooperative grid launches (megakernel prerequisite).
    pub cooperative_launch: bool,
    /// Whether the device can run multiple kernels concurrently from different streams.
    pub concurrent_kernels: bool,
    /// Number of independent async copy engines available.
    pub async_engine_count: i32,
    /// Number of streaming multiprocessors. Used by runtime planners that
    /// need to size concurrent graph-replay lanes against real hardware
    /// width instead of a fixed host-side constant.
    pub multi_processor_count: i32,
    /// Device-wide L2 cache capacity in bytes.
    pub l2_cache_bytes: i32,
    /// Memory clock rate in kHz, as reported by the CUDA driver.
    pub memory_clock_rate_khz: i32,
    /// Global memory bus width in bits.
    pub global_memory_bus_width_bits: i32,
    /// Maximum 32-bit registers usable by a single thread block. Required
    /// for occupancy-aware workgroup sizing (I4)  -  when ptxas reports a
    /// kernel's per-thread register pressure, this caps the largest block
    /// the driver can launch without spill.
    pub max_registers_per_block: i32,
    /// Maximum 32-bit registers available per streaming multiprocessor.
    /// Combined with kernel register pressure this gives the per-SM block
    /// concurrency limit for the I4 occupancy estimator.
    pub max_registers_per_sm: i32,
    /// Maximum threads resident on a streaming multiprocessor.
    /// `max_threads_per_sm / workgroup_size` is the upper bound on
    /// concurrent blocks per SM before register or shared-memory limits
    /// kick in.
    pub max_threads_per_sm: i32,
}

/// Centralized live CUDA device acquisition result.
#[derive(Debug, Clone)]
pub struct CudaDeviceHandle {
    /// Probed capabilities for the acquired device.
    pub caps: CudaDeviceCaps,
    /// Bound CUDA context for dispatch.
    pub ctx: Arc<CudaContext>,
}

impl CudaDeviceHandle {
    /// Acquire and bind a CUDA context for `ordinal`, returning the matching
    /// capability snapshot from the same CUDA device handle.
    ///
    /// # Errors
    ///
    /// Returns an actionable error when the CUDA driver cannot initialize, the
    /// ordinal is invalid, context creation fails, context binding fails, or a
    /// required device attribute cannot be queried.
    pub fn acquire_ordinal(ordinal: usize) -> Result<Self, String> {
        let device_count = CudaDeviceCaps::visible_device_count()?;
        if ordinal >= device_count {
            return Err(format!(
                "CUDA device ordinal {ordinal} is out of range for {device_count} visible device(s). Fix: select a CUDA device ordinal reported by `nvidia-smi`."
            ));
        }

        let ctx = CudaContext::new(ordinal)
            .map_err(|error| format_cuda_context_init_error(ordinal, error))?;
        ctx.bind_to_thread().map_err(|e| {
            format!(
                "CUDA context bind failed for ordinal {ordinal}: {e}. Fix: repair CUDA context ownership before dispatch; GPU-required runs must not continue with an unbound context."
            )
        })?;
        let caps = CudaDeviceCaps::probe_context(ordinal, &ctx)?;
        Ok(Self { caps, ctx })
    }
}

impl CudaDeviceCaps {
    fn required_u32_capability(&self, name: &str, value: i32) -> u32 {
        debug_assert!(
            value > 0,
            "CUDA device `{}` carried invalid {name}={value} after capability validation",
            self.name
        );
        if value <= 0 {
            tracing::error!(
                "CUDA device `{}` carried invalid {name}={value} after capability validation. Fix: reject corrupt capability snapshots during probe.",
                self.name
            );
            return 0;
        }
        u32::try_from(value).unwrap_or_else(|source| {
            tracing::error!(
                "CUDA device `{}` carried non-u32 {name}={value} after capability validation: {source}. Fix: reject corrupt capability snapshots during probe.",
                self.name
            );
            0
        })
    }

    /// Return the number of CUDA devices visible to the CUDA driver.
    ///
    /// # Errors
    ///
    /// Returns an error when the CUDA driver cannot initialize or report its
    /// visible device count.
    pub fn visible_device_count() -> Result<usize, String> {
        result::init().map_err(|e| {
            format!(
                "CUDA driver init failed: {e}. Fix: verify `nvidia-smi` succeeds and libcuda.so from the NVIDIA driver is visible to this process."
            )
        })?;
        let count = result::device::get_count()
            .map_err(|e| {
                format!(
                    "CUDA device-count query failed: {e}. Fix: repair CUDA driver/device visibility; a GPU-required host must not report zero devices."
                )
            })?;
        usize::try_from(count)
            .map_err(|_| format!("CUDA device-count query returned negative value {count}"))
    }

    /// Probe every CUDA device visible to the process.
    ///
    /// # Errors
    ///
    /// Returns an actionable error when any visible device cannot be probed.
    pub fn probe_all() -> Result<Vec<Self>, String> {
        let device_count = Self::visible_device_count()?;
        if device_count == 0 {
            return Err(
                "CUDA device-count query returned zero visible devices. Fix: this is a GPU-required release host; run `nvidia-smi -L`, repair CUDA_VISIBLE_DEVICES/container GPU passthrough, and do not silently continue on a CPU path."
                    .to_string(),
            );
        }
        let mut devices = reserved_vec(device_count, "cuda visible device probes")
            .map_err(|error| error.to_string())?;
        for ordinal in 0..device_count {
            devices.push(Self::probe(ordinal)?);
        }
        Ok(devices)
    }

    /// Probe the device using the raw CUDA driver API.
    ///
    /// # Errors
    ///
    /// Returns an error when the CUDA driver cannot initialize, the ordinal is
    /// out of range, or a required device attribute cannot be queried.
    pub fn probe(ordinal: usize) -> Result<Self, String> {
        let device_count = Self::visible_device_count()?;
        if ordinal >= device_count {
            return Err(format!(
                "CUDA device ordinal {ordinal} is out of range for {device_count} visible device(s). Fix: select a CUDA device ordinal reported by `nvidia-smi`."
            ));
        }

        let ctx = CudaContext::new(ordinal)
            .map_err(|error| format_cuda_context_init_error(ordinal, error))?;
        Self::probe_context(ordinal, &ctx)
    }

    fn probe_context(ordinal: usize, ctx: &CudaContext) -> Result<Self, String> {
        let dev = ctx.cu_device();

        let attr = |name: &str, attrib| {
            // SAFETY: cuDeviceGetCount / cuDeviceGet operate on raw pointers we own on
            // the current thread; the call returns CUresult and is wrapped in cuda_check.
            unsafe { result::device::get_attribute(dev, attrib) }
                .map_err(|e| format!("CUDA attribute query `{name}` failed: {e}"))
        };

        // SAFETY: cuDeviceGetCount / cuDeviceGet operate on raw pointers we own on
        // the current thread; the call returns CUresult and is wrapped in cuda_check.
        let total_memory = unsafe { result::device::total_mem(dev) }
            .map_err(|e| format!("CUDA total-memory query failed: {e}"))?;
        let name = result::device::get_name(dev)
            .map_err(|e| format!("CUDA device-name query failed: {e}"))?;
        let major = attr(
            "compute_capability_major",
            CUdevice_attribute::CU_DEVICE_ATTRIBUTE_COMPUTE_CAPABILITY_MAJOR,
        )?;
        let minor = attr(
            "compute_capability_minor",
            CUdevice_attribute::CU_DEVICE_ATTRIBUTE_COMPUTE_CAPABILITY_MINOR,
        )?;
        let max_threads_per_block = attr(
            "max_threads_per_block",
            CUdevice_attribute::CU_DEVICE_ATTRIBUTE_MAX_THREADS_PER_BLOCK,
        )?;
        let max_block_dim_x = attr(
            "max_block_dim_x",
            CUdevice_attribute::CU_DEVICE_ATTRIBUTE_MAX_BLOCK_DIM_X,
        )?;
        let max_block_dim_y = attr(
            "max_block_dim_y",
            CUdevice_attribute::CU_DEVICE_ATTRIBUTE_MAX_BLOCK_DIM_Y,
        )?;
        let max_block_dim_z = attr(
            "max_block_dim_z",
            CUdevice_attribute::CU_DEVICE_ATTRIBUTE_MAX_BLOCK_DIM_Z,
        )?;
        let max_grid_dim_x = attr(
            "max_grid_dim_x",
            CUdevice_attribute::CU_DEVICE_ATTRIBUTE_MAX_GRID_DIM_X,
        )?;
        let max_grid_dim_y = attr(
            "max_grid_dim_y",
            CUdevice_attribute::CU_DEVICE_ATTRIBUTE_MAX_GRID_DIM_Y,
        )?;
        let max_grid_dim_z = attr(
            "max_grid_dim_z",
            CUdevice_attribute::CU_DEVICE_ATTRIBUTE_MAX_GRID_DIM_Z,
        )?;
        let shared_memory_per_block = attr(
            "shared_memory_per_block",
            CUdevice_attribute::CU_DEVICE_ATTRIBUTE_MAX_SHARED_MEMORY_PER_BLOCK,
        )?;
        let shared_memory_per_sm = attr(
            "shared_memory_per_sm",
            CUdevice_attribute::CU_DEVICE_ATTRIBUTE_MAX_SHARED_MEMORY_PER_MULTIPROCESSOR,
        )?;
        let warp_size = attr(
            "warp_size",
            CUdevice_attribute::CU_DEVICE_ATTRIBUTE_WARP_SIZE,
        )?;
        let cooperative_launch = attr(
            "cooperative_launch",
            CUdevice_attribute::CU_DEVICE_ATTRIBUTE_COOPERATIVE_LAUNCH,
        )?;
        let concurrent_kernels = attr(
            "concurrent_kernels",
            CUdevice_attribute::CU_DEVICE_ATTRIBUTE_CONCURRENT_KERNELS,
        )?;
        let async_engine_count = attr(
            "async_engine_count",
            CUdevice_attribute::CU_DEVICE_ATTRIBUTE_ASYNC_ENGINE_COUNT,
        )?;
        let multi_processor_count = attr(
            "multi_processor_count",
            CUdevice_attribute::CU_DEVICE_ATTRIBUTE_MULTIPROCESSOR_COUNT,
        )?;
        let l2_cache_bytes = attr(
            "l2_cache_bytes",
            CUdevice_attribute::CU_DEVICE_ATTRIBUTE_L2_CACHE_SIZE,
        )?;
        let memory_clock_rate_khz = attr(
            "memory_clock_rate_khz",
            CUdevice_attribute::CU_DEVICE_ATTRIBUTE_MEMORY_CLOCK_RATE,
        )?;
        let global_memory_bus_width_bits = attr(
            "global_memory_bus_width_bits",
            CUdevice_attribute::CU_DEVICE_ATTRIBUTE_GLOBAL_MEMORY_BUS_WIDTH,
        )?;
        let max_registers_per_block = attr(
            "max_registers_per_block",
            CUdevice_attribute::CU_DEVICE_ATTRIBUTE_MAX_REGISTERS_PER_BLOCK,
        )?;
        let max_registers_per_sm = attr(
            "max_registers_per_sm",
            CUdevice_attribute::CU_DEVICE_ATTRIBUTE_MAX_REGISTERS_PER_MULTIPROCESSOR,
        )?;
        let max_threads_per_sm = attr(
            "max_threads_per_sm",
            CUdevice_attribute::CU_DEVICE_ATTRIBUTE_MAX_THREADS_PER_MULTIPROCESSOR,
        )?;

        let caps = Self {
            name,
            ordinal,
            compute_capability: (
                u32::try_from(major).map_err(|source| {
                    format!("CUDA device major compute capability was negative ({major}): {source}. Fix: repair CUDA driver attribute probing before dispatch.")
                })?,
                u32::try_from(minor).map_err(|source| {
                    format!("CUDA device minor compute capability was negative ({minor}): {source}. Fix: repair CUDA driver attribute probing before dispatch.")
                })?,
            ),
            total_memory: u64::try_from(total_memory).map_err(|source| {
                format!("CUDA total memory value {total_memory} does not fit u64: {source}. Fix: widen CudaDeviceCaps memory telemetry before dispatch.")
            })?,
            max_threads_per_block,
            max_block_dim: [max_block_dim_x, max_block_dim_y, max_block_dim_z],
            max_grid_dim: [max_grid_dim_x, max_grid_dim_y, max_grid_dim_z],
            shared_memory_per_block,
            shared_memory_per_sm,
            warp_size,
            cooperative_launch: cooperative_launch != 0,
            concurrent_kernels: concurrent_kernels != 0,
            async_engine_count,
            multi_processor_count,
            l2_cache_bytes,
            memory_clock_rate_khz,
            global_memory_bus_width_bits,
            max_registers_per_block,
            max_registers_per_sm,
            max_threads_per_sm,
        };
        caps.validate_required_attributes()?;
        Ok(caps)
    }

    fn validate_required_attributes(&self) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err(format!(
                "CUDA device ordinal {} returned an empty device name. Fix: repair CUDA driver probing before capability-dependent dispatch.",
                self.ordinal
            ));
        }
        if self.compute_capability.0 == 0 {
            return Err(format!(
                "CUDA device `{}` returned invalid compute capability {:?}. Fix: update the NVIDIA driver so CUDA attributes report a real SM target.",
                self.name, self.compute_capability
            ));
        }
        if self.total_memory == 0 {
            return Err(format!(
                "CUDA device `{}` reported zero total memory. Fix: repair CUDA device visibility; do not continue with bogus memory limits.",
                self.name
            ));
        }
        for (name, value) in [
            ("max_threads_per_block", self.max_threads_per_block),
            ("max_block_dim_x", self.max_block_dim[0]),
            ("max_block_dim_y", self.max_block_dim[1]),
            ("max_block_dim_z", self.max_block_dim[2]),
            ("max_grid_dim_x", self.max_grid_dim[0]),
            ("max_grid_dim_y", self.max_grid_dim[1]),
            ("max_grid_dim_z", self.max_grid_dim[2]),
            ("shared_memory_per_block", self.shared_memory_per_block),
            ("shared_memory_per_sm", self.shared_memory_per_sm),
            ("warp_size", self.warp_size),
            ("multi_processor_count", self.multi_processor_count),
            ("l2_cache_bytes", self.l2_cache_bytes),
            ("memory_clock_rate_khz", self.memory_clock_rate_khz),
            (
                "global_memory_bus_width_bits",
                self.global_memory_bus_width_bits,
            ),
            ("max_registers_per_block", self.max_registers_per_block),
            ("max_registers_per_sm", self.max_registers_per_sm),
            ("max_threads_per_sm", self.max_threads_per_sm),
        ] {
            if value <= 0 {
                return Err(format!(
                    "CUDA device `{}` reported invalid {name}={value}. Fix: repair CUDA capability probing before dispatch; zero/negative limits are a hard GPU configuration error.",
                    self.name
                ));
            }
        }
        Ok(())
    }

    /// Native CUDA SM number reported by the device compute capability.
    #[must_use]
    pub fn native_sm(&self) -> u32 {
        self.compute_capability.0 * 10 + self.compute_capability.1
    }

    /// PTX `.target sm_XX` selected for this device.
    ///
    /// The CUDA driver JIT accepts virtual PTX for the current architecture.
    /// Capping this value below the live device hides architecture-specific
    /// scheduling and invalidates cache keys across GPU generations.
    #[must_use]
    pub fn ptx_target_sm(&self) -> u32 {
        self.native_sm()
    }

    /// Shared memory available per CUDA thread block in bytes.
    #[must_use]
    pub fn shared_memory_per_block_bytes(&self) -> u32 {
        self.required_u32_capability("shared_memory_per_block", self.shared_memory_per_block)
    }

    /// Shared memory available per CUDA streaming multiprocessor in bytes.
    #[must_use]
    pub fn shared_memory_per_sm_bytes(&self) -> u32 {
        self.required_u32_capability("shared_memory_per_sm", self.shared_memory_per_sm)
    }

    /// Maximum threads per block as an unsigned launch-limit value.
    #[must_use]
    pub fn max_threads_per_block_u32(&self) -> u32 {
        self.required_u32_capability("max_threads_per_block", self.max_threads_per_block)
    }

    /// Maximum 32-bit registers per CUDA thread block.
    #[must_use]
    pub fn max_registers_per_block_u32(&self) -> u32 {
        self.required_u32_capability("max_registers_per_block", self.max_registers_per_block)
    }

    /// Maximum 32-bit registers per streaming multiprocessor.
    #[must_use]
    pub fn max_registers_per_sm_u32(&self) -> u32 {
        self.required_u32_capability("max_registers_per_sm", self.max_registers_per_sm)
    }

    /// Maximum resident threads per streaming multiprocessor.
    #[must_use]
    pub fn max_threads_per_sm_u32(&self) -> u32 {
        self.required_u32_capability("max_threads_per_sm", self.max_threads_per_sm)
    }

    /// Number of streaming multiprocessors as an unsigned runtime-planning value.
    #[must_use]
    pub fn multi_processor_count_u32(&self) -> u32 {
        self.required_u32_capability("multi_processor_count", self.multi_processor_count)
    }

    /// Device-wide L2 cache capacity in bytes.
    #[must_use]
    pub fn l2_cache_bytes_u32(&self) -> u32 {
        self.required_u32_capability("l2_cache_bytes", self.l2_cache_bytes)
    }

    /// Approximate peak global-memory bandwidth in decimal GB/s.
    #[must_use]
    pub fn memory_bandwidth_gbps(&self) -> u32 {
        let clock_khz =
            self.required_u32_capability("memory_clock_rate_khz", self.memory_clock_rate_khz);
        let bus_bits = self.required_u32_capability(
            "global_memory_bus_width_bits",
            self.global_memory_bus_width_bits,
        );
        let gbps = (u64::from(clock_khz) * u64::from(bus_bits)) / 4_000_000;
        u32::try_from(gbps.max(1)).unwrap_or_else(|source| {
            tracing::error!(
                "CUDA device `{}` memory bandwidth {gbps} GB/s does not fit u32: {source}. Fix: normalize the bandwidth model before exporting device profile telemetry.",
                self.name
            );
            u32::MAX
        })
    }

    /// NVIDIA CUDA architectural register ceiling per thread.
    #[must_use]
    pub fn max_registers_per_thread_u32(&self) -> u32 {
        self.max_registers_per_block_u32().min(255)
    }

    /// Per-axis block limits as unsigned launch-limit values.
    #[must_use]
    pub fn max_block_dim_u32(&self) -> [u32; 3] {
        self.max_block_dim
            .map(|value| self.required_u32_capability("max_block_dim axis", value))
    }

    /// Per-axis grid limits as unsigned launch-limit values.
    #[must_use]
    pub fn max_grid_dim_u32(&self) -> [u32; 3] {
        self.max_grid_dim
            .map(|value| self.required_u32_capability("max_grid_dim axis", value))
    }

    /// Warp width reported by the CUDA device.
    #[must_use]
    pub fn warp_size_u32(&self) -> Option<u32> {
        Some(self.required_warp_size_u32())
    }

    /// Warp width reported by the CUDA device after probe-time validation.
    #[must_use]
    pub fn required_warp_size_u32(&self) -> u32 {
        self.required_u32_capability("warp_size", self.warp_size)
    }

    /// Whether this device generation has native fp16 instructions.
    #[must_use]
    pub fn hardware_supports_f16(&self) -> bool {
        self.compute_capability >= (5, 3)
    }

    /// Whether this device generation has native bf16 instructions.
    #[must_use]
    pub fn hardware_supports_bf16(&self) -> bool {
        self.compute_capability >= (8, 0)
    }

    /// Whether this device generation exposes NVIDIA tensor-core instructions.
    #[must_use]
    pub fn hardware_supports_tensor_cores(&self) -> bool {
        self.compute_capability >= (7, 0)
    }

    /// Project a CUDA device snapshot into the workspace-wide
    /// [`vyre_foundation::optimizer::AdapterCaps`] (audit P0 #60). All vyre
    /// backends consume the same typed capability shape so passes that
    /// adapt to subgroup-ops, indirect dispatch, max workgroup size, or
    /// shared-memory budget take a single typed input regardless of
    /// backend identity.
    ///
    /// Mapping notes:
    /// - `supports_subgroup_ops`: CUDA always supports warp shuffles
    ///   (`__shfl_*`) on every supported architecture (compute capability
    ///   ≥ 3.0), so this is `true`.
    /// - `supports_indirect_dispatch`: CUDA exposes
    ///   `cuLaunchKernelEx` and `cuLaunchCooperativeKernel` with
    ///   indirect launch parameters; `true` when cooperative launch is
    ///   reported (the megakernel prerequisite that exercises this).
    /// - `supports_specialization_constants`: CUDA uses runtime kernel
    ///   parameters rather than pipeline-creation specialization constants;
    ///   surfaced as `false`.
    /// - `subgroup_size`: warp size (32 on every shipping NVIDIA GPU,
    ///   but probed live so future architectures stay correct).
    #[must_use]
    pub fn to_adapter_caps(&self) -> vyre_foundation::optimizer::AdapterCaps {
        self.to_device_profile().into()
    }

    /// Project the probed device into the neutral driver profile.
    #[must_use]
    pub fn to_device_profile(&self) -> vyre_driver::DeviceProfile {
        let subgroup = self.subgroup_caps();
        let profile = vyre_driver::DeviceProfile {
            backend: "cuda",
            supports_subgroup_ops: subgroup.supports_subgroup,
            supports_indirect_dispatch: self.cooperative_launch,
            supports_distributed_collectives: false,
            supports_specialization_constants: false,
            supports_f16: self.hardware_supports_f16(),
            supports_bf16: self.hardware_supports_bf16(),
            supports_trap_propagation: true,
            supports_tensor_cores: self.hardware_supports_tensor_cores(),
            has_mul_high: true,
            has_dual_issue_fp32_int32: true,
            has_subgroup_shuffle: subgroup.supports_subgroup,
            has_shared_memory: self.shared_memory_per_block_bytes() > 0,
            max_native_int_width: 64,
            max_workgroup_size: self.max_block_dim_u32(),
            max_invocations_per_workgroup: self.max_threads_per_block_u32(),
            max_shared_memory_bytes: self.shared_memory_per_block_bytes(),
            max_storage_buffer_binding_size: self.total_memory,
            subgroup_size: subgroup.subgroup_size,
            compute_units: self.multi_processor_count_u32(),
            regs_per_thread_max: self.max_registers_per_thread_u32(),
            l1_cache_bytes: 0,
            l2_cache_bytes: self.l2_cache_bytes_u32(),
            mem_bw_gbps: self.memory_bandwidth_gbps(),
            timing_quality: vyre_driver::DeviceTimingQuality::DeviceTimestamps,
            supports_device_timestamps: true,
            supports_hardware_counters: false,
            ideal_unroll_depth: 0,
            ideal_vector_pack_bits: 0,
            ideal_workgroup_tile: [0, 0, 0],
            shared_memory_bank_count: 32,
            shared_memory_bank_width_bytes: 4,
        };
        vyre_driver::DeviceSignatureTable::builtins().map_or(profile, |table| {
            table.apply_generation_to_profile(self.native_sm(), profile)
        })
    }

    /// Project CUDA warp capabilities into the shared subgroup record.
    #[must_use]
    pub fn subgroup_caps(&self) -> vyre_driver::SubgroupCaps {
        vyre_driver::SubgroupCaps::native(self.required_warp_size_u32())
    }
}

#[cfg(test)]

mod tests {
    use crate::synthetic_device_caps::blackwell_sm120_caps_default;

    #[test]
    fn cuda_profile_applies_builtin_sm_signature() {
        let profile = blackwell_sm120_caps_default().to_device_profile();
        let table =
            vyre_driver::DeviceSignatureTable::builtins().expect("Fix: builtin signatures load");
        let signature = table
            .find_architecture_generation(120)
            .expect("Fix: SM_120 must match the builtin Blackwell signature");

        assert_eq!(profile.compute_units, 170);
        assert_eq!(profile.ideal_unroll_depth, signature.ideal_unroll_depth);
        assert_eq!(
            profile.ideal_vector_pack_bits,
            signature.ideal_vector_pack_bits
        );
        assert_eq!(profile.ideal_workgroup_tile, signature.ideal_workgroup_tile);
        assert_eq!(profile.shared_memory_bank_count, signature.bank_count);
    }

    #[test]
    fn cuda_profile_preserves_probed_compute_units_without_builtin_signature() {
        let mut caps = blackwell_sm120_caps_default();
        caps.compute_capability = (99, 0);
        caps.multi_processor_count = 13;

        let profile = caps.to_device_profile();

        assert_eq!(profile.compute_units, 13);
        assert_eq!(profile.regs_per_thread_max, 255);
        assert_eq!(profile.l2_cache_bytes, 96 * 1024 * 1024);
        assert_eq!(profile.mem_bw_gbps, 1792);
        assert_eq!(profile.max_invocations_per_workgroup, 1024);
        assert_eq!(profile.max_shared_memory_bytes, 128 * 1024);
        assert_eq!(caps.shared_memory_per_sm_bytes(), 256 * 1024);
        assert_eq!(profile.shared_memory_bank_count, 32);
        assert_eq!(profile.shared_memory_bank_width_bytes, 4);
    }

    #[test]
    fn cuda_probe_all_rejects_zero_device_silent_fallback_by_contract() {
        let source = include_str!("device.rs");

        assert!(
            source.contains("device_count == 0")
                && source.contains("do not silently continue on a CPU path"),
            "Fix: CUDA device discovery must fail loudly when a GPU-required host reports zero visible devices."
        );
        assert!(
            !source.contains(concat!("(0..device_count)", ".map(Self::probe).collect()")),
            "Fix: CUDA device discovery must not hide zero devices behind an empty successful probe list."
        );
    }

    #[test]
    fn cuda_capability_conversion_has_no_production_panic_path() {
        let source = include_str!("device.rs");
        let start = source
            .find("fn required_u32_capability")
            .expect("Fix: CUDA capability conversion helper must exist");
        let end = source[start..]
            .find("/// Return the number of CUDA devices visible")
            .expect("Fix: CUDA capability conversion helper must stay before device discovery")
            + start;
        let helper = &source[start..end];

        assert!(
            !helper.contains("panic!("),
            "Fix: CUDA capability accessors must not abort production dispatch; probe-time validation must reject invalid capability snapshots with typed errors."
        );
        assert!(
            !helper.contains("unwrap_or(1)"),
            "Fix: CUDA capability accessors must not manufacture fake nonzero defaults after validation."
        );
        assert!(
            !helper.contains(" as u32"),
            "Fix: CUDA capability accessors must use checked integer conversion, not release-path narrowing casts."
        );
        let bandwidth_start = source
            .find("pub fn memory_bandwidth_gbps")
            .expect("Fix: CUDA memory bandwidth helper must exist");
        let bandwidth_end = source[bandwidth_start..]
            .find("/// NVIDIA CUDA architectural register ceiling")
            .expect("Fix: CUDA memory bandwidth helper should precede register helper")
            + bandwidth_start;
        let bandwidth_helper = &source[bandwidth_start..bandwidth_end];
        assert!(
            !bandwidth_helper.contains(" as u32"),
            "Fix: CUDA bandwidth telemetry must not narrow with an unchecked cast."
        );
        assert!(
            bandwidth_helper.contains("u32::try_from"),
            "Fix: CUDA bandwidth telemetry must use checked conversion after widened arithmetic."
        );
        assert!(
            source.contains("caps.validate_required_attributes()?"),
            "Fix: CUDA capability probing must validate launch-critical values before exposing infallible accessors."
        );
        assert!(
            source.contains("CUDA device major compute capability was negative")
                && source.contains("map_err(|source|"),
            "Fix: CUDA capability probe must return typed errors for corrupt driver attributes instead of panicking."
        );
    }
}
