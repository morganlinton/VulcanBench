//! Subgroup capability reporting honesty tests.
//!
//! Guarantees:
//! - `supports_subgroup_ops` is not hardcoded `false` on capable hardware
//! - Report matches adapter `SUBGROUP` feature and positive `min_subgroup_size`
//! - Consistent across `VyreBackend`, `BackendValidationCapabilities`, and `adapter_caps()`
//! - Subgroup-using programs compile and run when capability is reported `true`

mod common;
use common::shared_live_backend as live_backend;

use vyre::VyreBackend;
use vyre_driver_wgpu::WgpuBackend;
use vyre_foundation::validate::BackendValidationCapabilities;

fn selected_adapter(backend: &WgpuBackend) -> wgpu::Adapter {
    vyre_driver_wgpu::runtime::device::adapter_for_info(backend.adapter_info()).expect(
        "Fix: selected wgpu backend adapter must remain enumerable for live capability probing",
    )
}

// ------------------------------------------------------------------
// 1. Not hardcoded false on capable hardware
// ------------------------------------------------------------------

#[test]
fn subgroup_ops_not_hardcoded_false_on_capable_hardware() {
    let backend = live_backend();
    let adapter = selected_adapter(&backend);
    let limits = adapter.limits();
    let info = backend.adapter_info();

    let adapter_has_subgroup =
        adapter.features().contains(wgpu::Features::SUBGROUP) && limits.min_subgroup_size > 0;

    if adapter_has_subgroup {
        assert!(
            VyreBackend::supports_subgroup_ops(&backend),
            "Fix: supports_subgroup_ops must not be hardcoded false when adapter `{}` \
             has SUBGROUP and min_subgroup_size={}",
            info.name,
            limits.min_subgroup_size
        );
    }
}

// ------------------------------------------------------------------
// 2. Cross-surface consistency
// ------------------------------------------------------------------

#[test]
fn subgroup_capability_consistent_across_all_surfaces() {
    let backend = live_backend();

    let vyre_report = VyreBackend::supports_subgroup_ops(&backend);
    let bvc_report = BackendValidationCapabilities::supports_subgroup_ops(&backend);
    let caps_report = backend.adapter_caps().supports_subgroup_ops;

    assert_eq!(
        vyre_report, bvc_report,
        "Fix: VyreBackend and BackendValidationCapabilities must agree on subgroup_ops"
    );
    assert_eq!(
        vyre_report, caps_report,
        "Fix: VyreBackend and adapter_caps must agree on subgroup_ops"
    );
}

#[test]
fn subgroup_size_matches_capability_report() {
    let backend = live_backend();
    let has_subgroup = VyreBackend::supports_subgroup_ops(&backend);
    let size = backend.subgroup_size();

    if has_subgroup {
        assert!(
            size.is_some(),
            "Fix: backend reporting subgroup ops must expose Some(subgroup_size)"
        );
        let s = size.unwrap();
        assert!(
            s > 0,
            "Fix: reported subgroup_size must be positive when Some, got {s}"
        );
    } else {
        assert!(
            size.is_none() || size == Some(0),
            "Fix: backend without subgroup ops must report None or Some(0) for subgroup_size"
        );
    }
}

// ------------------------------------------------------------------
// 3. Functional verification when reported true
// ------------------------------------------------------------------

#[test]
fn subgroup_pipeline_compiles_when_capability_reported_true() {
    let backend = live_backend();

    if !VyreBackend::supports_subgroup_ops(&backend) {
        // Inapplicable on hardware without subgroup support.
        return;
    }

    // Compile probe: a raw WGSL shader that uses subgroup builtins.
    let wgsl = r#"
@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(1) @binding(2) var<uniform> params: vec4<u32>;

@compute @workgroup_size(32)
fn main(
    @builtin(global_invocation_id) gid: vec3<u32>,
    @builtin(subgroup_invocation_id) lane: u32,
    @builtin(subgroup_size) width: u32,
) {
    if (params.x == 4294967295u) {
        return;
    }
    let seed = input[0];
    if (gid.x == 0u) {
        output[0] = seed + subgroupAdd(select(1u, 0u, lane >= width));
    }
}
"#;

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        backend.dispatch_wgsl(wgsl, &[0; 4], 4, 32).is_ok()
    }));
    assert!(
        result.unwrap_or(false),
        "Fix: subgroup-capable backend must compile and dispatch a subgroup WGSL shader"
    );
}
