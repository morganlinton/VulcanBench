//! Tests for `scheduler.rs`. Split out per audit item #85 to keep the
//! parent file focused on production code.

use super::*;
use crate::ir::{BufferDecl, DataType, Expr, Node, Program, ShapePredicate};
use crate::ir_inner::model::program::LinearType;
use crate::lower::effects::ProgramEffects;
use crate::optimizer::passes::const_fold::ConstFold;
use crate::optimizer::passes::fusion::Fusion;
use crate::optimizer::passes::normalize_atomics::NormalizeAtomicsPass;
use crate::optimizer::passes::strength_reduce::StrengthReduce;
use crate::optimizer::{
    PassAnalysis, PassMetadata, PassResult, ProgramPass, RefusalReason, RewriteBatch,
    RewriteBatchCandidates, RewriteCandidate,
};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

mod rewrite_support;
use rewrite_support::*;

fn trivial_program() -> Program {
    Program::wrapped(
        vec![BufferDecl::read_write("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::u32(42))],
    )
}

fn linear_program() -> Program {
    Program::wrapped(
        vec![BufferDecl::read_write("out", 0, DataType::U32)
            .with_count(1)
            .with_linear_type(LinearType::Linear)],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::u32(42))],
    )
}

fn shape_predicate_program() -> Program {
    Program::wrapped(
        vec![BufferDecl::read_write("out", 0, DataType::U32)
            .with_count(64)
            .with_shape_predicate(ShapePredicate::MultipleOf(64))],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::u32(42))],
    )
}

fn invalid_shape_predicate_program() -> Program {
    Program::wrapped(
        vec![BufferDecl::read_write("out", 0, DataType::U32)
            .with_count(63)
            .with_shape_predicate(ShapePredicate::MultipleOf(64))],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::u32(42))],
    )
}

#[derive(Debug)]
struct TestPass {
    metadata: PassMetadata,
    changes: bool,
}

impl crate::optimizer::private::Sealed for TestPass {}

impl ProgramPass for TestPass {
    fn metadata(&self) -> PassMetadata {
        self.metadata
    }

    fn analyze(&self, _program: &Program) -> PassAnalysis {
        PassAnalysis::RUN
    }

    fn transform(&self, program: Program) -> PassResult {
        if self.changes {
            let mut entry = Clone::clone(&program).into_entry_vec();
            entry.push(Node::barrier());
            PassResult {
                program: program.with_rewritten_entry(entry),
                changed: true,
            }
        } else {
            PassResult::unchanged(program)
        }
    }

    fn fingerprint(&self, _program: &Program) -> u64 {
        0
    }
}

#[derive(Debug)]
struct BarrierAddingPass {
    metadata: PassMetadata,
    allowed: ProgramEffects,
}

impl crate::optimizer::private::Sealed for BarrierAddingPass {}

impl ProgramPass for BarrierAddingPass {
    fn metadata(&self) -> PassMetadata {
        self.metadata
    }

    fn analyze(&self, _program: &Program) -> PassAnalysis {
        PassAnalysis::RUN
    }

    fn transform(&self, program: Program) -> PassResult {
        let mut entry = Clone::clone(&program).into_entry_vec();
        entry.push(Node::barrier());
        PassResult {
            program: program.with_rewritten_entry(entry),
            changed: true,
        }
    }

    fn allowed_effect_additions(&self) -> ProgramEffects {
        self.allowed
    }

    fn fingerprint(&self, _program: &Program) -> u64 {
        0
    }
}

#[derive(Debug)]
struct LinearBreakingPass {
    metadata: PassMetadata,
}

impl crate::optimizer::private::Sealed for LinearBreakingPass {}

impl ProgramPass for LinearBreakingPass {
    fn metadata(&self) -> PassMetadata {
        self.metadata
    }

    fn analyze(&self, _program: &Program) -> PassAnalysis {
        PassAnalysis::RUN
    }

    fn transform(&self, program: Program) -> PassResult {
        let mut entry = Clone::clone(&program).into_entry_vec();
        entry.push(Node::store("out", Expr::u32(0), Expr::u32(7)));
        PassResult {
            program: program.with_rewritten_entry(entry),
            changed: true,
        }
    }

    fn fingerprint(&self, _program: &Program) -> u64 {
        0
    }
}

#[derive(Debug)]
struct ShapeBreakingPass {
    metadata: PassMetadata,
}

impl crate::optimizer::private::Sealed for ShapeBreakingPass {}

impl ProgramPass for ShapeBreakingPass {
    fn metadata(&self) -> PassMetadata {
        self.metadata
    }

    fn analyze(&self, _program: &Program) -> PassAnalysis {
        PassAnalysis::RUN
    }

