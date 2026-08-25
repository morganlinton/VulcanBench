//! Contracts for the process-wide shared wgpu backend.

use std::sync::Arc;

use vyre::VyreBackend;
use vyre_driver_wgpu::WgpuBackend;

fn selected_adapter(backend: &WgpuBackend) -> wgpu::Adapter {
    vyre_driver_wgpu::runtime::device::adapter_for_info(backend.adapter_info()).expect(
        "Fix: selected wgpu backend adapter must remain enumerable for live capability probing",
    )
}

#[test]
fn shared_backend_reuses_single_backend_instance() {
    let first = WgpuBackend::shared().expect("Fix: shared backend requires a configured GPU");
    let second = WgpuBackend::shared().expect("Fix: shared backend must reuse the configured GPU");

    assert!(
        Arc::ptr_eq(&first, &second),
        "Fix: WgpuBackend::shared must return the same Arc-backed backend so scan paths reuse arenas and pipeline caches"
    );
    assert_eq!(first.id(), "wgpu");
}

#[test]
fn shared_backend_reports_same_capabilities_as_concrete_backend() {
    let shared = WgpuBackend::shared().expect("Fix: shared backend requires a configured GPU");
    let concrete = WgpuBackend::new().expect("Fix: concrete backend requires a configured GPU");
    let adapter = selected_adapter(&shared);
    let adapter_limits = adapter.limits();
    let adapter_has_subgroup = adapter.features().contains(wgpu::Features::SUBGROUP)
        && adapter_limits.min_subgroup_size > 0;

    assert_eq!(
        shared.supports_subgroup_ops(),
        concrete.supports_subgroup_ops(),
        "Fix: shared backend must not stale or fabricate subgroup capability reporting"
    );
    assert_eq!(
        shared.supports_indirect_dispatch(),
        concrete.supports_indirect_dispatch(),
        "Fix: shared backend must not stale or fabricate indirect-dispatch capability reporting"
    );
    assert_eq!(
        shared.max_workgroup_size(),
        concrete.max_workgroup_size(),
        "Fix: shared backend must expose the same workgroup limits as the live concrete backend"
    );
    if adapter_has_subgroup {
        assert!(
            shared.supports_subgroup_ops(),
            "Fix: shared backend must not report subgroup_ops=false when the live adapter advertises SUBGROUP and min_subgroup_size={}.",
            adapter_limits.min_subgroup_size
        );
    }
    assert_eq!(
        shared.max_workgroup_size(),
        [
            shared.device_limits().max_compute_workgroup_size_x,
            shared.device_limits().max_compute_workgroup_size_y,
            shared.device_limits().max_compute_workgroup_size_z,
        ],
        "Fix: shared backend max_workgroup_size must come from the live selected device limits"
    );
}
