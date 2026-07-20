//! C1 substrate: whole-megakernel optimization domain.
//!
//! Per-arm optimization (the existing CSE/DCE per arm, then fuse) is
//! conservative  -  it can't see structural redundancy ACROSS arms.
//! When two adjacent arms produce the same intermediate result the
//! first arm could compute it once and the second arm could just
//! read it.
//!
//! This substrate owns the *cross-arm redundancy detector*: given a
//! per-arm sequence of `MegakernelWorkItem`s, identify pairs of arms that
//! emit the same op→input→output triple. The dispatcher uses the
//! verdict to skip the redundant compute.
//!
//! Pure substrate  -  no Program walk, no allocation outside the
//! returned redundancy report. The actual rewrite (collapse
//! redundant arms into one + rewire downstream readers) is the
//! Codex-side runtime work; this substrate just names the
//! optimization opportunity.

use crate::{megakernel::planner::MegakernelWorkItem, PipelineError};
use rustc_hash::FxHashMap;
use vyre_foundation::allocation::{try_reserve_hash_map_to_capacity, try_reserve_vec_to_capacity};

const DENSE_OUTPUT_UNIQUE_BITS: usize = 4096;
const DENSE_OUTPUT_UNIQUE_WORDS: usize = DENSE_OUTPUT_UNIQUE_BITS / u64::BITS as usize;

/// Report of cross-arm redundancy in a megakernel arm sequence.
///
/// Each pair `(early, late)` means arm `late` emits a MegakernelWorkItem that
/// is structurally identical to one already emitted by arm `early`
/// (and that arm has not been overwritten since). The runtime can
/// drop the `late` arm's redundant op and rewire its readers to the
/// `early` arm's output handle.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CrossArmRedundancy {
    /// (early_arm_index, late_arm_index, redundant_op_index_in_late_arm).
    /// `early_arm_index < late_arm_index` always; the late arm is
    /// the one whose op is redundant.
    pub redundant_pairs: Vec<(usize, usize, usize)>,
    /// Total redundant ops detected across the whole sequence.
    /// Equal to `redundant_pairs.len()` but exposed separately so
    /// the dispatcher can budget telemetry without scanning the vec.
    pub total_redundant_ops: usize,
}

impl CrossArmRedundancy {
    /// Empty report  -  no redundancy across arms.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether this report names any opportunity.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.redundant_pairs.is_empty()
    }
}

/// Reusable scratch for same-batch work-item dedupe.
#[derive(Debug, Default)]
pub struct RedundantWorkItemPruneScratch {
    first_seen: FxHashMap<(u32, u32, u32, u32), usize>,
}

impl RedundantWorkItemPruneScratch {
    /// Clear retained hash state while preserving useful capacity.
    pub fn clear(&mut self) {
        self.first_seen.clear();
    }

    fn try_prepare_for_len(&mut self, len: usize) -> Result<(), PipelineError> {
        self.first_seen.clear();
        let retained_ceiling = len.checked_mul(4).unwrap_or(usize::MAX).max(1024);
        if self.first_seen.capacity() > retained_ceiling {
            self.first_seen.shrink_to(len);
        }
        if self.first_seen.capacity() < len {
            try_reserve_hash_map_to_capacity(&mut self.first_seen, len).map_err(|source| {
                PipelineError::Backend(format!(
                    "megakernel redundant-work hash reservation failed for {len} item(s): {source}. Fix: shard the work batch before pruning."
                ))
            })?;
        }
        Ok(())
    }
}

/// Walk `arms` and detect cross-arm structural redundancy.
///
/// For each (op_handle, input_handle, output_handle) triple the
/// substrate sees in arm N, it remembers which arm produced it. If
/// an identical triple appears in a later arm M > N, the substrate
/// records `(N, M, op_idx_in_M)`. WorkItems are compared by the
/// `(op_handle, input_handle, output_handle)` triple alone  -  the
/// `param` field is treated as separate launch metadata.
///
/// O(total_ops)  -  uses one pass + one hash table. Allocation only
/// for the redundancy report and the seen-set.
#[must_use]
#[cfg(any(test, feature = "legacy-infallible"))]
pub fn detect_cross_arm_redundancy(arms: &[&[MegakernelWorkItem]]) -> CrossArmRedundancy {
    try_detect_cross_arm_redundancy(arms).unwrap_or_else(|error| {
        panic!(
            "megakernel cross-arm redundancy detection allocation failed: {error}. Fix: split the fused arm sequence before planning."
        )
    })
}

