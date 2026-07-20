//! Select-fold rewrite  -  fold `Select(BoolLit, a, b)` to a Copy of
//! the chosen arm; fold `Select(c, x, x)` to a Copy of `x`.
//!
//! Source-of-truth: `PERF_ROADMAP_2026-05-01.md` section A.4
//! (predicate hoisting / phi-select coalescing family). Companion
//! to `branch_collapse` which inlines literal-condition
//! StructuredIfThen  -  that pass operates on structured control
//! flow, this one on the value-level Select op.
//!
//! Patterns rewritten:
//! - `Select(Lit(true),  a, _)` → `Copy(a)`
//! - `Select(Lit(false), _, b)` → `Copy(b)`
//! - `Select(c, x, x)`           → `Copy(x)`  -  both arms identical so
//!   the condition has no observable effect
//!
//! Recurses into nested control flow. Idempotent. Wired into
//! `CANONICAL_REWRITE_PASSES` immediately after `boolean_simplify`
//! and `negate_cancel` so any literal-Bool conditions exposed by
//! those passes get the chance to fold here in the same fixpoint
//! phase.

use super::body_index::BodyIndex;
use crate::{KernelBody, KernelDescriptor, KernelOpKind};

#[must_use]
pub fn select_fold(desc: &KernelDescriptor) -> KernelDescriptor {
    let mut out = desc.clone();
    out.body = select_fold_body(out.body);
    out
}

fn select_fold_body(mut body: KernelBody) -> KernelBody {
    let index = BodyIndex::new(&body);

    // (op_idx, replace_id_to_copy)
    let mut rewrites: Vec<(usize, u32)> = Vec::new();

    for (idx, op) in body.ops.iter().enumerate() {
        if !matches!(op.kind, KernelOpKind::Select) {
            continue;
        }
        if op.operands.len() != 3 {
            continue;
        }
        let cond = op.operands[0];
        let true_val = op.operands[1];
        let false_val = op.operands[2];

        // Both arms identical  -  drop the condition.
        if true_val == false_val {
            rewrites.push((idx, true_val));
            continue;
        }

        // Literal cond  -  pick the live arm.
        if let Some(value) = index.bool_lit(&body, cond) {
            rewrites.push((idx, if value { true_val } else { false_val }));
        }
    }

    for (op_idx, replace_id) in rewrites {
        body.ops[op_idx].kind = KernelOpKind::Copy;
        body.ops[op_idx].operands = vec![replace_id];
    }

    body.child_bodies = body
        .child_bodies
        .into_iter()
        .map(select_fold_body)
        .collect();
    body
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BindingLayout, Dispatch, KernelOp, LiteralValue};

    fn empty_body() -> KernelBody {
        KernelBody {
            ops: Vec::new(),
            child_bodies: Vec::new(),
            literals: Vec::new(),
        }
    }

    fn descriptor_with(body: KernelBody) -> KernelDescriptor {
        KernelDescriptor {
            id: "select_fold_test".into(),
            bindings: BindingLayout { slots: Vec::new() },
            dispatch: Dispatch::new(1, 1, 1),
            body,
        }
    }

    fn lit_bool(body: &mut KernelBody, value: bool, result: u32) {
        let pool_idx = body.literals.len() as u32;
        body.literals.push(LiteralValue::Bool(value));
        body.ops.push(KernelOp {
            kind: KernelOpKind::Literal,
            operands: vec![pool_idx],
            result: Some(result),
        });
    }

    fn lit_u32(body: &mut KernelBody, value: u32, result: u32) {
        let pool_idx = body.literals.len() as u32;
        body.literals.push(LiteralValue::U32(value));
        body.ops.push(KernelOp {
            kind: KernelOpKind::Literal,
            operands: vec![pool_idx],
            result: Some(result),
        });
    }

    fn select(body: &mut KernelBody, cond: u32, true_val: u32, false_val: u32, result: u32) {
        body.ops.push(KernelOp {
            kind: KernelOpKind::Select,
            operands: vec![cond, true_val, false_val],
            result: Some(result),
        });
    }

    fn copied_source(desc: &KernelDescriptor, result: u32) -> u32 {
        let op = desc
            .body
            .ops
            .iter()
            .find(|op| op.result == Some(result))
            .unwrap();
        assert!(matches!(op.kind, KernelOpKind::Copy));
        op.operands[0]
    }

    #[test]
    fn select_true_picks_then_arm() {
        let mut body = empty_body();
        lit_bool(&mut body, true, 0);
        lit_u32(&mut body, 100, 1);
        lit_u32(&mut body, 200, 2);
        select(&mut body, 0, 1, 2, 3);
        let desc = select_fold(&descriptor_with(body));
        assert_eq!(copied_source(&desc, 3), 1);
    }

    #[test]
    fn select_false_picks_else_arm() {
        let mut body = empty_body();
        lit_bool(&mut body, false, 0);
        lit_u32(&mut body, 100, 1);
        lit_u32(&mut body, 200, 2);
        select(&mut body, 0, 1, 2, 3);
        let desc = select_fold(&descriptor_with(body));
        assert_eq!(copied_source(&desc, 3), 2);
    }

    #[test]
    fn select_with_identical_arms_drops_cond() {
        let mut body = empty_body();
        // cond is non-literal so bool_lit returns None  -  only the
        // identical-arms rule should fire.
        lit_u32(&mut body, 7, 0); // dummy non-bool source for cond
        lit_u32(&mut body, 42, 1);
        select(&mut body, 0, 1, 1, 2);
        let desc = select_fold(&descriptor_with(body));
        assert_eq!(copied_source(&desc, 2), 1);
    }

    #[test]
    fn select_with_non_literal_cond_unchanged() {
        let mut body = empty_body();
        lit_u32(&mut body, 7, 0); // cond is u32 lit, not bool  -  bool_lit returns None
        lit_u32(&mut body, 100, 1);
        lit_u32(&mut body, 200, 2);
        select(&mut body, 0, 1, 2, 3);
        let desc = select_fold(&descriptor_with(body));
        let op = desc
            .body
            .ops
            .iter()
            .find(|op| op.result == Some(3))
            .unwrap();
        assert!(
            matches!(op.kind, KernelOpKind::Select),
            "Fix: non-bool-literal cond must leave Select alone, got {:?}",
            op.kind
        );
    }

    #[test]
    fn rewrite_is_idempotent() {
        let mut body = empty_body();
        lit_bool(&mut body, true, 0);
        lit_u32(&mut body, 9, 1);
        lit_u32(&mut body, 17, 2);
        select(&mut body, 0, 1, 2, 3);
        let desc = descriptor_with(body);
        let once = select_fold(&desc);
        let twice = select_fold(&once);
        assert_eq!(once, twice);
    }

    #[test]
    fn recurses_into_child_bodies() {
        let mut child = empty_body();
        lit_bool(&mut child, true, 10);
        lit_u32(&mut child, 1, 11);
        lit_u32(&mut child, 2, 12);
        select(&mut child, 10, 11, 12, 13);

        let mut body = empty_body();
        body.child_bodies.push(child);
        let desc = select_fold(&descriptor_with(body));
        let op = desc.body.child_bodies[0]
            .ops
            .iter()
            .find(|op| op.result == Some(13))
            .unwrap();
        assert!(matches!(op.kind, KernelOpKind::Copy));
        assert_eq!(op.operands[0], 11);
    }
}
