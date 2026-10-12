//! `Mod` idempotence  -  fold `Mod(Mod(x, Lit(a)), Lit(a))` to
//! `Mod(x, Lit(a))`. Identity:
//!     ((x mod a) mod a) = (x mod a) for any a > 0.
//! Also folds `Mod(Mod(x, Lit(a)), Lit(b))` to `Mod(x, Lit(a))` when
//! b >= a > 0 (the inner already produced a value < a ≤ b, so the
//! outer Mod is the identity on it).
//!
//! Source-of-truth: `PERF_ROADMAP_2026-05-01.md` section A.4
//! (algebraic simplification family). Companion to `div_combine`.
//!
//! Patterns rewritten (when both Lits are U32, both > 0, the inner
//! Mod has exactly one consumer, and b >= a):
//! - `Mod(Mod(x, Lit(a)), Lit(b))` → `Mod(x, Lit(a))`
//!
//! Out-of-scope: zero divisors (preserved unchanged so the runtime
//! mod-by-zero trap stays observable), b < a (would require knowing
//! whether b divides a  -  handled instead by `egraph_saturation`),
//! signed mod (I32 left to a future pass), and `Mod(Lit, Mod(...))`.
//!
//! Recurses. Idempotent. Wired immediately after `div_combine` in
//! `CANONICAL_REWRITE_PASSES`.

use super::body_index::BodyIndex;
use crate::{KernelBody, KernelDescriptor, KernelOpKind};
use vyre_foundation::ir::BinOp;

#[must_use]
pub fn mod_idemp(desc: &KernelDescriptor) -> KernelDescriptor {
    let mut out = desc.clone();
    out.body = mod_idemp_body(out.body);
    out
}

fn mod_idemp_body(mut body: KernelBody) -> KernelBody {
    let index = BodyIndex::new(&body);

    // (op_idx_to_replace, replacement_result_id)
    let mut rewrites: Vec<(usize, u32)> = Vec::new();
    for (idx, op) in body.ops.iter().enumerate() {
        if !matches!(op.kind, KernelOpKind::BinOpKind(BinOp::Mod)) {
            continue;
        }
        if op.operands.len() != 2 {
            continue;
        }
        let lhs = op.operands[0];
        let rhs = op.operands[1];

        let Some(b) = index.u32_lit(&body, rhs) else {
            continue;
        };
        if b == 0 {
            continue;
        }

        let Some((inner_result, _x, a)) = inner_mod_with_rhs_lit(&body, &index, lhs) else {
            continue;
        };
        if a == 0 {
            continue;
        }

        if b >= a {
            // The outer Mod is the identity on the inner  -  replace the
            // outer's users with the inner's result. We do this by
            // turning the outer into a no-op alias: every user of
            // op.result will be patched to use inner_result.
            rewrites.push((idx, inner_result));
        }
    }

    // Apply by patching every operand reference from the outer's result
    // id over to the inner's result id. The outer op then becomes dead
    // and `descriptor_dce` reaps it on the next pass.
    for (op_idx, replacement) in &rewrites {
        let Some(outer_result) = body.ops[*op_idx].result else {
            continue;
        };
        for op in body.ops.iter_mut() {
            for operand in op.operands.iter_mut() {
                if *operand == outer_result {
                    *operand = *replacement;
                }
            }
        }
    }

    body.child_bodies = body.child_bodies.into_iter().map(mod_idemp_body).collect();
    body
}

