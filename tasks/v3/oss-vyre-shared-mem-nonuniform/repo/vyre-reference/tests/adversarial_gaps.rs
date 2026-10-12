//! Adversarial tests that expose real semantic gaps in the vyre-reference CPU
//! interpreter. Every assertion documents behavior that was previously untested.

use vyre::ir::{BinOp, BufferDecl, DataType, Expr, Node, Program, UnOp};
use vyre_reference::{
    execution::expr as eval_expr,
    execution::expr::Buffer,
    reference_eval,
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
    .expect("Fix: reference evaluator must evaluate generated expression")
}

fn float_bits(value: Value) -> u32 {
    match value {
        Value::Float(v) => (v as f32).to_bits(),
        other => panic!("expected float value, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// 1. Malformed / adversarial Programs
// ---------------------------------------------------------------------------

mod adversarial_gaps_part1 {

    include!("__split/adversarial_gaps_part1.rs");
}
mod adversarial_gaps_part2 {
    include!("__split/adversarial_gaps_part2.rs");
}
