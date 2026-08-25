//! Benchmark and hot-path driven optimizer pass selection.
//!
//! This is an execution input, not a docs surface: callers can build a
//! `PassScheduler` from [`registered_passes_for_profile_and_program`] so
//! expensive optimization families run only when program shape or runtime
//! telemetry justifies them. Correctness-critical normalizers remain selected.

use super::{
    registered_pass_registrations, CostModelFamily, OptimizerError, OptimizerProfile,
    OptimizerRunReport, PassMetadata, PassRunMetric, ProgramPassKind,
};
use crate::ir_inner::model::program::Program;
use crate::optimizer::hot_path_hints::HotPathHints;
use rustc_hash::{FxHashMap, FxHashSet};

const MIN_LOOP_NODES: usize = 12;
const MIN_MEMORY_BYTES: u64 = 16 * 1024;
const MIN_FUSION_REGIONS: usize = 2;
const MIN_DATAFLOW_NODES: usize = 64;
const MIN_MEGAKERNEL_NODES: usize = 512;

/// Why one pass was selected or skipped for a concrete Program.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PassSelectionReason {
    /// Pass is cheap or correctness-preserving enough to always keep.
    AlwaysOn,
    /// Program shape crosses the pass family's benchmark threshold.
    ProgramShape,
    /// Runtime hot-path telemetry says this region is expensive enough.
    HotPathTelemetry,
    /// Previous pass-cost metrics prove this pass reduced a cost proxy on the
    /// same pass family.
    PassCostReport,
    /// Pass was included because another selected pass requires it.
    RequiredDependency,
    /// Pass does not belong to the requested profile.
    ProfileRejected,
    /// Program and telemetry do not justify this pass family.
    BelowThreshold,
}

/// Selection result for one pass metadata row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PassSelectionDecision {
    /// Pass metadata.
    pub metadata: PassMetadata,
    /// Whether the pass should be instantiated for this Program.
    pub selected: bool,
    /// Stable reason for the decision.
    pub reason: PassSelectionReason,
    /// Stable selection priority. Higher values represent stronger evidence;
    /// callers that do not reorder can still surface this to explain why a pass
    /// was kept.
    pub priority: u16,
}

/// Instantiate registered passes accepted by `profile` and selected for
/// `program`.
///
/// # Errors
/// Returns [`OptimizerError`] if the live pass registry is invalid.
pub fn registered_passes_for_profile_and_program(
    profile: OptimizerProfile,
    program: &Program,
    hints: &HotPathHints,
) -> Result<Vec<ProgramPassKind>, OptimizerError> {
    let registrations = registered_pass_registrations()?;
    let metadata = registrations
        .iter()
        .map(|registration| registration.metadata)
        .collect::<Vec<_>>();
    let selected = selected_name_set(&metadata, profile, program, hints, None);
    let mut passes = Vec::with_capacity(selected.len());
    for registration in registrations.iter() {
        if selected.contains(registration.metadata.name) {
            passes.push(ProgramPassKind::from_boxed((registration.factory)()));
        }
    }
    Ok(passes)
}

/// Return pass-selection decisions for a metadata slice.
#[must_use]
pub fn select_pass_metadata_for_program(
    metadata: &[PassMetadata],
    profile: OptimizerProfile,
    program: &Program,
    hints: &HotPathHints,
) -> Vec<PassSelectionDecision> {
    select_pass_metadata_for_program_with_report(metadata, profile, program, hints, None)
}

/// Return pass-selection decisions using optional previous pass-cost metrics.
///
/// The report path is additive: no caller is required to provide profile data,
/// and the empty report behaves exactly like [`select_pass_metadata_for_program`].
#[must_use]
pub fn select_pass_metadata_for_program_with_report(
    metadata: &[PassMetadata],
    profile: OptimizerProfile,
    program: &Program,
    hints: &HotPathHints,
    report: Option<&OptimizerRunReport>,
) -> Vec<PassSelectionDecision> {
    let cost_report = report.map(PassCostReport::from_optimizer_report);
    let selected = selected_name_set(metadata, profile, program, hints, cost_report.as_ref());
    metadata
        .iter()
        .copied()
        .map(|metadata| {
            let profile_accepted = profile.accepts(metadata);
            let initially =
                initial_selection_reason(metadata, profile, program, hints, cost_report.as_ref());
            let selected_by_closure = selected.contains(metadata.name);
            let reason = if !profile_accepted {
                PassSelectionReason::ProfileRejected
            } else if matches!(initially, PassSelectionReason::BelowThreshold)
                && selected_by_closure
            {
                PassSelectionReason::RequiredDependency
            } else {
                initially
            };
            PassSelectionDecision {
                metadata,
                selected: selected_by_closure,
                priority: selection_priority(reason),
                reason,
            }
        })
        .collect()
}

