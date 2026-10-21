//! Bind-group reuse detection.
//!
//! Source-of-truth: `PERF_ROADMAP_2026-05-01.md` section D item D6.
//!
//! Two `KernelDescriptor`s that share an identical binding layout
//! (same slot/dtype/access for every binding) can share the same
//! `wgpu::BindGroup` instance across dispatches, avoiding the per-
//! dispatch bind-group construction cost.
//!
//! This is a cross-kernel optimization: takes a slice of descriptors,
//! groups them by binding-layout hash, and returns a `BindGroupReusePlan`
//! listing which descriptors share a layout.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use vyre_lower::{BindingLayout, KernelDescriptor};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReuseGroup {
    pub layout_hash: u64,
    /// Indices into the input slice of descriptors that share this layout.
    pub kernel_indices: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BindGroupReusePlan {
    pub groups: Vec<ReuseGroup>,
}

impl BindGroupReusePlan {
    /// Number of bind-group instances that would be saved by adopting
    /// the reuse plan: total descriptors minus number of unique layouts.
    #[must_use]
    pub fn instances_saved(&self) -> usize {
        let total: usize = self.groups.iter().map(|g| g.kernel_indices.len()).sum();
        total.saturating_sub(self.groups.len())
    }
}

#[must_use]
pub fn analyze(descriptors: &[&KernelDescriptor]) -> BindGroupReusePlan {
    let mut groups: BTreeMap<u64, Vec<usize>> = BTreeMap::new();
    for (i, desc) in descriptors.iter().enumerate() {
        let h = hash_binding_layout(&desc.bindings);
        groups.entry(h).or_default().push(i);
    }
    BindGroupReusePlan {
        groups: groups
            .into_iter()
            .filter(|(_, ks)| ks.len() > 1)
            .map(|(layout_hash, kernel_indices)| ReuseGroup {
                layout_hash,
                kernel_indices,
            })
            .collect(),
    }
}

fn hash_binding_layout(layout: &BindingLayout) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for slot in &layout.slots {
        slot.slot.hash(&mut hasher);
        format!("{:?}", slot.element_type).hash(&mut hasher);
        slot.element_count.hash(&mut hasher);
        slot.memory_class.hash(&mut hasher);
        slot.visibility.hash(&mut hasher);
        // Note: `name` is NOT hashed  -  names are caller-friendly debug
        // labels, not part of the layout contract.
    }
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre_foundation::ir::DataType;
    use vyre_lower::{
        BindingLayout, BindingSlot, BindingVisibility, Dispatch, KernelBody, KernelDescriptor,
        MemoryClass,
    };

    fn k(name: &str, layout: BindingLayout) -> KernelDescriptor {
        KernelDescriptor {
            id: name.into(),
            bindings: layout,
            dispatch: Dispatch::new(64, 1, 1),
            body: KernelBody {
                ops: vec![],
                child_bodies: vec![],
                literals: vec![],
            },
        }
    }

    fn one_u32_layout() -> BindingLayout {
        BindingLayout {
            slots: vec![BindingSlot {
                slot: 0,
                element_type: DataType::U32,
                element_count: None,
                memory_class: MemoryClass::Global,
                visibility: BindingVisibility::ReadWrite,
                name: "out".into(),
            }],
        }
    }

    fn one_f32_layout() -> BindingLayout {
        BindingLayout {
            slots: vec![BindingSlot {
                slot: 0,
                element_type: DataType::F32,
                element_count: None,
                memory_class: MemoryClass::Global,
                visibility: BindingVisibility::ReadWrite,
                name: "out".into(),
            }],
        }
    }

    #[test]
    fn no_descriptors_yields_empty_plan() {
        let p = analyze(&[]);
        assert!(p.groups.is_empty());
        assert_eq!(p.instances_saved(), 0);
    }

    #[test]
    fn single_descriptor_yields_no_reuse_groups() {
        let k1 = k("k1", one_u32_layout());
        let p = analyze(&[&k1]);
        assert!(p.groups.is_empty());
    }

    #[test]
    fn two_identical_layouts_form_reuse_group() {
        let k1 = k("k1", one_u32_layout());
        let k2 = k("k2", one_u32_layout());
        let p = analyze(&[&k1, &k2]);
        assert_eq!(p.groups.len(), 1);
        assert_eq!(p.groups[0].kernel_indices, vec![0, 1]);
        assert_eq!(p.instances_saved(), 1);
    }

    #[test]
    fn two_distinct_layouts_form_no_reuse_group() {
        let k1 = k("k1", one_u32_layout());
        let k2 = k("k2", one_f32_layout());
        let p = analyze(&[&k1, &k2]);
        assert!(p.groups.is_empty());
    }

    #[test]
    fn binding_name_does_not_affect_layout_hash() {
        let mut k1_layout = one_u32_layout();
        let mut k2_layout = one_u32_layout();
        k1_layout.slots[0].name = "alpha".into();
        k2_layout.slots[0].name = "beta".into();
        let k1 = k("k1", k1_layout);
        let k2 = k("k2", k2_layout);
        let p = analyze(&[&k1, &k2]);
        assert_eq!(
            p.groups.len(),
            1,
            "names are debug-only, not part of layout"
        );
    }

    #[test]
    fn three_kernels_two_layouts_one_reuse_group() {
        let k1 = k("k1", one_u32_layout());
        let k2 = k("k2", one_u32_layout());
        let k3 = k("k3", one_f32_layout());
        let p = analyze(&[&k1, &k2, &k3]);
        assert_eq!(p.groups.len(), 1);
        assert_eq!(p.groups[0].kernel_indices, vec![0, 1]);
        assert_eq!(p.instances_saved(), 1);
    }

    #[test]
    fn instances_saved_aggregates_across_multiple_groups() {
        let k1 = k("k1", one_u32_layout());
        let k2 = k("k2", one_u32_layout());
        let k3 = k("k3", one_u32_layout());
        let k4 = k("k4", one_f32_layout());
        let k5 = k("k5", one_f32_layout());
        let p = analyze(&[&k1, &k2, &k3, &k4, &k5]);
        assert_eq!(p.groups.len(), 2);
        // 5 kernels, 2 unique layouts → 5 - 2 = 3 saved bind-groups.
        assert_eq!(p.instances_saved(), 3);
    }
}
