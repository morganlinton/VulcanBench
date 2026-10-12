//! Benchmark-driven optimization pass selection.
//!
//! Expensive passes must not fire because a static list says so. They need
//! graph/frontier/reuse evidence showing that the launch, memory, or readback
//! cost they remove is larger than their own planning cost. This module makes
//! that decision explicit and deterministic.

use crate::accounting::{
    checked_add_u64_count as checked_add, checked_add_usize_count as checked_add_usize,
    ArithmeticOverflow,
};
use crate::numeric::checked_compose_basis_points_u64;
use crate::reservation_policy::{
    reserved_typed_vec as reserved_vec, ReservationPolicy, ReusableIndexScratch,
};

const BENCHMARK_PASS_SELECTION_RESERVATION: ReservationPolicy = ReservationPolicy::new(
    "benchmark pass selection",
    "shard the optimization candidate set before pass selection",
);

/// One optimization candidate with benchmark-derived thresholds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BenchmarkPassCandidate {
    /// Registered optimization pass id.
    pub pass_id: &'static str,
    /// Minimum active frontier items required before this pass is profitable.
    pub min_frontier_items: u64,
    /// Minimum repeated graph executions required before this pass is profitable.
    pub min_reuse_count: u64,
    /// Minimum readback bytes avoided before this pass is profitable.
    pub min_avoided_readback_bytes: u64,
    /// Estimated planning/compile cost in nanoseconds.
    pub planning_cost_ns: u64,
    /// Scratch bytes needed by the pass while planning/executing.
    pub scratch_bytes: u64,
    /// Expected speedup in basis points from committed benchmark evidence.
    pub expected_speedup_bps: u32,
    /// Whether the pass is mandatory when its thresholds are met.
    pub mandatory_when_profitable: bool,
}

/// Runtime benchmark sample used to select optimization passes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BenchmarkPassSelectionSample {
    /// Active frontier items in the current graph/query batch.
    pub frontier_items: u64,
    /// Number of repeated executions over the same resident graph shape.
    pub reuse_count: u64,
    /// Readback bytes the workload can avoid with compaction/aggregation.
    pub avoidable_readback_bytes: u64,
    /// Maximum total planning cost allowed.
    pub planning_budget_ns: u64,
    /// Maximum scratch bytes allowed for selected passes.
    pub scratch_budget_bytes: u64,
}

/// One skipped optimization with a stable reason.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SkippedBenchmarkPass {
    /// Registered optimization pass id.
    pub pass_id: &'static str,
    /// Stable reason.
    pub reason: BenchmarkPassSkipReason,
}

/// Stable skip reason for an optimization candidate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BenchmarkPassSkipReason {
    /// Frontier is too small for this pass to pay for itself.
    FrontierBelowThreshold,
    /// Graph reuse is too low for residency/cache/fusion work to amortize.
    ReuseBelowThreshold,
    /// Readback pressure is too low for compaction/aggregation to pay off.
    ReadbackBelowThreshold,
    /// Planning budget would be exceeded.
    PlanningBudgetExceeded,
    /// Scratch budget would be exceeded.
    ScratchBudgetExceeded,
}

/// Pass-selection output.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BenchmarkPassSelectionPlan {
    /// Selected pass ids in benchmark-value order.
    pub selected_pass_ids: Vec<&'static str>,
    /// Skipped pass ids with stable reasons.
    pub skipped_passes: Vec<SkippedBenchmarkPass>,
    /// Total selected planning cost.
    pub total_planning_cost_ns: u64,
    /// Total selected scratch bytes.
    pub total_scratch_bytes: u64,
    /// Product of selected speedup multipliers in basis points.
    pub projected_speedup_bps: u64,
}

/// Caller-owned scratch for repeated benchmark pass selection.
#[derive(Debug, Default)]
pub struct BenchmarkPassSelectionScratch {
    index_scratch: ReusableIndexScratch<&'static str>,
}

