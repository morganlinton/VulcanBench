//! Shared arithmetic dual-reference machinery.

/// Direct u32 binary reference over the first two little-endian words.
#[must_use]
pub(crate) fn binary_direct(input: &[u8], op: impl FnOnce(u32, u32) -> u32) -> Vec<u8> {
    let Some((left, right)) = read_two_words(input) else {
        return zero_word();
    };
    op(left, right).to_le_bytes().to_vec()
}

/// Wrapping-add reference implemented through carry propagation only.
#[must_use]
pub(crate) fn wrapping_add_bits_reference(input: &[u8]) -> Vec<u8> {
    let Some((left, right)) = read_two_words(input) else {
        return zero_word();
    };
    wrapping_add_bits(left, right).to_le_bytes().to_vec()
}

/// Wrapping-multiply reference implemented as shift-and-add over bits.
#[must_use]
pub(crate) fn wrapping_mul_shift_add_reference(input: &[u8]) -> Vec<u8> {
    let Some((mut multiplicand, mut multiplier)) = read_two_words(input) else {
        return zero_word();
    };
    let mut acc = 0u32;
    while multiplier != 0 {
        if multiplier & 1 != 0 {
            acc = wrapping_add_bits(acc, multiplicand);
        }
        multiplicand = multiplicand.wrapping_shl(1);
        multiplier >>= 1;
    }
    acc.to_le_bytes().to_vec()
}

fn wrapping_add_bits(mut left: u32, mut right: u32) -> u32 {
    while right != 0 {
        let carry = left & right;
        left ^= right;
        right = carry.wrapping_shl(1);
    }
    left
}

fn read_two_words(input: &[u8]) -> Option<(u32, u32)> {
    (input.len() >= 8).then(|| {
        (
            u32::from_le_bytes([input[0], input[1], input[2], input[3]]),
            u32::from_le_bytes([input[4], input[5], input[6], input[7]]),
        )
    })
}

fn zero_word() -> Vec<u8> {
    vec![0; 4]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_arithmetic_duals_match_native_wrapping_ops() {
        for case in 0..8192u32 {
            let left = case.wrapping_mul(0x9e37_79b9).rotate_left(case & 31);
            let right = case ^ 0xa5a5_5a5a_u32.rotate_right(case & 31);
            let mut input = Vec::with_capacity(8);
            input.extend_from_slice(&left.to_le_bytes());
            input.extend_from_slice(&right.to_le_bytes());

            assert_eq!(
                binary_direct(&input, u32::wrapping_add),
                wrapping_add_bits_reference(&input),
                "Fix: arithmetic add duals diverged for left={left:#010x} right={right:#010x}"
            );
            assert_eq!(
                binary_direct(&input, u32::wrapping_mul),
                wrapping_mul_shift_add_reference(&input),
                "Fix: arithmetic mul duals diverged for left={left:#010x} right={right:#010x}"
            );
        }
    }

    #[test]
    fn short_inputs_zero_fill_without_panicking() {
        assert_eq!(binary_direct(&[1, 2, 3], u32::wrapping_add), vec![0; 4]);
        assert_eq!(wrapping_add_bits_reference(&[1, 2, 3]), vec![0; 4]);
        assert_eq!(wrapping_mul_shift_add_reference(&[1, 2, 3]), vec![0; 4]);
    }
}
