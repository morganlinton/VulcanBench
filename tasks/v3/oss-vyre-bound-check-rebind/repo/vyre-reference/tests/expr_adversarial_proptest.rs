//! Integration test crate for the containing Vyre package.

#![allow(dead_code)]
//! Adversarial proptest coverage for every BinOp, UnOp, Cast, Atomic, Load,
//! BufLen, Store, Call, and Opaque expression variant (TEST-03 + TEST-04).
//!
//! Every property compares the reference interpreter against Rust-native
//! semantics or the documented error contract.  No stubs, no shortcuts.
use proptest::prelude::*;
use vyre_foundation::ir::{AtomicOp, BinOp, BufferDecl, DataType, Expr, Node, Program, UnOp};
use vyre_foundation::MemoryOrdering;
use vyre_reference::{
    execution::expr as eval_expr,
    execution::expr::Buffer,
    value::Value,
    workgroup::{Invocation, InvocationIds, Memory},
};
// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------
fn empty_program() -> Program {
    Program::wrapped(Vec::new(), [1, 1, 1], Vec::new())
}
fn zero_invocation(program: &Program) -> Invocation<'_> {
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
    .expect("Fix: flat evaluator must evaluate generated expression")
}
fn eval_binop_u32(op: BinOp, a: u32, b: u32) -> Value {
    let expr = Expr::BinOp {
        op,
        left: Box::new(Expr::u32(a)),
        right: Box::new(Expr::u32(b)),
    };
    eval_expr_value(&expr)
}
fn eval_binop_i32(op: BinOp, a: i32, b: i32) -> Value {
    let expr = Expr::BinOp {
        op,
        left: Box::new(Expr::i32(a)),
        right: Box::new(Expr::i32(b)),
    };
    eval_expr_value(&expr)
}
fn eval_binop_f32(op: BinOp, a: f32, b: f32) -> Value {
    let expr = Expr::BinOp {
        op,
        left: Box::new(Expr::f32(a)),
        right: Box::new(Expr::f32(b)),
    };
    eval_expr_value(&expr)
}
fn eval_unop_u32(op: UnOp, a: u32) -> Value {
    let expr = Expr::UnOp {
        op,
        operand: Box::new(Expr::u32(a)),
    };
    eval_expr_value(&expr)
}
fn eval_unop_i32(op: UnOp, a: i32) -> Value {
    let expr = Expr::UnOp {
        op,
        operand: Box::new(Expr::i32(a)),
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
fn canonical_f32(value: f32) -> f32 {
    if value.is_nan() {
        f32::from_bits(0x7FC0_0000)
    } else if value.is_subnormal() {
        f32::from_bits(value.to_bits() & 0x8000_0000)
    } else {
        value
    }
}
fn expected_f32(value: f32) -> Value {
    Value::Float(f64::from(canonical_f32(value)))
}
fn eval_cast(target: DataType, value: Expr) -> Value {
    let expr = Expr::Cast {
        target,
        value: Box::new(value),
    };
    eval_expr_value(&expr)
}
fn u32_adversarial() -> impl Strategy<Value = u32> {
    prop_oneof![any::<u32>(), Just(u32::MAX), Just(0), Just(1),]
}
fn i32_adversarial() -> impl Strategy<Value = i32> {
    prop_oneof![
        any::<i32>(),
        Just(i32::MIN),
        Just(i32::MAX),
        Just(0),
        Just(-1),
    ]
}
fn f32_adversarial() -> impl Strategy<Value = f32> {
    prop_oneof![
        any::<f32>(),
        Just(f32::NAN),
        Just(f32::INFINITY),
        Just(f32::NEG_INFINITY),
        Just(0.0),
        Just(-0.0),
        Just(1.0),
        Just(-1.0),
    ]
}
#[derive(Debug)]
struct DummyOpaque;
impl vyre_foundation::ir::ExprNode for DummyOpaque {
    fn extension_kind(&self) -> &'static str {
        "test.dummy"
    }
    fn debug_identity(&self) -> &str {
        "dummy"
    }
    fn result_type(&self) -> Option<DataType> {
        Some(DataType::U32)
    }
    fn cse_safe(&self) -> bool {
        false
    }
    fn stable_fingerprint(&self) -> [u8; 32] {
        [0; 32]
    }
    fn validate_extension(&self) -> Result<(), String> {
        Ok(())
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
// ---------------------------------------------------------------------------
// BinOp – u32
// ---------------------------------------------------------------------------
proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]
    #[test]
    fn prop_binop_add_u32(a in any::<u32>(), b in any::<u32>()) {
        prop_assert_eq!(eval_binop_u32(BinOp::Add, a, b), Value::U32(a.wrapping_add(b)));
    }
    #[test]
    fn prop_binop_sub_u32(a in any::<u32>(), b in any::<u32>()) {
        prop_assert_eq!(eval_binop_u32(BinOp::Sub, a, b), Value::U32(a.wrapping_sub(b)));
    }
    #[test]
    fn prop_binop_mul_u32(a in any::<u32>(), b in any::<u32>()) {
        prop_assert_eq!(eval_binop_u32(BinOp::Mul, a, b), Value::U32(a.wrapping_mul(b)));
    }
    #[test]
    fn prop_binop_div_u32(a in any::<u32>(), b in any::<u32>()) {
        let expected = if b == 0 { Value::U32(u32::MAX) } else { Value::U32(a / b) };
        prop_assert_eq!(eval_binop_u32(BinOp::Div, a, b), expected);
    }
    #[test]
    fn prop_binop_mod_u32(a in any::<u32>(), b in any::<u32>()) {
        let expected = if b == 0 { Value::U32(0) } else { Value::U32(a % b) };
        prop_assert_eq!(eval_binop_u32(BinOp::Mod, a, b), expected);
    }
    #[test]
    fn prop_binop_bitand_u32(a in any::<u32>(), b in any::<u32>()) {
        prop_assert_eq!(eval_binop_u32(BinOp::BitAnd, a, b), Value::U32(a & b));
    }
    #[test]
    fn prop_binop_bitor_u32(a in any::<u32>(), b in any::<u32>()) {
        prop_assert_eq!(eval_binop_u32(BinOp::BitOr, a, b), Value::U32(a | b));
    }
    #[test]
    fn prop_binop_bitxor_u32(a in any::<u32>(), b in any::<u32>()) {
        prop_assert_eq!(eval_binop_u32(BinOp::BitXor, a, b), Value::U32(a ^ b));
    }
    #[test]
    fn prop_binop_shl_u32(a in any::<u32>(), b in any::<u32>()) {
        // WGSL: shift amount modulo bit-width.
        prop_assert_eq!(eval_binop_u32(BinOp::Shl, a, b), Value::U32(a << (b & 31)));
    }
    #[test]
    fn prop_binop_shr_u32(a in any::<u32>(), b in any::<u32>()) {
        prop_assert_eq!(eval_binop_u32(BinOp::Shr, a, b), Value::U32(a >> (b & 31)));
    }
    #[test]
    fn prop_binop_eq_u32(a in any::<u32>(), b in any::<u32>()) {
        prop_assert_eq!(eval_binop_u32(BinOp::Eq, a, b), Value::Bool(a == b));
    }
    #[test]
    fn prop_binop_ne_u32(a in any::<u32>(), b in any::<u32>()) {
        prop_assert_eq!(eval_binop_u32(BinOp::Ne, a, b), Value::Bool(a != b));
    }
    #[test]
    fn prop_binop_lt_u32(a in any::<u32>(), b in any::<u32>()) {
        prop_assert_eq!(eval_binop_u32(BinOp::Lt, a, b), Value::Bool(a < b));
    }
    #[test]
    fn prop_binop_gt_u32(a in any::<u32>(), b in any::<u32>()) {
        prop_assert_eq!(eval_binop_u32(BinOp::Gt, a, b), Value::Bool(a > b));
    }
    #[test]
    fn prop_binop_le_u32(a in any::<u32>(), b in any::<u32>()) {
        prop_assert_eq!(eval_binop_u32(BinOp::Le, a, b), Value::Bool(a <= b));
    }
    #[test]
    fn prop_binop_ge_u32(a in any::<u32>(), b in any::<u32>()) {
        prop_assert_eq!(eval_binop_u32(BinOp::Ge, a, b), Value::Bool(a >= b));
    }
    #[test]
    fn prop_binop_and_u32(a in any::<u32>(), b in any::<u32>()) {
        prop_assert_eq!(eval_binop_u32(BinOp::And, a, b), Value::Bool(a != 0 && b != 0));
    }
    #[test]
    fn prop_binop_or_u32(a in any::<u32>(), b in any::<u32>()) {
        prop_assert_eq!(eval_binop_u32(BinOp::Or, a, b), Value::Bool(a != 0 || b != 0));
    }
    #[test]
    fn prop_binop_absdiff_u32(a in any::<u32>(), b in any::<u32>()) {
        prop_assert_eq!(eval_binop_u32(BinOp::AbsDiff, a, b), Value::U32(a.abs_diff(b)));
    }
    #[test]
    fn prop_binop_min_u32(a in any::<u32>(), b in any::<u32>()) {
        prop_assert_eq!(eval_binop_u32(BinOp::Min, a, b), Value::U32(a.min(b)));
    }
    #[test]
    fn prop_binop_max_u32(a in any::<u32>(), b in any::<u32>()) {
        prop_assert_eq!(eval_binop_u32(BinOp::Max, a, b), Value::U32(a.max(b)));
    }
}
// ---------------------------------------------------------------------------
// Edge cases: divide-by-zero (proptest with any::<u32> never generates 0)
// ---------------------------------------------------------------------------
mod expr_adversarial_proptest_part1 {
    include!("__split/expr_adversarial_proptest_part1.rs");
}
mod expr_adversarial_proptest_part2 {
    include!("__split/expr_adversarial_proptest_part2.rs");
}
