use vyre_reference::value::Value;

use super::relu::linear_relu;
use super::rms_norm::{rms_norm_linear, try_rms_norm_linear};
use super::tiled::{linear_tiled, linear_tiled_reference};
use crate::test_support::byte_pack::{decode_f32 as bytes_to_f32, f32_bytes as to_f32_bytes};
use vyre::Program;

const TOLERANCE_ULP: u32 = 2;

fn ordered_bits(value: f32) -> u32 {
    let bits = value.to_bits();
    if bits & 0x8000_0000 != 0 {
        !bits
    } else {
        bits | 0x8000_0000
    }
}

fn compare_ulp(a: &[f32], b: &[f32], n: u32, in_dim: u32, out_dim: u32) {
    assert_eq!(
        a.len(),
        b.len(),
        "rms_norm_linear parity output length mismatch n={n} in_dim={in_dim} out_dim={out_dim}: {} vs {}",
        a.len(),
        b.len()
    );

    for (lane, (lhs, rhs)) in a.iter().zip(b.iter()).enumerate() {
        if lhs.is_nan() || rhs.is_nan() {
            assert_eq!(
                lhs.to_bits(),
                rhs.to_bits(),
                "NaN payload mismatch at lane {lane} n={n} in_dim={in_dim} out_dim={out_dim}"
            );
            continue;
        }
        let diff = ordered_bits(*lhs).abs_diff(ordered_bits(*rhs));
        assert!(
            diff <= TOLERANCE_ULP,
            "ULP mismatch at lane {lane} n={n} in_dim={in_dim} out_dim={out_dim}: lhs={lhs:?} rhs={rhs:?} diff={diff}"
        );
    }
}

fn output_zero_bytes(program: &Program) -> Vec<u8> {
    let output = program
        .buffers()
        .iter()
        .find(|buffer| buffer.is_output())
        .expect("Fix: linear test program must declare an output buffer.");
    let bytes = output.output_byte_range().map_or(
        (output.count() as usize) * core::mem::size_of::<u32>(),
        |range| range.end.saturating_sub(range.start),
    );
    vec![0u8; bytes]
}

fn case_data(_n: u32, in_dim: u32, out_dim: u32, eps: f32) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
    let input = (0..in_dim)
        .map(|i| (i as f32 + 1.0) * if i % 2 == 0 { 0.37 } else { -0.41 })
        .collect::<Vec<_>>();
    let weights = (0..(in_dim * out_dim))
        .map(|i| (i as f32) * 0.011 + 0.23)
        .collect::<Vec<_>>();
    let bias = (0..out_dim)
        .map(|i| (i as f32) * 0.17 + eps)
        .collect::<Vec<_>>();
    (input, weights, bias)
}

fn linear_reference(
    input: &[f32],
    normalized: &[f32],
    weights: &[f32],
    bias: &[f32],
    out_dim: u32,
    in_dim: u32,
    n: u32,
    eps: f32,
) -> Vec<f32> {
    assert_eq!(
        normalized.len(),
        n as usize,
        "linear_reference must receive exactly n normalized values: got {} vs {}",
        normalized.len(),
        n
    );
    let inv_scale =
        1.0_f32 / ((normalized.iter().map(|v| v * v).sum::<f32>() / (n as f32)) + eps).sqrt();
    let mut output = bias.to_vec();
    for j in 0..out_dim as usize {
        let mut acc = output[j];
        for k in 0..in_dim as usize {
            acc += input[k] * inv_scale * weights[k * out_dim as usize + j];
        }
        output[j] = acc;
    }
    output
}

fn parity_case(n: u32, in_dim: u32, out_dim: u32) {
    let eps = 1e-5_f32;
    let (input, weights, bias) = case_data(n, in_dim, out_dim, eps);

    let fused = rms_norm_linear("input", "w", "b", "out", n, in_dim, out_dim, eps);
    let fused_inputs = vec![
        Value::from(to_f32_bytes(&input)),
        Value::from(to_f32_bytes(&weights)),
        Value::from(to_f32_bytes(&bias)),
        Value::from(vec![0u8; out_dim as usize * core::mem::size_of::<f32>()]),
    ];
    let fused_outputs = vyre_reference::reference_eval(&fused, &fused_inputs).expect(
        "Fix: fused rms_norm_linear must execute; restore this invariant before continuing.",
    );
    let fused_out = bytes_to_f32(&fused_outputs[0].to_bytes());
    let normalized = &input[0..n as usize];
    let expected = linear_reference(&input, normalized, &weights, &bias, out_dim, in_dim, n, eps);
    compare_ulp(&fused_out, &expected, n, in_dim, out_dim);
}

mod relu_builder;
mod rms_norm;
mod tiled;
