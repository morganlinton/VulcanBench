//! Live wgpu capability decisions shared by validation and dispatch.

use crate::runtime::device::EnabledFeatures;

#[inline]
pub(crate) fn is_real_gpu(adapter_info: &wgpu::AdapterInfo) -> bool {
    matches!(
        adapter_info.device_type,
        wgpu::DeviceType::DiscreteGpu
            | wgpu::DeviceType::IntegratedGpu
            | wgpu::DeviceType::VirtualGpu
    )
}

/// Wgpu compute indirect dispatch is a core command-buffer operation; the
/// honest gate is whether this backend owns a real GPU device with enough
/// storage-buffer space for the required u32 x/y/z dispatch tuple.
#[inline]
pub(crate) fn supports_indirect_dispatch(
    adapter_info: &wgpu::AdapterInfo,
    enabled: &EnabledFeatures,
) -> bool {
    supports_indirect_dispatch_limits(
        adapter_info,
        enabled.max_storage_buffer_binding_size,
        enabled.max_workgroup_size,
    )
}

#[inline]
pub(crate) fn supports_indirect_dispatch_limits(
    adapter_info: &wgpu::AdapterInfo,
    max_storage_buffer_binding_size: u64,
    max_workgroup_size: [u32; 3],
) -> bool {
    is_real_gpu(adapter_info)
        && max_storage_buffer_binding_size >= 12
        && max_workgroup_size.iter().all(|axis| *axis > 0)
}

#[inline]
pub(crate) fn supports_subgroup_for_adapter(
    features: wgpu::Features,
    limits: &wgpu::Limits,
) -> bool {
    subgroup_caps_for_adapter(features, limits).is_usable()
}

/// Subgroup support requires both the requested device feature and usable
/// subgroup-size limits for dispatch planning.
#[inline]
pub(crate) fn supports_subgroup_ops(enabled: &EnabledFeatures) -> bool {
    subgroup_caps(enabled).is_usable()
}

/// Shared subgroup capability record from live adapter features.
#[inline]
pub(crate) fn subgroup_caps_for_adapter(
    features: wgpu::Features,
    limits: &wgpu::Limits,
) -> vyre_driver::SubgroupCaps {
    vyre_driver::SubgroupCaps::from_feature_range(
        features.contains(wgpu::Features::SUBGROUP),
        features.contains(wgpu::Features::SUBGROUP_VERTEX),
        limits.min_subgroup_size,
        limits.max_subgroup_size,
    )
}

/// Shared subgroup capability record from the created device feature set.
#[inline]
pub(crate) fn subgroup_caps(enabled: &EnabledFeatures) -> vyre_driver::SubgroupCaps {
    vyre_driver::SubgroupCaps::from_feature_range(
        enabled.subgroup,
        false,
        enabled.min_subgroup_size,
        enabled.max_subgroup_size,
    )
}
