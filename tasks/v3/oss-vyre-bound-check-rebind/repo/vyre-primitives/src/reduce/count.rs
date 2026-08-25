//! `reduce_count`  -  population count over a packed bitset, written
//! as a single u32 into `out[0]`.

use vyre_foundation::ir::Program;

use super::atomic_scalar::{atomic_reduce_u32, AtomicReduceKind};

/// Canonical op id.
pub const OP_ID: &str = "vyre-primitives::reduce::count";

/// Build a Program: `out[0] = sum_{w} popcount(bitset[w])`.
#[must_use]
pub fn reduce_count(bitset: &str, out: &str, words: u32) -> Program {
    atomic_reduce_u32(bitset, out, words, AtomicReduceKind::PopcountSum, OP_ID)
}

/// CPU reference.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn cpu_ref(bitset: &[u32]) -> u32 {
    bitset.iter().map(|w| w.count_ones()).sum()
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        OP_ID,
        || reduce_count("bitset", "out", 2),
        Some(|| {
            let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
            vec![vec![to_bytes(&[0b1111, 0xFFFF_FFFF]), to_bytes(&[0])]]
        }),
        Some(|| {
            let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
            vec![vec![to_bytes(&[36])]]
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn total_bit_count() {
        assert_eq!(cpu_ref(&[0b1111, 0xFFFF_FFFF]), 36);
    }

    #[test]
    fn program_uses_parallel_grid_stride() {
        let program = reduce_count("bitset", "out", 513);
        assert_eq!(
            program.workgroup_size(),
            [crate::reduce::atomic_scalar::WORKGROUP_SIZE, 1, 1]
        );
    }
}
