//! Strict-self comparison folding  -  `Lt(x, x)` and `Gt(x, x)` always
//! evaluate to `false`, including for NaN floats (`NaN < NaN` is
//! `false` per IEEE-754 §5.11).
//!
//! Source-of-truth: `PERF_ROADMAP_2026-05-01.md` section A.4
//! (algebraic simplification family).
//!
//! Patterns rewritten:
//! - `Lt(x, x)` → `Copy(synth Lit(false))`
//! - `Gt(x, x)` → `Copy(synth Lit(false))`
//!
//! Out of scope deliberately:
//! - `Eq(x, x)` → `true`  -  UNSAFE under f32 because `NaN == NaN`
//!   is `false`. The descriptor IR is dtype-untyped at the rewrite
//!   layer, so we cannot tell `Eq(int, int)` from `Eq(float, float)`
//!   here. boolean_simplify already covers the safe sub-case
//!   `Eq(LitU32, LitU32)`.
//! - `Le(x, x)` → `true`, `Ge(x, x)` → `true`  -  same NaN caveat
//!   (`NaN <= NaN` is `false`).
//! - `Ne(x, x)` → `false`  -  UNSAFE for the same reason as `Eq`.
//!
//! Safe by construction: strict less-than-itself and strict
//! greater-than-itself are mathematical falsehoods under any total
//! order AND under IEEE-754's partial order.
//!
//! Recurses. Idempotent. Wired into `CANONICAL_REWRITE_PASSES`
//! immediately after `cmp_normalize` so the only self-comparisons
//! to inspect are normalized to `Lt`/`Le`/`Eq`/`Ne` form (Gt and Ge
//! get rewritten to Lt/Le by cmp_normalize first).

use super::literal::ResultAllocator;
use crate::{KernelBody, KernelDescriptor, KernelOpKind, LiteralValue};
use vyre_foundation::ir::BinOp;

#[must_use]
pub fn cmp_self_false(desc: &KernelDescriptor) -> KernelDescriptor {
    let mut out = desc.clone();
    let mut allocator = ResultAllocator::for_body_tree(&out.body);
    out.body = cmp_self_false_body(out.body, &mut allocator);
    out
}

