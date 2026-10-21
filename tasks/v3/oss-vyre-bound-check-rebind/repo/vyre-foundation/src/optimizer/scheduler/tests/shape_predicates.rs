//! Shape-predicate scheduler enforcement.

use super::*;
use crate::validate::shape_predicate::check_shape_predicates;

fn shape_gate_contract() -> SchedulerGateContract {
    SchedulerGateContract {
        build_breaking_pass: |metadata| ProgramPassKind::new(ShapeBreakingPass { metadata }),
        build_preserving_pass: |metadata| ProgramPassKind::new(ExprOnlyPass { metadata }),
        program: shape_predicate_program,
        enable: |scheduler| scheduler.with_shape_predicate_enforcement(true),
        is_enabled: PassScheduler::shape_predicate_enforcement,
        check_violations: |program| check_shape_predicates(program).len(),
        reverted_decision: PassRunDecision::ShapePredicateReverted,
        violation_counts: |metric| {
            (
                metric.shape_predicate_violations_before,
                metric.shape_predicate_violations_after,
            )
        },
    }
}

#[test]
fn shape_predicate_enforcement_disabled_by_default_keeps_shape_breaking_rewrites() {
    assert_gate_disabled_by_default_keeps_breaking_rewrite(
        &shape_gate_contract(),
        "shape_break_default_off",
        "shape-predicate enforcement must default to OFF for compatibility",
        "Fix: scheduler must converge",
        "with the gate disabled, a shape-breaking rewrite still lands",
    );
}

#[test]
fn shape_predicate_enforcement_reverts_new_shape_violations() {
    assert_gate_reverts_new_violations(
        &shape_gate_contract(),
        "shape_break_forbidden",
        "Fix: shape-predicate revert must converge",
        "shape-predicate gate must revert rewrites that break declared BufferDecl refinements",
    );
}

#[test]
fn shape_predicate_enforcement_metrics_reflect_post_revert_state() {
    assert_gate_revert_metrics_reflect_post_revert_state(
        &shape_gate_contract(),
        "shape_break_metric_check",
        "Fix: metrics run must converge",
        "shape-breaking pass must have run",
        "reverted shape-predicate violations must not land",
    );
}

#[test]
fn shape_predicate_enforcement_allows_shape_preserving_rewrites() {
    assert_gate_allows_preserving_rewrites(
        &shape_gate_contract(),
        "shape_preserving_expr_rewrite",
        "Fix: shape-preserving rewrite must converge",
        "value-only rewrite should land",
    );
}

#[test]
fn shape_predicate_enforcement_allows_repairs_of_existing_shape_violations() {
    let scheduler = PassScheduler::with_passes(vec![ProgramPassKind::new(ShapeRepairingPass {
        metadata: PassMetadata::new("shape_repair_existing_violation", &[], &[]),
    })])
    .with_shape_predicate_enforcement(true);

    let pre = invalid_shape_predicate_program();
    assert_eq!(
        check_shape_predicates(&pre).len(),
        1,
        "Fix: repair test must start from a genuine shape-predicate violation"
    );

    let report = scheduler
        .run_with_metrics(pre)
        .expect("Fix: shape-repairing rewrite must converge");
    let metric = first_ran_metric(&report);

    assert!(metric.changed, "repairing rewrite should land");
    assert_eq!(metric.decision, PassRunDecision::Changed);
    assert_eq!(metric.shape_predicate_violations_before, 1);
    assert_eq!(metric.shape_predicate_violations_after, 0);
    assert!(
        check_shape_predicates(&report.program).is_empty(),
        "shape-predicate gate must allow rewrites that reduce existing violations"
    );
}
