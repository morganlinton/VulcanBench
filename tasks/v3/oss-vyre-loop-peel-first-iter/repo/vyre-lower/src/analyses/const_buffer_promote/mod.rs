//! PERF B10: constant-buffer promotion candidate detection.
//!
//! Source-of-truth: `PERF_ROADMAP_2026-05-01.md` section B item B10.
//!
//! Small read-only data accessed many times across a workgroup
//! benefits from being promoted from a Storage/SSBO buffer to a
//! Constant/Uniform buffer. Constant buffers are cached in dedicated
//! scalar-read hardware and serve reads in 1-2 cycles vs 100s for
//! global memory.
//!
//! Eligibility (phase 1):
//! - `memory_class == Global` and `visibility == ReadOnly`
//! - `element_count.is_some()` (fixed size  -  constant buffers have a
//!   compile-time size limit, typically 64 KiB)
//! - Total bytes ≤ const-buffer budget (default 64 KiB)
//! - Multiple loads against the binding (single-load doesn't repay
//!   the cache-line preload)
//!
//! Rewrite consumers change `binding.memory_class` to `Constant` and
//! let each emitter map the descriptor class to its native artifact.

use crate::{
    BindingSlot, BindingVisibility, KernelBody, KernelDescriptor, KernelOpKind, MemoryClass,
};
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
#[cfg(test)]
use vyre_foundation::ir::DataType;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConstBufferCandidate {
    pub binding_slot: u32,
    pub bytes: u32,
    pub load_count: u32,
    /// Estimated speedup: roughly `1.0 + load_count * 0.4` capped at 8x.
    pub estimated_speedup_factor: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConstBufferPlan {
    pub kernel_id: String,
    pub candidates: Vec<ConstBufferCandidate>,
    pub total_bytes: u32,
    pub budget_bytes: u32,
}

impl ConstBufferPlan {
    #[must_use]
    pub fn fits_in_budget(&self) -> bool {
        self.total_bytes <= self.budget_bytes
    }
}

/// Default const-buffer budget: 64 KiB. Callers with tighter backend
/// limits should pass their real budget into the analysis entry point.
pub const DEFAULT_CONST_BUFFER_BUDGET_BYTES: u32 = 64 * 1024;

#[must_use]
pub fn analyze(desc: &KernelDescriptor) -> ConstBufferPlan {
    analyze_with_budget(desc, DEFAULT_CONST_BUFFER_BUDGET_BYTES)
}

#[must_use]
pub fn analyze_with_budget(desc: &KernelDescriptor, budget_bytes: u32) -> ConstBufferPlan {
    // Eligible bindings.
    let eligible: FxHashMap<u32, &BindingSlot> = desc
        .bindings
        .slots
        .iter()
        .filter(|b| {
            matches!(b.memory_class, MemoryClass::Global)
                && matches!(b.visibility, BindingVisibility::ReadOnly)
                && b.element_count.is_some()
        })
        .map(|b| (b.slot, b))
        .collect();

    // Count loads per slot.
    let mut load_counts =
        FxHashMap::<u32, u32>::with_capacity_and_hasher(eligible.len(), Default::default());
    count_loads(&desc.body, &eligible, &mut load_counts);

    let mut candidates = Vec::new();
    let mut total: u32 = 0;
    for (slot, count) in load_counts {
        if count < 2 {
            continue;
        }
        let binding = eligible[&slot];
        let bytes_per_elem = match binding.element_type.size_bytes() {
            Some(b) => b as u32,
            None => continue,
        };
        let elem_count = binding.element_count.unwrap_or(0);
        let bytes = bytes_per_elem.saturating_mul(elem_count);
        if bytes == 0 || bytes > budget_bytes {
            continue;
        }
        let speedup = (1.0 + count as f32 * 0.4).min(8.0);
        candidates.push(ConstBufferCandidate {
            binding_slot: slot,
            bytes,
            load_count: count,
            estimated_speedup_factor: speedup,
        });
        total = total.saturating_add(bytes);
    }
    candidates.sort_unstable_by_key(|candidate| candidate.binding_slot);
    ConstBufferPlan {
        kernel_id: desc.id.clone(),
        candidates,
        total_bytes: total,
        budget_bytes,
    }
}

fn count_loads(
    body: &KernelBody,
    eligible: &FxHashMap<u32, &BindingSlot>,
    counts: &mut FxHashMap<u32, u32>,
) {
    for op in &body.ops {
        if matches!(op.kind, KernelOpKind::LoadGlobal) {
            if let Some(slot) = op.operands.first() {
                if eligible.contains_key(slot) {
                    *counts.entry(*slot).or_insert(0) += 1;
                }
            }
        }
        for child_id in child_body_operands(&op.kind, &op.operands) {
            if let Some(child) = body.child_bodies.get(child_id as usize) {
                count_loads(child, eligible, counts);
            }
        }
    }
}

fn child_body_operands<'a>(
    kind: &KernelOpKind,
    operands: &'a [u32],
) -> impl Iterator<Item = u32> + 'a {
    let start = match kind {
        KernelOpKind::StructuredIfThen | KernelOpKind::StructuredIfThenElse => 1,
        KernelOpKind::StructuredForLoop { .. } => 2,
        KernelOpKind::StructuredBlock | KernelOpKind::Region { .. } => 0,
        _ => operands.len(),
    };
    operands.iter().skip(start).copied()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BindingLayout, Dispatch, KernelBody, KernelDescriptor, KernelOp, LiteralValue};

    fn ro_global_with_size(slot: u32, count: u32, dtype: DataType) -> BindingSlot {
        BindingSlot {
            slot,
            element_type: dtype,
            element_count: Some(count),
            memory_class: MemoryClass::Global,
            visibility: BindingVisibility::ReadOnly,
            name: format!("ro{slot}"),
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

    fn loads_kernel(slot: u32, load_count: u32, slots: Vec<BindingSlot>) -> KernelDescriptor {
        let mut ops = vec![KernelOp {
            kind: KernelOpKind::Literal,
            operands: vec![0],
            result: Some(0),
        }];
        for i in 0..load_count {
            ops.push(KernelOp {
                kind: KernelOpKind::LoadGlobal,
                operands: vec![slot, 0],
                result: Some(1 + i),
            });
        }
        KernelDescriptor {
            id: "loads".into(),
            bindings: BindingLayout { slots },
            dispatch: Dispatch::new(32, 1, 1),
            body: KernelBody {
                ops,
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(0)],
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
    fn fixed_size_ro_with_two_loads_is_candidate() {
        let p = analyze(&loads_kernel(
            0,
            2,
            vec![ro_global_with_size(0, 16, DataType::F32)],
        ));
        assert_eq!(p.candidates.len(), 1);
        assert_eq!(p.candidates[0].bytes, 64); // 16 * 4
        assert_eq!(p.candidates[0].load_count, 2);
    }

    #[test]
    fn runtime_sized_binding_not_candidate() {
        let mut binding = ro_global_with_size(0, 16, DataType::F32);
        binding.element_count = None;
        let p = analyze(&loads_kernel(0, 2, vec![binding]));
        assert!(p.candidates.is_empty());
    }

    #[test]
    fn read_write_binding_not_candidate() {
        let mut binding = ro_global_with_size(0, 16, DataType::F32);
        binding.visibility = BindingVisibility::ReadWrite;
        let p = analyze(&loads_kernel(0, 2, vec![binding]));
        assert!(p.candidates.is_empty());
    }

    #[test]
    fn single_load_not_candidate() {
        let p = analyze(&loads_kernel(
            0,
            1,
            vec![ro_global_with_size(0, 16, DataType::F32)],
        ));
        assert!(p.candidates.is_empty());
    }

    #[test]
    fn over_budget_binding_not_candidate() {
        // 1M elements * 4 bytes = 4 MiB >> 64 KiB budget.
        let p = analyze(&loads_kernel(
            0,
            2,
            vec![ro_global_with_size(0, 1_000_000, DataType::F32)],
        ));
        assert!(p.candidates.is_empty());
    }

    #[test]
    fn speedup_capped_at_8x() {
        let p = analyze(&loads_kernel(
            0,
            100,
            vec![ro_global_with_size(0, 16, DataType::F32)],
        ));
        assert_eq!(p.candidates[0].load_count, 100);
        // Without cap: 1 + 100*0.4 = 41. With cap: 8.0.
        assert!((p.candidates[0].estimated_speedup_factor - 8.0).abs() < 1e-5);
    }

    #[test]
    fn custom_budget_changes_eligibility() {
        let p = analyze_with_budget(
            &loads_kernel(0, 2, vec![ro_global_with_size(0, 16, DataType::F32)]),
            32, // 32 byte budget  -  64-byte binding doesn't fit
        );
        assert!(p.candidates.is_empty());
    }

    #[test]
    fn default_budget_is_64_kib() {
        assert_eq!(DEFAULT_CONST_BUFFER_BUDGET_BYTES, 65536);
    }
}
