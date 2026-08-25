use crate::ir::{BinOp, BufferDecl, DataType, Expr, Node, Program};
use crate::validate::validate;

const ABS_DIFF_I32_MESSAGE: &str =
    "can overflow (i32::MIN - i32::MAX invokes target-text signed-integer UB). Fix: cast operands to U32 before AbsDiff, or rewrite as an explicit branch.";

const NEGATE_I32_MESSAGE: &str =
    "Fix: use `0 - x` for wrapping i32 negation, cast to U32 before Negate, or guard with Select(i32::MIN, 0, -x).";

const SATURATING_MESSAGE: &str =
    "legal set is only U32 in the current lowering. Fix: cast both operands to U32, or clamp explicitly for I32/F32.";

const INTEGER_64_MESSAGE: &str =
    "64-bit integer arithmetic is outside vyre-foundation's cross-backend arithmetic contract. Fix: express the operation as a U32 pair with explicit carry/borrow, or use a backend-specific op whose schema declares native 64-bit arithmetic.";

fn assert_rejected(expr: Expr, output_ty: DataType, expected: &str) {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, output_ty)],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), expr)],
    );
    let errors = validate(&program);
    assert!(
        errors.iter().any(|error| error.message.contains(expected)),
        "expected critical rejection: {expected}"
    );
}

#[test]
fn val_001_abs_diff_on_i32_is_rejected() {
    let expr = Expr::abs_diff(Expr::i32(i32::MIN), Expr::i32(42));
    assert_rejected(expr, DataType::I32, ABS_DIFF_I32_MESSAGE);
}

#[test]
fn val_002_negate_on_i32_is_rejected() {
    let expr = Expr::negate(Expr::i32(i32::MIN));
    assert_rejected(expr, DataType::I32, NEGATE_I32_MESSAGE);
}

#[test]
fn val_003_saturating_i32_and_f32_are_rejected() {
    let i32_expr = Expr::BinOp {
        op: BinOp::SaturatingAdd,
        left: Box::new(Expr::i32(1)),
        right: Box::new(Expr::i32(2)),
    };
    assert_rejected(i32_expr, DataType::I32, SATURATING_MESSAGE);

    let f32_expr = Expr::BinOp {
        op: BinOp::SaturatingMul,
        left: Box::new(Expr::f32(1.0)),
        right: Box::new(Expr::f32(2.0)),
    };
    assert_rejected(f32_expr, DataType::F32, SATURATING_MESSAGE);
}

#[test]
fn val_004_arithmetic_on_i64_u64_is_rejected() {
    let i64_expr = Expr::BinOp {
        op: BinOp::Add,
        left: Box::new(Expr::i64(4)),
        right: Box::new(Expr::i64(2)),
    };
    assert_rejected(i64_expr, DataType::I64, INTEGER_64_MESSAGE);

    let u64_expr = Expr::BinOp {
        op: BinOp::Mul,
        left: Box::new(Expr::u64(4)),
        right: Box::new(Expr::u64(2)),
    };
    assert_rejected(u64_expr, DataType::U64, INTEGER_64_MESSAGE);
}
