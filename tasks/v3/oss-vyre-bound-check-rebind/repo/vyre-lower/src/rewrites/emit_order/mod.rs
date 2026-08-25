//! Emitter-order repair for descriptor SSA.
//!
//! The descriptor verifier permits references to values produced later in the
//! same body, but the Naga emitter is a single linear pass. This rewrite moves
//! only pure value producers before same-body consumers so emission can stay
//! linear without changing memory, atomic, carrier, or control-flow ordering.

use super::body_index::BodyIndex;
use crate::verify::{classify_operand, OperandClass};
use crate::{KernelBody, KernelDescriptor, KernelOp, KernelOpKind};

#[must_use]
pub fn emit_order(desc: &KernelDescriptor) -> KernelDescriptor {
    let mut out = desc.clone();
    schedule_body(&mut out.body);
    out
}

fn schedule_body(body: &mut KernelBody) {
    for child in &mut body.child_bodies {
        schedule_body(child);
    }
    let body_index = BodyIndex::new(body);
    let old_ops = std::mem::take(&mut body.ops);

    let mut emitted = vec![false; old_ops.len()];
    let mut new_ops = Vec::with_capacity(old_ops.len());
    for op_index in 0..old_ops.len() {
        emit_with_dependencies(op_index, &old_ops, &body_index, &mut emitted, &mut new_ops);
    }
    body.ops = new_ops;
}

fn emit_with_dependencies(
    index: usize,
    old_ops: &[KernelOp],
    body_index: &BodyIndex,
    emitted: &mut [bool],
    new_ops: &mut Vec<KernelOp>,
) {
    if emitted[index] {
        return;
    }
    let op = &old_ops[index];
    for (operand_pos, operand) in op.operands.iter().copied().enumerate() {
        if classify_operand(&op.kind, operand_pos) != OperandClass::ResultRef {
            continue;
        }
        let Some(producer_index) = body_index.producer_index(operand) else {
            continue;
        };
        if producer_index == index || emitted[producer_index] {
            continue;
        }
        if is_pure_movable(&old_ops[producer_index].kind) {
            emit_with_dependencies(producer_index, old_ops, body_index, emitted, new_ops);
        }
    }
    if !emitted[index] {
        emitted[index] = true;
        new_ops.push(op.clone());
    }
}

fn is_pure_movable(kind: &KernelOpKind) -> bool {
    matches!(
        kind,
        KernelOpKind::Literal
            | KernelOpKind::LocalInvocationId
            | KernelOpKind::GlobalInvocationId
            | KernelOpKind::WorkgroupId
            | KernelOpKind::SubgroupLocalId
            | KernelOpKind::SubgroupSize
            | KernelOpKind::LoopIndex { .. }
            | KernelOpKind::BufferLength
            | KernelOpKind::Copy
            | KernelOpKind::BinOpKind(_)
            | KernelOpKind::UnOpKind(_)
            | KernelOpKind::Fma
            | KernelOpKind::Select
            | KernelOpKind::Cast { .. }
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BindingLayout, Dispatch, KernelDescriptor, LiteralValue};
    use vyre_foundation::ir::BinOp;

    fn desc(ops: Vec<KernelOp>) -> KernelDescriptor {
        KernelDescriptor {
            id: "emit-order".into(),
            bindings: BindingLayout { slots: Vec::new() },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops,
                child_bodies: Vec::new(),
                literals: vec![LiteralValue::U32(1), LiteralValue::U32(2)],
            },
        }
    }

    #[test]
    fn hoists_pure_forward_dependency_before_consumer() {
        let out = emit_order(&desc(vec![
            KernelOp {
                kind: KernelOpKind::Literal,
                operands: vec![0],
                result: Some(0),
            },
            KernelOp {
                kind: KernelOpKind::BinOpKind(BinOp::Add),
                operands: vec![0, 2],
                result: Some(3),
            },
            KernelOp {
                kind: KernelOpKind::Literal,
                operands: vec![1],
                result: Some(2),
            },
        ]));

        assert_eq!(out.body.ops[1].result, Some(2));
        assert_eq!(out.body.ops[2].result, Some(3));
    }

    #[test]
    fn does_not_move_loads_or_side_effects() {
        let out = emit_order(&desc(vec![
            KernelOp {
                kind: KernelOpKind::BinOpKind(BinOp::Add),
                operands: vec![1, 2],
                result: Some(3),
            },
            KernelOp {
                kind: KernelOpKind::LoadGlobal,
                operands: vec![0, 1],
                result: Some(2),
            },
        ]));

        assert!(matches!(
            out.body.ops[0].kind,
            KernelOpKind::BinOpKind(BinOp::Add)
        ));
        assert!(matches!(out.body.ops[1].kind, KernelOpKind::LoadGlobal));
    }
}
