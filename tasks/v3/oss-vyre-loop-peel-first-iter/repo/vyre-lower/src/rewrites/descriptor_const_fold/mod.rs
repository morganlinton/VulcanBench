//! Constant folding rewrite.
//!
//! Folds compile-time-constant operations into literal-pool entries. Scalar
//! evaluation semantics are owned by `vyre-foundation::ir::eval`; this rewrite
//! only maps lowered literal-pool values to and from foundation IR literals so
//! every optimization layer shares one const-eval contract.

use crate::{KernelBody, KernelDescriptor, KernelOpKind, LiteralValue};
use rustc_hash::FxHashMap;
use vyre_foundation::ir::{
    eval::{fold_binary_literal, fold_cast_literal, fold_fma_literal, fold_unary_literal},
    BinOp, DataType, Expr, UnOp,
};

#[must_use]
pub fn descriptor_const_fold(desc: &KernelDescriptor) -> KernelDescriptor {
    let mut out = desc.clone();
    out.body = const_fold_body(out.body);
    out
}

fn const_fold_body(mut body: KernelBody) -> KernelBody {
    // Build a result-id → literal-pool index map. Storing indices avoids
    // cloning every literal into the side table before any fold has fired.
    let mut result_to_literal_idx: FxHashMap<u32, usize> =
        FxHashMap::with_capacity_and_hasher(body.ops.len(), Default::default());
    for op in &body.ops {
        if matches!(op.kind, KernelOpKind::Literal) {
            if let (Some(result), Some(pool_idx)) = (op.result, op.operands.first()) {
                let pool_idx = *pool_idx as usize;
                if pool_idx < body.literals.len() {
                    result_to_literal_idx.insert(result, pool_idx);
                }
            }
        }
    }

    for op in body.ops.iter_mut() {
        let folded = match &op.kind {
            KernelOpKind::BinOpKind(bin_op) => {
                if op.operands.len() != 2 {
                    None
                } else {
                    let lhs = result_to_literal_idx
                        .get(&op.operands[0])
                        .and_then(|idx| body.literals.get(*idx));
                    let rhs = result_to_literal_idx
                        .get(&op.operands[1])
                        .and_then(|idx| body.literals.get(*idx));
                    match (lhs, rhs) {
                        (Some(a), Some(b)) => fold_binop(*bin_op, a, b),
                        _ => None,
                    }
                }
            }
            KernelOpKind::UnOpKind(un_op) => {
                if op.operands.len() != 1 {
                    None
                } else {
                    result_to_literal_idx
                        .get(&op.operands[0])
                        .and_then(|idx| body.literals.get(*idx))
                        .and_then(|x| fold_unop(un_op, x))
                }
            }
            KernelOpKind::Cast { target } => {
                if op.operands.len() != 1 {
                    None
                } else {
                    result_to_literal_idx
                        .get(&op.operands[0])
                        .and_then(|idx| body.literals.get(*idx))
                        .and_then(|x| fold_cast(x, target))
                }
            }
            KernelOpKind::Fma => {
                if op.operands.len() != 3 {
                    None
                } else {
                    let a = result_to_literal_idx
                        .get(&op.operands[0])
                        .and_then(|idx| body.literals.get(*idx));
                    let b = result_to_literal_idx
                        .get(&op.operands[1])
                        .and_then(|idx| body.literals.get(*idx));
                    let c = result_to_literal_idx
                        .get(&op.operands[2])
                        .and_then(|idx| body.literals.get(*idx));
                    match (a, b, c) {
                        (Some(a), Some(b), Some(c)) => fold_fma(a, b, c),
                        _ => None,
                    }
                }
            }
            _ => None,
        };
        if let Some(value) = folded {
            // Replace this op with a Literal pointing at a new pool entry.
            let pool_idx = body.literals.len() as u32;
            body.literals.push(value);
            op.kind = KernelOpKind::Literal;
            op.operands.clear();
            op.operands.push(pool_idx);
            // Keep op.result so downstream refs still find this id.
            // Update the result_to_literal map so further folds in this
            // pass can see the new constant.
            if let Some(r) = op.result {
                result_to_literal_idx.insert(r, pool_idx as usize);
            }
        }
    }

    body.child_bodies = body.child_bodies.into_iter().map(const_fold_body).collect();

    body
}

