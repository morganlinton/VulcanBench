//! PassScheduler run methods + invalidation propagation.
//! Audit cleanup A21 (2026-04-30): split from monolithic scheduler.rs.

#![allow(unused_imports)]

use rustc_hash::{FxHashMap, FxHashSet};
use std::collections::BTreeSet;
use std::sync::OnceLock;

use super::{
    estimate_ir_allocations, IrAllocationEstimate, OptimizerRunReport, PassRunDecision,
    PassRunMetric, PassScheduler,
};
use crate::ir::{BufferDecl, Expr, Node};
use crate::ir_inner::model::program::Program;
use crate::optimizer::{
    fact_substrate::FactSubstrate, registered_passes, requirements_satisfied, OptimizerError,
    PassMetadata, ProgramPassKind, ProgramPassRegistration,
};
use crate::runtime::perf::PerfScope;

#[derive(Debug, Default)]
pub(crate) struct SchedulerFactState {
    substrate: Option<FactSubstrate>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct SchedulerFactEvent {
    reused: bool,
    recomputed: bool,
}

impl SchedulerFactState {
    fn prepare(&mut self, program: &Program) -> SchedulerFactEvent {
        if self
            .substrate
            .as_ref()
            .is_some_and(|facts| facts.is_fresh_for(program))
        {
            return SchedulerFactEvent {
                reused: true,
                recomputed: false,
            };
        }
        self.substrate = Some(FactSubstrate::derive(program));
        SchedulerFactEvent {
            reused: false,
            recomputed: true,
        }
    }

    fn invalidate(&mut self) -> bool {
        let Some(substrate) = self.substrate.as_mut() else {
            return false;
        };
        substrate.invalidate();
        true
    }
}

fn introduces_forbidden_effects(
    before: crate::lower::effects::ProgramEffects,
    after: crate::lower::effects::ProgramEffects,
    allowed_additions: crate::lower::effects::ProgramEffects,
) -> bool {
    !after
        .introduced_since(before)
        .is_subset_of(allowed_additions)
}

fn linear_type_violations(program: &Program) -> BTreeSet<String> {
    crate::validate::linear_type::check_linear_types(program)
        .into_iter()
        .map(|error| error.message.into_owned())
        .collect()
}

fn introduces_linear_type_violations(before: &BTreeSet<String>, after: &BTreeSet<String>) -> bool {
    after.iter().any(|violation| !before.contains(violation))
}

fn shape_predicate_violations(program: &Program) -> BTreeSet<String> {
    crate::validate::shape_predicate::check_shape_predicates(program)
        .into_iter()
        .map(|error| error.message.into_owned())
        .collect()
}

fn introduces_shape_predicate_violations(
    before: &BTreeSet<String>,
    after: &BTreeSet<String>,
) -> bool {
    after.iter().any(|violation| !before.contains(violation))
}

impl PassScheduler {
    /// `tag → pass names that depend on it` (their own name OR a `requires`
    /// entry equals the tag). Computed once per scheduler. Replaces the
    /// linear pass-list scan that the previous implementation ran on every
    /// invalidation event.
    fn dirty_trigger_index(&self) -> &FxHashMap<&'static str, Vec<usize>> {
        self.dirty_trigger_index_cache.get_or_init(|| {
            let mut index: FxHashMap<&'static str, Vec<usize>> = FxHashMap::default();
            index.reserve(self.passes.len() * 2);
            for (pass_index, pass) in self.passes.iter().enumerate() {
                let metadata = pass.metadata();
                index.entry(metadata.name).or_default().push(pass_index);
                for &req in metadata.requires {
                    index.entry(req).or_default().push(pass_index);
                }
            }
            index
        })
    }

