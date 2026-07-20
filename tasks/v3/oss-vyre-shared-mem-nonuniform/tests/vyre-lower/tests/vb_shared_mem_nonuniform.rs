//! Adversarial coverage: shared_mem_promote must never barrier in non-uniform CF.
//!
//! Complements the in-module IfThen regression with StructuredForLoop,
//! IfThenElse, nested conditionals, and a sibling-uniform root that must still
//! promote while a non-uniform child is left alone.

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

fn repeated_loads(gid_result: u32, load_a: u32, load_b: u32) -> Vec<KernelOp> {
    vec![
        op(KernelOpKind::GlobalInvocationId, vec![0], Some(gid_result)),
        op(KernelOpKind::LoadGlobal, vec![0, gid_result], Some(load_a)),
        op(KernelOpKind::LoadGlobal, vec![0, gid_result], Some(load_b)),
    ]
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
                op(KernelOpKind::Literal, vec![0], Some(0)),
                op(KernelOpKind::Literal, vec![1], Some(1)),
                KernelOp {
                    kind: KernelOpKind::StructuredForLoop {
                        loop_var: "i".into(),
                    },
                    operands: vec![0, 1, 0],
                    result: None,
                },
            ],
            child_bodies: vec![KernelBody {
                ops: repeated_loads(2, 3, 4),
                child_bodies: vec![],
                literals: vec![],
            }],
            literals: vec![LiteralValue::U32(0), LiteralValue::U32(4)],
        },
    };
    let output = shared_mem_promote(&input);
    let illegal = cooperative_ops(&output.body.child_bodies[0]);
    assert!(
        illegal.is_empty(),
        "must not insert {illegal:?} into StructuredForLoop body"
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
                op(KernelOpKind::StructuredIfThenElse, vec![0, 0, 1], None),
            ],
            child_bodies: vec![
                KernelBody {
                    ops: repeated_loads(1, 2, 3),
                    child_bodies: vec![],
                    literals: vec![],
                },
                KernelBody {
                    ops: repeated_loads(4, 5, 6),
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

#[test]
fn no_cooperative_ops_inside_nested_structured_if() {
    // Root: if (c0) { if (c1) { load; load } }
    let input = KernelDescriptor {
        id: "nested".into(),
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
                    op(KernelOpKind::Literal, vec![0], Some(1)),
                    op(KernelOpKind::StructuredIfThen, vec![1, 0], None),
                ],
                child_bodies: vec![KernelBody {
                    ops: repeated_loads(2, 3, 4),
                    child_bodies: vec![],
                    literals: vec![],
                }],
                literals: vec![LiteralValue::U32(1)],
            }],
            literals: vec![LiteralValue::U32(1)],
        },
    };
    let output = shared_mem_promote(&input);
    let outer = &output.body.child_bodies[0];
    assert!(
        cooperative_ops(outer).is_empty(),
        "outer if body must stay free of cooperative ops"
    );
    assert!(
        !outer.child_bodies.is_empty(),
        "nested structure must be preserved"
    );
    let inner = &outer.child_bodies[0];
    assert!(
        cooperative_ops(inner).is_empty(),
        "nested if body must not gain barriers/async ops"
    );
}

#[test]
fn still_promotes_uniform_root_when_sibling_child_is_nonuniform() {
    // Root has repeated loads (uniform) AND a StructuredIfThen child with its
    // own repeated loads. Root must still promote; the child must not.
    let input = KernelDescriptor {
        id: "sibling".into(),
        bindings: BindingLayout {
            slots: vec![binding(0, DataType::U32, BindingVisibility::ReadOnly)],
        },
        dispatch: Dispatch::new(32, 1, 1),
        body: KernelBody {
            ops: vec![
                op(KernelOpKind::GlobalInvocationId, vec![0], Some(0)),
                op(KernelOpKind::LoadGlobal, vec![0, 0], Some(1)),
                op(KernelOpKind::LoadGlobal, vec![0, 0], Some(2)),
                op(KernelOpKind::Literal, vec![0], Some(3)),
                op(KernelOpKind::StructuredIfThen, vec![3, 0], None),
            ],
            child_bodies: vec![KernelBody {
                ops: repeated_loads(4, 5, 6),
                child_bodies: vec![],
                literals: vec![],
            }],
            literals: vec![LiteralValue::U32(1)],
        },
    };
    let output = shared_mem_promote(&input);
    assert!(
        output
            .body
            .ops
            .iter()
            .any(|op| matches!(op.kind, KernelOpKind::AsyncLoad { .. })),
        "uniform root repeated loads must still promote"
    );
    let child_illegal = cooperative_ops(&output.body.child_bodies[0]);
    assert!(
        child_illegal.is_empty(),
        "non-uniform sibling child must not gain {child_illegal:?}"
    );
}

