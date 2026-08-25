//! Test: control flow.
use super::*;

#[test]
fn region_op_passes_through_with_comment() {
    let kernel = KernelDescriptor {
        id: "region".into(),
        bindings: BindingLayout { slots: vec![] },
        dispatch: Dispatch::new(1, 1, 1),
        body: KernelBody {
            ops: vec![KernelOp {
                kind: KernelOpKind::Region {
                    generator: "vyre.libs.test".into(),
                },
                operands: vec![0],
                result: None,
            }],
            child_bodies: vec![KernelBody {
                ops: vec![],
                child_bodies: vec![],
                literals: vec![],
            }],
            literals: vec![],
        },
    };
    let s = emit(&kernel).unwrap();
    assert!(s.contains("// region: vyre.libs.test"));
}

// ============== Structured control flow + composite ops (parity push) ==============

#[test]
fn structured_if_then_emits_branch_and_label() {
    let kernel = KernelDescriptor {
        id: "if_then".into(),
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
                    kind: KernelOpKind::StructuredIfThen,
                    operands: vec![0, 0],
                    result: None,
                },
            ],
            child_bodies: vec![empty_child_body()],
            literals: vec![LiteralValue::Bool(true)],
        },
    };
    let s = emit(&kernel).unwrap();
    assert!(s.contains("@!"), "must emit a negated-pred branch");
    assert!(s.contains("bra "));
    assert!(s.contains("$L_if_end_"), "must emit an if_end label");
}

#[test]
fn structured_if_then_else_emits_else_label_and_unconditional_jump() {
    let kernel = KernelDescriptor {
        id: "if_else".into(),
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
                    kind: KernelOpKind::StructuredIfThenElse,
                    operands: vec![0, 0, 1],
                    result: None,
                },
            ],
            child_bodies: vec![empty_child_body(), empty_child_body()],
            literals: vec![LiteralValue::Bool(false)],
        },
    };
    let s = emit(&kernel).unwrap();
    assert!(s.contains("$L_if_else_"), "must emit an else label");
    assert!(s.contains("$L_if_end_"));
    // Jump from end of then-body to end label.
    assert!(s.matches("bra ").count() >= 2, "if-else needs ≥ 2 bra ops");
}

#[test]
fn short_if_then_store_is_predicated_without_branch() {
    let kernel = KernelDescriptor {
        id: "if_store".into(),
        bindings: BindingLayout {
            slots: vec![BindingSlot {
                slot: 0,
                element_type: DataType::U32,
                element_count: Some(1),
                memory_class: MemoryClass::Global,
                visibility: BindingVisibility::ReadWrite,
                name: "out".into(),
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
                    operands: vec![2],
                    result: Some(2),
                },
                KernelOp {
                    kind: KernelOpKind::StructuredIfThen,
                    operands: vec![0, 0],
                    result: None,
                },
            ],
            child_bodies: vec![KernelBody {
                ops: vec![KernelOp {
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![0, 1, 2],
                    result: None,
                }],
                child_bodies: vec![],
                literals: vec![],
            }],
            literals: vec![
                LiteralValue::Bool(true),
                LiteralValue::U32(0),
                LiteralValue::U32(7),
            ],
        },
    };

    let s = emit(&kernel).unwrap();

    assert!(s.contains("@%p"), "store must be guarded by the condition");
    assert!(s.contains(" st.global.u32"));
    assert!(
        !s.contains("$L_if_end_"),
        "single-store if must avoid branch/label divergence"
    );
}

#[test]
fn short_if_then_literal_store_body_is_predicated_without_branch() {
    let kernel = KernelDescriptor {
        id: "if_literal_store".into(),
        bindings: BindingLayout {
            slots: vec![BindingSlot {
                slot: 0,
                element_type: DataType::U32,
                element_count: Some(1),
                memory_class: MemoryClass::Global,
                visibility: BindingVisibility::ReadWrite,
                name: "out".into(),
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
                    kind: KernelOpKind::StructuredIfThen,
                    operands: vec![0, 0],
                    result: None,
                },
            ],
            child_bodies: vec![KernelBody {
                ops: vec![
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![0],
                        result: Some(20),
                    },
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 1, 20],
                        result: None,
                    },
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(13)],
            }],
            literals: vec![LiteralValue::Bool(true), LiteralValue::U32(0)],
        },
    };

    let s = emit(&kernel).unwrap();

    assert!(s.contains("@%p"));
    assert!(s.contains(" st.global.u32"));
    assert!(
        !s.contains("$L_if_end_"),
        "short pure-prefix store body must use predication instead of branch/reconvergence"
    );
}

