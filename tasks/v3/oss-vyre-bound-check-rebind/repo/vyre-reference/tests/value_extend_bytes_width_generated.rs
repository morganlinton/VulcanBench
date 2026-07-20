//! Generated coverage for append-oriented `Value::extend_bytes_width`.
//!
//! The reference interpreter uses this path to avoid allocating one temporary
//! byte vector per argument. These tests pin the allocation-free encoder to the
//! same byte contract as `to_bytes_width`, including non-empty destinations and
//! batched argument packing.

use std::sync::Arc;

use vyre_reference::value::Value;

#[test]
fn generated_extend_bytes_width_preserves_existing_prefix_for_8192_cases() {
    let mut assertions = 0usize;
    for seed in 0u32..8192 {
        let value = generated_value(seed);
        let width = generated_width(seed);
        let mut out = generated_prefix(seed);
        let prefix_len = out.len();
        let expected_suffix = value.to_bytes_width(width);

        value
            .extend_bytes_width(width, &mut out)
            .expect("Fix: bounded generated reference values must encode without overflow.");

        assert_eq!(
            &out[..prefix_len],
            generated_prefix(seed).as_slice(),
            "Fix: allocation-free value encoding must not mutate existing destination bytes at seed {seed}."
        );
        assert_eq!(
            &out[prefix_len..],
            expected_suffix.as_slice(),
            "Fix: allocation-free value encoding must append the exact allocating encoding at seed {seed} width {width}."
        );
        assertions += 2;
    }
    assert_eq!(assertions, 8192 * 2);
}

#[test]
fn generated_batched_extend_matches_concatenated_allocating_encoding() {
    let mut assertions = 0usize;
    for seed in 0u32..4096 {
        let mut out = generated_prefix(seed ^ 0xCAFE_BABE);
        let prefix = out.clone();
        let mut expected = prefix.clone();
        for lane in 0..8u32 {
            let value = generated_value(seed.wrapping_mul(17).wrapping_add(lane));
            let width = generated_width(seed.rotate_left(lane + 1) ^ lane);
            value
                .extend_bytes_width(width, &mut out)
                .expect("Fix: generated batched values must encode without overflow.");
            expected.extend(value.to_bytes_width(width));
        }

        assert_eq!(
            out, expected,
            "Fix: allocation-free batched argument packing must equal concatenated allocating encodings at seed {seed}."
        );
        assert_eq!(
            &out[..prefix.len()],
            prefix.as_slice(),
            "Fix: allocation-free batched argument packing must preserve the caller prefix at seed {seed}."
        );
        assertions += 2;
    }
    assert_eq!(assertions, 4096 * 2);
}

#[test]
fn generated_nested_arrays_obey_declared_width_truncation_and_padding() {
    let mut assertions = 0usize;
    for seed in 0u32..2048 {
        let value = Value::Array(vec![
            generated_value(seed),
            Value::Array(vec![
                generated_value(seed ^ 0xA5A5_5A5A),
                generated_value(seed.rotate_left(9)),
            ]),
            Value::Bytes(Arc::from(generated_bytes(seed ^ 0x5A5A_A5A5))),
        ]);
        for width in [0usize, 1, 3, 4, 8, 17, 32, 63] {
            let mut out = Vec::with_capacity(width.max(value.to_bytes().len()));
            value
                .extend_bytes_width(width, &mut out)
                .expect("Fix: generated nested arrays must encode without overflow.");
            assert_eq!(
                out,
                value.to_bytes_width(width),
                "Fix: nested array allocation-free encoding must mirror declared-width truncation and padding at seed {seed} width {width}."
            );
            assertions += 1;
        }
    }
    assert_eq!(assertions, 2048 * 8);
}

fn generated_value(seed: u32) -> Value {
    match seed % 7 {
        0 => Value::U32(mix32(seed)),
        1 => Value::I32(mix32(seed ^ 0x1357_9BDF) as i32),
        2 => Value::U64((u64::from(mix32(seed)) << 32) | u64::from(mix32(seed.rotate_left(11)))),
        3 => Value::Bool((mix32(seed) & 1) != 0),
        4 => Value::Float(f64::from_bits(
            (u64::from(mix32(seed)) << 32) | u64::from(mix32(seed ^ 0xDEAD_BEEF)),
        )),
        5 => Value::Bytes(Arc::from(generated_bytes(seed))),
        _ => Value::Array(vec![
            Value::U32(mix32(seed)),
            Value::Bool((seed & 1) == 0),
            Value::Bytes(Arc::from(generated_bytes(seed.rotate_left(3)))),
        ]),
    }
}

fn generated_width(seed: u32) -> usize {
    const WIDTHS: &[usize] = &[0, 1, 2, 3, 4, 5, 7, 8, 13, 16, 31, 32, 64];
    WIDTHS[(mix32(seed) as usize) % WIDTHS.len()]
}

fn generated_prefix(seed: u32) -> Vec<u8> {
    let len = (mix32(seed ^ 0xBADC_0FFE) % 19) as usize;
    (0..len)
        .map(|index| mix32(seed.wrapping_add(index as u32)) as u8)
        .collect()
}

fn generated_bytes(seed: u32) -> Vec<u8> {
    let len = (mix32(seed ^ 0x9E37_79B9) % 41) as usize;
    (0..len)
        .map(|index| {
            mix32(
                seed.wrapping_mul(1_664_525)
                    .wrapping_add(index as u32)
                    .rotate_left(index as u32 & 15),
            ) as u8
        })
        .collect()
}

fn mix32(mut value: u32) -> u32 {
    value ^= value >> 16;
    value = value.wrapping_mul(0x7FEB_352D);
    value ^= value >> 15;
    value = value.wrapping_mul(0x846C_A68B);
    value ^ (value >> 16)
}
