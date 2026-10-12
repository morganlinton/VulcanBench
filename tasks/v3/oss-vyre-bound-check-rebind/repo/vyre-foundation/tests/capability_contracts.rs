//! Failure-oriented tests for capability requirement contracts.
//!
//! Ensures `check_backend_capabilities` reports every missing bit and that
//! `scan` correctly detects required features from program structure.

use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};
use vyre_foundation::program_caps::{
    check_backend_capabilities, scan, MissingCapability, RequiredCapabilities,
};

#[test]
fn missing_subgroup_ops_is_reported() {
    let program = Program::wrapped(
        vec![BufferDecl::storage(
            "out",
            0,
            BufferAccess::ReadWrite,
            DataType::U32,
        )],
        [1, 1, 1],
        vec![Node::let_bind(
            "s",
            Expr::subgroup_add(Expr::u32(1)),
        )],
    );
    let required = scan(&program);
    assert!(required.subgroup_ops, "subgroup_add must set subgroup_ops");
    let err = check_backend_capabilities(
        "test_backend",
        false,
        false,
        false,
        false,
        false,
        false,
        [64, 1, 1],
        &required,
    )
    .unwrap_err();
    assert_eq!(err.backend, "test_backend");
    assert!(err.missing.iter().any(|s| s == "subgroup_ops"));
    let msg = err.to_string();
    assert!(
        msg.contains("subgroup_ops"),
        "display must name the capability: {msg}"
    );
    assert!(
        msg.contains("Fix:"),
        "display must carry a Fix: hint: {msg}"
    );
}

#[test]
fn subgroup_builtin_expressions_require_subgroup_ops() {
    for expr in [Expr::subgroup_local_id(), Expr::subgroup_size()] {
        let program = Program::wrapped(
            vec![BufferDecl::storage(
                "out",
                0,
                BufferAccess::ReadWrite,
                DataType::U32,
            )],
            [1, 1, 1],
            vec![Node::store("out", Expr::u32(0), expr)],
        );
        let required = scan(&program);
        assert!(
            required.subgroup_ops,
            "subgroup builtin expressions must set subgroup_ops"
        );
    }
}

#[test]
fn missing_f16_is_reported() {
    let mut required = RequiredCapabilities::none();
    required.f16 = true;
    let err = check_backend_capabilities(
        "test",
        false,
        false,
        false,
        false,
        false,
        false,
        [64, 1, 1],
        &required,
    )
    .unwrap_err();
    assert!(err.missing.iter().any(|s| s == "f16"));
}

#[test]
fn missing_bf16_is_reported() {
    let mut required = RequiredCapabilities::none();
    required.bf16 = true;
    let err = check_backend_capabilities(
        "test",
        false,
        false,
        false,
        false,
        false,
        false,
        [64, 1, 1],
        &required,
    )
    .unwrap_err();
    assert!(err.missing.iter().any(|s| s == "bf16"));
}