fn inner_mod_with_rhs_lit(
    body: &KernelBody,
    index: &BodyIndex,
    result_id: u32,
) -> Option<(u32, u32, u32)> {
    let producer = index.producer(body, result_id)?;
    if !matches!(producer.kind, KernelOpKind::BinOpKind(BinOp::Mod)) {
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
    Some((result_id, lhs, c))
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
            id: "mod_idemp_test".into(),
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

    #[test]
    fn double_mod_with_equal_divisor_collapses() {
        // ((x mod 7) mod 7) → uses the inner result directly.
        let mut body = empty_body();
        nonliteral_source(&mut body, 0);
        lit_u32(&mut body, 7, 1);
        binop(&mut body, BinOp::Mod, 0, 1, 2); // inner = x mod 7 → result 2
        lit_u32(&mut body, 7, 3);
        binop(&mut body, BinOp::Mod, 2, 3, 4); // outer = inner mod 7 → result 4
        store(&mut body, 4);
        let desc = mod_idemp(&descriptor_with(body));
        // The store's value-operand should now be 2, not 4.
        let store_op = desc
            .body
            .ops
            .iter()
            .find(|o| matches!(o.kind, KernelOpKind::StoreGlobal))
            .unwrap();
        assert_eq!(store_op.operands[2], 2, "Fix: outer Mod must be aliased");
    }

    #[test]
    fn outer_divisor_larger_collapses() {
        // ((x mod 5) mod 100)  -  inner < 5 < 100, outer is identity.
        let mut body = empty_body();
        nonliteral_source(&mut body, 0);
        lit_u32(&mut body, 5, 1);
        binop(&mut body, BinOp::Mod, 0, 1, 2);
        lit_u32(&mut body, 100, 3);
        binop(&mut body, BinOp::Mod, 2, 3, 4);
        store(&mut body, 4);
        let desc = mod_idemp(&descriptor_with(body));
        let store_op = desc
            .body
            .ops
            .iter()
            .find(|o| matches!(o.kind, KernelOpKind::StoreGlobal))
            .unwrap();
        assert_eq!(store_op.operands[2], 2);
    }

    #[test]
    fn outer_divisor_smaller_left_alone() {
        // ((x mod 100) mod 5)  -  outer is meaningful (different residue).
        let mut body = empty_body();
        nonliteral_source(&mut body, 0);
        lit_u32(&mut body, 100, 1);
        binop(&mut body, BinOp::Mod, 0, 1, 2);
        lit_u32(&mut body, 5, 3);
        binop(&mut body, BinOp::Mod, 2, 3, 4);
        store(&mut body, 4);
        let desc = mod_idemp(&descriptor_with(body));
        let store_op = desc
            .body
            .ops
            .iter()
            .find(|o| matches!(o.kind, KernelOpKind::StoreGlobal))
            .unwrap();
        assert_eq!(
            store_op.operands[2], 4,
            "Fix: smaller-outer-mod has different residue, must NOT collapse."
        );
    }

    #[test]
    fn zero_inner_divisor_left_alone() {
        let mut body = empty_body();
        nonliteral_source(&mut body, 0);
        lit_u32(&mut body, 0, 1);
        binop(&mut body, BinOp::Mod, 0, 1, 2);
        lit_u32(&mut body, 5, 3);
        binop(&mut body, BinOp::Mod, 2, 3, 4);
        store(&mut body, 4);
        let desc = mod_idemp(&descriptor_with(body));
        let store_op = desc
            .body
            .ops
            .iter()
            .find(|o| matches!(o.kind, KernelOpKind::StoreGlobal))
            .unwrap();
        assert_eq!(store_op.operands[2], 4);
    }

    #[test]
    fn zero_outer_divisor_left_alone() {
        let mut body = empty_body();
        nonliteral_source(&mut body, 0);
        lit_u32(&mut body, 5, 1);
        binop(&mut body, BinOp::Mod, 0, 1, 2);
        lit_u32(&mut body, 0, 3);
        binop(&mut body, BinOp::Mod, 2, 3, 4);
        store(&mut body, 4);
        let desc = mod_idemp(&descriptor_with(body));
        let store_op = desc
            .body
            .ops
            .iter()
            .find(|o| matches!(o.kind, KernelOpKind::StoreGlobal))
            .unwrap();
        assert_eq!(store_op.operands[2], 4);
    }

    #[test]
    fn inner_with_multiple_consumers_left_alone() {
        let mut body = empty_body();
        nonliteral_source(&mut body, 0);
        lit_u32(&mut body, 7, 1);
        binop(&mut body, BinOp::Mod, 0, 1, 2);
        lit_u32(&mut body, 7, 3);
        binop(&mut body, BinOp::Mod, 2, 3, 4);
        binop(&mut body, BinOp::Add, 2, 0, 5);
        store(&mut body, 4);
        let desc = mod_idemp(&descriptor_with(body));
        let store_op = desc
            .body
            .ops
            .iter()
            .find(|o| matches!(o.kind, KernelOpKind::StoreGlobal))
            .unwrap();
        assert_eq!(store_op.operands[2], 4);
    }

    #[test]
    fn rewrite_is_idempotent() {
        let mut body = empty_body();
        nonliteral_source(&mut body, 0);
        lit_u32(&mut body, 7, 1);
        binop(&mut body, BinOp::Mod, 0, 1, 2);
        lit_u32(&mut body, 7, 3);
        binop(&mut body, BinOp::Mod, 2, 3, 4);
        store(&mut body, 4);
        let desc = descriptor_with(body);
        let once = mod_idemp(&desc);
        let twice = mod_idemp(&once);
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
        lit_u32(&mut child, 7, 11);
        binop(&mut child, BinOp::Mod, 10, 11, 12);
        lit_u32(&mut child, 7, 13);
        binop(&mut child, BinOp::Mod, 12, 13, 14);
        store(&mut child, 14);
        let mut body = empty_body();
        body.child_bodies.push(child);
        let desc = mod_idemp(&descriptor_with(body));
        let store_op = desc.body.child_bodies[0]
            .ops
            .iter()
            .find(|o| matches!(o.kind, KernelOpKind::StoreGlobal))
            .unwrap();
        assert_eq!(store_op.operands[2], 12);
    }
}
