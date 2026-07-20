use super::*;

#[test]
fn bool_global_load_uses_word_load_then_predicate_set() {
    let kernel = KernelDescriptor {
        id: "bool_load".into(),
        bindings: BindingLayout {
            slots: vec![
                BindingSlot {
                    slot: 0,
                    element_type: DataType::Bool,
                    element_count: Some(1),
                    memory_class: MemoryClass::Global,
                    visibility: BindingVisibility::ReadOnly,
                    name: "input".into(),
                },
                BindingSlot {
                    slot: 1,
                    element_type: DataType::U32,
                    element_count: Some(1),
                    memory_class: MemoryClass::Global,
                    visibility: BindingVisibility::WriteOnly,
                    name: "out".into(),
                },
            ],
        },
        dispatch: Dispatch::new(1, 1, 1),
        body: KernelBody {
            ops: vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::LoadGlobal,
                    operands: vec![0, 0],
                    result: Some(1),
                },
                KernelOp {
                    kind: KernelOpKind::Cast {
                        target: DataType::U32,
                    },
                    operands: vec![1],
                    result: Some(2),
                },
                KernelOp {
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![1, 0, 2],
                    result: None,
                },
            ],
            child_bodies: vec![],
            literals: vec![LiteralValue::U32(0)],
        },
    };

    let s = emit(&kernel).unwrap();
    assert!(
        !s.contains("ld.global.pred"),
        "PTX cannot load predicate registers from memory:\n{s}"
    );
    assert!(
        s.contains("ld.global.u32"),
        "Bool memory load must use the physical word ABI:\n{s}"
    );
    assert!(
        s.contains("setp.ne.u32"),
        "Bool memory load must canonicalize non-zero words to predicates:\n{s}"
    );
}

#[test]
fn bool_global_store_materializes_predicate_word() {
    let kernel = KernelDescriptor {
        id: "bool_store".into(),
        bindings: BindingLayout {
            slots: vec![BindingSlot {
                slot: 0,
                element_type: DataType::Bool,
                element_count: Some(1),
                memory_class: MemoryClass::Global,
                visibility: BindingVisibility::WriteOnly,
                name: "out".into(),
            }],
        },
        dispatch: Dispatch::new(1, 1, 1),
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
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![0, 0, 1],
                    result: None,
                },
            ],
            child_bodies: vec![],
            literals: vec![LiteralValue::U32(0), LiteralValue::Bool(true)],
        },
    };

    let s = emit(&kernel).unwrap();
    assert!(
        !s.contains("st.global.pred"),
        "PTX cannot store predicate registers to memory:\n{s}"
    );
    assert!(
        s.contains("selp.u32"),
        "Bool memory store must materialize a 0/1 word:\n{s}"
    );
    assert!(
        s.contains("st.global.u32"),
        "Bool memory store must use the physical word ABI:\n{s}"
    );
}