fn selected_name_set(
    metadata: &[PassMetadata],
    profile: OptimizerProfile,
    program: &Program,
    hints: &HotPathHints,
    cost_report: Option<&PassCostReport>,
) -> FxHashSet<&'static str> {
    let mut selected = FxHashSet::default();
    for pass in metadata {
        if matches!(
            initial_selection_reason(*pass, profile, program, hints, cost_report),
            PassSelectionReason::AlwaysOn
                | PassSelectionReason::ProgramShape
                | PassSelectionReason::HotPathTelemetry
                | PassSelectionReason::PassCostReport
        ) {
            selected.insert(pass.name);
        }
    }
    close_over_requirements(metadata, &mut selected);
    selected
}

fn close_over_requirements(metadata: &[PassMetadata], selected: &mut FxHashSet<&'static str>) {
    loop {
        let before = selected.len();
        for pass in metadata {
            if selected.contains(pass.name) {
                for &requirement in pass.requires {
                    if metadata
                        .iter()
                        .any(|candidate| candidate.name == requirement)
                    {
                        selected.insert(requirement);
                    }
                }
            }
        }
        if selected.len() == before {
            break;
        }
    }
}

fn initial_selection_reason(
    metadata: PassMetadata,
    profile: OptimizerProfile,
    program: &Program,
    hints: &HotPathHints,
    cost_report: Option<&PassCostReport>,
) -> PassSelectionReason {
    if !profile.accepts(metadata) {
        return PassSelectionReason::ProfileRejected;
    }
    if entry_region_is_hot(program, hints) {
        return PassSelectionReason::HotPathTelemetry;
    }
    if cost_report.is_some_and(|report| report.selects(metadata.name)) {
        return PassSelectionReason::PassCostReport;
    }
    let stats = program.stats();
    let reason_for = |above_threshold: bool| {
        if above_threshold {
            PassSelectionReason::ProgramShape
        } else {
            PassSelectionReason::BelowThreshold
        }
    };
    match metadata.cost_model_family {
        CostModelFamily::Loop => reason_for(stats.node_count >= MIN_LOOP_NODES),
        CostModelFamily::Memory => {
            reason_for(program.estimate_peak_vram_bytes() >= MIN_MEMORY_BYTES)
        }
        CostModelFamily::Fusion => {
            reason_for(stats.top_level_regions as usize >= MIN_FUSION_REGIONS)
        }
        CostModelFamily::Dataflow => reason_for(stats.node_count >= MIN_DATAFLOW_NODES),
        CostModelFamily::Megakernel => reason_for(stats.node_count >= MIN_MEGAKERNEL_NODES),
        CostModelFamily::Scalar | CostModelFamily::Sync | CostModelFamily::Unknown => {
            PassSelectionReason::AlwaysOn
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct PassCostReport {
    cost_reducing_passes: FxHashSet<&'static str>,
}

impl PassCostReport {
    fn from_optimizer_report(report: &OptimizerRunReport) -> Self {
        let mut cost_reducing_passes = FxHashSet::default();
        cost_reducing_passes.reserve(report.passes.len());
        let mut best_delta_by_pass: FxHashMap<&'static str, i128> = FxHashMap::default();
        for metric in &report.passes {
            let reduction = metric_cost_reduction(metric);
            if reduction > 0 {
                best_delta_by_pass
                    .entry(metric.pass)
                    .and_modify(|best| *best = (*best).max(reduction))
                    .or_insert(reduction);
            }
        }
        cost_reducing_passes.extend(best_delta_by_pass.into_keys());
        Self {
            cost_reducing_passes,
        }
    }

    fn selects(&self, pass: &'static str) -> bool {
        self.cost_reducing_passes.contains(pass)
    }
}

fn metric_cost_reduction(metric: &PassRunMetric) -> i128 {
    if !metric.changed {
        return 0;
    }
    [
        reduction(metric.nodes_before, metric.nodes_after),
        reduction(
            metric.static_storage_bytes_before,
            metric.static_storage_bytes_after,
        ),
        reduction(
            metric.instruction_count_before,
            metric.instruction_count_after,
        ),
        reduction(metric.memory_op_count_before, metric.memory_op_count_after),
        reduction(metric.atomic_op_count_before, metric.atomic_op_count_after),
        reduction(
            metric.control_flow_count_before,
            metric.control_flow_count_after,
        ),
        reduction(
            metric.register_pressure_before,
            metric.register_pressure_after,
        ),
        reduction(
            metric.ir_heap_allocations_before,
            metric.ir_heap_allocations_after,
        ),
        reduction(metric.ir_heap_bytes_before, metric.ir_heap_bytes_after),
    ]
    .into_iter()
    .max()
    .unwrap_or(0)
}

fn reduction<T>(before: T, after: T) -> i128
where
    T: TryInto<i128>,
{
    before.try_into().unwrap_or(i128::MAX) - after.try_into().unwrap_or(i128::MAX)
}

fn selection_priority(reason: PassSelectionReason) -> u16 {
    match reason {
        PassSelectionReason::HotPathTelemetry => 500,
        PassSelectionReason::PassCostReport => 400,
        PassSelectionReason::ProgramShape => 300,
        PassSelectionReason::AlwaysOn => 200,
        PassSelectionReason::RequiredDependency => 100,
        PassSelectionReason::BelowThreshold | PassSelectionReason::ProfileRejected => 0,
    }
}

fn entry_region_is_hot(program: &Program, hints: &HotPathHints) -> bool {
    program
        .entry_op_id()
        .is_some_and(|op_id| hints.is_hot(op_id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{BufferDecl, DataType, Expr, Node, Program};
    use crate::optimizer::{PassBoundaryClass, PassPhase, PassRunDecision};

    fn meta(
        name: &'static str,
        family: CostModelFamily,
        phase: PassPhase,
        requires: &'static [&'static str],
    ) -> PassMetadata {
        PassMetadata {
            name,
            requires,
            invalidates: &[],
            phase,
            boundary_class: PassBoundaryClass::AbiPreserving,
            requires_caps: &[],
            preserves_abi: true,
            cost_model_family: family,
        }
    }

    fn tiny_program() -> Program {
        Program::wrapped(
            vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
            [1, 1, 1],
            vec![Node::store("out", Expr::u32(0), Expr::u32(1))],
        )
    }

    fn pass_metric(
        pass: &'static str,
        changed: bool,
        nodes_before: usize,
        nodes_after: usize,
    ) -> PassRunMetric {
        PassRunMetric {
            iteration: 0,
            pass,
            ran: true,
            changed,
            decision: if changed {
                PassRunDecision::Changed
            } else {
                PassRunDecision::RanUnchanged
            },
            refusal_kind: None,
            required_analyses: &[],
            declared_invalidations: &[],
            fact_substrate_reused: !changed,
            fact_substrate_recomputed: changed,
            fact_substrate_invalidated: changed,
            effect_bits_before: 0,
            effect_bits_after: 0,
            linear_type_violations_before: 0,
            linear_type_violations_after: 0,
            shape_predicate_violations_before: 0,
            shape_predicate_violations_after: 0,
            runtime_ns: 100,
            nodes_before,
            nodes_after,
            static_storage_bytes_before: 4096,
            static_storage_bytes_after: 4096,
            instruction_count_before: 12,
            instruction_count_after: 12,
            memory_op_count_before: 1,
            memory_op_count_after: 1,
            atomic_op_count_before: 0,
            atomic_op_count_after: 0,
            control_flow_count_before: 0,
            control_flow_count_after: 0,
            register_pressure_before: 4,
            register_pressure_after: 4,
            ir_heap_allocations_before: 0,
            ir_heap_allocations_after: 0,
            ir_heap_bytes_before: 0,
            ir_heap_bytes_after: 0,
            research_trace: None,
        }
    }

    fn report_with(metric: PassRunMetric) -> OptimizerRunReport {
        OptimizerRunReport {
            program: tiny_program(),
            passes: vec![metric],
        }
    }

    #[test]
    fn small_cold_program_skips_expensive_memory_pass() {
        let decisions = select_pass_metadata_for_program(
            &[meta(
                "decode_scan_fuse",
                CostModelFamily::Memory,
                PassPhase::Memory,
                &[],
            )],
            OptimizerProfile::Release,
            &tiny_program(),
            &HotPathHints::default(),
        );
        assert_eq!(decisions[0].selected, false);
        assert_eq!(decisions[0].reason, PassSelectionReason::BelowThreshold);
    }

    #[test]
    fn hot_region_selects_expensive_pass() {
        let hints = HotPathHints::with_capacity(4);
        hints.record("hot_entry", 1_000_000, 4);
        let program = Program::wrapped(
            vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
            [1, 1, 1],
            vec![Node::store("out", Expr::u32(0), Expr::u32(1))],
        )
        .with_entry_op_id("hot_entry");
        let decisions = select_pass_metadata_for_program(
            &[meta(
                "decode_scan_fuse",
                CostModelFamily::Memory,
                PassPhase::Memory,
                &[],
            )],
            OptimizerProfile::Release,
            &program,
            &hints,
        );
        assert!(decisions[0].selected);
        assert_eq!(decisions[0].reason, PassSelectionReason::HotPathTelemetry);
        assert_eq!(
            decisions[0].priority,
            selection_priority(PassSelectionReason::HotPathTelemetry)
        );
        assert!(decisions[0].priority > selection_priority(PassSelectionReason::PassCostReport));
    }

    #[test]
    fn pass_cost_report_selects_cold_expensive_pass() {
        let report = report_with(pass_metric("decode_scan_fuse", true, 10, 3));
        let decisions = select_pass_metadata_for_program_with_report(
            &[meta(
                "decode_scan_fuse",
                CostModelFamily::Memory,
                PassPhase::Memory,
                &[],
            )],
            OptimizerProfile::Release,
            &tiny_program(),
            &HotPathHints::default(),
            Some(&report),
        );
        assert!(decisions[0].selected);
        assert_eq!(decisions[0].reason, PassSelectionReason::PassCostReport);
        assert_eq!(
            decisions[0].priority,
            selection_priority(PassSelectionReason::PassCostReport)
        );
    }

    #[test]
    fn non_reducing_cost_report_preserves_cold_fallback() {
        let report = report_with(pass_metric("decode_scan_fuse", false, 10, 3));
        let decisions = select_pass_metadata_for_program_with_report(
            &[meta(
                "decode_scan_fuse",
                CostModelFamily::Memory,
                PassPhase::Memory,
                &[],
            )],
            OptimizerProfile::Release,
            &tiny_program(),
            &HotPathHints::default(),
            Some(&report),
        );
        assert!(!decisions[0].selected);
        assert_eq!(decisions[0].reason, PassSelectionReason::BelowThreshold);
        assert_eq!(
            decisions[0].priority,
            selection_priority(PassSelectionReason::BelowThreshold)
        );
    }

    #[test]
    fn selected_pass_closes_over_required_dependencies() {
        let metadata = [
            meta(
                "shape_facts",
                CostModelFamily::Memory,
                PassPhase::Memory,
                &[],
            ),
            meta(
                "memory_optimizer",
                CostModelFamily::Scalar,
                PassPhase::ScalarAlgebra,
                &["shape_facts"],
            ),
        ];
        let decisions = select_pass_metadata_for_program(
            &metadata,
            OptimizerProfile::Release,
            &tiny_program(),
            &HotPathHints::default(),
        );
        assert!(decisions.iter().all(|decision| decision.selected));
        assert_eq!(decisions[0].reason, PassSelectionReason::RequiredDependency);
    }

    #[test]
    fn selected_registered_passes_run_through_scheduler() {
        let program = tiny_program();
        let passes = registered_passes_for_profile_and_program(
            OptimizerProfile::Release,
            &program,
            &HotPathHints::default(),
        )
        .expect("Fix: live registry selection must succeed");
        let optimized = crate::optimizer::PassScheduler::with_passes(passes)
            .run(program)
            .expect("Fix: selected release pass scheduler must converge");
        assert!(optimized.stats().node_count > 0);
    }
}