impl BenchmarkPassSelectionScratch {
    /// Allocate empty reusable pass-selection scratch.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Allocate reusable pass-selection scratch for a known candidate count.
    ///
    /// # Errors
    ///
    /// Returns [`BenchmarkPassSelectionError`] when scratch storage cannot be reserved.
    pub fn try_with_capacity(candidate_count: usize) -> Result<Self, BenchmarkPassSelectionError> {
        let mut scratch = Self::default();
        scratch.try_reserve_candidates(candidate_count)?;
        Ok(scratch)
    }

    /// Reserve reusable pass-selection scratch for a known candidate count.
    ///
    /// # Errors
    ///
    /// Returns [`BenchmarkPassSelectionError`] when scratch storage cannot be reserved.
    pub fn try_reserve_candidates(
        &mut self,
        candidate_count: usize,
    ) -> Result<(), BenchmarkPassSelectionError> {
        self.index_scratch.try_reserve_with(
            BENCHMARK_PASS_SELECTION_RESERVATION,
            candidate_count,
            "scratch.seen",
            "scratch.ordered_indices",
            storage_reserve_failed,
        )
    }

    /// Retained duplicate-detection capacity.
    #[must_use]
    pub fn seen_capacity(&self) -> usize {
        self.index_scratch.seen_capacity()
    }

    /// Retained candidate-ordering capacity.
    #[must_use]
    pub fn ordered_index_capacity(&self) -> usize {
        self.index_scratch.ordered_index_capacity()
    }
}

/// Benchmark-driven pass-selection errors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BenchmarkPassSelectionError {
    /// Candidate pass id is empty.
    EmptyPassId,
    /// Duplicate candidate pass id.
    DuplicatePassId {
        /// Duplicate pass id.
        pass_id: &'static str,
    },
    /// Candidate has no benchmark speedup evidence.
    MissingSpeedupEvidence {
        /// Invalid pass id.
        pass_id: &'static str,
    },
    /// Mandatory profitable pass could not fit the explicit budgets.
    MandatoryProfitablePassOverBudget {
        /// Pass id.
        pass_id: &'static str,
        /// Reason it could not fit.
        reason: BenchmarkPassSkipReason,
    },
    /// Arithmetic overflowed.
    CountOverflow {
        /// Field being computed.
        field: &'static str,
    },
    /// Scratch or result-vector storage reservation failed before pass selection.
    StorageReserveFailed {
        /// Field being reserved.
        field: &'static str,
        /// Requested total capacity.
        requested: usize,
        /// Allocator failure details.
        message: String,
    },
}

impl ArithmeticOverflow for BenchmarkPassSelectionError {
    fn arithmetic_overflow(field: &'static str) -> Self {
        Self::CountOverflow { field }
    }
}

impl std::fmt::Display for BenchmarkPassSelectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyPassId => write!(
                f,
                "benchmark pass selection received an empty pass id. Fix: register every pass before selection."
            ),
            Self::DuplicatePassId { pass_id } => write!(
                f,
                "benchmark pass selection received duplicate pass `{pass_id}`. Fix: keep one benchmark row per pass."
            ),
            Self::MissingSpeedupEvidence { pass_id } => write!(
                f,
                "benchmark pass `{pass_id}` has no positive speedup evidence. Fix: add committed benchmark evidence or remove the candidate."
            ),
            Self::MandatoryProfitablePassOverBudget { pass_id, reason } => write!(
                f,
                "mandatory profitable pass `{pass_id}` was blocked by {reason:?}. Fix: raise the explicit budget or shard before pass selection."
            ),
            Self::CountOverflow { field } => write!(
                f,
                "benchmark pass selection overflowed while computing {field}. Fix: shard the optimization candidate set."
            ),
            Self::StorageReserveFailed {
                field,
                requested,
                message,
            } => write!(
                f,
                "benchmark pass selection failed to reserve {field} for {requested} entries: {message}. Fix: shard the optimization candidate set before pass selection."
            ),
        }
    }
}

impl std::error::Error for BenchmarkPassSelectionError {}

