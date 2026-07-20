//! Shared bitwise dual-reference machinery.

/// Direct u32 binary reference over the first two little-endian words.
#[must_use]
pub(crate) fn binary_direct(input: &[u8], op: impl FnOnce(u32, u32) -> u32) -> Vec<u8> {
    let Some((left, right)) = read_two_words(input) else {
        return zero_word();
    };
    op(left, right).to_le_bytes().to_vec()
}

/// Bit-by-bit binary reference over the first two little-endian words.
#[must_use]
pub(crate) fn binary_bits(input: &[u8], op: impl Fn(bool, bool) -> bool) -> Vec<u8> {
    if input.len() < 8 {
        return zero_word();
    }
    let mut output = [0_u8; 4];
    for bit_index in 0..32 {
        if op(bit_at(input, bit_index), bit_at(input, bit_index + 32)) {
            output[bit_index / 8] |= 1 << (bit_index % 8);
        }
    }
    output.to_vec()
}

/// Direct u32 unary reference over the first little-endian word.
#[must_use]
pub(crate) fn unary_direct(input: &[u8], op: impl FnOnce(u32) -> u32) -> Vec<u8> {
    let Some(value) = read_one_word(input) else {
        return zero_word();
    };
    op(value).to_le_bytes().to_vec()
}

/// Bit-by-bit unary reference over the first little-endian word.
#[must_use]
pub(crate) fn unary_bits(input: &[u8], op: impl Fn(bool) -> bool) -> Vec<u8> {
    if input.len() < 4 {
        return zero_word();
    }
    let mut output = [0_u8; 4];
    for bit_index in 0..32 {
        if op(bit_at(input, bit_index)) {
            output[bit_index / 8] |= 1 << (bit_index % 8);
        }
    }
    output.to_vec()
}

/// Bit-walk reference for `left << (right & 31)`.
#[must_use]
pub(crate) fn shift_left_bits(input: &[u8]) -> Vec<u8> {
    let Some((_, right)) = read_two_words(input) else {
        return zero_word();
    };
    let shift = (right & 31) as usize;
    let mut output = [0_u8; 4];
    for bit_index in shift..32 {
        if bit_at(input, bit_index - shift) {
            output[bit_index / 8] |= 1 << (bit_index % 8);
        }
    }
    output.to_vec()
}

/// Bit-walk reference for `left >> (right & 31)`.
#[must_use]
pub(crate) fn shift_right_bits(input: &[u8]) -> Vec<u8> {
    let Some((_, right)) = read_two_words(input) else {
        return zero_word();
    };
    let shift = (right & 31) as usize;
    let mut output = [0_u8; 4];
    for bit_index in 0..(32 - shift) {
        if bit_at(input, bit_index + shift) {
            output[bit_index / 8] |= 1 << (bit_index % 8);
        }
    }
    output.to_vec()
}

/// Manual bit-count reference independent of `u32::count_ones`.
#[must_use]
pub(crate) fn popcount_bits(input: &[u8]) -> Vec<u8> {
    if input.len() < 4 {
        return zero_word();
    }
    let mut count = 0u32;
    for bit_index in 0..32 {
        count += u32::from(bit_at(input, bit_index));
    }
    count.to_le_bytes().to_vec()
}

/// Manual count-leading-zero reference independent of `u32::leading_zeros`.
#[must_use]
pub(crate) fn clz_bits(input: &[u8]) -> Vec<u8> {
    if input.len() < 4 {
        return zero_word();
    }
    let mut count = 0u32;
    for bit_index in (0..32).rev() {
        if bit_at(input, bit_index) {
            break;
        }
        count += 1;
    }
    count.to_le_bytes().to_vec()
}

fn read_one_word(input: &[u8]) -> Option<u32> {
    (input.len() >= 4).then(|| u32::from_le_bytes([input[0], input[1], input[2], input[3]]))
}

fn read_two_words(input: &[u8]) -> Option<(u32, u32)> {
    (input.len() >= 8).then(|| {
        (
            u32::from_le_bytes([input[0], input[1], input[2], input[3]]),
            u32::from_le_bytes([input[4], input[5], input[6], input[7]]),
        )
    })
}

fn bit_at(input: &[u8], bit_index: usize) -> bool {
    let byte = input[bit_index / 8];
    let mask = 1_u8 << (bit_index % 8);
    byte & mask != 0
}

