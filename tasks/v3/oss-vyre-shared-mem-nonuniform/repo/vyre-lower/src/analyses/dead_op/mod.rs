//! Dead-op detection on `KernelDescriptor`.
//!
//! A KernelOp is dead when:
//! 1. It produces a result (`op.result.is_some()`)
//! 2. No other op in the same kernel body (or its child bodies) reads
//!    that result
//! 3. The op has no side effects (i.e. not a Store, Barrier, AtomicXxx,
//!    AsyncXxx, Trap/Resume, IndirectDispatch, Return)
//!
//! Phase 1: detection only  -  returns a list of op-indices flagged as
//! dead. Phase 2: a real DCE rewrite that strips them from the
//! descriptor (defers to vyre-opt's optimizer pipeline).

use crate::op_properties::kernel_op_kind_is_dce_pure as is_pure;
use crate::operand_semantics::operand_is_result_reference;
use crate::{KernelBody, KernelDescriptor};
use rustc_hash::FxHashSet;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DeadOpReport {
    pub kernel_id: String,
    /// Op-indices (into `KernelBody.ops`) of detected dead ops.
    pub dead_op_indices: Vec<usize>,
    /// Total ops in the body (for context  -  `dead_count / total` gives
    /// the dead-code ratio).
    pub total_op_count: u32,
}

impl DeadOpReport {
    #[must_use]
    pub fn dead_count(&self) -> usize {
        self.dead_op_indices.len()
    }

    #[must_use]
    pub fn dead_ratio(&self) -> f32 {
        if self.total_op_count == 0 {
            0.0
        } else {
            self.dead_op_indices.len() as f32 / self.total_op_count as f32
        }
    }
}

#[must_use]
pub fn analyze(desc: &KernelDescriptor) -> DeadOpReport {
    // First pass: collect every result-id that any op produces. This
    // is the universe of valid "result references."
    let mut produced = FxHashSet::<u32>::with_capacity_and_hasher(
        count_ops(&desc.body) as usize,
        Default::default(),
    );
    walk_collect_produced(&desc.body, &mut produced);

    // Second pass: collect every operand that IS a produced result-id
    // (filtering out binding-slots, pool-indices, body-indices, axis
    // numbers, and any other operand that lives in a different
    // namespace from the result-id space).
    let mut referenced =
        FxHashSet::<u32>::with_capacity_and_hasher(produced.len(), Default::default());
    walk_collect_result_references(&desc.body, &produced, &mut referenced);

    // Third pass: find ops that produce a result not in `referenced`,
    // are pure (no side effects), and have a result id.
    let mut dead = Vec::new();
    walk_find_dead(&desc.body, &referenced, &mut dead, 0);

    DeadOpReport {
        kernel_id: desc.id.clone(),
        dead_op_indices: dead,
        total_op_count: count_ops(&desc.body),
    }
}

fn walk_collect_produced(body: &KernelBody, produced: &mut FxHashSet<u32>) {
    for op in &body.ops {
        for result in op.result_ids() {
            produced.insert(result);
        }
    }
    for child in &body.child_bodies {
        walk_collect_produced(child, produced);
    }
}

fn walk_collect_result_references(
    body: &KernelBody,
    produced: &FxHashSet<u32>,
    referenced: &mut FxHashSet<u32>,
) {
    for op in &body.ops {
        for (pos, operand_id) in op.operands.iter().enumerate() {
            if !operand_is_result_reference(&op.kind, pos) {
                continue;
            }
            if produced.contains(operand_id) {
                referenced.insert(*operand_id);
            }
        }
    }
    for child in &body.child_bodies {
        walk_collect_result_references(child, produced, referenced);
    }
}

fn count_ops(body: &KernelBody) -> u32 {
    let mut total: u32 = body.ops.len() as u32;
    for child in &body.child_bodies {
        total = total.saturating_add(count_ops(child));
    }
    total
}