/// Select optimization passes from benchmark evidence and workload stats.
///
/// # Errors
///
/// Returns [`BenchmarkPassSelectionError`] when candidates are invalid, budget
/// accounting overflows, mandatory profitable passes cannot fit the budget, or
/// planner storage cannot be reserved.
pub fn select_benchmark_passes(
    candidates: &[BenchmarkPassCandidate],
    sample: BenchmarkPassSelectionSample,
) -> Result<BenchmarkPassSelectionPlan, BenchmarkPassSelectionError> {
    let mut scratch = BenchmarkPassSelectionScratch::try_with_capacity(candidates.len())?;
    select_benchmark_passes_with_scratch(candidates, sample, &mut scratch)
}

/// Select optimization passes using caller-owned temporary storage.
///
/// # Errors
///
/// Returns [`BenchmarkPassSelectionError`] when candidates are invalid, budget
/// accounting overflows, mandatory profitable passes cannot fit the budget, or
/// planner storage cannot be reserved.
pub fn select_benchmark_passes_with_scratch(
    candidates: &[BenchmarkPassCandidate],
    sample: BenchmarkPassSelectionSample,
    scratch: &mut BenchmarkPassSelectionScratch,
) -> Result<BenchmarkPassSelectionPlan, BenchmarkPassSelectionError> {
    scratch.index_scratch.clear();
    scratch.try_reserve_candidates(candidates.len())?;
    for (index, candidate) in candidates.iter().enumerate() {
        if candidate.pass_id.is_empty() {
            return Err(BenchmarkPassSelectionError::EmptyPassId);
        }
        if !scratch.index_scratch.insert_seen(candidate.pass_id) {
            return Err(BenchmarkPassSelectionError::DuplicatePassId {
                pass_id: candidate.pass_id,
            });
        }
        if candidate.expected_speedup_bps <= 10_000 {
            return Err(BenchmarkPassSelectionError::MissingSpeedupEvidence {
                pass_id: candidate.pass_id,
            });
        }
        scratch.index_scratch.push_index(index);
    }
    scratch
        .index_scratch
        .ordered_indices_mut()
        .sort_unstable_by(|&left, &right| {
            candidates[right]
                .mandatory_when_profitable
                .cmp(&candidates[left].mandatory_when_profitable)
                .then_with(|| {
                    pass_value(&candidates[right])
                        .cmp(&pass_value(&candidates[left]))
                        .then_with(|| candidates[left].pass_id.cmp(candidates[right].pass_id))
                })
        });

    let (selected_pass_capacity, skipped_pass_capacity) =
        count_final_pass_buckets(candidates, sample, scratch.index_scratch.ordered_indices())?;
    let mut selected_pass_ids =
        reserved_selection_vec(selected_pass_capacity, "selected_pass_ids")?;
    let mut skipped_passes = reserved_selection_vec(skipped_pass_capacity, "skipped_passes")?;
    let mut total_planning_cost_ns = 0_u64;
    let mut total_scratch_bytes = 0_u64;
    let mut projected_speedup_bps = 10_000_u64;

    for &index in scratch.index_scratch.ordered_indices() {
        let candidate = candidates[index];
        if sample.frontier_items < candidate.min_frontier_items {
            skipped_passes.push(skipped(
                candidate.pass_id,
                BenchmarkPassSkipReason::FrontierBelowThreshold,
            ));
            continue;
        }
        if sample.reuse_count < candidate.min_reuse_count {
            skipped_passes.push(skipped(
                candidate.pass_id,
                BenchmarkPassSkipReason::ReuseBelowThreshold,
            ));
            continue;
        }
        if sample.avoidable_readback_bytes < candidate.min_avoided_readback_bytes {
            skipped_passes.push(skipped(
                candidate.pass_id,
                BenchmarkPassSkipReason::ReadbackBelowThreshold,
            ));
            continue;
        }

        let next_planning = checked_add(
            total_planning_cost_ns,
            candidate.planning_cost_ns,
            "planning cost",
        )?;
        if next_planning > sample.planning_budget_ns {
            handle_budget_skip(
                candidate,
                BenchmarkPassSkipReason::PlanningBudgetExceeded,
                &mut skipped_passes,
            )?;
            continue;
        }
        let next_scratch = checked_add(
            total_scratch_bytes,
            candidate.scratch_bytes,
            "scratch bytes",
        )?;
        if next_scratch > sample.scratch_budget_bytes {
            handle_budget_skip(
                candidate,
                BenchmarkPassSkipReason::ScratchBudgetExceeded,
                &mut skipped_passes,
            )?;
            continue;
        }

        selected_pass_ids.push(candidate.pass_id);
        total_planning_cost_ns = next_planning;
        total_scratch_bytes = next_scratch;
        projected_speedup_bps = checked_compose_basis_points_u64(
            projected_speedup_bps,
            u64::from(candidate.expected_speedup_bps),
        )
        .ok_or(BenchmarkPassSelectionError::CountOverflow {
            field: "projected speedup product",
        })?;
    }

    Ok(BenchmarkPassSelectionPlan {
        selected_pass_ids,
        skipped_passes,
        total_planning_cost_ns,
        total_scratch_bytes,
        projected_speedup_bps,
    })
}

