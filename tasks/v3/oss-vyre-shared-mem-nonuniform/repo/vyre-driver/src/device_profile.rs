//! Backend-neutral device capability profile.
//!
//! Concrete backend crates probe their native device/API surfaces and project
//! them into this value object. Shared optimizer, validation, launch, and
//! strategy code consume projections of this profile instead of carrying
//! independent capability records that can drift.

use vyre_foundation::optimizer::AdapterCaps;
use vyre_foundation::validate;

/// Quality class for backend timing data exposed through [`DeviceProfile`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeviceTimingQuality {
    /// The backend reports host wall-clock timing only.
    HostOnly,
    /// The backend can split host enqueue and host wait timing, but not trusted device elapsed time.
    HostEnqueueWait,
    /// The backend can report device elapsed time through timestamp queries or events.
    DeviceTimestamps,
    /// The backend can report device elapsed time plus hardware counter samples.
    HardwareCounters,
}

impl DeviceTimingQuality {
    /// Stable report/config string for timing-quality evidence.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::HostOnly => "host_only",
            Self::HostEnqueueWait => "host_enqueue_wait",
            Self::DeviceTimestamps => "device_timestamps",
            Self::HardwareCounters => "hardware_counters",
        }
    }
}

/// Device capability snapshot used across driver-shared planning.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeviceProfile {
    /// Stable backend identifier.
    pub backend: &'static str,
    /// The device and lowering path support subgroup intrinsics.
    pub supports_subgroup_ops: bool,
    /// The backend supports indirect dispatch.
    pub supports_indirect_dispatch: bool,
    /// The backend lowers distributed collective communication nodes.
    pub supports_distributed_collectives: bool,
    /// The backend supports compile-time specialization constants.
    pub supports_specialization_constants: bool,
    /// The backend lowers binary16 natively.
    pub supports_f16: bool,
    /// The backend lowers bfloat16 natively.
    pub supports_bf16: bool,
    /// The backend preserves explicit trap propagation.
    pub supports_trap_propagation: bool,
    /// The backend lowers matrix-engine operations for supported shapes.
    pub supports_tensor_cores: bool,
    /// Native unsigned multiply-high is available to lowering strategies.
    pub has_mul_high: bool,
    /// Integer and float pipelines can issue concurrently.
    pub has_dual_issue_fp32_int32: bool,
    /// Subgroup shuffle-like communication is available.
    pub has_subgroup_shuffle: bool,
    /// Explicit workgroup/shared memory is available.
    pub has_shared_memory: bool,
    /// Maximum native integer width in bits.
    pub max_native_int_width: u32,
    /// Maximum workgroup dimensions.
    pub max_workgroup_size: [u32; 3],
    /// Maximum invocations in one workgroup.
    pub max_invocations_per_workgroup: u32,
    /// Shared memory per workgroup in bytes.
    pub max_shared_memory_bytes: u32,
    /// Maximum single storage-buffer binding in bytes.
    pub max_storage_buffer_binding_size: u64,
    /// Native subgroup size, or `0` when unknown.
    pub subgroup_size: u32,
    /// Physical compute-unit count, or `0` when unknown.
    pub compute_units: u32,
    /// Maximum registers per thread, or `0` when unknown.
    pub regs_per_thread_max: u32,
    /// L1 cache size in bytes, or `0` when unknown.
    pub l1_cache_bytes: u32,
    /// L2 cache size in bytes, or `0` when unknown.
    pub l2_cache_bytes: u32,
    /// Peak memory bandwidth in GB/s, or `0` when unknown.
    pub mem_bw_gbps: u32,
    /// Timing-data quality exposed by this backend/device.
    pub timing_quality: DeviceTimingQuality,
    /// Device timestamp queries/events are available for dispatch timing.
    pub supports_device_timestamps: bool,
    /// Hardware counter sampling is available for benchmark telemetry.
    pub supports_hardware_counters: bool,
    /// Device-profile preferred unroll depth, or `0` when unknown.
    pub ideal_unroll_depth: u32,
    /// Device-profile preferred vector pack width in bits, or `0` when unknown.
    pub ideal_vector_pack_bits: u32,
    /// Device-profile preferred workgroup tile, or `[0, 0, 0]` when unknown.
    pub ideal_workgroup_tile: [u32; 3],
    /// Shared-memory bank count, or `0` when unknown.
    pub shared_memory_bank_count: u32,
    /// Shared-memory bank width in bytes, or `0` when unknown.
    pub shared_memory_bank_width_bytes: u32,
}

