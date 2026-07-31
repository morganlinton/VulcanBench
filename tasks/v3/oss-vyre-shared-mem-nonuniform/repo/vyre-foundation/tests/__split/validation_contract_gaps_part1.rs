use super::*;

#[test]
fn workgroup_size_zero_is_rejected() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [0, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::u32(1))],
    );
    let errors = validate(&program);
    assert!(
        errors
            .iter()
            .any(|e| e.message().contains("workgroup_size[0] is 0")),
        "zero workgroup dimension must be rejected, got {:?}",
        errors
    );
}

#[test]
fn duplicate_buffer_name_is_rejected() {
    let program = Program::wrapped(
        vec![
            BufferDecl::read("a", 0, DataType::U32).with_count(1),
            BufferDecl::read("a", 1, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::u32(1))],
    );
    let errors = validate(&program);
    assert!(
        errors
            .iter()
            .any(|e| e.message().contains("duplicate buffer name `a`")),
        "duplicate buffer name must be rejected, got {:?}",
        errors
    );
}

#[test]
fn duplicate_binding_slot_is_rejected() {
    let program = Program::wrapped(
        vec![
            BufferDecl::read("a", 0, DataType::U32).with_count(1),
            BufferDecl::read("b", 0, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::u32(1))],
    );
    let errors = validate(&program);
    assert!(
        errors
            .iter()
            .any(|e| e.message().contains("duplicate binding slot 0")),
        "duplicate binding slot must be rejected, got {:?}",
        errors
    );
}

#[test]
fn workgroup_buffer_zero_count_is_rejected() {
    let mut buf = BufferDecl::workgroup("scratch", 64, DataType::U32);
    buf.count = 0; // bypass constructor guard
    let program = Program::wrapped(
        vec![
            buf,
            BufferDecl::output("out", 0, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::u32(1))],
    );
    let errors = validate(&program);
    assert!(
        errors.iter().any(|e| e
            .message()
            .contains("workgroup buffer `scratch` has count 0")),
        "zero-count workgroup buffer must be rejected, got {:?}",
        errors
    );
}

