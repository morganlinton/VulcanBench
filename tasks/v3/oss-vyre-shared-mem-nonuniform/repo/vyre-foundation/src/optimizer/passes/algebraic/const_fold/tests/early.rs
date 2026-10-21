//! Const-fold tests  -  split per audit cleanup A13 (2026-04-30) so no
//! single test file exceeds the 1000-LOC hygiene cap.

use super::super::*;
use crate::ir::{BinOp, BufferDecl, DataType, Expr, Node, UnOp};
use crate::optimizer::{PassScheduler, ProgramPassKind};

#[test]
fn optimizer_const_fold_adds_literals() {
    let program = crate::optimizer::passes::cleanup::region_inline_engine::run(Program::wrapped(
        vec![BufferDecl::read_write("out", 0, DataType::U32)],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::add(Expr::u32(3), Expr::u32(4)),
        )],
    ));

    let optimized = PassScheduler::with_passes(vec![ProgramPassKind::new(ConstFold)])
        .run(program)
        .expect("Fix: const fold should converge");

    let body = crate::test_util::region_body(&optimized);
    assert!(matches!(
        &body[0],
        Node::Store {
            value: Expr::LitU32(7),
            ..
        }
    ));
}

#[test]
fn optimizer_const_fold_is_idempotent() {
    let program = Program::wrapped(
        Vec::new(),
        [1, 1, 1],
        vec![Node::let_bind(
            "x",
            Expr::bitxor(Expr::u32(0b1010), Expr::u32(0b1100)),
        )],
    );

    let scheduler = PassScheduler::with_passes(vec![ProgramPassKind::new(ConstFold)]);
    let once = scheduler
        .run(program)
        .expect("Fix: first run should converge");
    let twice = scheduler
        .run(once.clone())
        .expect("Fix: second run should converge");
    assert_eq!(once, twice);
}

#[test]
fn const_fold_folds_float_addition() {
    let program = crate::optimizer::passes::cleanup::region_inline_engine::run(Program::wrapped(
        vec![BufferDecl::read_write("out", 0, DataType::U32)],
        [1, 1, 1],
        vec![Node::let_bind(
            "x",
            Expr::add(Expr::f32(2.0), Expr::f32(3.0)),
        )],
    ));

    let optimized = PassScheduler::with_passes(vec![ProgramPassKind::new(ConstFold)])
        .run(program)
        .expect("Fix: float const fold should converge");

    let body = crate::test_util::region_body(&optimized);
    assert!(
        matches!(&body[0], Node::Let { value: Expr::LitF32(v), .. } if *v == 5.0),
        "2.0 + 3.0 should fold to 5.0"
    );
}

#[test]
fn const_fold_folds_fma_literals() {
    let program = crate::optimizer::passes::cleanup::region_inline_engine::run(Program::wrapped(
        vec![BufferDecl::read_write("out", 0, DataType::U32)],
        [1, 1, 1],
        vec![Node::let_bind(
            "x",
            Expr::Fma {
                a: Box::new(Expr::f32(2.0)),
                b: Box::new(Expr::f32(3.0)),
                c: Box::new(Expr::f32(1.0)),
            },
        )],
    ));

    let optimized = PassScheduler::with_passes(vec![ProgramPassKind::new(ConstFold)])
        .run(program)
        .expect("Fix: fma const fold should converge");

    let body = crate::test_util::region_body(&optimized);
    assert!(
        matches!(&body[0], Node::Let { value: Expr::LitF32(v), .. } if *v == 7.0),
        "fma(2.0, 3.0, 1.0) should fold to 7.0"
    );
}

#[test]
fn const_fold_skips_fma_with_nan() {
    let fma_expr = Expr::Fma {
        a: Box::new(Expr::f32(f32::NAN)),
        b: Box::new(Expr::f32(3.0)),
        c: Box::new(Expr::f32(1.0)),
    };
    assert!(fold_expr(&fma_expr).is_none(), "NaN fma must not fold");
}

#[test]
fn fma_zero_multiplier_does_not_hide_runtime_nan_or_inf() {
    let fma_expr = Expr::Fma {
        a: Box::new(Expr::f32(0.0)),
        b: Box::new(Expr::var("possibly_nan")),
        c: Box::new(Expr::f32(1.0)),
    };
    assert!(
        fold_expr(&fma_expr).is_none(),
        "fma(0, x, c) must not fold unless x is a finite literal"
    );
}

