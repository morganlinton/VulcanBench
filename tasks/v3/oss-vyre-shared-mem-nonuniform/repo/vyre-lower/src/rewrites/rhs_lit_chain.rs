use super::body_index::BodyIndex;
use super::literal::ResultAllocator;
use crate::{KernelBody, KernelDescriptor, KernelOpKind, LiteralValue};
use vyre_foundation::ir::BinOp;

#[derive(Clone, Copy)]
pub(crate) struct RhsLitChainRule {
    pub(crate) op: BinOp,
    pub(crate) combine_literals: fn(u32, u32) -> Option<u32>,
}

pub(crate) fn combine_rhs_lit_chain(
    desc: &KernelDescriptor,
    rule: RhsLitChainRule,
) -> KernelDescriptor {
    let mut out = desc.clone();
    let mut allocator = ResultAllocator::for_body_tree(&out.body);
    out.body = combine_body(out.body, rule, &mut allocator);
    out
}

fn combine_body(
    mut body: KernelBody,
    rule: RhsLitChainRule,
    allocator: &mut ResultAllocator,
) -> KernelBody {
    let index = BodyIndex::new(&body);

    let mut rewrites = Vec::new();
    for (idx, op) in body.ops.iter().enumerate() {
        if !matches!(op.kind, KernelOpKind::BinOpKind(bin) if bin == rule.op) {
            continue;
        }
        if op.operands.len() != 2 {
            continue;
        }
        let lhs = op.operands[0];
        let rhs = op.operands[1];
        let Some((x, a)) = candidate_with_rhs_lit(&body, &index, lhs, rule.op) else {
            continue;
        };
        let Some(b) = index.u32_lit(&body, rhs) else {
            continue;
        };
        if let Some(combined) = (rule.combine_literals)(a, b) {
            rewrites.push((idx, x, combined));
        }
    }

    for (op_idx, x_id, combined) in rewrites {
        let synth_id = allocator.push_literal(
            &mut body.ops,
            &mut body.literals,
            LiteralValue::U32(combined),
        );
        body.ops[op_idx].kind = KernelOpKind::BinOpKind(rule.op);
        body.ops[op_idx].operands = vec![x_id, synth_id];
    }

    body.child_bodies = body
        .child_bodies
        .into_iter()
        .map(|child| combine_body(child, rule, allocator))
        .collect();
    body
}