impl Default for DeviceProfile {
    fn default() -> Self {
        Self::conservative("unknown")
    }
}

impl DeviceProfile {
    /// Conservative profile for a backend that has not probed a device.
    #[must_use]
    pub const fn conservative(backend: &'static str) -> Self {
        Self {
            backend,
            supports_subgroup_ops: false,
            supports_indirect_dispatch: false,
            supports_distributed_collectives: false,
            supports_specialization_constants: false,
            supports_f16: false,
            supports_bf16: false,
            supports_trap_propagation: false,
            supports_tensor_cores: false,
            has_mul_high: false,
            has_dual_issue_fp32_int32: false,
            has_subgroup_shuffle: false,
            has_shared_memory: false,
            max_native_int_width: 32,
            max_workgroup_size: [1, 1, 1],
            max_invocations_per_workgroup: 1,
            max_shared_memory_bytes: 0,
            max_storage_buffer_binding_size: 0,
            subgroup_size: 0,
            compute_units: 0,
            regs_per_thread_max: 0,
            l1_cache_bytes: 0,
            l2_cache_bytes: 0,
            mem_bw_gbps: 0,
            timing_quality: DeviceTimingQuality::HostOnly,
            supports_device_timestamps: false,
            supports_hardware_counters: false,
            ideal_unroll_depth: 0,
            ideal_vector_pack_bits: 0,
            ideal_workgroup_tile: [0, 0, 0],
            shared_memory_bank_count: 0,
            shared_memory_bank_width_bytes: 0,
        }
    }

    /// Build a profile from the stable backend trait capability methods.
    #[must_use]
    pub fn from_backend(backend: &dyn crate::backend::VyreBackend) -> Self {
        let max_workgroup_size = backend.max_workgroup_size();
        Self {
            backend: backend.id(),
            supports_subgroup_ops: backend.supports_subgroup_ops(),
            supports_indirect_dispatch: backend.supports_indirect_dispatch(),
            supports_distributed_collectives: backend.supports_distributed_collectives(),
            supports_specialization_constants: false,
            supports_f16: backend.supports_f16(),
            supports_bf16: backend.supports_bf16(),
            supports_trap_propagation: false,
            supports_tensor_cores: backend.supports_tensor_cores(),
            has_mul_high: false,
            has_dual_issue_fp32_int32: false,
            has_subgroup_shuffle: backend.supports_subgroup_ops(),
            has_shared_memory: false,
            max_native_int_width: 32,
            max_workgroup_size,
            max_invocations_per_workgroup: backend.max_compute_invocations_per_workgroup(),
            max_shared_memory_bytes: 0,
            max_storage_buffer_binding_size: backend.max_storage_buffer_bytes(),
            subgroup_size: backend.subgroup_size().unwrap_or(0),
            compute_units: 0,
            regs_per_thread_max: 0,
            l1_cache_bytes: 0,
            l2_cache_bytes: 0,
            mem_bw_gbps: 0,
            timing_quality: DeviceTimingQuality::HostOnly,
            supports_device_timestamps: false,
            supports_hardware_counters: false,
            ideal_unroll_depth: 0,
            ideal_vector_pack_bits: 0,
            ideal_workgroup_tile: [0, 0, 0],
            shared_memory_bank_count: 0,
            shared_memory_bank_width_bytes: 0,
        }
    }

    /// Validation capability projection.
    #[must_use]
    pub const fn validation_capabilities(self) -> validate::BackendCapabilities {
        validate::BackendCapabilities {
            supports_subgroup_ops: self.supports_subgroup_ops,
            supports_indirect_dispatch: self.supports_indirect_dispatch,
            supports_specialization_constants: self.supports_specialization_constants,
            has_mul_high: self.has_mul_high,
            has_dual_issue_fp32_int32: self.has_dual_issue_fp32_int32,
            has_tensor_core_int: self.supports_tensor_cores,
            has_native_f16: self.supports_f16,
            has_warp_shuffle: self.has_subgroup_shuffle,
            has_shared_memory: self.has_shared_memory,
            has_transcendental_polynomial_emit: true,
            supports_distributed_collectives: self.supports_distributed_collectives,
            max_native_int_width: self.max_native_int_width,
        }
    }