fn cmp_self_false_body(mut body: KernelBody, allocator: &mut ResultAllocator) -> KernelBody {
    let mut rewrites: Vec<usize> = Vec::new();
    for (idx, op) in body.ops.iter().enumerate() {
        let bin = match &op.kind {
            KernelOpKind::BinOpKind(b) => *b,
            _ => continue,
        };
        if !matches!(bin, BinOp::Lt | BinOp::Gt) {
            continue;
        }
        if op.operands.len() != 2 {
            continue;
        }
        if op.operands[0] == op.operands[1] {
            rewrites.push(idx);
        }
    }
    if rewrites.is_empty() {
        body.child_bodies = body
            .child_bodies
            .into_iter()
            .map(|child| cmp_self_false_body(child, allocator))
            .collect();
        return body;
    }

    // One synthesised Bool(false) literal shared across all rewrites.
    let synth_id =
        allocator.push_literal(&mut body.ops, &mut body.literals, LiteralValue::Bool(false));

    for op_idx in rewrites {
        body.ops[op_idx].kind = KernelOpKind::Copy;
        body.ops[op_idx].operands = vec![synth_id];
    }

    body.child_bodies = body
        .child_bodies
        .into_iter()
        .map(|child| cmp_self_false_body(child, allocator))
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
            id: "cmp_self_false_test".into(),
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

    fn folded_to_false(desc: &KernelDescriptor, result: u32) {
        let op = desc
            .body
            .ops
            .iter()
            .find(|op| op.result == Some(result))
            .unwrap();
        assert!(matches!(op.kind, KernelOpKind::Copy));
        let synth_id = op.operands[0];
        let synth_op = desc
            .body
            .ops
            .iter()
            .find(|op| op.result == Some(synth_id))
            .unwrap();
        assert!(matches!(synth_op.kind, KernelOpKind::Literal));
        let pool_idx = synth_op.operands[0] as usize;
        assert_eq!(desc.body.literals[pool_idx], LiteralValue::Bool(false));
    }

    #[test]
    fn lt_self_folds_to_false() {
        let mut body = empty_body();
        lit_u32(&mut body, 7, 0);
        binop(&mut body, BinOp::Lt, 0, 0, 1);
        let desc = cmp_self_false(&descriptor_with(body));
        folded_to_false(&desc, 1);
    }

    #[test]
    fn gt_self_folds_to_false() {
        let mut body = empty_body();
        lit_u32(&mut body, 7, 0);
        binop(&mut body, BinOp::Gt, 0, 0, 1);
        let desc = cmp_self_false(&descriptor_with(body));
        folded_to_false(&desc, 1);
    }

    #[test]
    fn lt_distinct_operands_unchanged() {
        let mut body = empty_body();
        lit_u32(&mut body, 1, 0);
        lit_u32(&mut body, 2, 1);
        binop(&mut body, BinOp::Lt, 0, 1, 2);
        let desc = cmp_self_false(&descriptor_with(body));
        let op = desc
            .body
            .ops
            .iter()
            .find(|op| op.result == Some(2))
            .unwrap();
        assert!(matches!(op.kind, KernelOpKind::BinOpKind(BinOp::Lt)));
    }

    #[test]
    fn eq_self_left_alone_for_float_safety() {
        let mut body = empty_body();
        lit_u32(&mut body, 7, 0);
        binop(&mut body, BinOp::Eq, 0, 0, 1);
        let desc = cmp_self_false(&descriptor_with(body));
        let op = desc
            .body
            .ops
            .iter()
            .find(|op| op.result == Some(1))
            .unwrap();
        assert!(
            matches!(op.kind, KernelOpKind::BinOpKind(BinOp::Eq)),
            "Fix: Eq(x, x) must NOT fold here  -  NaN safety. boolean_simplify handles the safe Eq(LitU32, LitU32) sub-case."
        );
    }

    #[test]
    fn rewrite_is_idempotent() {
        let mut body = empty_body();
        lit_u32(&mut body, 1, 0);
        binop(&mut body, BinOp::Lt, 0, 0, 1);
        let desc = descriptor_with(body);
        let once = cmp_self_false(&desc);
        let twice = cmp_self_false(&once);
        assert_eq!(once, twice);
    }

    #[test]
    fn shared_synth_literal_for_multiple_folds() {
        // Two folds in the same body should share one synthesized
        // Bool(false) literal slot.
        let mut body = empty_body();
        lit_u32(&mut body, 7, 0);
        lit_u32(&mut body, 9, 1);
        binop(&mut body, BinOp::Lt, 0, 0, 2);
        binop(&mut body, BinOp::Gt, 1, 1, 3);
        let desc = cmp_self_false(&descriptor_with(body));
        let op_a = desc
            .body
            .ops
            .iter()
            .find(|op| op.result == Some(2))
            .unwrap();
        let op_b = desc
            .body
            .ops
            .iter()
            .find(|op| op.result == Some(3))
            .unwrap();
        assert!(matches!(op_a.kind, KernelOpKind::Copy));
        assert!(matches!(op_b.kind, KernelOpKind::Copy));
        assert_eq!(
            op_a.operands[0], op_b.operands[0],
            "Fix: both folded comparisons should reference the same synthesized Bool(false) op-id."
        );
    }

    #[test]
    fn recurses_into_child_bodies() {
        let mut child = empty_body();
        lit_u32(&mut child, 5, 10);
        binop(&mut child, BinOp::Lt, 10, 10, 11);
        let mut body = empty_body();
        body.child_bodies.push(child);
        let desc = cmp_self_false(&descriptor_with(body));
        let child_out = &desc.body.child_bodies[0];
        let op = child_out
            .ops
            .iter()
            .find(|op| op.result == Some(11))
            .unwrap();
        assert!(matches!(op.kind, KernelOpKind::Copy));
    }
}
