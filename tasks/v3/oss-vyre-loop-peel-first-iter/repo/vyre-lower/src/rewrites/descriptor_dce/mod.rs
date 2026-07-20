//! Dead-op elimination rewrite.
//!
//! Strips result-producing pure ops whose result is not referenced
//! anywhere in the descriptor tree. Result ids are **not** renumbered  -  they are
//! left as-is so that cross-body references (e.g. a child body that
//! reads a value produced in a parent body) remain valid. The verifier
//! does not require dense ids.

use crate::op_properties::kernel_op_kind_is_dce_pure as is_pure;
use crate::operand_semantics::operand_is_result_reference;
use crate::{KernelBody, KernelDescriptor};
use rustc_hash::FxHashSet;

#[must_use]
pub fn descriptor_dce(desc: &KernelDescriptor) -> KernelDescriptor {
    let mut out = desc.clone();
    let live_results = collect_live_results(&out.body);
    out.body = dce_body(out.body, &live_results);
    out
}

fn dce_body(mut body: KernelBody, live_results: &FxHashSet<u32>) -> KernelBody {
    let mut surviving_ops = Vec::with_capacity(body.ops.len());
    let old_ops = std::mem::take(&mut body.ops);
    for op in old_ops {
        let dead = op.result.is_some()
            && op
                .result_ids()
                .all(|result| !live_results.contains(&result))
            && is_pure(&op.kind);
        if !dead {
            surviving_ops.push(op);
        }
    }

    body.child_bodies = body
        .child_bodies
        .into_iter()
        .map(|child| dce_body(child, live_results))
        .collect();

    body.ops = surviving_ops;
    body
}

fn collect_live_results(body: &KernelBody) -> FxHashSet<u32> {
    let mut live_results =
        FxHashSet::with_capacity_and_hasher(body.ops.len().saturating_mul(2), Default::default());
    while propagate_live_operands(body, &mut live_results) {}
    live_results
}

fn propagate_live_operands(body: &KernelBody, live_results: &mut FxHashSet<u32>) -> bool {
    let mut changed = false;
    for child in body.child_bodies.iter().rev() {
        changed |= propagate_live_operands(child, live_results);
    }
    for op in body.ops.iter().rev() {
        if op_is_live_root_or_reachable(op, live_results) {
            for (pos, operand) in op.operands.iter().enumerate() {
                if operand_is_result_reference(&op.kind, pos) {
                    changed |= live_results.insert(*operand);
                }
            }
        }
    }
    changed
}