#[test]
fn still_promotes_inside_structured_block_child() {
    // StructuredBlock is unconditional grouping: child stays uniform and must promote.
    let input = KernelDescriptor {
        id: "block".into(),
        bindings: BindingLayout {
            slots: vec![binding(0, DataType::U32, BindingVisibility::ReadOnly)],
        },
        dispatch: Dispatch::new(32, 1, 1),
        body: KernelBody {
            ops: vec![op(KernelOpKind::StructuredBlock, vec![0], None)],
            child_bodies: vec![KernelBody {
                ops: repeated_loads(0, 1, 2),
                child_bodies: vec![],
                literals: vec![],
            }],
            literals: vec![],
        },
    };
    let output = shared_mem_promote(&input);
    let child = &output.body.child_bodies[0];
    assert!(
        child
            .ops
            .iter()
            .any(|op| matches!(op.kind, KernelOpKind::AsyncLoad { .. })),
        "StructuredBlock child is uniform and must still promote; ops={:?}",
        child.ops.iter().map(|o| &o.kind).collect::<Vec<_>>()
    );
    assert!(
        child
            .ops
            .iter()
            .any(|op| matches!(op.kind, KernelOpKind::Barrier { .. })),
        "uniform StructuredBlock promotion still inserts a barrier"
    );
}

#[test]
fn still_promotes_inside_region_child() {
    use std::sync::Arc;
    let input = KernelDescriptor {
        id: "region".into(),
        bindings: BindingLayout {
            slots: vec![binding(0, DataType::U32, BindingVisibility::ReadOnly)],
        },
        dispatch: Dispatch::new(32, 1, 1),
        body: KernelBody {
            ops: vec![KernelOp {
                kind: KernelOpKind::Region {
                    generator: Arc::from("test"),
                },
                operands: vec![0],
                result: None,
            }],
            child_bodies: vec![KernelBody {
                ops: repeated_loads(0, 1, 2),
                child_bodies: vec![],
                literals: vec![],
            }],
            literals: vec![],
        },
    };
    let output = shared_mem_promote(&input);
    let child = &output.body.child_bodies[0];
    assert!(
        child
            .ops
            .iter()
            .any(|op| matches!(op.kind, KernelOpKind::AsyncLoad { .. })),
        "Region child is uniform and must still promote"
    );
}

#[test]
fn no_cooperative_ops_in_structured_block_under_if() {
    // if (c) { StructuredBlock { load; load } } — the block does not restore uniformity.
    let input = KernelDescriptor {
        id: "block_under_if".into(),
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
                ops: vec![op(KernelOpKind::StructuredBlock, vec![0], None)],
                child_bodies: vec![KernelBody {
                    ops: repeated_loads(1, 2, 3),
                    child_bodies: vec![],
                    literals: vec![],
                }],
                literals: vec![],
            }],
            literals: vec![LiteralValue::U32(1)],
        },
    };
    let output = shared_mem_promote(&input);
    let if_body = &output.body.child_bodies[0];
    assert!(cooperative_ops(if_body).is_empty());
    assert!(
        !if_body.child_bodies.is_empty(),
        "nested StructuredBlock must remain"
    );
    assert!(
        cooperative_ops(&if_body.child_bodies[0]).is_empty(),
        "StructuredBlock under If is still non-uniform — no barriers"
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
    assert!(output.body.ops.iter().any(|op| matches!(op.kind, KernelOpKind::AsyncLoad { .. })));
    assert!(output.body.ops.iter().any(|op| matches!(op.kind, KernelOpKind::Barrier { .. })));
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
                ops: repeated_loads(1, 2, 3),
                child_bodies: vec![],
                literals: vec![],
            }],
            literals: vec![LiteralValue::U32(1)],
        },
    };
    let output = shared_mem_promote(&input);
    assert!(cooperative_ops(&output.body.child_bodies[0]).is_empty());
}
