//! Adapter/device limits honesty: every reported limit must originate from
//! the live wgpu adapter or device, never from a hardcoded constant.
//!
//! Guarantees:
//! - `max_workgroup_size` matches the adapter's enabled workgroup limits
//! - `max_compute_workgroups_per_dimension` matches the live device limits
//! - `max_compute_invocations_per_workgroup` matches the live device limits
//! - `max_storage_buffer_bytes` matches the live device limits
//! - `device_limits()` is the actual `wgpu::Limits` of the created device

mod common;
use common::shared_live_backend as live_backend;

use vyre::VyreBackend;
use vyre_driver_wgpu::WgpuBackend;

fn selected_adapter(backend: &WgpuBackend) -> wgpu::Adapter {
    vyre_driver_wgpu::runtime::device::adapter_for_info(backend.adapter_info()).expect(
        "Fix: selected wgpu backend adapter must remain enumerable for live capability probing",
    )
}

// ------------------------------------------------------------------
// 1. max_workgroup_size must come from adapter limits
// ------------------------------------------------------------------

#[test]
fn max_workgroup_size_matches_adapter_limits() {
    let backend = live_backend();
    let adapter = selected_adapter(&backend);
    let limits = adapter.limits();
    let info = backend.adapter_info();

    let reported = backend.max_workgroup_size();
    let expected = [
        limits.max_compute_workgroup_size_x,
        limits.max_compute_workgroup_size_y,
        limits.max_compute_workgroup_size_z,
    ];

    assert_eq!(
        reported, expected,
        "Fix: max_workgroup_size must match the adapter's compute workgroup size limits. \
         Got {:?}, expected {:?}. Adapter: {}",
        reported, expected, info.name
    );
}

// ------------------------------------------------------------------
// 2. max_compute_workgroups_per_dimension must come from device limits
// ------------------------------------------------------------------

#[test]
fn max_compute_workgroups_per_dimension_matches_device_limits() {
    let backend = live_backend();
    let device_limits = backend.device_limits();
    let info = backend.adapter_info();

    let reported = backend.max_compute_workgroups_per_dimension();
    let expected = device_limits.max_compute_workgroups_per_dimension;

    assert_eq!(
        reported, expected,
        "Fix: max_compute_workgroups_per_dimension must match live device limits. \
         Got {}, expected {}. Adapter: {}",
        reported, expected, info.name
    );
}

// ------------------------------------------------------------------
// 3. max_compute_invocations_per_workgroup must come from device limits
// ------------------------------------------------------------------

#[test]
fn max_compute_invocations_per_workgroup_matches_device_limits() {
    let backend = live_backend();
    let device_limits = backend.device_limits();
    let info = backend.adapter_info();

    let reported = backend.max_compute_invocations_per_workgroup();
    let expected = device_limits.max_compute_invocations_per_workgroup;

    assert_eq!(
        reported, expected,
        "Fix: max_compute_invocations_per_workgroup must match live device limits. \
         Got {}, expected {}. Adapter: {}",
        reported, expected, info.name
    );
}

// ------------------------------------------------------------------
// 4. max_storage_buffer_bytes must come from device limits
// ------------------------------------------------------------------

#[test]
fn max_storage_buffer_bytes_matches_device_limits() {
    let backend = live_backend();
    let device_limits = backend.device_limits();
    let info = backend.adapter_info();

    let reported = backend.max_storage_buffer_bytes();
    let expected = u64::from(device_limits.max_storage_buffer_binding_size);

    assert_eq!(
        reported, expected,
        "Fix: max_storage_buffer_bytes must match live device limits. \
         Got {}, expected {}. Adapter: {}",
        reported, expected, info.name
    );
}

// ------------------------------------------------------------------
// 5. device_limits() must be the actual wgpu device limits object
// ------------------------------------------------------------------

#[test]
fn device_limits_is_actual_wgpu_limits() {
    let backend = live_backend();
    let device_limits = backend.device_limits();
    let adapter = selected_adapter(&backend);
    let adapter_limits = adapter.limits();
    let info = backend.adapter_info();

    // The device limits should be at least as high as the adapter limits
    // for the fields we request explicitly at device creation.
    assert!(
        device_limits.max_compute_workgroup_size_x >= adapter_limits.max_compute_workgroup_size_x,
        "Fix: device max_compute_workgroup_size_x must not be lower than adapter limit. \
         Got {}, adapter has {}. Adapter: {}",
        device_limits.max_compute_workgroup_size_x,
        adapter_limits.max_compute_workgroup_size_x,
        info.name
    );

    assert!(
        device_limits.max_compute_workgroup_size_y >= adapter_limits.max_compute_workgroup_size_y,
        "Fix: device max_compute_workgroup_size_y must not be lower than adapter limit. \
         Got {}, adapter has {}. Adapter: {}",
        device_limits.max_compute_workgroup_size_y,
        adapter_limits.max_compute_workgroup_size_y,
        info.name
    );

    assert!(
        device_limits.max_compute_workgroup_size_z >= adapter_limits.max_compute_workgroup_size_z,
        "Fix: device max_compute_workgroup_size_z must not be lower than adapter limit. \
         Got {}, adapter has {}. Adapter: {}",
        device_limits.max_compute_workgroup_size_z,
        adapter_limits.max_compute_workgroup_size_z,
        info.name
    );
}
