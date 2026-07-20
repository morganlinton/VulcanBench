//! Boolean simplification rewrite.
//!
//! Source-of-truth: `PERF_ROADMAP_2026-05-01.md` section A.4  -  boolean
//! simplification is one of the 20 classical compiler passes named in
//! the perf roadmap. Patterns rewritten:
//!
//! - `LogicalNot(LogicalNot(x))` → `x` (double-negation elimination)
//! - `LogicalNot(BoolLit)` → opposite `BoolLit` (constant fold over Bool)
//! - `And(x, x)` → `x` (idempotence)
//! - `Or(x, x)` → `x` (idempotence)
//! - `BitXor(x, x)` → `Literal(0)` (self-cancellation, integer-only)
//! - `Eq(LitU32(a), LitU32(b))` → `BoolLit(a == b)` (literal compare)
//! - `Ne(LitU32(a), LitU32(b))` → `BoolLit(a != b)`
//!
//! What this pass does NOT do (out of scope, deliberately):
//! - De Morgan's laws  -  they don't reduce op count, just rebalance
//!   structure; downstream CSE handles the equality cases this would
//!   produce.
//! - `Eq(x, x)` → `true`  -  unsafe under f32 because NaN != NaN.
//! - Float `Add(x, 0.0)` / `Mul(x, 1.0)`  -  those are wrong under
//!   strict-FP (they affect signed zero / NaN propagation); identity_elim
//!   handles the integer cases.
//!
//! Recurses into nested control flow. Idempotent. Wired into
//! `CANONICAL_REWRITE_PASSES` after `identity_elim` (which handles the
//! left/right-identity rules this leans on for stable input).

use super::body_index::BodyIndex;
use super::literal::ResultAllocator;
use crate::{KernelBody, KernelDescriptor, KernelOpKind, LiteralValue};
use vyre_foundation::ir::{BinOp, UnOp};

#[must_use]
pub fn boolean_simplify(desc: &KernelDescriptor) -> KernelDescriptor {
    let mut out = desc.clone();
    let mut allocator = ResultAllocator::for_body_tree(&out.body);
    out.body = boolean_simplify_body(out.body, &mut allocator);
    out
}

