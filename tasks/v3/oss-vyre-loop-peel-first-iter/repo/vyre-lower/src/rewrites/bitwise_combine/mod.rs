//! Bitwise-chain combine  -  fold associative bitwise chains where
//! the inner op shares a literal with the outer of the same kind.
//! Companion to `add_combine` / `mul_combine` for the BitAnd /
//! BitOr / BitXor monoids.
//!
//! Source-of-truth: `PERF_ROADMAP_2026-05-01.md` section A.4
//! (algebraic simplification family).
//!
//! Patterns rewritten (BitAnd, BitOr, BitXor  -  all U32 literal):
//! - `Op(Op(x, Lit(a)), Lit(b))` → `Op(x, Lit(a OP b))`
//! - `Op(Lit(b), Op(x, Lit(a)))` → `Op(x, Lit(a OP b))`
//! - `Op(Lit(b), Op(Lit(a), x))` → `Op(x, Lit(a OP b))`
//! - `Op(Op(Lit(a), x), Lit(b))` → `Op(x, Lit(a OP b))`
//!
//! All three bitwise ops are total (no overflow), so unlike Add/Mul
//! the combine ALWAYS fires when both literals are present and the
//! inner op has exactly one consumer.
//!
//! Out of scope:
//! - Mixed bitwise chains (`BitAnd(BitOr(...), Lit)`)  -  different
//!   ops; not the same algebraic identity.
//!
//! Recurses. Idempotent. Wired into `CANONICAL_REWRITE_PASSES`
//! immediately after `mul_combine`.

use super::body_index::BodyIndex;
use super::literal::ResultAllocator;
use crate::{KernelBody, KernelDescriptor, KernelOpKind, LiteralValue};
use vyre_foundation::ir::BinOp;

#[must_use]
pub fn bitwise_combine(desc: &KernelDescriptor) -> KernelDescriptor {
    let mut out = desc.clone();
    let mut allocator = ResultAllocator::for_body_tree(&out.body);
    out.body = bitwise_combine_body(out.body, &mut allocator);
    out
}

fn bitwise_combine_body(mut body: KernelBody, allocator: &mut ResultAllocator) -> KernelBody {
    let index = BodyIndex::new(&body);

    // (op_idx, x_id, combined_lit, op_kind)
    let mut rewrites: Vec<(usize, u32, u32, BinOp)> = Vec::new();
    for (idx, op) in body.ops.iter().enumerate() {
        let outer = match &op.kind {
            KernelOpKind::BinOpKind(b @ (BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor)) => *b,
            _ => continue,
        };
        if op.operands.len() != 2 {
            continue;
        }
        let lhs = op.operands[0];
        let rhs = op.operands[1];

        if let Some((x, a)) = candidate_bitwise_with_lit(&body, &index, lhs, outer) {
            if let Some(b) = index.u32_lit(&body, rhs) {
                let combined = apply_bitwise(outer, a, b);
                rewrites.push((idx, x, combined, outer));
                continue;
            }
        }
        if let Some((x, a)) = candidate_bitwise_with_lit(&body, &index, rhs, outer) {
            if let Some(b) = index.u32_lit(&body, lhs) {
                let combined = apply_bitwise(outer, a, b);
                rewrites.push((idx, x, combined, outer));
            }
        }
    }

    for (op_idx, x_id, combined, op_kind) in rewrites {
        let synth_id = allocator.push_literal(
            &mut body.ops,
            &mut body.literals,
            LiteralValue::U32(combined),
        );
        body.ops[op_idx].kind = KernelOpKind::BinOpKind(op_kind);
        body.ops[op_idx].operands = vec![x_id, synth_id];
    }

    body.child_bodies = body
        .child_bodies
        .into_iter()
        .map(|child| bitwise_combine_body(child, allocator))
        .collect();
    body
}

fn apply_bitwise(op: BinOp, a: u32, b: u32) -> u32 {
    match op {
        BinOp::BitAnd => a & b,
        BinOp::BitOr => a | b,
        BinOp::BitXor => a ^ b,
        _ => unreachable!("Fix: bitwise_combine should only be invoked for BitAnd/BitOr/BitXor."),
    }
}

