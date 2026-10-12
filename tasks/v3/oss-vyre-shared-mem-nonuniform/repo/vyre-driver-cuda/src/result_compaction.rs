//! CUDA compact result readback planning adapter.

use vyre_driver::result_compaction::{
    plan_result_compaction, plan_result_compaction_with_scratch, CompactResultRecord,
    ResultCompactionError, ResultCompactionPlan, ResultCompactionScratch, ResultSlot,
};

/// One CUDA output slot before result compaction.
pub type CudaResultSlot = ResultSlot;

/// One compact CUDA readback record.
pub type CudaCompactResultRecord = CompactResultRecord;

/// Compact CUDA result readback plan.
pub type CudaResultCompactionPlan = ResultCompactionPlan;

/// Caller-owned scratch for repeated CUDA result-compaction planning.
pub type CudaResultCompactionScratch = ResultCompactionScratch;

/// CUDA result compaction errors.
pub type CudaResultCompactionError = ResultCompactionError;

/// Plan compact readback for small CUDA outputs.
pub fn plan_cuda_result_compaction(
    slots: &[CudaResultSlot],
    max_compact_record_bytes: u64,
) -> Result<CudaResultCompactionPlan, CudaResultCompactionError> {
    plan_result_compaction(slots, max_compact_record_bytes)
}

/// Plan compact readback using caller-owned temporary storage.
pub fn plan_cuda_result_compaction_with_scratch(
    slots: &[CudaResultSlot],
    max_compact_record_bytes: u64,
    scratch: &mut CudaResultCompactionScratch,
) -> Result<CudaResultCompactionPlan, CudaResultCompactionError> {
    plan_result_compaction_with_scratch(slots, max_compact_record_bytes, scratch)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cuda_result_compaction_is_adapter_not_policy_fork() {
        let source = include_str!("result_compaction.rs");
        assert!(source.contains("pub type CudaResultSlot = ResultSlot;"));
        assert!(source.contains("pub type CudaResultCompactionPlan = ResultCompactionPlan;"));
        assert!(source.contains("plan_result_compaction_with_scratch"));
        assert!(!source.contains(concat!("CUDA", "_NUMERIC")));
        assert!(!source.contains(concat!("BTree", "Set")));
        assert!(!source.contains(concat!("saturating", "_sub")));
        assert!(!source.contains(concat!("Vec::with_capacity", "(slots.len())")));
    }

    #[test]
    fn cuda_result_compaction_adapter_preserves_small_output_contract() {
        let plan =
            plan_cuda_result_compaction(&[slot(2, 0, 128), slot(1, 12, 128), slot(3, 24, 256)], 32)
                .expect("Fix: small CUDA outputs should compact through shared planner");

        assert_eq!(
            plan.compact_records,
            vec![
                CudaCompactResultRecord {
                    slot: 1,
                    compact_offset: 0,
                    bytes: 12,
                },
                CudaCompactResultRecord {
                    slot: 3,
                    compact_offset: 12,
                    bytes: 24,
                },
            ]
        );
        assert_eq!(plan.direct_slots, Vec::<u32>::new());
        assert_eq!(plan.full_capacity_bytes, 512);
        assert_eq!(plan.selected_readback_bytes, 36);
        assert_eq!(plan.avoided_readback_basis_points, 9_296);
    }

    #[test]
    fn cuda_result_compaction_adapter_reuses_shared_scratch() {
        let mut scratch =
            CudaResultCompactionScratch::try_with_capacity(96).expect("Fix: scratch capacity");
        let wide = (0..96)
            .rev()
            .map(|index| slot(index, 8, 64))
            .collect::<Vec<_>>();
        let first = plan_cuda_result_compaction_with_scratch(&wide, 16, &mut scratch)
            .expect("Fix: wide compact result set should plan with reusable scratch");
        let id_capacity = scratch.id_capacity();
        let ordered_index_capacity = scratch.ordered_index_capacity();

        assert_eq!(first.compact_records.len(), 96);
        assert_eq!(first.compact_records[0].slot, 0);

        let second = plan_cuda_result_compaction_with_scratch(
            &[slot(7, 0, 128), slot(3, 512, 1_024), slot(5, 16, 128)],
            32,
            &mut scratch,
        )
        .expect("Fix: smaller mixed result set should reuse previous scratch");

        assert_eq!(second.compact_records[0].slot, 5);
        assert_eq!(second.direct_slots, vec![3]);
        assert!(scratch.id_capacity() >= id_capacity);
        assert!(scratch.ordered_index_capacity() >= ordered_index_capacity);
    }

    #[test]
    fn cuda_result_compaction_adapter_preserves_generated_telemetry_contracts() {
        let mut scratch = CudaResultCompactionScratch::default();
        for slot_count in 1u32..=64 {
            for compact_threshold in 0u64..16 {
                let slots = (0..slot_count)
                    .rev()
                    .map(|slot_id| {
                        let meaningful = u64::from((slot_id % 13) + 1);
                        CudaResultSlot {
                            slot: slot_id,
                            meaningful_bytes: meaningful,
                            capacity_bytes: meaningful + compact_threshold + 4,
                        }
                    })
                    .collect::<Vec<_>>();

                let plan = plan_cuda_result_compaction_with_scratch(
                    &slots,
                    compact_threshold,
                    &mut scratch,
                )
                .expect("Fix: generated CUDA result compaction profile should plan");

                let expected_full = slots.iter().map(|slot| slot.capacity_bytes).sum::<u64>();
                let expected_selected = slots.iter().map(|slot| slot.meaningful_bytes).sum::<u64>();
                assert_eq!(plan.full_capacity_bytes, expected_full);
                assert_eq!(plan.selected_readback_bytes, expected_selected);
                assert_eq!(
                    plan.avoided_readback_bytes,
                    expected_full - expected_selected
                );
            }
        }
    }

    fn slot(slot: u32, meaningful_bytes: u64, capacity_bytes: u64) -> CudaResultSlot {
        CudaResultSlot {
            slot,
            meaningful_bytes,
            capacity_bytes,
        }
    }
}
