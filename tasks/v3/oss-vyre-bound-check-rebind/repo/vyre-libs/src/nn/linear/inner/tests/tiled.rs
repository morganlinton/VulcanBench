//! Integration test crate for the containing Vyre package.

use super::*;

#[test]
fn linear_tiled_matches_scalar_linear_with_bias_and_tail_tile() {
    let in_dim = 37_u32;
    let out_dim = 65_u32;
    let tile = 16_u32;
    let x = (0..in_dim)
        .map(|i| i.wrapping_mul(3).wrapping_add(1))
        .collect::<Vec<_>>();
    let w = (0..(in_dim * out_dim))
        .map(|i| i.wrapping_mul(5).wrapping_add(7))
        .collect::<Vec<_>>();
    let b = (0..out_dim)
        .map(|i| i.wrapping_mul(11).wrapping_add(13))
        .collect::<Vec<_>>();
    let to_bytes = vyre_primitives::wire::pack_u32_slice;
    let program = linear_tiled("x", "w", "b", "out", in_dim, out_dim, tile)
        .expect("Fix: linear_tiled must accept positive dimensions.");
    let outputs = vyre_reference::reference_eval(
        &program,
        &[
            Value::from(to_bytes(&x)),
            Value::from(to_bytes(&w)),
            Value::from(to_bytes(&b)),
            Value::from(output_zero_bytes(&program)),
        ],
    )
    .expect("Fix: linear_tiled must execute in the reference interpreter.");
    let actual = vyre_primitives::wire::decode_u32_le_bytes_all(&outputs[0].to_bytes());
    let expected = (0..out_dim as usize)
        .map(|j| {
            let mut acc = b[j];
            for k in 0..in_dim as usize {
                acc = acc.wrapping_add(x[k].wrapping_mul(w[k * out_dim as usize + j]));
            }
            acc
        })
        .collect::<Vec<_>>();
    assert_eq!(actual, expected);
}

#[test]
fn linear_tiled_accepts_logical_output_fixture_for_padded_storage() {
    let program = linear_tiled("x", "w", "b", "out", 4, 4, 2)
        .expect("Fix: linear_tiled must build for a 4x4 fixture.");
    let output = program
        .buffers()
        .iter()
        .find(|buffer| buffer.is_output())
        .expect("Fix: linear_tiled must declare an output buffer.");

    assert!(
        output.count() > 4,
        "Fix: linear_tiled should keep padded storage for cooperative tile launch geometry."
    );
    assert_eq!(
        output.output_byte_range(),
        Some(0..16),
        "Fix: linear_tiled must expose only the logical output bytes."
    );

    let x = crate::test_support::byte_pack::u32_bytes(&(0..4).collect::<Vec<_>>());
    let w = crate::test_support::byte_pack::u32_bytes(&(0..16).collect::<Vec<_>>());
    let bias = crate::test_support::byte_pack::u32_bytes(&[0, 0, 0, 0]);
    let outputs = vyre_reference::reference_eval(
        &program,
        &[
            Value::from(x),
            Value::from(w),
            Value::from(bias),
            Value::from(vec![0u8; 16]),
        ],
    )
    .expect("Fix: reference interpreter must pad output backing storage internally.");

    assert_eq!(
        outputs[0].to_bytes(),
        crate::test_support::byte_pack::u32_bytes(&[56, 62, 68, 74])
    );
}

#[test]
fn linear_tiled_matches_reference_on_adversarial_shapes() {
    let shapes = [
        (1_u32, 1_u32, 1_u32),
        (3, 5, 7),
        (15, 16, 17),
        (31, 32, 33),
        (63, 64, 65),
        (127, 128, 129),
        (255, 256, 257),
    ];
    let to_bytes = vyre_primitives::wire::pack_u32_slice;

    for (in_dim, out_dim, tile) in shapes {
        let x: Vec<u32> = (0..in_dim)
            .map(|i| i.wrapping_mul(3).wrapping_add(1))
            .collect();
        let w: Vec<u32> = (0..(in_dim * out_dim))
            .map(|i| i.wrapping_mul(5).wrapping_add(7))
            .collect();
        let b: Vec<u32> = (0..out_dim)
            .map(|i| i.wrapping_mul(11).wrapping_add(13))
            .collect();

        let opt = linear_tiled("x", "w", "b", "out", in_dim, out_dim, tile)
            .expect("Fix: linear_tiled must accept positive dimensions.");
        let ref_ = linear_tiled_reference("x", "w", "b", "out", in_dim, out_dim, tile)
            .expect("Fix: linear_tiled_reference must accept positive dimensions.");

        let opt_outputs = vyre_reference::reference_eval(
            &opt,
            &[
                Value::from(to_bytes(&x)),
                Value::from(to_bytes(&w)),
                Value::from(to_bytes(&b)),
                Value::from(output_zero_bytes(&opt)),
            ],
        )
        .expect("Fix: linear_tiled must execute.");
        let ref_outputs = vyre_reference::reference_eval(
            &ref_,
            &[
                Value::from(to_bytes(&x)),
                Value::from(to_bytes(&w)),
                Value::from(to_bytes(&b)),
                Value::from(vec![0u8; out_dim as usize * 4]),
            ],
        )
        .expect("Fix: linear_tiled_reference must execute.");

        assert_eq!(
            opt_outputs[0].to_bytes(),
            ref_outputs[0].to_bytes(),
            "linear_tiled must match linear_tiled_reference for (in_dim={in_dim}, out_dim={out_dim}, tile={tile})"
        );
    }
}

#[test]
fn linear_tiled_matches_reference_on_boundary_values() {
    let in_dim = 32_u32;
    let out_dim = 48_u32;
    let tile = 16_u32;
    let to_bytes = vyre_primitives::wire::pack_u32_slice;

    let boundary_cases: Vec<Vec<u32>> = vec![
        vec![0; in_dim as usize],
        vec![1; in_dim as usize],
        vec![u32::MAX; in_dim as usize],
        (0..in_dim).collect(),
    ];

    for x in &boundary_cases {
        let w: Vec<u32> = (0..(in_dim * out_dim))
            .map(|i| if i % 2 == 0 { 0 } else { 1 })
            .collect();
        let b: Vec<u32> = (0..out_dim).collect();

        let opt = linear_tiled("x", "w", "b", "out", in_dim, out_dim, tile)
            .expect("Fix: linear_tiled must accept positive dimensions.");
        let ref_ = linear_tiled_reference("x", "w", "b", "out", in_dim, out_dim, tile)
            .expect("Fix: linear_tiled_reference must accept positive dimensions.");

        let opt_outputs = vyre_reference::reference_eval(
            &opt,
            &[
                Value::from(to_bytes(x)),
                Value::from(to_bytes(&w)),
                Value::from(to_bytes(&b)),
                Value::from(output_zero_bytes(&opt)),
            ],
        )
        .expect("Fix: linear_tiled must execute.");
        let ref_outputs = vyre_reference::reference_eval(
            &ref_,
            &[
                Value::from(to_bytes(x)),
                Value::from(to_bytes(&w)),
                Value::from(to_bytes(&b)),
                Value::from(vec![0u8; out_dim as usize * 4]),
            ],
        )
        .expect("Fix: linear_tiled_reference must execute.");

        assert_eq!(
            opt_outputs[0].to_bytes(),
            ref_outputs[0].to_bytes(),
            "linear_tiled must match linear_tiled_reference for boundary input {:?}",
            x
        );
    }
}
