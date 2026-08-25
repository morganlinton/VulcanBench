//! Unary idempotence  -  fold doubled-up rounding/normalization ops
//! into a single application. Companion to `negate_cancel`'s
//! involution patterns; this pass handles ops that are idempotent
//! (`f(f(x)) == f(x)`) rather than involutory (`f(f(x)) == x`).
//!
//! Source-of-truth: `PERF_ROADMAP_2026-05-01.md` section A.4
//! (algebraic simplification family).
//!
//! Patterns rewritten:
//! - `Floor(Floor(x))` → `Copy(Floor(x))`
//! - `Ceil(Ceil(x))`   → `Copy(Ceil(x))`
//! - `Round(Round(x))` → `Copy(Round(x))`
//! - `Trunc(Trunc(x))` → `Copy(Trunc(x))`
//! - `Abs(Abs(x))`     → `Copy(Abs(x))`
//! - `Sign(Sign(x))`   → `Copy(Sign(x))`
//!
//! All six are safe for f32: each produces an integer-valued float
//! (or the values {-1, 0, 1} for Sign), which the second
//! application returns unchanged. No f32 rounding pitfalls because
//! the inner result has zero fractional bits or saturates at the
//! sign domain.
//!
//! Out of scope:
//! - `Negate(Negate(x))` and `BitNot(BitNot(x))`  -  those are
//!   INVOLUTIONS (handled by `negate_cancel`, fold to `Copy(x)`
//!   not `Copy(inner)`).
//! - `LogicalNot(LogicalNot(x))`  -  boolean involution, handled by
//!   `boolean_simplify`.
//!
//! Recurses. Idempotent. Wired into `CANONICAL_REWRITE_PASSES`
//! immediately after `negate_cancel` in the algebraic-simplification
//! cluster.

use super::body_index::BodyIndex;
use crate::{KernelBody, KernelDescriptor, KernelOpKind};
use vyre_foundation::ir::UnOp;

#[must_use]
pub fn unary_idemp(desc: &KernelDescriptor) -> KernelDescriptor {
    let mut out = desc.clone();
    out.body = unary_idemp_body(out.body);
    out
}

fn unary_idemp_body(mut body: KernelBody) -> KernelBody {
    let index = BodyIndex::new(&body);

    let mut rewrites: Vec<(usize, u32)> = Vec::new();
    for (idx, op) in body.ops.iter().enumerate() {
        let outer = match &op.kind {
            KernelOpKind::UnOpKind(u) => u.clone(),
            _ => continue,
        };
        if !is_idempotent_unop(&outer) {
            continue;
        }
        if op.operands.len() != 1 {
            continue;
        }
        let inner_id = op.operands[0];
        let Some(producer) = index.producer(&body, inner_id) else {
            continue;
        };
        if let KernelOpKind::UnOpKind(inner) = &producer.kind {
            if same_unop(&outer, inner) {
                // Outer op is redundant  -  replace with Copy of the
                // inner-op result (NOT inner-op's operand, since the
                // inner application is the one that did the work).
                rewrites.push((idx, inner_id));
            }
        }
    }
    for (op_idx, replace_id) in rewrites {
        body.ops[op_idx].kind = KernelOpKind::Copy;
        body.ops[op_idx].operands = vec![replace_id];
    }
    body.child_bodies = body
        .child_bodies
        .into_iter()
        .map(unary_idemp_body)
        .collect();
    body
}

fn is_idempotent_unop(op: &UnOp) -> bool {
    matches!(
        op,
        UnOp::Floor | UnOp::Ceil | UnOp::Round | UnOp::Trunc | UnOp::Abs | UnOp::Sign
    )
}

