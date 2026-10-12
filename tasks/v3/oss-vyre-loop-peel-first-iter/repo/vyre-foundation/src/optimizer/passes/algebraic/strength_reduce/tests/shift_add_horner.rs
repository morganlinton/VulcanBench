//! Integration test crate for the containing Vyre package.

use super::*;

#[test]
fn shift_add_mul_by_3() {
    // x * 3 → (x<<2) - x under non-adjacent-form decomposition.
    let result = reduce_expr(&Expr::mul(Expr::var("x"), Expr::u32(3)));
    assert!(result.is_some(), "x*3 must decompose");
    let r = result.unwrap();
    assert!(
        matches!(&r, Expr::BinOp { op: BinOp::Sub, .. }),
        "must be sub: {r:?}"
    );
}

#[test]
fn shift_add_mul_by_5() {
    // x * 5 → (x<<2) + (x<<0)
    let result = reduce_expr(&Expr::mul(Expr::var("x"), Expr::u32(5)));
    assert!(result.is_some(), "x*5 must decompose");
    let r = result.unwrap();
    assert!(
        matches!(&r, Expr::BinOp { op: BinOp::Add, .. }),
        "must be add: {r:?}"
    );
}

#[test]
fn shift_add_mul_by_7() {
    // x * 7 → (x<<3) - (x<<0)
    let result = reduce_expr(&Expr::mul(Expr::var("x"), Expr::u32(7)));
    assert!(result.is_some(), "x*7 must decompose");
    let r = result.unwrap();
    assert!(
        matches!(&r, Expr::BinOp { op: BinOp::Sub, .. }),
        "must be sub: {r:?}"
    );
}

#[test]
fn shift_add_mul_by_9() {
    // x * 9 → (x<<3) + (x<<0)
    let result = reduce_expr(&Expr::mul(Expr::var("x"), Expr::u32(9)));
    assert!(result.is_some(), "x*9 must decompose");
}

#[test]
fn shift_add_mul_by_15() {
    // x * 15 → (x<<4) - (x<<0)
    let result = reduce_expr(&Expr::mul(Expr::var("x"), Expr::u32(15)));
    assert!(result.is_some(), "x*15 must decompose");
    let r = result.unwrap();
    assert!(
        matches!(&r, Expr::BinOp { op: BinOp::Sub, .. }),
        "must be sub: {r:?}"
    );
}

#[test]
fn shift_add_decomposes_prime_11_with_naf() {
    // 11 = 16 - 4 - 1, which was missed by the old two-term recognizer.
    let result = reduce_expr(&Expr::mul(Expr::var("x"), Expr::u32(11)));
    assert!(result.is_some(), "x*11 must use the bounded NAF chain");
    let r = result.unwrap();
    assert!(
        matches!(&r, Expr::BinOp { op: BinOp::Sub, .. }),
        "must be a subtractive chain: {r:?}"
    );
}

#[test]
fn shift_add_skips_expensive_operands_to_avoid_duplication() {
    let expensive = Expr::add(Expr::load("input", Expr::gid_x()), Expr::u32(1));
    let result = reduce_expr(&Expr::mul(expensive, Expr::u32(11)));
    assert!(
        result.is_none(),
        "bounded shift/add chains must not duplicate non-trivial operands"
    );
}

#[test]
fn integer_mul_zero_and_one_fold() {
    assert_eq!(
        reduce_expr(&Expr::mul(Expr::var("x"), Expr::u32(0))),
        Some(Expr::u32(0))
    );
    assert_eq!(
        reduce_expr(&Expr::mul(Expr::u32(1), Expr::var("x"))),
        Some(Expr::var("x"))
    );
}

#[test]
fn shift_add_does_not_fire_for_floats() {
    // Float multiply should not trigger shift-add
    let result = shift_add_decompose(&Expr::var("x"), &Expr::f32(3.0));
    assert!(result.is_none());
}

#[test]
fn horner_rewrites_expanded_u32_quadratic() {
    let x = Expr::var("x");
    let quadratic = Expr::mul(Expr::mul(Expr::u32(3), x.clone()), x.clone());
    let linear = Expr::mul(Expr::u32(5), x.clone());
    let expanded = Expr::add(Expr::add(quadratic, linear), Expr::u32(7));

    let result = reduce_expr(&expanded).expect("Fix: u32 quadratic must rewrite to Horner form");
    let expected = Expr::add(
        Expr::mul(
            Expr::add(Expr::mul(Expr::u32(3), Expr::var("x")), Expr::u32(5)),
            Expr::var("x"),
        ),
        Expr::u32(7),
    );
    assert_eq!(result, expected);
}

