use super::*;

#[test]
fn program_with_no_buffers_executes_pure_nodes() {
    let program = Program::wrapped(
        vec![],
        [1, 1, 1],
        vec![Node::let_bind("x", Expr::u32(42))],
    );
    let outputs = reference_eval(&program, &[]).expect("Fix: program with no buffers must execute");
    assert!(outputs.is_empty());
}

#[test]
fn store_to_undefined_buffer_errors() {
    let program = Program::wrapped(
        vec![],
        [1, 1, 1],
        vec![Node::store("missing", Expr::u32(0), Expr::u32(1))],
    );
    let err = reference_eval(&program, &[])
        .expect_err("Fix: store to undefined buffer must be rejected");
    let message = err.to_string();
    assert!(
        message.contains("unknown buffer") || message.contains("missing"),
        "expected actionable buffer diagnostic, got: {message}"
    );
}

#[test]
fn load_from_undefined_buffer_errors() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::load("missing", Expr::u32(0)),
        )],
    );
    let err = reference_eval(&program, &[Value::from(vec![0u8; 4])])
        .expect_err("Fix: load from undefined buffer must be rejected");
    let message = err.to_string();
    assert!(
        message.contains("unknown buffer") || message.contains("missing"),
        "expected actionable buffer diagnostic, got: {message}"
    );
}

#[test]
fn u32_div_by_zero_in_program_returns_max() {
    // Validation rejects a statically-zero divisor (V044), so force a
    // dynamic zero by loading it from a buffer.
    let program = Program::wrapped(
        vec![
            BufferDecl::read("a", 0, DataType::U32).with_count(1),
            BufferDecl::read("b", 1, DataType::U32).with_count(1),
            BufferDecl::output("out", 2, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::div(Expr::load("a", Expr::u32(0)), Expr::load("b", Expr::u32(0))),
        )],
    );
    let outputs = reference_eval(
        &program,
        &[
            Value::from(7u32.to_le_bytes().to_vec()),
            Value::from(0u32.to_le_bytes().to_vec()),
            Value::from(vec![0u8; 4]),
        ],
    )
    .expect("Fix: u32 div by zero must be total in program context");
    assert_eq!(outputs[0].to_bytes(), u32::MAX.to_le_bytes().to_vec());
}

#[test]
fn i32_div_by_zero_in_program_errors() {
    // Validation rejects a statically-zero divisor, so load the divisor
    // dynamically. The output buffer must match the expression type (I32).
    let program = Program::wrapped(
        vec![
            BufferDecl::read("a", 0, DataType::I32).with_count(1),
            BufferDecl::read("b", 1, DataType::I32).with_count(1),
            BufferDecl::output("out", 2, DataType::I32).with_count(1),
        ],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::div(Expr::load("a", Expr::u32(0)), Expr::load("b", Expr::u32(0))),
        )],
    );
    let err = reference_eval(
        &program,
        &[
            Value::from(7i32.to_le_bytes().to_vec()),
            Value::from(0i32.to_le_bytes().to_vec()),
            Value::from(vec![0u8; 4]),
        ],
    )
    .expect_err("Fix: i32 div by zero must error in program context");
    assert!(
        err.to_string().contains("undefined backend semantics"),
        "expected undefined semantics error, got: {err}"
    );
}

#[test]
fn u32_mod_by_zero_in_program_returns_zero() {
    // Validation rejects a statically-zero divisor (V044), so force a
    // dynamic zero by loading it from a buffer.
    let program = Program::wrapped(
        vec![
            BufferDecl::read("a", 0, DataType::U32).with_count(1),
            BufferDecl::read("b", 1, DataType::U32).with_count(1),
            BufferDecl::output("out", 2, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::rem(Expr::load("a", Expr::u32(0)), Expr::load("b", Expr::u32(0))),
        )],
    );
    let outputs = reference_eval(
        &program,
        &[
            Value::from(7u32.to_le_bytes().to_vec()),
            Value::from(0u32.to_le_bytes().to_vec()),
            Value::from(vec![0u8; 4]),
        ],
    )
    .expect("Fix: u32 mod by zero must be total in program context");
    assert_eq!(outputs[0].to_bytes(), 0u32.to_le_bytes().to_vec());
}

