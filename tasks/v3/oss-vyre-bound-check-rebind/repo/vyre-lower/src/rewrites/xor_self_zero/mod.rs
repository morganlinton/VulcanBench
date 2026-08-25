//! Self-XOR zeroing  -  `BitXor(x, x)` always evaluates to `0` (any
//! integer width). `BitXor` is integer-only in the descriptor IR so
//! this rewrite is safe with no dtype tracking.
//!
//! Source-of-truth: `PERF_ROADMAP_2026-05-01.md` section A.4
//! (algebraic simplification family). Companion to `cmp_self_false`,
//! `bitwise_idemp`. Explicitly called out as a non-pattern in
//! `identity_elim`'s docs because that pass cannot synthesise typed
//! literals.
//!
//! Pattern rewritten:
//! - `BitXor(x, x)` → `Copy(synth Lit(U32(0)))`
//!
//! Out of scope deliberately:
//! - `Sub(x, x) → 0` is UNSAFE for floats (`NaN - NaN` is `NaN`, not
//!   `0`). Until the IR carries a per-op dtype tag at the rewrite
//!   layer this stays out.
//! - `Eq(x, x) → true`, `Ne(x, x) → false` are UNSAFE for floats
//!   (`NaN == NaN` is `false`). See `cmp_self_false`'s non-patterns.
//!
//! Recurses. Idempotent. Wired immediately after `cmp_self_false` in
//! `CANONICAL_REWRITE_PASSES`.

use super::literal::ResultAllocator;
use crate::{KernelBody, KernelDescriptor, KernelOpKind, LiteralValue};
use vyre_foundation::ir::BinOp;

#[must_use]
pub fn xor_self_zero(desc: &KernelDescriptor) -> KernelDescriptor {
    let mut out = desc.clone();
    let mut allocator = ResultAllocator::for_body_tree(&out.body);
    out.body = xor_self_zero_body(out.body, &mut allocator);
    out
}

fn xor_self_zero_body(mut body: KernelBody, allocator: &mut ResultAllocator) -> KernelBody {
    let mut rewrites: Vec<usize> = Vec::new();
    for (idx, op) in body.ops.iter().enumerate() {
        let bin = match &op.kind {
            KernelOpKind::BinOpKind(b) => *b,
            _ => continue,
        };
        if !matches!(bin, BinOp::BitXor) {
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
            .map(|child| xor_self_zero_body(child, allocator))
            .collect();
        return body;
    }

    let synth_id = allocator.push_literal(&mut body.ops, &mut body.literals, LiteralValue::U32(0));

    for op_idx in rewrites {
        body.ops[op_idx].kind = KernelOpKind::Copy;
        body.ops[op_idx].operands = vec![synth_id];
    }

    body.child_bodies = body
        .child_bodies
        .into_iter()
        .map(|child| xor_self_zero_body(child, allocator))
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
            id: "xor_self_zero_test".into(),
            bindings: BindingLayout { slots: Vec::new() },
            dispatch: Dispatch::new(1, 1, 1),
            body,
        }
    }

    fn nonliteral_source(body: &mut KernelBody, result: u32) {
        body.ops.push(KernelOp {
            kind: KernelOpKind::GlobalInvocationId,
            operands: vec![0],
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

    #[test]
    fn xor_with_self_collapses_to_zero_copy() {
        let mut body = empty_body();
        nonliteral_source(&mut body, 0);
        binop(&mut body, BinOp::BitXor, 0, 0, 1);
        let desc = xor_self_zero(&descriptor_with(body));
        let folded = op_at(&desc, 1);
        assert!(matches!(folded.kind, KernelOpKind::Copy));
        let synth_id = folded.operands[0];
        let synth = op_at(&desc, synth_id);
        assert!(matches!(synth.kind, KernelOpKind::Literal));
        let pool_idx = synth.operands[0] as usize;
        assert_eq!(desc.body.literals[pool_idx], LiteralValue::U32(0));
    }

    #[test]
    fn xor_with_distinct_operands_left_alone() {
        let mut body = empty_body();
        nonliteral_source(&mut body, 0);
        nonliteral_source(&mut body, 1);
        binop(&mut body, BinOp::BitXor, 0, 1, 2);
        let desc = xor_self_zero(&descriptor_with(body));
        let xor = op_at(&desc, 2);
        assert!(matches!(xor.kind, KernelOpKind::BinOpKind(BinOp::BitXor)));
        assert_eq!(xor.operands, vec![0, 1]);
    }

    #[test]
    fn other_self_binops_left_alone() {
        // Sub(x, x) is intentionally not handled  -  float NaN.
        let mut body = empty_body();
        nonliteral_source(&mut body, 0);
        binop(&mut body, BinOp::Sub, 0, 0, 1);
        let desc = xor_self_zero(&descriptor_with(body));
        let sub = op_at(&desc, 1);
        assert!(matches!(sub.kind, KernelOpKind::BinOpKind(BinOp::Sub)));
    }

    #[test]
    fn synth_lit_is_shared_across_multiple_rewrites() {
        let mut body = empty_body();
        nonliteral_source(&mut body, 0);
        nonliteral_source(&mut body, 1);
        binop(&mut body, BinOp::BitXor, 0, 0, 2);
        binop(&mut body, BinOp::BitXor, 1, 1, 3);
        let desc = xor_self_zero(&descriptor_with(body));
        let a = op_at(&desc, 2);
        let b = op_at(&desc, 3);
        assert_eq!(
            a.operands[0], b.operands[0],
            "Fix: both rewrites should reference the same synth literal."
        );
    }

    #[test]
    fn rewrite_is_idempotent() {
        let mut body = empty_body();
        nonliteral_source(&mut body, 0);
        binop(&mut body, BinOp::BitXor, 0, 0, 1);
        let desc = descriptor_with(body);
        let once = xor_self_zero(&desc);
        let twice = xor_self_zero(&once);
        assert_eq!(once, twice);
    }

    #[test]
    fn recurses_into_child_bodies() {
        let mut child = empty_body();
        nonliteral_source(&mut child, 0);
        binop(&mut child, BinOp::BitXor, 0, 0, 1);
        let mut body = empty_body();
        body.child_bodies.push(child);
        let desc = xor_self_zero(&descriptor_with(body));
        let folded = desc.body.child_bodies[0]
            .ops
            .iter()
            .find(|op| op.result == Some(1))
            .unwrap();
        assert!(matches!(folded.kind, KernelOpKind::Copy));
    }
}
