//! PERF B1 (PTX-side): vector-load fusion candidate detection.
//!
//! NVIDIA GPUs support packed vector loads: `ld.global.v2.u32` and
//! `ld.global.v4.u32` move 8 or 16 bytes per transaction with one
//! memory request, instead of 2 or 4 scalar 4-byte loads. On
//! memory-bound kernels this is up to 4× throughput AND reduces
//! per-load address-arithmetic instructions (mul.wide / add.u64).
//!
//! This pattern detects fusion candidates: groups of 2 or 4
//! consecutive `LoadGlobal` ops in the body's flat op stream that:
//!
//! 1. Read from the same `binding_slot`.
//! 2. Have indices `i, i+1, i+2, [i+3]` for the same base  -  detected
//!    when consecutive load's index_id is the result of an `Add(prev_index_id, Lit(1))`
//!    op present in the body.
//! 3. Have no intervening op (other than the index-increment Adds).
//! 4. The base index is naturally aligned for the vector width
//!    (alignment_required is reported; the host may need to verify
//!    this against the runtime allocation alignment).
//!
//! The PTX emitter consumes the same chain shape directly and emits a
//! packed vector load while binding every scalar result id to the
//! registers returned by the vector instruction.
//!
//! Same shape as `vyre-emit-naga::patterns::vec_pack` but PTX-aware:
//! reports vector widths PTX supports (`v2`, `v4`), alignment in
//! bytes, and the expected register class.

use serde::{Deserialize, Serialize};
use vyre_foundation::ir::DataType;
use vyre_lower::KernelDescriptor;

use super::vec_memory_fusion::{analyze_memory_fusion, MemoryFusionCandidate, MemoryFusionKind};

/// One fusion candidate: a group of consecutive scalar loads that
/// could be merged into a single PTX vector load.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FusionCandidate {
    /// Op-index of the FIRST load in the group.
    pub first_load_idx: usize,
    /// Number of loads in the group (2 or 4 only  -  PTX doesn't have
    /// `v3` loads).
    pub group_size: u8,
    /// Binding slot all loads share.
    pub binding_slot: u32,
    /// Element type all loads share  -  must be same.
    pub element_type: DataType,
    /// Required base-pointer alignment in bytes for the fused load
    /// to be valid: `group_size * element_size`. Host-side allocator
    /// must guarantee this.
    pub alignment_bytes: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct FusionPlan {
    pub candidates: Vec<FusionCandidate>,
}

#[must_use]
pub fn analyze(desc: &KernelDescriptor) -> FusionPlan {
    FusionPlan {
        candidates: analyze_memory_fusion(desc, MemoryFusionKind::Load)
            .into_iter()
            .map(FusionCandidate::from)
            .collect(),
    }
}

impl From<MemoryFusionCandidate> for FusionCandidate {
    fn from(candidate: MemoryFusionCandidate) -> Self {
        Self {
            first_load_idx: candidate.first_op_idx,
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
            visibility: BindingVisibility::ReadWrite,
            name: "buf".into(),
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
    fn no_loads_no_candidates() {
        let plan = analyze(&build(vec![], vec![]));
        assert!(plan.candidates.is_empty());
    }

    #[test]
    fn single_load_no_candidate() {
        let desc = build(
            vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::LoadGlobal,
                    operands: vec![0, 0],
                    result: Some(1),
                },
            ],
            vec![LiteralValue::U32(0)],
        );
        let plan = analyze(&desc);
        assert!(plan.candidates.is_empty());
    }

