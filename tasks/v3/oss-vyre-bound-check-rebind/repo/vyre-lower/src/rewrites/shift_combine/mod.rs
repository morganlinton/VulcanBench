//! Shift combination  -  fold `Shl(Shl(x, n), m)` to `Shl(x, n+m)`
//! and `Shr(Shr(x, n), m)` to `Shr(x, n+m)` when both shift amounts
//! are U32 literals and their sum fits in `[0, 32)`.
//!
//! Source-of-truth: `PERF_ROADMAP_2026-05-01.md` section A.4
//! (algebraic simplification family). Companion to `strength_reduce`
//! which produces shifts from mul/div by power-of-two  -  chained
//! shifts often appear after that pass folds nested multiplications.
//!
//! Patterns rewritten:
//! - `Shl(Shl(x, Lit(n)), Lit(m))` → `Shl(x, Lit(n + m))` when n + m < 32
//! - `Shr(Shr(x, Lit(n)), Lit(m))` → `Shr(x, Lit(n + m))` when n + m < 32
//!
//! Out of scope:
//! - `Shl(Shr(x, n), m)` and the reverse  -  these are NOT
//!   semantically equivalent to a single shift because the inner
//!   shift truncates bits; combining requires careful analysis.
//! - `n + m >= 32`  -  undefined behaviour territory for u32 shifts;
//!   we leave the chain alone rather than fold to an out-of-range
//!   shift count. (A future const_fold extension can handle the
//!   `n+m >= 32 → Lit(0)` case for u32.)
//!
//! Recurses into nested control flow. Idempotent. Wired into
//! `CANONICAL_REWRITE_PASSES` after `strength_reduce` (which produces
//! the input shifts) and before `descriptor_const_fold` (which
//! folds the new combined-shift constant if its operand is also a
//! literal).

use super::body_index::BodyIndex;
use super::literal::ResultAllocator;
use crate::{KernelBody, KernelDescriptor, KernelOpKind, LiteralValue};
use vyre_foundation::ir::BinOp;

#[must_use]
pub fn shift_combine(desc: &KernelDescriptor) -> KernelDescriptor {
    let mut out = desc.clone();
    let mut allocator = ResultAllocator::for_body_tree(&out.body);
    out.body = shift_combine_body(out.body, &mut allocator);
    out
}

