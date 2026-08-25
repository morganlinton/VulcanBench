//! CUDA adapter for benchmark-driven optimization pass selection.
//!
//! CUDA owns the pass ids and launch context. The pass-selection policy is
//! backend-neutral and lives in `vyre-driver`, so CUDA cannot silently fork the
//! evidence ranking, budget handling, or mandatory-pass semantics.

use vyre_driver::benchmark_pass_selection::{
    select_benchmark_passes, select_benchmark_passes_with_scratch, BenchmarkPassCandidate,
    BenchmarkPassSelectionError, BenchmarkPassSelectionPlan, BenchmarkPassSelectionSample,
    BenchmarkPassSelectionScratch, BenchmarkPassSkipReason, SkippedBenchmarkPass,
};

/// CUDA optimization candidate with benchmark-derived thresholds.
pub type CudaBenchmarkPassCandidate = BenchmarkPassCandidate;

/// Runtime benchmark sample used to select CUDA optimization passes.
pub type CudaBenchmarkPassSelectionSample = BenchmarkPassSelectionSample;

/// CUDA optimization pass skipped with a stable reason.
pub type CudaSkippedBenchmarkPass = SkippedBenchmarkPass;

/// Stable CUDA pass skip reason.
pub type CudaBenchmarkPassSkipReason = BenchmarkPassSkipReason;

/// CUDA pass-selection output.
pub type CudaBenchmarkPassSelectionPlan = BenchmarkPassSelectionPlan;

/// Caller-owned scratch for repeated CUDA benchmark pass selection.
pub type CudaBenchmarkPassSelectionScratch = BenchmarkPassSelectionScratch;

/// CUDA benchmark pass-selection error.
pub type CudaBenchmarkPassSelectionError = BenchmarkPassSelectionError;

/// Select CUDA optimization passes from benchmark evidence and workload stats.
///
/// # Errors
///
/// Returns [`CudaBenchmarkPassSelectionError`] when candidates are invalid,
/// budget accounting overflows, mandatory profitable passes cannot fit the
/// budget, or planner storage cannot be reserved.
pub fn select_cuda_benchmark_passes(
    candidates: &[CudaBenchmarkPassCandidate],
    sample: CudaBenchmarkPassSelectionSample,
) -> Result<CudaBenchmarkPassSelectionPlan, CudaBenchmarkPassSelectionError> {
    select_benchmark_passes(candidates, sample)
}

