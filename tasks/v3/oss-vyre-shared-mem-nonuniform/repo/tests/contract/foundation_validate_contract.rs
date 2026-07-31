// Workspace contract: foundation validator must stay rejection-complete.
//
// These tests encode non-negotiable validation contracts. If the validator
// becomes permissive and starts accepting the malformed programs below, this
// suite fails loudly.

use vyre_foundation::ir::{BinOp, BufferDecl, DataType, Expr, Node, Program};
use vyre_foundation::validate::validate;

fn output_program(nodes: Vec<Node>) -> Program {
    Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(4)],
        [1, 1, 1],
        nodes,
    )
}

fn must_reject(program: &Program, needle: &str, contract: &str) {
    let errors = validate(program);
    assert!(
        errors.iter().any(|error| error.message().contains(needle)),
        "contract `{contract}` broken: expected `{needle}`, got {errors:?}"
    );
}

#[test]
fn contract_zero_workgroup_dimension_is_never_allowed() {
    must_reject(
        &Program::wrapped(
            vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
            [0, 4, 8],
            vec![Node::Return],
        ),
        "workgroup_size[0] is 0",
        "zero workgroup dimension",
    );
}

#[test]
fn contract_multiple_output_buffers_remain_forbidden() {
    must_reject(
        &Program::wrapped(
            vec![
                BufferDecl::output("out_a", 0, DataType::U32).with_count(1),
                BufferDecl::output("out_b", 1, DataType::U32).with_count(1),
            ],
            [1, 1, 1],
            vec![Node::Return],
        ),
        "declares 2 output buffers",
        "single output buffer",
    );
}

#[test]
fn contract_static_zero_divisor_remains_v044() {
    must_reject(
        &output_program(vec![Node::let_bind(
            "x",
            Expr::div(Expr::u32(42), Expr::u32(0)),
        )]),
        "V044",
        "static zero divisor",
    );
}

#[test]
fn contract_u64_arithmetic_remains_forbidden() {
    must_reject(
        &output_program(vec![Node::let_bind(
            "x",
            Expr::add(Expr::u64(1), Expr::u64(2)),
        )]),
        "64-bit integer arithmetic",
        "u64 arithmetic",
    );
}

#[test]
fn contract_unknown_buffer_load_remains_forbidden() {
    must_reject(
        &output_program(vec![Node::let_bind(
            "x",
            Expr::load("phantom", Expr::u32(0)),
        )]),
        "load from unknown buffer `phantom`",
        "unknown buffer load",
    );
}

#[test]
fn contract_select_branch_type_mismatch_remains_v029() {
    must_reject(
        &output_program(vec![Node::let_bind(
            "x",
            Expr::select(Expr::bool(true), Expr::u32(1), Expr::f32(2.0)),
        )]),
        "V029",
        "select branch mismatch",
    );
}

#[test]
fn contract_assignment_type_mismatch_remains_v045() {
    must_reject(
        &output_program(vec![
            Node::let_bind("x", Expr::u32(1)),
            Node::assign("x", Expr::f32(1.0)),
        ]),
        "V045",
        "assignment type mismatch",
    );
}

#[test]
fn contract_cast_u64_to_f32_remains_unsupported() {
    must_reject(
        &output_program(vec![Node::let_bind(
            "x",
            Expr::cast(DataType::F32, Expr::u64(1)),
        )]),
        "unsupported cast from `u64` to `f32`",
        "u64 to f32 cast",
    );
}

#[test]
fn contract_if_condition_must_not_be_f32() {
    must_reject(
        &output_program(vec![Node::if_then_else(
            Expr::LitF32(1.0),
            vec![Node::store("out", Expr::u32(0), Expr::u32(1))],
            vec![],
        )]),
        "Node::If condition has type `f32` but must be `u32` or `bool`",
        "f32 if condition",
    );
}

#[test]
fn contract_duplicate_buffer_names_remain_forbidden() {
    must_reject(
        &Program::wrapped(
            vec![
                BufferDecl::read("dup", 0, DataType::U32).with_count(1),
                BufferDecl::read("dup", 1, DataType::U32).with_count(1),
            ],
            [1, 1, 1],
            vec![Node::Return],
        ),
        "duplicate buffer name `dup`",
        "duplicate buffer name",
    );
}

#[test]
fn contract_mixed_numeric_addition_remains_forbidden() {
    must_reject(
        &output_program(vec![Node::let_bind(
            "x",
            Expr::BinOp {
                op: BinOp::Add,
                left: Box::new(Expr::u32(1)),
                right: Box::new(Expr::LitF32(1.0)),
            },
        )]),
        "operands have mismatched numeric types: left=`u32`, right=`f32`",
        "mixed numeric add",
    );
}

#[test]
fn contract_invocation_id_axis_out_of_range_remains_forbidden() {
    must_reject(
        &output_program(vec![Node::let_bind("x", Expr::InvocationId { axis: 9 })]),
        "invocation/workgroup ID axis 9 out of range",
        "invocation id axis bound",
    );
}
