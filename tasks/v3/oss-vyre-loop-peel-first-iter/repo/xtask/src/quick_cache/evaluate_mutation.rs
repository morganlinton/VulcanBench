#![allow(
    missing_docs,
    dead_code,
    unused_imports,
    unused_variables,
    unreachable_patterns,
    clippy::all
)]
use crate::quick::{audit_laws, infer_laws, QuickOp, QuickStatus};
use crate::quick_cache::QuickMutation;

pub(crate) fn evaluate_mutation(
    op: &QuickOp,
    source: &str,
    mutation: QuickMutation,
) -> &'static str {
    if !source.contains(mutation.from) {
        return "skipped";
    }
    let mutated = QuickOp {
        id: op.id,
        arity: op.arity,
        laws: mutation.laws.unwrap_or(op.laws),
        eval: mutation.eval.unwrap_or(op.eval),
    };
    let killed_by_laws = audit_laws(&mutated).0 == QuickStatus::Fail;
    let killed_by_inference = infer_laws(&mutated).0 == QuickStatus::Fail;
    if killed_by_laws || killed_by_inference {
        "killed"
    } else {
        "survived"
    }
}
