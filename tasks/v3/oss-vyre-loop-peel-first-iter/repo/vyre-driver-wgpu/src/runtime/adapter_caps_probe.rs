//! Adapter-caps probe (C-B10).
//!
//! Extracts a [`vyre_foundation::optimizer::AdapterCaps`] from a live
//! `wgpu::Adapter`. Passes registered in the vyre-core
//! `PassManager` read these caps to adapt: subgroup intrinsics fire
//! only when `supports_subgroup_ops == true`; the fusion pass
//! (C-B8) checks `max_shared_memory_bytes` before collapsing
//! kernels; the megakernel (C-B9) filters its `worker_count`
//! against `max_workgroup_size`.
//!
//! The probe is pure: it reads the adapter's `get_info()`,
//! `features()`, and `limits()` and projects into the substrate-neutral
//! [`vyre_foundation::optimizer::AdapterCaps`] shape. No dispatch happens.

use crate::runtime::device::EnabledFeatures;
use vyre_driver::{DeviceProfile, DeviceSignatureTable, DeviceTimingQuality};
use vyre_foundation::optimizer::AdapterCaps;

/// Probe a live wgpu adapter and return the substrate-neutral
/// caps `PassManager` consumers use.
#[must_use]
pub fn probe(adapter: &wgpu::Adapter) -> AdapterCaps {
    probe_profile(adapter).into()
}

/// Probe a live adapter and return the neutral driver profile.
#[must_use]
pub fn probe_profile(adapter: &wgpu::Adapter) -> DeviceProfile {
    let features = adapter.features();
    let limits = adapter.limits();
    let info = adapter.get_info();

    let max_workgroup_size = [
        limits.max_compute_workgroup_size_x,
        limits.max_compute_workgroup_size_y,
        limits.max_compute_workgroup_size_z,
    ];
    let timestamp_queries = features.contains(wgpu::Features::TIMESTAMP_QUERY)
        && features.contains(wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS);
    let subgroup_caps = crate::capabilities::subgroup_caps_for_adapter(features, &limits);
    let profile = DeviceProfile {
        backend: backend_id_for(info.backend),
        supports_subgroup_ops: subgroup_caps.supports_subgroup,
        supports_indirect_dispatch: crate::capabilities::supports_indirect_dispatch_limits(
            &info,
            u64::from(limits.max_storage_buffer_binding_size),
            max_workgroup_size,
        ),
        supports_distributed_collectives: false,
        supports_specialization_constants: true,
        supports_f16: false,
        supports_bf16: false,
        supports_trap_propagation: true,
        supports_tensor_cores: false,
        has_mul_high: true,
        has_dual_issue_fp32_int32: false,
        has_subgroup_shuffle: subgroup_caps.supports_subgroup,
        has_shared_memory: limits.max_compute_workgroup_storage_size > 0,
        max_native_int_width: 32,
        max_workgroup_size,
        max_invocations_per_workgroup: limits.max_compute_invocations_per_workgroup,
        max_shared_memory_bytes: limits.max_compute_workgroup_storage_size,
        max_storage_buffer_binding_size: u64::from(limits.max_storage_buffer_binding_size),
        subgroup_size: subgroup_caps.subgroup_size,
        compute_units: 0,
        regs_per_thread_max: 0,
        l1_cache_bytes: 0,
        l2_cache_bytes: 0,
        mem_bw_gbps: 0,
        timing_quality: if timestamp_queries {
            DeviceTimingQuality::DeviceTimestamps
        } else {
            DeviceTimingQuality::HostEnqueueWait
        },
        supports_device_timestamps: timestamp_queries,
        supports_hardware_counters: false,
        ideal_unroll_depth: 0,
        ideal_vector_pack_bits: 0,
        ideal_workgroup_tile: [0, 0, 0],
        shared_memory_bank_count: 0,
        shared_memory_bank_width_bytes: 0,
    };
    DeviceSignatureTable::builtins().map_or(profile, |table| {
        table.apply_device_name_to_profile(&info.name, profile)
    })
}

/// Project the already-created backend device into optimizer caps.
///
/// This is the capability source production planners should prefer: it uses
/// the feature set that was actually requested at device creation, including
/// post-creation checks such as the subgroup smoke pipeline probe.
#[must_use]
pub fn from_backend(
    adapter_info: &wgpu::AdapterInfo,
    device_limits: &wgpu::Limits,
    enabled: &EnabledFeatures,
) -> AdapterCaps {
    from_backend_profile(adapter_info, device_limits, enabled).into()
}

/// Project the already-created backend device into the neutral profile.
#[must_use]
pub fn from_backend_profile(
    adapter_info: &wgpu::AdapterInfo,
    device_limits: &wgpu::Limits,
    enabled: &EnabledFeatures,
) -> DeviceProfile {
    let subgroup_caps = crate::capabilities::subgroup_caps(enabled);
    let timestamp_queries = enabled.timestamp_query && enabled.timestamp_query_inside_encoders;
    let profile = DeviceProfile {
        backend: backend_id_for(adapter_info.backend),
        supports_subgroup_ops: subgroup_caps.supports_subgroup,
        supports_indirect_dispatch: crate::capabilities::supports_indirect_dispatch(
            adapter_info,
            enabled,
        ),
        supports_distributed_collectives: false,
        supports_specialization_constants: true,
        supports_f16: false,
        supports_bf16: false,
        supports_trap_propagation: true,
        supports_tensor_cores: false,
        has_mul_high: true,
        has_dual_issue_fp32_int32: false,
        has_subgroup_shuffle: subgroup_caps.supports_subgroup,
        has_shared_memory: device_limits.max_compute_workgroup_storage_size > 0,
        max_native_int_width: 32,
        max_workgroup_size: enabled.max_workgroup_size,
        max_invocations_per_workgroup: device_limits.max_compute_invocations_per_workgroup,
        max_shared_memory_bytes: device_limits.max_compute_workgroup_storage_size,
        max_storage_buffer_binding_size: enabled.max_storage_buffer_binding_size,
        subgroup_size: subgroup_caps.subgroup_size,
        compute_units: 0,
        regs_per_thread_max: 0,
        l1_cache_bytes: 0,
        l2_cache_bytes: 0,
        mem_bw_gbps: 0,
        timing_quality: if timestamp_queries {
            DeviceTimingQuality::DeviceTimestamps
        } else {
            DeviceTimingQuality::HostEnqueueWait
        },
        supports_device_timestamps: timestamp_queries,
        supports_hardware_counters: false,
        ideal_unroll_depth: 0,
        ideal_vector_pack_bits: 0,
        ideal_workgroup_tile: [0, 0, 0],
        shared_memory_bank_count: 0,
        shared_memory_bank_width_bytes: 0,
    };
    DeviceSignatureTable::builtins().map_or(profile, |table| {
        table.apply_device_name_to_profile(&adapter_info.name, profile)
    })
}

