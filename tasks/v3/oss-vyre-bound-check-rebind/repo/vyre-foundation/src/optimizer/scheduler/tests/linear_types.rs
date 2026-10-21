//! Linear-type scheduler enforcement.

use super::*;
use crate::validate::linear_type::check_linear_types;

fn linear_gate_contract() -> SchedulerGateContract {
    SchedulerGateContract {
        build_breaking_pass: |metadata| ProgramPassKind::new(LinearBreakingPass { metadata }),
        build_preserving_pass: |metadata| ProgramPassKind::new(ExprOnlyPass { metadata }),
        program: linear_program,
        enable: |scheduler| scheduler.with_linear_type_enforcement(true),
        is_enabled: PassScheduler::linear_type_enforcement,
        check_violations: |program| check_linear_types(program).len(),
        reverted_decision: PassRunDecision::LinearTypeReverted,
        violation_counts: |metric| {
            (
                metric.linear_type_violations_before,
                metric.linear_type_violations_after,
            )
        },
    }
}

#[test]
fn linear_type_enforcement_disabled_by_default_keeps_linear_breaking_rewrites() {
    assert_gate_disabled_by_default_keeps_breaking_rewrite(
        &linear_gate_contract(),
        "linear_break_default_off",
        "linear-type enforcement must default to OFF for compatibility",
        "Fix: scheduler must converge",
        "with the gate disabled, a linear-breaking rewrite still lands",
    );
}

#[test]
fn linear_type_enforcement_reverts_new_linear_violations() {
    assert_gate_reverts_new_violations(
        &linear_gate_contract(),
        "linear_break_forbidden",
        "Fix: linear-type revert must converge",
        "linear-type gate must revert rewrites that break declared BufferAccess discipline",
    );
}

#[test]
fn linear_type_enforcement_metrics_reflect_post_revert_state() {
    assert_gate_revert_metrics_reflect_post_revert_state(
        &linear_gate_contract(),
        "linear_break_metric_check",
        "Fix: metrics run must converge",
        "linear-breaking pass must have run",
        "reverted linear-type violations must not land",
    );
}

#[test]
fn linear_type_enforcement_allows_linear_preserving_rewrites() {
    assert_gate_allows_preserving_rewrites(
        &linear_gate_contract(),
        "linear_preserving_expr_rewrite",
        "Fix: linear-preserving rewrite must converge",
        "value-only rewrite should land",
    );
}
