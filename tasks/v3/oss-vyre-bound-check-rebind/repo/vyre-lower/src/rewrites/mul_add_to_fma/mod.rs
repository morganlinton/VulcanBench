//! Mul-Add → FMA promotion rewrite.
//!
//! Pattern: `Add(Mul(a, b), c)` (or `Add(c, Mul(a, b))`) → `Fma(a, b, c)`.
//!
//! Only applied when:
//! 1. The `Mul` op's result is used by exactly one consumer (this `Add`).
//!    Otherwise removing the `Mul` would break other uses.
//! 2. We can prove float-producing operands by tracing each back to a
//!    `Literal::F32` or a `Cast { target: F32 }` ancestor. The descriptor
//!    model is intentionally untyped at the rewrite layer, so the only
//!    safe way to introduce `Fma` (which all backends emit as the FP
//!    intrinsic) is via positive proof. When the proof is unavailable the
//!    rewrite no-ops  -  never wrong, possibly conservative.
//!
//! After the rewrite the original `Mul` op is left in place but becomes
//! unreferenced; `descriptor_dce` (which runs later in the canonical
//! pipeline) cleans it up.
//!
//! Recurses into nested control flow (`StructuredIfThen/Else/ForLoop/
//! Block/Region`). Idempotent: a second invocation finds no further
//! Add(Mul(...), ...) shapes because the Add was already replaced.

use super::body_index::BodyIndex;
use crate::{KernelBody, KernelDescriptor, KernelOpKind, LiteralValue};
use vyre_foundation::ir::BinOp;
use vyre_foundation::ir::DataType;

/// Apply Mul-Add → FMA promotion across the descriptor body, recursing
/// into all nested bodies.
#[must_use]
pub fn mul_add_to_fma(desc: &KernelDescriptor) -> KernelDescriptor {
    let mut out = desc.clone();
    out.body = mul_add_to_fma_body(out.body);
    out
}

fn mul_add_to_fma_body(mut body: KernelBody) -> KernelBody {
    let index = BodyIndex::new(&body);

    // Two-pass: decide promotions, then apply.
    let mut promotions: Vec<(usize, u32, u32, u32)> = Vec::new(); // (add_idx, a_id, b_id, c_id)
    for (idx, op) in body.ops.iter().enumerate() {
        let bin = match &op.kind {
            KernelOpKind::BinOpKind(BinOp::Add) => BinOp::Add,
            _ => continue,
        };
        if op.operands.len() != 2 {
            continue;
        }
        let _ = bin;
        let lhs = op.operands[0];
        let rhs = op.operands[1];

        // Try lhs as the Mul side first, then rhs. The first hit wins;
        // both can't fire on the same Add anyway.
        let mul_side = candidate_mul(&body, &index, lhs)
            .map(|(a, b)| (a, b, rhs))
            .or_else(|| candidate_mul(&body, &index, rhs).map(|(a, b)| (a, b, lhs)));
        let Some((a_id, b_id, c_id)) = mul_side else {
            continue;
        };
        // Only promote when ALL three operand chains are provably float-
        // producing. The descriptor model is untyped so without proof
        // we can't safely emit Fma (which is FP-only at every backend).
        if !is_float_producing(&body, &index, a_id, 0)
            || !is_float_producing(&body, &index, b_id, 0)
            || !is_float_producing(&body, &index, c_id, 0)
        {
            continue;
        }
        promotions.push((idx, a_id, b_id, c_id));
    }

    for (add_idx, a_id, b_id, c_id) in promotions {
        body.ops[add_idx].kind = KernelOpKind::Fma;
        body.ops[add_idx].operands = vec![a_id, b_id, c_id];
    }

    body.child_bodies = body
        .child_bodies
        .into_iter()
        .map(mul_add_to_fma_body)
        .collect();
    body
}

