//! Negation-cancellation rewrite.
//!
//! Source-of-truth: `PERF_ROADMAP_2026-05-01.md` section A.4 (algebraic
//! simplification family). Companion to `boolean_simplify`'s
//! double-LogicalNot pattern  -  that pass handles the boolean case;
//! this one handles the bitwise + arithmetic cases.
//!
//! Patterns rewritten:
//! - `BitNot(BitNot(x))` → `x` (involution)
//! - `Negate(Negate(x))` → `x` (involution)
//! - `Sub(a, Negate(b))` → `Add(a, b)` (canonicalises to a single-op
//!   addition; downstream identity_elim then folds Add(_, 0) etc.)
//!
//! Out of scope:
//! - `Negate(Sub(a, b))` → `Sub(b, a)`  -  reorders operands but doesn't
//!   reduce op count; canonicalize handles operand sorting.
//! - `Add(a, Negate(b))` → `Sub(a, b)`  -  symmetric to the Sub-Negate
//!   case but the canonical direction is Sub(a, Negate(b)) → Add(a, b)
//!   so we don't oscillate; the inverse would defeat the rewrite.
//!
//! Recurses into nested control flow. Idempotent. Wired into
//! `CANONICAL_REWRITE_PASSES` after `boolean_simplify` so all three
//! involution families (LogicalNot, BitNot, Negate) are handled in
//! the same fixpoint phase.

use super::body_index::BodyIndex;
use crate::{KernelBody, KernelDescriptor, KernelOpKind};
use vyre_foundation::ir::{BinOp, UnOp};

#[must_use]
pub fn negate_cancel(desc: &KernelDescriptor) -> KernelDescriptor {
    let mut out = desc.clone();
    out.body = negate_cancel_body(out.body);
    out
}