fn same_unop(a: &UnOp, b: &UnOp) -> bool {
    // Discriminant equality is the right test  -  none of the
    // idempotent unops carry payloads, so structural equality
    // collapses to discriminant equality.
    std::mem::discriminant(a) == std::mem::discriminant(b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BindingLayout, Dispatch, KernelOp, LiteralValue};
    use vyre_foundation::ir::UnOp;

    fn empty_body() -> KernelBody {
        KernelBody {
            ops: Vec::new(),
            child_bodies: Vec::new(),
            literals: Vec::new(),
        }
    }

    fn descriptor_with(body: KernelBody) -> KernelDescriptor {
        KernelDescriptor {
            id: "unary_idemp_test".into(),
            bindings: BindingLayout { slots: Vec::new() },
            dispatch: Dispatch::new(1, 1, 1),
            body,
        }
    }

    fn lit_f32(body: &mut KernelBody, value: f32, result: u32) {
        let pool_idx = body.literals.len() as u32;
        body.literals.push(LiteralValue::F32(value));
        body.ops.push(KernelOp {
            kind: KernelOpKind::Literal,
            operands: vec![pool_idx],
            result: Some(result),
        });
    }

    fn unop(body: &mut KernelBody, op: UnOp, operand: u32, result: u32) {
        body.ops.push(KernelOp {
            kind: KernelOpKind::UnOpKind(op),
            operands: vec![operand],
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

    fn assert_collapses(idemp_op: UnOp) {
        let mut body = empty_body();
        lit_f32(&mut body, 3.7, 0);
        unop(&mut body, idemp_op.clone(), 0, 1);
        unop(&mut body, idemp_op, 1, 2);
        let desc = unary_idemp(&descriptor_with(body));
        // Outer op (result 2) → Copy of inner result (result 1).
        assert_eq!(copied_source(&desc, 2), 1);
    }

    #[test]
    fn floor_idemp() {
        assert_collapses(UnOp::Floor);
    }

    #[test]
    fn ceil_idemp() {
        assert_collapses(UnOp::Ceil);
    }

    #[test]
    fn round_idemp() {
        assert_collapses(UnOp::Round);
    }

    #[test]
    fn trunc_idemp() {
        assert_collapses(UnOp::Trunc);
    }

    #[test]
    fn abs_idemp() {
        assert_collapses(UnOp::Abs);
    }

    #[test]
    fn sign_idemp() {
        assert_collapses(UnOp::Sign);
    }

    #[test]
    fn mismatched_unops_do_not_fold() {
        // Floor(Ceil(x)) is NOT Floor(Floor(x))  -  skip.
        let mut body = empty_body();
        lit_f32(&mut body, 3.7, 0);
        unop(&mut body, UnOp::Ceil, 0, 1);
        unop(&mut body, UnOp::Floor, 1, 2);
        let desc = unary_idemp(&descriptor_with(body));
        let op = desc
            .body
            .ops
            .iter()
            .find(|op| op.result == Some(2))
            .unwrap();
        assert!(matches!(op.kind, KernelOpKind::UnOpKind(UnOp::Floor)));
    }

    #[test]
    fn non_idempotent_unop_unchanged() {
        // Sin(Sin(x)) is NOT idempotent; leave alone.
        let mut body = empty_body();
        lit_f32(&mut body, 1.0, 0);
        unop(&mut body, UnOp::Sin, 0, 1);
        unop(&mut body, UnOp::Sin, 1, 2);
        let desc = unary_idemp(&descriptor_with(body));
        let op = desc
            .body
            .ops
            .iter()
            .find(|op| op.result == Some(2))
            .unwrap();
        assert!(matches!(op.kind, KernelOpKind::UnOpKind(UnOp::Sin)));
    }

    #[test]
    fn rewrite_is_idempotent() {
        let mut body = empty_body();
        lit_f32(&mut body, 2.5, 0);
        unop(&mut body, UnOp::Floor, 0, 1);
        unop(&mut body, UnOp::Floor, 1, 2);
        let desc = descriptor_with(body);
        let once = unary_idemp(&desc);
        let twice = unary_idemp(&once);
        assert_eq!(once, twice);
    }

    #[test]
    fn recurses_into_child_bodies() {
        let mut child = empty_body();
        lit_f32(&mut child, 3.7, 10);
        unop(&mut child, UnOp::Abs, 10, 11);
        unop(&mut child, UnOp::Abs, 11, 12);
        let mut body = empty_body();
        body.child_bodies.push(child);
        let desc = unary_idemp(&descriptor_with(body));
        let op = desc.body.child_bodies[0]
            .ops
            .iter()
            .find(|op| op.result == Some(12))
            .unwrap();
        assert!(matches!(op.kind, KernelOpKind::Copy));
        assert_eq!(op.operands[0], 11);
    }
}