fn zero_word() -> Vec<u8> {
    vec![0; 4]
}

macro_rules! define_binary_bitwise_dual {
    ($marker:ident, $op_id:literal, $word_op:expr, $bit_op:expr) => {
        /// Operation ID for this bitwise primitive.
        pub const OP_ID: &str = $op_id;

        /// Direct word-oriented reference.
        pub mod reference_a {
            /// Evaluate the direct word-oriented bitwise reference.
            #[must_use]
            pub fn reference(input: &[u8]) -> Vec<u8> {
                super::super::common::binary_direct(input, $word_op)
            }
        }

        /// Independent bit-by-bit reference.
        pub mod reference_b {
            /// Evaluate the bit-by-bit bitwise reference.
            #[must_use]
            pub fn reference(input: &[u8]) -> Vec<u8> {
                super::super::common::binary_bits(input, $bit_op)
            }
        }

        /// Dual-reference marker for this bitwise primitive.
        pub struct $marker;

        impl $crate::dual::DualReference for $marker {
            fn reference_a(input: &[u8]) -> Vec<u8> {
                reference_a::reference(input)
            }

            fn reference_b(input: &[u8]) -> Vec<u8> {
                reference_b::reference(input)
            }
        }
    };
}

macro_rules! define_unary_bitwise_dual {
    ($marker:ident, $op_id:literal, $word_op:expr, $bit_op:expr) => {
        /// Operation ID for this bitwise primitive.
        pub const OP_ID: &str = $op_id;

        /// Direct word-oriented reference.
        pub mod reference_a {
            /// Evaluate the direct word-oriented bitwise reference.
            #[must_use]
            pub fn reference(input: &[u8]) -> Vec<u8> {
                super::super::common::unary_direct(input, $word_op)
            }
        }

        /// Independent bit-by-bit reference.
        pub mod reference_b {
            /// Evaluate the bit-by-bit bitwise reference.
            #[must_use]
            pub fn reference(input: &[u8]) -> Vec<u8> {
                super::super::common::unary_bits(input, $bit_op)
            }
        }

        /// Dual-reference marker for this bitwise primitive.
        pub struct $marker;

        impl $crate::dual::DualReference for $marker {
            fn reference_a(input: &[u8]) -> Vec<u8> {
                reference_a::reference(input)
            }

            fn reference_b(input: &[u8]) -> Vec<u8> {
                reference_b::reference(input)
            }
        }
    };
}

pub(crate) use define_binary_bitwise_dual;
pub(crate) use define_unary_bitwise_dual;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binary_direct_and_bits_match_on_generated_cases() {
        for case in 0..4096_u32 {
            let left = case.wrapping_mul(0x9e37_79b9);
            let right = case.rotate_left(7) ^ 0xa5a5_5a5a;
            let mut input = Vec::with_capacity(8);
            input.extend_from_slice(&left.to_le_bytes());
            input.extend_from_slice(&right.to_le_bytes());
            assert_eq!(
                binary_direct(&input, |a, b| a ^ b),
                binary_bits(&input, |a, b| a != b)
            );
        }
    }

    #[test]
    fn unary_direct_and_bits_match_on_generated_cases() {
        for case in 0..4096_u32 {
            let value = case.wrapping_mul(0x85eb_ca6b).rotate_left(case % 31);
            let input = value.to_le_bytes();
            assert_eq!(
                unary_direct(&input, |word| !word),
                unary_bits(&input, |bit| !bit)
            );
        }
    }

    #[test]
    fn short_inputs_zero_fill_without_panicking() {
        assert_eq!(binary_direct(&[1, 2, 3], |a, b| a & b), vec![0; 4]);
        assert_eq!(binary_bits(&[1, 2, 3], |a, b| a && b), vec![0; 4]);
        assert_eq!(unary_direct(&[1, 2, 3], |a| !a), vec![0; 4]);
        assert_eq!(unary_bits(&[1, 2, 3], |a| !a), vec![0; 4]);
        assert_eq!(shift_left_bits(&[1, 2, 3]), vec![0; 4]);
        assert_eq!(shift_right_bits(&[1, 2, 3]), vec![0; 4]);
        assert_eq!(popcount_bits(&[1, 2, 3]), vec![0; 4]);
        assert_eq!(clz_bits(&[1, 2, 3]), vec![0; 4]);
    }
}
