//! Subnormal f32 flushing determinism contract.
//!
//! The reference interpreter canonicalizes subnormal f32 values to signed
//! zero before and after every operation.  GPU backends may or may not
//! flush subnormals depending on hardware; these tests assert the exact
//! bit patterns the reference interpreter guarantees so that parity
//! comparisons have a deterministic ground truth.
//!
//! Every assertion uses `to_bits()` rather than approximate float equality.

use vyre::ir::{BinOp, Expr, UnOp};
use vyre_reference::execution::expr as eval_expr;
use vyre_reference::value::Value;
use vyre_reference::workgroup::{Invocation, InvocationIds, Memory};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn empty_program() -> vyre::ir::Program {
    vyre::ir::Program::wrapped(Vec::new(), [1, 1, 1], Vec::new())
}

fn zero_invocation(program: &vyre::ir::Program) -> Invocation<'_> {
    Invocation::new(InvocationIds::ZERO, program.entry())
}

fn eval_expr_value(expr: &Expr) -> Value {
    let program = empty_program();
    eval_expr::eval(
        expr,
        &mut zero_invocation(&program),
        &mut Memory::empty(),
        &program,
    )
    .expect("Fix: reference evaluator must evaluate generated expression")
}

fn eval_binop_f32(op: BinOp, a: f32, b: f32) -> Value {
    let expr = Expr::BinOp {
        op,
        left: Box::new(Expr::f32(a)),
        right: Box::new(Expr::f32(b)),
    };
    eval_expr_value(&expr)
}

fn eval_unop_f32(op: UnOp, a: f32) -> Value {
    let expr = Expr::UnOp {
        op,
        operand: Box::new(Expr::f32(a)),
    };
    eval_expr_value(&expr)
}

