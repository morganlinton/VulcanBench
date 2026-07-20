//! PERF B8: predicated execution detection for short divergent branches.
//!
//! PTX supports per-instruction predicates: `@%p add.u32 %r1, %r2, %r3`
//! executes the add only when predicate `%p` is true. For short
//! divergent branches (1-3 instructions in each arm), predicated
//! execution avoids the SIMT divergence cost: all threads execute
//! all instructions, but writes are masked by the predicate.
//!
//! The win: no warp divergence, no scoreboard stall, no per-arm
//! reconvergence overhead. The loss: every thread runs both arms.
//! Profitable when the arms are short (≤ 4 instructions each).
//!
//! Phase-1 detection: walk every `StructuredIfThen` /
//! `StructuredIfThenElse` op; for each, count ops in the then/else
//! body; if both bodies are ≤ 4 ops AND contain no non-predicatable
//! side effects, flag as a predicated-execution candidate. Ordinary
//! global/shared stores are predicatable on PTX and are handled by the
//! emitter, so treating every store as unsafe would suppress the fast
//! path for the exact branch shape this pass is meant to find.

use serde::{Deserialize, Serialize};
use vyre_lower::{KernelBody, KernelDescriptor, KernelOpKind};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PredicationCandidate {
    /// Op-index of the StructuredIfThen / StructuredIfThenElse.
    pub if_op_index: usize,
    pub then_body_op_count: u32,
    pub else_body_op_count: u32,
    /// Whether either body contains global stores. Kept as telemetry
    /// because store-heavy rule kernels are the main predication target;
    /// this does not imply unsafety on PTX.
    pub has_global_store: bool,
    /// Whether either body contains an effect that cannot be safely
    /// guarded with a PTX instruction predicate.
    #[serde(default)]
    pub has_unsafe_effect: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PredicationPlan {
    pub kernel_id: String,
    pub candidates: Vec<PredicationCandidate>,
}

impl PredicationPlan {
    #[must_use]
    pub fn safe_candidate_count(&self) -> usize {
        self.candidates
            .iter()
            .filter(|c| !c.has_unsafe_effect)
            .count()
    }
}

/// Maximum ops in either arm for predication to be profitable.
pub const PREDICATION_OP_THRESHOLD: u32 = 4;

#[must_use]
pub fn analyze(desc: &KernelDescriptor) -> PredicationPlan {
    let mut candidates = Vec::new();
    scan_body(&desc.body, &mut candidates, 0);
    PredicationPlan {
        kernel_id: desc.id.clone(),
        candidates,
    }
}

fn scan_body(
    body: &KernelBody,
    candidates: &mut Vec<PredicationCandidate>,
    op_index_offset: usize,
) {
    for (local_idx, op) in body.ops.iter().enumerate() {
        let op_index = op_index_offset + local_idx;
        match &op.kind {
            KernelOpKind::StructuredIfThen => {
                let Some(then_id) = op.operands.get(1).copied() else {
                    continue;
                };
                let Some(then) = body.child_bodies.get(then_id as usize) else {
                    continue;
                };
                let then_count = then.ops.len() as u32;
                let then_has_store = has_global_store(then);
                let then_has_unsafe_effect = has_unsafe_predicated_effect(then);
                if then_count <= PREDICATION_OP_THRESHOLD {
                    candidates.push(PredicationCandidate {
                        if_op_index: op_index,
                        then_body_op_count: then_count,
                        else_body_op_count: 0,
                        has_global_store: then_has_store,
                        has_unsafe_effect: then_has_unsafe_effect,
                    });
                }
            }
            KernelOpKind::StructuredIfThenElse => {
                let (Some(then_id), Some(else_id)) =
                    (op.operands.get(1).copied(), op.operands.get(2).copied())
                else {
                    continue;
                };
                let (Some(then), Some(else_b)) = (
                    body.child_bodies.get(then_id as usize),
                    body.child_bodies.get(else_id as usize),
                ) else {
                    continue;
                };
                let then_count = then.ops.len() as u32;
                let else_count = else_b.ops.len() as u32;
                let has_store = has_global_store(then) || has_global_store(else_b);
                let has_unsafe_effect =
                    has_unsafe_predicated_effect(then) || has_unsafe_predicated_effect(else_b);
                if then_count <= PREDICATION_OP_THRESHOLD && else_count <= PREDICATION_OP_THRESHOLD
                {
                    candidates.push(PredicationCandidate {
                        if_op_index: op_index,
                        then_body_op_count: then_count,
                        else_body_op_count: else_count,
                        has_global_store: has_store,
                        has_unsafe_effect,
                    });
                }
            }
            KernelOpKind::StructuredForLoop { .. }
            | KernelOpKind::StructuredBlock
            | KernelOpKind::Region { .. } => {
                if let Some(child_id) = op.operands.last() {
                    if let Some(child) = body.child_bodies.get(*child_id as usize) {
                        scan_body(child, candidates, op_index_offset + body.ops.len());
                    }
                }
            }
            _ => {}
        }
    }
}

fn has_global_store(body: &KernelBody) -> bool {
    body.ops
        .iter()
        .any(|op| matches!(op.kind, KernelOpKind::StoreGlobal))
}

fn has_unsafe_predicated_effect(body: &KernelBody) -> bool {
    body.ops.iter().any(|op| {
        matches!(
            op.kind,
            KernelOpKind::Atomic { .. }
                | KernelOpKind::Barrier { .. }
                | KernelOpKind::AsyncLoad { .. }
                | KernelOpKind::AsyncStore { .. }
                | KernelOpKind::AsyncWait { .. }
                | KernelOpKind::Trap { .. }
                | KernelOpKind::Resume { .. }
                | KernelOpKind::IndirectDispatch { .. }
                | KernelOpKind::Call { .. }
                | KernelOpKind::OpaqueExpr(..)
                | KernelOpKind::OpaqueNode(..)
                | KernelOpKind::StructuredIfThen
                | KernelOpKind::StructuredIfThenElse
                | KernelOpKind::StructuredForLoop { .. }
                | KernelOpKind::StructuredBlock
                | KernelOpKind::Region { .. }
                | KernelOpKind::Return
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre_lower::{
        BindingLayout, Dispatch, KernelBody, KernelDescriptor, KernelOp, LiteralValue,
    };

    fn make_if(then_op_count: u32) -> KernelDescriptor {
        let mut then_ops = Vec::new();
        for i in 0..then_op_count {
            then_ops.push(KernelOp {
                kind: KernelOpKind::Literal,
                operands: vec![0],
                result: Some(i + 100),
            });
        }
        KernelDescriptor {
            id: "if_kernel".into(),
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
                child_bodies: vec![KernelBody {
                    ops: then_ops,
                    child_bodies: vec![],
                    literals: vec![],
                }],
                literals: vec![LiteralValue::Bool(true)],
            },
        }
    }

    #[test]
    fn empty_kernel_has_no_candidates() {
        let desc = KernelDescriptor {
            id: "empty".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(64, 1, 1),
            body: KernelBody {
                ops: vec![],
                child_bodies: vec![],
                literals: vec![],
            },
        };
        let p = analyze(&desc);
        assert!(p.candidates.is_empty());
    }

    #[test]
    fn small_if_then_is_predication_candidate() {
        let desc = make_if(2);
        let p = analyze(&desc);
        assert_eq!(p.candidates.len(), 1);
        assert_eq!(p.candidates[0].then_body_op_count, 2);
        assert_eq!(p.candidates[0].else_body_op_count, 0);
        assert!(!p.candidates[0].has_global_store);
        assert_eq!(p.safe_candidate_count(), 1);
    }

    #[test]
    fn large_if_then_above_threshold_no_candidate() {
        let desc = make_if(10);
        let p = analyze(&desc);
        assert!(
            p.candidates.is_empty(),
            "10 ops > {PREDICATION_OP_THRESHOLD} threshold"
        );
    }

    #[test]
    fn boundary_case_at_threshold_qualifies() {
        let desc = make_if(PREDICATION_OP_THRESHOLD);
        let p = analyze(&desc);
        assert_eq!(p.candidates.len(), 1);
    }

    #[test]
    fn if_with_global_store_remains_safe_candidate() {
        let desc = KernelDescriptor {
            id: "store_in_if".into(),
            bindings: BindingLayout {
                slots: vec![vyre_lower::BindingSlot {
                    slot: 0,
                    element_type: vyre_foundation::ir::DataType::U32,
                    element_count: None,
                    memory_class: vyre_lower::MemoryClass::Global,
                    visibility: vyre_lower::BindingVisibility::ReadWrite,
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
                    ops: vec![KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 0, 1],
                        result: None,
                    }],
                    child_bodies: vec![],
                    literals: vec![],
                }],
                literals: vec![LiteralValue::Bool(true), LiteralValue::U32(7)],
            },
        };
        let p = analyze(&desc);
        assert_eq!(p.candidates.len(), 1);
        assert!(p.candidates[0].has_global_store);
        assert!(!p.candidates[0].has_unsafe_effect);
        assert_eq!(p.safe_candidate_count(), 1);
    }

    #[test]
    fn if_with_atomic_flagged_unsafe() {
        let desc = KernelDescriptor {
            id: "atomic_in_if".into(),
            bindings: BindingLayout {
                slots: vec![vyre_lower::BindingSlot {
                    slot: 0,
                    element_type: vyre_foundation::ir::DataType::U32,
                    element_count: None,
                    memory_class: vyre_lower::MemoryClass::Global,
                    visibility: vyre_lower::BindingVisibility::ReadWrite,
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
                    ops: vec![KernelOp {
                        kind: KernelOpKind::Atomic {
                            op: vyre_foundation::ir::AtomicOp::Add,
                            ordering:
                                vyre_foundation::runtime::memory_model::MemoryOrdering::SeqCst,
                        },
                        operands: vec![0, 0, 1],
                        result: Some(2),
                    }],
                    child_bodies: vec![],
                    literals: vec![],
                }],
                literals: vec![LiteralValue::Bool(true), LiteralValue::U32(7)],
            },
        };
        let p = analyze(&desc);
        assert_eq!(p.candidates.len(), 1);
        assert!(p.candidates[0].has_unsafe_effect);
        assert_eq!(p.safe_candidate_count(), 0);
    }

    #[test]
    fn if_else_both_small_qualifies() {
        let desc = KernelDescriptor {
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
                child_bodies: vec![
                    KernelBody {
                        ops: vec![KernelOp {
                            kind: KernelOpKind::Literal,
                            operands: vec![0],
                            result: Some(10),
                        }],
                        child_bodies: vec![],
                        literals: vec![],
                    },
                    KernelBody {
                        ops: vec![
                            KernelOp {
                                kind: KernelOpKind::Literal,
                                operands: vec![0],
                                result: Some(20),
                            },
                            KernelOp {
                                kind: KernelOpKind::Literal,
                                operands: vec![0],
                                result: Some(21),
                            },
                        ],
                        child_bodies: vec![],
                        literals: vec![],
                    },
                ],
                literals: vec![LiteralValue::Bool(true)],
            },
        };
        let p = analyze(&desc);
        assert_eq!(p.candidates.len(), 1);
        assert_eq!(p.candidates[0].then_body_op_count, 1);
        assert_eq!(p.candidates[0].else_body_op_count, 2);
    }

    #[test]
    fn if_else_either_too_large_no_candidate() {
        let mut else_ops = Vec::new();
        for i in 0..10 {
            else_ops.push(KernelOp {
                kind: KernelOpKind::Literal,
                operands: vec![0],
                result: Some(i + 200),
            });
        }
        let desc = KernelDescriptor {
            id: "if_else_big".into(),
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
                child_bodies: vec![
                    KernelBody {
                        ops: vec![],
                        child_bodies: vec![],
                        literals: vec![],
                    },
                    KernelBody {
                        ops: else_ops,
                        child_bodies: vec![],
                        literals: vec![],
                    },
                ],
                literals: vec![LiteralValue::Bool(true)],
            },
        };
        let p = analyze(&desc);
        assert!(p.candidates.is_empty(), "10-op else arm exceeds threshold");
    }

    #[test]
    fn threshold_constant_is_documented_value() {
        assert_eq!(PREDICATION_OP_THRESHOLD, 4);
    }

    #[test]
    fn malformed_if_without_child_operand_no_candidate() {
        let desc = KernelDescriptor {
            id: "malformed_if".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(64, 1, 1),
            body: KernelBody {
                ops: vec![KernelOp {
                    kind: KernelOpKind::StructuredIfThen,
                    operands: vec![0],
                    result: None,
                }],
                child_bodies: vec![KernelBody {
                    ops: vec![KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![0],
                        result: Some(1),
                    }],
                    child_bodies: vec![],
                    literals: vec![],
                }],
                literals: vec![LiteralValue::Bool(true)],
            },
        };
        let p = analyze(&desc);
        assert!(p.candidates.is_empty());
    }
}
