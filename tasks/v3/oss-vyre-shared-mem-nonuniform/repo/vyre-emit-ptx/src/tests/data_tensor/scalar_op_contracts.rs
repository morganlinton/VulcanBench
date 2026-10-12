use super::*;

#[test]
fn select_emits_selp_with_correct_dtype() {
    let kernel = KernelDescriptor {
        id: "select".into(),
        bindings: BindingLayout { slots: vec![] },
        dispatch: Dispatch::new(1, 1, 1),
        body: KernelBody {
            ops: vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                }, // cond bool
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![1],
                    result: Some(1),
                }, // u32
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![2],
                    result: Some(2),
                }, // u32
                KernelOp {
                    kind: KernelOpKind::Select,
                    operands: vec![0, 1, 2],
                    result: Some(3),
                },
            ],
            child_bodies: vec![],
            literals: vec![
                LiteralValue::Bool(true),
                LiteralValue::U32(10),
                LiteralValue::U32(20),
            ],
        },
    };
    let s = emit(&kernel).unwrap();
    assert!(s.contains("selp.u32"));
}

#[test]
fn atomic_compare_exchange_emits_atom_global_cas_b32() {
    use vyre_foundation::ir::{AtomicOp, MemoryOrdering};
    let kernel = KernelDescriptor {
        id: "cas".into(),
        bindings: BindingLayout {
            slots: vec![BindingSlot {
                slot: 0,
                element_type: DataType::U32,
                element_count: Some(4),
                memory_class: MemoryClass::Global,
                visibility: BindingVisibility::ReadWrite,
                name: "buf".into(),
            }],
        },
        dispatch: Dispatch::new(1, 1, 1),
        body: KernelBody {
            ops: vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                }, // index
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![1],
                    result: Some(1),
                }, // cmp
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![2],
                    result: Some(2),
                }, // new
                KernelOp {
                    kind: KernelOpKind::Atomic {
                        op: AtomicOp::CompareExchange,
                        ordering: MemoryOrdering::SeqCst,
                    },
                    operands: vec![0, 0, 1, 2],
                    result: Some(3),
                },
            ],
            child_bodies: vec![],
            literals: vec![
                LiteralValue::U32(0),
                LiteralValue::U32(7),
                LiteralValue::U32(8),
            ],
        },
    };
    let s = emit(&kernel).unwrap();
    assert!(
        s.contains("atom.global.cas.b32"),
        "must emit atom.global.cas.b32:\n{s}"
    );
}

#[test]
fn select_on_predicates_does_not_emit_selp_pred() {
    // PTX `selp` does not support `.pred` operands. ptxas rejects
    // `selp.pred` with "Unexpected instruction types specified for 'selp'".
    // When both arms are bool, lower as not/and/and/or.
    let kernel = KernelDescriptor {
        id: "select_pred".into(),
        bindings: BindingLayout { slots: vec![] },
        dispatch: Dispatch::new(1, 1, 1),
        body: KernelBody {
            ops: vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                }, // cond bool
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![1],
                    result: Some(1),
                }, // bool true
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![2],
                    result: Some(2),
                }, // bool false
                KernelOp {
                    kind: KernelOpKind::Select,
                    operands: vec![0, 1, 2],
                    result: Some(3),
                },
            ],
            child_bodies: vec![],
            literals: vec![
                LiteralValue::Bool(true),
                LiteralValue::Bool(true),
                LiteralValue::Bool(false),
            ],
        },
    };
    let s = emit(&kernel).unwrap();
    assert!(
        !s.contains("selp.pred"),
        "must not emit invalid selp.pred:\n{s}"
    );
    assert!(
        s.contains("not.pred") && s.contains("and.pred") && s.contains("or.pred"),
        "predicate select must lower to not/and/or:\n{s}"
    );
}

#[test]
fn fma_emits_fma_rn_with_dtype() {
    let kernel = KernelDescriptor {
        id: "fma".into(),
        bindings: BindingLayout { slots: vec![] },
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
                    kind: KernelOpKind::Literal,
                    operands: vec![2],
                    result: Some(2),
                },
                KernelOp {
                    kind: KernelOpKind::Fma,
                    operands: vec![0, 1, 2],
                    result: Some(3),
                },
            ],
            child_bodies: vec![],
            literals: vec![
                LiteralValue::F32(1.0),
                LiteralValue::F32(2.0),
                LiteralValue::F32(3.0),
            ],
        },
    };
    let s = emit(&kernel).unwrap();
    assert!(s.contains("fma.rn.f32"));
}
