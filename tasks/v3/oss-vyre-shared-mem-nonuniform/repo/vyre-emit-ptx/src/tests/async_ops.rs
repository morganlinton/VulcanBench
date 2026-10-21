//! Test: async ops.
use super::*;

#[test]
fn async_load_emits_bounded_sync_copy() {
    let kernel = KernelDescriptor {
        id: "async_load".into(),
        bindings: BindingLayout {
            slots: vec![
                BindingSlot {
                    slot: 0,
                    element_type: DataType::U32,
                    element_count: None,
                    memory_class: MemoryClass::Global,
                    visibility: BindingVisibility::ReadOnly,
                    name: "src".into(),
                },
                BindingSlot {
                    slot: 1,
                    element_type: DataType::U32,
                    element_count: Some(64),
                    memory_class: MemoryClass::Shared,
                    visibility: BindingVisibility::ReadWrite,
                    name: "dst".into(),
                },
            ],
        },
        dispatch: Dispatch::new(64, 1, 1),
        body: KernelBody {
            ops: vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![1],
                    result: Some(1),
                },
                KernelOp {
                    kind: KernelOpKind::AsyncLoad {
                        tag: "tile0".into(),
                    },
                    operands: vec![0, 1, 0, 1],
                    result: None,
                },
            ],
            child_bodies: vec![],
            literals: vec![LiteralValue::U32(0), LiteralValue::U32(16)],
        },
    };
    let s = emit_with_target(&kernel, ComputeCapability::SM_70).unwrap();
    assert!(s.contains("// async_load tag=tile0"));
    assert!(s.contains(".shared .align 4 .b8 shared_buf_1[256];"));
    assert!(s.contains("ld.global.u32"));
    assert!(s.contains("st.shared.u32"));
    assert!(s.contains("lowered as bounded synchronous copy"));
}

#[test]
fn async_load_uses_cp_async_on_sm_80() {
    let kernel = KernelDescriptor {
        id: "k".into(),
        bindings: BindingLayout {
            slots: vec![
                BindingSlot {
                    slot: 0,
                    element_type: DataType::U32,
                    element_count: None,
                    memory_class: MemoryClass::Global,
                    visibility: BindingVisibility::ReadOnly,
                    name: "src".into(),
                },
                BindingSlot {
                    slot: 1,
                    element_type: DataType::U32,
                    element_count: Some(64),
                    memory_class: MemoryClass::Shared,
                    visibility: BindingVisibility::ReadWrite,
                    name: "dst".into(),
                },
            ],
        },
        dispatch: Dispatch::new(64, 1, 1),
        body: KernelBody {
            ops: vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![1],
                    result: Some(1),
                },
                KernelOp {
                    kind: KernelOpKind::AsyncLoad { tag: "t".into() },
                    operands: vec![0, 1, 0, 1],
                    result: None,
                },
            ],
            child_bodies: vec![],
            literals: vec![LiteralValue::U32(0), LiteralValue::U32(16)],
        },
    };
    let s = emit_with_target(&kernel, ComputeCapability::SM_80).unwrap();
    assert!(s.contains("// cp.async_load tag=t"));
    assert!(s.contains("cp.async.ca.shared.global"));
    assert!(s.contains("cp.async.commit_group"));
    assert!(s.contains("cp.async.wait_group 0"));
    assert!(
        !s.contains("lowered as bounded synchronous copy"),
        "sm_80 global-to-shared U32 AsyncLoad must use the native cp.async path"
    );
}

#[test]
fn cp_async_wait_is_deferred_until_async_wait_to_overlap_compute() {
    let kernel = KernelDescriptor {
        id: "cp_async_overlap".into(),
        bindings: BindingLayout {
            slots: vec![
                BindingSlot {
                    slot: 0,
                    element_type: DataType::U32,
                    element_count: None,
                    memory_class: MemoryClass::Global,
                    visibility: BindingVisibility::ReadOnly,
                    name: "src".into(),
                },
                BindingSlot {
                    slot: 1,
                    element_type: DataType::U32,
                    element_count: Some(64),
                    memory_class: MemoryClass::Shared,
                    visibility: BindingVisibility::ReadWrite,
                    name: "dst".into(),
                },
            ],
        },
        dispatch: Dispatch::new(64, 1, 1),
        body: KernelBody {
            ops: vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![1],
                    result: Some(1),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![2],
                    result: Some(2),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![3],
                    result: Some(3),
                },
                KernelOp {
                    kind: KernelOpKind::AsyncLoad { tag: "tile".into() },
                    operands: vec![0, 1, 0, 1],
                    result: None,
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Add),
                    operands: vec![2, 3],
                    result: Some(4),
                },
                KernelOp {
                    kind: KernelOpKind::AsyncWait { tag: "tile".into() },
                    operands: vec![],
                    result: None,
                },
            ],
            child_bodies: vec![],
            literals: vec![
                LiteralValue::U32(0),
                LiteralValue::U32(16),
                LiteralValue::U32(7),
                LiteralValue::U32(9),
            ],
        },
    };
    let s = emit_with_target(&kernel, ComputeCapability::SM_80).unwrap();
    let commit = s
        .find("cp.async.commit_group;")
        .expect("native cp.async path must commit a group");
    let wait = s
        .find("cp.async.wait_group 0;")
        .expect("AsyncWait must drain the pending cp.async group");
    let overlapped_add = s[commit..wait]
        .find("add.u32")
        .expect("independent compute must remain between cp.async commit and wait");
    assert!(overlapped_add > 0);
}

#[test]
fn async_store_emits_bounded_sync_copy() {
    let kernel = KernelDescriptor {
        id: "async_store".into(),
        bindings: BindingLayout {
            slots: vec![
                BindingSlot {
                    slot: 0,
                    element_type: DataType::U32,
                    element_count: Some(64),
                    memory_class: MemoryClass::Shared,
                    visibility: BindingVisibility::ReadWrite,
                    name: "src".into(),
                },
                BindingSlot {
                    slot: 1,
                    element_type: DataType::U32,
                    element_count: Some(64),
                    memory_class: MemoryClass::Global,
                    visibility: BindingVisibility::ReadWrite,
                    name: "dst".into(),
                },
            ],
        },
        dispatch: Dispatch::new(64, 1, 1),
        body: KernelBody {
            ops: vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![1],
                    result: Some(1),
                },
                KernelOp {
                    kind: KernelOpKind::AsyncStore {
                        tag: "tile0".into(),
                    },
                    operands: vec![0, 1, 0, 1],
                    result: None,
                },
            ],
            child_bodies: vec![],
            literals: vec![LiteralValue::U32(0), LiteralValue::U32(16)],
        },
    };
    let s = emit_with_target(&kernel, ComputeCapability::SM_70).unwrap();
    assert!(s.contains("// async_store tag=tile0"));
    assert!(s.contains("ld.shared.u32"));
    assert!(s.contains("st.global.u32"));
}

#[test]
fn async_wait_emits_workgroup_memory_barrier() {
    let kernel = KernelDescriptor {
        id: "async_wait".into(),
        bindings: BindingLayout { slots: vec![] },
        dispatch: Dispatch::new(64, 1, 1),
        body: KernelBody {
            ops: vec![KernelOp {
                kind: KernelOpKind::AsyncWait { tag: "t".into() },
                operands: vec![],
                result: None,
            }],
            child_bodies: vec![],
            literals: vec![],
        },
    };
    let s = emit_with_target(&kernel, ComputeCapability::SM_80).unwrap();
    assert!(s.contains("membar.cta"));
}
