// Integration test module for the containing Vyre package.

use vyre_foundation::ir::{Expr, Program};
use vyre_reference::{
    eval_expr,
    value::Value,
    workgroup::{Invocation, InvocationIds, Memory},
};

#[test]
fn div_survives_max_value_boundary_without_panic() {
    // We are testing vyre reference implementation division to ensure it does not panic or trigger UB.
    // Spec behavior for i32::MIN / -1 is wrapping div.

    let program = Program::wrapped(Vec::new(), [1, 1, 1], Vec::new());
    let mut invocation = Invocation::new(InvocationIds::ZERO, program.entry());
    let mut memory = Memory::empty();

    // Hostile input pattern described by max_value: u32::MAX / i32::MIN / i32::MAX edge

    // i32::MIN / -1
    let expr = Expr::div(Expr::i32(i32::MIN), Expr::i32(-1));
    let result = eval_expr::eval(&expr, &mut invocation, &mut memory, &program)
        .expect("FINDING-DIV: i32::MIN / -1 should not error and must survive without panicking");

    assert_eq!(
        result,
        Value::I32(i32::MIN.wrapping_div(-1)),
        "FINDING-DIV: i32::MIN / -1 must evaluate to i32::MIN wrapped, rather than panicking"
    );

    // u32::MAX / u32::MAX
    let expr_u32 = Expr::div(Expr::u32(u32::MAX), Expr::u32(u32::MAX));
    let result_u32 = eval_expr::eval(&expr_u32, &mut invocation, &mut memory, &program).expect(
        "FINDING-DIV: u32::MAX / u32::MAX should not error and must survive without panicking",
    );

    assert_eq!(
        result_u32,
        Value::U32(1),
        "FINDING-DIV: u32::MAX / u32::MAX must evaluate to 1"
    );

    // i32::MAX / i32::MAX
    let expr_i32 = Expr::div(Expr::i32(i32::MAX), Expr::i32(i32::MAX));
    let result_i32 = eval_expr::eval(&expr_i32, &mut invocation, &mut memory, &program).expect(
        "FINDING-DIV: i32::MAX / i32::MAX should not error and must survive without panicking",
    );

    assert_eq!(
        result_i32,
        Value::I32(1),
        "FINDING-DIV: i32::MAX / i32::MAX must evaluate to 1"
    );
}