#[test]
fn horner_accepts_commuted_terms_and_implicit_coefficients() {
    let expanded = Expr::add(
        Expr::u32(9),
        Expr::add(Expr::var("x"), Expr::mul(Expr::var("x"), Expr::var("x"))),
    );

    let result = reduce_expr(&expanded).expect("Fix: x*x + x + c must rewrite");
    let expected = Expr::add(
        Expr::mul(
            Expr::add(Expr::mul(Expr::u32(1), Expr::var("x")), Expr::u32(1)),
            Expr::var("x"),
        ),
        Expr::u32(9),
    );
    assert_eq!(result, expected);
}

#[test]
fn horner_rejects_float_quadratic_to_preserve_rounding_contract() {
    let x = Expr::var("x");
    let quadratic = Expr::mul(Expr::mul(Expr::f32(3.0), x.clone()), x.clone());
    let linear = Expr::mul(Expr::f32(5.0), x);
    let expanded = Expr::add(Expr::add(quadratic, linear), Expr::f32(7.0));

    assert!(
        horner_polynomial_int(&expanded).is_none(),
        "float polynomial reassociation changes rounding and must stay untouched"
    );
}

// ── Shift fusion + shift-by-zero ─────────────────────────────────

// ── Degree-N Horner generalization ───────────────────────────────

/// Wrapping-u32 evaluator over the {Var, LitU32, Add, Mul} subset that the
/// Horner rewrite produces and consumes. Proves semantic equivalence, not
/// just expression shape.
fn eval_u32(expr: &Expr, x: u32) -> u32 {
    match expr {
        Expr::LitU32(v) => *v,
        Expr::LitI32(v) => *v as u32,
        Expr::Var(_) => x,
        Expr::BinOp {
            op: BinOp::Add,
            left,
            right,
        } => eval_u32(left, x).wrapping_add(eval_u32(right, x)),
        Expr::BinOp {
            op: BinOp::Mul,
            left,
            right,
        } => eval_u32(left, x).wrapping_mul(eval_u32(right, x)),
        Expr::UnOp {
            op: UnOp::Negate,
            operand,
        } => eval_u32(operand, x).wrapping_neg(),
        other => panic!("unexpected node in horner eval: {other:?}"),
    }
}

const HORNER_FUZZ_INPUTS: [u32; 9] = [0, 1, 2, 3, 7, 255, 65535, 0x8000_0000, u32::MAX];

#[test]
fn horner_cubic_is_semantically_equivalent_under_wrapping() {
    // 4x^3 + 3x^2 + 2x + 1
    let x = || Expr::var("x");
    let cubic = Expr::mul(Expr::mul(Expr::mul(Expr::u32(4), x()), x()), x());
    let quad = Expr::mul(Expr::mul(Expr::u32(3), x()), x());
    let lin = Expr::mul(Expr::u32(2), x());
    let expanded = Expr::add(Expr::add(Expr::add(cubic, quad), lin), Expr::u32(1));

    let reduced = reduce_expr(&expanded).expect("Fix: u32 cubic must rewrite to Horner form");
    let expected = Expr::add(
        Expr::mul(
            Expr::add(
                Expr::mul(
                    Expr::add(Expr::mul(Expr::u32(4), Expr::var("x")), Expr::u32(3)),
                    Expr::var("x"),
                ),
                Expr::u32(2),
            ),
            Expr::var("x"),
        ),
        Expr::u32(1),
    );
    assert_eq!(reduced, expected, "cubic Horner shape mismatch");
    for x in HORNER_FUZZ_INPUTS {
        assert_eq!(
            eval_u32(&expanded, x),
            eval_u32(&reduced, x),
            "cubic Horner diverged at x={x}"
        );
    }
}