#[test]
fn i32_mod_by_zero_errors_at_runtime() {
    // The IR validator rejects `Mod` with i32 operands entirely, so this
    // gap can only be reached through direct expression evaluation.
    let result = eval_expr::eval(
        &Expr::rem(Expr::i32(7), Expr::i32(0)),
        &mut zero_invocation(&empty_program()),
        &mut Memory::empty(),
        &empty_program(),
    );
    let err = result.expect_err("Fix: i32 mod by zero must error");
    assert!(
        err.to_string().contains("undefined backend semantics"),
        "expected undefined semantics error, got: {err}"
    );
}

#[test]
fn u32_shl_by_32_wraps_to_identity() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::shl(Expr::u32(1), Expr::u32(32)),
        )],
    );
    let outputs = reference_eval(&program, &[Value::from(vec![0u8; 4])])
        .expect("Fix: u32 shift by 32 must wrap modulo 32");
    assert_eq!(outputs[0].to_bytes(), 1u32.to_le_bytes().to_vec());
}

#[test]
fn bitwise_op_on_incompatible_types_errors_at_runtime() {
    // The IR validator rejects bitwise ops with mismatched operand types,
    // so this interpreter path is dead code for Programs. Test it directly
    // on the expression evaluator to document the runtime contract.
    let expr = Expr::BinOp {
        op: BinOp::BitAnd,
        left: Box::new(Expr::u32(1)),
        right: Box::new(Expr::f32(1.0)),
    };
    let err = eval_expr::eval(
        &expr,
        &mut zero_invocation(&empty_program()),
        &mut Memory::empty(),
        &empty_program(),
    )
    .expect_err("Fix: bitwise op on mismatched types must error at runtime");
    assert!(
        err.to_string().contains("mismatched operands"),
        "expected mismatched operand error, got: {err}"
    );
}

#[test]
fn store_after_conditional_return_is_skipped_when_branch_taken() {
    // A bare Return followed by a Store is rejected by validation as
    // unreachable code. Use a dynamic condition so the Return is conditional
    // and the Store is considered reachable by the validator.
    let program = Program::wrapped(
        vec![
            BufferDecl::read("cond", 0, DataType::U32).with_count(1),
            BufferDecl::output("out", 1, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![
            Node::if_then(
                Expr::load("cond", Expr::u32(0)),
                vec![Node::Return],
            ),
            Node::store("out", Expr::u32(0), Expr::u32(0xDEAD_BEEF)),
        ],
    );
    // cond = 1 (truthy) -> Return executes -> Store is skipped.
    let outputs = reference_eval(
        &program,
        &[Value::from(1u32.to_le_bytes().to_vec()), Value::from(vec![0u8; 4])],
    )
    .expect("Fix: conditional return must truncate execution cleanly");
    assert_eq!(outputs[0].to_bytes(), vec![0; 4]);
}

#[test]
fn loop_with_zero_iterations_skips_body() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![
            Node::store("out", Expr::u32(0), Expr::u32(0)),
            Node::loop_for(
                "i",
                Expr::u32(0),
                Expr::u32(0),
                vec![Node::store("out", Expr::u32(0), Expr::u32(0xBAD))],
            ),
        ],
    );
    let outputs = reference_eval(&program, &[Value::from(vec![0u8; 4])])
        .expect("Fix: loop with zero iterations must not execute body");
    assert_eq!(outputs[0].to_bytes(), vec![0; 4]);
}

