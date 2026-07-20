//! Generated scalar-semantics coverage for the public storage-graph oracle.

use vyre_foundation::ir::{BinOp, NodeId, NodeStorage, UnOp, Value as IrValue};
use vyre_reference::run_storage_graph;

#[test]
fn generated_u32_binary_storage_graph_semantics_match_contract_matrix() {
    let ops = u32_binary_cases();
    let values = generated_u32_values();
    let mut checked = 0usize;

    for case in 0..4096usize {
        let left = values[case % values.len()];
        let right = values[(case.wrapping_mul(37).wrapping_add(11)) % values.len()];
        for &(op, expected) in &ops {
            assert_eq!(
                eval_u32_bin(op, left, right)
                    .unwrap_or_else(|error| panic!("Fix: u32 {op:?} should evaluate: {error}")),
                expected(left, right),
                "case {case} op {op:?} left={left:#010x} right={right:#010x}"
            );
            checked += 1;
        }
    }

    assert_eq!(checked, 4096 * ops.len());
}

#[test]
fn generated_u32_unary_storage_graph_semantics_match_contract_matrix() {
    let ops = u32_unary_cases();
    let values = generated_u32_values();
    let mut checked = 0usize;

    for case in 0..8192usize {
        let value = values[(case.wrapping_mul(19).wrapping_add(3)) % values.len()];
        for (op, expected) in &ops {
            assert_eq!(
                eval_u32_un(op.clone(), value)
                    .unwrap_or_else(|error| panic!("Fix: u32 {op:?} should evaluate: {error}")),
                expected(value),
                "case {case} op {op:?} value={value:#010x}"
            );
            checked += 1;
        }
    }

    assert_eq!(checked, 8192 * ops.len());
}

#[test]
fn generated_bool_storage_graph_semantics_match_contract_matrix() {
    for &left in &[false, true] {
        for &right in &[false, true] {
            assert_eq!(
                eval_bool_bin(BinOp::And, left, right).unwrap(),
                IrValue::Bool(left && right)
            );
            assert_eq!(
                eval_bool_bin(BinOp::Or, left, right).unwrap(),
                IrValue::Bool(left || right)
            );
            assert_eq!(
                eval_bool_bin(BinOp::Eq, left, right).unwrap(),
                IrValue::Bool(left == right)
            );
            assert_eq!(
                eval_bool_bin(BinOp::Ne, left, right).unwrap(),
                IrValue::Bool(left != right)
            );
        }
        assert_eq!(
            eval_bool_un(UnOp::LogicalNot, left).unwrap(),
            IrValue::Bool(!left)
        );
    }
}

#[test]
fn generated_storage_graph_scalar_type_mismatches_are_actionable_errors() {
    let mismatch = vec![
        (NodeId(0), NodeStorage::LitU32(1)),
        (NodeId(1), NodeStorage::LitBool(true)),
        (
            NodeId(2),
            NodeStorage::BinOp {
                op: BinOp::Add,
                left: NodeId(0),
                right: NodeId(1),
            },
        ),
    ];
    let error = run_storage_graph(&mismatch, &[NodeId(2)])
        .expect_err("Fix: storage graph scalar type mismatch must fail.");
    assert!(
        error.to_string().contains("type mismatch"),
        "Fix: scalar mismatch errors must identify the type issue: {error}"
    );
}

type BinExpected = fn(u32, u32) -> IrValue;
type UnExpected = fn(u32) -> IrValue;

