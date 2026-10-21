//! Identity-element elimination rewrite.
//!
//! Detects `BinOp` ops where one operand is a Literal whose value is the
//! algebraic identity (or absorbing element) for that op, and rewrites
//! all references to the BinOp's result-id to point at the surviving
//! operand instead. The eliminated BinOp becomes dead  -  `descriptor_dce` cleans it
//! up; the literal stays put (CSE will dedupe duplicates if any).
//!
//! ## Patterns
//!
//! Right-identity (kept = lhs):
//! - `Add(x, 0)` → `x`
//! - `Sub(x, 0)` → `x`
//! - `Mul(x, 1)` → `x`
//! - `Div(x, 1)` → `x`
//! - `BitOr(x, 0)` → `x`
//! - `BitXor(x, 0)` → `x`
//! - `Shl(x, 0)` → `x`
//! - `Shr(x, 0)` → `x`
//! - `WrappingAdd(x, 0)` → `x`
//! - `WrappingSub(x, 0)` → `x`
//!
//! Left-identity (kept = rhs):
//! - `Add(0, x)` → `x`
//! - `Mul(1, x)` → `x`
//! - `BitOr(0, x)` → `x`
//! - `BitXor(0, x)` → `x`
//! - `WrappingAdd(0, x)` → `x`
//! - (Sub, Div, Shl, Shr are not commutative  -  left-identity does NOT
//!   apply.)
//!
//! Absorbing-zero (kept = the literal-zero side):
//! - `Mul(x, 0)` → `0`, `Mul(0, x)` → `0`
//! - `BitAnd(x, 0)` → `0`, `BitAnd(0, x)` → `0`
//!
//! Self-equality (kept = either operand, both are the same):
//! - `BitAnd(x, x)` → `x`
//! - `BitOr(x, x)` → `x`
//! - `Min(x, x)` → `x`
//! - `Max(x, x)` → `x`
//!
//! ## Pipeline interaction
//!
//! `identity_elim` only rewrites references; it does not strip ops.
//! The freshly-orphaned BinOp gets removed by the next `descriptor_dce` run. The
//! `run_all` ordering puts `identity_elim` before `descriptor_dce`/`descriptor_cse`. Because
//! `descriptor_const_fold` already collapses `BinOp(Literal, Literal)` into a single
//! `Literal`, identity_elim only sees `BinOp(var, Literal)` and
//! `BinOp(var, var)` shapes  -  the literal-vs-literal case is already
//! handled.
//!
//! ## Non-patterns
//!
//! `Sub(x, x)` and `BitXor(x, x)` are deliberately not rewritten here:
//! this pass performs pure result-id substitution and does not synthesize
//! typed literals. For floating-point `Sub`, NaN semantics also make a
//! blanket `x - x -> 0` rewrite incorrect. Rewrites that create new typed
//! ops belong in typed canonicalization/e-graph passes, not in this
//! substitution-only pass.

use vyre_foundation::ir::BinOp;
use vyre_foundation::optimizer::algebraic_rules::{
    binop_identity_replacement, IdentityReplacement, ScalarLiteral,
};

use crate::operand_semantics::operand_is_result_reference;
use crate::{KernelBody, KernelDescriptor, KernelOpKind, LiteralValue};
use rustc_hash::FxHashMap;

#[must_use]
pub fn identity_elim(desc: &KernelDescriptor) -> KernelDescriptor {
    let mut out = desc.clone();
    out.body = identity_elim_body(out.body);
    out
}

