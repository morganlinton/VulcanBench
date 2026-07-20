//! Tier 2.5 bitset primitives  -  `and`/`or`/`not`/`xor`/`popcount`/
//! `any`/`contains` over packed u32 bitsets. These are the LEGO
//! blocks every higher-level graph/taint composition reaches for
//! when combining two NodeSets.
//!
//! All primitives operate on the same bitset shape: a u32 buffer
//! with `word_count` slots, where bit `i` of word `w` represents
//! element `w * 32 + i`. Sizes are declared at `Program` build
//! time so the backend can allocate + validate layout up front.

/// Per-word bitwise AND over two packed bitsets.
pub mod and {
    crate::bitset::binary_word::define_bitwise_binary_op! {
        op_id: "vyre-primitives::bitset::and",
        fn_name: bitset_and,
        op_kind: And,
        combine: |a, b| a & b,
        inventory_words: 2,
        inventory_lhs: [0xFF00, 0x0F0F],
        inventory_rhs: [0xF0F0, 0xFF00],
        inventory_expected: [0xF000, 0x0F00],
        single_lhs: [0xFFFF_FFFF],
        single_rhs: [0xFFFF_FFFF],
        single_expected: [0xFFFF_FFFF],
        boundary_lhs: [0x8000_0000, 0x0000_0001],
        boundary_rhs: [0x8000_0000, 0x0000_0001],
        boundary_expected: [0x8000_0000, 0x0000_0001]
    }
}
/// In-place per-word bitwise AND (`target &= operand`).
pub mod and_into {
    crate::bitset::binary_word::define_bitwise_in_place_op! {
        op_id: "vyre-primitives::bitset::and_into",
        fn_name: bitset_and_into,
        op_kind: And,
        combine: |target: u32, operand: u32| target & operand,
        inventory_words: 2,
        inventory_target: [0xFFFF, 0xF0F0],
        inventory_operand: [0xFF00, 0xFFFF],
        inventory_expected: [0xFF00, 0xF0F0],
        cases: {
            full_mask_is_identity: {
                target: [0xDEAD_BEEF, 0x1234_5678],
                operand: [0xFFFF_FFFF, 0xFFFF_FFFF],
                expected: [0xDEAD_BEEF, 0x1234_5678]
            },
            empty_mask_zeros_target: {
                target: [0xDEAD_BEEF, 0x1234_5678],
                operand: [0, 0],
                expected: [0, 0]
            },
            second_mask_remains_monotone: {
                target: [0xFF00, 0xF0F0],
                operand: [0x0F00, 0x0F0F],
                expected: [0x0F00, 0x0000]
            }
        }
    }
}
pub mod and_not;
/// In-place per-word set difference (`target &= !operand`).
pub mod and_not_into {
    crate::bitset::binary_word::define_bitwise_in_place_op! {
        op_id: "vyre-primitives::bitset::and_not_into",
        fn_name: bitset_and_not_into,
        op_kind: AndNot,
        combine: |target: u32, operand: u32| target & !operand,
        inventory_words: 2,
        inventory_target: [0xFFFF, 0xF0F0],
        inventory_operand: [0x0F0F, 0x00FF],
        inventory_expected: [0xF0F0, 0xF000],
        cases: {
            subtraction_drops_waypoint_bits: {
                target: [0xFFFF, 0xF0F0],
                operand: [0xFF00, 0x00F0],
                expected: [0x00FF, 0xF000]
            },
            empty_subtrahend_is_identity: {
                target: [0xDEAD_BEEF, 0x1234_5678],
                operand: [0, 0],
                expected: [0xDEAD_BEEF, 0x1234_5678]
            },
            full_subtrahend_zeros_target: {
                target: [0xDEAD_BEEF, 0x1234_5678],
                operand: [0xFFFF_FFFF, 0xFFFF_FFFF],
                expected: [0, 0]
            },
            idempotent_on_repeat_shape: {
                target: [0x00FF],
                operand: [0xFF00],
                expected: [0x00FF]
            }
        }
    }
}
pub mod any;
pub(crate) mod binary_word;
pub(crate) mod bit_update;
/// Scalar mutate: clear bit `bit_idx` in `target`.
pub mod clear_bit {
    crate::bitset::bit_update::define_bit_update_op! {
        op_id: "vyre-primitives::bitset::clear_bit",
        fn_name: bitset_clear_bit,
        kind: Clear,
        inventory_input: [0xFFFF_FFFF, 0xFFFF_FFFF],
        inventory_expected: [0xFFFF_FFFE, 0xFFFF_FFFF]
    }
}
pub mod contains;
pub mod copy;
pub mod equal;
pub mod four_russians;
pub mod frontier;
pub mod not;
/// Per-word bitwise OR over two packed bitsets.
pub mod or {
    crate::bitset::binary_word::define_bitwise_binary_op! {
        op_id: "vyre-primitives::bitset::or",
        fn_name: bitset_or,
        op_kind: Or,
        combine: |a, b| a | b,
        inventory_words: 2,
        inventory_lhs: [0xFF00, 0x0F0F],
        inventory_rhs: [0x00FF, 0xF0F0],
        inventory_expected: [0xFFFF, 0xFFFF],
        single_lhs: [0xFFFF_FFFF],
        single_rhs: [0x0000_0000],
        single_expected: [0xFFFF_FFFF],
        boundary_lhs: [0x8000_0000, 0x0000_0000],
        boundary_rhs: [0x0000_0000, 0x0000_0001],
        boundary_expected: [0x8000_0000, 0x0000_0001]
    }
}
/// In-place per-word bitwise OR (`target |= operand`).
pub mod or_into {
    crate::bitset::binary_word::define_bitwise_in_place_op! {
        op_id: "vyre-primitives::bitset::or_into",
        fn_name: bitset_or_into,
        op_kind: Or,
        combine: |target: u32, operand: u32| target | operand,
        inventory_words: 2,
        inventory_target: [0xFFFF, 0xF0F0],
        inventory_operand: [0x0F0F, 0x00FF],
        inventory_expected: [0xFFFF, 0xF0FF],
        cases: {
            grows_empty_accumulator: {
                target: [0, 0],
                operand: [0xFF00, 0x0F0F],
                expected: [0xFF00, 0x0F0F]
            },
            reaches_full_union: {
                target: [0xFF00, 0x0F0F],
                operand: [0x00FF, 0xF0F0],
                expected: [0xFFFF, 0xFFFF]
            },
            repeat_full_union_is_idempotent: {
                target: [0xFFFF, 0xFFFF],
                operand: [0x00FF, 0xF0F0],
                expected: [0xFFFF, 0xFFFF]
            }
        }
    }
}
pub mod popcount;
pub(crate) mod relation;
pub mod select;
/// Scalar mutate: set bit `bit_idx` in `target`.
pub mod set_bit {
    crate::bitset::bit_update::define_bit_update_op! {
        op_id: "vyre-primitives::bitset::set_bit",
        fn_name: bitset_set_bit,
        kind: Set,
        inventory_input: [0, 0],
        inventory_expected: [1, 0]
    }
}
pub mod subset_of;
pub mod test_bit;
pub(crate) mod unary_word;
/// Per-word bitwise XOR over two packed bitsets.
pub mod xor {
    crate::bitset::binary_word::define_bitwise_binary_op! {
        op_id: "vyre-primitives::bitset::xor",
        fn_name: bitset_xor,
        op_kind: Xor,
        combine: |a, b| a ^ b,
        inventory_words: 1,
        inventory_lhs: [0xFFFF_0000],
        inventory_rhs: [0x0000_FFFF],
        inventory_expected: [0xFFFF_FFFF],
        single_lhs: [0xFFFF_FFFF],
        single_rhs: [0xFFFF_FFFF],
        single_expected: [0x0000_0000],
        boundary_lhs: [0x8000_0000, 0x0000_0001],
        boundary_rhs: [0x0000_0000, 0x0000_0001],
        boundary_expected: [0x8000_0000, 0x0000_0000]
    }
}
/// In-place per-word bitwise XOR (`target ^= operand`).
pub mod xor_into {
    crate::bitset::binary_word::define_bitwise_in_place_op! {
        op_id: "vyre-primitives::bitset::xor_into",
        fn_name: bitset_xor_into,
        op_kind: Xor,
        combine: |target: u32, operand: u32| target ^ operand,
        inventory_words: 2,
        inventory_target: [0xFFFF, 0xF0F0],
        inventory_operand: [0x0F0F, 0x00FF],
        inventory_expected: [0xF0F0, 0xF00F],
        cases: {
            xor_with_self_zeros: {
                target: [0xDEAD_BEEF, 0x1234_5678],
                operand: [0xDEAD_BEEF, 0x1234_5678],
                expected: [0, 0]
            },
            xor_with_zero_is_identity: {
                target: [0xFFFF, 0x0F0F],
                operand: [0, 0],
                expected: [0xFFFF, 0x0F0F]
            },
            xor_is_self_inverse_second_step: {
                target: [0x55AA],
                operand: [0xFF00],
                expected: [0xAAAA]
            },
            xor_distributes_per_word: {
                target: [0x00FF, 0xFF00],
                operand: [0x0F0F, 0xF0F0],
                expected: [0x0FF0, 0x0FF0]
            }
        }
    }
}
pub mod zero;

/// Stochastic computing primitive (#59)  -  bitstream multiplication
/// via AND. Power-efficient inference substrate.
pub mod stochastic_compute;

/// Words needed to hold a bitset over `n` elements.
///
/// Overflow-safe  -  `(n + 31) / 32` wraps to 0 for `n > u32::MAX - 31`;
/// `div_ceil` handles the overflow correctly. Per AUDIT_2026-04-24
/// F-CT-01 / F-LBL-01 (kimi).
#[must_use]
pub const fn bitset_words(n: u32) -> u32 {
    n.div_ceil(32)
}
