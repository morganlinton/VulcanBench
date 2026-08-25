//! Stable optimizer pass explanations.
//!
//! The scheduler owns execution and metrics. The catalog owns pass ownership,
//! invariants, and benchmark families. This module joins both into a stable
//! contributor-facing record that explains why a pass fired or skipped and
//! what contract it preserved.

use super::{
    pass_catalog::{optimization_catalog, OptimizationCatalogEntry},
    CostModelFamily, OptimizerError, OptimizerRunReport, PassBoundaryClass, PassPhase,
    PassRunDecision, PassRunMetric,
};
use std::collections::BTreeMap;

/// Whether a pass metric row was matched to the live optimizer catalog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogLookupStatus {
    /// The pass name matched a catalog entry, so owner/invariant/benchmark
    /// fields are authoritative.
    Cataloged,
    /// The pass name was absent from the catalog. This is valid for custom
    /// ad-hoc schedulers, but release pipelines should treat it as a finding.
    MissingCatalogEntry,
}

/// Source used to explain pass order and ownership.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PassOrderOrigin {
    /// The pass matched the live optimizer catalog, whose pass entries are
    /// derived from registered scheduler metadata.
    LiveOptimizerCatalog,
    /// The metric came from a caller-supplied report without a catalog entry;
    /// order is therefore only the report order.
    MetricsOnlyReport,
}

/// Signed before/after deltas for one pass metric row.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PassMetricDelta {
    /// `nodes_after - nodes_before`.
    pub nodes: i128,
    /// `static_storage_bytes_after - static_storage_bytes_before`.
    pub static_storage_bytes: i128,
    /// `instruction_count_after - instruction_count_before`.
    pub instruction_count: i128,
    /// `memory_op_count_after - memory_op_count_before`.
    pub memory_op_count: i128,
    /// `atomic_op_count_after - atomic_op_count_before`.
    pub atomic_op_count: i128,
    /// `control_flow_count_after - control_flow_count_before`.
    pub control_flow_count: i128,
    /// `register_pressure_after - register_pressure_before`.
    pub register_pressure: i128,
    /// `ir_heap_allocations_after - ir_heap_allocations_before`.
    pub ir_heap_allocations: i128,
    /// `ir_heap_bytes_after - ir_heap_bytes_before`.
    pub ir_heap_bytes: i128,
    /// `effect_bits_after - effect_bits_before`.
    pub effect_bits: i128,
    /// `linear_type_violations_after - linear_type_violations_before`.
    pub linear_type_violations: i128,
    /// `shape_predicate_violations_after - shape_predicate_violations_before`.
    pub shape_predicate_violations: i128,
}

impl PassMetricDelta {
    #[must_use]
    fn from_metric(metric: &PassRunMetric) -> Self {
        Self {
            nodes: delta(metric.nodes_before, metric.nodes_after),
            static_storage_bytes: delta(
                metric.static_storage_bytes_before,
                metric.static_storage_bytes_after,
            ),
            instruction_count: delta(
                metric.instruction_count_before,
                metric.instruction_count_after,
            ),
            memory_op_count: delta(metric.memory_op_count_before, metric.memory_op_count_after),
            atomic_op_count: delta(metric.atomic_op_count_before, metric.atomic_op_count_after),
            control_flow_count: delta(
                metric.control_flow_count_before,
                metric.control_flow_count_after,
            ),
            register_pressure: delta(
                metric.register_pressure_before,
                metric.register_pressure_after,
            ),
            ir_heap_allocations: delta(
                metric.ir_heap_allocations_before,
                metric.ir_heap_allocations_after,
            ),
            ir_heap_bytes: delta(metric.ir_heap_bytes_before, metric.ir_heap_bytes_after),
            effect_bits: delta(metric.effect_bits_before, metric.effect_bits_after),
            linear_type_violations: delta(
                metric.linear_type_violations_before,
                metric.linear_type_violations_after,
            ),
            shape_predicate_violations: delta(
                metric.shape_predicate_violations_before,
                metric.shape_predicate_violations_after,
            ),
        }
    }
}

