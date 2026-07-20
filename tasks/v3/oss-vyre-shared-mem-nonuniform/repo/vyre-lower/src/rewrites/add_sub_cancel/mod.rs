//! Add/Sub literal-cancel  -  fold:
//!     `Add(Sub(x, Lit(a)), Lit(a))` → `x`
//!     `Sub(Add(x, Lit(a)), Lit(a))` → `x`
//! Wrap-safe for U32: `(x − a) + a ≡ x (mod 2^32)` and
//! `(x + a) − a ≡ x (mod 2^32)`. No overflow check needed.
//!
//! Source-of-truth: `PERF_ROADMAP_2026-05-01.md` section A.4
//! (algebraic simplification family). Companion to `add_combine`,
//! `sub_combine`. Catches the residue left over when an earlier
//! pass folded the inner constant from a different shape (e.g. a
//! loop-invariant offset that was hoisted then later restored).
//!
//! Patterns rewritten (when both literals match in value, both are
//! U32, the inner BinOp has exactly one consumer):
//! - `Add(Sub(x, Lit(a)), Lit(a))` → `x`
//! - `Sub(Add(x, Lit(a)), Lit(a))` → `x`
//! - `Add(Lit(a), Sub(x, Lit(a)))` → `x`  (commuted Add)
//!
//! Out of scope:
//! - Non-matching literal pairs (`add_combine` / `sub_combine`
//!   already merge those when they can; otherwise they're not
//!   cancellable here).
//! - `Sub(Lit(a), Add(x, ...))` style  -  would need sign tracking.
//! - Multi-consumer inner ops.
//!
//! Recurses. Idempotent. Wired immediately after `sub_combine` in
//! `CANONICAL_REWRITE_PASSES`.

use super::body_index::BodyIndex;
use crate::{KernelBody, KernelDescriptor, KernelOpKind};
use vyre_foundation::ir::BinOp;

#[must_use]
pub fn add_sub_cancel(desc: &KernelDescriptor) -> KernelDescriptor {
    let mut out = desc.clone();
    out.body = add_sub_cancel_body(out.body);
    out
}

fn add_sub_cancel_body(mut body: KernelBody) -> KernelBody {
    let index = BodyIndex::new(&body);

    // (op_idx_to_replace, replacement_result_id_for_x)
    let mut rewrites: Vec<(usize, u32)> = Vec::new();
    for (idx, op) in body.ops.iter().enumerate() {
        let bin = match &op.kind {
            KernelOpKind::BinOpKind(b) => *b,
            _ => continue,
        };
        if op.operands.len() != 2 {
            continue;
        }
        let lhs = op.operands[0];
        let rhs = op.operands[1];

        match bin {
            BinOp::Add => {
                // Add(Sub(x, Lit(a)), Lit(a)) → x
                if let Some(a_outer) = index.u32_lit(&body, rhs) {
                    if let Some((x, a_inner)) = inner_with_rhs_lit(&body, &index, lhs, BinOp::Sub) {
                        if a_inner == a_outer {
                            rewrites.push((idx, x));
                            continue;
                        }
                    }
                }
                // Add(Lit(a), Sub(x, Lit(a))) → x  (commuted)
                if let Some(a_outer) = index.u32_lit(&body, lhs) {
                    if let Some((x, a_inner)) = inner_with_rhs_lit(&body, &index, rhs, BinOp::Sub) {
                        if a_inner == a_outer {
                            rewrites.push((idx, x));
                        }
                    }
                }
            }
            BinOp::Sub => {
                // Sub(Add(x, Lit(a)), Lit(a)) → x
                if let Some(a_outer) = index.u32_lit(&body, rhs) {
                    if let Some((x, a_inner)) = inner_with_rhs_lit(&body, &index, lhs, BinOp::Add) {
                        if a_inner == a_outer {
                            rewrites.push((idx, x));
                        }
                    }
                    // Also: Sub(Add(Lit(a), x), Lit(a)) → x  -  Add is commutative.
                    if let Some((x, a_inner)) = inner_with_lhs_lit(&body, &index, lhs, BinOp::Add) {
                        if a_inner == a_outer {
                            rewrites.push((idx, x));
                        }
                    }
                }
            }
            _ => {}
        }
    }

    // Apply: alias outer's result to x by patching every consumer.
    for (op_idx, replacement) in &rewrites {
        let Some(outer_result) = body.ops[*op_idx].result else {
            continue;
        };
        if outer_result == *replacement {
            continue;
        }
        for op in body.ops.iter_mut() {
            for operand in op.operands.iter_mut() {
                if *operand == outer_result {
                    *operand = *replacement;
                }
            }
        }
    }

    body.child_bodies = body
        .child_bodies
        .into_iter()
        .map(add_sub_cancel_body)
        .collect();
    body
}

fn inner_with_rhs_lit(
    body: &KernelBody,
    index: &BodyIndex,
    result_id: u32,
    inner_op: BinOp,
) -> Option<(u32, u32)> {
    let producer = index.producer(body, result_id)?;
    if !matches!(producer.kind, KernelOpKind::BinOpKind(b) if b == inner_op) {
        return None;
    }
    if producer.operands.len() != 2 {
        return None;
    }
    if !index.has_single_consumer(result_id) {
        return None;
    }
    let lhs = producer.operands[0];
    let rhs = producer.operands[1];
    let c = index.u32_lit(body, rhs)?;
    Some((lhs, c))
}

