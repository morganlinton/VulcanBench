//! Hidden tests: shared_mem_promote must not barrier in non-uniform CF.
use vyre_foundation::ir::DataType;
use vyre_lower::rewrites::shared_mem_promote::shared_mem_promote;
use vyre_lower::{
    BindingLayout, BindingSlot, BindingVisibility, Dispatch, KernelBody, KernelDescriptor,
    KernelOp, KernelOpKind, LiteralValue, MemoryClass,
};

fn op(kind: KernelOpKind, operands: Vec<u32>, result: Option<u32>) -> KernelOp {
    KernelOp {
        kind,
        operands,
        result,
    }
}

fn binding(slot: u32, dtype: DataType, visibility: BindingVisibility) -> BindingSlot {
    BindingSlot {
        slot,
        element_type: dtype,
        element_count: None,
        memory_class: MemoryClass::Global,
        visibility,
        name: format!("b{slot}"),
    }
}

fn cooperative_ops(body: &KernelBody) -> Vec<KernelOpKind> {
    body.ops
        .iter()
        .filter(|op| {
            matches!(
                op.kind,
                KernelOpKind::Barrier { .. }
                    | KernelOpKind::AsyncLoad { .. }
                    | KernelOpKind::AsyncWait { .. }
            )
        })
        .map(|op| op.kind.clone())
        .collect()
}

#[test]
fn no_cooperative_ops_inside_structured_if_then() {
    let input = KernelDescriptor {
        id: "guarded".into(),
        bindings: BindingLayout {
            slots: vec![binding(0, DataType::U32, BindingVisibility::ReadOnly)],
        },
        dispatch: Dispatch::new(32, 1, 1),
        body: KernelBody {
            ops: vec![
                op(KernelOpKind::Literal, vec![0], Some(0)),
                op(KernelOpKind::StructuredIfThen, vec![0, 0], None),
            ],
            child_bodies: vec![KernelBody {
                ops: vec![
                    op(KernelOpKind::GlobalInvocationId, vec![0], Some(1)),
                    op(KernelOpKind::LoadGlobal, vec![0, 1], Some(2)),
                    op(KernelOpKind::LoadGlobal, vec![0, 1], Some(3)),
                ],
                child_bodies: vec![],
                literals: vec![],
            }],
            literals: vec![LiteralValue::U32(1)],
        },
    };
    let output = shared_mem_promote(&input);
    let conditional_body = &output.body.child_bodies[0];
    let illegal = cooperative_ops(conditional_body);
    assert!(
        illegal.is_empty(),
        "must not insert {illegal:?} into StructuredIfThen body (had {} ops)",
        conditional_body.ops.len()
    );
    assert_eq!(
        conditional_body.ops.len(),
        3,
        "conditional body should keep its three original ops"
    );
}

#[test]
fn no_cooperative_ops_inside_structured_for_loop_body() {
    let input = KernelDescriptor {
        id: "looped".into(),
        bindings: BindingLayout {
            slots: vec![binding(0, DataType::U32, BindingVisibility::ReadOnly)],
        },
        dispatch: Dispatch::new(32, 1, 1),
        body: KernelBody {
            ops: vec![
                op(KernelOpKind::Literal, vec![0], Some(0)), // from
                op(KernelOpKind::Literal, vec![1], Some(1)), // to
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
                    op(KernelOpKind::GlobalInvocationId, vec![0], Some(2)),
                    op(KernelOpKind::LoadGlobal, vec![0, 2], Some(3)),
                    op(KernelOpKind::LoadGlobal, vec![0, 2], Some(4)),
                ],
                child_bodies: vec![],
                literals: vec![],
            }],
            literals: vec![LiteralValue::U32(0), LiteralValue::U32(4)],
        },
    };
    let output = shared_mem_promote(&input);
    let loop_body = &output.body.child_bodies[0];
    let illegal = cooperative_ops(loop_body);
    assert!(
        illegal.is_empty(),
        "must not insert {illegal:?} into StructuredForLoop body"
    );
}

#[test]
fn still_promotes_repeated_loads_in_uniform_root_body() {
    let input = KernelDescriptor {
        id: "root".into(),
        bindings: BindingLayout {
            slots: vec![binding(0, DataType::U32, BindingVisibility::ReadOnly)],
        },
        dispatch: Dispatch::new(32, 1, 1),
        body: KernelBody {
            ops: vec![
                op(KernelOpKind::GlobalInvocationId, vec![0], Some(0)),
                op(KernelOpKind::LoadGlobal, vec![0, 0], Some(1)),
                op(KernelOpKind::LoadGlobal, vec![0, 0], Some(2)),
            ],
            child_bodies: vec![],
            literals: vec![],
        },
    };
    let output = shared_mem_promote(&input);
    assert!(
        output
            .body
            .ops
            .iter()
            .any(|op| matches!(op.kind, KernelOpKind::AsyncLoad { .. })),
        "uniform root body with repeated loads must still promote"
    );
    assert!(
        output
            .body
            .ops
            .iter()
            .any(|op| matches!(op.kind, KernelOpKind::Barrier { .. })),
        "uniform root promotion still inserts a workgroup barrier"
    );
}


#[test]
fn no_cooperative_ops_inside_structured_if_then_else() {
    let input = KernelDescriptor {
        id: "ite".into(),
        bindings: BindingLayout {
            slots: vec![binding(0, DataType::U32, BindingVisibility::ReadOnly)],
        },
        dispatch: Dispatch::new(32, 1, 1),
        body: KernelBody {
            ops: vec![
                op(KernelOpKind::Literal, vec![0], Some(0)),
                // then=child0, else=child1
                op(KernelOpKind::StructuredIfThenElse, vec![0, 0, 1], None),
            ],
            child_bodies: vec![
                KernelBody {
                    ops: vec![
                        op(KernelOpKind::GlobalInvocationId, vec![0], Some(1)),
                        op(KernelOpKind::LoadGlobal, vec![0, 1], Some(2)),
                        op(KernelOpKind::LoadGlobal, vec![0, 1], Some(3)),
                    ],
                    child_bodies: vec![],
                    literals: vec![],
                },
                KernelBody {
                    ops: vec![
                        op(KernelOpKind::GlobalInvocationId, vec![0], Some(4)),
                        op(KernelOpKind::LoadGlobal, vec![0, 4], Some(5)),
                        op(KernelOpKind::LoadGlobal, vec![0, 4], Some(6)),
                    ],
                    child_bodies: vec![],
                    literals: vec![],
                },
            ],
            literals: vec![LiteralValue::U32(1)],
        },
    };
    let output = shared_mem_promote(&input);
    for (i, child) in output.body.child_bodies.iter().enumerate() {
        let illegal = cooperative_ops(child);
        assert!(
            illegal.is_empty(),
            "arm {i} must not gain cooperative ops {illegal:?}"
        );
    }
}