#[test]
fn horner_quartic_is_semantically_equivalent_under_wrapping() {
    // 2x^4 + 9x^3 + x^2 + 5x + 6
    let x = || Expr::var("x");
    let q4 = Expr::mul(
        Expr::mul(Expr::mul(Expr::mul(Expr::u32(2), x()), x()), x()),
        x(),
    );
    let q3 = Expr::mul(Expr::mul(Expr::mul(Expr::u32(9), x()), x()), x());
    let q2 = Expr::mul(Expr::mul(x(), x()), Expr::u32(1));
    let q1 = Expr::mul(Expr::u32(5), x());
    let expanded = Expr::add(
        Expr::add(Expr::add(Expr::add(q4, q3), q2), q1),
        Expr::u32(6),
    );

    let reduced = reduce_expr(&expanded).expect("Fix: u32 quartic must rewrite to Horner form");
    for x in HORNER_FUZZ_INPUTS {
        assert_eq!(
            eval_u32(&expanded, x),
            eval_u32(&reduced, x),
            "quartic Horner diverged at x={x}"
        );
    }
}

#[test]
fn horner_sparse_cubic_emits_no_zero_add_churn() {
    // 7x^3 + 4  ->  ((7*x)*x)*x + 4   (no `+ 0` for the missing x^2/x terms)
    let x = || Expr::var("x");
    let expanded = Expr::add(
        Expr::mul(Expr::mul(Expr::mul(Expr::u32(7), x()), x()), x()),
        Expr::u32(4),
    );
    let reduced = reduce_expr(&expanded).expect("Fix: sparse u32 cubic must rewrite");

    // Exactly one Add (the constant) and three Muls; no LitU32(0) introduced.
    fn count(expr: &Expr, adds: &mut u32, muls: &mut u32, zeros: &mut u32) {
        match expr {
            Expr::LitU32(0) => *zeros += 1,
            Expr::BinOp { op, left, right } => {
                match op {
                    BinOp::Add => *adds += 1,
                    BinOp::Mul => *muls += 1,
                    _ => {}
                }
                count(left, adds, muls, zeros);
                count(right, adds, muls, zeros);
            }
            _ => {}
        }
    }
    let (mut adds, mut muls, mut zeros) = (0, 0, 0);
    count(&reduced, &mut adds, &mut muls, &mut zeros);
    assert_eq!(
        (adds, muls, zeros),
        (1, 3, 0),
        "sparse Horner churn: {reduced:?}"
    );

    for x in HORNER_FUZZ_INPUTS {
        assert_eq!(
            eval_u32(&expanded, x),
            eval_u32(&reduced, x),
            "sparse diverged at x={x}"
        );
    }
}

#[test]
fn horner_rejects_multivariable_polynomial() {
    // x*x + y  mixes two variables; folding would be unsound.
    let expanded = Expr::add(Expr::mul(Expr::var("x"), Expr::var("x")), Expr::var("y"));
    assert!(
        horner_polynomial_int(&expanded).is_none(),
        "multi-variable polynomial must not fold to single-variable Horner"
    );
}

#[test]
fn horner_rejects_duplicate_degree_terms() {
    // x*x + x*x + 1: two degree-2 terms are ambiguous; leave for const-fold.
    let x = || Expr::var("x");
    let expanded = Expr::add(
        Expr::add(Expr::mul(x(), x()), Expr::mul(x(), x())),
        Expr::u32(1),
    );
    assert!(
        horner_polynomial_int(&expanded).is_none(),
        "duplicate-degree terms must be merged before Horner folds"
    );
}

#[test]
fn horner_rejects_load_bearing_term() {
    // A term containing a memory load is not a pure monomial.
    let x = || Expr::var("x");
    let expanded = Expr::add(
        Expr::add(
            Expr::mul(Expr::load("buf", Expr::gid_x()), x()),
            Expr::mul(x(), x()),
        ),
        Expr::u32(1),
    );
    assert!(
        horner_polynomial_int(&expanded).is_none(),
        "non-monomial (load-bearing) term must reject"
    );
}

#[test]
fn horner_leaves_linear_polynomials_untouched() {
    // 3x + 2 is degree 1; nothing to fold.
    let expanded = Expr::add(Expr::mul(Expr::u32(3), Expr::var("x")), Expr::u32(2));
    assert!(
        horner_polynomial_int(&expanded).is_none(),
        "linear polynomial has no Horner benefit"
    );
}

