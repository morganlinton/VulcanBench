//! PERF B1 (PTX-side): vector-store fusion candidate detection.
//!
//! Mirror of [`super::vec_load_fusion`] for `StoreGlobal`. NVIDIA
//! GPUs support `st.global.v2.u32` and `st.global.v4.u32` for packed
//! stores  -  same throughput benefits as the load side.
//!
//! Same chain shape: `Store(slot, base_idx, val0); Add(base, 1);
//! Store(slot, idx1, val1); Add(idx1, 1); Store(slot, idx2, val2); ...`
//! up to 4 stores. The PTX emitter lowers the same chain to packed
//! `st.global.v2/v4` instructions.
//!
//! Differences from the load-side analysis:
//! - Stores have no result-id (the chain check looks at the index
//!   operand instead of the result).
//! - The "value" operands of the chained stores are independent  -
//!   they go into the v2/v4 register the way they appear.
//! - Same alignment requirement: `group_size * elem_size` bytes.

use serde::{Deserialize, Serialize};
use vyre_foundation::ir::DataType;
use vyre_lower::KernelDescriptor;

use super::vec_memory_fusion::{analyze_memory_fusion, MemoryFusionCandidate, MemoryFusionKind};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FusionCandidate {
    /// Op-index of the FIRST store in the group.
    pub first_store_idx: usize,
    /// Number of stores in the group (2 or 4  -  PTX has no v3).
    pub group_size: u8,
    /// Binding slot all stores share.
    pub binding_slot: u32,
    /// Element type from the binding.
    pub element_type: DataType,
    /// Required base-pointer alignment in bytes.
    pub alignment_bytes: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct FusionPlan {
    pub candidates: Vec<FusionCandidate>,
}

#[must_use]
pub fn analyze(desc: &KernelDescriptor) -> FusionPlan {
    FusionPlan {
        candidates: analyze_memory_fusion(desc, MemoryFusionKind::Store)
            .into_iter()
            .map(FusionCandidate::from)
            .collect(),
    }
}

impl From<MemoryFusionCandidate> for FusionCandidate {
    fn from(candidate: MemoryFusionCandidate) -> Self {
        Self {
            first_store_idx: candidate.first_op_idx,
            group_size: candidate.group_size,
            binding_slot: candidate.binding_slot,
            element_type: candidate.element_type,
            alignment_bytes: candidate.alignment_bytes,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre_foundation::ir::{BinOp, DataType};
    use vyre_lower::{
        BindingLayout, BindingSlot, BindingVisibility, Dispatch, KernelBody, KernelDescriptor,
        KernelOp, KernelOpKind, LiteralValue, MemoryClass,
    };

    fn slot() -> BindingSlot {
        BindingSlot {
            slot: 0,
            element_type: DataType::U32,
            element_count: None,
            memory_class: MemoryClass::Global,
            visibility: BindingVisibility::WriteOnly,
            name: "out".into(),
        }
    }

    fn build(ops: Vec<KernelOp>, lits: Vec<LiteralValue>) -> KernelDescriptor {
        KernelDescriptor {
            id: "k".into(),
            bindings: BindingLayout {
                slots: vec![slot()],
            },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops,
                child_bodies: vec![],
                literals: lits,
            },
        }
    }

    #[test]
    fn no_stores_no_candidates() {
        assert!(analyze(&build(vec![], vec![])).candidates.is_empty());
    }

    #[test]
    fn two_consecutive_stores_with_idx_plus_one_form_v2_candidate() {
        // r0 = Lit(0)            // base idx
        // r1 = Lit(1)            // stride
        // r2 = Lit(7)            // val0
        // r3 = Lit(8)            // val1
        // Store(slot=0, idx=r0, val=r2)
        // r4 = Add(r0, r1)       // idx+1
        // Store(slot=0, idx=r4, val=r3)
        let desc = build(
            vec![
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
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![0, 0, 2],
                    result: None,
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Add),
                    operands: vec![0, 1],
                    result: Some(4),
                },
                KernelOp {
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![0, 4, 3],
                    result: None,
                },
            ],
            vec![
                LiteralValue::U32(0),
                LiteralValue::U32(1),
                LiteralValue::U32(7),
                LiteralValue::U32(8),
            ],
        );
        let plan = analyze(&desc);
        assert_eq!(plan.candidates.len(), 1);
        assert_eq!(plan.candidates[0].group_size, 2);
        assert_eq!(plan.candidates[0].alignment_bytes, 8);
    }

    #[test]
    fn four_stores_form_v4_candidate() {
        let desc = build(
            vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                }, // base
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![1],
                    result: Some(1),
                }, // stride
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![2],
                    result: Some(2),
                }, // val
                KernelOp {
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![0, 0, 2],
                    result: None,
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Add),
                    operands: vec![0, 1],
                    result: Some(3),
                },
                KernelOp {
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![0, 3, 2],
                    result: None,
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Add),
                    operands: vec![3, 1],
                    result: Some(4),
                },
                KernelOp {
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![0, 4, 2],
                    result: None,
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Add),
                    operands: vec![4, 1],
                    result: Some(5),
                },
                KernelOp {
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![0, 5, 2],
                    result: None,
                },
            ],
            vec![
                LiteralValue::U32(0),
                LiteralValue::U32(1),
                LiteralValue::U32(7),
            ],
        );
        let plan = analyze(&desc);
        assert_eq!(plan.candidates.len(), 1);
        assert_eq!(plan.candidates[0].group_size, 4);
        assert_eq!(plan.candidates[0].alignment_bytes, 16);
    }

    #[test]
    fn single_store_no_candidate() {
        let desc = build(
            vec![
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
            vec![LiteralValue::U32(0), LiteralValue::U32(7)],
        );
        let plan = analyze(&desc);
        assert!(plan.candidates.is_empty());
    }

    #[test]
    fn stores_to_different_slots_dont_chain() {
        let mut s2 = slot();
        s2.slot = 1;
        s2.name = "out2".into();
        let desc = KernelDescriptor {
            id: "k".into(),
            bindings: BindingLayout {
                slots: vec![slot(), s2],
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
                    },
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 0, 2],
                        result: None,
                    },
                    KernelOp {
                        kind: KernelOpKind::BinOpKind(BinOp::Add),
                        operands: vec![0, 1],
                        result: Some(3),
                    },
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![1, 3, 2],
                        result: None,
                    },
                ],
                child_bodies: vec![],
                literals: vec![
                    LiteralValue::U32(0),
                    LiteralValue::U32(1),
                    LiteralValue::U32(7),
                ],
            },
        };
        assert!(analyze(&desc).candidates.is_empty());
    }

    #[test]
    fn three_stores_only_yields_v2() {
        let desc = build(
            vec![
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
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![0, 0, 2],
                    result: None,
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Add),
                    operands: vec![0, 1],
                    result: Some(3),
                },
                KernelOp {
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![0, 3, 2],
                    result: None,
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Add),
                    operands: vec![3, 1],
                    result: Some(4),
                },
                KernelOp {
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![0, 4, 2],
                    result: None,
                },
            ],
            vec![
                LiteralValue::U32(0),
                LiteralValue::U32(1),
                LiteralValue::U32(7),
            ],
        );
        let plan = analyze(&desc);
        assert_eq!(plan.candidates.len(), 1);
        assert_eq!(plan.candidates[0].group_size, 2);
    }

    #[test]
    fn folded_literal_index_gap_stores_form_v4_candidate() {
        let desc = build(
            vec![
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
                    kind: KernelOpKind::Literal,
                    operands: vec![4],
                    result: Some(4),
                },
                KernelOp {
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![0, 0, 1],
                    result: None,
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![5],
                    result: Some(5),
                },
                KernelOp {
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![0, 5, 2],
                    result: None,
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![6],
                    result: Some(6),
                },
                KernelOp {
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![0, 6, 3],
                    result: None,
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![7],
                    result: Some(7),
                },
                KernelOp {
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![0, 7, 4],
                    result: None,
                },
            ],
            vec![
                LiteralValue::U32(0),
                LiteralValue::U32(10),
                LiteralValue::U32(11),
                LiteralValue::U32(12),
                LiteralValue::U32(13),
                LiteralValue::U32(1),
                LiteralValue::U32(2),
                LiteralValue::U32(3),
            ],
        );
        let plan = analyze(&desc);
        assert_eq!(plan.candidates.len(), 1);
        assert_eq!(plan.candidates[0].group_size, 4);
        assert_eq!(plan.candidates[0].alignment_bytes, 16);
    }

    #[test]
    fn value_producer_gap_does_not_form_candidate() {
        let desc = build(
            vec![
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
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![0, 2, 3],
                    result: None,
                },
            ],
            vec![
                LiteralValue::U32(0),
                LiteralValue::U32(10),
                LiteralValue::U32(1),
                LiteralValue::U32(11),
            ],
        );
        assert!(analyze(&desc).candidates.is_empty());
    }
}