fn fold_fma(a: &LiteralValue, b: &LiteralValue, c: &LiteralValue) -> Option<LiteralValue> {
    expr_to_literal(fold_fma_literal(
        &literal_to_expr(a),
        &literal_to_expr(b),
        &literal_to_expr(c),
    )?)
}

fn fold_cast(src: &LiteralValue, target: &DataType) -> Option<LiteralValue> {
    expr_to_literal(fold_cast_literal(target, &literal_to_expr(src))?)
}

fn fold_unop(op: &UnOp, a: &LiteralValue) -> Option<LiteralValue> {
    expr_to_literal(fold_unary_literal(op, &literal_to_expr(a))?)
}

fn fold_binop(op: BinOp, a: &LiteralValue, b: &LiteralValue) -> Option<LiteralValue> {
    expr_to_literal(fold_binary_literal(
        &op,
        &literal_to_expr(a),
        &literal_to_expr(b),
    )?)
}

fn literal_to_expr(value: &LiteralValue) -> Expr {
    match value {
        LiteralValue::U32(value) => Expr::u32(*value),
        LiteralValue::I32(value) => Expr::i32(*value),
        LiteralValue::F32(value) => Expr::f32(*value),
        LiteralValue::Bool(value) => Expr::bool(*value),
    }
}