fn pass_value(candidate: &BenchmarkPassCandidate) -> u128 {
    u128::from(candidate.expected_speedup_bps)
        * (u128::from(candidate.min_frontier_items)
            + u128::from(candidate.min_reuse_count)
            + u128::from(candidate.min_avoided_readback_bytes))
}

fn count_final_pass_buckets(
    candidates: &[BenchmarkPassCandidate],
    sample: BenchmarkPassSelectionSample,
    ordered_indices: &[usize],
) -> Result<(usize, usize), BenchmarkPassSelectionError> {
    let mut selected = 0usize;
    let mut skipped = 0usize;
    let mut total_planning_cost_ns = 0_u64;
    let mut total_scratch_bytes = 0_u64;
    for &index in ordered_indices {
        let candidate = candidates[index];
        if sample.frontier_items < candidate.min_frontier_items
            || sample.reuse_count < candidate.min_reuse_count
            || sample.avoidable_readback_bytes < candidate.min_avoided_readback_bytes
        {
            skipped = checked_add_usize(skipped, 1, "skipped pass count")?;
            continue;
        }
        let next_planning = checked_add(
            total_planning_cost_ns,
            candidate.planning_cost_ns,
            "planning cost",
        )?;
        if next_planning > sample.planning_budget_ns {
            if candidate.mandatory_when_profitable {
                return Err(
                    BenchmarkPassSelectionError::MandatoryProfitablePassOverBudget {
                        pass_id: candidate.pass_id,
                        reason: BenchmarkPassSkipReason::PlanningBudgetExceeded,
                    },
                );
            }
            skipped = checked_add_usize(skipped, 1, "skipped pass count")?;
            continue;
        }
        let next_scratch = checked_add(
            total_scratch_bytes,
            candidate.scratch_bytes,
            "scratch bytes",
        )?;
        if next_scratch > sample.scratch_budget_bytes {
            if candidate.mandatory_when_profitable {
                return Err(
                    BenchmarkPassSelectionError::MandatoryProfitablePassOverBudget {
                        pass_id: candidate.pass_id,
                        reason: BenchmarkPassSkipReason::ScratchBudgetExceeded,
                    },
                );
            }
            skipped = checked_add_usize(skipped, 1, "skipped pass count")?;
            continue;
        }
        selected = checked_add_usize(selected, 1, "selected pass count")?;
        total_planning_cost_ns = next_planning;
        total_scratch_bytes = next_scratch;
    }
    Ok((selected, skipped))
}

fn skipped(pass_id: &'static str, reason: BenchmarkPassSkipReason) -> SkippedBenchmarkPass {
    SkippedBenchmarkPass { pass_id, reason }
}