    fn transform(&self, program: Program) -> PassResult {
        let mut buffers = program.buffers().to_vec();
        if let Some(buffer) = buffers.first_mut() {
            *buffer = buffer.clone().with_count(63);
        }
        PassResult {
            program: program.with_rewritten_buffers(buffers),
            changed: true,
        }
    }

    fn fingerprint(&self, _program: &Program) -> u64 {
        0
    }
}

#[derive(Debug)]
struct ShapeRepairingPass {
    metadata: PassMetadata,
}

impl crate::optimizer::private::Sealed for ShapeRepairingPass {}

impl ProgramPass for ShapeRepairingPass {
    fn metadata(&self) -> PassMetadata {
        self.metadata
    }

    fn analyze(&self, _program: &Program) -> PassAnalysis {
        PassAnalysis::RUN
    }

    fn transform(&self, program: Program) -> PassResult {
        let mut buffers = program.buffers().to_vec();
        if let Some(buffer) = buffers.first_mut() {
            *buffer = buffer.clone().with_count(64);
        }
        PassResult {
            program: program.with_rewritten_buffers(buffers),
            changed: true,
        }
    }

    fn fingerprint(&self, _program: &Program) -> u64 {
        0
    }
}

#[derive(Debug)]
struct ExprOnlyPass {
    metadata: PassMetadata,
}

impl crate::optimizer::private::Sealed for ExprOnlyPass {}

impl ProgramPass for ExprOnlyPass {
    fn metadata(&self) -> PassMetadata {
        self.metadata
    }

    fn analyze(&self, _program: &Program) -> PassAnalysis {
        PassAnalysis::RUN
    }

    fn transform(&self, program: Program) -> PassResult {
        let mut entry = Clone::clone(&program).into_entry_vec();
        if rewrite_first_store_value(&mut entry) {
            return PassResult {
                program: program.with_rewritten_entry(entry),
                changed: true,
            };
        }
        PassResult::unchanged(program)
    }

    fn fingerprint(&self, _program: &Program) -> u64 {
        0
    }
}

#[derive(Debug)]
struct StoreValueRewritePass {
    metadata: PassMetadata,
    from: u32,
    to: u32,
}

impl crate::optimizer::private::Sealed for StoreValueRewritePass {}

impl ProgramPass for StoreValueRewritePass {
    fn metadata(&self) -> PassMetadata {
        self.metadata
    }

    fn analyze(&self, _program: &Program) -> PassAnalysis {
        PassAnalysis::RUN
    }

    fn transform(&self, program: Program) -> PassResult {
        let mut entry = Clone::clone(&program).into_entry_vec();
        if rewrite_store_values(&mut entry, self.from, self.to) {
            return PassResult {
                program: program.with_rewritten_entry(entry),
                changed: true,
            };
        }
        PassResult::unchanged(program)
    }

    fn fingerprint(&self, _program: &Program) -> u64 {
        u64::from(self.from) << 32 | u64::from(self.to)
    }
}

#[derive(Debug)]
struct SkipPass;

impl crate::optimizer::private::Sealed for SkipPass {}

impl ProgramPass for SkipPass {
    fn metadata(&self) -> PassMetadata {
        PassMetadata::new("skip_pass", &[], &[])
    }

    fn analyze(&self, _program: &Program) -> PassAnalysis {
        PassAnalysis::SKIP
    }

    fn transform(&self, program: Program) -> PassResult {
        PassResult::unchanged(program)
    }

    fn fingerprint(&self, _program: &Program) -> u64 {
        0
    }
}

#[derive(Debug)]
struct RefusingPass {
    metadata: PassMetadata,
}

impl crate::optimizer::private::Sealed for RefusingPass {}

impl ProgramPass for RefusingPass {
    fn metadata(&self) -> PassMetadata {
        self.metadata
    }

    fn analyze(&self, _program: &Program) -> PassAnalysis {
        PassAnalysis::RUN
    }

    fn transform(&self, _program: Program) -> PassResult {
        panic!("cost-monotone scheduler must call try_transform before transform")
    }

    fn try_transform(&self, _program: Program) -> Result<PassResult, RefusalReason> {
        Err(RefusalReason::CostIncrease {
            delta: 1,
            detail: "test pass refuses cost-up rewrite",
        })
    }

    fn fingerprint(&self, _program: &Program) -> u64 {
        0
    }
}

#[derive(Debug)]
struct BatchingPass {
    batch_calls: Arc<AtomicUsize>,
    transform_calls: Arc<AtomicUsize>,
    threshold: usize,
}

impl crate::optimizer::private::Sealed for BatchingPass {}

impl ProgramPass for BatchingPass {
    fn metadata(&self) -> PassMetadata {
        PassMetadata::new("batching_pass", &[], &[])
    }

