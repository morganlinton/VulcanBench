//! Test crate.

#![cfg(all(feature = "nn-attention", feature = "nn-norm"))]
#![allow(deprecated)]
use proptest::prelude::*;
use vyre::ir::Program;
use vyre_foundation::optimizer::pre_lowering::optimize;
use vyre_libs::harness::{all_entries, OpEntry};
use vyre_reference::value::Value;

fn entry(id: &'static str) -> &'static OpEntry {
    all_entries()
        .find(|entry| entry.id == id)
        .unwrap_or_else(|| panic!("Fix: missing OpEntry for {id}"))
}

fn bytes_from_f32(values: &[f32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

fn output_bytes(program: &Program, inputs: &[Vec<u8>]) -> Vec<Vec<u8>> {
    let values = inputs.iter().cloned().map(Value::from).collect::<Vec<_>>();
    vyre_reference::reference_eval(program, &values)
        .unwrap_or_else(|error| panic!("Fix: reference execution failed: {error}"))
        .into_iter()
        .map(|value| value.to_bytes())
        .collect()
}

fn harness_path_outputs(entry: &'static OpEntry, inputs: &[Vec<u8>]) -> Vec<Vec<u8>> {
    let program = (entry.build)();
    let errors = vyre::ir::validate(&program);
    assert!(
        errors.is_empty(),
        "Fix: {} failed validation on adversarial f32 input: {:?}",
        entry.id,
        errors
            .into_iter()
            .map(|error| error.message().to_string())
            .collect::<Vec<_>>()
    );
    let wire = program
        .to_wire()
        .unwrap_or_else(|error| panic!("Fix: {} failed wire encode: {error}", entry.id));
    let decoded = Program::from_wire(&wire)
        .unwrap_or_else(|error| panic!("Fix: {} failed wire decode: {error}", entry.id));
    let optimized_once = optimize(decoded);
    let optimized_twice = optimize(optimized_once.clone());
    assert_eq!(
        optimized_once, optimized_twice,
        "Fix: {} optimize() must be idempotent on adversarial f32 input",
        entry.id
    );
    output_bytes(&optimized_once, inputs)
}

fn special_f32() -> impl Strategy<Value = f32> {
    prop_oneof![
        Just(f32::NAN),
        Just(f32::from_bits(0x7fc0_0001)),
        Just(f32::INFINITY),
        Just(f32::NEG_INFINITY),
        Just(0.0f32),
        Just(-0.0f32),
        Just(f32::MAX),
        Just(f32::MIN_POSITIVE),
        Just(f32::from_bits(1)),
        any::<u32>().prop_map(f32::from_bits),
    ]
}

fn softmax_case() -> impl Strategy<Value = [f32; 4]> {
    prop::array::uniform4(special_f32())
}

fn attention_case() -> impl Strategy<Value = [f32; 8]> {
    prop::array::uniform8(special_f32())
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 128, .. ProptestConfig::default() })]

    #[test]
    fn softmax_special_values_match_harness(input in softmax_case()) {
        let inputs = vec![
            bytes_from_f32(&input),
            vec![0u8; input.len() * core::mem::size_of::<f32>()],
        ];
        let direct = std::panic::catch_unwind(|| output_bytes(&(entry("vyre-libs::nn::softmax").build)(), &inputs))
            .expect("Fix: softmax reference path must not panic on NaN/Inf/subnormal inputs");
        let harness = std::panic::catch_unwind(|| harness_path_outputs(entry("vyre-libs::nn::softmax"), &inputs))
            .expect("Fix: softmax universal harness path must not panic on NaN/Inf/subnormal inputs");
        prop_assert_eq!(direct, harness);
    }

    #[test]
    fn layer_norm_special_values_match_harness(input in softmax_case()) {
        let inputs = vec![
            bytes_from_f32(&input),
            vec![0u8; input.len() * core::mem::size_of::<f32>()],
        ];
        let direct = std::panic::catch_unwind(|| output_bytes(&(entry("vyre-libs::nn::layer_norm").build)(), &inputs))
            .expect("Fix: layer_norm reference path must not panic on NaN/Inf/subnormal inputs");
        let harness = std::panic::catch_unwind(|| harness_path_outputs(entry("vyre-libs::nn::layer_norm"), &inputs))
            .expect("Fix: layer_norm universal harness path must not panic on NaN/Inf/subnormal inputs");
        prop_assert_eq!(direct, harness);
    }

    #[test]
    fn attention_special_values_match_harness(q in attention_case(), k in attention_case(), v in attention_case()) {
        let inputs = vec![
            bytes_from_f32(&q),
            bytes_from_f32(&k),
            bytes_from_f32(&v),
            vec![0u8; q.len() * core::mem::size_of::<f32>()],
        ];
        let direct = std::panic::catch_unwind(|| output_bytes(&(entry("vyre-libs::nn::attention").build)(), &inputs))
            .expect("Fix: attention reference path must not panic on NaN/Inf/subnormal inputs");
        let harness = std::panic::catch_unwind(|| harness_path_outputs(entry("vyre-libs::nn::attention"), &inputs))
            .expect("Fix: attention universal harness path must not panic on NaN/Inf/subnormal inputs");
        prop_assert_eq!(direct, harness);
    }
}
