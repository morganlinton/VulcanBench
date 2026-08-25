use crate::quick::{law_violation::law_violation, QuickOp, QuickStatus};

pub(crate) fn audit_laws(op: &QuickOp) -> (QuickStatus, String) {
    if op.laws.is_empty() {
        return (QuickStatus::Skip, "no declared laws".to_string());
    }

    let failures: Vec<String> = op
        .laws
        .iter()
        .filter_map(|law| {
            law_violation(op, *law).map(|w| format!("{} violated at {w}", law.name()))
        })
        .collect();

    if failures.is_empty() {
        (
            QuickStatus::Pass,
            format!("{} declared laws confirmed", op.laws.len()),
        )
    } else {
        (QuickStatus::Fail, failures.join("; "))
    }
}