#[test]
fn fma_zero_addend_does_not_change_signed_zero_contract() {
    let fma_expr = Expr::Fma {
        a: Box::new(Expr::var("a")),
        b: Box::new(Expr::var("b")),
        c: Box::new(Expr::f32(0.0)),
    };
    assert!(
        fold_expr(&fma_expr).is_none(),
        "fma(a, b, +0.0) must not fold to mul because signed-zero rounding can differ"
    );
}

#[test]
fn cast_fold_u32_to_f32() {
    let expr = Expr::cast(DataType::F32, Expr::u32(10));
    let folded =
        fold_expr(&expr).expect("Fix: should fold; restore this invariant before continuing.");
    assert!(matches!(folded, Expr::LitF32(v) if v == 10.0));
}

#[test]
fn cast_fold_f32_to_u32() {
    let expr = Expr::cast(DataType::U32, Expr::f32(42.7));
    let folded =
        fold_expr(&expr).expect("Fix: should fold; restore this invariant before continuing.");
    assert!(matches!(folded, Expr::LitU32(42)));
}

#[test]
fn cast_fold_bool_to_u32() {
    let expr = Expr::cast(DataType::U32, Expr::bool(true));
    let folded =
        fold_expr(&expr).expect("Fix: should fold; restore this invariant before continuing.");
    assert!(matches!(folded, Expr::LitU32(1)));
}

#[test]
fn cast_fold_identity_is_noop() {
    let expr = Expr::cast(DataType::U32, Expr::u32(77));
    let folded =
        fold_expr(&expr).expect("Fix: should fold; restore this invariant before continuing.");
    assert!(matches!(folded, Expr::LitU32(77)));
}

#[test]
fn cast_fold_nan_does_not_fold() {
    let expr = Expr::cast(DataType::U32, Expr::f32(f32::NAN));
    assert!(fold_expr(&expr).is_none(), "NaN cast must not fold");
}

#[test]
fn const_fold_uses_shared_literal_eval_for_nested_trees() {
    let expr = Expr::mul(
        Expr::add(Expr::u32(2), Expr::u32(3)),
        Expr::sub(Expr::u32(11), Expr::u32(4)),
    );
    let shared = crate::ir::eval::fold_literal_tree(&expr)
        .expect("Fix: literal-only tree should fold through shared evaluator")
        .into_owned();
    assert_eq!(fold_expr(&expr), Some(shared));
}

#[test]
fn const_fold_bool_xor_uses_shared_literal_eval() {
    let expr = Expr::BinOp {
        op: BinOp::BitXor,
        left: Box::new(Expr::bool(true)),
        right: Box::new(Expr::bool(false)),
    };
    assert_eq!(fold_expr(&expr), Some(Expr::bool(true)));
}

#[test]
fn const_fold_does_not_fabricate_values_for_undefined_signed_division() {
    // Signed division by zero and the i32::MIN / -1 overflow are UNDEFINED on
    // the target backends, so const-fold must NOT fabricate a value for them:
    //   - the reference oracle ERRORS on both (vyre-reference div_i32 / rem_i32:
    //     "undefined backend semantics"), and
    //   - the Naga emitter lowers signed division to a raw SDiv with no
    //     divisor-zero guard (only UNSIGNED div-by-zero is guarded, to u32::MAX).
    // Folding `i32 x / 0` to 0 (or i32::MIN / -1 to i32::MIN) would make the
    // optimized program produce a value the unoptimized program never produces.
    // The folder must decline, mirroring the f32 div-by-zero path, leaving the
    // raw division for the divisor guard / oracle to handle.
    let undefined = &[
        Expr::div(Expr::i32(7), Expr::i32(0)),
        Expr::rem(Expr::i32(7), Expr::i32(0)),
        Expr::div(Expr::i32(i32::MIN), Expr::i32(-1)),
        Expr::rem(Expr::i32(i32::MIN), Expr::i32(-1)),
    ];
    for expr in undefined {
        assert_eq!(
            fold_expr(expr),
            None,
            "undefined signed division/remainder must NOT be const-folded: {expr:?}"
        );
    }

    // Sanity: the DEFINED signed cases still fold via wrapping arithmetic.
    assert_eq!(
        fold_expr(&Expr::div(Expr::i32(-7), Expr::i32(3))),
        Some(Expr::i32(-2)),
        "defined signed division must still fold"
    );
    assert_eq!(
        fold_expr(&Expr::rem(Expr::i32(-7), Expr::i32(3))),
        Some(Expr::i32(-1)),
        "defined signed remainder must still fold"
    );
}

