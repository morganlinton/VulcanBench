//! Capability contract tests for the live wgpu backend.

mod common;
use common::shared_live_backend as live_backend;

use vyre::ir::{BufferAccess, BufferDecl, DataType, Node, Program};
use vyre::{DispatchConfig, VyreBackend};
use vyre_driver_wgpu::WgpuBackend;
use vyre_foundation::validate::{BackendValidationCapabilities, ValidationOptions};

fn selected_adapter(backend: &WgpuBackend) -> wgpu::Adapter {
    vyre_driver_wgpu::runtime::device::adapter_for_info(backend.adapter_info()).expect(
        "Fix: selected wgpu backend adapter must remain enumerable for live capability probing",
    )
}

fn subgroup_pipeline_compiles(backend: &WgpuBackend) -> bool {
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
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        backend.dispatch_wgsl(wgsl, &[0; 4], 4, 32).is_ok()
    }))
    .unwrap_or(false)
}

#[test]
fn live_backend_reports_subgroup_and_indirect_support_truthfully() {
    let backend = live_backend();
    let info = backend.adapter_info();
    let adapter = selected_adapter(&backend);
    let adapter_features = adapter.features();
    let limits = adapter.limits();
    let adapter_claims_subgroup =
        adapter_features.contains(wgpu::Features::SUBGROUP) && limits.min_subgroup_size > 0;
    let subgroup_compiles = adapter_claims_subgroup && subgroup_pipeline_compiles(&backend);
    let expected_subgroup = adapter_claims_subgroup && subgroup_compiles;
    let expected_indirect = matches!(
        info.device_type,
        wgpu::DeviceType::DiscreteGpu
            | wgpu::DeviceType::IntegratedGpu
            | wgpu::DeviceType::VirtualGpu
    ) && u64::from(limits.max_storage_buffer_binding_size) >= 12
        && [
            limits.max_compute_workgroup_size_x,
            limits.max_compute_workgroup_size_y,
            limits.max_compute_workgroup_size_z,
        ]
        .iter()
        .all(|axis| *axis > 0);
    assert_eq!(
        <WgpuBackend as VyreBackend>::supports_subgroup_ops(&backend),
        expected_subgroup,
        "Fix: supports_subgroup_ops must require the live SUBGROUP feature, usable subgroup limits, and a compiling subgroup pipeline. Adapter: {}",
        info.name
    );
    assert_eq!(
        <WgpuBackend as VyreBackend>::supports_indirect_dispatch(&backend),
        expected_indirect,
        "Fix: supports_indirect_dispatch must match the live adapter/device dispatch tuple capability. Adapter: {}",
        info.name
    );
    assert!(
        <WgpuBackend as VyreBackend>::supports_subgroup_ops(&backend),
        "Fix: RTX 5090 wgpu backend enables wgpu::Features::SUBGROUP and the Naga lowering emits subgroup statements, so supports_subgroup_ops must be true."
    );
    assert!(
        <WgpuBackend as VyreBackend>::supports_indirect_dispatch(&backend),
        "Fix: wgpu backend extracts Node::IndirectDispatch before Naga codegen and submits dispatch_workgroups_indirect."
    );
    assert!(
        !<WgpuBackend as VyreBackend>::supports_async_compute(&backend),
        "Fix: wgpu dispatch_async is host-side asynchronous submission/readback, not a distinct GPU async-compute queue."
    );
    assert!(
        backend.subgroup_size().is_some(),
        "Fix: subgroup-capable backend must expose a native subgroup size for dispatch planning."
    );
    let validation = ValidationOptions::default().with_backend(&backend);
    assert!(
        validation
            .backend_capabilities
            .is_some_and(|caps| caps.supports_subgroup_ops && caps.supports_indirect_dispatch),
        "Fix: WgpuBackend must implement BackendValidationCapabilities so foundation validation sees the same live capability contract as VyreBackend."
    );
    assert!(
        BackendValidationCapabilities::supports_cast_target(&backend, &DataType::U64),
        "Fix: wgpu lowers U64 storage and safe casts through the vec2<u32> representation."
    );
    assert!(
        BackendValidationCapabilities::supports_cast_target(&backend, &DataType::I64),
        "Fix: wgpu must advertise I64 cast support — it shares U64's vec2<u32> backing and \
         sign-extends the high word on the live GPU (proven by widening_cast_64_parity)."
    );
    assert!(
        !BackendValidationCapabilities::supports_cast_target(&backend, &DataType::F16),
        "Fix: wgpu validation must reject F16 before lowering while this WGSL path rejects `enable f16`."
    );
}

#[test]
fn adapter_caps_probe_matches_live_backend_capability_contract() {
    let backend = live_backend();
    let adapter = selected_adapter(&backend);
    let caps = vyre_driver_wgpu::runtime::adapter_caps_probe::probe(&adapter);
    let live_caps = backend.adapter_caps();
    assert_eq!(
        caps.supports_subgroup_ops,
        <WgpuBackend as VyreBackend>::supports_subgroup_ops(&backend),
        "Fix: adapter_caps_probe subgroup report must not drift from WgpuBackend's live capability contract"
    );
    assert_eq!(
        caps.supports_indirect_dispatch,
        <WgpuBackend as VyreBackend>::supports_indirect_dispatch(&backend),
        "Fix: adapter_caps_probe indirect-dispatch report must not drift from WgpuBackend's live capability contract"
    );
    assert_eq!(
        caps.subgroup_size,
        backend.subgroup_size().unwrap_or(0),
        "Fix: adapter_caps_probe subgroup_size must match the backend's dispatch-planning width"
    );
    assert_eq!(
        live_caps.supports_subgroup_ops,
        <WgpuBackend as VyreBackend>::supports_subgroup_ops(&backend),
        "Fix: optimizer caps built from WgpuBackend must reflect post-device subgroup validation"
    );
    assert_eq!(
        live_caps.subgroup_size,
        backend.subgroup_size().unwrap_or(0),
        "Fix: optimizer caps built from WgpuBackend must use the dispatch-planning subgroup width"
    );
}

#[test]
fn unsupported_capabilities_stay_false_until_lowering_exists() {
    let backend = live_backend();
    assert!(
        !<WgpuBackend as VyreBackend>::supports_f16(&backend),
        "Fix: F16 stays false while this wgpu/Naga WGSL path rejects `enable f16`; do not advertise adapter-only support."
    );
    assert!(
        !<WgpuBackend as VyreBackend>::supports_bf16(&backend),
        "Fix: BF16 has no direct WGSL scalar lowering in this backend."
    );
    assert!(
        !<WgpuBackend as VyreBackend>::supports_tensor_cores(&backend),
        "Fix: wgpu storage-buffer lowering does not expose tensor-core/MMA intrinsics."
    );
}

#[test]
fn f16_programs_are_rejected_by_capability_gate_before_lowering() {
    let backend = live_backend();
    let program = Program::wrapped(
        vec![
            BufferDecl::storage("half_out", 0, BufferAccess::ReadWrite, DataType::F16)
                .with_count(1),
        ],
        [1, 1, 1],
        vec![Node::Return],
    );

    let error = backend
        .dispatch(&program, &[], &DispatchConfig::default())
        .expect_err("Fix: wgpu must reject F16 at capability validation before Naga lowering");
    let text = error.to_string();
    assert!(
        text.contains("missing required capabilities") && text.contains("f16"),
        "Fix: unsupported F16 programs must fail at the backend capability gate with an actionable capability error. Got {text}"
    );
}
