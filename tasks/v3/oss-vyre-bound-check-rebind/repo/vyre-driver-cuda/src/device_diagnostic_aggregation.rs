//! CUDA device-side diagnostic aggregation planning adapter.

use vyre_driver::device_diagnostic_aggregation::{
    plan_device_diagnostic_aggregation, plan_device_diagnostic_aggregation_with_scratch,
    DiagnosticAggregationError, DiagnosticAggregationPlan, DiagnosticAggregationScratch,
    DiagnosticCompactRange, DiagnosticShard,
};

/// One CUDA-resident diagnostic shard before aggregation.
pub type CudaDiagnosticShard = DiagnosticShard;

/// One compact CUDA diagnostic readback range.
pub type CudaDiagnosticCompactRange = DiagnosticCompactRange;

/// CUDA diagnostic aggregation plan.
pub type CudaDiagnosticAggregationPlan = DiagnosticAggregationPlan;

/// Caller-owned scratch for repeated CUDA diagnostic aggregation planning.
pub type CudaDiagnosticAggregationScratch = DiagnosticAggregationScratch;

/// CUDA diagnostic aggregation planning errors.
pub type CudaDiagnosticAggregationError = DiagnosticAggregationError;

/// Plan CUDA-side diagnostic aggregation and final-only compact readback.
pub fn plan_cuda_device_diagnostic_aggregation(
    shards: &[CudaDiagnosticShard],
    max_records_per_shard: u64,
    budget_bytes: u64,
) -> Result<CudaDiagnosticAggregationPlan, CudaDiagnosticAggregationError> {
    plan_device_diagnostic_aggregation(shards, max_records_per_shard, budget_bytes)
}

