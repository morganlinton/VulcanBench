//! CPU references for substrate kernels foundation needs locally.
//!
//! Foundation cannot depend on `vyre-primitives` (cycle: primitives
//! optionally depends on foundation IR types via the `vyre-foundation`
//! feature). The substrate kernels foundation needs in its
//! `pass_substrate` modules are inlined here as small CPU references.
//!
//! # The duplication is deliberate
//!
//! These kernels are intentionally duplicated with their counterparts
//! in `vyre-primitives::math` / `vyre-primitives::graph` /
//! `vyre-primitives::opt`. Each side targets a different layer:
//!
//! - `vyre_primitives::*::*_cpu`  -  the canonical CPU references, used
//!   by every crate below `vyre-libs` (driver, runtime, aot, cc, libs).
//! - `vyre_foundation::cpu_references::*` (this file)  -  foundation-local
//!   references used only by `pass_substrate` modules. Bypasses the
//!   primitives→foundation cycle.
//!
//! This is the pattern Linux uses for `arch/*/lib/memcpy` versus the
//! generic `lib/string.c::memcpy`  -  each layer keeps a fit-for-purpose
//! copy rather than introducing acrobatics to dedupe a small kernel.
//!
//! # Parity contract
//!
//! For every kernel in this file, there MUST be a corresponding
//! `*_cpu` function in `vyre-primitives` with byte-equivalent output
//! on byte-equivalent input. The
//! `vyre-harness::tests::cpu_references_parity` test asserts this
//! invariant  -  when these kernels drift, that test fails.
//!
//! When you add a kernel here, also:
//! 1. Confirm the matching primitive exists in `vyre-primitives::*`.
//! 2. Add a parity case to `vyre-harness::tests::cpu_references_parity`.
//! 3. If the math actually needs to change, change BOTH sides in the
//!    same commit.

#[cfg(test)]
use crate::cpu_op;

/// Apply a column mapping to a row vector, writing values into `target_size` slots.
///
/// AUDIT_2026-05-23: Deprecated - CPU reference. Use GPU `functor_apply` primitive.
#[deprecated(note = "CPU reference. Use GPU functor_apply primitive.")]
#[must_use]
pub fn functor_apply_cpu(source_row: &[u32], mapping: &[u32], target_size: u32) -> Vec<u32> {
    assert_eq!(source_row.len(), mapping.len());
    let mut out = vec![0u32; target_size as usize];
    for (i, &dst) in mapping.iter().enumerate() {
        out[dst as usize] = source_row[i];
    }
    out
}

/// Compose two row-major matrices `first[source_dim,middle_dim]` and
/// `second[middle_dim,target_dim]` on the CPU.
///
/// Uses row, scan, col loop order so the inner loop scans `second`
/// sequentially in row-major layout  -  2-4× faster than the naive
/// row, col, scan order on matrices that don't fit L1.
#[must_use]
pub fn monoidal_compose_cpu(
    first: &[f64],
    second: &[f64],
    source_dim: u32,
    middle_dim: u32,
    target_dim: u32,
) -> Vec<f64> {
    let source_dim = source_dim as usize;
    let middle_dim = middle_dim as usize;
    let target_dim = target_dim as usize;
    assert_eq!(first.len(), source_dim * middle_dim);
    assert_eq!(second.len(), middle_dim * target_dim);
    let mut out = vec![0.0; source_dim * target_dim];
    // row, scan, col order: the inner col-loop touches output and second
    // sequentially  -  both are consecutive in memory for row-major.
    for row in 0..source_dim {
        let out_row = &mut out[row * target_dim..(row + 1) * target_dim];
        for scan in 0..middle_dim {
            let first_value = first[row * middle_dim + scan];
            let second_row = &second[scan * target_dim..(scan + 1) * target_dim];
            for col in 0..target_dim {
                out_row[col] += first_value * second_row[col];
            }
        }
    }
    out
}

/// Advance a homotopy state by one Euler predictor step.
///
/// AUDIT_2026-05-23: Deprecated - CPU reference. Use GPU homotopy primitive.
#[deprecated(note = "CPU reference. Use GPU homotopy primitive.")]
#[must_use]
pub fn homotopy_euler_predictor_cpu(x_curr: &[f64], v: &[f64], dt: f64) -> Vec<f64> {
    x_curr
        .iter()
        .zip(v.iter())
        .map(|(&x, &dv)| x + dt * dv)
        .collect()
}

