//! Push-constant inlining pattern detection.
//!
//! Source-of-truth: `PERF_ROADMAP_2026-05-01.md` section D item D7.
//!
//! WGSL / SPIR-V / Vulkan support push constants  -  a small block of
//! scalar values stored in dedicated GPU registers rather than in a
//! uniform buffer. Reads from push constants are roughly 5-10x faster
//! than uniform buffer reads.
//!
//! A `BindingSlot` qualifies for push-constant promotion when:
//! - `memory_class == Constant`
//! - `element_type` is a small scalar (≤ 16 bytes total: U32/I32/F32/Bool)
//! - `element_count` is `Some(1)` (a single value, not an array)
//! - The total bytes across all candidate bindings ≤ push-constant
//!   block budget (default 128 bytes; backend-specific in practice).
//!
//! Phase 1 (this module): detection only. Returns a list of
//! `PushConstantCandidate` entries. Phase 2: actually rewrite the
//! emitted naga::Module to declare push constants instead of uniform
//! storage.

use serde::{Deserialize, Serialize};
use vyre_foundation::ir::DataType;
use vyre_lower::{BindingSlot, KernelDescriptor, MemoryClass};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PushConstantCandidate {
    pub binding_slot: u32,
    pub element_type: String, // formatted DataType for serde stability
    pub bytes: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PushConstantPlan {
    pub kernel_id: String,
    pub candidates: Vec<PushConstantCandidate>,
    pub total_bytes: u32,
    pub budget_bytes: u32,
}

impl PushConstantPlan {
    #[must_use]
    pub fn fits_in_budget(&self) -> bool {
        self.total_bytes <= self.budget_bytes
    }
}

/// Default push-constant block budget. WGSL spec is "implementation-
/// defined ≥ 128 bytes"; CUDA is 4 KiB; Vulkan 128 byte minimum
/// guaranteed (256 typical). 128 is the safe portable budget.
pub const DEFAULT_PUSH_CONSTANT_BUDGET_BYTES: u32 = 128;

#[must_use]
pub fn analyze(desc: &KernelDescriptor) -> PushConstantPlan {
    analyze_with_budget(desc, DEFAULT_PUSH_CONSTANT_BUDGET_BYTES)
}

#[must_use]
pub fn analyze_with_budget(desc: &KernelDescriptor, budget_bytes: u32) -> PushConstantPlan {
    // Upper bound on candidates is the binding-slot count: each slot
    // either qualifies (one push) or doesn't (no push). Pre-sizing
    // keeps the typical small-binding-set case to a single allocation.
    let mut candidates = Vec::with_capacity(desc.bindings.slots.len());
    let mut total: u32 = 0;
    for binding in &desc.bindings.slots {
        if let Some(bytes) = qualifies(binding) {
            candidates.push(PushConstantCandidate {
                binding_slot: binding.slot,
                element_type: format!("{:?}", binding.element_type),
                bytes,
            });
            total = total.saturating_add(bytes);
        }
    }
    PushConstantPlan {
        kernel_id: desc.id.clone(),
        candidates,
        total_bytes: total,
        budget_bytes,
    }
}

fn qualifies(binding: &BindingSlot) -> Option<u32> {
    if !matches!(binding.memory_class, MemoryClass::Constant) {
        return None;
    }
    if binding.element_count != Some(1) {
        return None;
    }
    let bytes = match binding.element_type {
        DataType::U8 | DataType::I8 | DataType::Bool => 4,
        DataType::U16 | DataType::I16 | DataType::F16 => 4,
        DataType::U32 | DataType::I32 | DataType::F32 => 4,
        DataType::U64 | DataType::I64 | DataType::F64 => 8,
        DataType::Vec2U32 => 8,
        DataType::Vec4U32 => 16,
        _ => return None,
    };
    Some(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre_lower::{
        BindingLayout, BindingSlot, BindingVisibility, Dispatch, KernelBody, KernelDescriptor,
    };

    fn const_binding(slot: u32, dtype: DataType) -> BindingSlot {
        BindingSlot {
            slot,
            element_type: dtype,
            element_count: Some(1),
            memory_class: MemoryClass::Constant,
            visibility: BindingVisibility::ReadOnly,
            name: format!("c{slot}"),
        }
    }

    fn empty_kernel(slots: Vec<BindingSlot>) -> KernelDescriptor {
        KernelDescriptor {
            id: "k".into(),
            bindings: BindingLayout { slots },
            dispatch: Dispatch::new(64, 1, 1),
            body: KernelBody {
                ops: vec![],
                child_bodies: vec![],
                literals: vec![],
            },
        }
    }

    #[test]
    fn empty_kernel_no_candidates() {
        let p = analyze(&empty_kernel(vec![]));
        assert!(p.candidates.is_empty());
        assert!(p.fits_in_budget());
    }

    #[test]
    fn single_const_u32_binding_qualifies() {
        let p = analyze(&empty_kernel(vec![const_binding(0, DataType::U32)]));
        assert_eq!(p.candidates.len(), 1);
        assert_eq!(p.candidates[0].bytes, 4);
        assert_eq!(p.total_bytes, 4);
    }

    #[test]
    fn const_vec4_qualifies_with_16_bytes() {
        let p = analyze(&empty_kernel(vec![const_binding(0, DataType::Vec4U32)]));
        assert_eq!(p.candidates[0].bytes, 16);
    }

    #[test]
    fn global_binding_does_not_qualify() {
        let p = analyze(&empty_kernel(vec![BindingSlot {
            slot: 0,
            element_type: DataType::U32,
            element_count: Some(1),
            memory_class: MemoryClass::Global,
            visibility: BindingVisibility::ReadOnly,
            name: "g".into(),
        }]));
        assert!(p.candidates.is_empty());
    }

    #[test]
    fn array_binding_does_not_qualify() {
        let p = analyze(&empty_kernel(vec![BindingSlot {
            slot: 0,
            element_type: DataType::U32,
            element_count: Some(64),
            memory_class: MemoryClass::Constant,
            visibility: BindingVisibility::ReadOnly,
            name: "arr".into(),
        }]));
        assert!(p.candidates.is_empty());
    }

    #[test]
    fn complex_dtype_does_not_qualify() {
        let p = analyze(&empty_kernel(vec![BindingSlot {
            slot: 0,
            element_type: DataType::Tensor,
            element_count: Some(1),
            memory_class: MemoryClass::Constant,
            visibility: BindingVisibility::ReadOnly,
            name: "tensor".into(),
        }]));
        assert!(p.candidates.is_empty());
    }

    #[test]
    fn many_small_consts_fit_in_budget() {
        let bindings: Vec<_> = (0..16).map(|s| const_binding(s, DataType::U32)).collect();
        let p = analyze(&empty_kernel(bindings));
        assert_eq!(p.candidates.len(), 16);
        assert_eq!(p.total_bytes, 64);
        assert!(p.fits_in_budget());
    }

    #[test]
    fn over_budget_does_not_fit() {
        let bindings: Vec<_> = (0..16)
            .map(|s| const_binding(s, DataType::Vec4U32))
            .collect();
        let p = analyze(&empty_kernel(bindings));
        // 16 * 16 = 256 bytes > 128 budget.
        assert_eq!(p.total_bytes, 256);
        assert!(!p.fits_in_budget());
    }

    #[test]
    fn custom_budget_changes_fit_classification() {
        let bindings: Vec<_> = (0..16)
            .map(|s| const_binding(s, DataType::Vec4U32))
            .collect();
        let p = analyze_with_budget(&empty_kernel(bindings), 512);
        assert!(p.fits_in_budget());
    }

    #[test]
    fn default_budget_constant_is_128() {
        assert_eq!(DEFAULT_PUSH_CONSTANT_BUDGET_BYTES, 128);
    }
}
