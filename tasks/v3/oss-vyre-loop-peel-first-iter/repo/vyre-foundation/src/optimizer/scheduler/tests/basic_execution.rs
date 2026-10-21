//! Integration test crate for the containing Vyre package.

use super::*;

#[test]
fn single_pass_converges() {
    let scheduler = PassScheduler::with_passes(vec![ProgramPassKind::new(ConstFold)]);
    let program = scheduler
        .run(trivial_program())
        .expect("single const_fold pass must converge");
    assert_eq!(program.workgroup_size(), [1, 1, 1]);
}

#[test]
fn run_with_metrics_reports_pass_runtime_and_ir_size() {
    let scheduler = PassScheduler::with_passes(vec![ProgramPassKind::new(ConstFold)]);
    let report = scheduler
        .run_with_metrics(trivial_program())
        .expect("Fix: metrics run should converge");

    assert_eq!(report.passes.len(), 1);
    let metric = &report.passes[0];
    assert_eq!(metric.pass, "const_fold");
    assert!(
        metric.ran,
        "const_fold should run on the first dirty iteration"
    );
    assert!(metric.nodes_before > 0);
    assert!(metric.nodes_after > 0);
    assert!(
        metric.ir_heap_allocations_before > 0,
        "metrics must include IR heap allocation pressure"
    );
    assert!(
        metric.ir_heap_bytes_before > 0,
        "metrics must include estimated IR heap bytes"
    );
    assert_eq!(
        report.program.stats().node_count,
        metric.nodes_after,
        "metric after-count must describe the returned program"
    );
}

#[test]
fn max_iterations_caps_execution() {
    // A scheduler with 0 max iterations must return MaxIterations error
    // if any pass reports changed = true. Use a program that const_fold
    // will actually change.
    let program = Program::wrapped(
        vec![BufferDecl::read_write("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::add(Expr::u32(1), Expr::u32(2)),
        )],
    );
    let scheduler =
        PassScheduler::with_passes(vec![ProgramPassKind::new(ConstFold)]).with_max_iterations(0);
    let result = scheduler.run(program);
    assert!(
        matches!(result, Err(OptimizerError::MaxIterations { .. })),
        "zero iterations should immediately hit max: {:?}",
        result
    );
}

#[test]
fn idempotent_pass_converges_in_two_iterations() {
    // const_fold on `3 + 4` should produce `7` in iteration 1,
    // then iteration 2 finds no changes → convergence.
    let program = Program::wrapped(
        vec![BufferDecl::read_write("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::add(Expr::u32(3), Expr::u32(4)),
        )],
    );
    let scheduler =
        PassScheduler::with_passes(vec![ProgramPassKind::new(ConstFold)]).with_max_iterations(2);
    let program = scheduler
        .run(program)
        .expect("should converge within 2 iterations");
    // Region + Store (node_count counts structural nodes; folding 3+4->7
    // rewrites the store's expr but leaves the store node). See
    // ir_inner::model::program::stats_test for the counting contract.
    assert_eq!(program.stats().node_count, 2);
}

#[test]
fn multiple_passes_execute() {
    let scheduler = PassScheduler::with_passes(vec![
        ProgramPassKind::new(ConstFold),
        ProgramPassKind::new(StrengthReduce),
    ]);
    let program = scheduler
        .run(trivial_program())
        .expect("const_fold then strength_reduce must converge");
    // Region + Store (trivial_program is a single store; passes rewrite its
    // expr but leave the store node). See stats_test for the contract.
    assert_eq!(program.stats().node_count, 2);
}

#[test]
fn with_max_iterations_is_configurable() {
    let scheduler =
        PassScheduler::with_passes(vec![ProgramPassKind::new(ConstFold)]).with_max_iterations(100);
    assert_eq!(scheduler.max_iterations, 100);
}

#[test]
fn default_scheduler_uses_registered_passes() {
    // The default scheduler should include every built-in pass.
    let scheduler = PassScheduler::default();
    assert!(
        scheduler.passes.len() >= 9,
        "must include at least 9 built-in passes, got {}",
        scheduler.passes.len()
    );
}

#[test]
fn transitive_dependents_unknown_pass_returns_empty() {
    let scheduler = PassScheduler::with_passes(vec![ProgramPassKind::new(ConstFold)]);
    assert!(scheduler.transitive_dependents("nonexistent").is_empty());
}

#[test]
fn reaches_unknown_pass_returns_false() {
    let scheduler = PassScheduler::with_passes(vec![ProgramPassKind::new(ConstFold)]);
    assert!(!scheduler.reaches("nonexistent", "const_fold"));
    assert!(!scheduler.reaches("const_fold", "nonexistent"));
}

#[test]
fn pair_commutes_same_pass_is_true() {
    let scheduler = PassScheduler::with_passes(vec![ProgramPassKind::new(ConstFold)]);
    assert!(scheduler.pair_commutes("const_fold", "const_fold"));
}

#[test]
fn pair_commutes_for_disjoint_invalidation_domains() {
    let scheduler = PassScheduler::with_passes(vec![
        ProgramPassKind::new(TestPass {
            metadata: PassMetadata::new("shape_cleanup", &[], &["shape_fact"]),
            changes: false,
        }),
        ProgramPassKind::new(TestPass {
            metadata: PassMetadata::new("memory_cleanup", &[], &["memory_fact"]),
            changes: false,
        }),
    ]);
    assert!(
        scheduler.pair_commutes("shape_cleanup", "memory_cleanup"),
        "passes with disjoint invalidation domains must be safely reorderable"
    );
}

#[test]
fn pair_commutes_rejects_requirement_invalidation() {
    let scheduler = PassScheduler::with_passes(vec![
        ProgramPassKind::new(TestPass {
            metadata: PassMetadata::new("shape_cleanup", &[], &["shape_fact"]),
            changes: false,
        }),
        ProgramPassKind::new(TestPass {
            metadata: PassMetadata::new("shape_consumer", &["shape_fact"], &[]),
            changes: false,
        }),
    ]);
    assert!(
        !scheduler.pair_commutes("shape_cleanup", "shape_consumer"),
        "a pass must not commute across another pass's required capability if it invalidates that capability"
    );
}