fn op_is_live_root_or_reachable(op: &crate::KernelOp, live_results: &FxHashSet<u32>) -> bool {
    !is_pure(&op.kind) || op.result_ids().any(|result| live_results.contains(&result))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BindingLayout, BindingSlot, BindingVisibility, Dispatch, KernelBody, KernelDescriptor,
        KernelOp, KernelOpKind, LiteralValue, MemoryClass,
    };
    use vyre_foundation::ir::{BinOp, DataType};

    fn store_kernel_with_dead_literal() -> KernelDescriptor {
        // Same shape as the dead-op test that exposed the bug.
        KernelDescriptor {
            id: "store_with_dead".into(),
            bindings: BindingLayout {
                slots: vec![BindingSlot {
                    slot: 0,
                    element_type: DataType::U32,
                    element_count: None,
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
                        kind: KernelOpKind::Literal,
                        operands: vec![2],
                        result: Some(2),
                    }, // dead
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 0, 1],
                        result: None,
                    },
                ],
                child_bodies: vec![],
                literals: vec![
                    LiteralValue::U32(0),
                    LiteralValue::U32(7),
                    LiteralValue::U32(99),
                ],
            },
        }
    }

    #[test]
    fn dce_removes_dead_literal() {
        let desc = store_kernel_with_dead_literal();
        let out = descriptor_dce(&desc);
        // Started with 4 ops, removes 1, ends with 3.
        assert_eq!(out.body.ops.len(), 3);
        // Surviving ops: Literal(0), Literal(1), StoreGlobal.
        // Result ids are left unchanged so cross-body refs stay valid.
        assert_eq!(out.body.ops[0].result, Some(0));
        assert_eq!(out.body.ops[1].result, Some(1));
        assert_eq!(out.body.ops[2].result, None);
        // StoreGlobal operands: slot=0 (unchanged, not a result-ref),
        // index_op_id (was 0, still 0), value_op_id (was 1, still 1).
        assert_eq!(out.body.ops[2].operands, vec![0, 0, 1]);
    }

    #[test]
    fn dce_on_empty_kernel_is_noop() {
        let desc = KernelDescriptor {
            id: "k".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops: vec![],
                child_bodies: vec![],
                literals: vec![],
            },
        };
        let out = descriptor_dce(&desc);
        assert!(out.body.ops.is_empty());
    }

    #[test]
    fn dce_is_idempotent() {
        let desc = store_kernel_with_dead_literal();
        let once = descriptor_dce(&desc);
        let twice = descriptor_dce(&once);
        assert_eq!(once.body.ops.len(), twice.body.ops.len());
        for (a, b) in once.body.ops.iter().zip(twice.body.ops.iter()) {
            assert_eq!(a.kind, b.kind);
            assert_eq!(a.result, b.result);
            assert_eq!(a.operands, b.operands);
        }
    }

    #[test]
    fn dce_preserves_arithmetic_when_used() {
        // tid; lit; add; store(out, 0, add). The Add is used, so DCE
        // shouldn't touch it.
        let desc = KernelDescriptor {
            id: "live_chain".into(),
            bindings: BindingLayout {
                slots: vec![BindingSlot {
                    slot: 0,
                    element_type: DataType::U32,
                    element_count: None,
                    memory_class: MemoryClass::Global,
                    visibility: BindingVisibility::WriteOnly,
                    name: "out".into(),
                }],
            },
            dispatch: Dispatch::new(64, 1, 1),
            body: KernelBody {
                ops: vec![
                    KernelOp {
                        kind: KernelOpKind::LocalInvocationId,
                        operands: vec![0],
                        result: Some(0),
                    },
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![0],
                        result: Some(1),
                    },
                    KernelOp {
                        kind: KernelOpKind::BinOpKind(BinOp::Add),
                        operands: vec![0, 1],
                        result: Some(2),
                    },
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![1],
                        result: Some(3),
                    },
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 3, 2],
                        result: None,
                    },
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(5), LiteralValue::U32(0)],
            },
        };
        let out = descriptor_dce(&desc);
        assert_eq!(out.body.ops.len(), 5, "every op is live in this kernel");
        // Result-ids preserved as-is when nothing's removed.
        assert_eq!(out.body.ops[0].result, Some(0));
        assert_eq!(out.body.ops[2].result, Some(2));
    }

    #[test]
    fn dce_removes_chain_of_dead_arithmetic() {
        // tid → r0; lit → r1; add(r0, r1) → r2; nothing reads r2.
        // Whole-chain DCE removes the Add and the pure producers that only fed it.
        let desc = KernelDescriptor {
            id: "dead_chain".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(64, 1, 1),
            body: KernelBody {
                ops: vec![
                    KernelOp {
                        kind: KernelOpKind::LocalInvocationId,
                        operands: vec![0],
                        result: Some(0),
                    },
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![0],
                        result: Some(1),
                    },
                    KernelOp {
                        kind: KernelOpKind::BinOpKind(BinOp::Add),
                        operands: vec![0, 1],
                        result: Some(2),
                    },
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(5)],
            },
        };
        let out = descriptor_dce(&desc);
        assert_eq!(out.body.ops.len(), 0, "entire pure dead chain removed");
    }

    #[test]
    fn dce_removes_multi_hop_dead_chain() {
        let desc = KernelDescriptor {
            id: "multi_dead_chain".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(64, 1, 1),
            body: KernelBody {
                ops: vec![
                    KernelOp {
                        kind: KernelOpKind::LocalInvocationId,
                        operands: vec![0],
                        result: Some(0),
                    },
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![0],
                        result: Some(1),
                    },
                    KernelOp {
                        kind: KernelOpKind::BinOpKind(BinOp::Add),
                        operands: vec![0, 1],
                        result: Some(2),
                    },
                    KernelOp {
                        kind: KernelOpKind::BinOpKind(BinOp::Mul),
                        operands: vec![2, 1],
                        result: Some(3),
                    },
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(5)],
            },
        };
        let out = descriptor_dce(&desc);
        assert!(out.body.ops.is_empty());
    }

    #[test]
    fn dce_leaves_ids_unchanged_after_removal() {
        // Three literals: r0 dead, r1 used, r2 dead, r3 used.
        // After DCE: r1 and r3 survive with their original ids so that
        // any cross-body references remain valid.
        use crate::{BindingSlot, BindingVisibility, MemoryClass};
        let desc = KernelDescriptor {
            id: "sparse".into(),
            bindings: BindingLayout {
                slots: vec![BindingSlot {
                    slot: 0,
                    element_type: DataType::U32,
                    element_count: None,
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
                    }, // dead
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![1],
                        result: Some(1),
                    }, // used as index
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![2],
                        result: Some(2),
                    }, // dead
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![3],
                        result: Some(3),
                    }, // used as value
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 1, 3],
                        result: None,
                    },
                ],
                child_bodies: vec![],
                literals: vec![
                    LiteralValue::U32(99),
                    LiteralValue::U32(0),
                    LiteralValue::U32(88),
                    LiteralValue::U32(7),
                ],
            },
        };
        let out = descriptor_dce(&desc);
        // 5 ops, 2 dead, 3 surviving.
        assert_eq!(out.body.ops.len(), 3);
        // Original ids preserved.
        assert_eq!(out.body.ops[0].result, Some(1));
        assert_eq!(out.body.ops[1].result, Some(3));
        assert_eq!(out.body.ops[2].result, None);
        // Store still references the original ids.
        assert_eq!(out.body.ops[2].operands, vec![0, 1, 3]);
    }

    #[test]
    fn dce_keeps_parent_results_used_by_child_body() {
        let child = KernelBody {
            ops: vec![KernelOp {
                kind: KernelOpKind::StoreGlobal,
                operands: vec![0, 9, 1],
                result: None,
            }],
            child_bodies: vec![],
            literals: vec![],
        };
        let mut ops = Vec::new();
        for id in 0..10 {
            ops.push(KernelOp {
                kind: KernelOpKind::Literal,
                operands: vec![id],
                result: Some(id),
            });
        }
        ops.push(KernelOp {
            kind: KernelOpKind::StructuredBlock,
            operands: vec![0],
            result: None,
        });

        let desc = KernelDescriptor {
            id: "child_capture".into(),
            bindings: BindingLayout {
                slots: vec![BindingSlot {
                    slot: 0,
                    element_type: DataType::U32,
                    element_count: None,
                    memory_class: MemoryClass::Global,
                    visibility: BindingVisibility::WriteOnly,
                    name: "out".into(),
                }],
            },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops,
                child_bodies: vec![child],
                literals: (0..10).map(LiteralValue::U32).collect(),
            },
        };

        assert_eq!(crate::verify::verify(&desc), Ok(()));
        let out = descriptor_dce(&desc);
        assert_eq!(crate::verify::verify(&out), Ok(()));
        assert!(
            out.body
                .ops
                .iter()
                .any(|op| op.result == Some(9) && matches!(op.kind, KernelOpKind::Literal)),
            "parent result 9 is read from the child body and must survive"
        );
    }

    #[test]
    fn dce_keeps_child_results_used_by_parent_after_block() {
        let child = KernelBody {
            ops: vec![KernelOp {
                kind: KernelOpKind::BinOpKind(BinOp::Add),
                operands: vec![0, 0],
                result: Some(1),
            }],
            child_bodies: vec![],
            literals: vec![],
        };
        let desc = KernelDescriptor {
            id: "parent_reads_child".into(),
            bindings: BindingLayout {
                slots: vec![BindingSlot {
                    slot: 0,
                    element_type: DataType::U32,
                    element_count: None,
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
                        kind: KernelOpKind::StructuredBlock,
                        operands: vec![0],
                        result: None,
                    },
                    KernelOp {
                        kind: KernelOpKind::BinOpKind(BinOp::Mul),
                        operands: vec![1, 0],
                        result: Some(2),
                    },
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 0, 2],
                        result: None,
                    },
                ],
                child_bodies: vec![child],
                literals: vec![LiteralValue::U32(7)],
            },
        };

        assert_eq!(crate::verify::verify(&desc), Ok(()));
        let out = descriptor_dce(&desc);
        assert_eq!(crate::verify::verify(&out), Ok(()));
        assert_eq!(out.body.child_bodies[0].ops.len(), 1);
        assert_eq!(out.body.child_bodies[0].ops[0].result, Some(1));
    }
}