#[test]
fn const_fold_float_subnormal_results_match_reference_contract() {
    let folded = fold_expr(&Expr::div(Expr::f32(f32::MIN_POSITIVE), Expr::f32(2.0)))
        .expect("Fix: finite non-zero f32 division should fold");
    assert!(
        matches!(folded, Expr::LitF32(value) if value.to_bits() == 0.0f32.to_bits()),
        "subnormal f32 fold results must flush to canonical +0.0"
    );
}

#[test]
fn double_negation_eliminated() {
    let inner = Expr::var("x");
    let double_neg = Expr::UnOp {
        op: UnOp::Negate,
        operand: Box::new(Expr::UnOp {
            op: UnOp::Negate,
            operand: Box::new(inner.clone()),
        }),
    };
    let folded = fold_expr(&double_neg)
        .expect("Fix: should simplify; restore this invariant before continuing.");
    assert_eq!(folded, inner);
}

#[test]
fn abs_neg_simplifies() {
    let x = Expr::var("x");
    let abs_neg = Expr::UnOp {
        op: UnOp::Abs,
        operand: Box::new(Expr::UnOp {
            op: UnOp::Negate,
            operand: Box::new(x.clone()),
        }),
    };
    let folded = fold_expr(&abs_neg)
        .expect("Fix: should simplify; restore this invariant before continuing.");
    let expected = Expr::UnOp {
        op: UnOp::Abs,
        operand: Box::new(x),
    };
    assert_eq!(folded, expected);
}

// ---- Algebraic identity tests ----

#[test]
fn add_zero_identity() {
    let x = Expr::var("x");
    // x + 0 → x
    let expr = Expr::add(x.clone(), Expr::u32(0));
    assert_eq!(fold_expr(&expr), Some(x.clone()));
    // 0 + x → x
    let expr = Expr::add(Expr::u32(0), x.clone());
    assert_eq!(fold_expr(&expr), Some(x));
}

#[test]
fn sub_zero_identity() {
    let x = Expr::var("x");
    let expr = Expr::sub(x.clone(), Expr::u32(0));
    assert_eq!(fold_expr(&expr), Some(x));
}

#[test]
fn mul_one_identity() {
    let x = Expr::var("x");
    // x * 1 → x
    let expr = Expr::mul(x.clone(), Expr::u32(1));
    assert_eq!(fold_expr(&expr), Some(x.clone()));
    // 1 * x → x
    let expr = Expr::mul(Expr::u32(1), x.clone());
    assert_eq!(fold_expr(&expr), Some(x));
}

#[test]
fn mul_zero_annihilator_int() {
    let x = Expr::var("x");
    // x * 0 → 0 (integer)
    let expr = Expr::mul(x.clone(), Expr::u32(0));
    assert_eq!(fold_expr(&expr), Some(Expr::u32(0)));
    // 0 * x → 0
    let expr = Expr::mul(Expr::u32(0), x);
    assert_eq!(fold_expr(&expr), Some(Expr::u32(0)));
}

#[test]
fn mul_zero_float_not_folded() {
    // Float 0*x might produce NaN if x is NaN  -  do not fold.
    let x = Expr::var("x");
    let expr = Expr::mul(x, Expr::f32(0.0));
    assert_eq!(fold_expr(&expr), None);
}

#[test]
fn div_one_identity() {
    let x = Expr::var("x");
    let expr = Expr::div(x.clone(), Expr::u32(1));
    assert_eq!(fold_expr(&expr), Some(x));
}

#[test]
fn bitand_zero_annihilator() {
    let x = Expr::var("x");
    let expr = Expr::bitand(x, Expr::u32(0));
    assert_eq!(fold_expr(&expr), Some(Expr::u32(0)));
}

#[test]
fn bitor_zero_identity() {
    let x = Expr::var("x");
    let expr = Expr::bitor(x.clone(), Expr::u32(0));
    assert_eq!(fold_expr(&expr), Some(x));
}

#[test]
fn bitxor_zero_identity() {
    let x = Expr::var("x");
    let expr = Expr::bitxor(x.clone(), Expr::u32(0));
    assert_eq!(fold_expr(&expr), Some(x));
}