/// Stable explanation for one scheduler metric row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PassExplanation {
    /// Fixpoint iteration index.
    pub iteration: usize,
    /// Stable pass name.
    pub pass: &'static str,
    /// Scheduler decision.
    pub decision: PassRunDecision,
    /// Human-stable reason for [`Self::decision`].
    pub reason: &'static str,
    /// Whether transform ran.
    pub ran: bool,
    /// Whether a rewrite landed after all scheduler gates.
    pub changed: bool,
    /// Refusal kind when the pass explicitly refused.
    pub refusal_kind: Option<&'static str>,
    /// Explicit scheduler stop reason for this pass row.
    pub scheduler_stop_reason: PassRunDecision,
    /// Analyses/capability tags required before this pass can run.
    pub required_analyses: &'static [&'static str],
    /// Analyses/capability tags preserved by this pass consideration.
    pub preserved_analyses: &'static [&'static str],
    /// Analyses/capability tags invalidated by this pass consideration.
    pub invalidated_analyses: &'static [&'static str],
    /// Analyses/capability tags the pass declares it may invalidate when it
    /// lands a rewrite.
    pub declared_invalidations: &'static [&'static str],
    /// Whether the scheduler-owned fact substrate was reused.
    pub fact_substrate_reused: bool,
    /// Whether the scheduler-owned fact substrate was recomputed.
    pub fact_substrate_recomputed: bool,
    /// Whether the scheduler-owned fact substrate was invalidated.
    pub fact_substrate_invalidated: bool,
    /// Runtime spent in transform, in nanoseconds.
    pub runtime_ns: u128,
    /// Catalog lookup status.
    pub catalog_status: CatalogLookupStatus,
    /// Source used for pass-order and ownership explanation.
    pub pass_order_origin: PassOrderOrigin,
    /// Optimization owner from the live catalog.
    pub owner: Option<&'static str>,
    /// Scheduler phase from the live catalog.
    pub phase: Option<PassPhase>,
    /// Boundary class from the live catalog.
    pub boundary_class: Option<PassBoundaryClass>,
    /// Invariant preserved by the pass.
    pub invariant: Option<&'static str>,
    /// Benchmark family that owns this pass.
    pub benchmark: Option<&'static str>,
    /// Cost model family declared by the pass.
    pub cost_model_family: Option<CostModelFamily>,
    /// Before/after IR and cost proxy deltas.
    pub delta: PassMetricDelta,
}

impl PassExplanation {
    /// Build an explanation row from one metric and an optional catalog entry.
    #[must_use]
    pub fn from_metric(
        metric: &PassRunMetric,
        catalog_entry: Option<&OptimizationCatalogEntry>,
    ) -> Self {
        let catalog_status = if catalog_entry.is_some() {
            CatalogLookupStatus::Cataloged
        } else {
            CatalogLookupStatus::MissingCatalogEntry
        };
        let pass_order_origin = if catalog_entry.is_some() {
            PassOrderOrigin::LiveOptimizerCatalog
        } else {
            PassOrderOrigin::MetricsOnlyReport
        };
        Self {
            iteration: metric.iteration,
            pass: metric.pass,
            decision: metric.decision,
            reason: decision_reason(metric.decision),
            ran: metric.ran,
            changed: metric.changed,
            refusal_kind: metric.refusal_kind,
            scheduler_stop_reason: metric.decision,
            required_analyses: metric.required_analyses,
            preserved_analyses: if metric.changed {
                &[]
            } else {
                metric.required_analyses
            },
            invalidated_analyses: if metric.changed {
                metric.declared_invalidations
            } else {
                &[]
            },
            declared_invalidations: metric.declared_invalidations,
            fact_substrate_reused: metric.fact_substrate_reused,
            fact_substrate_recomputed: metric.fact_substrate_recomputed,
            fact_substrate_invalidated: metric.fact_substrate_invalidated,
            runtime_ns: metric.runtime_ns,
            catalog_status,
            pass_order_origin,
            owner: catalog_entry.map(|entry| entry.owner),
            phase: catalog_entry.map(|entry| entry.phase),
            boundary_class: catalog_entry.map(|entry| entry.boundary_class),
            invariant: catalog_entry.map(|entry| entry.invariant),
            benchmark: catalog_entry.map(|entry| entry.benchmark),
            cost_model_family: catalog_entry.map(|entry| entry.cost_model_family),
            delta: PassMetricDelta::from_metric(metric),
        }
    }
}

impl OptimizerRunReport {
    /// Convert this metrics report into stable contributor-facing pass
    /// explanations using the live optimizer catalog.
    ///
    /// # Errors
    /// Returns [`OptimizerError`] when the live catalog cannot be built because
    /// registered pass scheduling is invalid.
    pub fn explanations(&self) -> Result<Vec<PassExplanation>, OptimizerError> {
        explain_optimizer_report(self)
    }
}

/// Explain a scheduler report using the live optimizer catalog.
///
/// # Errors
/// Returns [`OptimizerError`] when the live catalog cannot be built because
/// registered pass scheduling is invalid.
pub fn explain_optimizer_report(
    report: &OptimizerRunReport,
) -> Result<Vec<PassExplanation>, OptimizerError> {
    let catalog = optimization_catalog()?;
    Ok(explain_optimizer_report_with_catalog(report, &catalog))
}

