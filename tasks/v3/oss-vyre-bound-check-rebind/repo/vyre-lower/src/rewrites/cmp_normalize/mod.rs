//! Comparison normalization  -  fold `Gt(a, b)` to `Lt(b, a)` and
//! `Ge(a, b)` to `Le(b, a)` so structurally-equivalent comparisons
//! collapse to a single canonical form.
//!
//! Source-of-truth: `PERF_ROADMAP_2026-05-01.md` section A.4  -  the
//! "canonicalization for CSE" sub-family. Companion to `canonicalize`
//! (which sorts commutative-op operands) for the inverse-relation
//! family that canonicalize cannot touch (Gt and Lt are NOT
//! commutative  -  `Gt(a,b) ≠ Gt(b,a)`  -  so canonicalize leaves them
//! alone, but `Gt(a,b) ≡ Lt(b,a)` so this pass collapses them).
//!
//! The win is a CSE multiplier: a kernel that hand-codes the same
//! comparison once with `Gt` and again with `Lt(b, a)` previously
//! produced two distinct `BinOp` ops; now it produces one, which
//! `descriptor_cse` can dedupe.
//!
//! Backend emitters (naga, ptx, spirv) all emit equivalent code for
//! Lt vs Gt, so the canonical-direction choice is free at codegen.
//!
//! Recurses into nested control flow. Idempotent. Wired into
//! `CANONICAL_REWRITE_PASSES` immediately before `canonicalize` so
//! the canonicalize pass that follows can sort operands of the
//! commutative ops `cmp_normalize` doesn't touch.

use crate::{KernelBody, KernelDescriptor, KernelOpKind};
use vyre_foundation::ir::BinOp;

#[must_use]
pub fn cmp_normalize(desc: &KernelDescriptor) -> KernelDescriptor {
    let mut out = desc.clone();
    out.body = cmp_normalize_body(out.body);
    out
}

