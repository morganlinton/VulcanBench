//! Generated CPU-reference matrix for public f32 hardware intrinsic builders.
//!
//! The f32 Cat-C surface has precision-sensitive contracts: FMA must use
//! single-round `mul_add`, and inverse sqrt must clamp hostile inputs before
//! lowering. This matrix covers edge values and generated lanes beyond one
//! workgroup.

use vyre_foundation::ir::Program;
use vyre_reference::value::Value;

fn pack(values: &[f32]) -> Vec<u8> {
    vyre_primitives::wire::pack_f32_slice(values)
}

fn run(program: &Program, inputs: Vec<Vec<u8>>) -> Vec<u8> {
    let values: Vec<Value> = inputs
        .into_iter()
        .map(|bytes| Value::Bytes(bytes.into()))
        .collect();
    let outputs = vyre_reference::reference_eval(program, &values)
        .expect("Fix: f32 hardware intrinsic builder must execute on the CPU oracle.");
    assert_eq!(
        outputs.len(),
        1,
        "Fix: f32 hardware intrinsic emits one output buffer."
    );
    outputs[0].to_bytes()
}

fn generated_finite(len: usize, seed: u32) -> Vec<f32> {
    let edge = [
        -8.0,
        -1.0,
        -0.0,
        0.0,
        f32::MIN_POSITIVE,
        0.25,
        0.5,
        1.0,
        2.0,
        4.0,
        16.0,
        f32::MAX,
    ];
    let mut state = seed;
    (0..len)
        .map(|idx| {
            if idx < edge.len() {
                edge[idx]
            } else {
                state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                let unit = f32::from_bits((state >> 9) | 0x3f00_0000) - 1.0;
                if idx & 1 == 0 {
                    unit
                } else {
                    -unit
                }
            }
        })
        .collect()
}

fn generated_inverse_sqrt_inputs(len: usize, seed: u32) -> Vec<f32> {
    let hostile = [
        f32::NAN,
        f32::INFINITY,
        f32::NEG_INFINITY,
        -1.0,
        -0.0,
        0.0,
        f32::from_bits(1),
        f32::MIN_POSITIVE,
        0.25,
        1.0,
        4.0,
        16.0,
    ];
    let mut values = generated_finite(len, seed)
        .into_iter()
        .map(|value| value.abs() + 0.01)
        .collect::<Vec<_>>();
    for (idx, value) in hostile.into_iter().enumerate().take(values.len()) {
        values[idx] = value;
    }
    values
}

#[test]
fn generated_fma_f32_matrix_matches_mul_add_bits() {
    let lengths = [1usize, 2, 3, 4, 31, 32, 63, 64, 65, 257, 1024, 2048];
    let mut checked_lanes = 0usize;

    for &len in &lengths {
        let a = generated_finite(len, 0x0f1a_a011 ^ len as u32);
        let b = generated_finite(len, 0x0f1a_a012 ^ len as u32);
        let c = generated_finite(len, 0x0f1a_a013 ^ len as u32);
        let program =
            vyre_intrinsics::hardware::fma_f32::fma_f32::fma_f32("a", "b", "c", "out", len as u32);
        let got = run(
            &program,
            vec![pack(&a), pack(&b), pack(&c), vec![0u8; len.max(1) * 4]],
        );
        let expected = a
            .iter()
            .zip(b.iter())
            .zip(c.iter())
            .map(|((&x, &y), &z)| x.mul_add(y, z))
            .collect::<Vec<_>>();
        assert_eq!(got, pack(&expected), "fma_f32 failed for len {len}");
        checked_lanes += len;
    }

    assert_eq!(checked_lanes, lengths.iter().sum::<usize>());
}

#[test]
fn generated_inverse_sqrt_f32_matrix_matches_clamped_host_semantics() {
    let lengths = [1usize, 2, 3, 4, 31, 32, 63, 64, 65, 257, 1024, 2048];
    let mut checked_lanes = 0usize;

    for &len in &lengths {
        let input = generated_inverse_sqrt_inputs(len, 0x0f1a_b005 ^ len as u32);
        let program =
            vyre_intrinsics::hardware::inverse_sqrt_f32::inverse_sqrt_f32::inverse_sqrt_f32(
                "input", "out", len as u32,
            );
        let got = run(&program, vec![pack(&input), vec![0u8; len.max(1) * 4]]);
        let expected = input
            .iter()
            .map(|&x| {
                let safe_x = if x.is_finite() && x > f32::MIN_POSITIVE {
                    x
                } else {
                    f32::MIN_POSITIVE
                };
                1.0 / safe_x.sqrt()
            })
            .collect::<Vec<_>>();
        assert_eq!(
            got,
            pack(&expected),
            "inverse_sqrt_f32 failed for len {len}"
        );
        checked_lanes += len;
    }

    assert_eq!(checked_lanes, lengths.iter().sum::<usize>());
}
