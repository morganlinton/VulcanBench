//! Generated 64-bit unsigned scalar coverage for the public storage-graph oracle.

use vyre_foundation::ir::{BinOp, NodeId, NodeStorage, UnOp, Value as IrValue};
use vyre_reference::run_storage_graph;

#[test]
fn generated_u64_binary_storage_graph_semantics_match_contract_matrix() {
    let ops = u64_binary_cases();
    let values = generated_u64_values();
    let mut checked = 0usize;

    for case in 0..4096usize {
        let left = values[case % values.len()];
        let right = values[(case.wrapping_mul(43).wrapping_add(19)) % values.len()];
        for &(op, expected) in &ops {
            assert_eq!(
                eval_u64_bin(op, left, right)
                    .unwrap_or_else(|error| panic!("Fix: u64 {op:?} should evaluate: {error}")),
                expected(left, right),
                "case {case} op {op:?} left={left:#018x} right={right:#018x}"
            );
            checked += 1;
        }
    }

    assert_eq!(checked, 4096 * ops.len());
}

#[test]
fn generated_u64_unary_storage_graph_semantics_match_contract_matrix() {
    let ops = u64_unary_cases();
    let values = generated_u64_values();
    let mut checked = 0usize;

    for case in 0..8192usize {
        let value = values[(case.wrapping_mul(23).wrapping_add(7)) % values.len()];
        for (op, expected) in &ops {
            assert_eq!(
                eval_u64_un(op.clone(), value)
                    .unwrap_or_else(|error| panic!("Fix: u64 {op:?} should evaluate: {error}")),
                expected(value),
                "case {case} op {op:?} value={value:#018x}"
            );
            checked += 1;
        }
    }

    assert_eq!(checked, 8192 * ops.len());
}

#[test]
fn generated_u64_zero_divisor_semantics_are_total_and_backend_aligned() {
    for &left in &[0, 1, 2, u32::MAX as u64, (u32::MAX as u64) + 1, u64::MAX] {
        assert_eq!(
            eval_u64_bin(BinOp::Div, left, 0).unwrap(),
            IrValue::U64(u64::MAX),
            "u64 division by zero is totalized to u64::MAX for backend parity"
        );
        assert_eq!(
            eval_u64_bin(BinOp::Mod, left, 0).unwrap(),
            IrValue::U64(0),
            "u64 remainder by zero is totalized to zero for backend parity"
        );
    }
}

type U64BinaryExpected = fn(u64, u64) -> IrValue;
type U64UnaryExpected = fn(u64) -> IrValue;

fn u64_binary_cases() -> [(BinOp, U64BinaryExpected); 27] {
    [
        (BinOp::Add, |left, right| {
            IrValue::U64(left.wrapping_add(right))
        }),
        (BinOp::Sub, |left, right| {
            IrValue::U64(left.wrapping_sub(right))
        }),
        (BinOp::Mul, |left, right| {
            IrValue::U64(left.wrapping_mul(right))
        }),
        (BinOp::Div, |left, right| {
            IrValue::U64(if right == 0 { u64::MAX } else { left / right })
        }),
        (BinOp::Mod, |left, right| {
            IrValue::U64(if right == 0 { 0 } else { left % right })
        }),
        (BinOp::BitAnd, |left, right| IrValue::U64(left & right)),
        (BinOp::BitOr, |left, right| IrValue::U64(left | right)),
        (BinOp::BitXor, |left, right| IrValue::U64(left ^ right)),
        (BinOp::Shl, |left, right| {
            IrValue::U64(left.wrapping_shl((right & 63) as u32))
        }),
        (BinOp::Shr, |left, right| {
            IrValue::U64(left.wrapping_shr((right & 63) as u32))
        }),
        (BinOp::Eq, |left, right| IrValue::Bool(left == right)),
        (BinOp::Ne, |left, right| IrValue::Bool(left != right)),
        (BinOp::Lt, |left, right| IrValue::Bool(left < right)),
        (BinOp::Le, |left, right| IrValue::Bool(left <= right)),
        (BinOp::Gt, |left, right| IrValue::Bool(left > right)),
        (BinOp::Ge, |left, right| IrValue::Bool(left >= right)),
        (BinOp::Min, |left, right| IrValue::U64(left.min(right))),
        (BinOp::Max, |left, right| IrValue::U64(left.max(right))),
        (BinOp::SaturatingAdd, |left, right| {
            IrValue::U64(left.saturating_add(right))
        }),
        (BinOp::SaturatingSub, |left, right| {
            IrValue::U64(left.saturating_sub(right))
        }),
        (BinOp::SaturatingMul, |left, right| {
            IrValue::U64(left.saturating_mul(right))
        }),
        (BinOp::AbsDiff, |left, right| {
            IrValue::U64(left.abs_diff(right))
        }),
        (BinOp::WrappingAdd, |left, right| {
            IrValue::U64(left.wrapping_add(right))
        }),
        (BinOp::WrappingSub, |left, right| {
            IrValue::U64(left.wrapping_sub(right))
        }),
        (BinOp::MulHigh, |left, right| {
            IrValue::U64(((left as u128).wrapping_mul(right as u128) >> 64) as u64)
        }),
        (BinOp::And, |left, right| {
            IrValue::Bool(left != 0 && right != 0)
        }),
        (BinOp::Or, |left, right| {
            IrValue::Bool(left != 0 || right != 0)
        }),
    ]
}

fn u64_unary_cases() -> [(UnOp, U64UnaryExpected); 7] {
    [
        (UnOp::Negate, |value| IrValue::U64(0u64.wrapping_sub(value))),
        (UnOp::BitNot, |value| IrValue::U64(!value)),
        (UnOp::LogicalNot, |value| IrValue::Bool(value == 0)),
        (UnOp::Popcount, |value| {
            IrValue::U64(u64::from(value.count_ones()))
        }),
        (UnOp::Clz, |value| {
            IrValue::U64(u64::from(value.leading_zeros()))
        }),
        (UnOp::Ctz, |value| {
            IrValue::U64(u64::from(value.trailing_zeros()))
        }),
        (UnOp::ReverseBits, |value| {
            IrValue::U64(value.reverse_bits())
        }),
    ]
}

fn eval_u64_bin(op: BinOp, left: u64, right: u64) -> Result<IrValue, vyre_foundation::Error> {
    let graph = vec![
        (NodeId(0), NodeStorage::LitU64(left)),
        (NodeId(1), NodeStorage::LitU64(right)),
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

fn eval_u64_un(op: UnOp, value: u64) -> Result<IrValue, vyre_foundation::Error> {
    let graph = vec![
        (NodeId(0), NodeStorage::LitU64(value)),
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

fn generated_u64_values() -> Vec<u64> {
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
        u16::MAX as u64,
        (u16::MAX as u64) + 1,
        u32::MAX as u64,
        (u32::MAX as u64) + 1,
        i64::MAX as u64,
        i64::MIN as u64,
        u64::MAX - 1,
        u64::MAX,
    ];

    let mut state = 0x243f_6a88_85a3_08d3u64;
    for index in 0..1024u32 {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        values.push(state.rotate_left(index & 63));
    }
    values.sort_unstable();
    values.dedup();
    values
}