fn shift_combine_body(mut body: KernelBody, allocator: &mut ResultAllocator) -> KernelBody {
    let index = BodyIndex::new(&body);

    // (op_idx, x_id, combined_shift). The new combined shift lives in
    // a freshly synthesized Literal op pushed at the end of body.ops.
    let mut rewrites: Vec<(usize, u32, u32, BinOp)> = Vec::new();
    for (idx, op) in body.ops.iter().enumerate() {
        let outer = match &op.kind {
            KernelOpKind::BinOpKind(b @ (BinOp::Shl | BinOp::Shr)) => *b,
            _ => continue,
        };
        if op.operands.len() != 2 {
            continue;
        }
        let inner_id = op.operands[0];
        let outer_shift_id = op.operands[1];
        let outer_shift_lit = match index.u32_lit(&body, outer_shift_id) {
            Some(v) => v,
            None => continue,
        };
        let Some(inner_op) = index.producer(&body, inner_id) else {
            continue;
        };
        let inner_kind = match &inner_op.kind {
            KernelOpKind::BinOpKind(b) => *b,
            _ => continue,
        };
        if inner_kind != outer {
            continue;
        }
        if inner_op.operands.len() != 2 {
            continue;
        }
        let x_id = inner_op.operands[0];
        let inner_shift_id = inner_op.operands[1];
        let inner_shift_lit = match index.u32_lit(&body, inner_shift_id) {
            Some(v) => v,
            None => continue,
        };
        // Sum must fit in u32 AND stay below the bit width (32) to
        // avoid undefined-shift territory.
        let sum = match outer_shift_lit.checked_add(inner_shift_lit) {
            Some(s) if s < 32 => s,
            _ => continue,
        };
        rewrites.push((idx, x_id, sum, outer));
    }

    for (op_idx, x_id, sum, outer) in rewrites {
        let synth_id =
            allocator.push_literal(&mut body.ops, &mut body.literals, LiteralValue::U32(sum));
        body.ops[op_idx].kind = KernelOpKind::BinOpKind(outer);
        body.ops[op_idx].operands = vec![x_id, synth_id];
    }

    body.child_bodies = body
        .child_bodies
        .into_iter()
        .map(|child| shift_combine_body(child, allocator))
        .collect();
    body
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BindingLayout, Dispatch, KernelOp};
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
            id: "shift_combine_test".into(),
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
            .expect("Fix: target op must exist")
    }

    fn lit_value_at(desc: &KernelDescriptor, result: u32) -> u32 {
        let op = op_at(desc, result);
        assert!(matches!(op.kind, KernelOpKind::Literal));
        let pool_idx = op.operands[0] as usize;
        match desc.body.literals[pool_idx] {
            LiteralValue::U32(v) => v,
            _ => panic!("Fix: expected U32 literal"),
        }
    }

    #[test]
    fn shl_chain_combines_when_sum_fits() {
        // x << 2 << 3  →  x << 5
        let mut body = empty_body();
        lit_u32(&mut body, 7, 0); // x
        lit_u32(&mut body, 2, 1); // shift amount n
        binop(&mut body, BinOp::Shl, 0, 1, 2);
        lit_u32(&mut body, 3, 3); // shift amount m
        binop(&mut body, BinOp::Shl, 2, 3, 4);
        let desc = shift_combine(&descriptor_with(body));
        let op = op_at(&desc, 4);
        assert!(matches!(op.kind, KernelOpKind::BinOpKind(BinOp::Shl)));
        assert_eq!(op.operands[0], 0, "first operand must be the original x");
        let combined_shift = op.operands[1];
        assert_eq!(lit_value_at(&desc, combined_shift), 5);
    }

    #[test]
    fn shr_chain_combines_when_sum_fits() {
        let mut body = empty_body();
        lit_u32(&mut body, 0xFFFF, 0);
        lit_u32(&mut body, 4, 1);
        binop(&mut body, BinOp::Shr, 0, 1, 2);
        lit_u32(&mut body, 8, 3);
        binop(&mut body, BinOp::Shr, 2, 3, 4);
        let desc = shift_combine(&descriptor_with(body));
        let op = op_at(&desc, 4);
        assert!(matches!(op.kind, KernelOpKind::BinOpKind(BinOp::Shr)));
        assert_eq!(op.operands[0], 0);
        assert_eq!(lit_value_at(&desc, op.operands[1]), 12);
    }

    #[test]
    fn shl_chain_with_oversized_sum_left_alone() {
        // 16 + 16 = 32 → shift by 32 is UB on u32; refuse to combine.
        let mut body = empty_body();
        lit_u32(&mut body, 1, 0);
        lit_u32(&mut body, 16, 1);
        binop(&mut body, BinOp::Shl, 0, 1, 2);
        lit_u32(&mut body, 16, 3);
        binop(&mut body, BinOp::Shl, 2, 3, 4);
        let desc = shift_combine(&descriptor_with(body));
        let op = op_at(&desc, 4);
        assert!(matches!(op.kind, KernelOpKind::BinOpKind(BinOp::Shl)));
        assert_eq!(
            op.operands[0], 2,
            "operand should still reference the inner-shift result, not the original x"
        );
    }

    #[test]
    fn mixed_shl_shr_chain_left_alone() {
        // x << 2 >> 3  -  different ops, NOT combinable.
        let mut body = empty_body();
        lit_u32(&mut body, 7, 0);
        lit_u32(&mut body, 2, 1);
        binop(&mut body, BinOp::Shl, 0, 1, 2);
        lit_u32(&mut body, 3, 3);
        binop(&mut body, BinOp::Shr, 2, 3, 4);
        let desc = shift_combine(&descriptor_with(body));
        let op = op_at(&desc, 4);
        assert!(matches!(op.kind, KernelOpKind::BinOpKind(BinOp::Shr)));
        assert_eq!(
            op.operands[0], 2,
            "Shr operand 0 must still reference the inner Shl result"
        );
    }

    #[test]
    fn non_literal_shift_amount_left_alone() {
        let mut body = empty_body();
        lit_u32(&mut body, 7, 0);
        lit_u32(&mut body, 2, 1);
        binop(&mut body, BinOp::Shl, 0, 1, 2);
        // outer shift amount is the result of an Add, not a literal
        lit_u32(&mut body, 1, 3);
        lit_u32(&mut body, 1, 4);
        binop(&mut body, BinOp::Add, 3, 4, 5);
        binop(&mut body, BinOp::Shl, 2, 5, 6);
        let desc = shift_combine(&descriptor_with(body));
        let op = op_at(&desc, 6);
        assert_eq!(
            op.operands[0], 2,
            "operand 0 must still be the inner shift result"
        );
    }

    #[test]
    fn rewrite_is_idempotent() {
        let mut body = empty_body();
        lit_u32(&mut body, 7, 0);
        lit_u32(&mut body, 2, 1);
        binop(&mut body, BinOp::Shl, 0, 1, 2);
        lit_u32(&mut body, 3, 3);
        binop(&mut body, BinOp::Shl, 2, 3, 4);
        let desc = descriptor_with(body);
        let once = shift_combine(&desc);
        let twice = shift_combine(&once);
        assert_eq!(once, twice);
    }

    #[test]
    fn recurses_into_child_bodies() {
        let mut child = empty_body();
        lit_u32(&mut child, 7, 10);
        lit_u32(&mut child, 1, 11);
        binop(&mut child, BinOp::Shl, 10, 11, 12);
        lit_u32(&mut child, 2, 13);
        binop(&mut child, BinOp::Shl, 12, 13, 14);

        let mut body = empty_body();
        body.child_bodies.push(child);
        let desc = shift_combine(&descriptor_with(body));
        let op = desc.body.child_bodies[0]
            .ops
            .iter()
            .find(|op| op.result == Some(14))
            .unwrap();
        assert_eq!(
            op.operands[0], 10,
            "child-body inner result must be the original x"
        );
    }
}