    #[test]
    fn two_consecutive_loads_with_idx_plus_one_form_v2_candidate() {
        // r0 = Lit(0), r1 = Lit(1)
        // r2 = Load(slot=0, idx=r0)
        // r3 = Add(r0, r1)  ; idx+1
        // r4 = Load(slot=0, idx=r3)
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
                    kind: KernelOpKind::LoadGlobal,
                    operands: vec![0, 0],
                    result: Some(2),
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Add),
                    operands: vec![0, 1],
                    result: Some(3),
                },
                KernelOp {
                    kind: KernelOpKind::LoadGlobal,
                    operands: vec![0, 3],
                    result: Some(4),
                },
            ],
            vec![LiteralValue::U32(0), LiteralValue::U32(1)],
        );
        let plan = analyze(&desc);
        assert_eq!(plan.candidates.len(), 1);
        assert_eq!(plan.candidates[0].group_size, 2);
        assert_eq!(plan.candidates[0].binding_slot, 0);
        assert_eq!(plan.candidates[0].alignment_bytes, 8); // 2 * 4
    }

    #[test]
    fn four_consecutive_chained_loads_form_v4_candidate() {
        // r0 = Lit(0), r1 = Lit(1)
        // r2 = Load(0)
        // r3 = Add(0, 1)
        // r4 = Load(3)
        // r5 = Add(3, 1)
        // r6 = Load(5)
        // r7 = Add(5, 1)
        // r8 = Load(7)
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
                    kind: KernelOpKind::LoadGlobal,
                    operands: vec![0, 0],
                    result: Some(2),
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Add),
                    operands: vec![0, 1],
                    result: Some(3),
                },
                KernelOp {
                    kind: KernelOpKind::LoadGlobal,
                    operands: vec![0, 3],
                    result: Some(4),
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Add),
                    operands: vec![3, 1],
                    result: Some(5),
                },
                KernelOp {
                    kind: KernelOpKind::LoadGlobal,
                    operands: vec![0, 5],
                    result: Some(6),
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Add),
                    operands: vec![5, 1],
                    result: Some(7),
                },
                KernelOp {
                    kind: KernelOpKind::LoadGlobal,
                    operands: vec![0, 7],
                    result: Some(8),
                },
            ],
            vec![LiteralValue::U32(0), LiteralValue::U32(1)],
        );
        let plan = analyze(&desc);
        assert_eq!(plan.candidates.len(), 1);
        assert_eq!(plan.candidates[0].group_size, 4);
        assert_eq!(plan.candidates[0].alignment_bytes, 16); // 4 * 4
    }

    #[test]
    fn loads_to_different_slots_dont_chain() {
        let mut s2 = slot();
        s2.slot = 1;
        s2.name = "buf2".into();
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
                        kind: KernelOpKind::LoadGlobal,
                        operands: vec![0, 0],
                        result: Some(2),
                    },
                    KernelOp {
                        kind: KernelOpKind::BinOpKind(BinOp::Add),
                        operands: vec![0, 1],
                        result: Some(3),
                    },
                    // Different slot  -  chain breaks.
                    KernelOp {
                        kind: KernelOpKind::LoadGlobal,
                        operands: vec![1, 3],
                        result: Some(4),
                    },
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(0), LiteralValue::U32(1)],
            },
        };
        let plan = analyze(&desc);
        assert!(plan.candidates.is_empty());
    }

    #[test]
    fn non_unit_stride_doesnt_chain() {
        // Add by 2 instead of 1  -  not a v-load candidate.
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
                }, // 2
                KernelOp {
                    kind: KernelOpKind::LoadGlobal,
                    operands: vec![0, 0],
                    result: Some(2),
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Add),
                    operands: vec![0, 1],
                    result: Some(3),
                },
                KernelOp {
                    kind: KernelOpKind::LoadGlobal,
                    operands: vec![0, 3],
                    result: Some(4),
                },
            ],
            vec![LiteralValue::U32(0), LiteralValue::U32(2)],
        );
        let plan = analyze(&desc);
        assert!(plan.candidates.is_empty());
    }

    #[test]
    fn intervening_memory_effect_breaks_chain() {
        // Load r2; visible memory effect; Add; Load.
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
                    kind: KernelOpKind::LoadGlobal,
                    operands: vec![0, 0],
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
                    kind: KernelOpKind::LoadGlobal,
                    operands: vec![0, 3],
                    result: Some(4),
                },
            ],
            vec![LiteralValue::U32(0), LiteralValue::U32(1)],
        );
        // Pure arithmetic can be scheduled into a load gap, but visible
        // memory effects cannot be crossed by vector-load fusion.
        let plan = analyze(&desc);
        assert!(plan.candidates.is_empty());
    }

    #[test]
    fn three_loads_only_yields_v2_candidate() {
        // Chain of 3  -  PTX has no v3, so we report v2 (covers first 2).
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
                    kind: KernelOpKind::LoadGlobal,
                    operands: vec![0, 0],
                    result: Some(2),
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Add),
                    operands: vec![0, 1],
                    result: Some(3),
                },
                KernelOp {
                    kind: KernelOpKind::LoadGlobal,
                    operands: vec![0, 3],
                    result: Some(4),
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Add),
                    operands: vec![3, 1],
                    result: Some(5),
                },
                KernelOp {
                    kind: KernelOpKind::LoadGlobal,
                    operands: vec![0, 5],
                    result: Some(6),
                },
            ],
            vec![LiteralValue::U32(0), LiteralValue::U32(1)],
        );
        let plan = analyze(&desc);
        // First 2 loads form a v2 candidate; the 3rd is left as scalar.
        assert_eq!(plan.candidates.len(), 1);
        assert_eq!(plan.candidates[0].group_size, 2);
    }
}
