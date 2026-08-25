//! Output type for the shared-memory promotion analysis.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PromotionCandidate {
    /// Binding slot index of the global buffer being considered.
    pub binding_slot: u32,
    /// Number of times this binding is read by the kernel body
    /// (per-workgroup, summed across threads).
    pub access_count: u32,
    /// Bytes per element of the binding's element type.
    pub bytes_per_element: u32,
    /// Number of distinct element indices accessed per workgroup
    /// (the size of the tile that would need to be loaded).
    pub distinct_indices_per_workgroup: u32,
    /// Total bytes the promoted tile would occupy in shared memory.
    pub tile_bytes: u32,
    /// Estimated speedup from promotion. Conservative back-of-envelope:
    /// `(access_count - 1) * SHARED_VS_GLOBAL_RATIO`. For 5 accesses
    /// at the conventional 100x ratio, that's `4 * 100 / 5 ≈ 80x`
    /// for the promoted accesses; the report uses a less aggressive
    /// `5 + (access_count - 1) * 2` to avoid overpromise.
    pub estimated_speedup_factor: f32,
}

/// Full promotion plan for one `KernelDescriptor`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PromotionPlan {
    pub kernel_id: String,
    pub candidates: Vec<PromotionCandidate>,
    /// Total tile bytes if every candidate is promoted.
    pub total_tile_bytes: u32,
    /// Available shared-mem budget that was used during analysis.
    pub budget_bytes: u32,
}

impl PromotionPlan {
    /// True if the sum of every candidate's `tile_bytes` fits in the
    /// declared budget. When false, the caller must select a subset
    /// (e.g., highest-speedup-first) before applying.
    #[must_use]
    pub fn fits_in_budget(&self) -> bool {
        self.total_tile_bytes <= self.budget_bytes
    }

    /// Sort candidates highest-speedup-first.
    pub fn sorted_by_speedup(&self) -> Vec<&PromotionCandidate> {
        let mut s: Vec<_> = self.candidates.iter().collect();
        s.sort_by(|a, b| {
            b.estimated_speedup_factor
                .partial_cmp(&a.estimated_speedup_factor)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cand(slot: u32, accesses: u32, tile_bytes: u32, speedup: f32) -> PromotionCandidate {
        PromotionCandidate {
            binding_slot: slot,
            access_count: accesses,
            bytes_per_element: 4,
            distinct_indices_per_workgroup: tile_bytes / 4,
            tile_bytes,
            estimated_speedup_factor: speedup,
        }
    }

    #[test]
    fn empty_plan_fits_in_any_budget() {
        let p = PromotionPlan {
            kernel_id: "empty".into(),
            candidates: vec![],
            total_tile_bytes: 0,
            budget_bytes: 1024,
        };
        assert!(p.fits_in_budget());
    }

    #[test]
    fn single_candidate_below_budget_fits() {
        let p = PromotionPlan {
            kernel_id: "k".into(),
            candidates: vec![cand(0, 5, 1024, 7.0)],
            total_tile_bytes: 1024,
            budget_bytes: 4096,
        };
        assert!(p.fits_in_budget());
    }

    #[test]
    fn over_budget_does_not_fit() {
        let p = PromotionPlan {
            kernel_id: "k".into(),
            candidates: vec![cand(0, 5, 4096, 7.0), cand(1, 3, 4096, 5.0)],
            total_tile_bytes: 8192,
            budget_bytes: 4096,
        };
        assert!(!p.fits_in_budget());
    }

    #[test]
    fn sorted_by_speedup_orders_descending() {
        let p = PromotionPlan {
            kernel_id: "k".into(),
            candidates: vec![
                cand(0, 3, 256, 3.0),
                cand(1, 7, 256, 11.0),
                cand(2, 5, 256, 7.0),
            ],
            total_tile_bytes: 768,
            budget_bytes: 4096,
        };
        let sorted = p.sorted_by_speedup();
        assert_eq!(sorted[0].binding_slot, 1);
        assert_eq!(sorted[1].binding_slot, 2);
        assert_eq!(sorted[2].binding_slot, 0);
    }

    #[test]
    fn empty_sorted_is_empty() {
        let p = PromotionPlan {
            kernel_id: "k".into(),
            candidates: vec![],
            total_tile_bytes: 0,
            budget_bytes: 4096,
        };
        assert!(p.sorted_by_speedup().is_empty());
    }
}
