//! Integration test crate for the containing Vyre package.

use super::*;

#[test]
fn invalidation_marks_named_pass_and_requirement_dependents_dirty() {
    let scheduler = PassScheduler::with_passes(vec![
        ProgramPassKind::new(ConstFold),
        ProgramPassKind::new(StrengthReduce),
        ProgramPassKind::new(NormalizeAtomicsPass),
        ProgramPassKind::new(Fusion),
    ]);

    let mut dirty = FxHashSet::default();
    scheduler.mark_invalidated_passes(&["fusion"], &mut dirty);
    assert!(
        dirty.contains("fusion"),
        "pass-name invalidation must rerun that pass"
    );

    dirty.clear();
    scheduler.mark_invalidated_passes(&["const_fold"], &mut dirty);
    assert!(dirty.contains("const_fold"));
    assert!(
        dirty.contains("strength_reduce"),
        "passes requiring an invalidated pass/capability must rerun"
    );
}

#[test]
fn invalidating_prior_requirement_does_not_break_current_iteration() {
    let scheduler = PassScheduler::with_passes(vec![
        ProgramPassKind::new(TestPass {
            metadata: PassMetadata::new("prepare", &[], &[]),
            changes: false,
        }),
        ProgramPassKind::new(TestPass {
            metadata: PassMetadata::new("rewrite", &[], &["prepare"]),
            changes: true,
        }),
        ProgramPassKind::new(TestPass {
            metadata: PassMetadata::new("consume", &["prepare"], &[]),
            changes: false,
        }),
    ]);
    let report = scheduler
        .run_with_metrics(trivial_program())
        .expect("Fix: invalidating a prior requirement must queue a rerun, not make later passes unschedulable");

    assert!(
        report.passes.len() >= 6,
        "first iteration must queue prepare and consume for a second dirty-flag iteration"
    );
    assert!(
        report
            .passes
            .iter()
            .any(|metric| metric.iteration == 0 && metric.pass == "rewrite" && metric.changed),
        "the rewrite pass must land a change during the first metrics iteration"
    );
    assert_eq!(report.passes[3].pass, "prepare");
    assert!(
        report.passes[3].ran,
        "invalidating `prepare` must rerun the named pass on the next metrics iteration"
    );
    assert!(
        report
            .passes
            .iter()
            .any(|metric| metric.iteration == 1 && metric.pass == "consume" && metric.ran),
        "invalidating `prepare` must rerun dependents that require it"
    );
}

#[test]
fn run_with_metrics_tracks_expression_only_rewrites() {
    let scheduler = PassScheduler::with_passes(vec![ProgramPassKind::new(ExprOnlyPass {
        metadata: PassMetadata::new("expr_only", &[], &["value_numbering"]),
    })]);

    let report = scheduler
        .run_with_metrics(trivial_program())
        .expect("Fix: metrics run must converge for expression-only rewrites");
    assert_eq!(report.passes.len(), 2);
    let first = &report.passes[0];
    assert_eq!(first.pass, "expr_only");
    assert!(
        first.changed,
        "expression-only rewrites keep node_count stable but still changed the program and must invalidate downstream facts"
    );
    assert_eq!(
        first.nodes_before, first.nodes_after,
        "the regression target is a same-node-count expression rewrite"
    );
    assert!(
        !report.passes[1].changed,
        "the second iteration must observe convergence after the expression rewrite landed"
    );
}

#[test]
fn scheduler_fact_substrate_reuses_read_only_passes_and_invalidates_mutations() {
    let scheduler = PassScheduler::with_passes(vec![
        ProgramPassKind::new(TestPass {
            metadata: PassMetadata::new("read_a", &[], &[]),
            changes: false,
        }),
        ProgramPassKind::new(TestPass {
            metadata: PassMetadata::new("read_b", &["read_a"], &[]),
            changes: false,
        }),
        ProgramPassKind::new(StoreValueRewritePass {
            metadata: PassMetadata::new("mutate_a", &["read_b"], &[]),
            from: 42,
            to: 43,
        }),
        ProgramPassKind::new(StoreValueRewritePass {
            metadata: PassMetadata::new("mutate_b", &["mutate_a"], &[]),
            from: 43,
            to: 44,
        }),
    ]);

    let report = scheduler
        .run_with_metrics(trivial_program())
        .expect("Fix: scheduler fact-substrate metric run must converge");
    let first_iter = report
        .passes
        .iter()
        .filter(|metric| metric.iteration == 0)
        .collect::<Vec<_>>();
    assert_eq!(first_iter.len(), 4);

    assert_eq!(first_iter[0].pass, "read_a");
    assert!(first_iter[0].fact_substrate_recomputed);
    assert!(!first_iter[0].fact_substrate_reused);
    assert!(!first_iter[0].fact_substrate_invalidated);

    assert_eq!(first_iter[1].pass, "read_b");
    assert!(first_iter[1].fact_substrate_reused);
    assert!(!first_iter[1].fact_substrate_recomputed);
    assert!(!first_iter[1].fact_substrate_invalidated);

    assert_eq!(first_iter[2].pass, "mutate_a");
    assert!(first_iter[2].fact_substrate_reused);
    assert!(!first_iter[2].fact_substrate_recomputed);
    assert!(first_iter[2].fact_substrate_invalidated);

    assert_eq!(first_iter[3].pass, "mutate_b");
    assert!(!first_iter[3].fact_substrate_reused);
    assert!(first_iter[3].fact_substrate_recomputed);
    assert!(first_iter[3].fact_substrate_invalidated);

    assert_eq!(
        first_iter
            .iter()
            .filter(|metric| metric.fact_substrate_recomputed)
            .count(),
        2,
        "initial read and post-mutation pass should be the only first-iteration fact recomputes"
    );
    assert_eq!(
        first_iter
            .iter()
            .filter(|metric| metric.fact_substrate_reused)
            .count(),
        2,
        "read_b and mutate_a should reuse the scheduler-owned facts"
    );
    assert_eq!(
        first_iter
            .iter()
            .filter(|metric| metric.fact_substrate_invalidated)
            .count(),
        2,
        "each landed mutation should invalidate the scheduler-owned facts once"
    );
}
