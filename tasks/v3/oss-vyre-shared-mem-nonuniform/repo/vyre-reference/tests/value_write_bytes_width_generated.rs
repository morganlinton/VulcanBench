//! Generated coverage for allocation-free fixed-slot `Value` writes.

use std::sync::Arc;

use vyre_reference::value::Value;

#[test]
fn generated_write_bytes_width_matches_allocating_encoding_for_16384_cases() {
    let mut assertions = 0usize;
    for seed in 0u32..16_384 {
        let value = generated_value(seed);
        for width in [1usize, 2, 3, 4, 5, 8, 13, 16, 31, 32, 64] {
            let mut target = vec![0xA5; width];
            value.write_bytes_width_into(&mut target);
            assert_eq!(
                target,
                value.to_bytes_width(width),
                "Fix: allocation-free fixed-slot Value write drifted from to_bytes_width at seed {seed} width {width}."
            );
            assertions += 1;
        }
    }
    assert_eq!(assertions, 16_384 * 11);
}

#[test]
fn generated_write_bytes_width_overwrites_existing_target_bytes() {
    let mut assertions = 0usize;
    for seed in 0u32..8192 {
        let value = generated_value(seed);
        let width = ((mix32(seed ^ 0xFACE_B00C) % 96) + 1) as usize;
        let mut target = (0..width)
            .map(|index| mix32(seed.wrapping_add(index as u32)) as u8)
            .collect::<Vec<_>>();
        value.write_bytes_width_into(&mut target);

        let expected = value.to_bytes_width(width);
        assert_eq!(
            target, expected,
            "Fix: allocation-free fixed-slot Value write must overwrite stale target bytes at seed {seed}."
        );
        assertions += 1;
    }
    assert_eq!(assertions, 8192);
}

fn generated_value(seed: u32) -> Value {
    match mix32(seed) % 8 {
        0 => Value::U32(mix32(seed)),
        1 => Value::I32(mix32(seed ^ 0x1357_9BDF) as i32),
        2 => Value::U64((u64::from(mix32(seed)) << 32) | u64::from(mix32(seed.rotate_left(7)))),
        3 => Value::Bool((mix32(seed) & 1) != 0),
        4 => Value::Float(f64::from_bits(
            (u64::from(mix32(seed ^ 0xCAFE_BABE)) << 32) | u64::from(mix32(seed)),
        )),
        5 => Value::Bytes(Arc::from(generated_bytes(seed))),
        6 => Value::Array(vec![
            Value::U32(mix32(seed)),
            Value::Bool((seed & 1) == 1),
            Value::Bytes(Arc::from(generated_bytes(seed.rotate_left(3)))),
        ]),
        _ => Value::Array(vec![
            generated_leaf(seed ^ 0xA5A5_5A5A),
            Value::Array(vec![
                Value::U64(u64::from(mix32(seed))),
                Value::Bytes(Arc::from(generated_bytes(seed ^ 0x5A5A_A5A5))),
            ]),
        ]),
    }
}

fn generated_leaf(seed: u32) -> Value {
    match mix32(seed) % 6 {
        0 => Value::U32(mix32(seed)),
        1 => Value::I32(mix32(seed ^ 0x1357_9BDF) as i32),
        2 => Value::U64((u64::from(mix32(seed)) << 32) | u64::from(mix32(seed.rotate_left(7)))),
        3 => Value::Bool((mix32(seed) & 1) != 0),
        4 => Value::Float(f64::from_bits(
            (u64::from(mix32(seed ^ 0xCAFE_BABE)) << 32) | u64::from(mix32(seed)),
        )),
        _ => Value::Bytes(Arc::from(generated_bytes(seed))),
    }
}

fn generated_bytes(seed: u32) -> Vec<u8> {
    let len = (mix32(seed ^ 0x9E37_79B9) % 49) as usize;
    (0..len)
        .map(|index| {
            mix32(
                seed.wrapping_add(index as u32)
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