#[test]
fn multiple_output_buffers_are_rejected() {
    let program = Program::wrapped(
        vec![
            BufferDecl::output("out1", 0, DataType::U32).with_count(1),
            BufferDecl::output("out2", 1, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![],
    );
    let errors = validate(&program);
    assert!(
        errors
            .iter()
            .any(|e| e.message().contains("declares 2 output buffers")),
        "multiple outputs must be rejected, got {:?}",
        errors
    );
}

#[test]
fn store_to_unknown_buffer_is_rejected() {
    let program = output_program(vec![Node::store("missing", Expr::u32(0), Expr::u32(1))]);
    let errors = validate(&program);
    assert!(
        errors
            .iter()
            .any(|e| e.message().contains("store to unknown buffer `missing`")),
        "store to unknown buffer must be rejected, got {:?}",
        errors
    );
}

#[test]
fn store_to_read_only_buffer_is_rejected() {
    let program = Program::wrapped(
        vec![BufferDecl::read("ro", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::store("ro", Expr::u32(0), Expr::u32(1))],
    );
    let errors = validate(&program);
    assert!(
        errors
            .iter()
            .any(|e| e.message().contains("store to non-writable buffer `ro`")),
        "store to read-only buffer must be rejected, got {:?}",
        errors
    );
}

#[test]
fn load_from_unknown_buffer_is_rejected() {
    let program = output_program(vec![Node::let_bind(
        "x",
        Expr::load("missing", Expr::u32(0)),
    )]);
    let errors = validate(&program);
    assert!(
        errors
            .iter()
            .any(|e| e.message().contains("load from unknown buffer `missing`")),
        "load from unknown buffer must be rejected, got {:?}",
        errors
    );
}

#[test]
fn buflen_of_unknown_buffer_is_rejected() {
    let program = output_program(vec![Node::let_bind("x", Expr::buf_len("missing"))]);
    let errors = validate(&program);
    assert!(
        errors
            .iter()
            .any(|e| e.message().contains("buflen of unknown buffer `missing`")),
        "buflen of unknown buffer must be rejected, got {:?}",
        errors
    );
}

#[test]
fn assignment_to_undeclared_variable_is_rejected() {
    let program = output_program(vec![Node::assign("x", Expr::u32(1))]);
    let errors = validate(&program);
    assert!(
        errors.iter().any(|e| e
            .message()
            .contains("assignment to undeclared variable `x`")),
        "assignment to undeclared variable must be rejected, got {:?}",
        errors
    );
}

#[test]
fn assignment_to_loop_variable_is_rejected() {
    let program = output_program(vec![Node::loop_(
        "i",
        Expr::u32(0),
        Expr::u32(1),
        vec![Node::assign("i", Expr::u32(2))],
    )]);
    let errors = validate(&program);
    assert!(
        errors
            .iter()
            .any(|e| e.message().contains("assignment to loop variable `i`")),
        "assignment to loop variable must be rejected, got {:?}",
        errors
    );
}

#[test]
fn if_condition_with_f32_is_rejected() {
    let program = output_program(vec![Node::if_then_else(
        Expr::LitF32(1.0),
        vec![Node::store("out", Expr::u32(0), Expr::u32(1))],
        vec![],
    )]);
    let errors = validate(&program);
    assert!(
        errors.iter().any(|e| e
            .message()
            .contains("Node::If condition has type `f32` but must be `u32` or `bool`")),
        "If with f32 condition must be rejected, got {:?}",
        errors
    );
}

#[test]
fn loop_with_non_u32_from_bound_is_rejected() {
    let program = output_program(vec![Node::loop_(
        "i",
        Expr::LitF32(0.0),
        Expr::u32(1),
        vec![],
    )]);
    let errors = validate(&program);
    assert!(
        errors
            .iter()
            .any(|e| e.message().contains("Node::Loop from-bound has type `f32`")),
        "Loop with f32 from-bound must be rejected, got {:?}",
        errors
    );
}

#[test]
fn loop_with_non_u32_to_bound_is_rejected() {
    let program = output_program(vec![Node::loop_(
        "i",
        Expr::u32(0),
        Expr::LitF32(1.0),
        vec![],
    )]);
    let errors = validate(&program);
    assert!(
        errors
            .iter()
            .any(|e| e.message().contains("Node::Loop to-bound has type `f32`")),
        "Loop with f32 to-bound must be rejected, got {:?}",
        errors
    );
}

#[test]
fn barrier_in_divergent_if_is_rejected() {
    // The condition must be non-uniform across the workgroup for the
    // branch to be considered divergent. `InvocationId` differs per
    // lane, so the then-arm only fires for lane 0, leaving the
    // barrier reached by only part of the workgroup.
    let program = output_program(vec![Node::if_then_else(
        Expr::eq(Expr::InvocationId { axis: 0 }, Expr::u32(0)),
        vec![Node::barrier()],
        vec![],
    )]);
    let errors = validate(&program);
    assert!(
        errors.iter().any(|e| e
            .message()
            .contains("V010: barrier may be reached by only part of a workgroup")),
        "barrier inside divergent If must be rejected, got {:?}",
        errors
    );
}

#[test]
fn indirect_dispatch_misaligned_offset_is_rejected() {
    let program = Program::wrapped(
        vec![BufferDecl::read("counts", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::indirect_dispatch("counts", 2)],
    );
    let errors = validate(&program);
    assert!(
        errors.iter().any(|e| e
            .message()
            .contains("indirect dispatch offset 2 is not 4-byte aligned")),
        "misaligned indirect dispatch must be rejected, got {:?}",
        errors
    );
}

#[test]
fn indirect_dispatch_unknown_buffer_is_rejected() {
    let program = output_program(vec![Node::indirect_dispatch("counts", 0)]);
    let errors = validate(&program);
    assert!(
        errors.iter().any(|e| e
            .message()
            .contains("indirect dispatch references unknown buffer `counts`")),
        "indirect dispatch with unknown buffer must be rejected, got {:?}",
        errors
    );
}

#[test]
fn async_load_with_empty_tag_is_rejected() {
    let program = Program::wrapped(
        vec![BufferDecl::read_write("a", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::async_load_ext(
            "a",
            "a",
            Expr::u32(0),
            Expr::u32(1),
            "",
        )],
    );
    let errors = validate(&program);
    assert!(
        errors
            .iter()
            .any(|e| e.message().contains("async stream tag is empty")),
        "async load with empty tag must be rejected, got {:?}",
        errors
    );
}

#[test]
fn async_wait_with_empty_tag_is_rejected() {
    let program = output_program(vec![Node::async_wait("")]);
    let errors = validate(&program);
    assert!(
        errors
            .iter()
            .any(|e| e.message().contains("async stream tag is empty")),
        "async wait with empty tag must be rejected, got {:?}",
        errors
    );
}

#[test]
fn cast_to_bytes_from_non_bytes_is_rejected() {
    let program = output_program(vec![Node::let_bind(
        "x",
        Expr::cast(DataType::Bytes, Expr::u32(1)),
    )]);
    let errors = validate(&program);
    assert!(
        errors
            .iter()
            .any(|e| e.message().contains("cast to Bytes is unsupported")),
        "cast to Bytes from non-Bytes must be rejected, got {:?}",
        errors
    );
}

#[test]
fn fma_with_u32_operand_is_rejected() {
    let program = output_program(vec![Node::let_bind(
        "x",
        Expr::fma(Expr::u32(1), Expr::u32(2), Expr::u32(3)),
    )]);
    let errors = validate(&program);
    assert!(
        errors.iter().any(|e| e
            .message()
            .contains("Fma operand `a` has type `u32`, must be `f32`")),
        "Fma with u32 operand must be rejected, got {:?}",
        errors
    );
}

#[test]
fn select_with_mismatched_branch_types_is_rejected() {
    let program = output_program(vec![Node::let_bind(
        "x",
        Expr::select(Expr::bool(true), Expr::u32(1), Expr::LitF32(2.0)),
    )]);
    let errors = validate(&program);
    assert!(
        errors.iter().any(|e| e
            .message()
            .contains("Select branches have mismatched types")),
        "Select with mismatched types must be rejected, got {:?}",
        errors
    );
}

#[test]
fn binop_bool_plus_int_is_rejected() {
    let program = output_program(vec![Node::let_bind(
        "x",
        Expr::BinOp {
            op: BinOp::Add,
            left: Box::new(Expr::bool(true)),
            right: Box::new(Expr::u32(1)),
        },
    )]);
    let errors = validate(&program);
    assert!(
        errors.iter().any(|e| e
            .message()
            .contains("binary operation `Add` left operand has type `bool`")),
        "bool + int must be rejected, got {:?}",
        errors
    );
}

#[test]
fn binop_mixed_numeric_types_is_rejected() {
    let program = output_program(vec![Node::let_bind(
        "x",
        Expr::BinOp {
            op: BinOp::Add,
            left: Box::new(Expr::u32(1)),
            right: Box::new(Expr::LitF32(1.0)),
        },
    )]);
    let errors = validate(&program);
    assert!(
        errors.iter().any(|e| e
            .message()
            .contains("operands have mismatched numeric types: left=`u32`, right=`f32`")),
        "u32 + f32 must be rejected, got {:?}",
        errors
    );
}

