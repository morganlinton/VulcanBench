//! Reference-oracle coverage for saturating integer binops.

use vyre::ir::{BinOp, BufferAccess, BufferDecl, DataType, Expr, Node, Program};
use vyre_reference::value::Value;

fn run_binop(op: BinOp, left: u32, right: u32) -> u32 {
    let program = Program::wrapped(
        vec![
            BufferDecl::storage("left", 0, BufferAccess::ReadOnly, DataType::U32).with_count(1),
            BufferDecl::storage("right", 1, BufferAccess::ReadOnly, DataType::U32).with_count(1),
            BufferDecl::output("out", 2, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::BinOp {
                op,
                left: Box::new(Expr::load("left", Expr::u32(0))),
                right: Box::new(Expr::load("right", Expr::u32(0))),
            },
        )],
    );
    let inputs = vec![
        Value::Bytes(left.to_le_bytes().to_vec().into()),
        Value::Bytes(right.to_le_bytes().to_vec().into()),
        Value::Bytes(vec![0u8; 4].into()),
    ];
    let outputs = vyre_reference::reference_eval(&program, &inputs)
        .expect("Fix: reference interpreter must evaluate saturating integer binops");
    let bytes = outputs[0].to_bytes();
    u32::from_le_bytes(bytes[..4].try_into().unwrap())
}

#[test]
fn saturating_add_uses_u32_saturation_not_wraparound() {
    assert_eq!(run_binop(BinOp::SaturatingAdd, u32::MAX, 1), u32::MAX);
    assert_eq!(run_binop(BinOp::SaturatingAdd, 40, 2), 42);
}

#[test]
fn saturating_sub_uses_u32_floor_not_wraparound() {
    assert_eq!(run_binop(BinOp::SaturatingSub, 0, 1), 0);
    assert_eq!(run_binop(BinOp::SaturatingSub, 40, 2), 38);
}

#[test]
fn saturating_mul_uses_u32_saturation_not_wraparound() {
    assert_eq!(run_binop(BinOp::SaturatingMul, u32::MAX, 2), u32::MAX);
    assert_eq!(run_binop(BinOp::SaturatingMul, 7, 6), 42);
}