fn handle_budget_skip(
    candidate: BenchmarkPassCandidate,
    reason: BenchmarkPassSkipReason,
    skipped_passes: &mut Vec<SkippedBenchmarkPass>,
) -> Result<(), BenchmarkPassSelectionError> {
    if candidate.mandatory_when_profitable {
        return Err(
            BenchmarkPassSelectionError::MandatoryProfitablePassOverBudget {
                pass_id: candidate.pass_id,
                reason,
            },
        );
    }
    skipped_passes.push(skipped(candidate.pass_id, reason));
    Ok(())
}

fn reserved_selection_vec<T>(
    capacity: usize,
    field: &'static str,
) -> Result<Vec<T>, BenchmarkPassSelectionError> {
    reserved_vec(
        BENCHMARK_PASS_SELECTION_RESERVATION,
        capacity,
        field,
        storage_reserve_failed,
    )
}

fn storage_reserve_failed(
    field: &'static str,
    requested: usize,
    message: String,
) -> BenchmarkPassSelectionError {
    BenchmarkPassSelectionError::StorageReserveFailed {
        field,
        requested,
        message,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn benchmark_pass_selection_picks_profitable_passes_by_value() {
        let plan = select_benchmark_passes(
            &[
                candidate(
                    "device.adjacent-launch-fusion",
                    1_000,
                    4,
                    0,
                    100,
                    64,
                    18_000,
                    true,
                ),
                candidate(
                    "device.result-compaction",
                    1,
                    1,
                    4_096,
                    20,
                    16,
                    12_000,
                    false,
                ),
                candidate(
                    "device.megakernel-plan-cache",
                    1,
                    64,
                    0,
                    50,
                    32,
                    25_000,
                    true,
                ),
            ],
            BenchmarkPassSelectionSample {
                frontier_items: 2_000,
                reuse_count: 128,
                avoidable_readback_bytes: 8_192,
                planning_budget_ns: 200,
                scratch_budget_bytes: 128,
            },
        )
        .expect("Fix: profitable passes should select");

        assert_eq!(plan.selected_pass_ids.len(), 3);
        assert!(plan
            .selected_pass_ids
            .contains(&"device.megakernel-plan-cache"));
        assert!(plan
            .selected_pass_ids
            .contains(&"device.adjacent-launch-fusion"));
        assert!(plan.selected_pass_ids.contains(&"device.result-compaction"));
        assert_eq!(plan.total_planning_cost_ns, 170);
        assert_eq!(plan.total_scratch_bytes, 112);
        assert!(plan.projected_speedup_bps > 50_000);
    }

    #[test]
    fn benchmark_pass_selection_skips_unprofitable_passes_with_stable_reasons() {
        let plan = select_benchmark_passes(
            &[
                candidate(
                    "device.adjacent-launch-fusion",
                    1_000,
                    4,
                    0,
                    10,
                    8,
                    15_000,
                    false,
                ),
                candidate(
                    "device.result-compaction",
                    1,
                    1,
                    4_096,
                    10,
                    8,
                    11_000,
                    false,
                ),
            ],
            BenchmarkPassSelectionSample {
                frontier_items: 10,
                reuse_count: 1,
                avoidable_readback_bytes: 128,
                planning_budget_ns: 100,
                scratch_budget_bytes: 100,
            },
        )
        .expect("Fix: unprofitable optional passes should skip");

        assert_eq!(plan.selected_pass_ids, Vec::<&'static str>::new());
        assert_eq!(plan.skipped_passes.len(), 2);
        assert!(plan.skipped_passes.contains(&SkippedBenchmarkPass {
            pass_id: "device.adjacent-launch-fusion",
            reason: BenchmarkPassSkipReason::FrontierBelowThreshold,
        }));
        assert!(plan.skipped_passes.contains(&SkippedBenchmarkPass {
            pass_id: "device.result-compaction",
            reason: BenchmarkPassSkipReason::ReadbackBelowThreshold,
        }));
    }

    #[test]
    fn benchmark_pass_selection_ranks_huge_values_without_saturation_ties() {
        let plan = select_benchmark_passes(
            &[
                candidate(
                    "device.a-lexicographic-low-value",
                    u64::MAX,
                    u64::MAX,
                    u64::MAX - 1,
                    1,
                    1,
                    11_000,
                    false,
                ),
                candidate(
                    "device.z-lexicographic-high-value",
                    u64::MAX,
                    u64::MAX,
                    u64::MAX,
                    1,
                    1,
                    11_000,
                    false,
                ),
            ],
            BenchmarkPassSelectionSample {
                frontier_items: u64::MAX,
                reuse_count: u64::MAX,
                avoidable_readback_bytes: u64::MAX,
                planning_budget_ns: 10,
                scratch_budget_bytes: 10,
            },
        )
        .expect("Fix: huge benchmark evidence should rank without saturating value ties");

        assert_eq!(
            plan.selected_pass_ids[0],
            "device.z-lexicographic-high-value",
            "Fix: pass ranking must use widened arithmetic; saturating u64 scoring would tie these candidates and incorrectly choose lexicographic order."
        );
    }

    #[test]
    fn benchmark_pass_selection_rejects_missing_evidence_and_blocked_mandatory() {
        assert_eq!(
            select_benchmark_passes(
                &[candidate("device.bad", 1, 1, 0, 1, 1, 10_000, false)],
                sample(),
            )
            .expect_err("zero speedup evidence should fail"),
            BenchmarkPassSelectionError::MissingSpeedupEvidence {
                pass_id: "device.bad",
            }
        );
        assert_eq!(
            select_benchmark_passes(
                &[candidate("device.mandatory", 1, 1, 0, 101, 1, 11_000, true,)],
                sample(),
            )
            .expect_err("mandatory profitable pass cannot exceed budget"),
            BenchmarkPassSelectionError::MandatoryProfitablePassOverBudget {
                pass_id: "device.mandatory",
                reason: BenchmarkPassSkipReason::PlanningBudgetExceeded,
            }
        );
    }

    #[test]
    fn benchmark_pass_selection_does_not_let_optional_passes_starve_mandatory_passes() {
        let plan = select_benchmark_passes(
            &[
                candidate(
                    "device.optional-high-value",
                    1,
                    1,
                    1_000_000,
                    100,
                    1,
                    20_000,
                    false,
                ),
                candidate("device.mandatory-low-value", 1, 1, 1, 100, 1, 11_000, true),
            ],
            BenchmarkPassSelectionSample {
                frontier_items: 1,
                reuse_count: 1,
                avoidable_readback_bytes: 1_000_000,
                planning_budget_ns: 100,
                scratch_budget_bytes: 8,
            },
        )
        .expect("Fix: mandatory profitable pass must reserve budget before optional passes");

        assert_eq!(plan.selected_pass_ids, vec!["device.mandatory-low-value"]);
        assert_eq!(
            plan.skipped_passes,
            vec![SkippedBenchmarkPass {
                pass_id: "device.optional-high-value",
                reason: BenchmarkPassSkipReason::PlanningBudgetExceeded,
            }]
        );
    }

    #[test]
    fn benchmark_pass_selection_avoids_tree_sets_and_candidate_vector_copies() {
        let src = include_str!("benchmark_pass_selection.rs");
        assert!(
            !src.contains(concat!("BTree", "Set")),
            "Fix: benchmark pass selection should hash pass ids and sort candidate indices by value."
        );
        assert!(
            !src.contains(concat!("candidates", ".to_vec()")),
            "Fix: benchmark pass selection should not copy all candidates before value ordering."
        );
        assert!(
            src.contains("BenchmarkPassSelectionScratch::try_with_capacity(candidates.len())?"),
            "Fix: benchmark pass selection must stage scratch with fallible release-path allocation."
        );
        assert!(
            src.contains("scratch.try_reserve_candidates(candidates.len())?"),
            "Fix: caller-owned benchmark pass-selection scratch must grow through fallible reservation."
        );
        assert!(
            src.contains("ReusableIndexScratch"),
            "Fix: benchmark pass-selection duplicate detection and ordering scratch must share the paired typed fallible reservation helper."
        );
        assert!(
            src.contains("StorageReserveFailed"),
            "Fix: benchmark pass-selection allocation failures must surface as actionable planning errors."
        );
        assert!(
            !src.contains(concat!("FxHashSet::with_capacity", "_and_hasher")),
            "Fix: benchmark pass-selection scratch hash storage must not allocate infallibly."
        );
        assert!(
            !src.contains(concat!("Vec::with_capacity", "(candidate_count)"))
                && !src.contains(concat!("Vec::with_capacity", "(candidates.len())")),
            "Fix: benchmark pass-selection scratch/result vectors must not allocate infallibly."
        );
    }

    #[test]
    fn benchmark_pass_selection_reuses_caller_owned_candidate_scratch() {
        let mut scratch =
            BenchmarkPassSelectionScratch::try_with_capacity(64).expect("Fix: scratch capacity");
        let names = [
            "device.synthetic.00",
            "device.synthetic.01",
            "device.synthetic.02",
            "device.synthetic.03",
            "device.synthetic.04",
            "device.synthetic.05",
            "device.synthetic.06",
            "device.synthetic.07",
            "device.synthetic.08",
            "device.synthetic.09",
            "device.synthetic.10",
            "device.synthetic.11",
            "device.synthetic.12",
            "device.synthetic.13",
            "device.synthetic.14",
            "device.synthetic.15",
        ];
        let mut wide = Vec::new();
        wide.try_reserve_exact(names.len())
            .expect("Fix: synthetic pass vector capacity");
        for (index, name) in names.iter().copied().enumerate() {
            wide.push(candidate(
                name,
                1,
                1,
                1,
                1,
                1,
                11_000 + u32::try_from(index).expect("Fix: synthetic pass index fits in u32"),
                false,
            ));
        }
        let first = select_benchmark_passes_with_scratch(
            &wide,
            BenchmarkPassSelectionSample {
                frontier_items: 64,
                reuse_count: 64,
                avoidable_readback_bytes: 64,
                planning_budget_ns: 128,
                scratch_budget_bytes: 128,
            },
            &mut scratch,
        )
        .expect("Fix: wide benchmark pass selection should plan with reusable scratch");
        let seen_capacity = scratch.seen_capacity();
        let ordered_index_capacity = scratch.ordered_index_capacity();

        assert_eq!(first.selected_pass_ids.len(), names.len());

        let second = select_benchmark_passes_with_scratch(
            &[
                candidate("device.reused.high", 1, 1, 1, 10, 8, 20_000, false),
                candidate("device.reused.low", 1, 1, 1, 10, 8, 12_000, false),
            ],
            sample(),
            &mut scratch,
        )
        .expect("Fix: smaller benchmark pass selection should reuse previous scratch");

        assert_eq!(second.selected_pass_ids[0], "device.reused.high");
        assert!(scratch.seen_capacity() >= seen_capacity);
        assert!(scratch.ordered_index_capacity() >= ordered_index_capacity);
    }

    #[test]
    fn generated_benchmark_pass_profiles_preserve_budget_priority_and_ordering_contracts() {
        let mut scratch = BenchmarkPassSelectionScratch::default();
        for candidate_count in 1usize..=64 {
            for budget_multiplier in 1u64..=16 {
                let mut candidates = Vec::new();
                candidates
                    .try_reserve_exact(candidate_count)
                    .expect("Fix: generated candidate capacity");
                for index in 0..candidate_count {
                    let mandatory = index % 5 == 0;
                    candidates.push(candidate(
                        if mandatory {
                            "device.generated.mandatory"
                        } else {
                            "device.generated.optional"
                        },
                        1,
                        1,
                        u64::try_from(index % 4).expect("Fix: index fits"),
                        1 + u64::try_from(index % 3).expect("Fix: index fits"),
                        1,
                        11_000 + u32::try_from(index % 1_000).expect("Fix: index fits"),
                        mandatory,
                    ));
                    candidates[index].pass_id = generated_pass_id(index);
                }

                let plan = select_benchmark_passes_with_scratch(
                    &candidates,
                    BenchmarkPassSelectionSample {
                        frontier_items: 128,
                        reuse_count: 128,
                        avoidable_readback_bytes: 128,
                        planning_budget_ns: budget_multiplier * 64,
                        scratch_budget_bytes: budget_multiplier * 64,
                    },
                    &mut scratch,
                )
                .expect("Fix: generated benchmark pass selection profile should plan");

                let mut used_planning = 0u64;
                let mut used_scratch = 0u64;
                for pass_id in &plan.selected_pass_ids {
                    let candidate = candidates
                        .iter()
                        .find(|candidate| candidate.pass_id == *pass_id)
                        .expect("Fix: selected pass must map to a generated candidate");
                    used_planning += candidate.planning_cost_ns;
                    used_scratch += candidate.scratch_bytes;
                }
                assert_eq!(plan.total_planning_cost_ns, used_planning);
                assert_eq!(plan.total_scratch_bytes, used_scratch);
                assert!(plan.total_planning_cost_ns <= budget_multiplier * 64);
                assert!(plan.total_scratch_bytes <= budget_multiplier * 64);
                assert!(plan.projected_speedup_bps >= 10_000);
            }
        }
    }

    fn generated_pass_id(index: usize) -> &'static str {
        const IDS: [&str; 64] = [
            "device.generated.00",
            "device.generated.01",
            "device.generated.02",
            "device.generated.03",
            "device.generated.04",
            "device.generated.05",
            "device.generated.06",
            "device.generated.07",
            "device.generated.08",
            "device.generated.09",
            "device.generated.10",
            "device.generated.11",
            "device.generated.12",
            "device.generated.13",
            "device.generated.14",
            "device.generated.15",
            "device.generated.16",
            "device.generated.17",
            "device.generated.18",
            "device.generated.19",
            "device.generated.20",
            "device.generated.21",
            "device.generated.22",
            "device.generated.23",
            "device.generated.24",
            "device.generated.25",
            "device.generated.26",
            "device.generated.27",
            "device.generated.28",
            "device.generated.29",
            "device.generated.30",
            "device.generated.31",
            "device.generated.32",
            "device.generated.33",
            "device.generated.34",
            "device.generated.35",
            "device.generated.36",
            "device.generated.37",
            "device.generated.38",
            "device.generated.39",
            "device.generated.40",
            "device.generated.41",
            "device.generated.42",
            "device.generated.43",
            "device.generated.44",
            "device.generated.45",
            "device.generated.46",
            "device.generated.47",
            "device.generated.48",
            "device.generated.49",
            "device.generated.50",
            "device.generated.51",
            "device.generated.52",
            "device.generated.53",
            "device.generated.54",
            "device.generated.55",
            "device.generated.56",
            "device.generated.57",
            "device.generated.58",
            "device.generated.59",
            "device.generated.60",
            "device.generated.61",
            "device.generated.62",
            "device.generated.63",
        ];
        IDS[index]
    }

    fn sample() -> BenchmarkPassSelectionSample {
        BenchmarkPassSelectionSample {
            frontier_items: 10,
            reuse_count: 10,
            avoidable_readback_bytes: 10,
            planning_budget_ns: 100,
            scratch_budget_bytes: 100,
        }
    }

    fn candidate(
        pass_id: &'static str,
        min_frontier_items: u64,
        min_reuse_count: u64,
        min_avoided_readback_bytes: u64,
        planning_cost_ns: u64,
        scratch_bytes: u64,
        expected_speedup_bps: u32,
        mandatory_when_profitable: bool,
    ) -> BenchmarkPassCandidate {
        BenchmarkPassCandidate {
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
}
