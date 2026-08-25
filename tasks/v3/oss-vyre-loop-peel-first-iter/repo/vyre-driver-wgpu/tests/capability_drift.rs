//! Failure-oriented tests for backend trait / capability drift.
//!
//! These tests lock the contract that [`vyre::VyreBackend`] capability
//! queries and [`vyre_foundation::validate::BackendValidationCapabilities`]
//! remain in sync, and that capabilities without a lowering path stay
//! honestly `false` (LAW 9).

mod common;
use common::shared_live_backend as live_backend;

use vyre::VyreBackend;
use vyre_driver_wgpu::WgpuBackend;
use vyre_foundation::validate::BackendValidationCapabilities;

#[test]
fn backend_validation_capabilities_do_not_drift_from_vyre_backend() {
    let backend = live_backend();

    assert_eq!(
        BackendValidationCapabilities::backend_name(&backend),
        VyreBackend::id(&backend),
        "Fix: backend name must not drift between BackendValidationCapabilities and VyreBackend"
    );

    assert_eq!(
        BackendValidationCapabilities::supports_subgroup_ops(&backend),
        <WgpuBackend as VyreBackend>::supports_subgroup_ops(&backend),
        "Fix: subgroup_ops capability report must not drift between trait implementations"
    );

    assert_eq!(
        BackendValidationCapabilities::supports_indirect_dispatch(&backend),
        <WgpuBackend as VyreBackend>::supports_indirect_dispatch(&backend),
        "Fix: indirect_dispatch capability report must not drift between trait implementations"
    );

    let snapshot = BackendValidationCapabilities::backend_capabilities(&backend);
    assert_eq!(
        snapshot.supports_specialization_constants,
        BackendValidationCapabilities::supports_specialization_constants(&backend),
        "Fix: specialization_constants must not drift between direct validation query and exported snapshot"
    );

    assert!(
        BackendValidationCapabilities::supports_cast_target(&backend, &vyre::ir::DataType::U64),
        "Fix: wgpu validation must continue accepting U64 cast targets lowered through the vec2<u32> representation"
    );
    assert!(
        BackendValidationCapabilities::supports_cast_target(&backend, &vyre::ir::DataType::I64),
        "Fix: wgpu validation must accept I64 cast targets — same vec2<u32> backing as U64, sign-extended on hardware"
    );
    assert!(
        !BackendValidationCapabilities::supports_cast_target(&backend, &vyre::ir::DataType::F16),
        "Fix: wgpu validation must reject F16 cast targets until end-to-end f16 lowering exists"
    );
}

#[test]
fn unsupported_capabilities_stay_false_until_lowering_exists() {
    let backend = live_backend();

    assert!(
        !backend.supports_f16(),
        "Fix: supports_f16 must stay false until the wgpu/Naga WGSL path emits `enable f16` and f16 arithmetic lowering. Flipping early is a LAW 9 violation."
    );

    assert!(
        !backend.supports_bf16(),
        "Fix: supports_bf16 must stay false until a dedicated BF16 lowering path lands."
    );

    assert!(
        !backend.supports_tensor_cores(),
        "Fix: supports_tensor_cores must stay false until MMA/tensor-core intrinsics are emitted."
    );
}

#[test]
fn subgroup_capability_is_consistent_internally() {
    let backend = live_backend();

    let has_subgroup = <WgpuBackend as VyreBackend>::supports_subgroup_ops(&backend);
    let size = backend.subgroup_size();

    if has_subgroup {
        assert!(
            size.is_some(),
            "Fix: backend that reports subgroup ops must expose a subgroup size"
        );
    } else {
        assert!(
            size.is_none(),
            "Fix: backend without subgroup ops must report None for subgroup_size"
        );
    }
}

#[test]
fn specialization_constants_trait_matches_pipeline_validation() {
    // VYRE-DRV-CAP-001: the backend trait claims supports_specialization_constants=true,
    // but pipeline.rs historically passed supports_specialization_constants=false to
    // execution_plan::plan_with_options. This test documents the invariant that the
    // trait and the validation options must agree.
    let backend = live_backend();

    let trait_val = BackendValidationCapabilities::backend_capabilities(&backend)
        .supports_specialization_constants;
    let bvc_val = BackendValidationCapabilities::supports_specialization_constants(&backend);

    assert!(
        bvc_val,
        "Fix: wgpu owns the specialization lowering/cache path, so BackendValidationCapabilities must report supports_specialization_constants=true"
    );
    assert_eq!(
        trait_val, bvc_val,
        "Fix: BackendValidationCapabilities snapshot and direct query must agree on specialization_constants. snapshot={trait_val}, query={bvc_val}"
    );
}