fn expr_to_literal(expr: Expr) -> Option<LiteralValue> {
    match expr {
        Expr::LitU32(value) => Some(LiteralValue::U32(value)),
        Expr::LitI32(value) => Some(LiteralValue::I32(value)),
        Expr::LitF32(value) => Some(LiteralValue::F32(value)),
        Expr::LitBool(value) => Some(LiteralValue::Bool(value)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BindingLayout, Dispatch, KernelBody, KernelDescriptor, KernelOp, KernelOpKind};

    fn fold_kernel(op: BinOp, a: u32, b: u32) -> KernelDescriptor {
        KernelDescriptor {
            id: "fold".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops: vec![
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
                        kind: KernelOpKind::BinOpKind(op),
                        operands: vec![0, 1],
                        result: Some(2),
                    },
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(a), LiteralValue::U32(b)],
            },
        }
    }

    #[test]
    fn const_fold_add_u32() {
        let out = descriptor_const_fold(&fold_kernel(BinOp::Add, 3, 4));
        // The Add op should now be a Literal pointing at the new pool entry (7).
        assert!(matches!(out.body.ops[2].kind, KernelOpKind::Literal));
        let pool_idx = out.body.ops[2].operands[0] as usize;
        assert_eq!(out.body.literals[pool_idx], LiteralValue::U32(7));
    }

    #[test]
    fn const_fold_mul_u32() {
        let out = descriptor_const_fold(&fold_kernel(BinOp::Mul, 6, 7));
        let pool_idx = out.body.ops[2].operands[0] as usize;
        assert_eq!(out.body.literals[pool_idx], LiteralValue::U32(42));
    }

    #[test]
    fn const_fold_div_by_zero_uses_shared_contract() {
        let out = descriptor_const_fold(&fold_kernel(BinOp::Div, 10, 0));
        assert!(matches!(out.body.ops[2].kind, KernelOpKind::Literal));
        let pool_idx = out.body.ops[2].operands[0] as usize;
        assert_eq!(out.body.literals[pool_idx], LiteralValue::U32(u32::MAX));
    }

    #[test]
    fn const_fold_skips_when_one_operand_not_literal() {
        // tid + 5  -  tid is not a literal, so nothing to fold.
        let desc = KernelDescriptor {
            id: "no_fold".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(64, 1, 1),
            body: KernelBody {
                ops: vec![
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
                        kind: KernelOpKind::BinOpKind(BinOp::Add),
                        operands: vec![0, 1],
                        result: Some(2),
                    },
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(5)],
            },
        };
        let out = descriptor_const_fold(&desc);
        // Add stays as BinOp because tid isn't a literal.
        assert!(matches!(
            out.body.ops[2].kind,
            KernelOpKind::BinOpKind(BinOp::Add)
        ));
    }

    #[test]
    fn const_fold_is_idempotent() {
        let desc = fold_kernel(BinOp::Add, 3, 4);
        let once = descriptor_const_fold(&desc);
        let twice = descriptor_const_fold(&once);
        assert_eq!(once.body.ops.len(), twice.body.ops.len());
        assert_eq!(once.body.literals, twice.body.literals);
    }

    #[test]
    fn const_fold_chain_propagates() {
        // (3 + 4) * 5 → fold to (7) → fold to 35.
        let desc = KernelDescriptor {
            id: "chain".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops: vec![
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
                        kind: KernelOpKind::Literal,
                        operands: vec![2],
                        result: Some(3),
                    },
                    KernelOp {
                        kind: KernelOpKind::BinOpKind(BinOp::Mul),
                        operands: vec![2, 3],
                        result: Some(4),
                    },
                ],
                child_bodies: vec![],
                literals: vec![
                    LiteralValue::U32(3),
                    LiteralValue::U32(4),
                    LiteralValue::U32(5),
                ],
            },
        };
        let out = descriptor_const_fold(&desc);
        // Both Add and Mul should be replaced with Literals.
        assert!(matches!(out.body.ops[2].kind, KernelOpKind::Literal));
        assert!(matches!(out.body.ops[4].kind, KernelOpKind::Literal));
        // Final Mul result should be 35.
        let pool_idx = out.body.ops[4].operands[0] as usize;
        assert_eq!(out.body.literals[pool_idx], LiteralValue::U32(35));
    }

    #[test]
    fn const_fold_f32_min_max() {
        let desc = KernelDescriptor {
            id: "f_minmax".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops: vec![
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
                        kind: KernelOpKind::BinOpKind(BinOp::Min),
                        operands: vec![0, 1],
                        result: Some(2),
                    },
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::F32(3.0), LiteralValue::F32(5.0)],
            },
        };
        let out = descriptor_const_fold(&desc);
        let pool_idx = out.body.ops[2].operands[0] as usize;
        assert_eq!(out.body.literals[pool_idx], LiteralValue::F32(3.0));
    }

    #[test]
    fn const_fold_f32_min_with_nan_not_folded() {
        let desc = KernelDescriptor {
            id: "f_min_nan".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops: vec![
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
                        kind: KernelOpKind::BinOpKind(BinOp::Min),
                        operands: vec![0, 1],
                        result: Some(2),
                    },
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::F32(f32::NAN), LiteralValue::F32(5.0)],
            },
        };
        let out = descriptor_const_fold(&desc);
        // NaN refused  -  Min op stays.
        assert!(matches!(
            out.body.ops[2].kind,
            KernelOpKind::BinOpKind(BinOp::Min)
        ));
    }

    #[test]
    fn const_fold_f32_only_when_finite() {
        let desc = KernelDescriptor {
            id: "f_inf".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops: vec![
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
                        kind: KernelOpKind::BinOpKind(BinOp::Div),
                        operands: vec![0, 1],
                        result: Some(2),
                    },
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::F32(1.0), LiteralValue::F32(0.0)],
            },
        };
        let out = descriptor_const_fold(&desc);
        // 1.0 / 0.0 = inf  -  div by 0 prevents fold.
        assert!(matches!(
            out.body.ops[2].kind,
            KernelOpKind::BinOpKind(BinOp::Div)
        ));
    }

    #[test]
    fn const_fold_bool_and_or_xor() {
        let test = |op: BinOp, a: bool, b: bool, expected: bool| {
            let desc = KernelDescriptor {
                id: "b".into(),
                bindings: BindingLayout { slots: vec![] },
                dispatch: Dispatch::new(1, 1, 1),
                body: KernelBody {
                    ops: vec![
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
                            kind: KernelOpKind::BinOpKind(op),
                            operands: vec![0, 1],
                            result: Some(2),
                        },
                    ],
                    child_bodies: vec![],
                    literals: vec![LiteralValue::Bool(a), LiteralValue::Bool(b)],
                },
            };

            let out = descriptor_const_fold(&desc);
            let pool_idx = out.body.ops[2].operands[0] as usize;
            assert_eq!(out.body.literals[pool_idx], LiteralValue::Bool(expected));
        };
        test(BinOp::And, true, false, false);
        test(BinOp::Or, true, false, true);
        test(BinOp::BitXor, true, false, true);
        test(BinOp::BitXor, true, true, false);
    }

    fn unop_kernel(op: UnOp, lit: LiteralValue) -> KernelDescriptor {
        KernelDescriptor {
            id: "unop".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops: vec![
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![0],
                        result: Some(0),
                    },
                    KernelOp {
                        kind: KernelOpKind::UnOpKind(op),
                        operands: vec![0],
                        result: Some(1),
                    },
                ],
                child_bodies: vec![],
                literals: vec![lit],
            },
        }
    }

    fn folded_value(desc: &KernelDescriptor) -> Option<LiteralValue> {
        let out = descriptor_const_fold(desc);
        if !matches!(out.body.ops[1].kind, KernelOpKind::Literal) {
            return None;
        }
        let pool_idx = out.body.ops[1].operands[0] as usize;
        out.body.literals.get(pool_idx).cloned()
    }

    #[test]
    fn const_fold_unop_bitnot_u32() {
        let v = folded_value(&unop_kernel(UnOp::BitNot, LiteralValue::U32(0))).unwrap();
        assert_eq!(v, LiteralValue::U32(0xFFFF_FFFF));
    }

    #[test]
    fn const_fold_unop_negate_i32() {
        let v = folded_value(&unop_kernel(UnOp::Negate, LiteralValue::I32(7))).unwrap();
        assert_eq!(v, LiteralValue::I32(-7));
    }

    #[test]
    fn const_fold_unop_logical_not_bool() {
        let v = folded_value(&unop_kernel(UnOp::LogicalNot, LiteralValue::Bool(true))).unwrap();
        assert_eq!(v, LiteralValue::Bool(false));
        let v = folded_value(&unop_kernel(UnOp::LogicalNot, LiteralValue::Bool(false))).unwrap();
        assert_eq!(v, LiteralValue::Bool(true));
    }

    #[test]
    fn const_fold_unop_popcount_u32() {
        let v = folded_value(&unop_kernel(UnOp::Popcount, LiteralValue::U32(0xFF))).unwrap();
        assert_eq!(v, LiteralValue::U32(8));
    }

    #[test]
    fn const_fold_unop_clz_u32() {
        let v = folded_value(&unop_kernel(UnOp::Clz, LiteralValue::U32(1))).unwrap();
        assert_eq!(v, LiteralValue::U32(31));
        let v = folded_value(&unop_kernel(UnOp::Clz, LiteralValue::U32(0xFF00_0000))).unwrap();
        assert_eq!(v, LiteralValue::U32(0));
    }

    #[test]
    fn const_fold_unop_floor_f32() {
        let v = folded_value(&unop_kernel(UnOp::Floor, LiteralValue::F32(3.7))).unwrap();
        assert_eq!(v, LiteralValue::F32(3.0));
    }

    #[test]
    fn const_fold_unop_abs_f32() {
        let v = folded_value(&unop_kernel(UnOp::Abs, LiteralValue::F32(-2.5))).unwrap();
        assert_eq!(v, LiteralValue::F32(2.5));
    }

    #[test]
    fn const_fold_unop_sqrt_negative_folds_to_canonical_nan() {
        let folded = folded_value(&unop_kernel(UnOp::Sqrt, LiteralValue::F32(-1.0)));
        let Some(LiteralValue::F32(value)) = folded else {
            panic!("sqrt of negative should fold through the shared literal evaluator");
        };
        assert_eq!(value.to_bits(), 0x7FC0_0000);
    }

    #[test]
    fn const_fold_unop_type_mismatch_not_folded() {
        // Negate on a Bool  -  no semantics; must not fold.
        let folded = folded_value(&unop_kernel(UnOp::Negate, LiteralValue::Bool(true)));
        assert!(folded.is_none());
    }

    fn cast_kernel(src: LiteralValue, target: DataType) -> KernelDescriptor {
        KernelDescriptor {
            id: "cast".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops: vec![
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![0],
                        result: Some(0),
                    },
                    KernelOp {
                        kind: KernelOpKind::Cast { target },
                        operands: vec![0],
                        result: Some(1),
                    },
                ],
                child_bodies: vec![],
                literals: vec![src],
            },
        }
    }

    fn cast_folded_value(src: LiteralValue, target: DataType) -> Option<LiteralValue> {
        let out = descriptor_const_fold(&cast_kernel(src, target));
        if !matches!(out.body.ops[1].kind, KernelOpKind::Literal) {
            return None;
        }
        let pool_idx = out.body.ops[1].operands[0] as usize;
        out.body.literals.get(pool_idx).cloned()
    }

    #[test]
    fn const_fold_cast_u32_to_i32() {
        let v = cast_folded_value(LiteralValue::U32(7), DataType::I32).unwrap();
        assert_eq!(v, LiteralValue::I32(7));
    }

    #[test]
    fn const_fold_cast_i32_to_u32_negative_wraps() {
        let v = cast_folded_value(LiteralValue::I32(-1), DataType::U32).unwrap();
        assert_eq!(v, LiteralValue::U32(u32::MAX));
    }

    #[test]
    fn const_fold_cast_u32_to_f32() {
        let v = cast_folded_value(LiteralValue::U32(42), DataType::F32).unwrap();
        assert_eq!(v, LiteralValue::F32(42.0));
    }

    #[test]
    fn const_fold_cast_f32_to_u32_in_range() {
        let v = cast_folded_value(LiteralValue::F32(3.7), DataType::U32).unwrap();
        assert_eq!(v, LiteralValue::U32(3));
    }

    #[test]
    fn const_fold_cast_f32_to_u32_negative_uses_shared_contract() {
        let v = cast_folded_value(LiteralValue::F32(-1.0), DataType::U32).unwrap();
        assert_eq!(v, LiteralValue::U32(0));
    }

    #[test]
    fn const_fold_cast_f32_nan_not_folded() {
        let v = cast_folded_value(LiteralValue::F32(f32::NAN), DataType::I32);
        assert!(v.is_none(), "NaN must not fold");
    }

    #[test]
    fn const_fold_cast_bool_to_u32() {
        assert_eq!(
            cast_folded_value(LiteralValue::Bool(true), DataType::U32).unwrap(),
            LiteralValue::U32(1)
        );
        assert_eq!(
            cast_folded_value(LiteralValue::Bool(false), DataType::U32).unwrap(),
            LiteralValue::U32(0)
        );
    }

    #[test]
    fn const_fold_cast_same_type_is_noop_fold() {
        let v = cast_folded_value(LiteralValue::U32(99), DataType::U32).unwrap();
        assert_eq!(v, LiteralValue::U32(99));
    }

    #[test]
    fn const_fold_cast_unsupported_pair_not_folded() {
        // U32 -> F64 stays outside descriptor fold coverage; F64 exists in
        // DataType, but this pass only folds casts it can prove exactly.
        let v = cast_folded_value(LiteralValue::U32(1), DataType::F64);
        assert!(v.is_none(), "unsupported cast must not fold");
    }

    fn cmp_kernel(op: BinOp, a: u32, b: u32) -> KernelDescriptor {
        fold_kernel(op, a, b)
    }

    fn cmp_folded_bool(op: BinOp, a: u32, b: u32) -> Option<bool> {
        let out = descriptor_const_fold(&cmp_kernel(op, a, b));
        if !matches!(out.body.ops[2].kind, KernelOpKind::Literal) {
            return None;
        }
        let pool_idx = out.body.ops[2].operands[0] as usize;
        match out.body.literals.get(pool_idx) {
            Some(LiteralValue::Bool(b)) => Some(*b),
            _ => None,
        }
    }

    fn fold_i32_kernel(op: BinOp, a: i32, b: i32) -> KernelDescriptor {
        KernelDescriptor {
            id: "i32fold".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops: vec![
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
                        kind: KernelOpKind::BinOpKind(op),
                        operands: vec![0, 1],
                        result: Some(2),
                    },
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::I32(a), LiteralValue::I32(b)],
            },
        }
    }

    fn fold_i32_value(op: BinOp, a: i32, b: i32) -> Option<i32> {
        let out = descriptor_const_fold(&fold_i32_kernel(op, a, b));
        if !matches!(out.body.ops[2].kind, KernelOpKind::Literal) {
            return None;
        }
        let pool_idx = out.body.ops[2].operands[0] as usize;
        match out.body.literals.get(pool_idx) {
            Some(LiteralValue::I32(v)) => Some(*v),
            _ => None,
        }
    }

    #[test]
    fn const_fold_i32_bitwise() {
        assert_eq!(fold_i32_value(BinOp::BitAnd, 0xFF, 0x0F), Some(0x0F));
        assert_eq!(fold_i32_value(BinOp::BitOr, 0xF0, 0x0F), Some(0xFF));
        assert_eq!(fold_i32_value(BinOp::BitXor, 0xFF, 0x0F), Some(0xF0));
    }

    #[test]
    fn const_fold_i32_shl_shr_arithmetic() {
        assert_eq!(fold_i32_value(BinOp::Shl, 1, 4), Some(16));
        assert_eq!(fold_i32_value(BinOp::Shr, 16, 2), Some(4));
        // Arithmetic right-shift sign-extends.
        assert_eq!(fold_i32_value(BinOp::Shr, -16, 2), Some(-4));
    }

    #[test]
    fn const_fold_wrapping_add_sub_u32() {
        // 0xFFFFFFFF + 1 wraps to 0  -  same result as Add (already
        // wraps), but the WrappingAdd variant carries different
        // semantic intent.
        let out = descriptor_const_fold(&fold_kernel(BinOp::WrappingAdd, 0xFFFF_FFFF, 1));
        let pool_idx = out.body.ops[2].operands[0] as usize;
        assert_eq!(out.body.literals[pool_idx], LiteralValue::U32(0));

        let out = descriptor_const_fold(&fold_kernel(BinOp::WrappingSub, 0, 1));
        let pool_idx = out.body.ops[2].operands[0] as usize;
        assert_eq!(out.body.literals[pool_idx], LiteralValue::U32(0xFFFF_FFFF));
    }

    #[test]
    fn const_fold_eq_u32() {
        assert_eq!(cmp_folded_bool(BinOp::Eq, 7, 7), Some(true));
        assert_eq!(cmp_folded_bool(BinOp::Eq, 7, 8), Some(false));
    }

    #[test]
    fn const_fold_ne_u32() {
        assert_eq!(cmp_folded_bool(BinOp::Ne, 7, 7), Some(false));
        assert_eq!(cmp_folded_bool(BinOp::Ne, 7, 8), Some(true));
    }

    #[test]
    fn const_fold_lt_le_gt_ge_u32() {
        assert_eq!(cmp_folded_bool(BinOp::Lt, 3, 5), Some(true));
        assert_eq!(cmp_folded_bool(BinOp::Lt, 5, 5), Some(false));
        assert_eq!(cmp_folded_bool(BinOp::Le, 5, 5), Some(true));
        assert_eq!(cmp_folded_bool(BinOp::Gt, 5, 3), Some(true));
        assert_eq!(cmp_folded_bool(BinOp::Gt, 5, 5), Some(false));
        assert_eq!(cmp_folded_bool(BinOp::Ge, 5, 5), Some(true));
    }

    #[test]
    fn const_fold_eq_bool() {
        let desc = KernelDescriptor {
            id: "be".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops: vec![
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
                        kind: KernelOpKind::BinOpKind(BinOp::Eq),
                        operands: vec![0, 1],
                        result: Some(2),
                    },
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::Bool(true), LiteralValue::Bool(false)],
            },
        };
        let out = descriptor_const_fold(&desc);
        let pool_idx = out.body.ops[2].operands[0] as usize;
        assert_eq!(out.body.literals[pool_idx], LiteralValue::Bool(false));
    }

    #[test]
    fn const_fold_f32_nan_compare_not_folded() {
        // NaN comparisons have IEEE-specific semantics; fold_binop
        // refuses them.
        let desc = KernelDescriptor {
            id: "nan".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops: vec![
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
                        kind: KernelOpKind::BinOpKind(BinOp::Eq),
                        operands: vec![0, 1],
                        result: Some(2),
                    },
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::F32(f32::NAN), LiteralValue::F32(1.0)],
            },
        };
        let out = descriptor_const_fold(&desc);
        // Stays as Eq  -  NaN refused.
        assert!(matches!(
            out.body.ops[2].kind,
            KernelOpKind::BinOpKind(BinOp::Eq)
        ));
    }

    #[test]
    fn const_fold_comparison_chains_into_select_fold() {
        // Lt(3, 5) → Lit(true). Then Select(Lit(true), then, else)
        // → then via identity_elim. End-to-end via run_all so the
        // pipeline composition is exercised.
        let desc = KernelDescriptor {
            id: "chain".into(),
            bindings: BindingLayout {
                slots: vec![crate::BindingSlot {
                    slot: 0,
                    element_type: vyre_foundation::ir::DataType::U32,
                    element_count: None,
                    memory_class: crate::MemoryClass::Global,
                    visibility: crate::BindingVisibility::ReadWrite,
                    name: "out".into(),
                }],
            },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops: vec![
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![0],
                        result: Some(0),
                    }, // 3
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![1],
                        result: Some(1),
                    }, // 5
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![2],
                        result: Some(2),
                    }, // 0 (idx)
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![3],
                        result: Some(3),
                    }, // 7 (then val)
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![4],
                        result: Some(4),
                    }, // 99 (else val)
                    KernelOp {
                        kind: KernelOpKind::BinOpKind(BinOp::Lt),
                        operands: vec![0, 1],
                        result: Some(5),
                    }, // → Lit(true)
                    KernelOp {
                        kind: KernelOpKind::Select,
                        operands: vec![5, 3, 4],
                        result: Some(6),
                    }, // → r3 (=7)
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 2, 6],
                        result: None,
                    },
                ],
                child_bodies: vec![],
                literals: vec![
                    LiteralValue::U32(3),
                    LiteralValue::U32(5),
                    LiteralValue::U32(0),
                    LiteralValue::U32(7),
                    LiteralValue::U32(99),
                ],
            },
        };

        let out = crate::rewrites::run_all(&desc);
        // Final store value must reference the literal pool entry holding
        // U32(7)  -  the then branch was selected.
        let store = out
            .body
            .ops
            .iter()
            .find(|o| matches!(o.kind, KernelOpKind::StoreGlobal))
            .expect("Fix: store survived");
        let val_id = store.operands[2];
        // Find the producer of val_id; it must be a Literal of U32(7).
        let producer = out
            .body
            .ops
            .iter()
            .find(|o| o.result == Some(val_id))
            .expect("Fix: producer of val");
        assert!(matches!(producer.kind, KernelOpKind::Literal));
        let pool_idx = producer.operands[0] as usize;
        assert_eq!(out.body.literals[pool_idx], LiteralValue::U32(7));
    }

    #[test]
    fn const_fold_fma_finite_f32() {
        // Fma(2, 3, 4) = 2*3 + 4 = 10
        let desc = KernelDescriptor {
            id: "fma".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops: vec![
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
                ],
                child_bodies: vec![],
                literals: vec![
                    LiteralValue::F32(2.0),
                    LiteralValue::F32(3.0),
                    LiteralValue::F32(4.0),
                ],
            },
        };
        let out = descriptor_const_fold(&desc);
        assert!(matches!(out.body.ops[3].kind, KernelOpKind::Literal));
        let pool_idx = out.body.ops[3].operands[0] as usize;
        assert_eq!(out.body.literals[pool_idx], LiteralValue::F32(10.0));
    }

    #[test]
    fn const_fold_fma_infinite_result_uses_shared_contract() {
        // Fma(MAX, 2, 0) overflows to +inf under the foundation literal
        // evaluator. Lower must not keep a separate finite-only policy.
        let desc = KernelDescriptor {
            id: "fma_overflow".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops: vec![
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
                ],
                child_bodies: vec![],
                literals: vec![
                    LiteralValue::F32(f32::MAX),
                    LiteralValue::F32(2.0),
                    LiteralValue::F32(0.0),
                ],
            },
        };
        let out = descriptor_const_fold(&desc);
        assert!(matches!(out.body.ops[3].kind, KernelOpKind::Literal));
        let pool_idx = out.body.ops[3].operands[0] as usize;
        let LiteralValue::F32(value) = out.body.literals[pool_idx] else {
            panic!("FMA overflow should fold to an f32 literal");
        };
        assert!(value.is_infinite() && value.is_sign_positive());
    }

    #[test]
    fn const_fold_fma_skips_when_one_operand_not_literal() {
        // Fma(tid, lit, lit)  -  tid not a literal.
        let desc = KernelDescriptor {
            id: "fma_tid".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(64, 1, 1),
            body: KernelBody {
                ops: vec![
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
                        kind: KernelOpKind::Fma,
                        operands: vec![0, 1, 2],
                        result: Some(3),
                    },
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::F32(2.0), LiteralValue::F32(4.0)],
            },
        };
        let out = descriptor_const_fold(&desc);
        assert!(matches!(out.body.ops[3].kind, KernelOpKind::Fma));
    }

    #[test]
    fn const_fold_fma_int_operands_not_folded() {
        // Fma is fundamentally a float op; integer operands shouldn't fold.
        let desc = KernelDescriptor {
            id: "fma_int".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops: vec![
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
                ],
                child_bodies: vec![],
                literals: vec![
                    LiteralValue::U32(2),
                    LiteralValue::U32(3),
                    LiteralValue::U32(4),
                ],
            },
        };
        let out = descriptor_const_fold(&desc);
        assert!(matches!(out.body.ops[3].kind, KernelOpKind::Fma));
    }

    #[test]
    fn const_fold_unop_skips_when_operand_not_literal() {
        let desc = KernelDescriptor {
            id: "no_fold".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops: vec![
                    KernelOp {
                        kind: KernelOpKind::LocalInvocationId,
                        operands: vec![0],
                        result: Some(0),
                    },
                    KernelOp {
                        kind: KernelOpKind::UnOpKind(UnOp::BitNot),
                        operands: vec![0],
                        result: Some(1),
                    },
                ],
                child_bodies: vec![],
                literals: vec![],
            },
        };
        let out = descriptor_const_fold(&desc);
        // UnOp on non-literal stays as UnOp.
        assert!(matches!(
            out.body.ops[1].kind,
            KernelOpKind::UnOpKind(UnOp::BitNot)
        ));
    }
}