/// Walk `arms` and detect cross-arm structural redundancy with fallible staging.
///
/// # Errors
///
/// Returns [`PipelineError::Backend`] when host hash/report storage cannot be
/// reserved for the fused arm sequence.
pub fn try_detect_cross_arm_redundancy(
    arms: &[&[MegakernelWorkItem]],
) -> Result<CrossArmRedundancy, PipelineError> {
    // (op_handle, input_handle, output_handle) → (arm_idx, op_idx)
    let total_ops = arms.iter().map(|arm| arm.len()).sum();
    let mut first_seen: FxHashMap<(u32, u32, u32), usize> = FxHashMap::default();
    reserve_hash_map(&mut first_seen, total_ops, "cross-arm first-seen")?;
    let mut report = CrossArmRedundancy {
        redundant_pairs: Vec::new(),
        total_redundant_ops: 0,
    };
    for (arm_idx, arm) in arms.iter().enumerate() {
        for (op_idx, item) in arm.iter().enumerate() {
            let key = (item.op_handle, item.input_handle, item.output_handle);
            match first_seen.get(&key) {
                Some(&early_arm_idx) if early_arm_idx < arm_idx => {
                    reserve_redundant_pairs(&mut report.redundant_pairs, 1, "cross-arm report")?;
                    report
                        .redundant_pairs
                        .push((early_arm_idx, arm_idx, op_idx));
                }
                Some(_) => {
                    // Same arm  -  not a cross-arm redundancy.
                }
                None => {
                    first_seen.insert(key, arm_idx);
                }
            }
        }
    }
    report.total_redundant_ops = report.redundant_pairs.len();
    Ok(report)
}

/// Copy `items` into `out`, dropping later work items that are byte-for-byte
/// redundant with an earlier item.
///
/// This is the runtime-safe rewrite for the opportunity named by
/// [`detect_cross_arm_redundancy`]. The detector intentionally ignores `param`
/// so it can flag broad structural reuse; the rewrite is stricter because
/// concrete megakernel publishers pass `param` as an opcode argument. A
/// duplicate `(op_handle, input_handle, output_handle, param)` writes the same
/// result slot from the same input through the same operation with the same
/// argument, so the later item only burns queue capacity and GPU cycles. The
/// first item is retained; all later duplicates are omitted from `out`.
///
/// When no duplicates are found, `out` is left empty so hot callers can keep
/// using the original borrowed queue without paying an avoidable copy.
///
#[cfg(any(test, feature = "legacy-infallible"))]
pub fn prune_redundant_work_items_into(
    items: &[MegakernelWorkItem],
    out: &mut Vec<MegakernelWorkItem>,
) -> CrossArmRedundancy {
    try_prune_redundant_work_items_into(items, out).unwrap_or_else(|error| {
        panic!(
            "megakernel redundant-work pruning allocation failed: {error}. Fix: shard the work batch before pruning."
        )
    })
}

/// Copy `items` into `out`, dropping later exact duplicates with fallible
/// staging.
///
/// # Errors
///
/// Returns [`PipelineError::Backend`] when host hash/report/output storage
/// cannot be reserved for the batch.
pub fn try_prune_redundant_work_items_into(
    items: &[MegakernelWorkItem],
    out: &mut Vec<MegakernelWorkItem>,
) -> Result<CrossArmRedundancy, PipelineError> {
    let mut scratch = RedundantWorkItemPruneScratch::default();
    try_prune_redundant_work_items_with_scratch_into(items, out, &mut scratch)
}

/// Copy `items` into `out`, dropping later exact duplicates while reusing the
/// caller-owned hash scratch across dispatches.
///
/// This is the hot megakernel-dispatch entry point. The legacy
/// [`prune_redundant_work_items_into`] wrapper remains for callers that do not
/// own persistent dispatch scratch.
#[cfg(any(test, feature = "legacy-infallible"))]
pub fn prune_redundant_work_items_with_scratch_into(
    items: &[MegakernelWorkItem],
    out: &mut Vec<MegakernelWorkItem>,
    scratch: &mut RedundantWorkItemPruneScratch,
) -> CrossArmRedundancy {
    try_prune_redundant_work_items_with_scratch_into(items, out, scratch).unwrap_or_else(|error| {
        panic!(
            "megakernel redundant-work pruning allocation failed: {error}. Fix: shard the work batch before pruning."
        )
    })
}