fn cmp_normalize_body(mut body: KernelBody) -> KernelBody {
    for op in body.ops.iter_mut() {
        let bin = match &op.kind {
            KernelOpKind::BinOpKind(b) => *b,
            _ => continue,
        };
        if op.operands.len() != 2 {
            continue;
        }
        let (new_op, swap) = match bin {
            BinOp::Gt => (Some(BinOp::Lt), true),
            BinOp::Ge => (Some(BinOp::Le), true),
            _ => (None, false),
        };
        if let Some(replacement) = new_op {
            op.kind = KernelOpKind::BinOpKind(replacement);
            if swap {
                op.operands.swap(0, 1);
            }
        }
    }

    body.child_bodies = body
        .child_bodies
        .into_iter()
        .map(cmp_normalize_body)
        .collect();
    body
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BindingLayout, Dispatch, KernelOp, LiteralValue};
    use vyre_foundation::ir::BinOp;

    fn empty_body() -> KernelBody {
        KernelBody {
            ops: Vec::new(),
            child_bodies: Vec::new(),
            literals: Vec::new(),
        }
    }

    fn descriptor_with(body: KernelBody) -> KernelDescriptor {
        KernelDescriptor {
            id: "cmp_normalize_test".into(),
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

    fn binop(body: &mut KernelBody, op: BinOp, lhs: u32, rhs: u32, result: u32) {
        body.ops.push(KernelOp {
            kind: KernelOpKind::BinOpKind(op),
            operands: vec![lhs, rhs],
            result: Some(result),
        });
    }

    fn op_at(desc: &KernelDescriptor, result: u32) -> &KernelOp {
        desc.body
            .ops
            .iter()
            .find(|op| op.result == Some(result))
            .expect("Fix: target result op must exist")
    }

    #[test]
    fn gt_normalises_to_lt_with_swapped_operands() {
        let mut body = empty_body();
        lit_u32(&mut body, 5, 0);
        lit_u32(&mut body, 10, 1);
        binop(&mut body, BinOp::Gt, 0, 1, 2); // Gt(5, 10)
        let desc = cmp_normalize(&descriptor_with(body));
        let op = op_at(&desc, 2);
        assert!(matches!(op.kind, KernelOpKind::BinOpKind(BinOp::Lt)));
        assert_eq!(op.operands, vec![1, 0], "operands must be swapped");
    }

    #[test]
    fn ge_normalises_to_le_with_swapped_operands() {
        let mut body = empty_body();
        lit_u32(&mut body, 5, 0);
        lit_u32(&mut body, 10, 1);
        binop(&mut body, BinOp::Ge, 0, 1, 2);
        let desc = cmp_normalize(&descriptor_with(body));
        let op = op_at(&desc, 2);
        assert!(matches!(op.kind, KernelOpKind::BinOpKind(BinOp::Le)));
        assert_eq!(op.operands, vec![1, 0]);
    }

    #[test]
    fn lt_unchanged() {
        let mut body = empty_body();
        lit_u32(&mut body, 5, 0);
        lit_u32(&mut body, 10, 1);
        binop(&mut body, BinOp::Lt, 0, 1, 2);
        let desc = cmp_normalize(&descriptor_with(body));
        let op = op_at(&desc, 2);
        assert!(matches!(op.kind, KernelOpKind::BinOpKind(BinOp::Lt)));
        assert_eq!(op.operands, vec![0, 1], "Lt operands must not be reordered");
    }

    #[test]
    fn le_unchanged() {
        let mut body = empty_body();
        lit_u32(&mut body, 5, 0);
        lit_u32(&mut body, 10, 1);
        binop(&mut body, BinOp::Le, 0, 1, 2);
        let desc = cmp_normalize(&descriptor_with(body));
        let op = op_at(&desc, 2);
        assert!(matches!(op.kind, KernelOpKind::BinOpKind(BinOp::Le)));
        assert_eq!(op.operands, vec![0, 1]);
    }

    #[test]
    fn non_comparison_ops_unchanged() {
        let mut body = empty_body();
        lit_u32(&mut body, 5, 0);
        lit_u32(&mut body, 10, 1);
        binop(&mut body, BinOp::Add, 0, 1, 2);
        binop(&mut body, BinOp::Sub, 0, 1, 3);
        let desc = cmp_normalize(&descriptor_with(body));
        assert!(matches!(
            op_at(&desc, 2).kind,
            KernelOpKind::BinOpKind(BinOp::Add)
        ));
        assert!(matches!(
            op_at(&desc, 3).kind,
            KernelOpKind::BinOpKind(BinOp::Sub)
        ));
    }

    #[test]
    fn rewrite_is_idempotent() {
        let mut body = empty_body();
        lit_u32(&mut body, 5, 0);
        lit_u32(&mut body, 10, 1);
        binop(&mut body, BinOp::Gt, 0, 1, 2);
        let desc = descriptor_with(body);
        let once = cmp_normalize(&desc);
        let twice = cmp_normalize(&once);
        assert_eq!(once, twice);
    }

    #[test]
    fn recurses_into_child_bodies() {
        let mut child = empty_body();
        lit_u32(&mut child, 5, 10);
        lit_u32(&mut child, 10, 11);
        binop(&mut child, BinOp::Gt, 10, 11, 12);

        let mut body = empty_body();
        body.child_bodies.push(child);
        let desc = cmp_normalize(&descriptor_with(body));
        let op = desc.body.child_bodies[0]
            .ops
            .iter()
            .find(|op| op.result == Some(12))
            .unwrap();
        assert!(matches!(op.kind, KernelOpKind::BinOpKind(BinOp::Lt)));
        assert_eq!(op.operands, vec![11, 10]);
    }

    #[test]
    fn cse_collapses_gt_and_swapped_lt_after_normalize() {
        // Gt(a, b) and Lt(b, a) are semantically identical; before
        // cmp_normalize they have distinct shapes, so CSE can't merge
        // them. After normalize both become Lt(b, a).
        let mut body = empty_body();
        lit_u32(&mut body, 7, 0);
        lit_u32(&mut body, 9, 1);
        binop(&mut body, BinOp::Gt, 0, 1, 2); // Gt(7, 9)
        binop(&mut body, BinOp::Lt, 1, 0, 3); // Lt(9, 7)  -  same answer
        let desc = cmp_normalize(&descriptor_with(body));
        // Both ops should now have kind=Lt and operands=[1, 0].
        let op_a = op_at(&desc, 2);
        let op_b = op_at(&desc, 3);
        assert!(matches!(op_a.kind, KernelOpKind::BinOpKind(BinOp::Lt)));
        assert!(matches!(op_b.kind, KernelOpKind::BinOpKind(BinOp::Lt)));
        assert_eq!(op_a.operands, op_b.operands);
    }
}
