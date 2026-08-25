//! Generated signed-integer and floating scalar coverage for the storage graph oracle.

use vyre_foundation::ir::{BinOp, NodeId, NodeStorage, UnOp, Value as IrValue};
use vyre_reference::run_storage_graph;

#[test]
fn generated_i32_binary_storage_graph_semantics_match_contract_matrix() {
    let values = generated_i32_values();
    let mut checked = 0usize;

    for case in 0..4096usize {
        let left = values[case % values.len()];
        let right = values[(case.wrapping_mul(41).wrapping_add(17)) % values.len()];

        for &(op, expected) in &i32_total_binary_cases() {
            assert_eq!(
                eval_i32_bin(op, left, right)
                    .unwrap_or_else(|error| panic!("Fix: i32 {op:?} should evaluate: {error}")),
                expected(left, right),
                "case {case} op {op:?} left={left} right={right}"
            );
            checked += 1;
        }

        if right != 0 && !(left == i32::MIN && right == -1) {
            assert_eq!(
                eval_i32_bin(BinOp::Div, left, right).unwrap(),
                IrValue::I32(left / right)
            );
            assert_eq!(
                eval_i32_bin(BinOp::Mod, left, right).unwrap(),
                IrValue::I32(left % right)
            );
            checked += 2;
        }
    }

    assert!(
        checked > 4096 * i32_total_binary_cases().len(),
        "Fix: generated i32 matrix must include valid division/remainder cases."
    );
}

#[test]
fn generated_i32_division_edges_are_actionable_errors() {
    for &(left, right) in &[
        (0, 0),
        (1, 0),
        (-1, 0),
        (i32::MIN, 0),
        (i32::MAX, 0),
        (i32::MIN, -1),
    ] {
        for op in [BinOp::Div, BinOp::Mod] {
            let error = eval_i32_bin(op, left, right)
                .expect_err("Fix: invalid i32 division/remainder must fail.");
            assert!(
                error
                    .to_string()
                    .contains("undefined target-text semantics"),
                "Fix: invalid i32 division error must explain the signed edge case: {error}"
            );
        }
    }
}

#[test]
fn generated_i32_unary_storage_graph_semantics_match_contract_matrix() {
    for value in generated_i32_values() {
        assert_eq!(
            eval_i32_un(UnOp::Negate, value)
                .unwrap_or_else(|error| panic!("Fix: i32 negate should evaluate: {error}")),
            IrValue::I32(value.wrapping_neg()),
            "value={value}"
        );
    }
}

#[test]
fn generated_f32_binary_storage_graph_semantics_match_contract_matrix() {
    let values = generated_f32_values();
    let mut checked = 0usize;
    for case in 0..2048usize {
        let left = values[case % values.len()];
        let right = values[(case.wrapping_mul(29).wrapping_add(5)) % values.len()];
        for (op, expected) in f32_binary_cases() {
            let actual = eval_f32_bin(op, left, right)
                .unwrap_or_else(|error| panic!("Fix: f32 {op:?} should evaluate: {error}"));
            assert_ir_value_eq(
                actual,
                expected(canonical_f32(left), canonical_f32(right)),
                &format!("case {case} op {op:?} left={left:?} right={right:?}"),
            );
            checked += 1;
        }
    }
    assert_eq!(checked, 2048 * f32_binary_cases().len());
}

#[test]
fn generated_f32_unary_storage_graph_semantics_match_contract_matrix() {
    let values = generated_f32_values();
    let mut checked = 0usize;
    for case in 0..4096usize {
        let value = values[(case.wrapping_mul(13).wrapping_add(7)) % values.len()];
        for (op, expected) in f32_unary_cases() {
            let actual = eval_f32_un(op.clone(), value)
                .unwrap_or_else(|error| panic!("Fix: f32 {op:?} should evaluate: {error}"));
            assert_ir_value_eq(
                actual,
                expected(canonical_f32(value)),
                &format!("case {case} op {op:?} value={value:?}"),
            );
            checked += 1;
        }
    }
    assert_eq!(checked, 4096 * f32_unary_cases().len());
}

type I32BinaryExpected = fn(i32, i32) -> IrValue;
type F32BinaryExpected = fn(f32, f32) -> IrValue;
type F32UnaryExpected = fn(f32) -> IrValue;

fn i32_total_binary_cases() -> [(BinOp, I32BinaryExpected); 17] {
    [
        (BinOp::Add, |left, right| {
            IrValue::I32(left.wrapping_add(right))
        }),
        (BinOp::Sub, |left, right| {
            IrValue::I32(left.wrapping_sub(right))
        }),
        (BinOp::Mul, |left, right| {
            IrValue::I32(left.wrapping_mul(right))
        }),
        (BinOp::BitAnd, |left, right| IrValue::I32(left & right)),
        (BinOp::BitOr, |left, right| IrValue::I32(left | right)),
        (BinOp::BitXor, |left, right| IrValue::I32(left ^ right)),
        (BinOp::Shl, |left, right| {
            IrValue::I32(left.wrapping_shl(u32::from_ne_bytes(right.to_ne_bytes()) & 31))
        }),
        (BinOp::Shr, |left, right| {
            IrValue::I32(left.wrapping_shr(u32::from_ne_bytes(right.to_ne_bytes()) & 31))
        }),
        (BinOp::Eq, |left, right| IrValue::Bool(left == right)),
        (BinOp::Ne, |left, right| IrValue::Bool(left != right)),
        (BinOp::Lt, |left, right| IrValue::Bool(left < right)),
        (BinOp::Le, |left, right| IrValue::Bool(left <= right)),
        (BinOp::Gt, |left, right| IrValue::Bool(left > right)),
        (BinOp::Ge, |left, right| IrValue::Bool(left >= right)),
        (BinOp::Min, |left, right| IrValue::I32(left.min(right))),
        (BinOp::Max, |left, right| IrValue::I32(left.max(right))),
        (BinOp::SaturatingAdd, |left, right| {
            IrValue::I32(left.saturating_add(right))
        }),
    ]
}