/// Copy `items` into `out`, dropping later exact duplicates while reusing
/// caller-owned hash scratch and fallible output/report staging.
///
/// # Errors
///
/// Returns [`PipelineError::Backend`] when host hash/report/output storage
/// cannot be reserved for the batch.
pub fn try_prune_redundant_work_items_with_scratch_into(
    items: &[MegakernelWorkItem],
    out: &mut Vec<MegakernelWorkItem>,
    scratch: &mut RedundantWorkItemPruneScratch,
) -> Result<CrossArmRedundancy, PipelineError> {
    out.clear();

    if output_handles_are_dense_unique(items) {
        scratch.clear();
        return Ok(CrossArmRedundancy::new());
    }

    scratch.try_prepare_for_len(items.len())?;
    let mut report = CrossArmRedundancy {
        redundant_pairs: Vec::new(),
        total_redundant_ops: 0,
    };
    let mut found_duplicate = false;

    for (idx, item) in items.iter().copied().enumerate() {
        let key = (
            item.op_handle,
            item.input_handle,
            item.output_handle,
            item.param,
        );
        if let Some(&early_idx) = scratch.first_seen.get(&key) {
            if !found_duplicate {
                reserve_work_items(out, items.len().checked_sub(1).unwrap_or(0), "dedup output")?;
                out.extend_from_slice(&items[..idx]);
                found_duplicate = true;
            }
            reserve_redundant_pairs(&mut report.redundant_pairs, 1, "dedup report")?;
            report.redundant_pairs.push((early_idx, idx, 0));
            continue;
        }
        scratch.first_seen.insert(key, idx);
        if found_duplicate {
            out.push(item);
        }
    }

    report.total_redundant_ops = report.redundant_pairs.len();
    Ok(report)
}

fn reserve_hash_map<K, V>(
    values: &mut FxHashMap<K, V>,
    additional: usize,
    label: &'static str,
) -> Result<(), PipelineError>
where
    K: Eq + std::hash::Hash,
{
    if additional > 0 {
        let capacity = values.len().checked_add(additional).ok_or_else(|| {
            PipelineError::Backend(format!(
                "megakernel {label} reservation overflowed for {additional} additional entry slot(s). Fix: shard the work batch before whole-megakernel optimization."
            ))
        })?;
        try_reserve_hash_map_to_capacity(values, capacity).map_err(|source| {
            PipelineError::Backend(format!(
                "megakernel {label} reservation failed for {additional} additional entry slot(s): {source}. Fix: shard the work batch before whole-megakernel optimization."
            ))
        })?;
    }
    Ok(())
}

fn reserve_redundant_pairs(
    values: &mut Vec<(usize, usize, usize)>,
    additional: usize,
    label: &'static str,
) -> Result<(), PipelineError> {
    values.try_reserve(additional).map_err(|source| {
        PipelineError::Backend(format!(
            "megakernel {label} reservation failed for {additional} additional pair slot(s): {source}. Fix: shard the work batch before whole-megakernel optimization."
        ))
    })
}

fn reserve_work_items(
    values: &mut Vec<MegakernelWorkItem>,
    capacity: usize,
    label: &'static str,
) -> Result<(), PipelineError> {
    if values.capacity() < capacity {
        try_reserve_vec_to_capacity(values, capacity).map_err(|source| {
            PipelineError::Backend(format!(
                "megakernel {label} reservation failed for {capacity} item slot(s): {source}. Fix: shard the work batch before whole-megakernel optimization."
            ))
        })?;
    }
    Ok(())
}