    #[cfg(test)]
    pub(crate) fn mark_invalidated_passes(
        &self,
        invalidated: &[&'static str],
        next_dirty: &mut FxHashSet<&'static str>,
    ) {
        let index = self.dirty_trigger_index();
        for &tag in invalidated {
            if let Some(triggered) = index.get(tag) {
                for &pass_index in triggered {
                    if let Some(pass) = self.passes.get(pass_index) {
                        next_dirty.insert(pass.metadata().name);
                    }
                }
            }
        }
    }

    fn mark_invalidated_pass_flags(&self, invalidated: &[&'static str], next_dirty: &mut [bool]) {
        let index = self.dirty_trigger_index();
        for &tag in invalidated {
            if let Some(triggered) = index.get(tag) {
                for &pass_index in triggered {
                    if let Some(slot) = next_dirty.get_mut(pass_index) {
                        *slot = true;
                    }
                }
            }
        }
    }

    /// Execute the scheduled passes repeatedly until convergence or max iterations are reached.
    ///
    /// # Errors
    ///
    /// Returns [`OptimizerError`] if pass dependencies are unsatisfied or the
    /// scheduler fails to converge within the configured iteration bound.
    pub fn run(&self, program: Program) -> Result<Program, OptimizerError> {
        let mut program = program;
        let mut last_pass = "<none>";
        let mut dirty = self.initial_dirty_flags();
        let mut next_dirty = vec![false; self.passes.len()];

        for _ in 0..self.max_iterations {
            next_dirty.fill(false);
            let (next, changed, changed_by) =
                self.run_once_flags(program, &dirty, &mut next_dirty)?;
            program = next;
            if let Some(name) = changed_by {
                last_pass = name;
            }
            std::mem::swap(&mut dirty, &mut next_dirty);
            if !changed {
                return Ok(program.reconcile_runnable_top_level());
            }
        }
        Err(OptimizerError::MaxIterations {
            max_iterations: self.max_iterations,
            last_pass,
        })
    }

    /// Execute the scheduled passes and return per-pass runtime/IR counters.
    ///
    /// This mirrors [`Self::run`] but retains counters that identify expensive
    /// or clone-heavy passes without requiring a profiler.
    ///
    /// # Errors
    ///
    /// Returns [`OptimizerError`] if pass dependencies are unsatisfied or the
    /// scheduler fails to converge within the configured iteration bound.
    pub fn run_with_metrics(&self, program: Program) -> Result<OptimizerRunReport, OptimizerError> {
        let mut program = program;
        let mut last_pass = "<none>";
        let mut dirty = self.initial_dirty_flags();
        let mut next_dirty = vec![false; self.passes.len()];
        let mut fact_state = SchedulerFactState::default();
        let mut metrics = Vec::with_capacity(
            self.execution_order
                .len()
                .saturating_mul(self.max_iterations),
        );

        for iteration in 0..self.max_iterations {
            next_dirty.fill(false);
            let (next, changed, changed_by) = self.run_once_with_metrics(
                program,
                &dirty,
                &mut next_dirty,
                iteration,
                &mut metrics,
                &mut fact_state,
            )?;
            program = next;
            if let Some(name) = changed_by {
                last_pass = name;
            }
            std::mem::swap(&mut dirty, &mut next_dirty);
            if !changed {
                return Ok(OptimizerRunReport {
                    program: program.reconcile_runnable_top_level(),
                    passes: metrics,
                });
            }
        }
        Err(OptimizerError::MaxIterations {
            max_iterations: self.max_iterations,
            last_pass,
        })
    }

    fn run_once_flags(
        &self,
        mut program: Program,
        dirty: &[bool],
        next_dirty: &mut [bool],
    ) -> Result<(Program, bool, Option<&'static str>), OptimizerError> {
        let mut available = (!self.requirements_prevalidated).then(|| {
            let mut available = FxHashSet::default();
            available.reserve(self.execution_order.len());
            available
        });
        let mut changed = false;
        let mut changed_by = None;
        for &pass_index in &self.execution_order {
            let Some(pass) = self.passes.get(pass_index) else {
                continue;
            };
            let metadata = pass.metadata();
            if let Some(available) = available.as_ref() {
                if !requirements_satisfied(metadata, available) {
                    let missing = metadata
                        .requires
                        .iter()
                        .copied()
                        .find(|requirement| !available.contains(requirement))
                        .unwrap_or("<unknown>");
                    return Err(OptimizerError::UnsatisfiedRequirement {
                        pass: metadata.name,
                        missing,
                    });
                }
            }

            if dirty.get(pass_index).copied().unwrap_or(false) && pass.analyze(&program).should_run
            {
                let enforce_gates = self.enforce_cost_monotone
                    || self.enforce_effect_handlers
                    || self.enforce_linear_types
                    || self.enforce_shape_predicates;
                program = if enforce_gates {
                    let pre_cost = self
                        .enforce_cost_monotone
                        .then(|| crate::optimizer::cost::CostCertificate::for_program(&program));
                    let pre_effects = self
                        .enforce_effect_handlers
                        .then(|| crate::lower::effects::compute_program_effects(&program));
                    let pre_linear_violations = self
                        .enforce_linear_types
                        .then(|| linear_type_violations(&program));
                    let pre_shape_predicate_violations = self
                        .enforce_shape_predicates
                        .then(|| shape_predicate_violations(&program));
                    let pre_snapshot = Clone::clone(&program);
                    match pass.try_batch_apply(program) {
                        Ok(result) => {
                            if result.changed {
                                let mut rejected = false;
                                if let Some(pre_effects) = pre_effects {
                                    let post_effects =
                                        crate::lower::effects::compute_program_effects(
                                            &result.program,
                                        );
                                    rejected = introduces_forbidden_effects(
                                        pre_effects,
                                        post_effects,
                                        pass.allowed_effect_additions(),
                                    );
                                }
                                if !rejected {
                                    if let Some(pre_cost) = pre_cost {
                                        let post_cost =
                                            crate::optimizer::cost::CostCertificate::for_program(
                                                &result.program,
                                            );
                                        if !post_cost.dominates_or_equal(&pre_cost) {
                                            rejected = true;
                                        }
                                    }
                                }
                                if !rejected {
                                    if let Some(pre_linear_violations) = &pre_linear_violations {
                                        let post_linear_violations =
                                            linear_type_violations(&result.program);
                                        if introduces_linear_type_violations(
                                            pre_linear_violations,
                                            &post_linear_violations,
                                        ) {
                                            rejected = true;
                                        }
                                    }
                                }
                                if !rejected {
                                    if let Some(pre_shape_predicate_violations) =
                                        &pre_shape_predicate_violations
                                    {
                                        let post_shape_predicate_violations =
                                            shape_predicate_violations(&result.program);
                                        if introduces_shape_predicate_violations(
                                            pre_shape_predicate_violations,
                                            &post_shape_predicate_violations,
                                        ) {
                                            rejected = true;
                                        }
                                    }
                                }
                                if rejected {
                                    pre_snapshot
                                } else {
                                    changed = true;
                                    changed_by = Some(pass.pass_id());
                                    self.mark_invalidated_pass_flags(
                                        metadata.invalidates,
                                        next_dirty,
                                    );
                                    result.program
                                }
                            } else {
                                result.program
                            }
                        }
                        Err(_refusal) => pre_snapshot,
                    }
                } else {
                    let result = pass.batch_apply(program);
                    if result.changed {
                        changed = true;
                        changed_by = Some(pass.pass_id());
                        self.mark_invalidated_pass_flags(metadata.invalidates, next_dirty);
                    }
                    result.program
                };
            }
            if let Some(available) = available.as_mut() {
                available.insert(metadata.name);
            }
        }

        Ok((program, changed, changed_by))
    }

    #[expect(
        clippy::too_many_lines,
        reason = "scheduler metric collection keeps before/after counters colocated with pass execution"
    )]
    pub(crate) fn run_once_with_metrics(
        &self,
        mut program: Program,
        dirty: &[bool],
        next_dirty: &mut [bool],
        iteration: usize,
        metrics: &mut Vec<PassRunMetric>,
        fact_state: &mut SchedulerFactState,
    ) -> Result<(Program, bool, Option<&'static str>), OptimizerError> {
        let mut available = (!self.requirements_prevalidated).then(|| {
            let mut available = FxHashSet::default();
            available.reserve(self.execution_order.len());
            available
        });
        let mut changed = false;
        let mut changed_by = None;
        let mut cached_allocation_estimate: Option<IrAllocationEstimate> = None;

        for &pass_index in &self.execution_order {
            let Some(pass) = self.passes.get(pass_index) else {
                continue;
            };
            let metadata = pass.metadata();
            if let Some(available) = available.as_ref() {
                if !requirements_satisfied(metadata, available) {
                    let missing = metadata
                        .requires
                        .iter()
                        .copied()
                        .find(|requirement| !available.contains(requirement))
                        .unwrap_or("<unknown>");
                    return Err(OptimizerError::UnsatisfiedRequirement {
                        pass: metadata.name,
                        missing,
                    });
                }
            }

            let before_stats = *program.stats();
            let before_allocations = *cached_allocation_estimate
                .get_or_insert_with(|| estimate_ir_allocations(&program));

            let mut metric = PassRunMetric {
                iteration,
                pass: metadata.name,
                research_trace: self.research_trace_for(metadata.name),
                ran: false,
                changed: false,
                decision: PassRunDecision::CleanSkipped,
                refusal_kind: None,
                required_analyses: metadata.requires,
                declared_invalidations: metadata.invalidates,
                fact_substrate_reused: false,
                fact_substrate_recomputed: false,
                fact_substrate_invalidated: false,
                effect_bits_before: 0,
                effect_bits_after: 0,
                linear_type_violations_before: 0,
                linear_type_violations_after: 0,
                shape_predicate_violations_before: 0,
                shape_predicate_violations_after: 0,
                runtime_ns: 0,
                nodes_before: before_stats.node_count,
                nodes_after: before_stats.node_count,
                static_storage_bytes_before: before_stats.static_storage_bytes,
                static_storage_bytes_after: before_stats.static_storage_bytes,
                instruction_count_before: before_stats.instruction_count,
                instruction_count_after: before_stats.instruction_count,
                memory_op_count_before: before_stats.memory_op_count,
                memory_op_count_after: before_stats.memory_op_count,
                atomic_op_count_before: before_stats.atomic_op_count,
                atomic_op_count_after: before_stats.atomic_op_count,
                control_flow_count_before: before_stats.control_flow_count,
                control_flow_count_after: before_stats.control_flow_count,
                register_pressure_before: before_stats.register_pressure_estimate,
                register_pressure_after: before_stats.register_pressure_estimate,
                ir_heap_allocations_before: before_allocations.allocations,
                ir_heap_allocations_after: before_allocations.allocations,
                ir_heap_bytes_before: before_allocations.bytes,
                ir_heap_bytes_after: before_allocations.bytes,
            };

            if dirty.get(pass_index).copied().unwrap_or(false) {
                let fact_event = fact_state.prepare(&program);
                metric.fact_substrate_reused = fact_event.reused;
                metric.fact_substrate_recomputed = fact_event.recomputed;
                if !pass.analyze(&program).should_run {
                    metric.decision = PassRunDecision::AnalysisSkipped;
                    metrics.push(metric);
                    if let Some(available) = available.as_mut() {
                        available.insert(metadata.name);
                    }
                    continue;
                }
                metric.ran = true;
                let perf_scope = PerfScope::start("vyre-foundation", metadata.name);
                let mut landed_changed = false;
                let enforce_gates = self.enforce_cost_monotone
                    || self.enforce_effect_handlers
                    || self.enforce_linear_types
                    || self.enforce_shape_predicates;
                program = if enforce_gates {
                    let pre_cost_for_gate = self
                        .enforce_cost_monotone
                        .then(|| crate::optimizer::cost::CostCertificate::for_program(&program));
                    let pre_effects_for_gate = if self.enforce_effect_handlers {
                        crate::lower::effects::compute_program_effects(&program)
                    } else {
                        crate::lower::effects::ProgramEffects::empty()
                    };
                    if self.enforce_effect_handlers {
                        metric.effect_bits_before = pre_effects_for_gate.bits();
                    }
                    let pre_linear_violations_for_gate = if self.enforce_linear_types {
                        linear_type_violations(&program)
                    } else {
                        BTreeSet::new()
                    };
                    if self.enforce_linear_types {
                        metric.linear_type_violations_before = pre_linear_violations_for_gate.len();
                    }
                    let pre_shape_predicate_violations_for_gate = if self.enforce_shape_predicates {
                        shape_predicate_violations(&program)
                    } else {
                        BTreeSet::new()
                    };
                    if self.enforce_shape_predicates {
                        metric.shape_predicate_violations_before =
                            pre_shape_predicate_violations_for_gate.len();
                    }
                    let pre_snapshot_for_gate = Clone::clone(&program);

                    let result = pass.try_batch_apply(program);
                    metric.runtime_ns = u128::from(perf_scope.finish().elapsed_ns);
                    match result {
                        Ok(result) if result.changed => {
                            let post_effects = if self.enforce_effect_handlers {
                                crate::lower::effects::compute_program_effects(&result.program)
                            } else {
                                crate::lower::effects::ProgramEffects::empty()
                            };
                            if self.enforce_effect_handlers
                                && introduces_forbidden_effects(
                                    pre_effects_for_gate,
                                    post_effects,
                                    pass.allowed_effect_additions(),
                                )
                            {
                                metric.decision = PassRunDecision::EffectReverted;
                                metric.effect_bits_after = pre_effects_for_gate.bits();
                                metric.linear_type_violations_after =
                                    pre_linear_violations_for_gate.len();
                                metric.shape_predicate_violations_after =
                                    pre_shape_predicate_violations_for_gate.len();
                                pre_snapshot_for_gate
                            } else if self.enforce_linear_types
                                && introduces_linear_type_violations(
                                    &pre_linear_violations_for_gate,
                                    &linear_type_violations(&result.program),
                                )
                            {
                                metric.decision = PassRunDecision::LinearTypeReverted;
                                metric.effect_bits_after = pre_effects_for_gate.bits();
                                metric.linear_type_violations_after =
                                    pre_linear_violations_for_gate.len();
                                metric.shape_predicate_violations_after =
                                    pre_shape_predicate_violations_for_gate.len();
                                pre_snapshot_for_gate
                            } else if self.enforce_shape_predicates
                                && introduces_shape_predicate_violations(
                                    &pre_shape_predicate_violations_for_gate,
                                    &shape_predicate_violations(&result.program),
                                )
                            {
                                metric.decision = PassRunDecision::ShapePredicateReverted;
                                metric.effect_bits_after = pre_effects_for_gate.bits();
                                metric.linear_type_violations_after =
                                    pre_linear_violations_for_gate.len();
                                metric.shape_predicate_violations_after =
                                    pre_shape_predicate_violations_for_gate.len();
                                pre_snapshot_for_gate
                            } else if let Some(pre_cost) = pre_cost_for_gate {
                                let post_cost =
                                    crate::optimizer::cost::CostCertificate::for_program(
                                        &result.program,
                                    );
                                if post_cost.dominates_or_equal(&pre_cost) {
                                    landed_changed = true;
                                    metric.decision = PassRunDecision::Changed;
                                    metric.effect_bits_after = post_effects.bits();
                                    if self.enforce_linear_types {
                                        metric.linear_type_violations_after =
                                            linear_type_violations(&result.program).len();
                                    }
                                    if self.enforce_shape_predicates {
                                        metric.shape_predicate_violations_after =
                                            shape_predicate_violations(&result.program).len();
                                    }
                                    result.program
                                } else {
                                    // Cost-monotone-down violation: a tracked dimension
                                    // increased without an explicit refusal. Drop the
                                    // rewrite, restore the pre-snapshot. The metrics
                                    // captured below reflect the post-revert shape.
                                    metric.decision = PassRunDecision::CostReverted;
                                    metric.effect_bits_after = pre_effects_for_gate.bits();
                                    metric.linear_type_violations_after =
                                        pre_linear_violations_for_gate.len();
                                    metric.shape_predicate_violations_after =
                                        pre_shape_predicate_violations_for_gate.len();
                                    pre_snapshot_for_gate
                                }
                            } else {
                                landed_changed = true;
                                metric.decision = PassRunDecision::Changed;
                                metric.effect_bits_after = post_effects.bits();
                                if self.enforce_linear_types {
                                    metric.linear_type_violations_after =
                                        linear_type_violations(&result.program).len();
                                }
                                if self.enforce_shape_predicates {
                                    metric.shape_predicate_violations_after =
                                        shape_predicate_violations(&result.program).len();
                                }
                                result.program
                            }
                        }
                        Ok(result) => {
                            metric.decision = if result.changed {
                                PassRunDecision::Changed
                            } else {
                                PassRunDecision::RanUnchanged
                            };
                            metric.effect_bits_after = pre_effects_for_gate.bits();
                            metric.linear_type_violations_after =
                                pre_linear_violations_for_gate.len();
                            metric.shape_predicate_violations_after =
                                pre_shape_predicate_violations_for_gate.len();
                            result.program
                        }
                        Err(refusal) => {
                            metric.decision = PassRunDecision::Refused;
                            metric.refusal_kind = Some(refusal.kind());
                            metric.effect_bits_after = pre_effects_for_gate.bits();
                            metric.linear_type_violations_after =
                                pre_linear_violations_for_gate.len();
                            metric.shape_predicate_violations_after =
                                pre_shape_predicate_violations_for_gate.len();
                            pre_snapshot_for_gate
                        }
                    }
                } else {
                    let result = pass.batch_apply(program);
                    metric.runtime_ns = u128::from(perf_scope.finish().elapsed_ns);
                    if result.changed {
                        landed_changed = true;
                        metric.decision = PassRunDecision::Changed;
                        result.program
                    } else {
                        metric.decision = PassRunDecision::RanUnchanged;
                        result.program
                    }
                };
                let after_stats = *program.stats();
                let after_allocations = if landed_changed {
                    estimate_ir_allocations(&program)
                } else {
                    before_allocations
                };
                cached_allocation_estimate = Some(after_allocations);
                metric.nodes_after = after_stats.node_count;
                metric.static_storage_bytes_after = after_stats.static_storage_bytes;
                metric.instruction_count_after = after_stats.instruction_count;
                metric.memory_op_count_after = after_stats.memory_op_count;
                metric.atomic_op_count_after = after_stats.atomic_op_count;
                metric.control_flow_count_after = after_stats.control_flow_count;
                metric.register_pressure_after = after_stats.register_pressure_estimate;
                metric.ir_heap_allocations_after = after_allocations.allocations;
                metric.ir_heap_bytes_after = after_allocations.bytes;
                // Reflect post-gate state. Expression-only rewrites often keep
                // the same node count; they still invalidate downstream facts.
                metric.changed = landed_changed;
                if metric.changed {
                    changed = true;
                    changed_by = Some(pass.pass_id());
                    metric.fact_substrate_invalidated = fact_state.invalidate();
                    self.mark_invalidated_pass_flags(metadata.invalidates, next_dirty);
                }
            }
            metrics.push(metric);
            if let Some(available) = available.as_mut() {
                available.insert(metadata.name);
            }
        }

        Ok((program, changed, changed_by))
    }

    fn initial_dirty_flags(&self) -> Vec<bool> {
        self.initial_dirty_flags_cache
            .get_or_init(|| vec![true; self.passes.len()])
            .clone()
    }
}