fn candidate_with_rhs_lit(
    body: &KernelBody,
    index: &BodyIndex,
    result_id: u32,
    op: BinOp,
) -> Option<(u32, u32)> {
    let producer = index.producer(body, result_id)?;
    if !matches!(producer.kind, KernelOpKind::BinOpKind(bin) if bin == op) {
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
    let literal = index.u32_lit(body, rhs)?;
    Some((lhs, literal))
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;
    use crate::{BindingLayout, Dispatch, KernelOp};

    pub(crate) fn empty_body() -> KernelBody {
        KernelBody {
            ops: Vec::new(),
            child_bodies: Vec::new(),
            literals: Vec::new(),
        }
    }

    pub(crate) fn descriptor_with(id: &'static str, body: KernelBody) -> KernelDescriptor {
        KernelDescriptor {
            id: id.into(),
            bindings: BindingLayout { slots: Vec::new() },
            dispatch: Dispatch::new(1, 1, 1),
            body,
        }
    }

    pub(crate) fn nonliteral_source(body: &mut KernelBody, result: u32) {
        body.ops.push(KernelOp {
            kind: KernelOpKind::GlobalInvocationId,
            operands: vec![0],
            result: Some(result),
        });
    }

    pub(crate) fn lit_u32(body: &mut KernelBody, value: u32, result: u32) {
        let pool_idx = body.literals.len() as u32;
        body.literals.push(LiteralValue::U32(value));
        body.ops.push(KernelOp {
            kind: KernelOpKind::Literal,
            operands: vec![pool_idx],
            result: Some(result),
        });
    }

    pub(crate) fn binop(body: &mut KernelBody, op: BinOp, lhs: u32, rhs: u32, result: u32) {
        body.ops.push(KernelOp {
            kind: KernelOpKind::BinOpKind(op),
            operands: vec![lhs, rhs],
            result: Some(result),
        });
    }

    pub(crate) fn op_at(desc: &KernelDescriptor, result: u32) -> &KernelOp {
        desc.body
            .ops
            .iter()
            .find(|op| op.result == Some(result))
            .expect("Fix: target op must exist")
    }

    pub(crate) fn lit_value_at(desc: &KernelDescriptor, id: u32) -> u32 {
        let op = op_at(desc, id);
        assert!(matches!(op.kind, KernelOpKind::Literal));
        let pool_idx = op.operands[0] as usize;
        match desc.body.literals[pool_idx] {
            LiteralValue::U32(v) => v,
            _ => panic!("Fix: expected U32 literal"),
        }
    }

    pub(crate) fn assert_rhs_chain_combines(
        descriptor_id: &'static str,
        rewrite: fn(&KernelDescriptor) -> KernelDescriptor,
        op: BinOp,
        inner_literal: u32,
        outer_literal: u32,
        expected_literal: u32,
    ) {
        let mut body = empty_body();
        nonliteral_source(&mut body, 0);
        lit_u32(&mut body, inner_literal, 1);
        binop(&mut body, op, 0, 1, 2);
        lit_u32(&mut body, outer_literal, 3);
        binop(&mut body, op, 2, 3, 4);
        let desc = rewrite(&descriptor_with(descriptor_id, body));
        let outer = op_at(&desc, 4);
        assert!(
            matches!(outer.kind, KernelOpKind::BinOpKind(bin) if bin == op),
            "Fix: rhs literal chain rewrite must preserve the outer operator."
        );
        assert_eq!(outer.operands[0], 0);
        assert_eq!(lit_value_at(&desc, outer.operands[1]), expected_literal);
    }

    pub(crate) fn assert_rhs_chain_left_alone(
        descriptor_id: &'static str,
        rewrite: fn(&KernelDescriptor) -> KernelDescriptor,
        op: BinOp,
        inner_literal: u32,
        outer_literal: u32,
        reason: &str,
    ) {
        let mut body = empty_body();
        nonliteral_source(&mut body, 0);
        lit_u32(&mut body, inner_literal, 1);
        binop(&mut body, op, 0, 1, 2);
        lit_u32(&mut body, outer_literal, 3);
        binop(&mut body, op, 2, 3, 4);
        let desc = rewrite(&descriptor_with(descriptor_id, body));
        let outer = op_at(&desc, 4);
        assert_eq!(outer.operands[0], 2, "{reason}");
    }

    pub(crate) fn assert_multi_consumer_rhs_chain_left_alone(
        descriptor_id: &'static str,
        rewrite: fn(&KernelDescriptor) -> KernelDescriptor,
        op: BinOp,
        inner_literal: u32,
        outer_literal: u32,
    ) {
        let mut body = empty_body();
        nonliteral_source(&mut body, 0);
        lit_u32(&mut body, inner_literal, 1);
        binop(&mut body, op, 0, 1, 2);
        lit_u32(&mut body, outer_literal, 3);
        binop(&mut body, op, 2, 3, 4);
        binop(&mut body, BinOp::Add, 2, 0, 5);
        let desc = rewrite(&descriptor_with(descriptor_id, body));
        let outer = op_at(&desc, 4);
        assert_eq!(
            outer.operands[0], 2,
            "Fix: inner op must have exactly one consumer for rhs-chain folding."
        );
    }

    pub(crate) fn assert_rhs_chain_rewrite_is_idempotent(
        descriptor_id: &'static str,
        rewrite: fn(&KernelDescriptor) -> KernelDescriptor,
        op: BinOp,
        inner_literal: u32,
        outer_literal: u32,
    ) {
        let mut body = empty_body();
        nonliteral_source(&mut body, 0);
        lit_u32(&mut body, inner_literal, 1);
        binop(&mut body, op, 0, 1, 2);
        lit_u32(&mut body, outer_literal, 3);
        binop(&mut body, op, 2, 3, 4);
        let desc = descriptor_with(descriptor_id, body);
        let once = rewrite(&desc);
        let twice = rewrite(&once);
        assert_eq!(once, twice);
    }

    pub(crate) fn assert_rhs_chain_recurses_into_child(
        descriptor_id: &'static str,
        rewrite: fn(&KernelDescriptor) -> KernelDescriptor,
        op: BinOp,
        inner_literal: u32,
        outer_literal: u32,
        expected_literal: u32,
    ) {
        let mut child = empty_body();
        nonliteral_source(&mut child, 10);
        lit_u32(&mut child, inner_literal, 11);
        binop(&mut child, op, 10, 11, 12);
        lit_u32(&mut child, outer_literal, 13);
        binop(&mut child, op, 12, 13, 14);
        let mut body = empty_body();
        body.child_bodies.push(child);
        let desc = rewrite(&descriptor_with(descriptor_id, body));
        let outer = desc.body.child_bodies[0]
            .ops
            .iter()
            .find(|op| op.result == Some(14))
            .expect("Fix: child outer op must exist after rhs-chain rewrite.");
        assert_eq!(outer.operands[0], 10);
        let lit_idx = desc.body.child_bodies[0]
            .ops
            .iter()
            .find(|op| op.result == Some(outer.operands[1]))
            .expect("Fix: rewritten child literal must exist.")
            .operands[0] as usize;
        assert_eq!(
            desc.body.child_bodies[0].literals[lit_idx],
            LiteralValue::U32(expected_literal)
        );
    }
}