#[test]
fn horner_rejects_literal_free_polynomial_to_keep_operand_type() {
    // x*x + x has no explicit u32 literal, so the variable's integer type is
    // unknown here. Folding would inject u32 coefficient literals that could
    // mistype an i32 operand, so the rewrite must decline.
    let x = || Expr::var("x");
    let expanded = Expr::add(Expr::mul(x(), x()), x());
    assert!(
        horner_polynomial_int(&expanded).is_none(),
        "literal-free polynomial must not fold (operand type unproven)"
    );
}

#[test]
fn horner_signed_cubic_is_semantically_equivalent_under_wrapping() {
    // -2x^3 + 5x^2 - 3x + 7 with i32 (two's-complement wrapping) coefficients.
    let x = || Expr::var("x");
    let c3 = Expr::mul(Expr::mul(Expr::mul(Expr::i32(-2), x()), x()), x());
    let c2 = Expr::mul(Expr::mul(Expr::i32(5), x()), x());
    let c1 = Expr::mul(Expr::i32(-3), x());
    let expanded = Expr::add(Expr::add(Expr::add(c3, c2), c1), Expr::i32(7));

    let reduced = reduce_expr(&expanded).expect("Fix: i32 cubic must rewrite to Horner form");
    // Coefficients must stay in the i32 domain, never u32.
    fn assert_no_u32_lit(expr: &Expr) {
        match expr {
            Expr::LitU32(_) => panic!("i32 Horner must not emit u32 literals: {expr:?}"),
            Expr::BinOp { left, right, .. } => {
                assert_no_u32_lit(left);
                assert_no_u32_lit(right);
            }
            _ => {}
        }
    }
    assert_no_u32_lit(&reduced);

    for x in HORNER_FUZZ_INPUTS {
        assert_eq!(
            eval_u32(&expanded, x),
            eval_u32(&reduced, x),
            "signed cubic Horner diverged at x bits={x:#010x}"
        );
    }
}

#[test]
fn horner_rejects_mixed_integer_domains() {
    // 2*x*x mixed with an i32 constant: the operand domain is contradictory.
    let x = || Expr::var("x");
    let expanded = Expr::add(Expr::mul(Expr::mul(Expr::u32(2), x()), x()), Expr::i32(3));
    assert!(
        horner_polynomial_int(&expanded).is_none(),
        "mixing u32 and i32 literals must not fold to a single domain"
    );
}

// ── Negative signed-constant multiply ────────────────────────────

#[test]
fn mul_by_negative_constant_factors_out_negate() {
    // x * -5 → Negate(x * 5); the inner product reduces on a later pass.
    let reduced = reduce_expr(&Expr::mul(Expr::var("x"), Expr::i32(-5)))
        .expect("Fix: x * -5 must factor out a negate");
    let expected = Expr::negate(Expr::mul(Expr::var("x"), Expr::i32(5)));
    assert_eq!(reduced, expected);
    // Both forms agree across adversarial i32 bit patterns.
    for bits in HORNER_FUZZ_INPUTS {
        assert_eq!(
            eval_u32(&Expr::mul(Expr::var("x"), Expr::i32(-5)), bits),
            eval_u32(&reduced, bits),
            "negate-factoring diverged at x bits={bits:#010x}"
        );
    }
}

#[test]
fn mul_by_negative_constant_handles_constant_on_left() {
    let reduced = reduce_expr(&Expr::mul(Expr::i32(-8), Expr::var("x")))
        .expect("Fix: -8 * x must factor out a negate");
    assert_eq!(
        reduced,
        Expr::negate(Expr::mul(Expr::var("x"), Expr::i32(8)))
    );
}

#[test]
fn mul_by_negative_one_left_to_const_fold() {
    // -1 is owned by const-fold (x * -1 → Negate(x)); strength-reduce declines.
    assert!(
        reduce_expr(&Expr::mul(Expr::var("x"), Expr::i32(-1))).is_none(),
        "x * -1 must be left for const-fold's negate identity"
    );
}

#[test]
fn mul_by_i32_min_is_not_negated() {
    // i32::MIN cannot be negated without overflow, so the rule must decline.
    assert!(
        reduce_expr(&Expr::mul(Expr::var("x"), Expr::i32(i32::MIN))).is_none(),
        "x * i32::MIN must not factor a negate (un-negatable)"
    );
}
