//! Generated property coverage for f32 comparison oracle semantics.

use proptest::prelude::*;
use vyre::ir::{BinOp, BufferDecl, DataType, Expr, Node, Program};
use vyre_reference::ieee754::canonical_f32;
use vyre_reference::reference_eval;

fn bool_output_program(expr: Expr) -> Program {
    Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::Bool).with_count(1)],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), expr)],
    )
}

fn eval_compare_word(op: BinOp, left: f32, right: f32) -> u32 {
    let expr = Expr::BinOp {
        op,
        left: Box::new(Expr::f32(left)),
        right: Box::new(Expr::f32(right)),
    };
    let outputs = reference_eval(&bool_output_program(expr), &[])
        .expect("Fix: generated f32 comparison program must evaluate");
    let bytes = outputs[0].to_bytes();
    u32::from_le_bytes(
        bytes
            .as_slice()
            .try_into()
            .expect("Fix: generated Bool output must be one u32 ABI word."),
    )
}

fn expected_compare(op: BinOp, left: f32, right: f32) -> bool {
    let left = canonical_f32(left);
    let right = canonical_f32(right);
    match op {
        BinOp::Eq => left == right,
        BinOp::Ne => left != right,
        BinOp::Lt => left < right,
        BinOp::Le => left <= right,
        BinOp::Gt => left > right,
        BinOp::Ge => left >= right,
        _ => unreachable!("comparison property only accepts comparison ops"),
    }
}

fn f32_bits_strategy() -> impl Strategy<Value = u32> {
    prop_oneof![
        any::<u32>(),
        Just(0x0000_0000),
        Just(0x8000_0000),
        Just(0x0000_0001),
        Just(0x8000_0001),
        Just(0x007f_ffff),
        Just(0x807f_ffff),
        Just(0x0080_0000),
        Just(0x8080_0000),
        Just(0x3f80_0000),
        Just(0xbf80_0000),
        Just(0x7f80_0000),
        Just(0xff80_0000),
        Just(0x7fc0_0000),
        Just(0x7fa1_2345),
        Just(0xffa1_2345),
        Just(0xffff_ffff),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10_000))]

    #[test]
    fn generated_f32_comparison_oracle_matches_unordered_ieee_contract(
        left_bits in f32_bits_strategy(),
        right_bits in f32_bits_strategy(),
    ) {
        let left = f32::from_bits(left_bits);
        let right = f32::from_bits(right_bits);
        for op in [BinOp::Eq, BinOp::Ne, BinOp::Lt, BinOp::Le, BinOp::Gt, BinOp::Ge] {
            prop_assert_eq!(
                eval_compare_word(op, left, right),
                u32::from(expected_compare(op, left, right)),
                "Fix: f32 comparison oracle diverged for {:?}, left={:#010x}, right={:#010x}.",
                op,
                left_bits,
                right_bits
            );
        }
    }
}
