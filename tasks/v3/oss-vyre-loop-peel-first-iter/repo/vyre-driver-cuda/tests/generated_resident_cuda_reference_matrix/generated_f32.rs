use super::*;

pub(crate) fn generated_f32_values(salt: u32) -> Vec<f32> {
    const BITS: &[u32] = &[
        0x0000_0000,
        0x8000_0000,
        0x3f80_0000,
        0xbf80_0000,
        0x4000_0000,
        0xc000_0000,
        0x3f00_0000,
        0xbf00_0000,
        0x0000_0001,
        0x8000_0001,
        0x007f_ffff,
        0x807f_ffff,
        0x0080_0000,
        0x8080_0000,
        0x7f7f_ffff,
        0xff7f_ffff,
        0x7f80_0000,
        0xff80_0000,
        0x7fc0_0000,
        0xffc0_0000,
        0x7fa0_0001,
        0x7fff_ffff,
        0xffff_ffff,
    ];
    (0..LANE_COUNT)
        .map(|lane| {
            let lane_word = lane as u32;
            let seed = BITS[lane % BITS.len()];
            let mixed = seed ^ salt.rotate_left(lane_word & 31);
            f32::from_bits(if lane % 5 == 0 { seed } else { mixed })
        })
        .collect()
}

pub(crate) fn generated_f32_nonzero_values(salt: u32) -> Vec<f32> {
    generated_f32_values(salt)
        .into_iter()
        .enumerate()
        .map(|(lane, value)| {
            let bits = value.to_bits();
            if bits & 0x7fff_ffff == 0 {
                f32::from_bits(0x3f80_0000 | ((lane as u32) & 0x007f_ffff))
            } else {
                value
            }
        })
        .collect()
}

pub(crate) fn generated_f32_sqrt_domain_values(salt: u32) -> Vec<f32> {
    generated_f32_values(salt)
        .into_iter()
        .enumerate()
        .map(|(lane, value)| {
            let magnitude = value.to_bits() & 0x7fff_ffff;
            let exponent = magnitude & 0x7f80_0000;
            let mantissa = magnitude & 0x007f_ffff;
            if exponent == 0x7f80_0000 && mantissa != 0 {
                f32::from_bits(0x3f80_0000 | ((lane as u32) & 0x000f_ffff))
            } else {
                f32::from_bits(magnitude)
            }
        })
        .collect()
}

pub(crate) fn generated_f32_classification_values() -> Vec<f32> {
    const BITS: &[u32] = &[
        0x0000_0000,
        0x8000_0000,
        0x0000_0001,
        0x8000_0001,
        0x007f_ffff,
        0x807f_ffff,
        0x0080_0000,
        0x8080_0000,
        0x3f80_0000,
        0xbf80_0000,
        0x7f7f_ffff,
        0xff7f_ffff,
        0x7f80_0000,
        0xff80_0000,
        0x7fc0_0000,
        0xffc0_0000,
        0x7fa0_0001,
        0x7fff_ffff,
        0xffff_ffff,
    ];
    (0..LANE_COUNT)
        .map(|lane| f32::from_bits(BITS[lane % BITS.len()]))
        .collect()
}

