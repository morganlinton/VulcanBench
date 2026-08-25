//! Non-blocking automata worklist policy for resident megakernel scheduling.
//!
//! Automata traversal and graph-style frontier traversal both reduce to
//! state/index pairs that may expand irregularly. This module owns the
//! runtime policy and evidence contract for choosing a non-blocking worklist
//! path without introducing automata-specific protocol words.

use vyre_driver::backend::BackendError;

use super::planner::MegakernelWorkItem;
use super::task::{TaskPriority, TaskState, TaskWorkItem, TASK_FLAG_REQUEUE_REQUESTED};

/// Schema version for automata worklist benchmark evidence.
pub const AUTOMATA_WORKLIST_EVIDENCE_SCHEMA_VERSION: u32 = 1;

/// One automata frontier item encoded as a state/index pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AutomataStateIndex {
    /// Automata state id.
    pub state_id: u32,
    /// Input byte index associated with the state.
    pub byte_index: u32,
}

impl AutomataStateIndex {
    /// Construct a state/index pair.
    #[must_use]
    pub const fn new(state_id: u32, byte_index: u32) -> Self {
        Self {
            state_id,
            byte_index,
        }
    }

    /// Encode this state/index pair into the shared continuation task ABI.
    #[must_use]
    pub fn to_task_work_item(
        self,
        task_id: u32,
        tenant_id: u32,
        priority: TaskPriority,
        op_handle: u32,
        input_handle: u32,
        output_handle: u32,
    ) -> TaskWorkItem {
        let mut task = TaskWorkItem::from_work_item(
            task_id,
            tenant_id,
            priority,
            MegakernelWorkItem {
                op_handle,
                input_handle,
                output_handle,
                param: self.state_id,
            },
        );
        task.state = TaskState::Ready.word();
        task.continuation_pc = self.byte_index;
        task.continuation_data = self.state_id;
        task.flags |= TASK_FLAG_REQUEUE_REQUESTED;
        task
    }
}

/// Scheduler mode selected for an automata worklist request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AutomataWorklistMode {
    /// No work is queued.
    Empty,
    /// The frontier is small enough for a blocking DFA/NFA kernel baseline.
    Blocking,
    /// Use a non-blocking state/index worklist.
    NonBlocking,
    /// The worklist must spill or shard before resident execution.
    SpillRequired,
}

impl AutomataWorklistMode {
    /// Stable label for benchmark and release evidence.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::Blocking => "blocking",
            Self::NonBlocking => "non_blocking",
            Self::SpillRequired => "spill_required",
        }
    }
}

/// Inputs for non-blocking automata worklist policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AutomataWorklistRequest {
    /// Current worklist depth in state/index pairs.
    pub worklist_depth: u32,
    /// Number of state visits measured or planned for this corpus slice.
    pub state_visit_count: u64,
    /// Active-lane or occupancy proxy in basis points.
    pub occupancy_proxy_bps: u16,
    /// Blocking DFA/NFA active time for the same corpus slice.
    pub blocking_active_time_ns: u64,
    /// Non-blocking worklist active time for the same corpus slice.
    pub nonblocking_active_time_ns: u64,
}

impl AutomataWorklistRequest {
    /// Construct an empty worklist request.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            worklist_depth: 0,
            state_visit_count: 0,
            occupancy_proxy_bps: 0,
            blocking_active_time_ns: 0,
            nonblocking_active_time_ns: 0,
        }
    }
}

/// Policy for selecting blocking versus non-blocking automata traversal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AutomataWorklistPolicy {
    /// Worklist depth at or above which non-blocking traversal is preferred.
    pub nonblocking_depth_threshold: u32,
    /// Worklist depth above which the request should spill or shard first.
    pub spill_depth_threshold: u32,
    /// Multiplier applied to depth to derive a state-visit budget.
    pub state_visit_budget_multiplier: u32,
    /// Occupancy below this value prefers non-blocking traversal.
    pub low_occupancy_threshold_bps: u16,
}

impl Default for AutomataWorklistPolicy {
    fn default() -> Self {
        Self::standard()
    }
}

impl AutomataWorklistPolicy {
    /// Standard policy for resident automata worklists.
    #[must_use]
    pub const fn standard() -> Self {
        Self {
            nonblocking_depth_threshold: 64,
            spill_depth_threshold: 1_048_576,
            state_visit_budget_multiplier: 8,
            low_occupancy_threshold_bps: 5_000,
        }
    }

    /// Recommend a scheduler mode for a resident automata worklist.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when derived state-visit budgets overflow.
    pub fn recommend(
        self,
        request: AutomataWorklistRequest,
    ) -> Result<AutomataWorklistRecommendation, BackendError> {
        let state_visit_budget = request
            .worklist_depth
            .checked_mul(self.state_visit_budget_multiplier)
            .map(u64::from)
            .ok_or_else(|| {
                BackendError::new(
                    "automata worklist state-visit budget overflowed u32. Fix: shard the state-index frontier before resident scheduling.",
                )
            })?;
        let mode = if request.worklist_depth == 0 {
            AutomataWorklistMode::Empty
        } else if request.worklist_depth > self.spill_depth_threshold {
            AutomataWorklistMode::SpillRequired
        } else if request.worklist_depth >= self.nonblocking_depth_threshold
            || request.occupancy_proxy_bps < self.low_occupancy_threshold_bps
        {
            AutomataWorklistMode::NonBlocking
        } else {
            AutomataWorklistMode::Blocking
        };
        Ok(AutomataWorklistRecommendation {
            mode,
            worklist_depth: request.worklist_depth,
            state_visit_budget,
            state_visit_count: request.state_visit_count,
            occupancy_proxy_bps: request.occupancy_proxy_bps.min(10_000),
            match_parity_required: true,
            reports_state_index_pairs: true,
        })
    }

