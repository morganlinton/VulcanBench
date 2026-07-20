//! Minimal quick-check path.

use std::time::Instant;

pub(crate) mod audit_laws;
mod binary_law;
pub(crate) mod cmd_quick_check;
mod eval_xor;
mod format_seconds;
mod impl_quicklaw;
mod impl_quickstatus;
pub(crate) mod infer_laws;
mod infer_quick_laws;
mod law_violation;
mod load_spec_by_id;
mod located_spec;
mod print_quick_report;
mod push_stage;
pub(crate) mod quick_law;
pub(crate) mod quick_op;
mod quick_report;
mod quick_stage;
pub(crate) mod quick_status;
mod scoped_category_check;

pub(crate) use audit_laws::audit_laws;
pub(crate) use cmd_quick_check::cmd_quick_check;
pub(crate) use infer_laws::infer_laws;
pub(crate) use print_quick_report::print_quick_report;
pub(crate) use quick_law::QuickLaw;
pub(crate) use quick_op::QuickOp;
pub(crate) use quick_status::QuickStatus;

pub(crate) fn run_quick_check(op_id: &str) -> quick_report::QuickReport {
    let start = Instant::now();
    let mut stages: Vec<quick_stage::QuickStage> = Vec::new();

    let Some(located) = load_spec_by_id::load_spec_by_id(op_id) else {
        return quick_report::QuickReport {
            op_id: op_id.to_string(),
            stages,
            total: start.elapsed(),
            pass: false,
            reason: Some(format!("unknown op: {op_id}")),
        };
    };

    push_stage::push_stage(&mut stages, "category-check", || {
        scoped_category_check::scoped_category_check(&located.spec, &located.source_file)
    });
    push_stage::push_stage(&mut stages, "audit-laws", || {
        audit_laws::audit_laws(&located.spec)
    });
    push_stage::push_stage(&mut stages, "infer-laws", || {
        infer_laws::infer_laws(&located.spec)
    });

    let pass = stages
        .iter()
        .all(|s| matches!(s.status, quick_status::QuickStatus::Pass));
    let reason = if pass {
        None
    } else {
        Some(
            stages
                .iter()
                .find(|s| !matches!(s.status, quick_status::QuickStatus::Pass))
                .map(|s| s.detail.clone())
                .unwrap_or_else(|| "unknown failure".to_string()),
        )
    };

    quick_report::QuickReport {
        op_id: op_id.to_string(),
        stages,
        total: start.elapsed(),
        pass,
        reason,
    }
}