fn output_handles_are_dense_unique(items: &[MegakernelWorkItem]) -> bool {
    if items.len() <= 1 {
        return true;
    }
    if items.len() > DENSE_OUTPUT_UNIQUE_BITS {
        return false;
    }

    let mut min = u32::MAX;
    let mut max = 0u32;
    for item in items {
        min = min.min(item.output_handle);
        max = max.max(item.output_handle);
    }
    let Some(range) = u64::from(max)
        .checked_sub(u64::from(min))
        .and_then(|value| value.checked_add(1))
    else {
        return false;
    };
    if range > DENSE_OUTPUT_UNIQUE_BITS as u64 {
        return false;
    }

    let mut seen = [0u64; DENSE_OUTPUT_UNIQUE_WORDS];
    for item in items {
        let Some(delta) = item.output_handle.checked_sub(min) else {
            return false;
        };
        let Ok(offset) = usize::try_from(delta) else {
            return false;
        };
        let word = offset / 64;
        let bit = 1u64
            << (offset % 64);
        if (seen[word] & bit) != 0 {
            return false;
        }
        seen[word] |= bit;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(op: u32, inp: u32, out: u32) -> MegakernelWorkItem {
        MegakernelWorkItem {
            op_handle: op,
            input_handle: inp,
            output_handle: out,
            param: 0,
        }
    }

    #[test]
    fn empty_arms_have_no_redundancy() {
        let arms: [&[MegakernelWorkItem]; 0] = [];
        assert_eq!(
            detect_cross_arm_redundancy(&arms),
            CrossArmRedundancy::new()
        );
    }

    #[test]
    fn single_arm_with_repeats_has_no_cross_arm_redundancy() {
        let a = vec![item(1, 0, 5), item(1, 0, 5), item(2, 5, 6)];
        let arms: [&[MegakernelWorkItem]; 1] = [&a];
        let report = detect_cross_arm_redundancy(&arms);
        assert!(report.is_empty(), "intra-arm repeats are not cross-arm");
        assert_eq!(report.total_redundant_ops, 0);
    }

    #[test]
    fn identical_arms_report_full_overlap() {
        let a = vec![item(1, 0, 5), item(2, 5, 6)];
        let b = vec![item(1, 0, 5), item(2, 5, 6)];
        let arms: [&[MegakernelWorkItem]; 2] = [&a, &b];
        let report = detect_cross_arm_redundancy(&arms);
        assert_eq!(report.total_redundant_ops, 2);
        assert_eq!(report.redundant_pairs, vec![(0, 1, 0), (0, 1, 1)]);
    }

    #[test]
    fn fully_disjoint_arms_have_no_redundancy() {
        let a = vec![item(1, 0, 5)];
        let b = vec![item(2, 7, 8)];
        let arms: [&[MegakernelWorkItem]; 2] = [&a, &b];
        assert!(detect_cross_arm_redundancy(&arms).is_empty());
    }

    #[test]
    fn redundancy_uses_first_seen_arm_index() {
        // Op appears in arms 0, 2, 3  -  both 2 and 3 should reference 0.
        let a = vec![item(1, 0, 5)];
        let b = vec![item(99, 0, 0)];
        let c = vec![item(1, 0, 5)];
        let d = vec![item(1, 0, 5)];
        let arms: [&[MegakernelWorkItem]; 4] = [&a, &b, &c, &d];
        let report = detect_cross_arm_redundancy(&arms);
        assert_eq!(report.total_redundant_ops, 2);
        assert_eq!(report.redundant_pairs, vec![(0, 2, 0), (0, 3, 0)]);
    }

    #[test]
    fn param_field_does_not_affect_redundancy() {
        // Same (op, in, out) triple but different param  -  still
        // cross-arm redundant by this substrate's contract.
        let a = vec![MegakernelWorkItem {
            op_handle: 1,
            input_handle: 0,
            output_handle: 5,
            param: 7,
        }];
        let b = vec![MegakernelWorkItem {
            op_handle: 1,
            input_handle: 0,
            output_handle: 5,
            param: 99,
        }];

        let arms: [&[MegakernelWorkItem]; 2] = [&a, &b];
        let report = detect_cross_arm_redundancy(&arms);
        assert_eq!(report.total_redundant_ops, 1);
    }

    #[test]
    fn different_inputs_are_not_redundant() {
        let a = vec![item(1, 0, 5)];
        let b = vec![item(1, 1, 5)]; // different input handle
        let arms: [&[MegakernelWorkItem]; 2] = [&a, &b];
        assert!(detect_cross_arm_redundancy(&arms).is_empty());
    }

    #[test]
    fn different_outputs_are_not_redundant() {
        let a = vec![item(1, 0, 5)];
        let b = vec![item(1, 0, 6)]; // different output handle
        let arms: [&[MegakernelWorkItem]; 2] = [&a, &b];
        assert!(detect_cross_arm_redundancy(&arms).is_empty());
    }

    #[test]
    fn op_index_refers_to_late_arm_position() {
        // Verify the third tuple element is the index WITHIN the
        // late arm, not a global op index.
        let a = vec![item(1, 0, 5)];
        let b = vec![item(99, 0, 0), item(1, 0, 5), item(42, 0, 0)];
        let arms: [&[MegakernelWorkItem]; 2] = [&a, &b];
        let report = detect_cross_arm_redundancy(&arms);
        assert_eq!(report.redundant_pairs, vec![(0, 1, 1)]);
    }

    #[test]
    fn prune_redundant_work_items_drops_later_duplicates() {
        let items = vec![
            item(1, 0, 5),
            item(2, 5, 6),
            item(1, 0, 5),
            item(3, 6, 7),
            item(2, 5, 6),
        ];
        let mut out = Vec::new();

        let report = prune_redundant_work_items_into(&items, &mut out);

        assert_eq!(out, vec![item(1, 0, 5), item(2, 5, 6), item(3, 6, 7)]);
        assert_eq!(report.total_redundant_ops, 2);
        assert_eq!(report.redundant_pairs, vec![(0, 2, 0), (1, 4, 0)]);
    }

    #[test]
    fn prune_redundant_work_items_reuses_hash_scratch() {
        let items = vec![item(1, 0, 5), item(2, 5, 6), item(1, 0, 5), item(3, 6, 7)];
        let mut out = Vec::new();
        let mut scratch = RedundantWorkItemPruneScratch::default();

        let first = prune_redundant_work_items_with_scratch_into(&items, &mut out, &mut scratch);
        let retained_capacity = scratch.first_seen.capacity();
        out.clear();
        let second = prune_redundant_work_items_with_scratch_into(&items, &mut out, &mut scratch);

        assert_eq!(first, second);
        assert!(
            scratch.first_seen.capacity() >= retained_capacity,
            "hot megakernel dedupe must retain hash capacity across repeated dispatches"
        );
        assert_eq!(out, vec![item(1, 0, 5), item(2, 5, 6), item(3, 6, 7)]);
    }

    #[test]
    fn prune_redundant_work_items_handles_empty_input() {
        let mut out = vec![item(99, 99, 99)];

        let report = prune_redundant_work_items_into(&[], &mut out);

        assert!(report.is_empty());
        assert!(out.is_empty());
    }

    #[test]
    fn prune_redundant_work_items_all_duplicates_keep_one() {
        let items = vec![item(1, 0, 5), item(1, 0, 5), item(1, 0, 5)];
        let mut out = Vec::new();

        let report = prune_redundant_work_items_into(&items, &mut out);

        assert_eq!(out, vec![item(1, 0, 5)]);
        assert_eq!(report.total_redundant_ops, 2);
        assert_eq!(report.redundant_pairs, vec![(0, 1, 0), (0, 2, 0)]);
    }

    #[test]
    fn prune_redundant_work_items_preserves_order_after_first_duplicate() {
        let items = vec![
            item(1, 0, 5),
            item(2, 5, 6),
            item(1, 0, 5),
            item(3, 6, 7),
            item(4, 7, 8),
        ];
        let mut out = Vec::new();

        let report = prune_redundant_work_items_into(&items, &mut out);

        assert_eq!(
            out,
            vec![item(1, 0, 5), item(2, 5, 6), item(3, 6, 7), item(4, 7, 8)]
        );
        assert_eq!(report.redundant_pairs, vec![(0, 2, 0)]);
    }

    #[test]
    fn prune_redundant_work_items_leaves_output_empty_when_no_copy_needed() {
        let items = vec![item(1, 0, 5)];
        let mut out = vec![item(99, 99, 99)];

        let report = prune_redundant_work_items_into(&items, &mut out);

        assert!(report.is_empty());
        assert!(out.is_empty());
    }

    #[test]
    fn prune_redundant_work_items_keeps_distinct_params() {
        let mut a = item(1, 0, 5);
        a.param = 7;
        let mut b = item(1, 0, 5);
        b.param = 99;
        let items = vec![a, b];
        let mut out = Vec::new();

        let report = prune_redundant_work_items_into(&items, &mut out);

        assert!(report.is_empty());
        assert!(out.is_empty());
    }

    #[test]
    fn output_handles_dense_unique_accepts_single_owner_outputs() {
        let items = vec![item(1, 0, 5), item(1, 0, 6), item(1, 0, 7)];

        assert!(output_handles_are_dense_unique(&items));
    }

    #[test]
    fn output_handles_dense_unique_rejects_repeated_output() {
        let items = vec![item(1, 0, 5), item(2, 0, 5)];

        assert!(!output_handles_are_dense_unique(&items));
    }

    #[test]
    fn prune_redundant_work_items_still_catches_duplicate_with_repeated_output() {
        let items = vec![item(1, 0, 5), item(2, 0, 6), item(1, 0, 5)];
        let mut out = Vec::new();

        let report = prune_redundant_work_items_into(&items, &mut out);

        assert_eq!(report.total_redundant_ops, 1);
        assert_eq!(out, vec![item(1, 0, 5), item(2, 0, 6)]);
    }
}