fn inner_with_lhs_lit(
    body: &KernelBody,
    index: &BodyIndex,
    result_id: u32,
    inner_op: BinOp,
) -> Option<(u32, u32)> {
    let producer = index.producer(body, result_id)?;
    if !matches!(producer.kind, KernelOpKind::BinOpKind(b) if b == inner_op) {
        return None;
    }
    if producer.operands.len() != 2 {
        return None;
    }
    if !index.has_single_consumer(result_id) {
        return None;
    }
    let lhs = producer.operands[0];
    let rhs = producer.operands[1];
    let c = index.u32_lit(body, lhs)?;
    Some((rhs, c))
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
            id: "add_sub_cancel_test".into(),
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

    fn store(body: &mut KernelBody, value_id: u32) {
        body.ops.push(KernelOp {
            kind: KernelOpKind::StoreGlobal,
            operands: vec![0, 0, value_id],
            result: None,
        });
    }

    fn store_value(desc: &KernelDescriptor) -> u32 {
        let s = desc
            .body
            .ops
            .iter()
            .find(|op| matches!(op.kind, KernelOpKind::StoreGlobal))
            .expect("Fix: store");
        s.operands[2]
    }

    #[test]
    fn add_after_sub_with_matching_lit_cancels() {
        // (x - 5) + 5 → x
        let mut body = empty_body();
        nonliteral_source(&mut body, 0);
        lit_u32(&mut body, 5, 1);
        binop(&mut body, BinOp::Sub, 0, 1, 2);
        lit_u32(&mut body, 5, 3);
        binop(&mut body, BinOp::Add, 2, 3, 4);
        store(&mut body, 4);
        let desc = add_sub_cancel(&descriptor_with(body));
        assert_eq!(store_value(&desc), 0);
    }

    #[test]
    fn sub_after_add_with_matching_lit_cancels() {
        // (x + 5) - 5 → x
        let mut body = empty_body();
        nonliteral_source(&mut body, 0);
        lit_u32(&mut body, 5, 1);
        binop(&mut body, BinOp::Add, 0, 1, 2);
        lit_u32(&mut body, 5, 3);
        binop(&mut body, BinOp::Sub, 2, 3, 4);
        store(&mut body, 4);
        let desc = add_sub_cancel(&descriptor_with(body));
        assert_eq!(store_value(&desc), 0);
    }

    #[test]
    fn commuted_add_lit_left_cancels() {
        // 5 + (x - 5) → x
        let mut body = empty_body();
        nonliteral_source(&mut body, 0);
        lit_u32(&mut body, 5, 1);
        binop(&mut body, BinOp::Sub, 0, 1, 2);
        lit_u32(&mut body, 5, 3);
        binop(&mut body, BinOp::Add, 3, 2, 4);
        store(&mut body, 4);
        let desc = add_sub_cancel(&descriptor_with(body));
        assert_eq!(store_value(&desc), 0);
    }

    #[test]
    fn sub_after_commuted_add_cancels() {
        // (5 + x) - 5 → x
        let mut body = empty_body();
        nonliteral_source(&mut body, 0);
        lit_u32(&mut body, 5, 1);
        binop(&mut body, BinOp::Add, 1, 0, 2);
        lit_u32(&mut body, 5, 3);
        binop(&mut body, BinOp::Sub, 2, 3, 4);
        store(&mut body, 4);
        let desc = add_sub_cancel(&descriptor_with(body));
        assert_eq!(store_value(&desc), 0);
    }

    #[test]
    fn mismatched_literals_left_alone() {
        // (x - 5) + 7  -  different residue, must NOT cancel.
        let mut body = empty_body();
        nonliteral_source(&mut body, 0);
        lit_u32(&mut body, 5, 1);
        binop(&mut body, BinOp::Sub, 0, 1, 2);
        lit_u32(&mut body, 7, 3);
        binop(&mut body, BinOp::Add, 2, 3, 4);
        store(&mut body, 4);
        let desc = add_sub_cancel(&descriptor_with(body));
        assert_eq!(
            store_value(&desc),
            4,
            "Fix: only equal-literal pairs should cancel."
        );
    }

    #[test]
    fn inner_with_multiple_consumers_left_alone() {
        let mut body = empty_body();
        nonliteral_source(&mut body, 0);
        lit_u32(&mut body, 5, 1);
        binop(&mut body, BinOp::Sub, 0, 1, 2);
        lit_u32(&mut body, 5, 3);
        binop(&mut body, BinOp::Add, 2, 3, 4);
        binop(&mut body, BinOp::Mul, 2, 0, 5);
        store(&mut body, 4);
        let desc = add_sub_cancel(&descriptor_with(body));
        assert_eq!(store_value(&desc), 4);
    }

    #[test]
    fn rewrite_is_idempotent() {
        let mut body = empty_body();
        nonliteral_source(&mut body, 0);
        lit_u32(&mut body, 5, 1);
        binop(&mut body, BinOp::Sub, 0, 1, 2);
        lit_u32(&mut body, 5, 3);
        binop(&mut body, BinOp::Add, 2, 3, 4);
        store(&mut body, 4);
        let desc = descriptor_with(body);
        let once = add_sub_cancel(&desc);
        let twice = add_sub_cancel(&once);
        assert_eq!(once, twice);
    }

    #[test]
    fn recurses_into_child_bodies() {
        let mut child = empty_body();
        nonliteral_source(&mut child, 10);
        lit_u32(&mut child, 5, 11);
        binop(&mut child, BinOp::Sub, 10, 11, 12);
        lit_u32(&mut child, 5, 13);
        binop(&mut child, BinOp::Add, 12, 13, 14);
        store(&mut child, 14);
        let mut body = empty_body();
        body.child_bodies.push(child);
        let desc = add_sub_cancel(&descriptor_with(body));
        let store_op = desc.body.child_bodies[0]
            .ops
            .iter()
            .find(|op| matches!(op.kind, KernelOpKind::StoreGlobal))
            .unwrap();
        assert_eq!(store_op.operands[2], 10);
    }
}