fn identity_elim_body(mut body: KernelBody) -> KernelBody {
    // Step 1: build literal lookup  -  result_id → LiteralValue (only for
    // ops that are themselves Literal kind).
    let mut lit_value: FxHashMap<u32, LiteralValue> = FxHashMap::default();
    for op in &body.ops {
        if matches!(op.kind, KernelOpKind::Literal) {
            if let Some(rid) = op.result {
                if let Some(&pool_idx) = op.operands.first() {
                    if let Some(lv) = body.literals.get(pool_idx as usize) {
                        lit_value.insert(rid, lv.clone());
                    }
                }
            }
        }
    }

    // Step 2: walk in order, building id_remap.
    let mut id_remap: FxHashMap<u32, u32> = FxHashMap::default();
    for op in &body.ops {
        match &op.kind {
            KernelOpKind::BinOpKind(bin_op) => {
                if op.operands.len() < 2 {
                    continue;
                }
                let lhs_raw = op.operands[0];
                let rhs_raw = op.operands[1];
                // Apply the existing remap to the operand ids  -  earlier
                // identity-elim may have substituted them already
                // conceptually (we do the rewrite in step 3, but the
                // kept side must reflect whatever it would resolve to).
                let lhs = resolve(lhs_raw, &id_remap);
                let rhs = resolve(rhs_raw, &id_remap);
                let Some(rid) = op.result else { continue };
                if let Some(kept_id) = decide_kept(*bin_op, lhs, rhs, &lit_value) {
                    id_remap.insert(rid, kept_id);
                }
            }
            KernelOpKind::Select => {
                // Select(cond, true_val, false_val). When cond is a
                // Bool literal, substitute the result with the picked
                // branch's id. Pure id-substitution; the Select op
                // becomes dead and DCE drops it.
                if op.operands.len() < 3 {
                    continue;
                }
                let cond_raw = op.operands[0];
                let true_raw = op.operands[1];
                let false_raw = op.operands[2];
                let cond_resolved = resolve(cond_raw, &id_remap);
                let Some(rid) = op.result else { continue };
                let kept = match lit_value.get(&cond_resolved) {
                    Some(LiteralValue::Bool(true)) => Some(resolve(true_raw, &id_remap)),
                    Some(LiteralValue::Bool(false)) => Some(resolve(false_raw, &id_remap)),
                    _ => None,
                };
                if let Some(kept_id) = kept {
                    id_remap.insert(rid, kept_id);
                }
            }
            KernelOpKind::Fma => {
                // Fma(a, b, c) = a*b + c. When either factor is
                // Literal(0), the result equals c  -  substitute the
                // Fma's result-id with c's id. (Lit(1) cases would
                // simplify to Add(other_factor, c) but require
                // synthesizing a new op; that's outside identity_elim's
                // pure id-substitution model.)
                if op.operands.len() < 3 {
                    continue;
                }
                let a_raw = op.operands[0];
                let b_raw = op.operands[1];
                let c_raw = op.operands[2];
                let a = resolve(a_raw, &id_remap);
                let b = resolve(b_raw, &id_remap);
                let c = resolve(c_raw, &id_remap);
                let Some(rid) = op.result else { continue };
                let a_zero = lit_value
                    .get(&a)
                    .map(|value| scalar_literal(value).is_numeric_zero())
                    .unwrap_or(false);
                let b_zero = lit_value
                    .get(&b)
                    .map(|value| scalar_literal(value).is_numeric_zero())
                    .unwrap_or(false);
                if a_zero || b_zero {
                    id_remap.insert(rid, c);
                }
            }
            _ => continue,
        }
    }

    if id_remap.is_empty() {
        // Recurse into children for completeness; nothing to do here.
        body.child_bodies = body
            .child_bodies
            .into_iter()
            .map(identity_elim_body)
            .collect();
        return body;
    }

    // Step 3: rewrite operand references through id_remap. We do NOT
    // strip the eliminated BinOp ops here  -  DCE handles that. Keeping
    // them in place avoids any re-numbering of result-ids in this pass,
    // so we don't have to touch every operand list.
    for op in &mut body.ops {
        for pos in 0..op.operands.len() {
            if operand_is_result_reference(&op.kind, pos) {
                op.operands[pos] = resolve(op.operands[pos], &id_remap);
            }
        }
    }

    body.child_bodies = body
        .child_bodies
        .into_iter()
        .map(identity_elim_body)
        .collect();
    body
}

fn resolve(id: u32, remap: &FxHashMap<u32, u32>) -> u32 {
    let mut cur = id;
    // Path-compression-free transitive resolution. Bounded by the number
    // of remap entries  -  chains can't loop (each remap maps a higher
    // result-id forward to one already produced).
    let mut hops = 0usize;
    while let Some(&nxt) = remap.get(&cur) {
        if nxt == cur {
            break;
        }
        cur = nxt;
        hops += 1;
        if hops > remap.len() + 1 {
            break;
        }
    }
    cur
}