#[test]
fn missing_indirect_dispatch_is_reported() {
    let program = Program::wrapped(
        vec![BufferDecl::read("counts", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::indirect_dispatch("counts", 0)],
    );
    let required = scan(&program);
    assert!(required.indirect_dispatch);
    let err = check_backend_capabilities(
        "test",
        false,
        false,
        false,
        false,
        false,
        false,
        [64, 1, 1],
        &required,
    )
    .unwrap_err();
    assert!(err.missing.iter().any(|s| s == "indirect_dispatch"));
}

#[test]
fn missing_trap_propagation_is_reported() {
    let program = Program::wrapped(
        vec![BufferDecl::storage(
            "out",
            0,
            BufferAccess::ReadWrite,
            DataType::U32,
        )],
        [1, 1, 1],
        vec![Node::trap(Expr::u32(0), "fault")],
    );
    let required = scan(&program);
    assert!(required.trap);
    let err = check_backend_capabilities(
        "test",
        false,
        false,
        false,
        false,
        false,
        false,
        [64, 1, 1],
        &required,
    )
    .unwrap_err();
    assert!(err.missing.iter().any(|s| s == "trap_propagation"));
}

#[test]
fn missing_distributed_collectives_is_reported() {
    let mut required = RequiredCapabilities::none();
    required.distributed_collectives = true;
    let err = check_backend_capabilities(
        "test",
        false,
        false,
        false,
        false,
        false,
        false,
        [64, 1, 1],
        &required,
    )
    .unwrap_err();
    assert!(err.missing.iter().any(|s| s == "distributed_collectives"));
}

#[test]
fn workgroup_size_exceeding_backend_limit_is_reported() {
    let mut required = RequiredCapabilities::none();
    required.max_workgroup_size = [256, 1, 1];
    let err = check_backend_capabilities(
        "test",
        false,
        false,
        false,
        false,
        false,
        false,
        [128, 1, 1],
        &required,
    )
    .unwrap_err();
    assert!(err.missing.iter().any(|s| s == "workgroup_size"));
}

#[test]
fn zero_backend_workgroup_size_is_unlimited() {
    let mut required = RequiredCapabilities::none();
    required.max_workgroup_size = [256, 1, 1];
    assert_eq!(
        check_backend_capabilities(
            "test",
            false,
            false,
            false,
            false,
            false,
            false,
            [0, 0, 0],
            &required,
        ),
        Ok(()),
        "zero backend workgroup size must mean unlimited"
    );
}

#[test]
fn all_capabilities_together_return_ok_when_supported() {
    let mut required = RequiredCapabilities::none();
    required.subgroup_ops = true;
    required.f16 = true;
    required.bf16 = true;
    required.indirect_dispatch = true;
    required.trap = true;
    required.distributed_collectives = true;
    required.max_workgroup_size = [64, 1, 1];
    assert_eq!(
        check_backend_capabilities(
            "test",
            true,
            true,
            true,
            true,
            true,
            true,
            [128, 1, 1],
            &required,
        ),
        Ok(()),
        "fully supported backend must pass"
    );
}

#[test]
fn empty_program_requires_no_capabilities() {
    let program = Program::wrapped(
        vec![BufferDecl::storage(
            "out",
            0,
            BufferAccess::ReadWrite,
            DataType::U32,
        )],
        [1, 1, 1],
        vec![Node::let_bind("x", Expr::u32(0))],
    );
    let required = scan(&program);
    assert!(!required.subgroup_ops);
    assert!(!required.f16);
    assert!(!required.bf16);
    assert!(!required.indirect_dispatch);
    assert!(!required.trap);
    assert_eq!(required.max_workgroup_size, [1, 1, 1]);
}

#[test]
fn required_capabilities_union_is_fieldwise_or() {
    let mut a = RequiredCapabilities::none();
    a.subgroup_ops = true;
    a.f16 = true;
    a.max_workgroup_size = [64, 1, 1];
    a.static_storage_bytes = 100;

    let mut b = RequiredCapabilities::none();
    b.bf16 = true;
    b.max_workgroup_size = [32, 2, 1];
    b.static_storage_bytes = 50;

    let u = a.union(b);
    assert!(u.subgroup_ops);
    assert!(u.f16);
    assert!(u.bf16);
    assert_eq!(u.max_workgroup_size, [64, 2, 1]);
    assert_eq!(u.static_storage_bytes, 150);
}

#[test]
fn missing_capability_implements_std_error() {
    let err = MissingCapability {
        backend: "foo".into(),
        missing: vec!["bar".to_string()],
    };
    let dyn_err: &(dyn std::error::Error) = &err;
    assert!(dyn_err.source().is_none());
    let msg = dyn_err.to_string();
    assert!(msg.contains("foo"));
    assert!(msg.contains("bar"));
}

#[test]
fn async_dispatch_sets_async_dispatch_cap() {
    let program = Program::wrapped(
        vec![BufferDecl::storage(
            "out",
            0,
            BufferAccess::ReadWrite,
            DataType::U32,
        )],
        [1, 1, 1],
        vec![Node::async_load("tag")],
    );
    let required = scan(&program);
    assert!(required.async_dispatch);
}

#[test]
fn tensor_ops_detected_from_buffer_element_type() {
    let program = Program::wrapped(
        vec![BufferDecl::storage(
            "out",
            0,
            BufferAccess::ReadWrite,
            DataType::Tensor,
        )],
        [1, 1, 1],
        vec![],
    );
    let required = scan(&program);
    assert!(required.tensor_ops);
}

#[test]
fn f64_detected_from_buffer_element_type() {
    let program = Program::wrapped(
        vec![BufferDecl::storage(
            "out",
            0,
            BufferAccess::ReadWrite,
            DataType::F64,
        )],
        [1, 1, 1],
        vec![],
    );
    let required = scan(&program);
    assert!(required.f64);
}