/// Explain a scheduler report using a caller-supplied catalog snapshot.
#[must_use]
pub fn explain_optimizer_report_with_catalog(
    report: &OptimizerRunReport,
    catalog: &[OptimizationCatalogEntry],
) -> Vec<PassExplanation> {
    let catalog_by_name = catalog
        .iter()
        .map(|entry| (entry.name, entry))
        .collect::<BTreeMap<_, _>>();
    report
        .passes
        .iter()
        .map(|metric| {
            PassExplanation::from_metric(metric, catalog_by_name.get(metric.pass).copied())
        })
        .collect()
}

#[must_use]
fn decision_reason(decision: PassRunDecision) -> &'static str {
    match decision {
        PassRunDecision::CleanSkipped => {
            "pass was clean; no invalidated dependency required analysis"
        }
        PassRunDecision::AnalysisSkipped => {
            "pass was dirty but its analysis hook proved no rewrite was profitable"
        }
        PassRunDecision::RanUnchanged => "pass ran but produced the same program",
        PassRunDecision::Changed => "pass ran and landed a rewrite that passed scheduler gates",
        PassRunDecision::CostReverted => {
            "pass produced a cost-increasing rewrite, so cost-monotone enforcement reverted it"
        }
        PassRunDecision::EffectReverted => {
            "pass introduced undeclared effects, so effects-handler enforcement reverted it"
        }
        PassRunDecision::LinearTypeReverted => {
            "pass introduced a new linear-type violation, so linear enforcement reverted it"
        }
        PassRunDecision::ShapePredicateReverted => {
            "pass introduced a new shape-predicate violation, so liquid-shape enforcement reverted it"
        }
        PassRunDecision::Refused => {
            "pass explicitly refused to rewrite and reported a structured refusal kind"
        }
    }
}