fn backend_id_for(backend: wgpu::Backend) -> &'static str {
    match backend {
        wgpu::Backend::Vulkan => "vulkan",
        wgpu::Backend::Metal => "native_module",
        wgpu::Backend::Dx12 => "dx12",
        wgpu::Backend::Gl => "gl",
        wgpu::Backend::BrowserWebGpu => "webgpu",
        wgpu::Backend::Noop => "noop",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backend_id_maps_every_wgpu_backend() {
        assert_eq!(backend_id_for(wgpu::Backend::Vulkan), "vulkan");
        assert_eq!(backend_id_for(wgpu::Backend::Metal), "native_module");
        assert_eq!(backend_id_for(wgpu::Backend::Dx12), "dx12");
        assert_eq!(backend_id_for(wgpu::Backend::Gl), "gl");
        assert_eq!(backend_id_for(wgpu::Backend::BrowserWebGpu), "webgpu");
        assert_eq!(backend_id_for(wgpu::Backend::Noop), "noop");
    }

    #[test]
    fn backend_profile_applies_builtin_device_signature_alias() {
        let adapter_info = wgpu::AdapterInfo {
            name: "NVIDIA GeForce RTX 5090".to_string(),
            vendor: 0x10de,
            device: 0x2c02,
            device_type: wgpu::DeviceType::DiscreteGpu,
            driver: "nvidia".to_string(),
            driver_info: "570".to_string(),
            backend: wgpu::Backend::Vulkan,
        };
        let limits = wgpu::Limits {
            max_compute_workgroup_size_x: 1024,
            max_compute_workgroup_size_y: 1024,
            max_compute_workgroup_size_z: 64,
            max_compute_invocations_per_workgroup: 1024,
            max_compute_workgroup_storage_size: 128 * 1024,
            ..wgpu::Limits::default()
        };
        let enabled = EnabledFeatures {
            max_workgroup_size: [1024, 1024, 64],
            max_storage_buffer_binding_size: 1 << 30,
            min_subgroup_size: 32,
            max_subgroup_size: 32,
            ..EnabledFeatures::default()
        };

        let profile = from_backend_profile(&adapter_info, &limits, &enabled);
        let table = DeviceSignatureTable::builtins().expect("Fix: builtin device signatures load");
        let signature = table
            .find_device_name("RTX 5090")
            .expect("Fix: RTX 5090 must match the builtin Blackwell signature");

        assert_eq!(profile.ideal_unroll_depth, signature.ideal_unroll_depth);
        assert_eq!(
            profile.ideal_vector_pack_bits,
            signature.ideal_vector_pack_bits
        );
        assert_eq!(profile.ideal_workgroup_tile, signature.ideal_workgroup_tile);
        assert_eq!(
            profile.shared_memory_bank_width_bytes,
            signature.bank_width_bytes
        );
    }

    #[test]
    fn backend_profile_uses_created_device_limits_and_enabled_features() {
        let adapter_info = wgpu::AdapterInfo {
            name: "Unit Test Adapter".to_string(),
            vendor: 0,
            device: 0,
            device_type: wgpu::DeviceType::DiscreteGpu,
            driver: "unit".to_string(),
            driver_info: "unit".to_string(),
            backend: wgpu::Backend::Vulkan,
        };
        let limits = wgpu::Limits {
            max_compute_workgroup_size_x: 1024,
            max_compute_workgroup_size_y: 512,
            max_compute_workgroup_size_z: 64,
            max_compute_invocations_per_workgroup: 768,
            max_compute_workgroup_storage_size: 96 * 1024,
            ..wgpu::Limits::default()
        };
        let enabled = EnabledFeatures {
            max_workgroup_size: [768, 256, 32],
            max_storage_buffer_binding_size: 512 << 20,
            ..EnabledFeatures::default()
        };

        let profile = from_backend_profile(&adapter_info, &limits, &enabled);
        let caps = from_backend(&adapter_info, &limits, &enabled);

        assert_eq!(
            profile.max_workgroup_size, enabled.max_workgroup_size,
            "profile must use the workgroup limits actually enabled on the created device"
        );
        assert_eq!(
            caps.max_workgroup_size, enabled.max_workgroup_size,
            "optimizer caps must inherit live enabled workgroup limits"
        );
        assert_eq!(
            profile.max_invocations_per_workgroup,
            limits.max_compute_invocations_per_workgroup
        );
        assert_eq!(
            profile.max_shared_memory_bytes,
            limits.max_compute_workgroup_storage_size
        );
        assert_eq!(
            profile.max_storage_buffer_binding_size,
            enabled.max_storage_buffer_binding_size
        );
    }
}