/// If `result_id` was produced by a `Mul` with exactly one use, return
/// `(mul_a, mul_b)`. Otherwise return `None`.
fn candidate_mul(body: &KernelBody, index: &BodyIndex, result_id: u32) -> Option<(u32, u32)> {
    let producer = index.producer(body, result_id)?;
    if !matches!(producer.kind, KernelOpKind::BinOpKind(BinOp::Mul)) {
        return None;
    }
    if producer.operands.len() != 2 {
        return None;
    }
    if !index.has_single_consumer(result_id) {
        return None;
    }
    Some((producer.operands[0], producer.operands[1]))
}

/// Recursive trace: is `result_id` provably produced by float ops?
const FLOAT_TRACE_DEPTH_LIMIT: usize = 8;

fn is_float_producing(body: &KernelBody, index: &BodyIndex, result_id: u32, depth: usize) -> bool {
    if depth >= FLOAT_TRACE_DEPTH_LIMIT {
        return false;
    }
    let Some(op) = index.producer(body, result_id) else {
        return false;
    };
    match &op.kind {
        KernelOpKind::Literal => {
            let pool_idx = match op.operands.first() {
                Some(p) => *p as usize,
                None => return false,
            };
            matches!(body.literals.get(pool_idx), Some(LiteralValue::F32(_)))
        }
        KernelOpKind::Cast { target } => matches!(
            target,
            DataType::F32 | DataType::F16 | DataType::BF16 | DataType::F64
        ),
        KernelOpKind::Fma => true,
        KernelOpKind::BinOpKind(_) => {
            // Float in → float out (when we've proved at least one float
            // operand). Recurse on the first operand; the BinOp inherits
            // its dtype from its inputs.
            op.operands
                .iter()
                .any(|operand| is_float_producing(body, index, *operand, depth + 1))
        }
        KernelOpKind::UnOpKind(_) | KernelOpKind::Select => op
            .operands
            .iter()
            .any(|operand| is_float_producing(body, index, *operand, depth + 1)),
        _ => false,
    }
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

    fn lit_f32(body: &mut KernelBody, value: f32, result: u32) {
        let pool_idx = body.literals.len() as u32;
        body.literals.push(LiteralValue::F32(value));
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

    fn mul(body: &mut KernelBody, a: u32, b: u32, result: u32) {
        body.ops.push(KernelOp {
            kind: KernelOpKind::BinOpKind(BinOp::Mul),
            operands: vec![a, b],
            result: Some(result),
        });
    }

    fn add(body: &mut KernelBody, a: u32, b: u32, result: u32) {
        body.ops.push(KernelOp {
            kind: KernelOpKind::BinOpKind(BinOp::Add),
            operands: vec![a, b],
            result: Some(result),
        });
    }

    fn descriptor_with(body: KernelBody) -> KernelDescriptor {
        KernelDescriptor {
            id: "fma_test_kernel".into(),
            bindings: BindingLayout { slots: Vec::new() },
            dispatch: Dispatch::new(1, 1, 1),
            body,
        }
    }

    #[test]
    fn fma_replaces_add_of_mul_lhs_when_floats() {
        let mut body = empty_body();
        lit_f32(&mut body, 1.0, 0);
        lit_f32(&mut body, 2.0, 1);
        lit_f32(&mut body, 3.0, 2);
        mul(&mut body, 0, 1, 3); // result-3 = 1.0 * 2.0
        add(&mut body, 3, 2, 4); // result-4 = result-3 + 3.0
        let desc = descriptor_with(body);
        let out = mul_add_to_fma(&desc);
        let add_op = out
            .body
            .ops
            .iter()
            .find(|op| op.result == Some(4))
            .expect("Fix: result-4 op must survive the rewrite (now as Fma).");
        assert!(
            matches!(add_op.kind, KernelOpKind::Fma),
            "Fix: Add(Mul(a,b), c) over floats must become Fma(a, b, c)  -  got {:?}",
            add_op.kind
        );
        assert_eq!(
            add_op.operands,
            vec![0, 1, 2],
            "Fix: Fma operands must be [a_id, b_id, c_id] = [0, 1, 2]."
        );
    }

    #[test]
    fn fma_replaces_add_of_mul_rhs_when_floats() {
        let mut body = empty_body();
        lit_f32(&mut body, 1.0, 0);
        lit_f32(&mut body, 2.0, 1);
        lit_f32(&mut body, 3.0, 2);
        mul(&mut body, 0, 1, 3);
        add(&mut body, 2, 3, 4); // result-4 = 3.0 + (1.0 * 2.0)
        let desc = descriptor_with(body);
        let out = mul_add_to_fma(&desc);
        let add_op = out
            .body
            .ops
            .iter()
            .find(|op| op.result == Some(4))
            .expect("Fix: result-4 op must survive the rewrite.");
        assert!(matches!(add_op.kind, KernelOpKind::Fma));
        assert_eq!(add_op.operands, vec![0, 1, 2]);
    }

    #[test]
    fn no_fma_when_mul_has_multiple_consumers() {
        // Mul result feeds two Adds  -  promoting one would still leave
        // the other depending on the Mul, so neither can fire.
        let mut body = empty_body();
        lit_f32(&mut body, 1.0, 0);
        lit_f32(&mut body, 2.0, 1);
        lit_f32(&mut body, 3.0, 2);
        mul(&mut body, 0, 1, 3);
        add(&mut body, 3, 2, 4);
        add(&mut body, 3, 0, 5); // second consumer of result-3
        let desc = descriptor_with(body);
        let out = mul_add_to_fma(&desc);
        for op in &out.body.ops {
            assert!(
                !matches!(op.kind, KernelOpKind::Fma),
                "Fix: must not promote when the Mul has more than one consumer."
            );
        }
    }

    #[test]
    fn no_fma_when_operands_are_integer_literals() {
        // Pure integer pattern  -  Fma is FP at every backend, so refuse
        // without positive float proof.
        let mut body = empty_body();
        lit_u32(&mut body, 1, 0);
        lit_u32(&mut body, 2, 1);
        lit_u32(&mut body, 3, 2);
        mul(&mut body, 0, 1, 3);
        add(&mut body, 3, 2, 4);
        let desc = descriptor_with(body);
        let out = mul_add_to_fma(&desc);
        for op in &out.body.ops {
            assert!(
                !matches!(op.kind, KernelOpKind::Fma),
                "Fix: integer Add(Mul(...), ...) must not promote  -  Fma is FP-only at every backend."
            );
        }
    }

    #[test]
    fn rewrite_is_idempotent() {
        let mut body = empty_body();
        lit_f32(&mut body, 1.0, 0);
        lit_f32(&mut body, 2.0, 1);
        lit_f32(&mut body, 3.0, 2);
        mul(&mut body, 0, 1, 3);
        add(&mut body, 3, 2, 4);
        let desc = descriptor_with(body);
        let once = mul_add_to_fma(&desc);
        let twice = mul_add_to_fma(&once);
        assert_eq!(
            once, twice,
            "Fix: rewrite must reach a fixpoint after one application."
        );
    }

    #[test]
    fn recurses_into_child_bodies() {
        let mut child = empty_body();
        lit_f32(&mut child, 1.0, 10);
        lit_f32(&mut child, 2.0, 11);
        lit_f32(&mut child, 3.0, 12);
        mul(&mut child, 10, 11, 13);
        add(&mut child, 13, 12, 14);

        let mut body = empty_body();
        body.child_bodies = vec![child];
        let desc = descriptor_with(body);
        let out = mul_add_to_fma(&desc);
        let child_out = &out.body.child_bodies[0];
        let promoted = child_out
            .ops
            .iter()
            .find(|op| op.result == Some(14))
            .expect("Fix: child-body promotion target must survive the rewrite.");
        assert!(
            matches!(promoted.kind, KernelOpKind::Fma),
            "Fix: rewrite must recurse into child bodies. Got {:?}.",
            promoted.kind
        );
    }
}