fn negate_cancel_body(mut body: KernelBody) -> KernelBody {
    let index = BodyIndex::new(&body);

    enum Rewrite {
        // Replace op_idx's kind/operands with Copy(replace_id).
        Copy { op_idx: usize, replace_id: u32 },
        // Rewrite Sub(a, Negate(b)) → Add(a, b).
        SubNegateToAdd { op_idx: usize, a_id: u32, b_id: u32 },
    }
    let mut rewrites: Vec<Rewrite> = Vec::new();

    for (idx, op) in body.ops.iter().enumerate() {
        match &op.kind {
            KernelOpKind::UnOpKind(outer @ (UnOp::BitNot | UnOp::Negate)) => {
                if op.operands.len() != 1 {
                    continue;
                }
                let inner_id = op.operands[0];
                let Some(producer) = index.producer(&body, inner_id) else {
                    continue;
                };
                if let KernelOpKind::UnOpKind(inner) = &producer.kind {
                    if outer == inner && producer.operands.len() == 1 {
                        rewrites.push(Rewrite::Copy {
                            op_idx: idx,
                            replace_id: producer.operands[0],
                        });
                    }
                }
            }
            KernelOpKind::BinOpKind(BinOp::Sub) => {
                if op.operands.len() != 2 {
                    continue;
                }
                let a_id = op.operands[0];
                let neg_id = op.operands[1];
                let Some(neg_producer) = index.producer(&body, neg_id) else {
                    continue;
                };
                if matches!(neg_producer.kind, KernelOpKind::UnOpKind(UnOp::Negate))
                    && neg_producer.operands.len() == 1
                {
                    rewrites.push(Rewrite::SubNegateToAdd {
                        op_idx: idx,
                        a_id,
                        b_id: neg_producer.operands[0],
                    });
                }
            }
            _ => {}
        }
    }

    for r in rewrites {
        match r {
            Rewrite::Copy { op_idx, replace_id } => {
                body.ops[op_idx].kind = KernelOpKind::Copy;
                body.ops[op_idx].operands = vec![replace_id];
            }
            Rewrite::SubNegateToAdd { op_idx, a_id, b_id } => {
                body.ops[op_idx].kind = KernelOpKind::BinOpKind(BinOp::Add);
                body.ops[op_idx].operands = vec![a_id, b_id];
            }
        }
    }

    body.child_bodies = body
        .child_bodies
        .into_iter()
        .map(negate_cancel_body)
        .collect();
    body
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BindingLayout, Dispatch, KernelOp, LiteralValue};
    use vyre_foundation::ir::{BinOp, UnOp};

    fn empty_body() -> KernelBody {
        KernelBody {
            ops: Vec::new(),
            child_bodies: Vec::new(),
            literals: Vec::new(),
        }
    }

    fn descriptor_with(body: KernelBody) -> KernelDescriptor {
        KernelDescriptor {
            id: "negate_cancel_test".into(),
            bindings: BindingLayout { slots: Vec::new() },
            dispatch: Dispatch::new(1, 1, 1),
            body,
        }
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

    fn lit_i32(body: &mut KernelBody, value: i32, result: u32) {
        let pool_idx = body.literals.len() as u32;
        body.literals.push(LiteralValue::I32(value));
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

    fn binop(body: &mut KernelBody, op: BinOp, lhs: u32, rhs: u32, result: u32) {
        body.ops.push(KernelOp {
            kind: KernelOpKind::BinOpKind(op),
            operands: vec![lhs, rhs],
            result: Some(result),
        });
    }

    fn kind_at(desc: &KernelDescriptor, result: u32) -> &KernelOpKind {
        &desc
            .body
            .ops
            .iter()
            .find(|op| op.result == Some(result))
            .expect("Fix: target result op must survive the rewrite")
            .kind
    }

    fn copied_source(desc: &KernelDescriptor, result: u32) -> u32 {
        let op = desc
            .body
            .ops
            .iter()
            .find(|op| op.result == Some(result))
            .expect("Fix: result op must exist");
        assert!(
            matches!(op.kind, KernelOpKind::Copy),
            "Fix: expected Copy at result {result}, got {:?}",
            op.kind
        );
        op.operands[0]
    }

    #[test]
    fn double_bitnot_eliminated() {
        let mut body = empty_body();
        lit_u32(&mut body, 0xCAFE_BABE, 0);
        unop(&mut body, UnOp::BitNot, 0, 1);
        unop(&mut body, UnOp::BitNot, 1, 2);
        let desc = negate_cancel(&descriptor_with(body));
        assert_eq!(copied_source(&desc, 2), 0);
    }

    #[test]
    fn double_negate_eliminated() {
        let mut body = empty_body();
        lit_i32(&mut body, -42, 0);
        unop(&mut body, UnOp::Negate, 0, 1);
        unop(&mut body, UnOp::Negate, 1, 2);
        let desc = negate_cancel(&descriptor_with(body));
        assert_eq!(copied_source(&desc, 2), 0);
    }

    #[test]
    fn mismatched_unops_do_not_cancel() {
        // BitNot(Negate(x)) is NOT BitNot(BitNot(x)); leave alone.
        let mut body = empty_body();
        lit_i32(&mut body, 7, 0);
        unop(&mut body, UnOp::Negate, 0, 1);
        unop(&mut body, UnOp::BitNot, 1, 2);
        let desc = negate_cancel(&descriptor_with(body));
        assert!(
            matches!(kind_at(&desc, 2), KernelOpKind::UnOpKind(UnOp::BitNot)),
            "Fix: BitNot(Negate(x)) must not collapse  -  different involutions"
        );
    }

    #[test]
    fn sub_negate_rewrites_to_add() {
        // Sub(a, Negate(b)) → Add(a, b)
        let mut body = empty_body();
        lit_i32(&mut body, 10, 0);
        lit_i32(&mut body, 3, 1);
        unop(&mut body, UnOp::Negate, 1, 2); // -3
        binop(&mut body, BinOp::Sub, 0, 2, 3); // 10 - (-3)
        let desc = negate_cancel(&descriptor_with(body));
        let op = desc
            .body
            .ops
            .iter()
            .find(|op| op.result == Some(3))
            .unwrap();
        assert!(
            matches!(op.kind, KernelOpKind::BinOpKind(BinOp::Add)),
            "Fix: Sub(a, Negate(b)) must become Add(a, b), got {:?}",
            op.kind
        );
        assert_eq!(op.operands, vec![0, 1]);
    }

    #[test]
    fn sub_with_non_negate_rhs_unchanged() {
        let mut body = empty_body();
        lit_i32(&mut body, 10, 0);
        lit_i32(&mut body, 3, 1);
        binop(&mut body, BinOp::Sub, 0, 1, 2); // 10 - 3, not 10 - (-3)
        let desc = negate_cancel(&descriptor_with(body));
        assert!(matches!(
            kind_at(&desc, 2),
            KernelOpKind::BinOpKind(BinOp::Sub)
        ));
    }

    #[test]
    fn rewrite_is_idempotent() {
        let mut body = empty_body();
        lit_u32(&mut body, 1, 0);
        unop(&mut body, UnOp::BitNot, 0, 1);
        unop(&mut body, UnOp::BitNot, 1, 2);
        let desc = descriptor_with(body);
        let once = negate_cancel(&desc);
        let twice = negate_cancel(&once);
        assert_eq!(once, twice);
    }

    #[test]
    fn recurses_into_child_bodies() {
        let mut child = empty_body();
        lit_i32(&mut child, 5, 10);
        unop(&mut child, UnOp::Negate, 10, 11);
        unop(&mut child, UnOp::Negate, 11, 12);

        let mut body = empty_body();
        body.child_bodies.push(child);
        let desc = negate_cancel(&descriptor_with(body));
        let copy_op = desc.body.child_bodies[0]
            .ops
            .iter()
            .find(|op| op.result == Some(12))
            .unwrap();
        assert!(matches!(copy_op.kind, KernelOpKind::Copy));
        assert_eq!(copy_op.operands[0], 10);
    }
}