fn f32_binary_cases() -> [(BinOp, F32BinaryExpected); 12] {
    [
        (BinOp::Add, |left, right| {
            IrValue::F32(canonical_f32(left + right))
        }),
        (BinOp::Sub, |left, right| {
            IrValue::F32(canonical_f32(left - right))
        }),
        (BinOp::Mul, |left, right| {
            IrValue::F32(canonical_f32(left * right))
        }),
        (BinOp::Div, |left, right| {
            IrValue::F32(canonical_f32(left / right))
        }),
        (BinOp::Eq, |left, right| {
            IrValue::Bool(
                left.partial_cmp(&right)
                    .is_some_and(std::cmp::Ordering::is_eq),
            )
        }),
        (BinOp::Ne, |left, right| {
            IrValue::Bool(
                left.partial_cmp(&right)
                    .is_none_or(|ordering| !ordering.is_eq()),
            )
        }),
        (BinOp::Lt, |left, right| IrValue::Bool(left < right)),
        (BinOp::Le, |left, right| IrValue::Bool(left <= right)),
        (BinOp::Gt, |left, right| IrValue::Bool(left > right)),
        (BinOp::Ge, |left, right| IrValue::Bool(left >= right)),
        (BinOp::Min, |left, right| {
            IrValue::F32(canonical_f32(left.min(right)))
        }),
        (BinOp::Max, |left, right| {
            IrValue::F32(canonical_f32(left.max(right)))
        }),
    ]
}

fn f32_unary_cases() -> [(UnOp, F32UnaryExpected); 3] {
    [
        (UnOp::Negate, |value| IrValue::F32(canonical_f32(-value))),
        (UnOp::InverseSqrt, |value| {
            IrValue::F32(canonical_f32(1.0 / value.sqrt()))
        }),
        (UnOp::Reciprocal, |value| {
            IrValue::F32(canonical_f32(1.0 / value))
        }),
    ]
}

fn eval_i32_bin(op: BinOp, left: i32, right: i32) -> Result<IrValue, vyre_foundation::Error> {
    let graph = vec![
        (NodeId(0), NodeStorage::LitI32(left)),
        (NodeId(1), NodeStorage::LitI32(right)),
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

fn eval_i32_un(op: UnOp, value: i32) -> Result<IrValue, vyre_foundation::Error> {
    let graph = vec![
        (NodeId(0), NodeStorage::LitI32(value)),
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

fn eval_f32_bin(op: BinOp, left: f32, right: f32) -> Result<IrValue, vyre_foundation::Error> {
    let graph = vec![
        (NodeId(0), NodeStorage::LitF32(left)),
        (NodeId(1), NodeStorage::LitF32(right)),
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

fn eval_f32_un(op: UnOp, value: f32) -> Result<IrValue, vyre_foundation::Error> {
    let graph = vec![
        (NodeId(0), NodeStorage::LitF32(value)),
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

fn generated_i32_values() -> Vec<i32> {
    let mut values = vec![
        i32::MIN,
        i32::MIN + 1,
        -1_000_000,
        -65_536,
        -257,
        -256,
        -129,
        -128,
        -2,
        -1,
        0,
        1,
        2,
        3,
        31,
        32,
        127,
        128,
        255,
        256,
        65_535,
        65_536,
        i32::MAX - 1,
        i32::MAX,
    ];
    let mut state = 0x6a09_e667u32;
    for index in 0..512u32 {
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        values.push(i32::from_ne_bytes(
            state.rotate_left(index & 31).to_ne_bytes(),
        ));
    }
    values.sort_unstable();
    values.dedup();
    values
}

fn generated_f32_values() -> Vec<f32> {
    let mut values = vec![
        0.0,
        -0.0,
        1.0,
        -1.0,
        2.0,
        -2.0,
        0.5,
        -0.5,
        f32::MIN_POSITIVE,
        -f32::MIN_POSITIVE,
        f32::from_bits(1),
        f32::from_bits(0x8000_0001),
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NAN,
        f32::from_bits(0x7FC0_1234),
    ];
    let mut state = 0x3c6e_f372u32;
    for index in 0..256u32 {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        values.push(f32::from_bits(state.rotate_left(index & 31)));
    }
    values
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

fn assert_ir_value_eq(actual: IrValue, expected: IrValue, context: &str) {
    match (actual, expected) {
        (IrValue::F32(actual), IrValue::F32(expected)) => assert_eq!(
            actual.to_bits(),
            expected.to_bits(),
            "Fix: f32 result bits mismatch for {context}"
        ),
        (actual, expected) => assert_eq!(
            actual, expected,
            "Fix: scalar result mismatch for {context}"
        ),
    }
}