fn float_bits(value: Value) -> u32 {
    match value {
        Value::Float(v) => (v as f32).to_bits(),
        other => panic!("expected float value, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// 1. canonical_f32() direct tests  -  every subnormal bit pattern
// ---------------------------------------------------------------------------

#[test]
fn canonical_f32_flushes_all_positive_subnormals_to_positive_zero() {
    for bits in 0x0000_0001..=0x007F_FFFF {
        let input = f32::from_bits(bits);
        let output = vyre_reference::ieee754::canonical_f32(input);
        assert_eq!(
            output.to_bits(),
            0x0000_0000,
            "canonical_f32(0x{bits:08x}) must flush to +0.0, got 0x{:08x}",
            output.to_bits()
        );
    }
}

#[test]
fn canonical_f32_flushes_all_negative_subnormals_to_negative_zero() {
    for bits in 0x8000_0001..=0x807F_FFFF {
        let input = f32::from_bits(bits);
        let output = vyre_reference::ieee754::canonical_f32(input);
        assert_eq!(
            output.to_bits(),
            0x8000_0000,
            "canonical_f32(0x{bits:08x}) must flush to -0.0, got 0x{:08x}",
            output.to_bits()
        );
    }
}

// ---------------------------------------------------------------------------
// 2. binop_f32 with subnormal operands
// ---------------------------------------------------------------------------

#[test]
fn binop_add_subnormal_operands_produce_canonical_results() {
    let pos_sub = f32::from_bits(0x0000_0001);
    let neg_sub = f32::from_bits(0x8000_0001);

    // pos_sub + pos_sub → canonicalized to 0.0 + 0.0 = +0.0
    assert_eq!(
        float_bits(eval_binop_f32(BinOp::Add, pos_sub, pos_sub)),
        0x0000_0000
    );
    // neg_sub + neg_sub → canonicalized to -0.0 + -0.0 = -0.0
    assert_eq!(
        float_bits(eval_binop_f32(BinOp::Add, neg_sub, neg_sub)),
        0x8000_0000
    );
    // pos_sub + 0.0 → 0.0 + 0.0 = +0.0
    assert_eq!(
        float_bits(eval_binop_f32(BinOp::Add, pos_sub, 0.0f32)),
        0x0000_0000
    );
    // neg_sub + 0.0 → -0.0 + 0.0 = +0.0 (IEEE-754: -0.0 + 0.0 = +0.0)
    assert_eq!(
        float_bits(eval_binop_f32(BinOp::Add, neg_sub, 0.0f32)),
        0x0000_0000
    );
}

#[test]
fn binop_mul_subnormal_operands_produce_canonical_results() {
    let pos_sub = f32::from_bits(0x0000_0001);
    let neg_sub = f32::from_bits(0x8000_0001);

    // pos_sub * 1.0 → 0.0 * 1.0 = +0.0
    assert_eq!(
        float_bits(eval_binop_f32(BinOp::Mul, pos_sub, 1.0f32)),
        0x0000_0000
    );
    // neg_sub * 1.0 → -0.0 * 1.0 = -0.0
    assert_eq!(
        float_bits(eval_binop_f32(BinOp::Mul, neg_sub, 1.0f32)),
        0x8000_0000
    );
    // neg_sub * -1.0 → -0.0 * -1.0 = +0.0
    assert_eq!(
        float_bits(eval_binop_f32(BinOp::Mul, neg_sub, -1.0f32)),
        0x0000_0000
    );
}

#[test]
fn binop_div_subnormal_operands_produce_canonical_results() {
    let pos_sub = f32::from_bits(0x0000_0001);
    let neg_sub = f32::from_bits(0x8000_0001);

    // pos_sub / 2.0 → 0.0 / 2.0 = +0.0
    assert_eq!(
        float_bits(eval_binop_f32(BinOp::Div, pos_sub, 2.0f32)),
        0x0000_0000
    );
    // neg_sub / 2.0 → -0.0 / 2.0 = -0.0
    assert_eq!(
        float_bits(eval_binop_f32(BinOp::Div, neg_sub, 2.0f32)),
        0x8000_0000
    );
    // 1.0 / pos_sub → 1.0 / 0.0 = +inf (after canonicalization of operand)
    assert_eq!(
        float_bits(eval_binop_f32(BinOp::Div, 1.0f32, pos_sub)),
        f32::INFINITY.to_bits()
    );
    // 1.0 / neg_sub → 1.0 / -0.0 = -inf
    assert_eq!(
        float_bits(eval_binop_f32(BinOp::Div, 1.0f32, neg_sub)),
        f32::NEG_INFINITY.to_bits()
    );
}

// ---------------------------------------------------------------------------
// 3. unop_f32 with subnormal inputs
// ---------------------------------------------------------------------------

#[test]
fn unop_sqrt_subnormal_input_produces_canonical_result() {
    let pos_sub = f32::from_bits(0x0000_0001);
    let neg_sub = f32::from_bits(0x8000_0001);

    // sqrt(pos_sub) → sqrt(0.0) = +0.0
    assert_eq!(float_bits(eval_unop_f32(UnOp::Sqrt, pos_sub)), 0x0000_0000);
    // sqrt(neg_sub) → sqrt(-0.0) = -0.0
    assert_eq!(float_bits(eval_unop_f32(UnOp::Sqrt, neg_sub)), 0x8000_0000);
}

#[test]
fn unop_sin_subnormal_input_produces_canonical_result() {
    let pos_sub = f32::from_bits(0x0000_0001);
    let neg_sub = f32::from_bits(0x8000_0001);

    // sin(pos_sub) → sin(0.0) = +0.0
    assert_eq!(float_bits(eval_unop_f32(UnOp::Sin, pos_sub)), 0x0000_0000);
    // sin(neg_sub) → sin(-0.0) = -0.0
    assert_eq!(float_bits(eval_unop_f32(UnOp::Sin, neg_sub)), 0x8000_0000);
}

#[test]
fn unop_cos_subnormal_input_produces_canonical_result() {
    let pos_sub = f32::from_bits(0x0000_0001);
    let neg_sub = f32::from_bits(0x8000_0001);

    // cos(pos_sub) → cos(0.0) = 1.0
    assert_eq!(
        float_bits(eval_unop_f32(UnOp::Cos, pos_sub)),
        1.0f32.to_bits()
    );
    // cos(neg_sub) → cos(-0.0) = 1.0
    assert_eq!(
        float_bits(eval_unop_f32(UnOp::Cos, neg_sub)),
        1.0f32.to_bits()
    );
}

#[test]
fn unop_exp_subnormal_input_produces_canonical_result() {
    let pos_sub = f32::from_bits(0x0000_0001);
    let neg_sub = f32::from_bits(0x8000_0001);

    // exp(pos_sub) → exp(0.0) = 1.0
    assert_eq!(
        float_bits(eval_unop_f32(UnOp::Exp, pos_sub)),
        1.0f32.to_bits()
    );
    // exp(neg_sub) → exp(-0.0) = 1.0
    assert_eq!(
        float_bits(eval_unop_f32(UnOp::Exp, neg_sub)),
        1.0f32.to_bits()
    );
}

// ---------------------------------------------------------------------------
// 4. Boundary between normal and subnormal
// ---------------------------------------------------------------------------

#[test]
fn boundary_normal_subnormal_transition() {
    // f32::MIN_POSITIVE is the smallest normal number (0x0080_0000)
    let min_normal = f32::MIN_POSITIVE;
    let just_below_normal = f32::from_bits(0x007F_FFFF); // largest subnormal

    // canonical_f32 must not touch normals
    assert_eq!(
        vyre_reference::ieee754::canonical_f32(min_normal).to_bits(),
        min_normal.to_bits()
    );
    // canonical_f32 must flush the largest subnormal
    assert_eq!(
        vyre_reference::ieee754::canonical_f32(just_below_normal).to_bits(),
        0x0000_0000
    );

    // min_normal / 2.0 produces a subnormal result, which is then canonicalized
    assert_eq!(
        float_bits(eval_binop_f32(BinOp::Div, min_normal, 2.0f32)),
        0x0000_0000,
        "MIN_POSITIVE / 2.0 must canonicalize to +0.0"
    );
    // -min_normal / 2.0 produces negative subnormal result
    assert_eq!(
        float_bits(eval_binop_f32(BinOp::Div, -min_normal, 2.0f32)),
        0x8000_0000,
        "-MIN_POSITIVE / 2.0 must canonicalize to -0.0"
    );
}

// ---------------------------------------------------------------------------
// 5. -0.0f and +0.0f preservation through canonicalization
// ---------------------------------------------------------------------------

#[test]
fn signed_zero_preserved_through_canonicalization() {
    assert_eq!(
        vyre_reference::ieee754::canonical_f32(0.0f32).to_bits(),
        0x0000_0000
    );
    assert_eq!(
        vyre_reference::ieee754::canonical_f32(-0.0f32).to_bits(),
        0x8000_0000
    );
}

#[test]
fn signed_zero_preserved_through_binops() {
    // +0.0 + -0.0 = +0.0 (IEEE-754)
    assert_eq!(
        float_bits(eval_binop_f32(BinOp::Add, 0.0f32, -0.0f32)),
        0x0000_0000
    );
    // -0.0 + -0.0 = -0.0
    assert_eq!(
        float_bits(eval_binop_f32(BinOp::Add, -0.0f32, -0.0f32)),
        0x8000_0000
    );
    // +0.0 * -1.0 = -0.0
    assert_eq!(
        float_bits(eval_binop_f32(BinOp::Mul, 0.0f32, -1.0f32)),
        0x8000_0000
    );
    // -0.0 * -1.0 = +0.0
    assert_eq!(
        float_bits(eval_binop_f32(BinOp::Mul, -0.0f32, -1.0f32)),
        0x0000_0000
    );
    // +0.0 / 1.0 = +0.0
    assert_eq!(
        float_bits(eval_binop_f32(BinOp::Div, 0.0f32, 1.0f32)),
        0x0000_0000
    );
    // -0.0 / 1.0 = -0.0
    assert_eq!(
        float_bits(eval_binop_f32(BinOp::Div, -0.0f32, 1.0f32)),
        0x8000_0000
    );
}

#[test]
fn signed_zero_preserved_through_unops() {
    // sqrt(-0.0) = -0.0
    assert_eq!(float_bits(eval_unop_f32(UnOp::Sqrt, -0.0f32)), 0x8000_0000);
    // sin(-0.0) = -0.0
    assert_eq!(float_bits(eval_unop_f32(UnOp::Sin, -0.0f32)), 0x8000_0000);
    // cos(-0.0) = 1.0
    assert_eq!(
        float_bits(eval_unop_f32(UnOp::Cos, -0.0f32)),
        1.0f32.to_bits()
    );
    // exp(-0.0) = 1.0
    assert_eq!(
        float_bits(eval_unop_f32(UnOp::Exp, -0.0f32)),
        1.0f32.to_bits()
    );
    // negate(-0.0) = +0.0
    assert_eq!(
        float_bits(eval_unop_f32(UnOp::Negate, -0.0f32)),
        0x0000_0000
    );
    // abs(-0.0) = +0.0
    assert_eq!(float_bits(eval_unop_f32(UnOp::Abs, -0.0f32)), 0x0000_0000);
}

// ---------------------------------------------------------------------------
// Extra: normals and infinities are never altered by canonical_f32; NaNs are canonical.
// ---------------------------------------------------------------------------

#[test]
fn canonical_f32_preserves_normals_and_infinities() {
    let cases = [
        1.0f32,
        -1.0f32,
        f32::MAX,
        f32::MIN,
        -f32::MAX,
        f32::MIN_POSITIVE,
        -f32::MIN_POSITIVE,
        f32::INFINITY,
        f32::NEG_INFINITY,
    ];
    for &input in &cases {
        let output = vyre_reference::ieee754::canonical_f32(input);
        assert_eq!(
            output.to_bits(),
            input.to_bits(),
            "canonical_f32 must preserve normal/special values unchanged: input=0x{:08x}",
            input.to_bits()
        );
    }
}

#[test]
fn canonical_f32_canonicalizes_all_nan_payloads() {
    for input in [
        f32::NAN,
        f32::from_bits(0x7FC0_0000), // quiet NaN
        f32::from_bits(0xFFC0_0000), // negative quiet NaN
        f32::from_bits(0x7F80_0001), // signalling NaN
    ] {
        let output = vyre_reference::ieee754::canonical_f32(input);
        assert_eq!(
            output.to_bits(),
            0x7FC0_0000,
            "canonical_f32 must canonicalize every NaN payload: input=0x{:08x}",
            input.to_bits()
        );
    }
}