fn candidate_bitwise_with_lit(
    body: &KernelBody,
    index: &BodyIndex,
    result_id: u32,
    expected_op: BinOp,
) -> Option<(u32, u32)> {
    let producer = index.producer(body, result_id)?;
    let inner_op = match &producer.kind {
        KernelOpKind::BinOpKind(b) => *b,
        _ => return None,
    };
    if inner_op != expected_op {
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
    if let Some(c) = index.u32_lit(body, rhs) {
        return Some((lhs, c));
    }
    if let Some(c) = index.u32_lit(body, lhs) {
        return Some((rhs, c));
    }
    None
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
            id: "bitwise_combine_test".into(),
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

    fn op_at(desc: &KernelDescriptor, result: u32) -> &KernelOp {
        desc.body
            .ops
            .iter()
            .find(|op| op.result == Some(result))
            .expect("Fix: target op must exist")
    }

    fn lit_value_at(desc: &KernelDescriptor, id: u32) -> u32 {
        let op = op_at(desc, id);
        assert!(matches!(op.kind, KernelOpKind::Literal));
        let pool_idx = op.operands[0] as usize;
        match desc.body.literals[pool_idx] {
            LiteralValue::U32(v) => v,
            _ => panic!(),
        }
    }

    fn assert_combines(op: BinOp, a: u32, b: u32, expected: u32) {
        let mut body = empty_body();
        nonliteral_source(&mut body, 0);
        lit_u32(&mut body, a, 1);
        binop(&mut body, op, 0, 1, 2);
        lit_u32(&mut body, b, 3);
        binop(&mut body, op, 2, 3, 4);
        let desc = bitwise_combine(&descriptor_with(body));
        let outer = op_at(&desc, 4);
        assert!(matches!(outer.kind, KernelOpKind::BinOpKind(o) if o == op));
        assert_eq!(outer.operands[0], 0);
        assert_eq!(lit_value_at(&desc, outer.operands[1]), expected);
    }

    #[test]
    fn bitand_chain_combines() {
        assert_combines(BinOp::BitAnd, 0xFF00, 0x0FF0, 0x0F00);
    }

    #[test]
    fn bitor_chain_combines() {
        assert_combines(BinOp::BitOr, 0x00F0, 0x0F00, 0x0FF0);
    }

    #[test]
    fn bitxor_chain_combines() {
        assert_combines(BinOp::BitXor, 0xFFFF, 0xF0F0, 0x0F0F);
    }

    #[test]
    fn mixed_bitwise_chain_left_alone() {
        // BitAnd outer with BitOr inner  -  different ops, no combine.
        let mut body = empty_body();
        nonliteral_source(&mut body, 0);
        lit_u32(&mut body, 0xFF, 1);
        binop(&mut body, BinOp::BitOr, 0, 1, 2);
        lit_u32(&mut body, 0x0F, 3);
        binop(&mut body, BinOp::BitAnd, 2, 3, 4);
        let desc = bitwise_combine(&descriptor_with(body));
        let outer = op_at(&desc, 4);
        assert!(matches!(outer.kind, KernelOpKind::BinOpKind(BinOp::BitAnd)));
        assert_eq!(
            outer.operands[0], 2,
            "Fix: mixed BitAnd-BitOr chain must NOT combine."
        );
    }

    #[test]
    fn inner_with_multiple_consumers_left_alone() {
        let mut body = empty_body();
        nonliteral_source(&mut body, 0);
        lit_u32(&mut body, 0xFF, 1);
        binop(&mut body, BinOp::BitAnd, 0, 1, 2);
        lit_u32(&mut body, 0x0F, 3);
        binop(&mut body, BinOp::BitAnd, 2, 3, 4);
        binop(&mut body, BinOp::BitAnd, 2, 0, 5);
        let desc = bitwise_combine(&descriptor_with(body));
        let outer = op_at(&desc, 4);
        assert_eq!(outer.operands[0], 2);
    }

    #[test]
    fn rewrite_is_idempotent() {
        let mut body = empty_body();
        nonliteral_source(&mut body, 0);
        lit_u32(&mut body, 0xFF, 1);
        binop(&mut body, BinOp::BitAnd, 0, 1, 2);
        lit_u32(&mut body, 0x0F, 3);
        binop(&mut body, BinOp::BitAnd, 2, 3, 4);
        let desc = descriptor_with(body);
        let once = bitwise_combine(&desc);
        let twice = bitwise_combine(&once);
        assert_eq!(once, twice);
    }

    #[test]
    fn recurses_into_child_bodies() {
        let mut child = empty_body();
        child.ops.push(KernelOp {
            kind: KernelOpKind::GlobalInvocationId,
            operands: vec![0],
            result: Some(10),
        });
        lit_u32(&mut child, 0xCC, 11);
        binop(&mut child, BinOp::BitOr, 10, 11, 12);
        lit_u32(&mut child, 0x33, 13);
        binop(&mut child, BinOp::BitOr, 12, 13, 14);
        let mut body = empty_body();
        body.child_bodies.push(child);
        let desc = bitwise_combine(&descriptor_with(body));
        let outer = desc.body.child_bodies[0]
            .ops
            .iter()
            .find(|op| op.result == Some(14))
            .unwrap();
        assert_eq!(outer.operands[0], 10);
        let lit_idx = desc.body.child_bodies[0]
            .ops
            .iter()
            .find(|op| op.result == Some(outer.operands[1]))
            .unwrap()
            .operands[0] as usize;
        assert_eq!(
            desc.body.child_bodies[0].literals[lit_idx],
            LiteralValue::U32(0xFF)
        );
    }
}