fn u32_binary_cases() -> [(BinOp, BinExpected); 27] {
    [
        (BinOp::Add, |left, right| {
            IrValue::U32(left.wrapping_add(right))
        }),
        (BinOp::Sub, |left, right| {
            IrValue::U32(left.wrapping_sub(right))
        }),
        (BinOp::Mul, |left, right| {
            IrValue::U32(left.wrapping_mul(right))
        }),
        (BinOp::Div, |left, right| {
            IrValue::U32(left.checked_div(right).unwrap_or(u32::MAX))
        }),
        (BinOp::Mod, |left, right| {
            IrValue::U32(left.checked_rem(right).unwrap_or(0))
        }),
        (BinOp::BitAnd, |left, right| IrValue::U32(left & right)),
        (BinOp::BitOr, |left, right| IrValue::U32(left | right)),
        (BinOp::BitXor, |left, right| IrValue::U32(left ^ right)),
        (BinOp::Shl, |left, right| {
            IrValue::U32(left.wrapping_shl(right & 31))
        }),
        (BinOp::Shr, |left, right| {
            IrValue::U32(left.wrapping_shr(right & 31))
        }),
        (BinOp::Eq, |left, right| IrValue::Bool(left == right)),
        (BinOp::Ne, |left, right| IrValue::Bool(left != right)),
        (BinOp::Lt, |left, right| IrValue::Bool(left < right)),
        (BinOp::Le, |left, right| IrValue::Bool(left <= right)),
        (BinOp::Gt, |left, right| IrValue::Bool(left > right)),
        (BinOp::Ge, |left, right| IrValue::Bool(left >= right)),
        (BinOp::Min, |left, right| IrValue::U32(left.min(right))),
        (BinOp::Max, |left, right| IrValue::U32(left.max(right))),
        (BinOp::SaturatingAdd, |left, right| {
            IrValue::U32(left.saturating_add(right))
        }),
        (BinOp::SaturatingSub, |left, right| {
            IrValue::U32(left.saturating_sub(right))
        }),
        (BinOp::SaturatingMul, |left, right| {
            IrValue::U32(left.saturating_mul(right))
        }),
        (BinOp::AbsDiff, |left, right| {
            IrValue::U32(left.abs_diff(right))
        }),
        (BinOp::RotateLeft, |left, right| {
            IrValue::U32(left.rotate_left(right & 31))
        }),
        (BinOp::RotateRight, |left, right| {
            IrValue::U32(left.rotate_right(right & 31))
        }),
        (BinOp::MulHigh, |left, right| {
            IrValue::U32(((u64::from(left).wrapping_mul(u64::from(right))) >> 32) as u32)
        }),
        (BinOp::And, |left, right| {
            IrValue::Bool(left != 0 && right != 0)
        }),
        (BinOp::Or, |left, right| {
            IrValue::Bool(left != 0 || right != 0)
        }),
    ]
}

fn u32_unary_cases() -> [(UnOp, UnExpected); 6] {
    [
        (UnOp::BitNot, |value| IrValue::U32(!value)),
        (UnOp::LogicalNot, |value| IrValue::Bool(value == 0)),
        (UnOp::Popcount, |value| IrValue::U32(value.count_ones())),
        (UnOp::Clz, |value| IrValue::U32(value.leading_zeros())),
        (UnOp::Ctz, |value| IrValue::U32(value.trailing_zeros())),
        (UnOp::ReverseBits, |value| {
            IrValue::U32(value.reverse_bits())
        }),
    ]
}

fn eval_u32_bin(op: BinOp, left: u32, right: u32) -> Result<IrValue, vyre_foundation::Error> {
    let graph = vec![
        (NodeId(0), NodeStorage::LitU32(left)),
        (NodeId(1), NodeStorage::LitU32(right)),
        (
            NodeId(2),
            NodeStorage::BinOp {
                op,
                left: NodeId(0),
                right: NodeId(1),
            },
        ),
    ];
    Ok(run_storage_graph(&graph, &[NodeId(2)])?[0])
}

fn eval_u32_un(op: UnOp, value: u32) -> Result<IrValue, vyre_foundation::Error> {
    let graph = vec![
        (NodeId(0), NodeStorage::LitU32(value)),
        (
            NodeId(1),
            NodeStorage::UnOp {
                op,
                operand: NodeId(0),
            },
        ),
    ];
    Ok(run_storage_graph(&graph, &[NodeId(1)])?[0])
}

fn eval_bool_bin(op: BinOp, left: bool, right: bool) -> Result<IrValue, vyre_foundation::Error> {
    let graph = vec![
        (NodeId(0), NodeStorage::LitBool(left)),
        (NodeId(1), NodeStorage::LitBool(right)),
        (
            NodeId(2),
            NodeStorage::BinOp {
                op,
                left: NodeId(0),
                right: NodeId(1),
            },
        ),
    ];
    Ok(run_storage_graph(&graph, &[NodeId(2)])?[0])
}

fn eval_bool_un(op: UnOp, value: bool) -> Result<IrValue, vyre_foundation::Error> {
    let graph = vec![
        (NodeId(0), NodeStorage::LitBool(value)),
        (
            NodeId(1),
            NodeStorage::UnOp {
                op,
                operand: NodeId(0),
            },
        ),
    ];
    Ok(run_storage_graph(&graph, &[NodeId(1)])?[0])
}

fn generated_u32_values() -> Vec<u32> {
    let mut values = vec![
        0,
        1,
        2,
        3,
        7,
        8,
        15,
        16,
        31,
        32,
        63,
        64,
        127,
        128,
        255,
        256,
        1023,
        1024,
        u16::MAX as u32,
        (u16::MAX as u32) + 1,
        i32::MAX as u32,
        i32::MIN as u32,
        u32::MAX - 1,
        u32::MAX,
    ];
    let mut state = 0x9e37_79b9u32;
    for index in 0..512u32 {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        values.push(state.rotate_left(index & 31));
    }
    values.sort_unstable();
    values.dedup();
    values
}