    /// Optimizer capability projection.
    #[must_use]
    pub const fn adapter_caps(self) -> AdapterCaps {
        AdapterCaps {
            backend: self.backend,
            supports_subgroup_ops: self.supports_subgroup_ops,
            supports_indirect_dispatch: self.supports_indirect_dispatch,
            supports_specialization_constants: self.supports_specialization_constants,
            max_workgroup_size: self.max_workgroup_size,
            max_invocations_per_workgroup: self.max_invocations_per_workgroup,
            max_shared_memory_bytes: self.max_shared_memory_bytes,
            max_storage_buffer_binding_size: self.max_storage_buffer_binding_size,
            subgroup_size: self.subgroup_size,
            compute_units: self.compute_units,
            regs_per_thread_max: self.regs_per_thread_max,
            l1_cache_bytes: self.l1_cache_bytes,
            l2_cache_bytes: self.l2_cache_bytes,
            mem_bw_gbps: self.mem_bw_gbps,
            ideal_unroll_depth: self.ideal_unroll_depth,
            ideal_vector_pack_bits: self.ideal_vector_pack_bits,
            ideal_workgroup_tile: self.ideal_workgroup_tile,
            shared_memory_bank_count: self.shared_memory_bank_count,
            shared_memory_bank_width_bytes: self.shared_memory_bank_width_bytes,
        }
    }

    /// Strategy capability projection.
    #[must_use]
    pub const fn strategy_capabilities(self) -> validate::BackendCapabilities {
        self.validation_capabilities()
    }
}

impl From<DeviceProfile> for AdapterCaps {
    #[inline]
    fn from(profile: DeviceProfile) -> Self {
        profile.adapter_caps()
    }
}

impl From<DeviceProfile> for validate::BackendCapabilities {
    #[inline]
    fn from(profile: DeviceProfile) -> Self {
        profile.validation_capabilities()
    }
}

#[cfg(test)]
mod tests {
    use super::{DeviceProfile, DeviceTimingQuality};

    #[test]
    fn timing_quality_has_stable_report_strings() {
        assert_eq!(DeviceTimingQuality::HostOnly.as_str(), "host_only");
        assert_eq!(
            DeviceTimingQuality::HostEnqueueWait.as_str(),
            "host_enqueue_wait"
        );
        assert_eq!(
            DeviceTimingQuality::DeviceTimestamps.as_str(),
            "device_timestamps"
        );
        assert_eq!(
            DeviceTimingQuality::HardwareCounters.as_str(),
            "hardware_counters"
        );
    }

    #[test]
    fn projections_share_the_same_feature_bits() {
        let profile = DeviceProfile {
            backend: "test",
            supports_subgroup_ops: true,
            supports_indirect_dispatch: true,
            supports_distributed_collectives: true,
            supports_specialization_constants: true,
            supports_f16: true,
            supports_bf16: false,
            supports_trap_propagation: true,
            supports_tensor_cores: true,
            has_mul_high: true,
            has_dual_issue_fp32_int32: true,
            has_subgroup_shuffle: true,
            has_shared_memory: true,
            max_native_int_width: 64,
            max_workgroup_size: [256, 1, 1],
            max_invocations_per_workgroup: 256,
            max_shared_memory_bytes: 48 * 1024,
            max_storage_buffer_binding_size: 1 << 30,
            subgroup_size: 32,
            compute_units: 128,
            regs_per_thread_max: 255,
            l1_cache_bytes: 128 * 1024,
            l2_cache_bytes: 64 * 1024 * 1024,
            mem_bw_gbps: 1700,
            timing_quality: super::DeviceTimingQuality::HardwareCounters,
            supports_device_timestamps: true,
            supports_hardware_counters: true,
            ideal_unroll_depth: 8,
            ideal_vector_pack_bits: 128,
            ideal_workgroup_tile: [16, 16, 1],
            shared_memory_bank_count: 32,
            shared_memory_bank_width_bytes: 4,
        };

        let validation = profile.validation_capabilities();
        let adapter = profile.adapter_caps();
        let strategy = profile.strategy_capabilities();

        assert!(validation.supports_subgroup_ops);
        assert!(validation.supports_distributed_collectives);
        assert!(adapter.supports_subgroup_ops);
        assert!(strategy.has_warp_shuffle);
        assert_eq!(adapter.max_invocations_per_workgroup, 256);
        assert_eq!(adapter.ideal_unroll_depth, 8);
        assert_eq!(adapter.ideal_vector_pack_bits, 128);
        assert_eq!(adapter.ideal_workgroup_tile, [16, 16, 1]);
        assert_eq!(strategy.max_native_int_width, 64);
        assert_eq!(
            profile.timing_quality,
            super::DeviceTimingQuality::HardwareCounters
        );
        assert!(profile.supports_device_timestamps);
        assert!(profile.supports_hardware_counters);
    }
}
