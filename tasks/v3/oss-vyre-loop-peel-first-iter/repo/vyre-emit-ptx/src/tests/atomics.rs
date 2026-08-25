//! Test: atomics.
use super::*;

#[test]
fn nested_if_inside_for_emits_correct_label_nesting() {
    // for { if { ... } }
    let kernel = KernelDescriptor {
        id: "nested".into(),
        bindings: BindingLayout { slots: vec![] },
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
                    kind: KernelOpKind::StructuredForLoop {
                        loop_var: "i".into(),
                    },
                    operands: vec![0, 1, 0],
                    result: None,
                },
            ],
            child_bodies: vec![KernelBody {
                ops: vec![
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![0],
                        result: Some(10),
                    },
                    KernelOp {
                        kind: KernelOpKind::StructuredIfThen,
                        operands: vec![10, 0],
                        result: None,
                    },
                ],
                child_bodies: vec![empty_child_body()],
                literals: vec![LiteralValue::Bool(true)],
            }],
            literals: vec![LiteralValue::U32(0), LiteralValue::U32(8)],
        },
    };
    let s = emit(&kernel).unwrap();
    assert!(s.contains("$L_for_head_"));
    assert!(s.contains("$L_if_end_"));
}

#[test]
fn atomic_add_emits_atom_global_add_u32() {
    use vyre_foundation::ir::AtomicOp;
    let kernel = KernelDescriptor {
        id: "atomic_add".into(),
        bindings: BindingLayout {
            slots: vec![BindingSlot {
                slot: 0,
                element_type: DataType::U32,
                element_count: None,
                memory_class: MemoryClass::Global,
                visibility: BindingVisibility::ReadWrite,
                name: "counter".into(),
            }],
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
                    kind: KernelOpKind::Atomic {
                        op: AtomicOp::Add,
                        ordering: vyre_foundation::runtime::memory_model::MemoryOrdering::SeqCst,
                    },
                    operands: vec![0, 0, 1],
                    result: Some(2),
                },
            ],
            child_bodies: vec![],
            literals: vec![LiteralValue::U32(0), LiteralValue::U32(1)],
        },
    };
    let s = emit(&kernel).unwrap();
    assert!(s.contains("atom.global.add.u32"));
}

#[test]
fn atomic_exchange_emits_atom_global_exch_b32() {
    use vyre_foundation::ir::AtomicOp;
    let kernel = KernelDescriptor {
        id: "atomic_exchange".into(),
        bindings: BindingLayout {
            slots: vec![BindingSlot {
                slot: 0,
                element_type: DataType::U32,
                element_count: None,
                memory_class: MemoryClass::Global,
                visibility: BindingVisibility::ReadWrite,
                name: "slot".into(),
            }],
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
                    kind: KernelOpKind::Atomic {
                        op: AtomicOp::Exchange,
                        ordering: vyre_foundation::runtime::memory_model::MemoryOrdering::SeqCst,
                    },
                    operands: vec![0, 0, 1],
                    result: Some(2),
                },
            ],
            child_bodies: vec![],
            literals: vec![LiteralValue::U32(0), LiteralValue::U32(1)],
        },
    };
    let s = emit(&kernel).unwrap();
    assert!(
        s.contains("atom.global.exch.b32"),
        "PTX exch must use bit-size suffix, not .u32:\n{s}"
    );
    assert!(
        !s.contains("atom.global.exch.u32"),
        "ptxas rejects atom.global.exch.u32:\n{s}"
    );
}

#[test]
fn atomic_bitwise_emits_atom_global_b32_suffix() {
    use vyre_foundation::ir::AtomicOp;
    for (op, mnemonic) in [
        (AtomicOp::And, "and"),
        (AtomicOp::Or, "or"),
        (AtomicOp::Xor, "xor"),
    ] {
        let kernel = KernelDescriptor {
            id: "atomic_bitwise".into(),
            bindings: BindingLayout {
                slots: vec![BindingSlot {
                    slot: 0,
                    element_type: DataType::U32,
                    element_count: None,
                    memory_class: MemoryClass::Global,
                    visibility: BindingVisibility::ReadWrite,
                    name: "slot".into(),
                }],
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
                        kind: KernelOpKind::Atomic {
                            op,
                            ordering:
                                vyre_foundation::runtime::memory_model::MemoryOrdering::Relaxed,
                        },
                        operands: vec![0, 0, 1],
                        result: Some(2),
                    },
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(0), LiteralValue::U32(1)],
            },
        };
        let s = emit(&kernel).unwrap();
        assert!(
            s.contains(&format!("atom.global.{mnemonic}.b32")),
            "PTX atom.{mnemonic} must use .b32, not .u32/.s32:\n{s}"
        );
    }
}

#[test]
fn atomic_bitwise_bool_operand_materializes_u32_before_atom() {
    use vyre_foundation::ir::AtomicOp;
    let kernel = KernelDescriptor {
        id: "atomic_bool_to_b32".into(),
        bindings: BindingLayout {
            slots: vec![BindingSlot {
                slot: 0,
                element_type: DataType::U32,
                element_count: None,
                memory_class: MemoryClass::Global,
                visibility: BindingVisibility::ReadWrite,
                name: "slot".into(),
            }],
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
                    operands: vec![1],
                    result: Some(2),
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Eq),
                    operands: vec![1, 2],
                    result: Some(3),
                },
                KernelOp {
                    kind: KernelOpKind::Atomic {
                        op: AtomicOp::Or,
                        ordering: vyre_foundation::runtime::memory_model::MemoryOrdering::Relaxed,
                    },
                    operands: vec![0, 0, 3],
                    result: Some(4),
                },
            ],
            child_bodies: vec![],
            literals: vec![LiteralValue::U32(0), LiteralValue::U32(7)],
        },
    };
    let s = emit(&kernel).unwrap();
    let atom_line = s
        .lines()
        .find(|line| line.contains("atom.global.or.b32"))
        .expect("atomic OR must emit .b32");
    assert!(
        s.contains("selp.u32"),
        "bool atomic operand must be materialized as 0/1 before atom.global.or.b32:\n{s}"
    );
    assert!(
        !atom_line.contains("], %p"),
        "ptxas rejects predicate operands for atom.global.or.b32; got:\n{atom_line}\n{s}"
    );
}

#[test]
fn atomic_min_max_emit_correct_mnemonic() {
    use vyre_foundation::ir::AtomicOp;
    for (op, mnemonic) in [(AtomicOp::Min, "min"), (AtomicOp::Max, "max")] {
        let kernel = KernelDescriptor {
            id: "atomic_minmax".into(),
            bindings: BindingLayout {
                slots: vec![BindingSlot {
                    slot: 0,
                    element_type: DataType::U32,
                    element_count: None,
                    memory_class: MemoryClass::Global,
                    visibility: BindingVisibility::ReadWrite,
                    name: "b".into(),
                }],
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
                        kind: KernelOpKind::Atomic {
                            op,
                            ordering:
                                vyre_foundation::runtime::memory_model::MemoryOrdering::Relaxed,
                        },
                        operands: vec![0, 0, 1],
                        result: Some(2),
                    },
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(0), LiteralValue::U32(7)],
            },
        };
        let s = emit(&kernel).unwrap();
        assert!(s.contains(&format!("atom.global.{mnemonic}.u32")));
    }
}