/// Evaluate the linear homotopy `(1 - t) * g_x + t * f_x`.
///
/// AUDIT_2026-05-23: Deprecated - CPU reference. Use GPU homotopy primitive.
#[deprecated(note = "CPU reference. Use GPU homotopy primitive.")]
#[must_use]
pub fn linear_homotopy_cpu(g_x: &[f64], f_x: &[f64], t: f64) -> Vec<f64> {
    let s = 1.0 - t;
    g_x.iter()
        .zip(f_x.iter())
        .map(|(&g, &f)| s * g + t * f)
        .collect()
}

/// Run one breadth-first expansion step in a matroid exchange graph.
///
/// Frontier-driven: iterates only nodes in `f_in` as sources, then
/// scans their adjacency row. For sparse frontiers this is O(|F|×n)
/// instead of the previous O(n²) full scan.
///
/// AUDIT_2026-05-23: Deprecated - CPU reference. Use GPU matroid BFS primitive.
#[deprecated(note = "CPU reference. Use GPU matroid BFS primitive.")]
#[must_use]
pub fn matroid_exchange_bfs_step_cpu(
    f_in: &[u32],
    adj: &[u32],
    v: &[u32],
    n: usize,
) -> (Vec<u32>, bool) {
    let mut out = vec![0u32; n];
    let mut any = false;
    // Collect frontier indices once  -  avoids the inner k-loop scan
    // for every candidate j. On realistic matroid graphs the frontier
    // is 1-5% of n, making this 20-100× faster.
    let frontier: Vec<usize> = (0..n).filter(|&k| f_in[k] != 0).collect();
    for k in &frontier {
        let row = &adj[k * n..(k + 1) * n];
        for j in 0..n {
            if v[j] == 0 && row[j] != 0 && out[j] == 0 {
                out[j] = 1;
                any = true;
            }
        }
    }
    (out, any)
}

/// Run one weighted Jacobi smoothing step for a dense `n x n` system.
///
/// Pre-extracts the diagonal reciprocals so the hot loop only does
/// a fused multiply-add per element. For n>64 this is ~1.5× faster
/// than recomputing `a[i*n+i]` with a branch per row.
///
/// AUDIT_2026-05-23: Deprecated - CPU reference. Use GPU Jacobi smooth primitive.
#[deprecated(note = "CPU reference. Use GPU Jacobi smooth primitive.")]
#[must_use]
pub fn jacobi_smooth_step_cpu(
    matrix: &[f64],
    rhs: &[f64],
    estimate: &[f64],
    weight: f64,
    dimension: u32,
) -> Vec<f64> {
    let dimension = dimension as usize;
    // Pre-extract diagonal reciprocals once (O(n) setup, saves O(n²)
    // redundant index arithmetic + branch in the hot loop).
    let inv_diag: Vec<f64> = (0..dimension)
        .map(|row| {
            let diagonal = matrix[row * dimension + row];
            if diagonal.abs() > 1e-30 {
                1.0 / diagonal
            } else {
                1.0
            }
        })
        .collect();
    (0..dimension)
        .map(|row_idx| {
            let row = &matrix[row_idx * dimension..row_idx * dimension + dimension];
            let residual: f64 = rhs[row_idx]
                - row
                    .iter()
                    .zip(estimate.iter())
                    .map(|(&coefficient, &value)| coefficient * value)
                    .sum::<f64>();
            estimate[row_idx] + weight * residual * inv_diag[row_idx]
        })
        .collect()
}

#[cfg(test)]
pub(crate) fn primitive_math_div_cpu(input: &[u8], output: &mut Vec<u8>) {
    output.clear();
    if input.len() < 8 {
        output.extend_from_slice(&0u32.to_le_bytes());
        return;
    }
    let lhs = u32::from_le_bytes([input[0], input[1], input[2], input[3]]);
    let rhs = u32::from_le_bytes([input[4], input[5], input[6], input[7]]);
    output.extend_from_slice(&if rhs == 0 { 0 } else { lhs / rhs }.to_le_bytes());
}