#[test]
fn select_identical_branches() {
    let x = Expr::var("x");
    // Select(cond, x, x) → x
    let expr = Expr::Select {
        cond: Box::new(Expr::var("c")),
        true_val: Box::new(x.clone()),
        false_val: Box::new(x.clone()),
    };
    assert_eq!(fold_expr(&expr), Some(x));
}

// ---- FMA synthesis tests ----

#[test]
fn fma_synthesis_mul_plus_c() {
    // (a * b) + c → Fma(a, b, c)
    let a = Expr::var("a");
    let b = Expr::var("b");
    let c = Expr::f32(1.0);
    let expr = Expr::add(Expr::mul(a.clone(), b.clone()), c.clone());
    let result = fold_expr(&expr)
        .expect("Fix: should synthesize fma; restore this invariant before continuing.");
    assert_eq!(
        result,
        Expr::Fma {
            a: Box::new(a),
            b: Box::new(b),
            c: Box::new(c),
        }
    );
}

#[test]
fn fma_synthesis_c_plus_mul() {
    // c + (a * b) → Fma(a, b, c)
    let a = Expr::var("a");
    let b = Expr::var("b");
    let c = Expr::f32(2.5);
    let expr = Expr::add(c.clone(), Expr::mul(a.clone(), b.clone()));
    let result = fold_expr(&expr)
        .expect("Fix: should synthesize fma; restore this invariant before continuing.");
    assert_eq!(
        result,
        Expr::Fma {
            a: Box::new(a),
            b: Box::new(b),
            c: Box::new(c),
        }
    );
}

#[test]
fn fma_synthesis_mul_minus_c() {
    let a = Expr::var("a");
    let b = Expr::var("b");
    let c = Expr::f32(2.0);
    let result = fold_expr(&Expr::sub(Expr::mul(a.clone(), b.clone()), c.clone()))
        .expect("Fix: should synthesize fma for mul-minus-c");

    assert_eq!(result, Expr::fma(a, b, Expr::negate(c)));
}

#[test]
fn fma_synthesis_c_minus_mul_uses_negated_multiplicand() {
    let a = Expr::var("a");
    let b = Expr::var("b");
    let c = Expr::f32(2.0);
    let result = fold_expr(&Expr::sub(c.clone(), Expr::mul(a.clone(), b.clone())))
        .expect("Fix: should synthesize fma for c-minus-mul");

    assert_eq!(result, Expr::fma(Expr::negate(a), b, c));
}

#[test]
fn fma_synthesis_nested_mul_add_chain() {
    let a = Expr::var("a");
    let b = Expr::var("b");
    let c = Expr::var("c");
    let d = Expr::f32(4.0);
    let result = fold_expr(&Expr::add(
        Expr::mul(a.clone(), b.clone()),
        Expr::mul(c.clone(), d.clone()),
    ))
    .expect("Fix: should synthesize fma for mul-add chain with float evidence");

    assert_eq!(result, Expr::fma(a, b, Expr::mul(c, d)));
}

#[test]
fn fma_synthesis_does_not_fire_for_unknown_integer_shape() {
    let expr = Expr::add(Expr::mul(Expr::var("a"), Expr::var("b")), Expr::var("c"));
    assert_eq!(fold_expr(&expr), None);
}

// ---- Self-operand identity tests ----

#[test]
fn sub_self_is_zero() {
    let x = Expr::var("x");
    let expr = Expr::sub(x.clone(), x);
    assert_eq!(fold_expr(&expr), Some(Expr::u32(0)));
}

#[test]
fn xor_self_is_zero() {
    let x = Expr::var("x");
    let expr = Expr::bitxor(x.clone(), x);
    assert_eq!(fold_expr(&expr), Some(Expr::u32(0)));
}

#[test]
fn and_self_is_self() {
    let x = Expr::var("x");
    let expr = Expr::bitand(x.clone(), x.clone());
    assert_eq!(fold_expr(&expr), Some(x));
}

#[test]
fn or_self_is_self() {
    let x = Expr::var("x");
    let expr = Expr::bitor(x.clone(), x.clone());
    assert_eq!(fold_expr(&expr), Some(x));
}

#[test]
fn sub_self_complex_expr() {
    let ab = Expr::add(Expr::var("a"), Expr::var("b"));
    let expr = Expr::sub(ab.clone(), ab);
    assert_eq!(fold_expr(&expr), Some(Expr::u32(0)));
}
