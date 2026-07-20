use crate::quick::{infer_quick_laws::infer_quick_laws, QuickOp, QuickStatus};

pub(crate) fn infer_laws(op: &QuickOp) -> (QuickStatus, String) {
    let inferred = infer_quick_laws(op);
    let missing: Vec<String> = inferred
        .iter()
        .filter(|law| !op.laws.iter().any(|declared| declared == *law))
        .map(|law| law.recommendation())
        .collect();

    if missing.is_empty() {
        (
            QuickStatus::Pass,
            format!("{} inferred laws already declared", inferred.len()),
        )
    } else {
        (
            QuickStatus::Fail,
            format!("missing inferred laws: {}", missing.join("; ")),
        )
    }
}