/// Select CUDA optimization passes using caller-owned temporary storage.
///
/// # Errors
///
/// Returns [`CudaBenchmarkPassSelectionError`] when candidates are invalid,
/// budget accounting overflows, mandatory profitable passes cannot fit the
/// budget, or planner storage cannot be reserved.
pub fn select_cuda_benchmark_passes_with_scratch(
    candidates: &[CudaBenchmarkPassCandidate],
    sample: CudaBenchmarkPassSelectionSample,
    scratch: &mut CudaBenchmarkPassSelectionScratch,
) -> Result<CudaBenchmarkPassSelectionPlan, CudaBenchmarkPassSelectionError> {
    select_benchmark_passes_with_scratch(candidates, sample, scratch)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    const CUDA_GRAPH_CAPTURE: &str = "cuda.graph_capture_replay";
    const CUDA_LAUNCH_FUSION: &str = "cuda.launch_fusion";
    const CUDA_COMPACTION: &str = "cuda.result_compaction";
    const CUDA_RESIDENCY: &str = "cuda.resident_graph_reuse";

    fn candidate(
        pass_id: &'static str,
        min_frontier_items: u64,
        min_reuse_count: u64,
        min_avoided_readback_bytes: u64,
        planning_cost_ns: u64,
        scratch_bytes: u64,
        expected_speedup_bps: u32,
        mandatory_when_profitable: bool,
    ) -> CudaBenchmarkPassCandidate {
        CudaBenchmarkPassCandidate {
            pass_id,
            min_frontier_items,
            min_reuse_count,
            min_avoided_readback_bytes,
            planning_cost_ns,
            scratch_bytes,
            expected_speedup_bps,
            mandatory_when_profitable,
        }
    }

    fn generous_sample() -> CudaBenchmarkPassSelectionSample {
        CudaBenchmarkPassSelectionSample {
            frontier_items: 1_000_000,
            reuse_count: 64,
            avoidable_readback_bytes: 128 * 1024 * 1024,
            planning_budget_ns: 1_000_000,
            scratch_budget_bytes: 64 * 1024 * 1024,
        }
    }

    #[test]
    fn cuda_benchmark_pass_selection_is_adapter_not_policy_fork() {
        let source = fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/benchmark_pass_selection.rs"
        ))
        .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - CUDA benchmark pass selection source should be readable");
        let local_value_helper = ["fn pass_", "value"].concat();
        let local_sort_policy = ["sort_unstable", "_by"].concat();
        assert!(source.contains("vyre_driver::benchmark_pass_selection"));
        assert!(!source.contains(&local_value_helper));
        assert!(!source.contains(&local_sort_policy));
    }

    #[test]
    fn cuda_benchmark_pass_selection_uses_shared_profitable_pass_ordering() {
        let candidates = [
            candidate(CUDA_LAUNCH_FUSION, 1, 1, 1, 20_000, 4096, 15_000, false),
            candidate(
                CUDA_GRAPH_CAPTURE,
                4096,
                8,
                64 * 1024,
                40_000,
                8192,
                20_000,
                false,
            ),
        ];
        let plan = select_cuda_benchmark_passes(&candidates, generous_sample()).unwrap();

        assert_eq!(
            plan.selected_pass_ids,
            vec![CUDA_GRAPH_CAPTURE, CUDA_LAUNCH_FUSION]
        );
        assert!(plan.skipped_passes.is_empty());
        assert_eq!(plan.total_planning_cost_ns, 60_000);
        assert_eq!(plan.total_scratch_bytes, 12_288);
        assert!(plan.projected_speedup_bps > 10_000);
    }

    #[test]
    fn cuda_benchmark_pass_selection_keeps_stable_skip_reasons() {
        let candidates = [
            candidate("cuda.frontier_threshold", 4096, 1, 1, 10, 10, 12_000, false),
            candidate("cuda.reuse_threshold", 1, 32, 1, 10, 10, 12_000, false),
            candidate("cuda.readback_threshold", 1, 1, 4096, 10, 10, 12_000, false),
        ];
        let sample = CudaBenchmarkPassSelectionSample {
            frontier_items: 128,
            reuse_count: 2,
            avoidable_readback_bytes: 128,
            planning_budget_ns: 1_000,
            scratch_budget_bytes: 1_000,
        };

        let plan = select_cuda_benchmark_passes(&candidates, sample).unwrap();
        assert!(plan.selected_pass_ids.is_empty());
        assert_eq!(plan.skipped_passes.len(), candidates.len());
        assert!(plan.skipped_passes.iter().any(|skipped| {
            skipped.pass_id == "cuda.frontier_threshold"
                && skipped.reason == CudaBenchmarkPassSkipReason::FrontierBelowThreshold
        }));
        assert!(plan.skipped_passes.iter().any(|skipped| {
            skipped.pass_id == "cuda.reuse_threshold"
                && skipped.reason == CudaBenchmarkPassSkipReason::ReuseBelowThreshold
        }));
        assert!(plan.skipped_passes.iter().any(|skipped| {
            skipped.pass_id == "cuda.readback_threshold"
                && skipped.reason == CudaBenchmarkPassSkipReason::ReadbackBelowThreshold
        }));
    }

    #[test]
    fn cuda_benchmark_pass_selection_refuses_to_starve_mandatory_cuda_passes() {
        let candidates = [candidate(
            CUDA_RESIDENCY,
            1,
            1,
            1,
            10_000,
            4096,
            18_000,
            true,
        )];
        let sample = CudaBenchmarkPassSelectionSample {
            planning_budget_ns: 9999,
            ..generous_sample()
        };

        let error = select_cuda_benchmark_passes(&candidates, sample).unwrap_err();
        assert_eq!(
            error,
            CudaBenchmarkPassSelectionError::MandatoryProfitablePassOverBudget {
                pass_id: CUDA_RESIDENCY,
                reason: CudaBenchmarkPassSkipReason::PlanningBudgetExceeded,
            }
        );
    }

    #[test]
    fn cuda_benchmark_pass_selection_reuses_shared_scratch() {
        let candidates = [
            candidate(CUDA_COMPACTION, 256, 2, 4096, 1000, 1024, 13_000, false),
            candidate(CUDA_RESIDENCY, 512, 8, 8192, 2000, 2048, 14_000, false),
            candidate(
                CUDA_LAUNCH_FUSION,
                1024,
                16,
                16_384,
                3000,
                4096,
                15_000,
                false,
            ),
        ];
        let mut scratch = CudaBenchmarkPassSelectionScratch::try_with_capacity(1).unwrap();

        let first =
            select_cuda_benchmark_passes_with_scratch(&candidates, generous_sample(), &mut scratch)
                .unwrap();
        let seen_capacity = scratch.seen_capacity();
        let ordered_capacity = scratch.ordered_index_capacity();
        let second = select_cuda_benchmark_passes_with_scratch(
            &candidates[..1],
            generous_sample(),
            &mut scratch,
        )
        .unwrap();

        assert_eq!(first.selected_pass_ids.len(), candidates.len());
        assert_eq!(second.selected_pass_ids, vec![CUDA_COMPACTION]);
        assert!(seen_capacity >= candidates.len());
        assert!(ordered_capacity >= candidates.len());
        assert_eq!(scratch.seen_capacity(), seen_capacity);
        assert_eq!(scratch.ordered_index_capacity(), ordered_capacity);
    }

    #[test]
    fn generated_cuda_benchmark_profiles_preserve_shared_budget_contracts() {
        const IDS: [&str; 8] = [
            "cuda.profile.pass0",
            "cuda.profile.pass1",
            "cuda.profile.pass2",
            "cuda.profile.pass3",
            "cuda.profile.pass4",
            "cuda.profile.pass5",
            "cuda.profile.pass6",
            "cuda.profile.pass7",
        ];

        for profile in 0_u64..64 {
            for budget_shape in 0_u64..16 {
                let candidates: Vec<_> = IDS
                    .iter()
                    .enumerate()
                    .map(|(index, pass_id)| {
                        let index = index as u64;
                        candidate(
                            pass_id,
                            64 + ((profile + index) % 17) * 32,
                            1 + ((profile + index) % 7),
                            128 + ((profile * 11 + index * 13) % 31) * 64,
                            100 + index * 37,
                            256 + index * 128,
                            11_000 + ((profile as u32 + index as u32) % 31) * 100,
                            false,
                        )
                    })
                    .collect();
                let sample = CudaBenchmarkPassSelectionSample {
                    frontier_items: 128 + profile * 32,
                    reuse_count: 1 + (profile % 8),
                    avoidable_readback_bytes: 512 + profile * 256,
                    planning_budget_ns: 150 + budget_shape * 150,
                    scratch_budget_bytes: 512 + budget_shape * 512,
                };

                let plan = select_cuda_benchmark_passes(&candidates, sample).unwrap();

                assert!(plan.total_planning_cost_ns <= sample.planning_budget_ns);
                assert!(plan.total_scratch_bytes <= sample.scratch_budget_bytes);
                assert_eq!(
                    plan.selected_pass_ids.len() + plan.skipped_passes.len(),
                    IDS.len()
                );
                assert!(plan
                    .selected_pass_ids
                    .iter()
                    .all(|selected| IDS.contains(selected)));
                assert!(plan
                    .skipped_passes
                    .iter()
                    .all(|skipped| IDS.contains(&skipped.pass_id)));
            }
        }
    }
}
