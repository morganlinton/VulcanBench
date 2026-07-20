//! J1  -  consume [`crate::analyses::layout_aos_to_soa::analyze`] and
//! produce a stable per-binding [`LayoutHint`] map.
//!
//! ## What this is
//!
//! The analysis module identifies bindings whose `Vec{N}` AoS layout
//! could profit from an SoA split (multiple loads × N components ×
//! coalescing wins). This rewrite is the consumer that closes the
//! analysis-action loop at descriptor level.
//!
//! ## Contract
//!
//! Callers run [`promote`] when they only need a stable layout hint, or
//! [`plan_binding_splits`] when they own ABI construction and can append
//! scalar component bindings to the descriptor/bind plan.
//!
//! Backends that apply the split plan emit per-component loads against
//! scalar component buffers, gaining the speedup the analysis estimated.

use crate::analyses::layout_aos_to_soa;
use crate::{BindingSlot, KernelDescriptor};
use vyre_foundation::ir::DataType;

/// Per-binding layout preference produced by [`promote`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LayoutHint {
    /// Default. Compound element types (Vec{N}) live as a single
    /// interleaved buffer.
    Aos,
    /// Compound element types should be split across N parallel
    /// scalar buffers  -  the access pattern justified the split per
    /// the analysis pass.
    Soa,
}

/// Concrete binding-level SoA split plan for one AoS binding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoaBindingSplit {
    /// Original interleaved binding slot.
    pub source_slot: u32,
    /// First newly allocated scalar component slot.
    pub first_component_slot: u32,
    /// Scalar component bindings, one per lane.
    pub component_bindings: Vec<BindingSlot>,
}

/// Complete SoA wire-layout plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoaBindingSplitPlan {
    /// Split candidates in stable source-slot order.
    pub splits: Vec<SoaBindingSplit>,
}

impl SoaBindingSplitPlan {
    /// True when no binding needs a split.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.splits.is_empty()
    }

    /// Number of new scalar component bindings across every split.
    #[must_use]
    pub fn component_binding_count(&self) -> usize {
        self.splits
            .iter()
            .map(|split| split.component_bindings.len())
            .sum()
    }
}

/// Consume the layout-transform analysis plan and produce a hint
/// per binding slot. Bindings the analyzer flagged as candidates
/// get [`LayoutHint::Soa`]; everything else is [`LayoutHint::Aos`].
///
/// The result is a sorted `Vec<(slot, LayoutHint)>` so iteration is
/// deterministic and emitter caches that key on the result are
/// stable across runs.
#[must_use]
pub fn promote(desc: &KernelDescriptor) -> Vec<(u32, LayoutHint)> {
    let plan = layout_aos_to_soa::analyze(desc);
    let mut hints: Vec<(u32, LayoutHint)> = desc
        .bindings
        .slots
        .iter()
        .map(|s| (s.slot, LayoutHint::Aos))
        .collect();
    for cand in &plan.candidates {
        if let Some(entry) = hints
            .iter_mut()
            .find(|(slot, _)| *slot == cand.binding_slot)
        {
            entry.1 = LayoutHint::Soa;
        }
    }
    hints.sort_unstable_by_key(|(slot, _)| *slot);
    hints
}

/// Build the concrete binding split plan for every SoA candidate.
///
/// The plan is the ABI-changing half of J1: callers that own wire-format
/// compatibility can append `component_bindings` to the descriptor/bind plan
/// and lower component loads against `first_component_slot + lane`. Keeping it
/// as a plan avoids silently changing descriptor hashes in backends that only
/// asked for advisory hints.
#[must_use]
pub fn plan_binding_splits(desc: &KernelDescriptor) -> SoaBindingSplitPlan {
    let analysis = layout_aos_to_soa::analyze(desc);
    // Only consider host-visible slots when allocating component slots.
    // Shared/Scratch slots live in the WORKGROUP_SLOT_BASE (1<<24) range
    // and are not host-bound; mixing them in here would push every
    // SoA-split component past the wgpu max binding index (1000) and the
    // bind group layout validator would reject it. Same hazard fix as the
    // trap sidecar slot allocator in vyre-lower::lower::add_trap_sidecar_binding.
    let mut next_slot = desc
        .bindings
        .slots
        .iter()
        .filter(|slot| {
            !matches!(
                slot.memory_class,
                crate::MemoryClass::Shared | crate::MemoryClass::Scratch,
            )
        })
        .map(|slot| slot.slot)
        .max()
        .unwrap_or(0)
        .saturating_add(1);
    let mut splits = Vec::with_capacity(analysis.candidates.len());
    for candidate in analysis.candidates {
        let Some(source) = desc
            .bindings
            .slots
            .iter()
            .find(|slot| slot.slot == candidate.binding_slot)
        else {
            continue;
        };
        let Some(component_type) = component_type(&source.element_type) else {
            continue;
        };
        let first_component_slot = next_slot;
        let mut component_bindings = Vec::with_capacity(candidate.component_count as usize);
        for lane in 0..candidate.component_count {
            let mut binding = source.clone();
            binding.slot = next_slot;
            binding.element_type = component_type.clone();
            binding.name = format!("{}_soa{lane}", source.name);
            component_bindings.push(binding);
            next_slot = next_slot.saturating_add(1);
        }
        splits.push(SoaBindingSplit {
            source_slot: source.slot,
            first_component_slot,
            component_bindings,
        });
    }
    SoaBindingSplitPlan { splits }
}

