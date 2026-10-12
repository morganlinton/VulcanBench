use super::*;

#[test]
fn div_u32_by_zero_is_max() {
    assert_eq!(eval_binop_u32(BinOp::Div, 42, 0), Value::U32(u32::MAX));
}

#[test]
fn mod_u32_by_zero_is_zero() {
    assert_eq!(eval_binop_u32(BinOp::Mod, 42, 0), Value::U32(0));
}

// ---------------------------------------------------------------------------
// BinOp – i32 (signed bitwise + div-by-zero + shift)
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn prop_binop_add_i32(a in any::<i32>(), b in any::<i32>()) {
        prop_assert_eq!(eval_binop_i32(BinOp::Add, a, b), Value::I32(a.wrapping_add(b)));
    }

    #[test]
    fn prop_binop_sub_i32(a in any::<i32>(), b in any::<i32>()) {
        prop_assert_eq!(eval_binop_i32(BinOp::Sub, a, b), Value::I32(a.wrapping_sub(b)));
    }

    #[test]
    fn prop_binop_mul_i32(a in any::<i32>(), b in any::<i32>()) {
        prop_assert_eq!(eval_binop_i32(BinOp::Mul, a, b), Value::I32(a.wrapping_mul(b)));
    }

    #[test]
    fn prop_binop_div_i32(a in any::<i32>(), b in any::<i32>()) {
        let program = empty_program();
        let expr = Expr::BinOp {
            op: BinOp::Div,
            left: Box::new(Expr::i32(a)),
            right: Box::new(Expr::i32(b)),
        };
        let result = eval_expr::eval(&expr, &mut zero_invocation(&program), &mut Memory::empty(), &program);
        if b == 0 || (a == i32::MIN && b == -1) {
            prop_assert!(result.is_err(), "i32 division by zero or overflow must error, got: {result:?}");
        } else {
            prop_assert_eq!(result.unwrap(), Value::I32(a.wrapping_div(b)));
        }
    }

    #[test]
    fn prop_binop_mod_i32(a in any::<i32>(), b in any::<i32>()) {
        let program = empty_program();
        let expr = Expr::BinOp {
            op: BinOp::Mod,
            left: Box::new(Expr::i32(a)),
            right: Box::new(Expr::i32(b)),
        };
        let result = eval_expr::eval(&expr, &mut zero_invocation(&program), &mut Memory::empty(), &program);
        if b == 0 || (a == i32::MIN && b == -1) {
            prop_assert!(result.is_err(), "i32 remainder by zero or overflow must error, got: {result:?}");
        } else {
            prop_assert_eq!(result.unwrap(), Value::I32(a.wrapping_rem(b)));
        }
    }

    #[test]
    fn prop_binop_bitand_i32(a in any::<i32>(), b in any::<i32>()) {
        prop_assert_eq!(eval_binop_i32(BinOp::BitAnd, a, b), Value::I32(a & b));
    }

    #[test]
    fn prop_binop_bitor_i32(a in any::<i32>(), b in any::<i32>()) {
        prop_assert_eq!(eval_binop_i32(BinOp::BitOr, a, b), Value::I32(a | b));
    }

    #[test]
    fn prop_binop_bitxor_i32(a in any::<i32>(), b in any::<i32>()) {
        prop_assert_eq!(eval_binop_i32(BinOp::BitXor, a, b), Value::I32(a ^ b));
    }

    #[test]
    fn prop_binop_shl_i32(a in any::<i32>(), b in any::<i32>()) {
        prop_assert_eq!(eval_binop_i32(BinOp::Shl, a, b), Value::I32(a.wrapping_shl((b as u32) & 31)));
    }

    #[test]
    fn prop_binop_shr_i32(a in any::<i32>(), b in any::<i32>()) {
        prop_assert_eq!(eval_binop_i32(BinOp::Shr, a, b), Value::I32(a.wrapping_shr((b as u32) & 31)));
    }

    #[test]
    fn prop_binop_eq_i32(a in any::<i32>(), b in any::<i32>()) {
        prop_assert_eq!(eval_binop_i32(BinOp::Eq, a, b), Value::Bool(a == b));
    }

    #[test]
    fn prop_binop_lt_i32(a in any::<i32>(), b in any::<i32>()) {
        prop_assert_eq!(eval_binop_i32(BinOp::Lt, a, b), Value::Bool(a < b));
    }

    #[test]
    fn prop_binop_absdiff_i32(a in any::<i32>(), b in any::<i32>()) {
        prop_assert_eq!(eval_binop_i32(BinOp::AbsDiff, a, b), Value::U32(a.abs_diff(b)));
    }

    #[test]
    fn prop_binop_min_i32(a in any::<i32>(), b in any::<i32>()) {
        prop_assert_eq!(eval_binop_i32(BinOp::Min, a, b), Value::I32(a.min(b)));
    }

    #[test]
    fn prop_binop_max_i32(a in any::<i32>(), b in any::<i32>()) {
        prop_assert_eq!(eval_binop_i32(BinOp::Max, a, b), Value::I32(a.max(b)));
    }
}

// ---------------------------------------------------------------------------
// Edge cases: i32 divide-by-zero and MIN / -1 overflow
// ---------------------------------------------------------------------------

#[test]
fn div_i32_by_zero_errors() {
    let program = empty_program();
    let expr = Expr::BinOp {
        op: BinOp::Div,
        left: Box::new(Expr::i32(42)),
        right: Box::new(Expr::i32(0)),
    };
    let result = eval_expr::eval(
        &expr,
        &mut zero_invocation(&program),
        &mut Memory::empty(),
        &program,
    );
    assert!(matches!(result, Err(_)), "i32 division by zero must error");
}

