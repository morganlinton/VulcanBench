use crate::common::GENERATED_LANE_COUNT as LANE_COUNT;

pub(crate) const MAX_F32_ULP: u32 = 1;

const F32_CONTROL_BITS: &[u32] = &[
    0x0000_0000,
    0x8000_0000,
    0x3f80_0000,
    0xbf80_0000,
    0x4000_0000,
    0xc000_0000,
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
];

pub(crate) fn generated_f32_values(salt: u32) -> Vec<f32> {
    (0..LANE_COUNT)
        .map(|lane| {
            let lane = lane as u32;
            let seed = F32_CONTROL_BITS[lane as usize % F32_CONTROL_BITS.len()];
            let mixed = seed ^ salt.rotate_left(lane & 31);
            f32::from_bits(if lane % 5 == 0 { seed } else { mixed })
        })
        .collect()
}

pub(crate) fn generated_u32_values(salt: u32) -> Vec<u32> {
    (0..LANE_COUNT)
        .map(|lane| {
            let lane = lane as u32;
            let seed = match lane % 16 {
                0 => 0,
                1 => 1,
                2 => u32::MAX,
                3 => 0x8000_0000,
                4 => 0x7fff_ffff,
                5 => 0x5555_5555,
                6 => 0xaaaa_aaaa,
                7 => 0x0123_4567,
                _ => lane.wrapping_mul(0x9e37_79b9),
            };
            seed ^ salt.rotate_left(lane & 31) ^ lane.rotate_right((salt ^ lane) & 31)
        })
        .collect()
}

pub(crate) fn generated_i32_values(salt: u32) -> Vec<i32> {
    generated_u32_values(salt)
        .into_iter()
        .enumerate()
        .map(|(lane, word)| {
            let signed_seed = match lane % 10 {
                0 => i32::MIN,
                1 => i32::MAX,
                2 => -1,
                3 => 1,
                4 => -1024,
                5 => 1024,
                _ => word as i32,
            };
            signed_seed ^ word.rotate_left((lane as u32) & 31) as i32
        })
        .collect()
}
