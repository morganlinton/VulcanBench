use super::*;

#[test]
fn materialized_input_key_separates_tuple_boundaries_for_4096_generated_cases() {
    for seed in 0_u32..4096 {
        let left_len = ((seed.wrapping_mul(17) ^ seed.rotate_left(5)) % 31 + 1) as usize;
        let right_len = ((seed.wrapping_mul(29) ^ seed.rotate_left(9)) % 31 + 1) as usize;
        let mut state = seed ^ 0xC0DA_CAFE;
        let mut left = Vec::with_capacity(left_len);
        let mut right = Vec::with_capacity(right_len);
        for index in 0..left_len {
            state = state
                .wrapping_mul(1_664_525)
                .wrapping_add(1_013_904_223)
                .rotate_left((index as u32) & 15);
            left.push((state ^ seed.rotate_left(index as u32 & 31)) as u8);
        }
        for index in 0..right_len {
            state = state
                .wrapping_mul(22_695_477)
                .wrapping_add(1)
                .rotate_left((index as u32) & 7);
            right.push((state ^ seed.rotate_right(index as u32 & 31)) as u8);
        }
        let mut concatenated = Vec::with_capacity(left_len + right_len);
        concatenated.extend_from_slice(&left);
        concatenated.extend_from_slice(&right);

        let tuple_key = materialized_input_key(&[left.as_slice(), right.as_slice()])
            .expect("Fix: generated tuple materialized-input key must fit");
        let concatenated_key = materialized_input_key(&[concatenated.as_slice()])
            .expect("Fix: generated concatenated materialized-input key must fit");
        let empty_separated_key = materialized_input_key(&[left.as_slice(), &[], right.as_slice()])
            .expect("Fix: generated empty-separated materialized-input key must fit");

        assert_ne!(
            tuple_key, concatenated_key,
            "Fix: materialized CUDA output cache key must length-prefix inputs so tuple boundaries cannot alias for generated case {seed}."
        );
        assert_ne!(
            tuple_key, empty_separated_key,
            "Fix: materialized CUDA output cache key must include empty input slots instead of collapsing them for generated case {seed}."
        );
    }
}

#[test]
fn materialized_input_key_changes_on_4096_single_byte_mutations() {
    for seed in 0_u32..4096 {
        let len = ((seed.wrapping_mul(37) ^ seed.rotate_left(11)) % 96 + 1) as usize;
        let mut bytes = Vec::with_capacity(len);
        let mut state = seed ^ 0xA5A5_5A5A;
        for index in 0..len {
            state = state
                .wrapping_mul(1_103_515_245)
                .wrapping_add(12_345)
                .rotate_left((index as u32) & 15);
            bytes.push((state >> ((index & 3) * 8)) as u8);
        }
        let mut mutated = bytes.clone();
        let mutation_index = (seed as usize) % len;
        mutated[mutation_index] ^= 0x80 | ((seed as u8) & 0x7f);

        let base_key = materialized_input_key(&[bytes.as_slice()])
            .expect("Fix: base generated materialized-input key must fit");
        let mutated_key = materialized_input_key(&[mutated.as_slice()])
            .expect("Fix: mutated generated materialized-input key must fit");

        assert_ne!(
            base_key, mutated_key,
            "Fix: materialized CUDA output cache key must change when one byte changes for generated case {seed}."
        );
    }
}

