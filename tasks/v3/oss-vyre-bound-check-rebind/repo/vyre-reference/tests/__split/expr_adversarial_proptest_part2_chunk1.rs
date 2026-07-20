use super::super::*;

#[test]
fn mod_i32_by_zero_errors() {
    let program = empty_program();
    let expr = Expr::BinOp {
        op: BinOp::Mod,
        left: Box::new(Expr::i32(42)),
        right: Box::new(Expr::i32(0)),
    };
    let result = eval_expr::eval(
        &expr,
        &mut zero_invocation(&program),
        &mut Memory::empty(),
        &program,
    );
    assert!(matches!(result, Err(_)), "i32 remainder by zero must error");
}

#[test]
fn div_i32_min_by_neg_one_errors() {
    let program = empty_program();
    let expr = Expr::BinOp {
        op: BinOp::Div,
        left: Box::new(Expr::i32(i32::MIN)),
        right: Box::new(Expr::i32(-1)),
    };
    let result = eval_expr::eval(
        &expr,
        &mut zero_invocation(&program),
        &mut Memory::empty(),
        &program,
    );
    assert!(matches!(result, Err(_)), "i32 MIN / -1 overflow must error");
}

// ---------------------------------------------------------------------------
// BinOp – f32
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn prop_binop_add_f32(a in any::<f32>(), b in any::<f32>()) {
        let expected = expected_f32(canonical_f32(a) + canonical_f32(b));
        prop_assert_eq!(eval_binop_f32(BinOp::Add, a, b), expected);
    }

    #[test]
    fn prop_binop_sub_f32(a in any::<f32>(), b in any::<f32>()) {
        let expected = expected_f32(canonical_f32(a) - canonical_f32(b));
        prop_assert_eq!(eval_binop_f32(BinOp::Sub, a, b), expected);
    }

    #[test]
    fn prop_binop_mul_f32(a in any::<f32>(), b in any::<f32>()) {
        let expected = expected_f32(canonical_f32(a) * canonical_f32(b));
        prop_assert_eq!(eval_binop_f32(BinOp::Mul, a, b), expected);
    }

    #[test]
    fn prop_binop_div_f32(a in any::<f32>(), b in any::<f32>()) {
        let expected = expected_f32(canonical_f32(a) / canonical_f32(b));
        prop_assert_eq!(eval_binop_f32(BinOp::Div, a, b), expected);
    }

    #[test]
    fn prop_binop_min_f32(a in any::<f32>(), b in any::<f32>()) {
        let expected = expected_f32(canonical_f32(a).min(canonical_f32(b)));
        prop_assert_eq!(eval_binop_f32(BinOp::Min, a, b), expected);
    }

    #[test]
    fn prop_binop_max_f32(a in any::<f32>(), b in any::<f32>()) {
        let expected = expected_f32(canonical_f32(a).max(canonical_f32(b)));
        prop_assert_eq!(eval_binop_f32(BinOp::Max, a, b), expected);
    }

    #[test]
    fn prop_binop_eq_f32(a in any::<f32>(), b in any::<f32>()) {
        prop_assert_eq!(eval_binop_f32(BinOp::Eq, a, b), Value::Bool(canonical_f32(a) == canonical_f32(b)));
    }

    #[test]
    fn prop_binop_lt_f32(a in any::<f32>(), b in any::<f32>()) {
        prop_assert_eq!(eval_binop_f32(BinOp::Lt, a, b), Value::Bool(canonical_f32(a) < canonical_f32(b)));
    }
}

// ---------------------------------------------------------------------------
// UnOp – u32
// ---------------------------------------------------------------------------

