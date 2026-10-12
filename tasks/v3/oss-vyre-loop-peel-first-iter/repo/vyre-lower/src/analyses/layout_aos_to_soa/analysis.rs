//! Walk every binding; if its dtype is a compound type (`Vec`,
//! `TensorShaped`, fixed-size `Array`) AND it has multiple loads,
//! flag it as a layout-transform candidate.

use super::plan::{LayoutCandidate, LayoutTransformPlan};
use crate::analyses::load_counts::count_global_loads_by_slot;
use crate::KernelDescriptor;
use rustc_hash::FxHashMap;
use vyre_foundation::ir::DataType;

#[must_use]
pub fn analyze(desc: &KernelDescriptor) -> LayoutTransformPlan {
    let compound: FxHashMap<u32, u32> = desc
        .bindings
        .slots
        .iter()
        .filter_map(|b| compound_lane_count(&b.element_type).map(|c| (b.slot, c)))
        .collect();

    let mut load_counts: FxHashMap<u32, u32> =
        FxHashMap::with_capacity_and_hasher(compound.len(), Default::default());
    count_global_loads_by_slot(
        &desc.body,
        &|slot| compound.contains_key(&slot),
        &mut load_counts,
    );

    let mut candidates = Vec::new();
    for (slot, count) in load_counts {
        if count >= 2 {
            let component_count = *compound.get(&slot).unwrap_or(&1);
            let speedup = 1.0 + (component_count.saturating_sub(1) as f32) * 0.3;
            candidates.push(LayoutCandidate {
                binding_slot: slot,
                load_count: count,
                component_count,
                estimated_speedup_factor: speedup,
            });
        }
    }
    candidates.sort_unstable_by_key(|candidate| candidate.binding_slot);

    LayoutTransformPlan {
        kernel_id: desc.id.clone(),
        candidates,
    }
}

/// Return the lane / component count for a compound dtype, or `None`
/// for scalars (which are already SoA-friendly).
fn compound_lane_count(dtype: &DataType) -> Option<u32> {
    match dtype {
        DataType::Vec { count, .. } => Some(*count as u32),
        DataType::Vec2U32 => Some(2),
        DataType::Vec4U32 => Some(4),
        DataType::TensorShaped { shape, .. } if !shape.is_empty() => {
            // Use the innermost dimension as the lane count for AoS→SoA.
            shape.last().copied()
        }
        DataType::Array { .. } => Some(2), // Array is the AoS shape itself; conservative split count.
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BindingLayout, BindingSlot, BindingVisibility, Dispatch, KernelBody, KernelDescriptor,
        KernelOp, KernelOpKind, LiteralValue, MemoryClass,
    };

    fn vec4_binding(slot: u32) -> BindingSlot {
        BindingSlot {
            slot,
            element_type: DataType::Vec {
                element: Box::new(DataType::F32),
                count: 4,
            },
            element_count: None,
            memory_class: MemoryClass::Global,
            visibility: BindingVisibility::ReadOnly,
            name: format!("v{slot}"),
        }
    }

    fn scalar_binding(slot: u32) -> BindingSlot {
        BindingSlot {
            slot,
            element_type: DataType::F32,
            element_count: None,
            memory_class: MemoryClass::Global,
            visibility: BindingVisibility::ReadOnly,
            name: format!("s{slot}"),
        }
    }

    #[test]
    fn empty_kernel_has_no_candidates() {
        let desc = KernelDescriptor {
            id: "k".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(64, 1, 1),
            body: KernelBody {
                ops: vec![],
                child_bodies: vec![],
                literals: vec![],
            },
        };
        assert!(analyze(&desc).candidates.is_empty());
    }

    #[test]
    fn scalar_binding_is_not_candidate() {
        let desc = KernelDescriptor {
            id: "k".into(),
            bindings: BindingLayout {
                slots: vec![scalar_binding(0)],
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
                        kind: KernelOpKind::LoadGlobal,
                        operands: vec![0, 0],
                        result: Some(1),
                    },
                    KernelOp {
                        kind: KernelOpKind::LoadGlobal,
                        operands: vec![0, 0],
                        result: Some(2),
                    },
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(0)],
            },
        };
        assert!(analyze(&desc).candidates.is_empty());
    }

    #[test]
    fn vec4_binding_with_two_loads_is_candidate() {
        let desc = KernelDescriptor {
            id: "k".into(),
            bindings: BindingLayout {
                slots: vec![vec4_binding(0)],
            },
            dispatch: Dispatch::new(32, 1, 1),
            body: KernelBody {
                ops: vec![
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
                    KernelOp {
                        kind: KernelOpKind::LoadGlobal,
                        operands: vec![0, 0],
                        result: Some(2),
                    },
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(0)],
            },
        };
        let p = analyze(&desc);
        assert_eq!(p.candidates.len(), 1);
        assert_eq!(p.candidates[0].component_count, 4);
        assert_eq!(p.candidates[0].load_count, 2);
        // 1.0 + (4-1)*0.3 = 1.9
        assert!((p.candidates[0].estimated_speedup_factor - 1.9).abs() < 1e-5);
    }

    #[test]
    fn vec4_binding_with_one_load_is_not_candidate() {
        let desc = KernelDescriptor {
            id: "k".into(),
            bindings: BindingLayout {
                slots: vec![vec4_binding(0)],
            },
            dispatch: Dispatch::new(32, 1, 1),
            body: KernelBody {
                ops: vec![
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
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(0)],
            },
        };
        assert!(analyze(&desc).candidates.is_empty());
    }

    #[test]
    fn structured_if_else_counts_both_load_branches() {
        let desc = KernelDescriptor {
            id: "k".into(),
            bindings: BindingLayout {
                slots: vec![vec4_binding(0)],
            },
            dispatch: Dispatch::new(32, 1, 1),
            body: KernelBody {
                ops: vec![KernelOp {
                    kind: KernelOpKind::StructuredIfThenElse,
                    operands: vec![99, 0, 1],
                    result: None,
                }],
                child_bodies: vec![
                    KernelBody {
                        ops: vec![KernelOp {
                            kind: KernelOpKind::LoadGlobal,
                            operands: vec![0, 0],
                            result: Some(1),
                        }],
                        child_bodies: vec![],
                        literals: vec![],
                    },
                    KernelBody {
                        ops: vec![KernelOp {
                            kind: KernelOpKind::LoadGlobal,
                            operands: vec![0, 0],
                            result: Some(2),
                        }],
                        child_bodies: vec![],
                        literals: vec![],
                    },
                ],
                literals: vec![],
            },
        };

        let plan = analyze(&desc);

        assert_eq!(plan.candidates.len(), 1);
        assert_eq!(plan.candidates[0].load_count, 2);
    }

    #[test]
    fn vec2u32_recognized_as_compound() {
        assert_eq!(compound_lane_count(&DataType::Vec2U32), Some(2));
    }

    #[test]
    fn vec4u32_recognized_as_compound() {
        assert_eq!(compound_lane_count(&DataType::Vec4U32), Some(4));
    }

    #[test]
    fn scalar_types_return_none() {
        assert_eq!(compound_lane_count(&DataType::F32), None);
        assert_eq!(compound_lane_count(&DataType::U32), None);
        assert_eq!(compound_lane_count(&DataType::Bool), None);
    }
}