#[must_use]
fn delta<T>(before: T, after: T) -> i128
where
    T: TryInto<i128>,
{
    after.try_into().unwrap_or(i128::MAX) - before.try_into().unwrap_or(i128::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::Program;

    const REQUIRED_ANALYSES: &[&str] = &["shape_facts", "use_facts"];
    const DECLARED_INVALIDATIONS: &[&str] = &["shape_facts", "range_facts"];

    fn metric(pass: &'static str, decision: PassRunDecision) -> PassRunMetric {
        PassRunMetric {
            iteration: 2,
            pass,
            ran: matches!(
                decision,
                PassRunDecision::RanUnchanged
                    | PassRunDecision::Changed
                    | PassRunDecision::CostReverted
                    | PassRunDecision::EffectReverted
                    | PassRunDecision::LinearTypeReverted
                    | PassRunDecision::ShapePredicateReverted
                    | PassRunDecision::Refused
            ),
            changed: decision == PassRunDecision::Changed,
            decision,
            refusal_kind: (decision == PassRunDecision::Refused).then_some("cost_increase"),
            required_analyses: REQUIRED_ANALYSES,
            declared_invalidations: DECLARED_INVALIDATIONS,
            fact_substrate_reused: matches!(
                decision,
                PassRunDecision::RanUnchanged | PassRunDecision::Changed
            ),
            fact_substrate_recomputed: !matches!(decision, PassRunDecision::CleanSkipped),
            fact_substrate_invalidated: decision == PassRunDecision::Changed,
            effect_bits_before: 0b001,
            effect_bits_after: 0b101,
            linear_type_violations_before: 0,
            linear_type_violations_after: 1,
            shape_predicate_violations_before: 1,
            shape_predicate_violations_after: 0,
            runtime_ns: 17,
            nodes_before: 10,
            nodes_after: 7,
            static_storage_bytes_before: 64,
            static_storage_bytes_after: 32,
            instruction_count_before: 20,
            instruction_count_after: 11,
            memory_op_count_before: 5,
            memory_op_count_after: 3,
            atomic_op_count_before: 2,
            atomic_op_count_after: 1,
            control_flow_count_before: 4,
            control_flow_count_after: 6,
            register_pressure_before: 8,
            register_pressure_after: 5,
            ir_heap_allocations_before: 9,
            ir_heap_allocations_after: 4,
            ir_heap_bytes_before: 128,
            ir_heap_bytes_after: 80,
            research_trace: None,
        }
    }

    #[test]
    fn decision_reasons_are_stable_and_actionable() {
        for decision in [
            PassRunDecision::CleanSkipped,
            PassRunDecision::AnalysisSkipped,
            PassRunDecision::RanUnchanged,
            PassRunDecision::Changed,
            PassRunDecision::CostReverted,
            PassRunDecision::EffectReverted,
            PassRunDecision::LinearTypeReverted,
            PassRunDecision::ShapePredicateReverted,
            PassRunDecision::Refused,
        ] {
            let reason = decision_reason(decision);
            assert!(
                !reason.is_empty() && reason.contains("pass"),
                "Fix: every pass decision must explain why the scheduler made it."
            );
        }
    }

    #[test]
    fn explanation_records_catalog_contract_and_metric_delta() {
        let catalog = optimization_catalog().expect("Fix: optimizer catalog must build");
        let entry = catalog
            .iter()
            .find(|entry| entry.name == "megakernel.allocation_reuse")
            .expect("Fix: release catalog must contain megakernel allocation reuse");
        let explanation = PassExplanation::from_metric(
            &metric("megakernel.allocation_reuse", PassRunDecision::Changed),
            Some(entry),
        );

        assert_eq!(explanation.catalog_status, CatalogLookupStatus::Cataloged);
        assert_eq!(
            explanation.pass_order_origin,
            PassOrderOrigin::LiveOptimizerCatalog
        );
        assert_eq!(explanation.owner, Some("vyre-runtime-rules"));
        assert_eq!(explanation.invariant, Some(entry.invariant));
        assert_eq!(
            explanation.benchmark,
            Some("vyre.megakernel.optimizer.rules")
        );
        assert_eq!(explanation.scheduler_stop_reason, PassRunDecision::Changed);
        assert_eq!(explanation.required_analyses, REQUIRED_ANALYSES);
        assert!(explanation.preserved_analyses.is_empty());
        assert_eq!(explanation.invalidated_analyses, DECLARED_INVALIDATIONS);
        assert_eq!(explanation.declared_invalidations, DECLARED_INVALIDATIONS);
        assert!(explanation.fact_substrate_reused);
        assert!(explanation.fact_substrate_recomputed);
        assert!(explanation.fact_substrate_invalidated);
        assert_eq!(explanation.delta.nodes, -3);
        assert_eq!(explanation.delta.control_flow_count, 2);
        assert_eq!(explanation.delta.effect_bits, 4);
        assert_eq!(explanation.delta.linear_type_violations, 1);
        assert_eq!(explanation.delta.shape_predicate_violations, -1);
        assert_eq!(
            explanation.reason,
            decision_reason(PassRunDecision::Changed)
        );
    }

    #[test]
    fn report_explanation_surfaces_uncataloged_custom_passes() {
        let report = OptimizerRunReport {
            program: Program::empty(),
            passes: vec![metric(
                "external.custom.pass",
                PassRunDecision::RanUnchanged,
            )],
        };
        let explanations = explain_optimizer_report_with_catalog(&report, &[]);

        assert_eq!(explanations.len(), 1);
        assert_eq!(
            explanations[0].catalog_status,
            CatalogLookupStatus::MissingCatalogEntry
        );
        assert_eq!(
            explanations[0].pass_order_origin,
            PassOrderOrigin::MetricsOnlyReport
        );
        assert!(explanations[0].owner.is_none());
        assert!(explanations[0].invariant.is_none());
    }

    #[test]
    fn unchanged_pass_explanation_preserves_required_analyses_without_invalidating_facts() {
        let explanation = PassExplanation::from_metric(
            &metric("external.custom.pass", PassRunDecision::RanUnchanged),
            None,
        );

        assert_eq!(
            explanation.scheduler_stop_reason,
            PassRunDecision::RanUnchanged
        );
        assert_eq!(explanation.required_analyses, REQUIRED_ANALYSES);
        assert_eq!(explanation.preserved_analyses, REQUIRED_ANALYSES);
        assert!(explanation.invalidated_analyses.is_empty());
        assert_eq!(explanation.declared_invalidations, DECLARED_INVALIDATIONS);
        assert!(explanation.fact_substrate_reused);
        assert!(explanation.fact_substrate_recomputed);
        assert!(!explanation.fact_substrate_invalidated);
    }

    #[test]
    fn run_report_explanations_use_live_catalog() {
        let report = OptimizerRunReport {
            program: Program::empty(),
            passes: vec![metric(
                "megakernel.allocation_reuse",
                PassRunDecision::Changed,
            )],
        };
        let explanations = report
            .explanations()
            .expect("Fix: run report explanations must build from live catalog");

        assert_eq!(explanations.len(), 1);
        assert_eq!(
            explanations[0].catalog_status,
            CatalogLookupStatus::Cataloged
        );
        assert_eq!(
            explanations[0].benchmark,
            Some("vyre.megakernel.optimizer.rules")
        );
    }
}