    /// Recommend a scheduler mode and emit benchmark evidence.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when derived state-visit budgets overflow.
    pub fn recommend_with_evidence(
        self,
        request: AutomataWorklistRequest,
    ) -> Result<(AutomataWorklistRecommendation, AutomataWorklistEvidence), BackendError> {
        let recommendation = self.recommend(request)?;
        let evidence = AutomataWorklistEvidence {
            schema_version: AUTOMATA_WORKLIST_EVIDENCE_SCHEMA_VERSION,
            selected_mode: recommendation.mode,
            worklist_depth: recommendation.worklist_depth,
            state_visit_count: recommendation.state_visit_count,
            occupancy_proxy_bps: recommendation.occupancy_proxy_bps,
            blocking_active_time_ns: request.blocking_active_time_ns,
            nonblocking_active_time_ns: request.nonblocking_active_time_ns,
            match_parity_required: recommendation.match_parity_required,
            reports_state_index_pairs: recommendation.reports_state_index_pairs,
        };
        Ok((recommendation, evidence))
    }
}

/// Policy output consumed by resident regex and graph-style schedulers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AutomataWorklistRecommendation {
    /// Selected scheduler mode.
    pub mode: AutomataWorklistMode,
    /// Worklist depth in state/index pairs.
    pub worklist_depth: u32,
    /// Derived state-visit budget for the resident scheduler.
    pub state_visit_budget: u64,
    /// Observed or planned state visits.
    pub state_visit_count: u64,
    /// Occupancy proxy in basis points.
    pub occupancy_proxy_bps: u16,
    /// True when blocking and non-blocking outputs must match.
    pub match_parity_required: bool,
    /// True when benchmark evidence must report state/index-pair work.
    pub reports_state_index_pairs: bool,
}

/// Benchmark evidence for blocking versus non-blocking automata worklists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AutomataWorklistEvidence {
    /// Evidence schema version.
    pub schema_version: u32,
    /// Selected scheduler mode.
    pub selected_mode: AutomataWorklistMode,
    /// Worklist depth in state/index pairs.
    pub worklist_depth: u32,
    /// Number of state visits reported by the benchmark.
    pub state_visit_count: u64,
    /// Occupancy proxy in basis points.
    pub occupancy_proxy_bps: u16,
    /// Blocking DFA/NFA active time for the same corpus slice.
    pub blocking_active_time_ns: u64,
    /// Non-blocking worklist active time for the same corpus slice.
    pub nonblocking_active_time_ns: u64,
    /// True when benchmark outputs must prove match parity.
    pub match_parity_required: bool,
    /// True when evidence reports state/index-pair work instead of opaque jobs.
    pub reports_state_index_pairs: bool,
}

impl AutomataWorklistEvidence {
    /// Return true when the evidence contains the required benchmark fields.
    #[must_use]
    pub fn is_complete(self) -> bool {
        self.schema_version == AUTOMATA_WORKLIST_EVIDENCE_SCHEMA_VERSION
            && self.occupancy_proxy_bps <= 10_000
            && self.match_parity_required
            && self.reports_state_index_pairs
            && (self.selected_mode == AutomataWorklistMode::Empty || self.worklist_depth != 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_index_pair_uses_shared_task_work_item_abi() {
        let pair = AutomataStateIndex::new(17, 4096);
        let task = pair.to_task_work_item(5, 3, TaskPriority::High, 99, 12, 13);

        assert_eq!(task.state, TaskState::Ready.word());
        assert_eq!(task.task_id, 5);
        assert_eq!(task.tenant_id, 3);
        assert_eq!(task.priority, TaskPriority::High.word());
        assert_eq!(task.op_handle, 99);
        assert_eq!(task.input_handle, 12);
        assert_eq!(task.output_handle, 13);
        assert_eq!(task.param, 17);
        assert_eq!(task.continuation_pc, 4096);
        assert_eq!(task.continuation_data, 17);
        assert_eq!(task.flags & TASK_FLAG_REQUEUE_REQUESTED, TASK_FLAG_REQUEUE_REQUESTED);
    }

    #[test]
    fn policy_emits_nonblocking_worklist_evidence() {
        let policy = AutomataWorklistPolicy::standard();
        let request = AutomataWorklistRequest {
            worklist_depth: policy.nonblocking_depth_threshold,
            state_visit_count: 2048,
            occupancy_proxy_bps: 2_500,
            blocking_active_time_ns: 900,
            nonblocking_active_time_ns: 600,
        };

        let (recommendation, evidence) = policy
            .recommend_with_evidence(request)
            .expect("Fix: valid automata worklist request should emit evidence");

        assert_eq!(recommendation.mode, AutomataWorklistMode::NonBlocking);
        assert_eq!(
            recommendation.state_visit_budget,
            u64::from(policy.nonblocking_depth_threshold * policy.state_visit_budget_multiplier)
        );
        assert_eq!(evidence.schema_version, AUTOMATA_WORKLIST_EVIDENCE_SCHEMA_VERSION);
        assert_eq!(evidence.selected_mode, AutomataWorklistMode::NonBlocking);
        assert_eq!(evidence.worklist_depth, policy.nonblocking_depth_threshold);
        assert_eq!(evidence.state_visit_count, 2048);
        assert_eq!(evidence.occupancy_proxy_bps, 2_500);
        assert_eq!(evidence.blocking_active_time_ns, 900);
        assert_eq!(evidence.nonblocking_active_time_ns, 600);
        assert!(evidence.match_parity_required);
        assert!(evidence.reports_state_index_pairs);
        assert!(evidence.is_complete());
    }
}
