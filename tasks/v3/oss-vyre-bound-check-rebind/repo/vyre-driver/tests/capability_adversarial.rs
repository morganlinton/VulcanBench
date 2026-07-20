//! Backend capability negotiation adversarial tests.
//!
//! These tests exercise the public capability snapshot used by driver strategy
//! selection and foundation validation. The important contract is fail-closed:
//! absent capabilities must not accidentally enable backend-sensitive IR.

use vyre_foundation::validate::{BackendCapabilities, ValidationOptions};

#[test]
fn default_capabilities_are_fail_closed() {
    let caps = BackendCapabilities::default();

    assert!(!caps.supports_subgroup_ops);
    assert!(!caps.supports_indirect_dispatch);
    assert!(!caps.supports_specialization_constants);
    assert!(!caps.supports_distributed_collectives);
    assert!(!caps.has_mul_high);
    assert!(!caps.has_dual_issue_fp32_int32);
    assert!(!caps.has_tensor_core_int);
    assert!(!caps.has_native_f16);
    assert!(!caps.has_warp_shuffle);
    assert!(!caps.has_shared_memory);
    assert!(!caps.has_transcendental_polynomial_emit);
    assert_eq!(caps.max_native_int_width, 0);
}

#[test]
fn capability_snapshots_are_copyable_without_aliasing_state() {
    let caps = BackendCapabilities {
        supports_subgroup_ops: true,
        supports_indirect_dispatch: true,
        supports_specialization_constants: true,
        supports_distributed_collectives: false,
        has_mul_high: true,
        has_dual_issue_fp32_int32: true,
        has_tensor_core_int: false,
        has_native_f16: true,
        has_warp_shuffle: true,
        has_shared_memory: true,
        has_transcendental_polynomial_emit: false,
        max_native_int_width: 64,
    };

    let copied = caps;

    assert_eq!(copied, caps);
    assert!(copied.supports_subgroup_ops);
    assert_eq!(copied.max_native_int_width, 64);
}

#[test]
fn validation_options_preserve_backend_capability_snapshot() {
    let caps = BackendCapabilities {
        supports_subgroup_ops: true,
        supports_indirect_dispatch: false,
        supports_specialization_constants: true,
        supports_distributed_collectives: false,
        max_native_int_width: 64,
        ..BackendCapabilities::default()
    };

    let options = ValidationOptions::universal().with_backend_capabilities(caps);
    let snapshot = options
        .backend_capabilities
        .expect("capability snapshot must be preserved");

    assert!(options.requires_subgroup_ops());
    assert!(snapshot.supports_specialization_constants);
    assert_eq!(snapshot.max_native_int_width, 64);
}

#[test]
fn capability_flags_are_independent() {
    let subgroup_only = BackendCapabilities {
        supports_subgroup_ops: true,
        ..BackendCapabilities::default()
    };
    let collectives_only = BackendCapabilities {
        supports_distributed_collectives: true,
        ..BackendCapabilities::default()
    };

    assert!(subgroup_only.supports_subgroup_ops);
    assert!(!subgroup_only.supports_distributed_collectives);
    assert!(!collectives_only.supports_subgroup_ops);
    assert!(collectives_only.supports_distributed_collectives);
}
