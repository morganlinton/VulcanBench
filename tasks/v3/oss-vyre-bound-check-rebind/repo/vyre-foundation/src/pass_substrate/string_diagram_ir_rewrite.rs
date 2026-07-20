//! String-diagram composition checks for IR rewrite arrows.

use crate::cpu_references::monoidal_compose_cpu;

/// Compose arrows `f: A -> B` and `g: B -> C` as dense matrices.
#[must_use]
pub fn compose_ir_arrows(
    first: &[f64],
    second: &[f64],
    source_dim: u32,
    middle_dim: u32,
    target_dim: u32,
) -> Vec<f64> {
    monoidal_compose_cpu(first, second, source_dim, middle_dim, target_dim)
}

/// Identity arrow for an `n`-object diagram.
#[must_use]
pub fn identity_arrow(n: u32) -> Vec<f64> {
    let mut out = vec![0.0; (n * n) as usize];
    for i in 0..n {
        out[(i * n + i) as usize] = 1.0;
    }
    out
}

/// Check associativity of three composable IR arrows.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn composition_associates(
    first: &[f64],
    second: &[f64],
    third: &[f64],
    source_dim: u32,
    first_target_dim: u32,
    second_target_dim: u32,
    final_target_dim: u32,
) -> bool {
    let second_after_first = compose_ir_arrows(
        first,
        second,
        source_dim,
        first_target_dim,
        second_target_dim,
    );
    let third_after_pair = compose_ir_arrows(
        &second_after_first,
        third,
        source_dim,
        second_target_dim,
        final_target_dim,
    );
    let third_after_second = compose_ir_arrows(
        second,
        third,
        first_target_dim,
        second_target_dim,
        final_target_dim,
    );
    let pair_after_first = compose_ir_arrows(
        first,
        &third_after_second,
        source_dim,
        first_target_dim,
        final_target_dim,
    );
    third_after_pair
        .iter()
        .zip(pair_after_first.iter())
        .all(|(left, right)| (left - right).abs() < 1e-9 * (1.0 + left.abs() + right.abs()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_arrow_is_identity_matrix() {
        let id = identity_arrow(3);
        assert_eq!(id.len(), 9);
        // Diagonal = 1.0, off-diagonal = 0.0.
        assert_eq!(id[0], 1.0); // (0,0)
        assert_eq!(id[4], 1.0); // (1,1)
        assert_eq!(id[8], 1.0); // (2,2)
        assert_eq!(id[1], 0.0); // (0,1)
        assert_eq!(id[3], 0.0); // (1,0)
    }

    #[test]
    fn compose_with_identity_is_identity() {
        // f ∘ id = f
        let f = vec![1.0, 2.0, 3.0, 4.0]; // 2x2
        let id = identity_arrow(2);
        let result = compose_ir_arrows(&id, &f, 2, 2, 2);
        for (got, expected) in result.iter().zip(f.iter()) {
            assert!(
                (got - expected).abs() < 1e-12,
                "compose with identity should be identity"
            );
        }
    }

    #[test]
    fn composition_is_associative() {
        // f: 2→2, g: 2→2, h: 2→2.
        let f = vec![1.0, 0.0, 0.0, 1.0]; // identity
        let g = vec![0.0, 1.0, 1.0, 0.0]; // swap
        let h = vec![2.0, 0.0, 0.0, 3.0]; // scale
        assert!(composition_associates(&f, &g, &h, 2, 2, 2, 2));
    }

    #[test]
    fn identity_arrow_size_zero() {
        let id = identity_arrow(0);
        assert!(id.is_empty());
    }
}