#[test]
fn short_if_then_two_store_body_is_fully_predicated_without_branch() {
    let kernel = KernelDescriptor {
        id: "if_two_stores".into(),
        bindings: BindingLayout {
            slots: vec![BindingSlot {
                slot: 0,
                element_type: DataType::U32,
                element_count: Some(2),
                memory_class: MemoryClass::Global,
                visibility: BindingVisibility::ReadWrite,
                name: "out".into(),
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
                    kind: KernelOpKind::StructuredIfThen,
                    operands: vec![0, 0],
                    result: None,
                },
            ],
            child_bodies: vec![KernelBody {
                ops: vec![
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![0],
                        result: Some(20),
                    },
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 1, 20],
                        result: None,
                    },
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![1],
                        result: Some(21),
                    },
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 1, 21],
                        result: None,
                    },
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(13), LiteralValue::U32(17)],
            }],
            literals: vec![LiteralValue::Bool(true), LiteralValue::U32(0)],
        },
    };

    let s = emit(&kernel).unwrap();

    assert!(s.contains("@%p"));
    assert_eq!(s.matches(" st.global.u32").count(), 2);
    assert!(
        !s.contains("$L_if_end_"),
        "short multi-store body must use predication instead of branch/reconvergence"
    );
}

#[test]
fn short_if_else_stores_are_dual_predicated_without_reconvergence_branch() {
    let kernel = KernelDescriptor {
        id: "if_else_store".into(),
        bindings: BindingLayout {
            slots: vec![BindingSlot {
                slot: 0,
                element_type: DataType::U32,
                element_count: Some(1),
                memory_class: MemoryClass::Global,
                visibility: BindingVisibility::ReadWrite,
                name: "out".into(),
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
                    operands: vec![2],
                    result: Some(2),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![3],
                    result: Some(3),
                },
                KernelOp {
                    kind: KernelOpKind::StructuredIfThenElse,
                    operands: vec![0, 0, 1],
                    result: None,
                },
            ],
            child_bodies: vec![
                KernelBody {
                    ops: vec![KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 1, 2],
                        result: None,
                    }],
                    child_bodies: vec![],
                    literals: vec![],
                },
                KernelBody {
                    ops: vec![KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 1, 3],
                        result: None,
                    }],
                    child_bodies: vec![],
                    literals: vec![],
                },
            ],
            literals: vec![
                LiteralValue::Bool(true),
                LiteralValue::U32(0),
                LiteralValue::U32(7),
                LiteralValue::U32(9),
            ],
        },
    };

    let s = emit(&kernel).unwrap();

    assert!(s.contains("@%p"));
    assert!(s.contains("@!%p"));
    assert_eq!(s.matches(" st.global.u32").count(), 2);
    assert!(
        !s.contains("$L_if_else_") && !s.contains("$L_if_end_"),
        "dual predicated store arms must avoid SIMT branch and reconvergence labels"
    );
}

#[test]
fn short_if_else_literal_store_bodies_are_dual_predicated_without_branch() {
    let kernel = KernelDescriptor {
        id: "if_else_literal_store".into(),
        bindings: BindingLayout {
            slots: vec![BindingSlot {
                slot: 0,
                element_type: DataType::U32,
                element_count: Some(1),
                memory_class: MemoryClass::Global,
                visibility: BindingVisibility::ReadWrite,
                name: "out".into(),
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
                    kind: KernelOpKind::StructuredIfThenElse,
                    operands: vec![0, 0, 1],
                    result: None,
                },
            ],
            child_bodies: vec![
                KernelBody {
                    ops: vec![
                        KernelOp {
                            kind: KernelOpKind::Literal,
                            operands: vec![0],
                            result: Some(20),
                        },
                        KernelOp {
                            kind: KernelOpKind::StoreGlobal,
                            operands: vec![0, 1, 20],
                            result: None,
                        },
                    ],
                    child_bodies: vec![],
                    literals: vec![LiteralValue::U32(21)],
                },
                KernelBody {
                    ops: vec![
                        KernelOp {
                            kind: KernelOpKind::Literal,
                            operands: vec![0],
                            result: Some(21),
                        },
                        KernelOp {
                            kind: KernelOpKind::StoreGlobal,
                            operands: vec![0, 1, 21],
                            result: None,
                        },
                    ],
                    child_bodies: vec![],
                    literals: vec![LiteralValue::U32(34)],
                },
            ],
            literals: vec![LiteralValue::Bool(true), LiteralValue::U32(0)],
        },
    };

    let s = emit(&kernel).unwrap();

    assert!(s.contains("@%p"));
    assert!(s.contains("@!%p"));
    assert_eq!(s.matches(" st.global.u32").count(), 2);
    assert!(
        !s.contains("$L_if_else_") && !s.contains("$L_if_end_"),
        "short pure-prefix store arms must avoid SIMT branch and reconvergence labels"
    );
}

#[test]
fn structured_for_loop_emits_head_label_setp_and_jump_back() {
    let kernel = KernelDescriptor {
        id: "for_loop".into(),
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
            child_bodies: vec![empty_child_body()],
            literals: vec![LiteralValue::U32(0), LiteralValue::U32(64)],
        },
    };
    let s = emit(&kernel).unwrap();
    assert!(s.contains("$L_for_head_"), "must emit head label");
    assert!(s.contains("$L_for_exit_"), "must emit exit label");
    assert!(s.contains("setp.ge.u32"), "must emit loop-bound predicate");
    assert!(s.contains("// for i in"));
}
