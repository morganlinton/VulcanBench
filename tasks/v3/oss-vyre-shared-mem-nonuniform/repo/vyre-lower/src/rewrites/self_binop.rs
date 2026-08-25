use crate::{KernelBody, KernelDescriptor, KernelOpKind};
use vyre_foundation::ir::BinOp;

pub(super) fn rewrite_self_binops<F>(desc: &KernelDescriptor, should_rewrite: F) -> KernelDescriptor
where
    F: Fn(BinOp) -> bool + Copy,
{
    let mut out = desc.clone();
    out.body = rewrite_body(out.body, should_rewrite);
    out
}

fn rewrite_body<F>(mut body: KernelBody, should_rewrite: F) -> KernelBody
where
    F: Fn(BinOp) -> bool + Copy,
{
    for op in &mut body.ops {
        let bin = match &op.kind {
            KernelOpKind::BinOpKind(bin) => *bin,
            _ => continue,
        };
        if !should_rewrite(bin) || op.operands.len() != 2 || op.operands[0] != op.operands[1] {
            continue;
        }

        let replacement = op.operands[0];
        op.kind = KernelOpKind::Copy;
        op.operands.clear();
        op.operands.push(replacement);
    }

    body.child_bodies = body
        .child_bodies
        .into_iter()
        .map(|child| rewrite_body(child, should_rewrite))
        .collect();
    body
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

    pub(crate) fn descriptor_with(id: &str, body: KernelBody) -> KernelDescriptor {
        KernelDescriptor {
            id: id.into(),
            bindings: BindingLayout { slots: Vec::new() },
            dispatch: Dispatch::new(1, 1, 1),
            body,
        }
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

    pub(crate) fn op_with_result(desc: &KernelDescriptor, result: u32) -> &KernelOp {
        desc.body
            .ops
            .iter()
            .find(|op| op.result == Some(result))
            .expect("Fix: test descriptor must contain the requested result id")
    }

    pub(crate) fn copied_source(desc: &KernelDescriptor, result: u32) -> u32 {
        let op = op_with_result(desc, result);
        assert!(matches!(op.kind, KernelOpKind::Copy));
        op.operands[0]
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::{
        binop, copied_source, descriptor_with, empty_body, lit_u32, op_with_result,
    };
    use crate::{KernelDescriptor, KernelOpKind};
    use vyre_foundation::ir::BinOp;

    struct SelfBinopPassContract {
        descriptor_id: &'static str,
        rewrite: fn(&KernelDescriptor) -> KernelDescriptor,
        collapse_a: BinOp,
        collapse_b: BinOp,
        distinct_operands_op: BinOp,
        untouched_self_op: BinOp,
        child_op: BinOp,
    }

    fn assert_self_binop_pass_contract(contract: SelfBinopPassContract) {
        assert_self_case(&contract, contract.collapse_a, 0xCAFE, 0);
        assert_self_case(&contract, contract.collapse_b, 0xBABE, 0);
        assert_distinct_operands_stay_binary(&contract);
        assert_non_member_self_op_stays_binary(&contract);
        assert_idempotent(&contract);
        assert_recurses_into_child_bodies(&contract);
    }

    fn assert_self_case(contract: &SelfBinopPassContract, op: BinOp, value: u32, source: u32) {
        let mut body = empty_body();
        lit_u32(&mut body, value, source);
        binop(&mut body, op, source, source, source + 1);
        let desc = (contract.rewrite)(&descriptor_with(contract.descriptor_id, body));
        assert_eq!(copied_source(&desc, source + 1), source);
    }

    fn assert_distinct_operands_stay_binary(contract: &SelfBinopPassContract) {
        let mut body = empty_body();
        lit_u32(&mut body, 1, 0);
        lit_u32(&mut body, 2, 1);
        binop(&mut body, contract.distinct_operands_op, 0, 1, 2);
        let desc = (contract.rewrite)(&descriptor_with(contract.descriptor_id, body));
        let op = op_with_result(&desc, 2);
        assert!(matches!(
            op.kind,
            KernelOpKind::BinOpKind(kind) if kind == contract.distinct_operands_op
        ));
    }

    fn assert_non_member_self_op_stays_binary(contract: &SelfBinopPassContract) {
        let mut body = empty_body();
        lit_u32(&mut body, 7, 0);
        binop(&mut body, contract.untouched_self_op, 0, 0, 1);
        let desc = (contract.rewrite)(&descriptor_with(contract.descriptor_id, body));
        let op = op_with_result(&desc, 1);
        assert!(matches!(
            op.kind,
            KernelOpKind::BinOpKind(kind) if kind == contract.untouched_self_op
        ));
    }

    fn assert_idempotent(contract: &SelfBinopPassContract) {
        let mut body = empty_body();
        lit_u32(&mut body, 1, 0);
        binop(&mut body, contract.collapse_a, 0, 0, 1);
        let desc = descriptor_with(contract.descriptor_id, body);
        let once = (contract.rewrite)(&desc);
        let twice = (contract.rewrite)(&once);
        assert_eq!(once, twice);
    }

    fn assert_recurses_into_child_bodies(contract: &SelfBinopPassContract) {
        let mut child = empty_body();
        lit_u32(&mut child, 9, 10);
        binop(&mut child, contract.child_op, 10, 10, 11);
        let mut body = empty_body();
        body.child_bodies.push(child);
        let desc = (contract.rewrite)(&descriptor_with(contract.descriptor_id, body));
        let op = desc.body.child_bodies[0]
            .ops
            .iter()
            .find(|op| op.result == Some(11))
            .expect("Fix: child-body rewrite test must preserve result id 11");
        assert!(matches!(op.kind, KernelOpKind::Copy));
        assert_eq!(op.operands[0], 10);
    }

    #[test]
    fn bitwise_idemp_contract() {
        assert_self_binop_pass_contract(SelfBinopPassContract {
            descriptor_id: "bitwise_idemp_test",
            rewrite: crate::rewrites::bitwise_idemp::bitwise_idemp,
            collapse_a: BinOp::BitAnd,
            collapse_b: BinOp::BitOr,
            distinct_operands_op: BinOp::BitAnd,
            untouched_self_op: BinOp::BitXor,
            child_op: BinOp::BitOr,
        });
    }

    #[test]
    fn min_max_idemp_contract() {
        assert_self_binop_pass_contract(SelfBinopPassContract {
            descriptor_id: "min_max_idemp_test",
            rewrite: crate::rewrites::min_max_idemp::min_max_idemp,
            collapse_a: BinOp::Min,
            collapse_b: BinOp::Max,
            distinct_operands_op: BinOp::Min,
            untouched_self_op: BinOp::Add,
            child_op: BinOp::Max,
        });
    }
}