fn walk_find_dead(
    body: &KernelBody,
    referenced: &FxHashSet<u32>,
    dead: &mut Vec<usize>,
    op_index_offset: usize,
) {
    for (local_idx, op) in body.ops.iter().enumerate() {
        let op_index = op_index_offset + local_idx;
        if op.result.is_some() {
            if op.result_ids().all(|result| !referenced.contains(&result)) && is_pure(&op.kind) {
                dead.push(op_index);
            }
        }
    }
    for child in &body.child_bodies {
        walk_find_dead(child, referenced, dead, op_index_offset + body.ops.len());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BindingLayout, BindingSlot, BindingVisibility, Dispatch, KernelBody, KernelDescriptor,
        KernelOp, KernelOpKind, LiteralValue, MemoryClass,
    };
    use vyre_foundation::ir::{BinOp, DataType};

    #[test]
    fn empty_kernel_has_no_dead_ops() {
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
        let r = analyze(&desc);
        assert!(r.dead_op_indices.is_empty());
        assert_eq!(r.total_op_count, 0);
        assert!((r.dead_ratio() - 0.0).abs() < 1e-6);
    }

    #[test]
    fn unused_literal_is_dead() {
        // Two literals; one is used in a store, the other is dead.
        let desc = KernelDescriptor {
            id: "k".into(),
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
        };
        let r = analyze(&desc);
        assert_eq!(r.dead_op_indices.len(), 1);
        assert_eq!(r.dead_op_indices[0], 2); // the third op
        assert_eq!(r.dead_count(), 1);
    }

    #[test]
    fn store_is_never_dead_even_with_no_result() {
        // Store has no result by definition; should NOT be flagged as dead.
        let desc = KernelDescriptor {
            id: "k".into(),
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
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 0, 1],
                        result: None,
                    },
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(0), LiteralValue::U32(7)],
            },
        };
        let r = analyze(&desc);
        assert!(r.dead_op_indices.is_empty());
    }

    #[test]
    fn unused_arithmetic_op_is_dead() {
        // tid; lit; add(tid, lit) → never used.
        let desc = KernelDescriptor {
            id: "k".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(64, 1, 1),
            body: KernelBody {
                ops: vec![
                    KernelOp {
                        kind: KernelOpKind::LocalInvocationId,
                        operands: vec![],
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
        let r = analyze(&desc);
        // op at index 2 (the Add) is dead  -  its result 2 is never used
        // (and operands 0 and 1 ARE used by it, so they're alive).
        // op at index 0 (tid) is dead too  -  result 0 is only used by
        // the dead Add, so transitively dead.
        // op at index 1 (lit) is dead too  -  same reason.
        // Phase-1 conservative: only flag direct deads (an op whose
        // result is unreferenced)  -  the chain-DCE is phase 2.
        assert_eq!(r.dead_op_indices.len(), 1);
        assert_eq!(r.dead_op_indices[0], 2);
    }

    #[test]
    fn dead_ratio_computed_correctly() {
        // 4 ops, 1 dead → ratio 0.25.
        let r = DeadOpReport {
            kernel_id: "k".into(),
            dead_op_indices: vec![2],
            total_op_count: 4,
        };
        assert!((r.dead_ratio() - 0.25).abs() < 1e-5);
    }

    #[test]
    fn return_is_never_dead() {
        let desc = KernelDescriptor {
            id: "k".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops: vec![KernelOp {
                    kind: KernelOpKind::Return,
                    operands: vec![],
                    result: None,
                }],
                child_bodies: vec![],
                literals: vec![],
            },
        };
        let r = analyze(&desc);
        assert!(r.dead_op_indices.is_empty());
    }

    #[test]
    fn barrier_is_never_dead() {
        let desc = KernelDescriptor {
            id: "k".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(64, 1, 1),
            body: KernelBody {
                ops: vec![KernelOp {
                    kind: KernelOpKind::Barrier {
                        ordering: vyre_foundation::runtime::memory_model::MemoryOrdering::SeqCst,
                    },
                    operands: vec![],
                    result: None,
                }],
                child_bodies: vec![],
                literals: vec![],
            },
        };
        let r = analyze(&desc);
        assert!(r.dead_op_indices.is_empty());
    }
}