/// Plan CUDA-side diagnostic aggregation using caller-owned temporary storage.
pub fn plan_cuda_device_diagnostic_aggregation_with_scratch(
    shards: &[CudaDiagnosticShard],
    max_records_per_shard: u64,
    budget_bytes: u64,
    scratch: &mut CudaDiagnosticAggregationScratch,
) -> Result<CudaDiagnosticAggregationPlan, CudaDiagnosticAggregationError> {
    plan_device_diagnostic_aggregation_with_scratch(
        shards,
        max_records_per_shard,
        budget_bytes,
        scratch,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre_driver::device_diagnostic_aggregation::diagnostic_compression_ratio_bps;

    #[test]
    fn cuda_diagnostic_aggregation_is_adapter_not_policy_fork() {
        let source = include_str!("device_diagnostic_aggregation.rs");
        assert!(source.contains("pub type CudaDiagnosticShard = DiagnosticShard;"));
        assert!(
            source.contains("pub type CudaDiagnosticAggregationPlan = DiagnosticAggregationPlan;")
        );
        assert!(source.contains("plan_device_diagnostic_aggregation_with_scratch"));
        assert!(!source.contains(concat!("BTree", "Set")));
        assert!(!source.contains(concat!("shards", ".to_vec()")));
        assert!(!source.contains(concat!("CUDA", "_NUMERIC")));
        assert!(!source.contains(concat!("checked_mul_u64", "_count")));
        assert!(!source.contains(concat!("Vec::with_capacity", "(shards.len())")));
    }

    #[test]
    fn diagnostic_aggregation_compacts_sparse_device_diagnostics() {
        let plan = plan_cuda_device_diagnostic_aggregation(
            &[
                shard(2, 2_000, 4, 32, 24, 16, 0b010),
                shard(1, 1_000, 2, 32, 24, 16, 0b001),
                shard(3, 4_000, 0, 32, 24, 16, 0),
            ],
            64,
            1_024,
        )
        .expect("Fix: sparse diagnostics should aggregate on device");

        assert_eq!(
            plan.compact_ranges,
            vec![
                CudaDiagnosticCompactRange {
                    shard: 1,
                    compact_offset: 0,
                    records: 2,
                    bytes: 48,
                },
                CudaDiagnosticCompactRange {
                    shard: 2,
                    compact_offset: 48,
                    records: 4,
                    bytes: 96,
                },
            ]
        );
        assert_eq!(plan.counter_readback_bytes, 48);
        assert_eq!(plan.compact_readback_bytes, 144);
        assert_eq!(plan.host_readback_bytes, 192);
        assert_eq!(plan.raw_candidate_readback_bytes, 224_000);
        assert_eq!(plan.avoided_readback_bytes, 223_808);
        assert!(plan.compression_ratio_bps < 10);
        assert!(plan.requires_device_prefix_scan);
        assert!(plan.final_only_host_readback);
    }

    #[test]
    fn diagnostic_aggregation_caps_overflow_without_host_filtering() {
        let plan = plan_cuda_device_diagnostic_aggregation(
            &[shard(7, 1_000, 10, 32, 16, 8, 0b111)],
            3,
            128,
        )
        .expect("Fix: overflow should be represented by device-side flags");

        assert_eq!(plan.compact_ranges[0].records, 3);
        assert_eq!(plan.overflow_records, 7);
        assert!(plan.requires_overflow_flag);
        assert_eq!(plan.host_readback_bytes, 56);
        assert!(!plan.requires_device_prefix_scan);
    }

    #[test]
    fn diagnostic_aggregation_rejects_invalid_or_cpu_shaped_inputs() {
        assert_eq!(
            plan_cuda_device_diagnostic_aggregation(
                &[shard(1, 8, 1, 32, 24, 8, 1), shard(1, 8, 1, 32, 24, 8, 1)],
                4,
                1_024,
            )
            .expect_err("duplicate shard should fail"),
            CudaDiagnosticAggregationError::DuplicateShard { shard: 1 }
        );
        assert_eq!(
            plan_cuda_device_diagnostic_aggregation(&[shard(2, 8, 9, 32, 24, 8, 1)], 4, 1_024)
                .expect_err("emitted diagnostics cannot exceed candidates"),
            CudaDiagnosticAggregationError::EmittedExceedsCandidates {
                shard: 2,
                emitted_diagnostics: 9,
                candidate_items: 8,
            }
        );
        assert_eq!(
            plan_cuda_device_diagnostic_aggregation(&[shard(3, 8, 1, 32, 24, 8, 0)], 4, 1_024)
                .expect_err("diagnostics must retain class mask"),
            CudaDiagnosticAggregationError::MissingSeverityMask { shard: 3 }
        );
    }

    #[test]
    fn cuda_diagnostic_aggregation_reuses_shared_scratch() {
        let mut scratch = CudaDiagnosticAggregationScratch::try_with_capacity(128)
            .expect("Fix: scratch capacity");
        let wide = (0..128)
            .rev()
            .map(|index| shard(index, 1_024, 1, 32, 16, 8, 1))
            .collect::<Vec<_>>();
        let first =
            plan_cuda_device_diagnostic_aggregation_with_scratch(&wide, 4, 1 << 20, &mut scratch)
                .expect("Fix: wide diagnostic aggregation should plan with reusable scratch");
        let id_capacity = scratch.id_capacity();
        let ordered_index_capacity = scratch.ordered_index_capacity();

        assert_eq!(first.compact_ranges.len(), 128);
        assert_eq!(first.compact_ranges[0].shard, 0);

        let second = plan_cuda_device_diagnostic_aggregation_with_scratch(
            &[
                shard(9, 1_000, 0, 32, 24, 16, 0),
                shard(3, 1_000, 7, 32, 24, 16, 1),
            ],
            3,
            1 << 20,
            &mut scratch,
        )
        .expect("Fix: smaller diagnostic aggregation should reuse previous scratch");

        assert_eq!(second.compact_ranges[0].shard, 3);
        assert_eq!(second.overflow_records, 4);
        assert!(scratch.id_capacity() >= id_capacity);
        assert!(scratch.ordered_index_capacity() >= ordered_index_capacity);
    }

    #[test]
    fn cuda_diagnostic_aggregation_generated_profiles_preserve_shared_contracts() {
        let mut scratch = CudaDiagnosticAggregationScratch::default();
        for shard_count in 1u32..=64 {
            for cap in 1u64..=16 {
                let shards = (0..shard_count)
                    .rev()
                    .map(|id| {
                        let candidates = u64::from((id % 17) + 1) * 4;
                        let emitted = u64::from(id % 5);
                        shard(
                            id,
                            candidates,
                            emitted.min(candidates),
                            16,
                            12,
                            8,
                            if emitted == 0 { 0 } else { 1 << (id % 8) },
                        )
                    })
                    .collect::<Vec<_>>();

                let plan = plan_cuda_device_diagnostic_aggregation_with_scratch(
                    &shards,
                    cap,
                    u64::MAX,
                    &mut scratch,
                )
                .expect("Fix: generated CUDA diagnostic aggregation profile should plan");

                let expected_raw = shards
                    .iter()
                    .map(|shard| shard.candidate_items * shard.raw_item_bytes)
                    .sum::<u64>();
                let expected_counter = shards.iter().map(|shard| shard.counter_bytes).sum::<u64>();
                let expected_compact = shards
                    .iter()
                    .map(|shard| shard.emitted_diagnostics.min(cap) * shard.diagnostic_record_bytes)
                    .sum::<u64>();
                assert_eq!(plan.raw_candidate_readback_bytes, expected_raw);
                assert_eq!(plan.counter_readback_bytes, expected_counter);
                assert_eq!(plan.compact_readback_bytes, expected_compact);
                assert_eq!(
                    plan.host_readback_bytes,
                    expected_counter + expected_compact
                );
                assert!(plan
                    .compact_ranges
                    .windows(2)
                    .all(|pair| pair[0].shard < pair[1].shard));
                assert!(plan.final_only_host_readback);
            }
        }
    }

    #[test]
    fn diagnostic_aggregation_production_ratio_path_does_not_panic() {
        let source = include_str!("device_diagnostic_aggregation.rs");
        let production = source
            .split("#[cfg(test)]")
            .next()
            .expect("Fix: diagnostic aggregation source must contain production section");
        assert!(
            !production.contains(".expect(")
                && !production.contains(concat!("panic", "!("))
                && !production.contains(".unwrap_or_else("),
            "Fix: CUDA diagnostic aggregation production planning must return errors or bounded telemetry instead of panicking."
        );
        assert_eq!(
            diagnostic_compression_ratio_bps(u64::MAX, 1),
            u32::MAX,
            "Fix: diagnostic compression telemetry must remain bounded when a pathological ratio exceeds export width."
        );
    }

    fn shard(
        shard: u32,
        candidate_items: u64,
        emitted_diagnostics: u64,
        raw_item_bytes: u64,
        diagnostic_record_bytes: u64,
        counter_bytes: u64,
        severity_mask: u32,
    ) -> CudaDiagnosticShard {
        CudaDiagnosticShard {
            shard,
            candidate_items,
            emitted_diagnostics,
            raw_item_bytes,
            diagnostic_record_bytes,
            counter_bytes,
            severity_mask,
        }
    }
}
