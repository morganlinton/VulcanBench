use super::body_index::BodyIndex;
use super::literal::ResultAllocator;
use crate::{KernelBody, KernelDescriptor, KernelOpKind, LiteralValue};
use vyre_foundation::ir::BinOp;

#[derive(Clone, Copy)]
pub(crate) struct CommutativeLitChainRule {
    pub(crate) op: BinOp,
    pub(crate) combine_literals: fn(u32, u32) -> Option<u32>,
}

pub(crate) fn combine_commutative_lit_chain(
    desc: &KernelDescriptor,
    rule: CommutativeLitChainRule,
) -> KernelDescriptor {
    let mut out = desc.clone();
    let mut allocator = ResultAllocator::for_body_tree(&out.body);
    out.body = combine_body(out.body, rule, &mut allocator);
    out
}

fn combine_body(
    mut body: KernelBody,
    rule: CommutativeLitChainRule,
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

        if let Some((x, a)) = candidate_with_lit(&body, &index, lhs, rule.op) {
            if let Some(b) = index.u32_lit(&body, rhs) {
                if let Some(combined) = (rule.combine_literals)(a, b) {
                    rewrites.push((idx, x, combined));
                    continue;
                }
            }
        }
        if let Some((x, a)) = candidate_with_lit(&body, &index, rhs, rule.op) {
            if let Some(b) = index.u32_lit(&body, lhs) {
                if let Some(combined) = (rule.combine_literals)(a, b) {
                    rewrites.push((idx, x, combined));
                }
            }
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

fn candidate_with_lit(
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
    if let Some(literal) = index.u32_lit(body, rhs) {
        return Some((lhs, literal));
    }
    if let Some(literal) = index.u32_lit(body, lhs) {
        return Some((rhs, literal));
    }
    None
}

#[cfg(test)]
pub(crate) mod test_support {
    use crate::{
        BindingLayout, Dispatch, KernelBody, KernelDescriptor, KernelOp, KernelOpKind, LiteralValue,
    };
    use vyre_foundation::ir::BinOp;

    pub(crate) fn empty_body() -> KernelBody {
        KernelBody {
            ops: Vec::new(),
            child_bodies: Vec::new(),
            literals: Vec::new(),
        }
    }

    pub(crate) fn descriptor_with(body: KernelBody) -> KernelDescriptor {
        KernelDescriptor {
            id: "commutative_lit_chain_test".into(),
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
            LiteralValue::U32(value) => value,
            _ => panic!("Fix: expected U32 literal"),
        }
    }

    pub(crate) struct CommutativeLitChainContract {
        pub(crate) rewrite: fn(&KernelDescriptor) -> KernelDescriptor,
        pub(crate) op: BinOp,
        pub(crate) combine_literals: fn(u32, u32) -> Option<u32>,
        pub(crate) first: u32,
        pub(crate) second: u32,
        pub(crate) combined: u32,
        pub(crate) overflow_first: u32,
        pub(crate) overflow_second: u32,
    }

    pub(crate) fn assert_commutative_lit_chain_contract(case: CommutativeLitChainContract) {
        assert_generated_literal_matrix_combines(&case);
        assert_all_symmetric_forms_combine(&case);
        assert_overflow_left_alone(&case);
        assert_multi_consumer_inner_left_alone(&case);
        assert_non_literal_outer_left_alone(&case);
        assert_rewrite_is_idempotent(&case);
        assert_recurses_into_child_bodies(&case);
    }

    fn assert_generated_literal_matrix_combines(case: &CommutativeLitChainContract) {
        let samples = [
            0u32, 1, 2, 3, 4, 7, 8, 15, 16, 31, 32, 63, 64, 127, 128, 255,
        ];
        let mut checked = 0usize;
        for first in samples {
            for second in samples {
                let combined = (case.combine_literals)(first, second)
                    .expect("Fix: generated non-overflow sample must combine");
                for inner_lit_on_left in [false, true] {
                    for outer_lit_on_left in [false, true] {
                        let mut body = empty_body();
                        nonliteral_source(&mut body, 0);
                        lit_u32(&mut body, first, 1);
                        if inner_lit_on_left {
                            binop(&mut body, case.op, 1, 0, 2);
                        } else {
                            binop(&mut body, case.op, 0, 1, 2);
                        }
                        lit_u32(&mut body, second, 3);
                        if outer_lit_on_left {
                            binop(&mut body, case.op, 3, 2, 4);
                        } else {
                            binop(&mut body, case.op, 2, 3, 4);
                        }

                        let desc = (case.rewrite)(&descriptor_with(body));
                        let outer = op_at(&desc, 4);
                        assert!(matches!(outer.kind, KernelOpKind::BinOpKind(op) if op == case.op));
                        assert_eq!(outer.operands[0], 0);
                        assert_eq!(lit_value_at(&desc, outer.operands[1]), combined);
                        checked += 1;
                    }
                }
            }
        }
        assert_eq!(
            checked, 1024,
            "Fix: generated commutative literal-chain matrix must cover all symmetric placements"
        );
    }

    fn assert_all_symmetric_forms_combine(case: &CommutativeLitChainContract) {
        for inner_lit_on_left in [false, true] {
            for outer_lit_on_left in [false, true] {
                let mut body = empty_body();
                nonliteral_source(&mut body, 0);
                lit_u32(&mut body, case.first, 1);
                if inner_lit_on_left {
                    binop(&mut body, case.op, 1, 0, 2);
                } else {
                    binop(&mut body, case.op, 0, 1, 2);
                }
                lit_u32(&mut body, case.second, 3);
                if outer_lit_on_left {
                    binop(&mut body, case.op, 3, 2, 4);
                } else {
                    binop(&mut body, case.op, 2, 3, 4);
                }

                let desc = (case.rewrite)(&descriptor_with(body));
                let outer = op_at(&desc, 4);
                assert!(matches!(outer.kind, KernelOpKind::BinOpKind(op) if op == case.op));
                assert_eq!(
                    outer.operands[0], 0,
                    "Fix: combined chain must preserve the non-literal source as operand 0"
                );
                assert_eq!(lit_value_at(&desc, outer.operands[1]), case.combined);
            }
        }
    }

    fn assert_overflow_left_alone(case: &CommutativeLitChainContract) {
        let mut body = empty_body();
        nonliteral_source(&mut body, 0);
        lit_u32(&mut body, case.overflow_first, 1);
        binop(&mut body, case.op, 0, 1, 2);
        lit_u32(&mut body, case.overflow_second, 3);
        binop(&mut body, case.op, 2, 3, 4);

        let desc = (case.rewrite)(&descriptor_with(body));
        let outer = op_at(&desc, 4);
        assert_eq!(
            outer.operands[0], 2,
            "Fix: refuse to fold when literal combination would overflow"
        );
    }

    fn assert_multi_consumer_inner_left_alone(case: &CommutativeLitChainContract) {
        let mut body = empty_body();
        nonliteral_source(&mut body, 0);
        lit_u32(&mut body, case.first, 1);
        binop(&mut body, case.op, 0, 1, 2);
        lit_u32(&mut body, case.second, 3);
        binop(&mut body, case.op, 2, 3, 4);
        binop(&mut body, case.op, 2, 0, 5);

        let desc = (case.rewrite)(&descriptor_with(body));
        let outer = op_at(&desc, 4);
        assert_eq!(
            outer.operands[0], 2,
            "Fix: inner chain op must have exactly one consumer"
        );
    }

    fn assert_non_literal_outer_left_alone(case: &CommutativeLitChainContract) {
        let mut body = empty_body();
        nonliteral_source(&mut body, 0);
        lit_u32(&mut body, case.first, 1);
        binop(&mut body, case.op, 0, 1, 2);
        lit_u32(&mut body, 1, 3);
        lit_u32(&mut body, 1, 4);
        binop(&mut body, case.op, 3, 4, 5);
        binop(&mut body, case.op, 2, 5, 6);

        let desc = (case.rewrite)(&descriptor_with(body));
        let outer = op_at(&desc, 6);
        assert_eq!(outer.operands[0], 2);
        assert_eq!(outer.operands[1], 5);
    }

    fn assert_rewrite_is_idempotent(case: &CommutativeLitChainContract) {
        let mut body = empty_body();
        nonliteral_source(&mut body, 0);
        lit_u32(&mut body, case.first, 1);
        binop(&mut body, case.op, 0, 1, 2);
        lit_u32(&mut body, case.second, 3);
        binop(&mut body, case.op, 2, 3, 4);

        let desc = descriptor_with(body);
        let once = (case.rewrite)(&desc);
        let twice = (case.rewrite)(&once);
        assert_eq!(once, twice);
    }

    fn assert_recurses_into_child_bodies(case: &CommutativeLitChainContract) {
        let mut child = empty_body();
        nonliteral_source(&mut child, 10);
        lit_u32(&mut child, case.first, 11);
        binop(&mut child, case.op, 10, 11, 12);
        lit_u32(&mut child, case.second, 13);
        binop(&mut child, case.op, 12, 13, 14);
        let mut body = empty_body();
        body.child_bodies.push(child);

        let desc = (case.rewrite)(&descriptor_with(body));
        let outer = desc.body.child_bodies[0]
            .ops
            .iter()
            .find(|op| op.result == Some(14))
            .unwrap();
        assert!(matches!(outer.kind, KernelOpKind::BinOpKind(op) if op == case.op));
        assert_eq!(outer.operands[0], 10);
        let lit_idx = desc.body.child_bodies[0]
            .ops
            .iter()
            .find(|op| op.result == Some(outer.operands[1]))
            .unwrap()
            .operands[0] as usize;
        assert_eq!(
            desc.body.child_bodies[0].literals[lit_idx],
            LiteralValue::U32(case.combined)
        );
    }
}
