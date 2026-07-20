//! Multigrid-style smoothing step used by matroid fusion relaxations.

/// Run one weighted Jacobi smoothing step for the dense relaxation system.
#[must_use]
pub fn matroid_solve_step(a: &[f64], b: &[f64], x: &[f64], weight: f64, n: u32) -> Vec<f64> {
    let mut out = Vec::with_capacity(n as usize);
    matroid_solve_step_into(a, b, x, weight, n, &mut out);
    out
}

/// Run one weighted Jacobi smoothing step into caller-owned storage.
pub fn matroid_solve_step_into(
    a: &[f64],
    b: &[f64],
    x: &[f64],
    weight: f64,
    n: u32,
    out: &mut Vec<f64>,
) {
    let n = n as usize;
    out.clear();
    out.reserve(n);

    for i in 0..n {
        let mut ax_i = 0.0;
        for j in 0..n {
            let a_ij = a.get(i * n + j).copied().unwrap_or(0.0);
            let x_j = x.get(j).copied().unwrap_or(0.0);
            ax_i += a_ij * x_j;
        }

        let residual = b.get(i).copied().unwrap_or(0.0) - ax_i;
        let diagonal = a.get(i * n + i).copied().unwrap_or(0.0);
        let guarded_diagonal = if diagonal.abs() > 1e-30 {
            diagonal
        } else {
            1.0
        };
        let x_i = x.get(i).copied().unwrap_or(0.0);
        out.push(x_i + weight * residual / guarded_diagonal);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_system_one_step_converges() {
        // A = I₃, b = [1,2,3], x = [0,0,0], w = 1.0.
        // One step: x_new = b (since A is identity).
        let a = vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];
        let b = vec![1.0, 2.0, 3.0];
        let x = vec![0.0, 0.0, 0.0];
        let result = matroid_solve_step(&a, &b, &x, 1.0, 3);
        assert!((result[0] - 1.0).abs() < 1e-12);
        assert!((result[1] - 2.0).abs() < 1e-12);
        assert!((result[2] - 3.0).abs() < 1e-12);
    }

    #[test]
    fn zero_weight_preserves_state() {
        let a = vec![2.0, 0.0, 0.0, 2.0];
        let b = vec![10.0, 20.0];
        let x = vec![5.0, 7.0];
        let result = matroid_solve_step(&a, &b, &x, 0.0, 2);
        assert_eq!(result, vec![5.0, 7.0]);
    }

    #[test]
    fn partial_weight_moves_toward_solution() {
        // A = I₂, b = [10, 20], x = [0, 0], w = 0.5.
        // x_new = 0 + 0.5 * (b - 0) / 1.0 = [5, 10].
        let a = vec![1.0, 0.0, 0.0, 1.0];
        let b = vec![10.0, 20.0];
        let x = vec![0.0, 0.0];
        let result = matroid_solve_step(&a, &b, &x, 0.5, 2);
        assert!((result[0] - 5.0).abs() < 1e-12);
        assert!((result[1] - 10.0).abs() < 1e-12);
    }

    #[test]
    fn matroid_solve_step_into_reuses_storage() {
        let a = vec![2.0, 0.0, 0.0, 4.0];
        let b = vec![4.0, 8.0];
        let x = vec![0.0, 0.0];
        let mut out = Vec::with_capacity(16);
        out.extend_from_slice(&[99.0, 98.0, 97.0]);
        let capacity = out.capacity();

        matroid_solve_step_into(&a, &b, &x, 0.5, 2, &mut out);

        assert_eq!(out, vec![1.0, 1.0]);
        assert_eq!(out.capacity(), capacity);
    }

    #[test]
    fn missing_entries_are_zero_padded_instead_of_panicking() {
        let a = vec![2.0];
        let b = vec![4.0];
        let x = vec![1.0];
        let result = matroid_solve_step(&a, &b, &x, 0.5, 2);

        assert_eq!(result, vec![1.5, 0.0]);
    }

    #[test]
    fn zero_diagonal_uses_unit_diagonal_guard() {
        let a = vec![0.0, 0.0, 0.0, 0.0];
        let b = vec![2.0, -4.0];
        let x = vec![1.0, 3.0];
        let result = matroid_solve_step(&a, &b, &x, 0.25, 2);

        assert_eq!(result, vec![1.5, 2.0]);
    }
}