#[cfg(test)]
pub(crate) fn cpu_fn_for_composition(id: &str) -> Option<cpu_op::CpuFn> {
    match id {
        "primitive.math.div" => Some(primitive_math_div_cpu),
        _ => None,
    }
}

#[cfg(test)]
#[allow(deprecated)]
mod tests {
    use super::*;

    // ── functor_apply_cpu ──

    #[test]
    fn functor_apply_identity_mapping() {
        // Identity mapping: [0→0, 1→1, 2→2].
        let source = vec![10, 20, 30];
        let mapping = vec![0, 1, 2];
        let result = functor_apply_cpu(&source, &mapping, 3);
        assert_eq!(result, vec![10, 20, 30]);
    }

    #[test]
    fn functor_apply_permutation() {
        // Reverse permutation: [0→2, 1→1, 2→0].
        let source = vec![10, 20, 30];
        let mapping = vec![2, 1, 0];
        let result = functor_apply_cpu(&source, &mapping, 3);
        assert_eq!(result, vec![30, 20, 10]);
    }

    #[test]
    fn functor_apply_expansion() {
        // Scatter into larger target.
        let source = vec![5, 7];
        let mapping = vec![1, 3];
        let result = functor_apply_cpu(&source, &mapping, 5);
        assert_eq!(result, vec![0, 5, 0, 7, 0]);
    }

    // ── monoidal_compose_cpu ──

