//! Test crate.

#![cfg(feature = "logical")]
#![allow(deprecated)]
use proptest::prelude::*;
use vyre_reference::value::Value;

fn bytes(words: &[u32]) -> Vec<u8> {
    words.iter().flat_map(|word| word.to_le_bytes()).collect()
}

fn run(program: &vyre::Program, a: &[u32; 4], b: &[u32; 4]) -> [u32; 4] {
    let outputs = vyre_reference::reference_eval(
        program,
        &[
            Value::from(bytes(a)),
            Value::from(bytes(b)),
            Value::from(vec![0u8; 16]),
        ],
    )
    .unwrap_or_else(|error| panic!("Fix: logical reference run failed: {error}"));
    let raw = outputs[0].to_bytes();
    std::array::from_fn(|index| {
        let offset = index * 4;
        u32::from_le_bytes(raw[offset..offset + 4].try_into().unwrap())
    })
}

fn op_expected<F>(a: &[u32; 4], b: &[u32; 4], f: F) -> [u32; 4]
where
    F: Fn(u32, u32) -> u32,
{
    std::array::from_fn(|index| f(a[index], b[index]))
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 256, .. ProptestConfig::default() })]

    #[test]
    fn and_matches_bitwise_semantics(a in any::<[u32; 4]>(), b in any::<[u32; 4]>()) {
        prop_assert_eq!(
            run(&vyre_libs::logical::and("a", "b", "out", 4), &a, &b),
            op_expected(&a, &b, |lhs, rhs| lhs & rhs)
        );
    }

    #[test]
    fn or_matches_bitwise_semantics(a in any::<[u32; 4]>(), b in any::<[u32; 4]>()) {
        prop_assert_eq!(
            run(&vyre_libs::logical::or("a", "b", "out", 4), &a, &b),
            op_expected(&a, &b, |lhs, rhs| lhs | rhs)
        );
    }

    #[test]
    fn xor_matches_bitwise_semantics(a in any::<[u32; 4]>(), b in any::<[u32; 4]>()) {
        prop_assert_eq!(
            run(&vyre_libs::logical::xor("a", "b", "out", 4), &a, &b),
            op_expected(&a, &b, |lhs, rhs| lhs ^ rhs)
        );
    }

    #[test]
    fn nand_matches_bitwise_semantics(a in any::<[u32; 4]>(), b in any::<[u32; 4]>()) {
        prop_assert_eq!(
            run(&vyre_libs::logical::nand("a", "b", "out", 4), &a, &b),
            op_expected(&a, &b, |lhs, rhs| !(lhs & rhs))
        );
    }

    #[test]
    fn nor_matches_bitwise_semantics(a in any::<[u32; 4]>(), b in any::<[u32; 4]>()) {
        prop_assert_eq!(
            run(&vyre_libs::logical::nor("a", "b", "out", 4), &a, &b),
            op_expected(&a, &b, |lhs, rhs| !(lhs | rhs))
        );
    }
}
