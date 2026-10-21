//! `reduce_sum`  -  wrapping unsigned sum over a u32 ValueSet.

use vyre_foundation::ir::Program;

use super::atomic_scalar::{atomic_reduce_u32, AtomicReduceKind};

/// Canonical op id.
pub const OP_ID: &str = "vyre-primitives::reduce::sum";

/// Build a Program: `out[0] = (Σ values_i) mod 2^32`.
#[must_use]
pub fn reduce_sum(values: &str, out: &str, count: u32) -> Program {
    atomic_reduce_u32(values, out, count, AtomicReduceKind::Sum, OP_ID)
}

/// CPU reference.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn cpu_ref(values: &[u32]) -> u32 {
    values.iter().copied().fold(0u32, u32::wrapping_add)
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        OP_ID,
        || reduce_sum("values", "out", 4),
        Some(|| {
            let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
            vec![vec![to_bytes(&[1, 2, 3, 4]), to_bytes(&[0])]]
        }),
        Some(|| {
            let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
            vec![vec![to_bytes(&[10])]]
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sums_values() {
        assert_eq!(cpu_ref(&[1, 2, 3, 4]), 10);
    }

    #[test]
    fn wraps_on_overflow() {
        assert_eq!(cpu_ref(&[u32::MAX, 1]), 0);
    }

    #[test]
    fn program_uses_parallel_grid_stride() {
        let program = reduce_sum("values", "out", 513);
        assert_eq!(
            program.workgroup_size(),
            [crate::reduce::atomic_scalar::WORKGROUP_SIZE, 1, 1]
        );
        assert!(
            !format!("{:?}", program.entry()).contains("grid_size_pending"),
            "reduce_sum program must not carry unresolved grid-size markers"
        );
    }
}
