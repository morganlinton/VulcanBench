use super::super::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn prop_unop_negate_u32(a in any::<u32>()) {
        prop_assert_eq!(eval_unop_u32(UnOp::Negate, a), Value::U32(0u32.wrapping_sub(a)));
    }

    #[test]
    fn prop_unop_bitnot_u32(a in any::<u32>()) {
        prop_assert_eq!(eval_unop_u32(UnOp::BitNot, a), Value::U32(!a));
    }

    #[test]
    fn prop_unop_logicalnot_u32(a in any::<u32>()) {
        prop_assert_eq!(eval_unop_u32(UnOp::LogicalNot, a), Value::Bool(a == 0));
    }

    #[test]
    fn prop_unop_popcount_u32(a in any::<u32>()) {
        prop_assert_eq!(eval_unop_u32(UnOp::Popcount, a), Value::U32(a.count_ones()));
    }

    #[test]
    fn prop_unop_clz_u32(a in any::<u32>()) {
        prop_assert_eq!(eval_unop_u32(UnOp::Clz, a), Value::U32(a.leading_zeros()));
    }

    #[test]
    fn prop_unop_ctz_u32(a in any::<u32>()) {
        prop_assert_eq!(eval_unop_u32(UnOp::Ctz, a), Value::U32(a.trailing_zeros()));
    }

    #[test]
    fn prop_unop_reverse_bits_u32(a in any::<u32>()) {
        prop_assert_eq!(eval_unop_u32(UnOp::ReverseBits, a), Value::U32(a.reverse_bits()));
    }
}

// ---------------------------------------------------------------------------
// UnOp – i32
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn prop_unop_negate_i32(a in any::<i32>()) {
        prop_assert_eq!(eval_unop_i32(UnOp::Negate, a), Value::I32(0i32.wrapping_sub(a)));
    }

    #[test]
    fn prop_unop_bitnot_i32(a in any::<i32>()) {
        prop_assert_eq!(eval_unop_i32(UnOp::BitNot, a), Value::I32(!a));
    }

    #[test]
    fn prop_unop_logicalnot_i32(a in any::<i32>()) {
        prop_assert_eq!(eval_unop_i32(UnOp::LogicalNot, a), Value::Bool(a == 0));
    }

    #[test]
    fn prop_unop_popcount_i32(a in any::<i32>()) {
        prop_assert_eq!(eval_unop_i32(UnOp::Popcount, a), Value::I32(a.count_ones() as i32));
    }

    #[test]
    fn prop_unop_clz_i32(a in any::<i32>()) {
        prop_assert_eq!(eval_unop_i32(UnOp::Clz, a), Value::I32(a.leading_zeros() as i32));
    }

    #[test]
    fn prop_unop_ctz_i32(a in any::<i32>()) {
        prop_assert_eq!(eval_unop_i32(UnOp::Ctz, a), Value::I32(a.trailing_zeros() as i32));
    }

    #[test]
    fn prop_unop_reverse_bits_i32(a in any::<i32>()) {
        prop_assert_eq!(eval_unop_i32(UnOp::ReverseBits, a), Value::I32(a.reverse_bits()));
    }
}

// ---------------------------------------------------------------------------
// UnOp – f32
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn prop_unop_negate_f32(a in any::<f32>()) {
        prop_assert_eq!(eval_unop_f32(UnOp::Negate, a), expected_f32(-canonical_f32(a)));
    }

    #[test]
    fn prop_unop_abs_f32(a in any::<f32>()) {
        prop_assert_eq!(eval_unop_f32(UnOp::Abs, a), expected_f32(canonical_f32(a).abs()));
    }

    #[test]
    fn prop_unop_sqrt_f32(a in any::<f32>()) {
        prop_assert_eq!(eval_unop_f32(UnOp::Sqrt, a), expected_f32(libm::sqrtf(canonical_f32(a))));
    }

    #[test]
    fn prop_unop_sin_f32(a in any::<f32>()) {
        prop_assert_eq!(eval_unop_f32(UnOp::Sin, a), expected_f32(libm::sinf(canonical_f32(a))));
    }

    #[test]
    fn prop_unop_cos_f32(a in any::<f32>()) {
        prop_assert_eq!(eval_unop_f32(UnOp::Cos, a), expected_f32(libm::cosf(canonical_f32(a))));
    }

    #[test]
    fn prop_unop_floor_f32(a in any::<f32>()) {
        prop_assert_eq!(eval_unop_f32(UnOp::Floor, a), expected_f32(canonical_f32(a).floor()));
    }

    #[test]
    fn prop_unop_ceil_f32(a in any::<f32>()) {
        prop_assert_eq!(eval_unop_f32(UnOp::Ceil, a), expected_f32(canonical_f32(a).ceil()));
    }

    #[test]
    fn prop_unop_round_f32(a in any::<f32>()) {
        prop_assert_eq!(eval_unop_f32(UnOp::Round, a), expected_f32(canonical_f32(a).round()));
    }

    #[test]
    fn prop_unop_trunc_f32(a in any::<f32>()) {
        prop_assert_eq!(eval_unop_f32(UnOp::Trunc, a), expected_f32(canonical_f32(a).trunc()));
    }

    #[test]
    fn prop_unop_sign_f32(a in any::<f32>()) {
        let a = canonical_f32(a);
        let expected = if a.is_nan() {
            f64::from(f32::NAN)
        } else if a > 0.0 {
            1.0
        } else if a < 0.0 {
            -1.0
        } else {
            0.0
        };
        prop_assert_eq!(eval_unop_f32(UnOp::Sign, a), Value::Float(expected));
    }

    #[test]
    fn prop_unop_isnan_f32(a in any::<f32>()) {
        prop_assert_eq!(eval_unop_f32(UnOp::IsNan, a), Value::Bool(a.is_nan()));
    }

    #[test]
    fn prop_unop_isinf_f32(a in any::<f32>()) {
        prop_assert_eq!(eval_unop_f32(UnOp::IsInf, a), Value::Bool(a.is_infinite()));
    }

    #[test]
    fn prop_unop_isfinite_f32(a in any::<f32>()) {
        prop_assert_eq!(eval_unop_f32(UnOp::IsFinite, a), Value::Bool(a.is_finite()));
    }
}

// ---------------------------------------------------------------------------
// Cast – adversarial type-to-type pairs
// ---------------------------------------------------------------------------

