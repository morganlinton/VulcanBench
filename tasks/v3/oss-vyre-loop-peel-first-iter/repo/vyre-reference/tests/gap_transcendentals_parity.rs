//! Release gap #1 - reference completeness (deterministic transcendentals).
//!
//! See `contracts/release.md`. The CPU reference oracle must emit
//! byte-identical f32 results for sin/cos/sqrt/exp/log across proptest
//! inputs. Cross-backend bitwise GPU parity is tracked separately in
//! `vyre-driver-wgpu/tests/gap_transcendentals_parity.rs`.

use proptest::prelude::*;
use vyre::ir::{Expr, UnOp};
use vyre_reference::{
    execution::expr as eval_expr,
    ieee754::{canonical_cos, canonical_exp, canonical_log, canonical_sin, canonical_sqrt},
    value::Value,
    workgroup::{Invocation, InvocationIds, Memory},
};

fn empty_program() -> vyre::ir::Program {
    vyre::ir::Program::wrapped(Vec::new(), [1, 1, 1], Vec::new())
}

fn zero_invocation(program: &vyre::ir::Program) -> Invocation<'_> {
    Invocation::new(InvocationIds::ZERO, program.entry())
}

fn eval_unop_f32(op: &UnOp, input: f32) -> Value {
    let program = empty_program();
    let expr = Expr::UnOp {
        op: op.clone(),
        operand: Box::new(Expr::f32(input)),
    };
    eval_expr::eval(
        &expr,
        &mut zero_invocation(&program),
        &mut Memory::empty(),
        &program,
    )
    .expect("Fix: reference interpreter must evaluate generated transcendental expression")
}

fn float_bits(value: Value) -> u32 {
    match value {
        Value::Float(v) => (v as f32).to_bits(),
        other => panic!("Fix: transcendental UnOp must return Float, got {other:?}"),
    }
}

fn canonical_for(op: &UnOp, input: f32) -> f32 {
    match op {
        UnOp::Sin => canonical_sin(input),
        UnOp::Cos => canonical_cos(input),
        UnOp::Sqrt => canonical_sqrt(input),
        UnOp::Exp => canonical_exp(input),
        UnOp::Log => canonical_log(input),
        other => panic!(
            "Fix: gap_transcendentals_parity only covers sin/cos/sqrt/exp/log, got {other:?}"
        ),
    }
}

fn transcendental_strategy() -> impl Strategy<Value = (UnOp, f32)> {
    prop_oneof![
        (-10.0f32..10.0f32).prop_map(|x| (UnOp::Sin, x)),
        (-10.0f32..10.0f32).prop_map(|x| (UnOp::Cos, x)),
        (0.0f32..10.0f32).prop_map(|x| (UnOp::Sqrt, x)),
        (-10.0f32..10.0f32).prop_map(|x| (UnOp::Exp, x)),
        (0.000_001f32..10.0f32).prop_map(|x| (UnOp::Log, x)),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 1000,
        ..ProptestConfig::default()
    })]

    #[test]
    fn reference_transcendentals_match_canonical_oracle((op, input) in transcendental_strategy()) {
        let reference_bits = float_bits(eval_unop_f32(&op, input));
        let canonical_bits = canonical_for(&op, input).to_bits();
        prop_assert_eq!(
            reference_bits,
            canonical_bits,
            "gap_transcendentals_parity: reference {:?}({}) bits {:#010x} must match canonical {:#010x}",
            op,
            input,
            reference_bits,
            canonical_bits
        );
    }

    #[test]
    fn reference_transcendentals_are_deterministic((op, input) in transcendental_strategy()) {
        let first = float_bits(eval_unop_f32(&op, input));
        let second = float_bits(eval_unop_f32(&op, input));
        prop_assert_eq!(
            first,
            second,
            "gap_transcendentals_parity: repeated reference {:?}({}) diverged {:#010x} vs {:#010x}",
            op,
            input,
            first,
            second
        );
    }
}

#[test]
fn reference_transcendental_ops_return_float_values() {
    for (label, op) in [
        ("sin", UnOp::Sin),
        ("cos", UnOp::Cos),
        ("sqrt", UnOp::Sqrt),
        ("exp", UnOp::Exp),
        ("log", UnOp::Log),
    ] {
        match eval_unop_f32(&op, 1.0) {
            Value::Float(_) => {}
            other => panic!("gap_transcendentals_parity: {label} must return Float, got {other:?}"),
        }
    }
}
