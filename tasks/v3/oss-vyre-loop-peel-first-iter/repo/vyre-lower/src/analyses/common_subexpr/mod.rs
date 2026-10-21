//! Common-subexpression detection on `KernelDescriptor`.
//!
//! Detects pairs of ops that compute the same value  -  same `KernelOpKind`,
//! same operand list  -  and could share a single result instead of
//! recomputing.
//!
//! Returns groups of equivalent ops. The descriptor CSE rewrite picks a
//! canonical op per group and rewrites every subsequent reference to point at
//! the canonical.
//!
//! ## Soundness note
//!
//! Most ops are keyed by exact operand order. A small allow-list of operations
//! with bit-exact symmetric semantics is keyed with sorted binary operands so
//! `xor(x, y)` and `xor(y, x)` share a group. Arithmetic add/mul are not
//! normalized here because this descriptor layer does not carry enough
//! dtype/FP-mode context to prove bit-identical results across all backends.

use crate::{KernelBody, KernelDescriptor, KernelOp, KernelOpKind};
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use vyre_foundation::ir::BinOp;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EquivalenceGroup {
    /// Op-indices that all compute the same value. The first element
    /// is the canonical op (chosen by lowest op-index); the rest are
    /// the duplicates that could be eliminated.
    pub op_indices: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CommonSubexprReport {
    pub kernel_id: String,
    pub groups: Vec<EquivalenceGroup>,
}

impl CommonSubexprReport {
    /// Number of ops that could be eliminated if every group is
    /// canonicalized: total ops in groups minus number of groups.
    #[must_use]
    pub fn ops_eliminable(&self) -> usize {
        self.groups
            .iter()
            .map(|g| g.op_indices.len().saturating_sub(1))
            .sum()
    }
}

#[must_use]
pub fn analyze(desc: &KernelDescriptor) -> CommonSubexprReport {
    analyze_body(desc.id.clone(), &desc.body)
}

#[must_use]
pub fn analyze_body(kernel_id: String, body: &KernelBody) -> CommonSubexprReport {
    analyze_body_impl(kernel_id, body, true)
}

#[must_use]
pub fn analyze_body_shallow(kernel_id: String, body: &KernelBody) -> CommonSubexprReport {
    analyze_body_impl(kernel_id, body, false)
}

fn analyze_body_impl(
    kernel_id: String,
    body: &KernelBody,
    include_children: bool,
) -> CommonSubexprReport {
    let mut buckets: FxHashMap<OpKey, Vec<usize>> = FxHashMap::default();
    let mut next_index = 0usize;
    if include_children {
        walk_body(body, &mut buckets, &mut next_index);
    } else {
        walk_ops(body, &mut buckets, &mut next_index);
    }

    let groups = buckets
        .into_iter()
        .filter(|(_, idxs)| idxs.len() >= 2)
        .map(|(_, op_indices)| EquivalenceGroup { op_indices })
        .collect();

    CommonSubexprReport { kernel_id, groups }
}

fn walk_body(
    body: &KernelBody,
    buckets: &mut FxHashMap<OpKey, Vec<usize>>,
    next_index: &mut usize,
) {
    walk_ops(body, buckets, next_index);
    for child in &body.child_bodies {
        walk_body(child, buckets, next_index);
    }
}

fn walk_ops(body: &KernelBody, buckets: &mut FxHashMap<OpKey, Vec<usize>>, next_index: &mut usize) {
    for op in &body.ops {
        let op_index = *next_index;
        *next_index = next_index.saturating_add(1);
        // Side-effect ops (stores, barriers, etc.) are NEVER candidates
        // for CSE  -  repeating them is the user's intent, not redundancy.
        if !is_eligible(&op.kind) {
            continue;
        }
        let key = OpKey::from_op(op);
        buckets.entry(key).or_default().push(op_index);
    }
}

fn is_eligible(kind: &KernelOpKind) -> bool {
    matches!(
        kind,
        KernelOpKind::Literal
            | KernelOpKind::LocalInvocationId
            | KernelOpKind::GlobalInvocationId
            | KernelOpKind::WorkgroupId
            | KernelOpKind::SubgroupLocalId
            | KernelOpKind::SubgroupSize
            | KernelOpKind::BinOpKind(_)
            | KernelOpKind::UnOpKind(_)
            | KernelOpKind::Fma
            | KernelOpKind::Select
            | KernelOpKind::Cast { .. }
            | KernelOpKind::BufferLength
    )
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct OpKey {
    kind: KernelOpKind,
    operands: SmallVec<[u32; 4]>,
}

impl OpKey {
    fn from_op(op: &KernelOp) -> Self {
        let mut operands = SmallVec::from_slice(&op.operands);
        if let KernelOpKind::BinOpKind(bin_op) = &op.kind {
            normalize_commutative_operands(*bin_op, &mut operands);
        }
        Self {
            kind: op.kind.clone(),
            operands,
        }
    }
}

fn normalize_commutative_operands(bin_op: BinOp, operands: &mut SmallVec<[u32; 4]>) {
    if operands.len() != 2 || !is_bit_exact_commutative_binop(bin_op) {
        return;
    }
    if operands[0] > operands[1] {
        operands.swap(0, 1);
    }
}

fn is_bit_exact_commutative_binop(bin_op: BinOp) -> bool {
    matches!(
        bin_op,
        BinOp::BitAnd
            | BinOp::BitOr
            | BinOp::BitXor
            | BinOp::Eq
            | BinOp::Ne
            | BinOp::And
            | BinOp::Or
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BindingLayout, Dispatch, KernelBody, KernelDescriptor, KernelOp, LiteralValue};
    use vyre_foundation::ir::BinOp;

    #[test]
    fn empty_kernel_no_groups() {
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
        assert!(r.groups.is_empty());
        assert_eq!(r.ops_eliminable(), 0);
    }

    #[test]
    fn two_identical_literals_form_group() {
        let desc = KernelDescriptor {
            id: "dup_lit".into(),
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
                        operands: vec![0],
                        result: Some(1),
                    },
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(7)],
            },
        };
        let r = analyze(&desc);
        assert_eq!(r.groups.len(), 1);
        assert_eq!(r.groups[0].op_indices, vec![0, 1]);
        assert_eq!(r.ops_eliminable(), 1);
    }

    #[test]
    fn distinct_literal_pool_indices_are_distinct() {
        let desc = KernelDescriptor {
            id: "two_lits".into(),
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
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(7), LiteralValue::U32(8)],
            },
        };
        let r = analyze(&desc);
        assert!(r.groups.is_empty());
    }

    #[test]
    fn duplicate_binop_with_same_operands_grouped() {
        let desc = KernelDescriptor {
            id: "dup_add".into(),
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
                        kind: KernelOpKind::BinOpKind(BinOp::Add),
                        operands: vec![0, 1],
                        result: Some(2),
                    },
                    KernelOp {
                        kind: KernelOpKind::BinOpKind(BinOp::Add),
                        operands: vec![0, 1],
                        result: Some(3),
                    },
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(3), LiteralValue::U32(4)],
            },
        };
        let r = analyze(&desc);
        assert_eq!(r.groups.len(), 1);
        assert_eq!(r.groups[0].op_indices, vec![2, 3]);
    }

    #[test]
    fn arithmetic_commutative_swap_not_grouped_without_type_context() {
        // Add may be integer, wrapping, or floating point at this layer. Keep
        // it order-sensitive until descriptor ops carry enough semantic context
        // to prove bit-identical results for every backend.
        let desc = KernelDescriptor {
            id: "comm".into(),
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
                        kind: KernelOpKind::BinOpKind(BinOp::Add),
                        operands: vec![0, 1],
                        result: Some(2),
                    },
                    KernelOp {
                        kind: KernelOpKind::BinOpKind(BinOp::Add),
                        operands: vec![1, 0],
                        result: Some(3),
                    },
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(3), LiteralValue::U32(4)],
            },
        };
        let r = analyze(&desc);
        assert!(
            r.groups.is_empty(),
            "descriptor CSE must not normalize arithmetic add without dtype context"
        );
    }

    #[test]
    fn bit_exact_commutative_swap_is_grouped() {
        let desc = KernelDescriptor {
            id: "comm_bitxor".into(),
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
                        kind: KernelOpKind::BinOpKind(BinOp::BitXor),
                        operands: vec![0, 1],
                        result: Some(2),
                    },
                    KernelOp {
                        kind: KernelOpKind::BinOpKind(BinOp::BitXor),
                        operands: vec![1, 0],
                        result: Some(3),
                    },
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(3), LiteralValue::U32(4)],
            },
        };
        let r = analyze(&desc);
        assert_eq!(r.groups.len(), 1);
        assert_eq!(r.groups[0].op_indices, vec![2, 3]);
    }

    #[test]
    fn store_ops_not_grouped_even_if_identical() {
        // Two identical stores must NOT be CSE'd  -  they're side effects.
        use crate::{BindingSlot, BindingVisibility, MemoryClass};
        use vyre_foundation::ir::DataType;
        let desc = KernelDescriptor {
            id: "double_store".into(),
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
        // Stores are excluded from CSE eligibility.
        let store_groups = r
            .groups
            .iter()
            .filter(|g| {
                g.op_indices
                    .iter()
                    .any(|&i| matches!(desc.body.ops[i].kind, KernelOpKind::StoreGlobal))
            })
            .count();
        assert_eq!(store_groups, 0);
    }

    #[test]
    fn three_identical_literals_eliminate_two() {
        let desc = KernelDescriptor {
            id: "three".into(),
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
                        operands: vec![0],
                        result: Some(1),
                    },
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![0],
                        result: Some(2),
                    },
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(42)],
            },
        };
        let r = analyze(&desc);
        assert_eq!(r.groups.len(), 1);
        assert_eq!(r.groups[0].op_indices, vec![0, 1, 2]);
        assert_eq!(r.ops_eliminable(), 2); // 3 ops, keep 1, eliminate 2
    }

    #[test]
    fn local_invocation_id_calls_grouped() {
        // Two LocalInvocationId calls are equivalent (constant per
        // thread, same in any order).
        let desc = KernelDescriptor {
            id: "tid_dup".into(),
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
                        kind: KernelOpKind::LocalInvocationId,
                        operands: vec![],
                        result: Some(1),
                    },
                ],
                child_bodies: vec![],
                literals: vec![],
            },
        };
        let r = analyze(&desc);
        assert_eq!(r.groups.len(), 1);
    }

    #[test]
    fn sibling_child_body_indices_are_monotonic_not_overlapping() {
        let child = KernelBody {
            ops: vec![KernelOp {
                kind: KernelOpKind::Literal,
                operands: vec![0],
                result: Some(1),
            }],
            child_bodies: vec![],
            literals: vec![],
        };
        let desc = KernelDescriptor {
            id: "siblings".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops: vec![KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                }],
                child_bodies: vec![child.clone(), child],
                literals: vec![LiteralValue::U32(9)],
            },
        };

        let r = analyze(&desc);
        assert_eq!(r.groups.len(), 1);
        assert_eq!(
            r.groups[0].op_indices,
            vec![0, 1, 2],
            "sibling children must receive distinct preorder op indices"
        );
    }

    #[test]
    fn shallow_analysis_excludes_child_bodies() {
        let child = KernelBody {
            ops: vec![KernelOp {
                kind: KernelOpKind::Literal,
                operands: vec![0],
                result: Some(1),
            }],
            child_bodies: vec![],
            literals: vec![],
        };
        let body = KernelBody {
            ops: vec![KernelOp {
                kind: KernelOpKind::Literal,
                operands: vec![0],
                result: Some(0),
            }],
            child_bodies: vec![child],
            literals: vec![LiteralValue::U32(9)],
        };

        let recursive = analyze_body("recursive".into(), &body);
        let shallow = analyze_body_shallow("shallow".into(), &body);
        assert_eq!(recursive.groups.len(), 1);
        assert!(shallow.groups.is_empty());
    }
}