    fn analyze(&self, _program: &Program) -> PassAnalysis {
        PassAnalysis::RUN
    }

    fn transform(&self, program: Program) -> PassResult {
        self.transform_calls.fetch_add(1, Ordering::Relaxed);
        rewrite_matching_stores(program, None)
    }

    fn supports_planar_batching(&self) -> bool {
        true
    }

    fn rewrite_candidates(&self, program: &Program) -> RewriteBatchCandidates {
        let mut candidates = Vec::new();
        collect_store_candidates(program.entry(), &mut candidates);
        let width = candidates.len() as u32;
        RewriteBatchCandidates::new(candidates, 1, width, 2).with_threshold(self.threshold)
    }

    fn apply_rewrite_batch(&self, program: Program, batch: &RewriteBatch) -> PassResult {
        self.batch_calls.fetch_add(1, Ordering::Relaxed);
        rewrite_matching_stores(program, Some(batch))
    }

    fn fingerprint(&self, _program: &Program) -> u64 {
        0
    }
}


struct SchedulerGateContract {
    build_breaking_pass: fn(PassMetadata) -> ProgramPassKind,
    build_preserving_pass: fn(PassMetadata) -> ProgramPassKind,
    program: fn() -> Program,
    enable: fn(PassScheduler) -> PassScheduler,
    is_enabled: fn(&PassScheduler) -> bool,
    check_violations: fn(&Program) -> usize,
    reverted_decision: PassRunDecision,
    violation_counts: fn(&PassRunMetric) -> (usize, usize),
}

fn assert_gate_disabled_by_default_keeps_breaking_rewrite(
    gate: &SchedulerGateContract,
    pass_id: &'static str,
    default_message: &str,
    run_message: &str,
    violation_message: &str,
) {
    let scheduler = PassScheduler::with_passes(vec![(gate.build_breaking_pass)(
        PassMetadata::new(pass_id, &[], &[]),
    )]);
    assert!(!(gate.is_enabled)(&scheduler), "{default_message}");

    let post = scheduler.run((gate.program)()).expect(run_message);
    assert!((gate.check_violations)(&post) > 0, "{violation_message}");
}

fn assert_gate_reverts_new_violations(
    gate: &SchedulerGateContract,
    pass_id: &'static str,
    run_message: &str,
    revert_message: &str,
) {
    let scheduler = (gate.enable)(PassScheduler::with_passes(vec![
        (gate.build_breaking_pass)(PassMetadata::new(pass_id, &[], &[])),
    ]));

    let post = scheduler.run((gate.program)()).expect(run_message);
    assert_eq!((gate.check_violations)(&post), 0, "{revert_message}");
}

fn assert_gate_revert_metrics_reflect_post_revert_state(
    gate: &SchedulerGateContract,
    pass_id: &'static str,
    run_message: &str,
    ran_message: &str,
    changed_message: &str,
) {
    let scheduler = (gate.enable)(PassScheduler::with_passes(vec![
        (gate.build_breaking_pass)(PassMetadata::new(pass_id, &[], &[])),
    ]));

    let report = scheduler
        .run_with_metrics((gate.program)())
        .expect(run_message);
    assert_eq!(report.passes.len(), 1);
    let metric = &report.passes[0];

    assert!(metric.ran, "{ran_message}");
    assert!(!metric.changed, "{changed_message}");
    assert_eq!(metric.decision, gate.reverted_decision);
    let (violations_before, violations_after) = (gate.violation_counts)(metric);
    assert_eq!(violations_before, 0);
    assert_eq!(violations_after, 0);
    assert_eq!(metric.refusal_kind, None);
}

fn assert_gate_allows_preserving_rewrites(
    gate: &SchedulerGateContract,
    pass_id: &'static str,
    run_message: &str,
    changed_message: &str,
) {
    let scheduler = (gate.enable)(PassScheduler::with_passes(vec![(gate
        .build_preserving_pass)(
        PassMetadata::new(pass_id, &[], &[]),
    )]));

    let report = scheduler
        .run_with_metrics((gate.program)())
        .expect(run_message);
    let metric = first_ran_metric(&report);

    assert!(metric.changed, "{changed_message}");
    assert_eq!(metric.decision, PassRunDecision::Changed);
    let (violations_before, violations_after) = (gate.violation_counts)(metric);
    assert_eq!(violations_before, 0);
    assert_eq!(violations_after, 0);
}

fn first_ran_metric(report: &OptimizerRunReport) -> &PassRunMetric {
    report
        .passes
        .iter()
        .find(|metric| metric.ran)
        .expect("Fix: preserving rewrite should produce one ran metric row")
}

mod basic_execution;
mod batching;
mod cost_monotone;
mod effect_handlers;
mod invalidation_metrics;
mod linear_types;
mod lookup_identity;
mod shape_predicates;
