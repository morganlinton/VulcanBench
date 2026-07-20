//! CPU witness pins for fixture-bearing vyre-libs ops.

#![allow(deprecated)]
#![cfg(all(
    feature = "nn-attention",
    feature = "nn-norm",
    feature = "matching-dfa",
    feature = "crypto-blake3"
))]

use vyre_libs::harness::all_entries;
use vyre_reference::value::Value;

fn entry(id: &'static str) -> &'static vyre_libs::harness::OpEntry {
    all_entries()
        .find(|entry| entry.id == id)
        .unwrap_or_else(|| panic!("Fix: missing OpEntry for {id}"))
}

fn assert_entry_matches_declared_witness(id: &'static str) {
    let entry = entry(id);
    let inputs = (entry.test_inputs.expect("Fix: test_inputs required"))();
    let expected = (entry
        .expected_output
        .expect("Fix: expected_output required"))();
    assert_entry_matches_cases(id, entry.build, inputs, expected);
}

fn assert_entry_matches_cases(
    id: &'static str,
    build: fn() -> vyre::Program,
    inputs: Vec<Vec<Vec<u8>>>,
    expected: Vec<Vec<Vec<u8>>>,
) {
    assert_eq!(
        inputs.len(),
        expected.len(),
        "Fix: witness vector count mismatch for {id}"
    );
    for (case_index, (input_set, expected_outputs)) in
        inputs.iter().zip(expected.iter()).enumerate()
    {
        let outputs = vyre_reference::reference_eval(
            &build(),
            &input_set
                .iter()
                .cloned()
                .map(Value::from)
                .collect::<Vec<_>>(),
        )
        .unwrap_or_else(|error| panic!("Fix: reference run failed for {id}: {error}"))
        .into_iter()
        .map(|value| value.to_bytes())
        .collect::<Vec<_>>();
        assert_eq!(
            outputs, *expected_outputs,
            "CPU witness drift for {id} case {case_index}"
        );
    }
}

fn assert_entry_matches_pinned_witness(id: &'static str, expected: Vec<Vec<Vec<u8>>>) {
    let entry = entry(id);
    let inputs = (entry.test_inputs.expect("Fix: test_inputs required"))();
    assert_entry_matches_cases(id, entry.build, inputs, expected);
}

#[test]
fn softmax_cpu_witness_is_pinned() {
    assert_entry_matches_declared_witness("vyre-libs::nn::softmax");
}

#[test]
fn attention_cpu_witness_is_pinned() {
    assert_entry_matches_declared_witness("vyre-libs::nn::attention");
}

#[test]
fn layer_norm_cpu_witness_is_pinned() {
    assert_entry_matches_declared_witness("vyre-libs::nn::layer_norm");
}

#[test]
fn matmul_tiled_cpu_witness_is_pinned() {
    assert_entry_matches_declared_witness("vyre-libs::math::matmul_tiled");
}

#[test]
fn broadcast_cpu_witness_is_pinned() {
    assert_entry_matches_declared_witness("vyre-libs::math::broadcast");
}

#[test]
fn relu_cpu_witness_is_pinned() {
    assert_entry_matches_declared_witness("vyre-libs::nn::relu");
}

#[test]
fn linear_cpu_witness_is_pinned() {
    assert_entry_matches_declared_witness("vyre-libs::nn::linear");
}

#[test]
fn fnv1a32_cpu_witness_is_pinned() {
    assert_entry_matches_declared_witness("vyre-libs::hash::fnv1a32");
}

#[test]
fn blake3_cpu_witness_is_pinned() {
    assert_entry_matches_declared_witness("vyre-libs::hash::blake3_compress");
}

#[test]
fn aho_corasick_cpu_witness_is_pinned() {
    assert_entry_matches_declared_witness("vyre-libs::matching::aho_corasick");
}

#[test]
fn adler32_cpu_witness_is_pinned() {
    assert_entry_matches_pinned_witness(
        "vyre-libs::hash::adler32",
        vec![vec![vec![0x27, 0x01, 0x4d, 0x02]]],
    );
}

#[test]
fn crc32_cpu_witness_is_pinned() {
    assert_entry_matches_pinned_witness(
        "vyre-libs::hash::crc32",
        vec![vec![vec![0xc2, 0x41, 0x24, 0x35]]],
    );
}

#[test]
fn fnv1a64_cpu_witness_is_pinned() {
    assert_entry_matches_pinned_witness(
        "vyre-libs::hash::fnv1a64",
        vec![vec![vec![0x4b, 0x57, 0x41, 0x05, 0x19, 0xa2, 0x1f, 0xe7]]],
    );
}

#[test]
fn matmul_cpu_witness_is_pinned() {
    assert_entry_matches_pinned_witness(
        "vyre-libs::math::matmul",
        vec![vec![vec![
            0x3e, 0x00, 0x00, 0x00, 0x44, 0x00, 0x00, 0x00, 0x4a, 0x00, 0x00, 0x00, 0x50, 0x00,
            0x00, 0x00, 0xae, 0x00, 0x00, 0x00, 0xc4, 0x00, 0x00, 0x00, 0xda, 0x00, 0x00, 0x00,
            0xf0, 0x00, 0x00, 0x00, 0x1e, 0x01, 0x00, 0x00, 0x44, 0x01, 0x00, 0x00, 0x6a, 0x01,
            0x00, 0x00, 0x90, 0x01, 0x00, 0x00, 0x8e, 0x01, 0x00, 0x00, 0xc4, 0x01, 0x00, 0x00,
            0xfa, 0x01, 0x00, 0x00, 0x30, 0x02, 0x00, 0x00,
        ]]],
    );
}

#[test]
fn silu_cpu_witness_is_pinned() {
    assert_entry_matches_pinned_witness(
        "vyre-libs::nn::silu",
        vec![vec![vec![
            0x00, 0x00, 0x00, 0x00, 0xa8, 0x26, 0x3b, 0x3f, 0xb1, 0xb2, 0x89, 0xbe, 0xea, 0x7b,
            0xe1, 0x3f,
        ]]],
    );
}

#[test]
fn substring_cpu_witness_is_pinned() {
    assert_entry_matches_pinned_witness(
        "vyre-libs::matching::substring_search",
        vec![
            vec![vec![
                0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00,
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x00, 0x00, 0x00,
            ]],
            vec![vec![
                0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00,
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x00, 0x00, 0x00,
            ]],
        ],
    );
}