fn component_type(dtype: &DataType) -> Option<DataType> {
    match dtype {
        DataType::Vec { element, .. } => Some((**element).clone()),
        DataType::Vec2U32 | DataType::Vec4U32 => Some(DataType::U32),
        DataType::TensorShaped { element, .. } => Some((**element).clone()),
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
    use vyre_foundation::ir::DataType;

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

    fn load_op(slot: u32, idx_id: u32, result: u32) -> KernelOp {
        KernelOp {
            kind: KernelOpKind::LoadGlobal,
            operands: vec![slot, idx_id],
            result: Some(result),
        }
    }

    #[test]
    fn empty_descriptor_produces_empty_hints() {
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
        assert!(promote(&desc).is_empty());
    }

    #[test]
    fn scalar_binding_stays_aos() {
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
                    load_op(0, 0, 1),
                    load_op(0, 0, 2),
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(0)],
            },
        };
        let hints = promote(&desc);
        assert_eq!(hints, vec![(0, LayoutHint::Aos)]);
    }

    #[test]
    fn vec4_with_multiple_loads_promotes_to_soa() {
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
                    load_op(0, 0, 1),
                    load_op(0, 0, 2),
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(0)],
            },
        };
        let hints = promote(&desc);
        assert_eq!(hints, vec![(0, LayoutHint::Soa)]);
    }

    #[test]
    fn mixed_bindings_get_per_slot_hints() {
        let desc = KernelDescriptor {
            id: "k".into(),
            bindings: BindingLayout {
                slots: vec![scalar_binding(0), vec4_binding(1)],
            },
            dispatch: Dispatch::new(32, 1, 1),
            body: KernelBody {
                ops: vec![
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![0],
                        result: Some(0),
                    },
                    load_op(0, 0, 1),
                    load_op(1, 0, 2),
                    load_op(1, 0, 3),
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(0)],
            },
        };
        let hints = promote(&desc);
        assert_eq!(hints, vec![(0, LayoutHint::Aos), (1, LayoutHint::Soa)]);
    }

    #[test]
    fn promote_is_idempotent() {
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
                    load_op(0, 0, 1),
                    load_op(0, 0, 2),
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(0)],
            },
        };
        let once = promote(&desc);
        let twice = promote(&desc);
        assert_eq!(once, twice);
    }

    #[test]
    fn split_plan_allocates_scalar_component_bindings() {
        let desc = KernelDescriptor {
            id: "k".into(),
            bindings: BindingLayout {
                slots: vec![vec4_binding(7)],
            },
            dispatch: Dispatch::new(32, 1, 1),
            body: KernelBody {
                ops: vec![
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![0],
                        result: Some(0),
                    },
                    load_op(7, 0, 1),
                    load_op(7, 0, 2),
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(0)],
            },
        };
        let plan = plan_binding_splits(&desc);
        assert_eq!(plan.component_binding_count(), 4);
        assert_eq!(plan.splits[0].source_slot, 7);
        assert_eq!(plan.splits[0].first_component_slot, 8);
        assert_eq!(
            plan.splits[0]
                .component_bindings
                .iter()
                .map(|binding| binding.slot)
                .collect::<Vec<_>>(),
            vec![8, 9, 10, 11]
        );
        assert!(plan.splits[0]
            .component_bindings
            .iter()
            .all(|binding| matches!(binding.element_type, DataType::F32)));
    }
}