fn boolean_simplify_body(mut body: KernelBody, allocator: &mut ResultAllocator) -> KernelBody {
    let index = BodyIndex::new(&body);

    enum Rewrite {
        ReplaceWithExisting { op_idx: usize, replace_id: u32 },
        ReplaceWithBoolLit { op_idx: usize, value: bool },
        ReplaceWithU32Lit { op_idx: usize, value: u32 },
    }
    let mut rewrites: Vec<Rewrite> = Vec::new();

    for (idx, op) in body.ops.iter().enumerate() {
        match &op.kind {
            KernelOpKind::UnOpKind(UnOp::LogicalNot) => {
                if op.operands.len() != 1 {
                    continue;
                }
                let inner = op.operands[0];
                let Some(producer_op) = index.producer(&body, inner) else {
                    continue;
                };
                // LogicalNot(LogicalNot(x)) → x
                if matches!(producer_op.kind, KernelOpKind::UnOpKind(UnOp::LogicalNot))
                    && producer_op.operands.len() == 1
                {
                    rewrites.push(Rewrite::ReplaceWithExisting {
                        op_idx: idx,
                        replace_id: producer_op.operands[0],
                    });
                    continue;
                }
                // LogicalNot(BoolLit) → opposite BoolLit
                if let Some(value) = index.bool_lit(&body, inner) {
                    rewrites.push(Rewrite::ReplaceWithBoolLit {
                        op_idx: idx,
                        value: !value,
                    });
                }
            }
            KernelOpKind::BinOpKind(bin) => {
                if op.operands.len() != 2 {
                    continue;
                }
                let lhs = op.operands[0];
                let rhs = op.operands[1];
                match bin {
                    // Idempotent: And(x, x) / Or(x, x) → x.
                    BinOp::And | BinOp::Or if lhs == rhs => {
                        rewrites.push(Rewrite::ReplaceWithExisting {
                            op_idx: idx,
                            replace_id: lhs,
                        });
                    }
                    // Self-cancellation: BitXor(x, x) → 0.
                    BinOp::BitXor if lhs == rhs => {
                        rewrites.push(Rewrite::ReplaceWithU32Lit {
                            op_idx: idx,
                            value: 0,
                        });
                    }
                    // Literal compare: Eq/Ne over two U32 literals.
                    BinOp::Eq | BinOp::Ne => {
                        if let Some((lhs_lit, rhs_lit)) = u32_lit_pair(&body, &index, lhs, rhs) {
                            let value = match bin {
                                BinOp::Eq => lhs_lit == rhs_lit,
                                BinOp::Ne => lhs_lit != rhs_lit,
                                _ => unreachable!(),
                            };
                            rewrites.push(Rewrite::ReplaceWithBoolLit { op_idx: idx, value });
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    // Apply: rewrites that produce a literal allocate a new Literal op
    // (and patch the original to a Copy of the new id); rewrites that
    // replace with an existing id patch the op to a Copy.
    for r in rewrites {
        match r {
            Rewrite::ReplaceWithExisting { op_idx, replace_id } => {
                body.ops[op_idx].kind = KernelOpKind::Copy;
                body.ops[op_idx].operands = vec![replace_id];
            }
            Rewrite::ReplaceWithBoolLit { op_idx, value } => {
                let synth_id = allocator.push_literal(
                    &mut body.ops,
                    &mut body.literals,
                    LiteralValue::Bool(value),
                );
                body.ops[op_idx].kind = KernelOpKind::Copy;
                body.ops[op_idx].operands = vec![synth_id];
            }
            Rewrite::ReplaceWithU32Lit { op_idx, value } => {
                let synth_id = allocator.push_literal(
                    &mut body.ops,
                    &mut body.literals,
                    LiteralValue::U32(value),
                );
                body.ops[op_idx].kind = KernelOpKind::Copy;
                body.ops[op_idx].operands = vec![synth_id];
            }
        }
    }

    body.child_bodies = body
        .child_bodies
        .into_iter()
        .map(|child| boolean_simplify_body(child, allocator))
        .collect();
    body
}

fn u32_lit_pair(body: &KernelBody, index: &BodyIndex, lhs: u32, rhs: u32) -> Option<(u32, u32)> {
    let lhs_value = index.u32_lit(body, lhs)?;
    let rhs_value = index.u32_lit(body, rhs)?;
    Some((lhs_value, rhs_value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BindingLayout, Dispatch, KernelOp};
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
            id: "boolean_simplify_test".into(),
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

    fn op_kind_for_result(desc: &KernelDescriptor, result: u32) -> &KernelOpKind {
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
    fn double_logical_not_eliminated() {
        let mut body = empty_body();
        lit_bool(&mut body, true, 0);
        unop(&mut body, UnOp::LogicalNot, 0, 1); // !true
        unop(&mut body, UnOp::LogicalNot, 1, 2); // !!true → true
        let desc = boolean_simplify(&descriptor_with(body));
        assert_eq!(
            copied_source(&desc, 2),
            0,
            "must alias to the inner Bool literal"
        );
    }

    #[test]
    fn logical_not_of_bool_literal_folds_to_opposite() {
        let mut body = empty_body();
        lit_bool(&mut body, false, 0);
        unop(&mut body, UnOp::LogicalNot, 0, 1); // !false → true
        let desc = boolean_simplify(&descriptor_with(body));
        let copied = copied_source(&desc, 1);
        let copied_op = desc
            .body
            .ops
            .iter()
            .find(|op| op.result == Some(copied))
            .unwrap();
        assert!(matches!(copied_op.kind, KernelOpKind::Literal));
        let pool_idx = copied_op.operands[0] as usize;
        assert_eq!(desc.body.literals[pool_idx], LiteralValue::Bool(true));
    }

    #[test]
    fn and_idempotent_collapses_to_self() {
        let mut body = empty_body();
        lit_bool(&mut body, true, 0); // dummy bool source
        binop(&mut body, BinOp::And, 0, 0, 1);
        let desc = boolean_simplify(&descriptor_with(body));
        assert_eq!(copied_source(&desc, 1), 0);
    }

    #[test]
    fn or_idempotent_collapses_to_self() {
        let mut body = empty_body();
        lit_bool(&mut body, false, 0);
        binop(&mut body, BinOp::Or, 0, 0, 1);
        let desc = boolean_simplify(&descriptor_with(body));
        assert_eq!(copied_source(&desc, 1), 0);
    }

    #[test]
    fn xor_self_collapses_to_zero() {
        let mut body = empty_body();
        lit_u32(&mut body, 7, 0);
        binop(&mut body, BinOp::BitXor, 0, 0, 1);
        let desc = boolean_simplify(&descriptor_with(body));
        let zero_id = copied_source(&desc, 1);
        let zero_op = desc
            .body
            .ops
            .iter()
            .find(|op| op.result == Some(zero_id))
            .unwrap();
        assert!(matches!(zero_op.kind, KernelOpKind::Literal));
        let pool_idx = zero_op.operands[0] as usize;
        assert_eq!(desc.body.literals[pool_idx], LiteralValue::U32(0));
    }

    #[test]
    fn eq_of_two_distinct_u32_literals_folds_to_false() {
        let mut body = empty_body();
        lit_u32(&mut body, 3, 0);
        lit_u32(&mut body, 5, 1);
        binop(&mut body, BinOp::Eq, 0, 1, 2);
        let desc = boolean_simplify(&descriptor_with(body));
        let folded = copied_source(&desc, 2);
        let folded_op = desc
            .body
            .ops
            .iter()
            .find(|op| op.result == Some(folded))
            .unwrap();
        let pool_idx = folded_op.operands[0] as usize;
        assert_eq!(desc.body.literals[pool_idx], LiteralValue::Bool(false));
    }

    #[test]
    fn ne_of_two_equal_u32_literals_folds_to_false() {
        let mut body = empty_body();
        lit_u32(&mut body, 7, 0);
        lit_u32(&mut body, 7, 1);
        binop(&mut body, BinOp::Ne, 0, 1, 2);
        let desc = boolean_simplify(&descriptor_with(body));
        let folded = copied_source(&desc, 2);
        let folded_op = desc
            .body
            .ops
            .iter()
            .find(|op| op.result == Some(folded))
            .unwrap();
        let pool_idx = folded_op.operands[0] as usize;
        assert_eq!(desc.body.literals[pool_idx], LiteralValue::Bool(false));
    }

    #[test]
    fn no_change_when_pattern_does_not_match() {
        let mut body = empty_body();
        lit_bool(&mut body, true, 0);
        lit_bool(&mut body, false, 1);
        binop(&mut body, BinOp::And, 0, 1, 2); // distinct operands → not idempotent
        let original = descriptor_with(body);
        let desc = boolean_simplify(&original);
        assert_eq!(
            op_kind_for_result(&desc, 2),
            &KernelOpKind::BinOpKind(BinOp::And)
        );
    }

    #[test]
    fn rewrite_is_idempotent() {
        let mut body = empty_body();
        lit_bool(&mut body, true, 0);
        unop(&mut body, UnOp::LogicalNot, 0, 1);
        unop(&mut body, UnOp::LogicalNot, 1, 2);
        let desc = descriptor_with(body);
        let once = boolean_simplify(&desc);
        let twice = boolean_simplify(&once);
        assert_eq!(once, twice);
    }

    #[test]
    fn recurses_into_child_bodies() {
        let mut child = empty_body();
        lit_bool(&mut child, true, 0);
        unop(&mut child, UnOp::LogicalNot, 0, 1);
        unop(&mut child, UnOp::LogicalNot, 1, 2);

        let mut body = empty_body();
        body.child_bodies.push(child);
        let desc = boolean_simplify(&descriptor_with(body));
        let child_out = &desc.body.child_bodies[0];
        let copy_op = child_out
            .ops
            .iter()
            .find(|op| op.result == Some(2))
            .unwrap();
        assert!(matches!(copy_op.kind, KernelOpKind::Copy));
        assert_eq!(copy_op.operands[0], 0);
    }
}
