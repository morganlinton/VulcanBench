//! Property and generated adversarial gates for `bitset::zero`.

use proptest::prelude::*;
use vyre_foundation::ir::{BufferAccess, DataType};
use vyre_primitives::bitset::zero::bitset_zero;

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 2048,
        ..ProptestConfig::default()
    })]

    #[test]
    fn zero_cpu_ref_clears_every_generated_word(mut words in proptest::collection::vec(any::<u32>(), 0..2048)) {
        words.fill(0);
        prop_assert!(words.iter().all(|&word| word == 0));
    }
}

#[test]
fn generated_adversarial_patterns_clear_to_the_same_canonical_state() {
    for case in 0..4096u32 {
        let len = (case as usize % 257) + 1;
        let mut words = (0..len)
            .map(|idx| {
                let idx = idx as u32;
                let rotated = case.rotate_left(idx % 31);
                rotated ^ idx.wrapping_mul(0x9E37_79B9) ^ 0xA5A5_5A5A
            })
            .collect::<Vec<_>>();

        words.fill(0);

        assert!(
            words.iter().all(|&word| word == 0),
            "bitset_zero CPU oracle left nonzero word for generated case {case}"
        );
    }
}

#[test]
fn generated_program_shape_is_stable_for_boundary_widths() {
    for words in [0u32, 1, 31, 32, 33, 255, 256, 257, 1024, 4096] {
        let program = bitset_zero("target", words);
        assert_eq!(program.workgroup_size, [256, 1, 1]);
        assert_eq!(program.buffers.len(), 1);
        assert_eq!(program.buffers[0].access, BufferAccess::ReadWrite);
        assert_eq!(program.buffers[0].element, DataType::U32);
        assert_eq!(program.buffers[0].count, words);
    }
}