    #[test]
    fn compose_identity_matrices() {
        // I₂ × I₂ = I₂.
        let id = vec![1.0, 0.0, 0.0, 1.0];
        let result = monoidal_compose_cpu(&id, &id, 2, 2, 2);
        assert_eq!(result, vec![1.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn compose_known_product() {
        // [[1,2],[3,4]] × [[1,0],[0,1]] = [[1,2],[3,4]].
        let f = vec![1.0, 2.0, 3.0, 4.0];
        let id = vec![1.0, 0.0, 0.0, 1.0];
        let result = monoidal_compose_cpu(&f, &id, 2, 2, 2);
        assert_eq!(result, vec![1.0, 2.0, 3.0, 4.0]);
    }

    #[test]
    fn compose_non_square() {
        // [1,2,3] (1×3) × [[1],[0],[1]] (3×1) = [1+0+3] = [4] (1×1).
        let f = vec![1.0, 2.0, 3.0];
        let g = vec![1.0, 0.0, 1.0];
        let result = monoidal_compose_cpu(&f, &g, 1, 3, 1);
        assert_eq!(result, vec![4.0]);
    }

    // ── homotopy_euler_predictor_cpu ──

    #[test]
    fn euler_zero_dt_is_identity() {
        let x = vec![1.0, 2.0, 3.0];
        let v = vec![10.0, 20.0, 30.0];
        let result = homotopy_euler_predictor_cpu(&x, &v, 0.0);
        assert_eq!(result, vec![1.0, 2.0, 3.0]);
    }

    #[test]
    fn euler_unit_step() {
        let x = vec![0.0, 0.0];
        let v = vec![1.0, -1.0];
        let result = homotopy_euler_predictor_cpu(&x, &v, 1.0);
        assert_eq!(result, vec![1.0, -1.0]);
    }

    #[test]
    fn euler_half_step() {
        let x = vec![2.0];
        let v = vec![4.0];
        let result = homotopy_euler_predictor_cpu(&x, &v, 0.5);
        assert_eq!(result, vec![4.0]); // 2.0 + 0.5 * 4.0 = 4.0
    }

    // ── linear_homotopy_cpu ──

    #[test]
    fn homotopy_t0_returns_g() {
        let g = vec![1.0, 2.0];
        let f = vec![10.0, 20.0];
        let result = linear_homotopy_cpu(&g, &f, 0.0);
        assert_eq!(result, vec![1.0, 2.0]);
    }

    #[test]
    fn homotopy_t1_returns_f() {
        let g = vec![1.0, 2.0];
        let f = vec![10.0, 20.0];
        let result = linear_homotopy_cpu(&g, &f, 1.0);
        assert_eq!(result, vec![10.0, 20.0]);
    }

    #[test]
    fn homotopy_midpoint() {
        let g = vec![0.0, 0.0];
        let f = vec![10.0, 20.0];
        let result = linear_homotopy_cpu(&g, &f, 0.5);
        assert_eq!(result, vec![5.0, 10.0]);
    }

    // ── matroid_exchange_bfs_step_cpu ──

    #[test]
    fn bfs_step_reaches_adjacent_unvisited() {
        // n=3. Frontier = {0}. 0→1, 0→2. Visited = {}.
        let f_in = vec![1, 0, 0];
        #[rustfmt::skip]
        let adj = vec![
            0, 1, 1,
            0, 0, 0,
            0, 0, 0,
        ];
        let visited = vec![0, 0, 0];
        let (reached, any) = matroid_exchange_bfs_step_cpu(&f_in, &adj, &visited, 3);
        assert!(any);
        assert_eq!(reached[1], 1);
        assert_eq!(reached[2], 1);
    }

    #[test]
    fn bfs_step_skips_visited() {
        // n=3. Frontier = {0}. 0→1, 0→2. Visited = {1}.
        let f_in = vec![1, 0, 0];
        #[rustfmt::skip]
        let adj = vec![
            0, 1, 1,
            0, 0, 0,
            0, 0, 0,
        ];
        let visited = vec![0, 1, 0]; // node 1 already visited
        let (reached, any) = matroid_exchange_bfs_step_cpu(&f_in, &adj, &visited, 3);
        assert!(any);
        assert_eq!(reached[1], 0, "should skip visited node 1");
        assert_eq!(reached[2], 1);
    }

    #[test]
    fn bfs_step_no_progress_reports_false() {
        // No adjacency from frontier.
        let f_in = vec![1, 0, 0];
        let adj = vec![0; 9];
        let visited = vec![0, 0, 0];
        let (_, any) = matroid_exchange_bfs_step_cpu(&f_in, &adj, &visited, 3);
        assert!(!any);
    }

    // ── jacobi_smooth_step_cpu ──

    #[test]
    fn jacobi_identity_system_converges() {
        // A = I, b = [1, 2], x = [0, 0], w = 1.0.
        // x_new[i] = x[i] + w * (b[i] - A*x[i]) / A[i,i]
        //          = 0 + 1.0 * (b[i] - 0) / 1.0 = b[i].
        let a = vec![1.0, 0.0, 0.0, 1.0];
        let b = vec![1.0, 2.0];
        let x = vec![0.0, 0.0];
        let result = jacobi_smooth_step_cpu(&a, &b, &x, 1.0, 2);
        assert!((result[0] - 1.0).abs() < 1e-12);
        assert!((result[1] - 2.0).abs() < 1e-12);
    }

    #[test]
    fn jacobi_zero_weight_preserves_x() {
        let a = vec![1.0, 0.0, 0.0, 1.0];
        let b = vec![10.0, 20.0];
        let x = vec![3.0, 7.0];
        let result = jacobi_smooth_step_cpu(&a, &b, &x, 0.0, 2);
        assert_eq!(result, vec![3.0, 7.0]);
    }

    // ── primitive_math_div_cpu ──

    #[test]
    fn div_normal() {
        let input: Vec<u8> = [10u32.to_le_bytes(), 3u32.to_le_bytes()].concat();
        let mut output = Vec::new();
        primitive_math_div_cpu(&input, &mut output);
        let result = u32::from_le_bytes(output[0..4].try_into().unwrap());
        assert_eq!(result, 3); // 10 / 3 = 3 (integer division)
    }

    #[test]
    fn div_by_zero_returns_zero() {
        let input: Vec<u8> = [42u32.to_le_bytes(), 0u32.to_le_bytes()].concat();
        let mut output = Vec::new();
        primitive_math_div_cpu(&input, &mut output);
        let result = u32::from_le_bytes(output[0..4].try_into().unwrap());
        assert_eq!(result, 0, "division by zero must return 0, not panic");
    }

    // ── cpu_fn_for_composition ──

    #[test]
    fn cpu_fn_lookup_known() {
        assert!(cpu_fn_for_composition("primitive.math.div").is_some());
    }

    #[test]
    fn cpu_fn_lookup_unknown() {
        assert!(cpu_fn_for_composition("nonexistent.op").is_none());
    }
}