/// Decide what the BinOp's result should resolve to, if anything.
/// Returns `Some(kept_operand_id)` if the BinOp is an identity case;
/// `None` if no rewrite applies.
fn decide_kept(
    op: BinOp,
    lhs_id: u32,
    rhs_id: u32,
    lit_value: &FxHashMap<u32, LiteralValue>,
) -> Option<u32> {
    let lhs_lit = lit_value.get(&lhs_id);
    let rhs_lit = lit_value.get(&rhs_id);
    match binop_identity_replacement(
        op,
        lhs_id == rhs_id,
        lhs_lit.map(scalar_literal),
        rhs_lit.map(scalar_literal),
    ) {
        Some(IdentityReplacement::Left) => Some(lhs_id),
        Some(IdentityReplacement::Right) => Some(rhs_id),
        None => None,
    }
}

fn scalar_literal(v: &LiteralValue) -> ScalarLiteral {
    match v {
        LiteralValue::U32(value) => ScalarLiteral::U32(*value),
        LiteralValue::I32(value) => ScalarLiteral::I32(*value),
        LiteralValue::F32(value) => ScalarLiteral::F32(*value),
        LiteralValue::Bool(value) => ScalarLiteral::Bool(*value),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BindingLayout, Dispatch, KernelBody, KernelDescriptor, KernelOp, KernelOpKind, LiteralValue,
    };
    use vyre_foundation::ir::BinOp;

    fn empty_desc(ops: Vec<KernelOp>, literals: Vec<LiteralValue>) -> KernelDescriptor {
        KernelDescriptor {
            id: "k".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops,
                child_bodies: vec![],
                literals,
            },
        }
    }

    #[test]
    fn add_x_zero_eliminates_to_x() {
        // r0 = Literal(varying), r1 = Literal(0), r2 = Add(r0, r1), Store(_, _, r2)
        let desc = empty_desc(
            vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![1],
                    result: Some(1),
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Add),
                    operands: vec![0, 1],
                    result: Some(2),
                },
                KernelOp {
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![0, 0, 2],
                    result: None,
                },
            ],
            vec![LiteralValue::U32(7), LiteralValue::U32(0)],
        );
        let out = identity_elim(&desc);
        // The Store op's value operand (pos=2) should now reference r0
        // (the kept side), not r2.
        assert_eq!(out.body.ops[3].kind, KernelOpKind::StoreGlobal);
        assert_eq!(out.body.ops[3].operands, vec![0, 0, 0]);
    }

    #[test]
    fn add_zero_x_eliminates_to_x() {
        let desc = empty_desc(
            vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                }, // 0
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![1],
                    result: Some(1),
                }, // varying
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Add),
                    operands: vec![0, 1],
                    result: Some(2),
                },
                KernelOp {
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![0, 0, 2],
                    result: None,
                },
            ],
            vec![LiteralValue::U32(0), LiteralValue::U32(7)],
        );
        let out = identity_elim(&desc);
        assert_eq!(out.body.ops[3].operands, vec![0, 0, 1]);
    }

    #[test]
    fn mul_x_one_eliminates_to_x() {
        let desc = empty_desc(
            vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                }, // varying
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![1],
                    result: Some(1),
                }, // 1
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Mul),
                    operands: vec![0, 1],
                    result: Some(2),
                },
                KernelOp {
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![0, 0, 2],
                    result: None,
                },
            ],
            vec![LiteralValue::U32(42), LiteralValue::U32(1)],
        );
        let out = identity_elim(&desc);
        assert_eq!(out.body.ops[3].operands, vec![0, 0, 0]);
    }

    #[test]
    fn mul_x_zero_absorbs_to_zero() {
        let desc = empty_desc(
            vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                }, // varying
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![1],
                    result: Some(1),
                }, // 0
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Mul),
                    operands: vec![0, 1],
                    result: Some(2),
                },
                KernelOp {
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![0, 0, 2],
                    result: None,
                },
            ],
            vec![LiteralValue::U32(99), LiteralValue::U32(0)],
        );
        let out = identity_elim(&desc);
        // Store's value should reference r1 (the literal 0), not r2.
        assert_eq!(out.body.ops[3].operands, vec![0, 0, 1]);
    }

    #[test]
    fn bitand_x_zero_absorbs_to_zero() {
        let desc = empty_desc(
            vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![1],
                    result: Some(1),
                }, // 0
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::BitAnd),
                    operands: vec![0, 1],
                    result: Some(2),
                },
                KernelOp {
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![0, 0, 2],
                    result: None,
                },
            ],
            vec![LiteralValue::U32(0xFF), LiteralValue::U32(0)],
        );
        let out = identity_elim(&desc);
        assert_eq!(out.body.ops[3].operands, vec![0, 0, 1]);
    }

    #[test]
    fn shl_x_zero_eliminates_to_x() {
        let desc = empty_desc(
            vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![1],
                    result: Some(1),
                }, // 0
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Shl),
                    operands: vec![0, 1],
                    result: Some(2),
                },
                KernelOp {
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![0, 0, 2],
                    result: None,
                },
            ],
            vec![LiteralValue::U32(5), LiteralValue::U32(0)],
        );
        let out = identity_elim(&desc);
        assert_eq!(out.body.ops[3].operands, vec![0, 0, 0]);
    }

    #[test]
    fn sub_zero_x_does_not_eliminate() {
        // Sub is not commutative  -  `0 - x` is negation, not identity.
        let desc = empty_desc(
            vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                }, // 0
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![1],
                    result: Some(1),
                }, // varying
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Sub),
                    operands: vec![0, 1],
                    result: Some(2),
                },
                KernelOp {
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![0, 0, 2],
                    result: None,
                },
            ],
            vec![LiteralValue::U32(0), LiteralValue::U32(5)],
        );
        let out = identity_elim(&desc);
        // Store should still reference r2  -  Sub(0, x) is not eliminable.
        assert_eq!(out.body.ops[3].operands, vec![0, 0, 2]);
    }

    #[test]
    fn div_one_x_does_not_eliminate() {
        // Div is not commutative  -  `1 / x` is reciprocal, not identity.
        let desc = empty_desc(
            vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                }, // 1
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![1],
                    result: Some(1),
                }, // varying
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Div),
                    operands: vec![0, 1],
                    result: Some(2),
                },
                KernelOp {
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![0, 0, 2],
                    result: None,
                },
            ],
            vec![LiteralValue::U32(1), LiteralValue::U32(5)],
        );
        let out = identity_elim(&desc);
        assert_eq!(out.body.ops[3].operands, vec![0, 0, 2]);
    }

    #[test]
    fn bitand_x_x_eliminates_to_x() {
        // r0 = Lit(varying); r1 = BitAnd(r0, r0); Store uses r1.
        let desc = empty_desc(
            vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::BitAnd),
                    operands: vec![0, 0],
                    result: Some(1),
                },
                KernelOp {
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![0, 0, 1],
                    result: None,
                },
            ],
            vec![LiteralValue::U32(0xCAFE)],
        );
        let out = identity_elim(&desc);
        assert_eq!(out.body.ops[2].operands, vec![0, 0, 0]);
    }

    #[test]
    fn min_x_x_eliminates_to_x() {
        let desc = empty_desc(
            vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Min),
                    operands: vec![0, 0],
                    result: Some(1),
                },
                KernelOp {
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![0, 0, 1],
                    result: None,
                },
            ],
            vec![LiteralValue::U32(5)],
        );
        let out = identity_elim(&desc);
        assert_eq!(out.body.ops[2].operands, vec![0, 0, 0]);
    }

    #[test]
    fn chained_eliminations_compose() {
        // r0 = Lit(varying), r1 = Lit(0), r2 = Lit(1)
        // r3 = Add(r0, r1)   -> kept = r0
        // r4 = Mul(r3, r2)   -> kept = r3 -> resolves to r0
        // Store(r4) should end up referencing r0.
        let desc = empty_desc(
            vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![1],
                    result: Some(1),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![2],
                    result: Some(2),
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Add),
                    operands: vec![0, 1],
                    result: Some(3),
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Mul),
                    operands: vec![3, 2],
                    result: Some(4),
                },
                KernelOp {
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![0, 0, 4],
                    result: None,
                },
            ],
            vec![
                LiteralValue::U32(42),
                LiteralValue::U32(0),
                LiteralValue::U32(1),
            ],
        );
        let out = identity_elim(&desc);
        assert_eq!(out.body.ops[5].operands, vec![0, 0, 0]);
    }

    #[test]
    fn non_identity_literals_do_nothing() {
        let desc = empty_desc(
            vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![1],
                    result: Some(1),
                }, // 5, not identity
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Add),
                    operands: vec![0, 1],
                    result: Some(2),
                },
                KernelOp {
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![0, 0, 2],
                    result: None,
                },
            ],
            vec![LiteralValue::U32(7), LiteralValue::U32(5)],
        );
        let out = identity_elim(&desc);
        // Store still references the Add result.
        assert_eq!(out.body.ops[3].operands, vec![0, 0, 2]);
    }

    #[test]
    fn float_zero_works_for_add() {
        let desc = empty_desc(
            vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![1],
                    result: Some(1),
                }, // 0.0
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Add),
                    operands: vec![0, 1],
                    result: Some(2),
                },
                KernelOp {
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![0, 0, 2],
                    result: None,
                },
            ],
            vec![
                LiteralValue::F32(std::f32::consts::PI),
                LiteralValue::F32(0.0),
            ],
        );
        let out = identity_elim(&desc);
        assert_eq!(out.body.ops[3].operands, vec![0, 0, 0]);
    }

    #[test]
    fn idempotent_on_already_eliminated() {
        let desc = empty_desc(
            vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![1],
                    result: Some(1),
                }, // 0
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Add),
                    operands: vec![0, 1],
                    result: Some(2),
                },
                KernelOp {
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![0, 0, 2],
                    result: None,
                },
            ],
            vec![LiteralValue::U32(7), LiteralValue::U32(0)],
        );
        let once = identity_elim(&desc);
        let twice = identity_elim(&once);
        assert_eq!(once.body.ops.len(), twice.body.ops.len());
        assert_eq!(once.body.ops[3].operands, twice.body.ops[3].operands);
    }

    #[test]
    fn empty_kernel_is_noop() {
        let desc = empty_desc(vec![], vec![]);
        let out = identity_elim(&desc);
        assert!(out.body.ops.is_empty());
    }

    #[test]
    fn select_with_true_cond_picks_then_branch() {
        // r0 = Lit(true), r1 = Lit(7), r2 = Lit(99),
        // r3 = Select(r0, r1, r2), Store(_, _, r3)
        // After identity_elim: Store should reference r1 (the then branch).
        let desc = empty_desc(
            vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![1],
                    result: Some(1),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![2],
                    result: Some(2),
                },
                KernelOp {
                    kind: KernelOpKind::Select,
                    operands: vec![0, 1, 2],
                    result: Some(3),
                },
                KernelOp {
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![0, 0, 3],
                    result: None,
                },
            ],
            vec![
                LiteralValue::Bool(true),
                LiteralValue::U32(7),
                LiteralValue::U32(99),
            ],
        );
        let out = identity_elim(&desc);
        assert_eq!(out.body.ops[4].operands, vec![0, 0, 1]);
    }

    #[test]
    fn select_with_false_cond_picks_else_branch() {
        let desc = empty_desc(
            vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                }, // false
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![1],
                    result: Some(1),
                }, // 7
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![2],
                    result: Some(2),
                }, // 99
                KernelOp {
                    kind: KernelOpKind::Select,
                    operands: vec![0, 1, 2],
                    result: Some(3),
                },
                KernelOp {
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![0, 0, 3],
                    result: None,
                },
            ],
            vec![
                LiteralValue::Bool(false),
                LiteralValue::U32(7),
                LiteralValue::U32(99),
            ],
        );
        let out = identity_elim(&desc);
        // Picks the else branch (r2).
        assert_eq!(out.body.ops[4].operands, vec![0, 0, 2]);
    }

    #[test]
    fn select_with_non_literal_cond_unchanged() {
        // tid as cond  -  not a literal. Select stays.
        let desc = empty_desc(
            vec![
                KernelOp {
                    kind: KernelOpKind::LocalInvocationId,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(1),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![1],
                    result: Some(2),
                },
                KernelOp {
                    kind: KernelOpKind::Select,
                    operands: vec![0, 1, 2],
                    result: Some(3),
                },
                KernelOp {
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![0, 0, 3],
                    result: None,
                },
            ],
            vec![LiteralValue::U32(7), LiteralValue::U32(99)],
        );
        let out = identity_elim(&desc);
        // Store still references the Select result.
        assert_eq!(out.body.ops[4].operands, vec![0, 0, 3]);
    }

    #[test]
    fn fma_with_a_zero_picks_c() {
        // Fma(0, x, c) → c
        let desc = empty_desc(
            vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                }, // 0
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![1],
                    result: Some(1),
                }, // x
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![2],
                    result: Some(2),
                }, // c
                KernelOp {
                    kind: KernelOpKind::Fma,
                    operands: vec![0, 1, 2],
                    result: Some(3),
                },
                KernelOp {
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![0, 0, 3],
                    result: None,
                },
            ],
            vec![
                LiteralValue::F32(0.0),
                LiteralValue::F32(7.0),
                LiteralValue::F32(99.0),
            ],
        );
        let out = identity_elim(&desc);
        // Store should now reference r2 (c).
        assert_eq!(out.body.ops[4].operands, vec![0, 0, 2]);
    }

    #[test]
    fn fma_with_b_zero_picks_c() {
        // Fma(x, 0, c) → c
        let desc = empty_desc(
            vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                }, // x
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![1],
                    result: Some(1),
                }, // 0
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![2],
                    result: Some(2),
                }, // c
                KernelOp {
                    kind: KernelOpKind::Fma,
                    operands: vec![0, 1, 2],
                    result: Some(3),
                },
                KernelOp {
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![0, 0, 3],
                    result: None,
                },
            ],
            vec![
                LiteralValue::F32(7.0),
                LiteralValue::F32(0.0),
                LiteralValue::F32(99.0),
            ],
        );
        let out = identity_elim(&desc);
        assert_eq!(out.body.ops[4].operands, vec![0, 0, 2]);
    }

    #[test]
    fn fma_with_no_zero_unchanged() {
        // Fma(x, y, c) where neither factor is 0  -  Fma op stays.
        let desc = empty_desc(
            vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![1],
                    result: Some(1),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![2],
                    result: Some(2),
                },
                KernelOp {
                    kind: KernelOpKind::Fma,
                    operands: vec![0, 1, 2],
                    result: Some(3),
                },
                KernelOp {
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![0, 0, 3],
                    result: None,
                },
            ],
            vec![
                LiteralValue::F32(7.0),
                LiteralValue::F32(2.0),
                LiteralValue::F32(99.0),
            ],
        );
        let out = identity_elim(&desc);
        // Store still references r3 (Fma's result).
        assert_eq!(out.body.ops[4].operands, vec![0, 0, 3]);
    }

    #[test]
    fn select_with_non_bool_cond_unchanged() {
        // U32 literal as cond  -  wrong type, won't fold.
        let desc = empty_desc(
            vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                }, // U32 1
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![1],
                    result: Some(1),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![2],
                    result: Some(2),
                },
                KernelOp {
                    kind: KernelOpKind::Select,
                    operands: vec![0, 1, 2],
                    result: Some(3),
                },
                KernelOp {
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![0, 0, 3],
                    result: None,
                },
            ],
            vec![
                LiteralValue::U32(1),
                LiteralValue::U32(7),
                LiteralValue::U32(99),
            ],
        );
        let out = identity_elim(&desc);
        assert_eq!(out.body.ops[4].operands, vec![0, 0, 3]);
    }
}
