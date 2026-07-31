//! Focused 5090 conform checks for Cat-A fixture-bearing ops.

use std::sync::OnceLock;

use vyre::ir::{BufferAccess, Program};
use vyre_driver::{DispatchConfig, VyreBackend};
use vyre_driver_wgpu::WgpuBackend;
use vyre_libs::harness::{all_entries, fp_contract, OpEntry};

fn backend() -> &'static WgpuBackend {
    static BACKEND: OnceLock<WgpuBackend> = OnceLock::new();
    BACKEND.get_or_init(|| {
        let adapters = vyre_driver_wgpu::runtime::device::enumerate_adapters();
        assert!(
            !adapters.is_empty(),
            "Fix: cat_a_conform requires a live GPU adapter; this host is expected to expose the RTX 5090."
        );
        WgpuBackend::acquire().expect("Fix: cat_a_conform must acquire the live 5090 backend")
    })
}

fn entry(id: &'static str) -> &'static OpEntry {
    all_entries()
        .find(|entry| entry.id == id)
        .unwrap_or_else(|| panic!("Fix: missing OpEntry for {id}"))
}

fn assert_gpu_matches_fixture(id: &'static str) {
    let entry = entry(id);
    let program = (entry.build)();
    let config = dispatch_config_for_fixture(&program);
    let inputs = (entry.test_inputs.expect("Fix: test_inputs required"))();
    let expected = (entry
        .expected_output
        .expect("Fix: expected_output required"))();
    assert_eq!(
        inputs.len(),
        expected.len(),
        "Fix: fixture case count mismatch for {id}"
    );
    assert!(
        !inputs.is_empty(),
        "Fix: {id} has empty test_inputs; GPU conform fixtures must execute at least one case."
    );
    assert!(
        !expected.is_empty(),
        "Fix: {id} has empty expected_output; GPU conform fixtures must provide an oracle."
    );

    for (case_index, (input_set, expected_outputs)) in
        inputs.iter().zip(expected.iter()).enumerate()
    {
        let outputs = backend()
            .dispatch(&program, input_set, &config)
            .unwrap_or_else(|error| {
                panic!("Fix: 5090 dispatch failed for {id} case {case_index}: {error}")
            });
        let tolerance = fp_contract::effective_tolerance(entry.id, &program);
        assert_outputs_match(entry, tolerance, &outputs, expected_outputs, case_index);
    }
}

fn dispatch_config_for_fixture(program: &Program) -> DispatchConfig {
    let mut config = DispatchConfig::default();
    let workgroup = program.workgroup_size();
    if workgroup[1] == 1 && workgroup[2] == 1 {
        return config;
    }

    let lanes = u64::from(workgroup[0])
        .saturating_mul(u64::from(workgroup[1]))
        .saturating_mul(u64::from(workgroup[2]));
    let max_writable_count = program
        .buffers()
        .iter()
        .filter(|decl| matches!(decl.access(), BufferAccess::ReadWrite) || decl.is_output())
        .map(|decl| u64::from(decl.count()))
        .max()
        .unwrap_or(1);
    assert!(
        max_writable_count <= lanes,
        "Fix: Cat-A fixture with non-1D workgroup_size {workgroup:?} needs explicit multi-workgroup grid; {max_writable_count} writable elements exceed {lanes} lanes"
    );
    config.grid_override = Some([1, 1, 1]);
    config
}

fn assert_outputs_match(
    entry: &OpEntry,
    tolerance: u32,
    actual: &[Vec<u8>],
    expected: &[Vec<u8>],
    case_index: usize,
) {
    assert_eq!(
        actual.len(),
        expected.len(),
        "Fix: output buffer count mismatch for {} case {}",
        entry.id,
        case_index
    );
    for (buffer_index, (actual_bytes, expected_bytes)) in
        actual.iter().zip(expected.iter()).enumerate()
    {
        assert_eq!(
            actual_bytes.len(),
            expected_bytes.len(),
            "Fix: output byte count mismatch for {} case {} buffer {}",
            entry.id,
            case_index,
            buffer_index
        );
        if tolerance == 0 {
            assert_eq!(
                actual_bytes, expected_bytes,
                "GPU witness drift for {} case {} buffer {}",
                entry.id, case_index, buffer_index
            );
            continue;
        }

        assert!(
            f32_buffer_matches(actual_bytes, expected_bytes, tolerance),
            "GPU witness drift for {} case {} buffer {} exceeded {} ULPs",
            entry.id,
            case_index,
            buffer_index,
            tolerance
        );
    }
}

fn f32_buffer_matches(actual: &[u8], expected: &[u8], max_ulp: u32) -> bool {
    actual
        .chunks_exact(4)
        .zip(expected.chunks_exact(4))
        .all(|(left, right)| {
            let left = f32::from_bits(u32::from_le_bytes(left.try_into().expect("4 bytes")));
            let right = f32::from_bits(u32::from_le_bytes(right.try_into().expect("4 bytes")));
            left.to_bits() == right.to_bits()
                || ulp_distance(left, right).is_some_and(|distance| distance <= max_ulp)
        })
}

fn ulp_distance(left: f32, right: f32) -> Option<u32> {
    if left.is_nan() || right.is_nan() {
        return None;
    }
    let left = ordered_bits(left);
    let right = ordered_bits(right);
    Some(left.abs_diff(right))
}

fn ordered_bits(value: f32) -> u32 {
    let bits = value.to_bits();
    if bits & 0x8000_0000 != 0 {
        !bits
    } else {
        bits | 0x8000_0000
    }
}

#[test]
fn matmul_tiled_matches_fixture_on_5090() {
    assert_gpu_matches_fixture("vyre-libs::math::matmul_tiled");
}

#[test]
fn softmax_matches_fixture_on_5090() {
    assert_gpu_matches_fixture("vyre-libs::nn::softmax");
}

#[test]
fn layer_norm_matches_fixture_on_5090() {
    assert_gpu_matches_fixture("vyre-libs::nn::layer_norm");
}

#[test]
fn attention_matches_fixture_on_5090() {
    assert_gpu_matches_fixture("vyre-libs::nn::attention");
}
