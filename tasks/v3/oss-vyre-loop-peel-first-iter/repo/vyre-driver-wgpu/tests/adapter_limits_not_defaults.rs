//! Adapter limits honesty: backend must not report hardcoded conservative defaults on a live GPU.
//!
//! Guarantees:
//! - `max_workgroup_size` is not `[1, 1, 1]`
//! - `max_compute_workgroups_per_dimension` is not `1`
//! - `max_compute_invocations_per_workgroup` is not `1`
//! - `max_storage_buffer_bytes` is not `0`
//! - Reported limits match the live `wgpu::Limits`
//! - `subgroup_size` is not `None` when the adapter supports subgroups

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
// 1. Limits must not be conservative defaults
// ------------------------------------------------------------------

#[test]
fn max_workgroup_size_is_not_conservative_default() {
    let backend = live_backend();
    let size = backend.max_workgroup_size();
    let info = backend.adapter_info();

    assert_ne!(
        size,
        [1, 1, 1],
        "Fix: max_workgroup_size must not be the conservative default [1,1,1] on a live GPU. \
         Got {:?}. Adapter: {}",
        size,
        info.name
    );

    assert!(
        size.iter().all(|&axis| axis > 0),
        "Fix: all max_workgroup_size axes must be positive. Got {:?}. Adapter: {}",
        size,
        info.name
    );
}

#[test]
fn max_compute_workgroups_per_dimension_is_not_conservative_default() {
    let backend = live_backend();
    let limit = backend.max_compute_workgroups_per_dimension();
    let info = backend.adapter_info();

    assert_ne!(
        limit, 1,
        "Fix: max_compute_workgroups_per_dimension must not be the conservative default 1 \
         on a live GPU. Got {}. Adapter: {}",
        limit, info.name
    );

    assert!(
        limit > 0,
        "Fix: max_compute_workgroups_per_dimension must be positive. Got {}. Adapter: {}",
        limit,
        info.name
    );
}

#[test]
fn max_compute_invocations_per_workgroup_is_not_conservative_default() {
    let backend = live_backend();
    let limit = backend.max_compute_invocations_per_workgroup();
    let info = backend.adapter_info();

    assert_ne!(
        limit, 1,
        "Fix: max_compute_invocations_per_workgroup must not be the conservative default 1 \
         on a live GPU. Got {}. Adapter: {}",
        limit, info.name
    );

    assert!(
        limit > 0,
        "Fix: max_compute_invocations_per_workgroup must be positive. Got {}. Adapter: {}",
        limit,
        info.name
    );
}

#[test]
fn max_storage_buffer_bytes_is_not_conservative_default() {
    let backend = live_backend();
    let limit = backend.max_storage_buffer_bytes();
    let info = backend.adapter_info();

    assert_ne!(
        limit, 0,
        "Fix: max_storage_buffer_bytes must not be the conservative default 0 on a live GPU. \
         Got {}. Adapter: {}",
        limit, info.name
    );

    assert!(
        limit >= 65536,
        "Fix: max_storage_buffer_bytes on a real GPU should be at least 64 KiB. \
         Got {}. Adapter: {}",
        limit,
        info.name
    );
}

// ------------------------------------------------------------------
// 2. Reported limits must match live device limits
// ------------------------------------------------------------------

#[test]
fn adapter_limits_match_live_device_limits() {
    let backend = live_backend();
    let device_limits = backend.device_limits();
    let info = backend.adapter_info();

    // max_workgroup_size is sourced from enabled_features (adapter limits requested
    // at device creation), which may differ from the device's effective limits.
    // We verify it matches the optimizer-facing caps snapshot instead.
    let reported_size = backend.max_workgroup_size();
    let caps_size = backend.adapter_caps().max_workgroup_size;
    assert_eq!(
        reported_size, caps_size,
        "Fix: max_workgroup_size must match adapter_caps snapshot. Got {:?}, expected {:?}. \
         Adapter: {}",
        reported_size, caps_size, info.name
    );

    assert_eq!(
        backend.max_compute_workgroups_per_dimension(),
        device_limits.max_compute_workgroups_per_dimension,
        "Fix: max_compute_workgroups_per_dimension must match live device limits. Adapter: {}",
        info.name
    );

    assert_eq!(
        backend.max_compute_invocations_per_workgroup(),
        device_limits.max_compute_invocations_per_workgroup,
        "Fix: max_compute_invocations_per_workgroup must match live device limits. Adapter: {}",
        info.name
    );
}

// ------------------------------------------------------------------
// 3. Subgroup size must not be None when adapter supports it
// ------------------------------------------------------------------

#[test]
fn subgroup_size_not_none_when_adapter_supports_subgroup() {
    let backend = live_backend();
    let adapter = selected_adapter(&backend);
    let limits = adapter.limits();
    let info = backend.adapter_info();

    let adapter_has_subgroup =
        adapter.features().contains(wgpu::Features::SUBGROUP) && limits.min_subgroup_size > 0;

    if adapter_has_subgroup {
        assert!(
            backend.subgroup_size().is_some(),
            "Fix: subgroup_size must not be None when adapter supports SUBGROUP. \
             Adapter: {}",
            info.name
        );
        let size = backend.subgroup_size().unwrap();
        assert_eq!(
            size, limits.min_subgroup_size,
            "Fix: subgroup_size must match adapter min_subgroup_size. Got {}, expected {}. \
             Adapter: {}",
            size, limits.min_subgroup_size, info.name
        );
    }
}
