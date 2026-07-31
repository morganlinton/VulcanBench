//! `reduce_count_non_zero`  -  count the non-zero lanes in a u32 ValueSet.

use vyre_foundation::ir::Program;

use super::atomic_scalar::{atomic_reduce_u32, AtomicReduceKind};

/// Canonical op id.
pub const OP_ID: &str = "vyre-primitives::reduce::count_non_zero";

/// Build a Program: `out[0] = |{ i | values[i] != 0 }|`.
#[must_use]
pub fn reduce_count_non_zero(values: &str, out: &str, count: u32) -> Program {
    atomic_reduce_u32(values, out, count, AtomicReduceKind::CountNonZero, OP_ID)
}

/// CPU reference.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn cpu_ref(values: &[u32]) -> u32 {
    values.iter().filter(|&&value| value != 0).count() as u32
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        OP_ID,
        || reduce_count_non_zero("values", "out", 4),
        Some(|| {
            let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
            vec![vec![
                to_bytes(&[1, 0, 1, 1]),
                to_bytes(&[0]),
            ]]
        }),
        Some(|| {
            let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
            vec![vec![to_bytes(&[3])]]
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_non_zero_lanes() {
        assert_eq!(cpu_ref(&[0, 7, 0, 9, 1]), 3);
    }

    #[test]
    fn empty_values_count_zero() {
        assert_eq!(cpu_ref(&[]), 0);
    }

    #[test]
    fn program_uses_parallel_grid_stride() {
        let program = reduce_count_non_zero("values", "out", 513);
        assert_eq!(
            program.workgroup_size(),
            [crate::reduce::atomic_scalar::WORKGROUP_SIZE, 1, 1]
        );
    }

    #[test]
    fn generated_count_non_zero_oracle_covers_large_streams() {
        for case in 0..4096u32 {
            let len = 257 + (case.wrapping_mul(23) % 1024) as usize;
            let mut state = 0xC0DE_CAFE_u32 ^ case.wrapping_mul(0x9E37_79B9);
            let mut values = Vec::with_capacity(len);
            for index in 0..len {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                values.push(if (state.wrapping_add(index as u32)) % 11 == 0 {
                    0
                } else {
                    state
                });
            }

            let expected = values.iter().filter(|&&value| value != 0).count() as u32;
            assert_eq!(cpu_ref(&values), expected, "generated case {case}");
        }
    }
}