#[test]
fn loop_with_from_greater_than_to_skips_body() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![
            Node::store("out", Expr::u32(0), Expr::u32(0)),
            Node::loop_for(
                "i",
                Expr::u32(5),
                Expr::u32(0),
                vec![Node::store("out", Expr::u32(0), Expr::u32(0xBAD))],
            ),
        ],
    );
    let outputs = reference_eval(&program, &[Value::from(vec![0u8; 4])])
        .expect("Fix: loop with from >= to must execute zero iterations");
    assert_eq!(outputs[0].to_bytes(), vec![0; 4]);
}

#[test]
fn negative_i32_index_is_rejected_not_wrapped() {
    // WGSL allows negative i32 indices by casting to u32 (wrapping).
    // The reference interpreter rejects them. This test documents the gap.
    let program = Program::wrapped(
        vec![
            BufferDecl::read("in", 0, DataType::U32).with_count(1),
            BufferDecl::output("out", 1, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::load("in", Expr::i32(-1)),
        )],
    );
    let err = reference_eval(
        &program,
        &[Value::from(vec![0xAB; 4]), Value::from(vec![0u8; 4])],
    )
    .expect_err(
        "Fix: negative i32 index must be rejected (or wrapped if WGSL parity is desired)",
    );
    let message = err.to_string();
    assert!(
        message.contains("cannot be represented as u32"),
        "expected u32 representation error, got: {message}"
    );
}

// ---------------------------------------------------------------------------
// 2. Float edge cases
// ---------------------------------------------------------------------------

#[test]
fn nan_propagates_through_f32_add() {
    let result = eval_expr_value(&Expr::add(Expr::f32(f32::NAN), Expr::f32(1.0)));
    assert_eq!(
        float_bits(result),
        0x7FC0_0000,
        "NaN + x must yield canonical NaN"
    );
}

#[test]
fn nan_propagates_through_f32_sub() {
    let result = eval_expr_value(&Expr::sub(Expr::f32(f32::NAN), Expr::f32(1.0)));
    assert_eq!(
        float_bits(result),
        0x7FC0_0000,
        "NaN - x must yield canonical NaN"
    );
}

#[test]
fn nan_propagates_through_f32_mul() {
    let result = eval_expr_value(&Expr::mul(Expr::f32(f32::NAN), Expr::f32(1.0)));
    assert_eq!(
        float_bits(result),
        0x7FC0_0000,
        "NaN * x must yield canonical NaN"
    );
}

#[test]
fn nan_propagates_through_f32_div() {
    let result = eval_expr_value(&Expr::div(Expr::f32(f32::NAN), Expr::f32(1.0)));
    assert_eq!(
        float_bits(result),
        0x7FC0_0000,
        "NaN / x must yield canonical NaN"
    );
}

#[test]
fn inf_minus_inf_is_nan() {
    let result = eval_expr_value(&Expr::sub(
        Expr::f32(f32::INFINITY),
        Expr::f32(f32::INFINITY),
    ));
    assert_eq!(
        float_bits(result),
        0x7FC0_0000,
        "Inf - Inf must yield canonical NaN"
    );
}

#[test]
fn zero_div_zero_is_nan() {
    let result = eval_expr_value(&Expr::div(Expr::f32(0.0), Expr::f32(0.0)));
    assert_eq!(
        float_bits(result),
        0x7FC0_0000,
        "0.0 / 0.0 must yield canonical NaN"
    );
}

#[test]
fn f32_to_u32_overflow_saturates_to_max() {
    let result = eval_expr_value(&Expr::cast(DataType::U32, Expr::f32(1e20)));
    assert_eq!(
        result,
        Value::U32(u32::MAX),
        "f32->u32 overflow must saturate"
    );
}

#[test]
fn f32_to_u32_negative_is_zero() {
    let result = eval_expr_value(&Expr::cast(DataType::U32, Expr::f32(-1.0)));
    assert_eq!(result, Value::U32(0), "f32->u32 negative must truncate to zero");
}

#[test]
fn f32_to_u32_nan_is_zero() {
    let result = eval_expr_value(&Expr::cast(DataType::U32, Expr::f32(f32::NAN)));
    assert_eq!(result, Value::U32(0), "f32->u32 NaN must truncate to zero");
}

