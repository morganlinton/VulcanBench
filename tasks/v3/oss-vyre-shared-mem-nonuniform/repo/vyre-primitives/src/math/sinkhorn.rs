//! One Sinkhorn-Knopp iteration for entropic optimal transport.
//!
//! Given a non-negative `m × n` cost matrix `C` and target marginals
//! `a` (size m) and `b` (size n), the Sinkhorn algorithm computes
//! the entropy-regularized optimal-transport plan
//!
//! ```text
//!   T = diag(u) · K · diag(v)
//!   K = exp(-C / ε)
//! ```
//!
//! by alternating row-then-column normalization on `K`:
//!
//! ```text
//!   u ← a ./ (K · v)
//!   v ← b ./ (Kᵀ · u)
//! ```
//!
//! Each iteration is two matrix-vector products + two elementwise
//! divides. Both matvecs are special cases of
//! [`crate::math::semiring_gemm`] with shape `m × n · n × 1` /
//! `n × m · m × 1` over the `Real` semiring.
//!
//! This file ships the **scaling-step combiner** that takes a
//! pre-computed `K · v` and divides `a` by it elementwise to update
//! `u`. Composing `semiring_gemm` (matvec) + this primitive +
//! `semiring_gemm` (transposed matvec) + this primitive in the
//! caller's loop gives the full Sinkhorn iteration.
//!
//! # Why this primitive is dual-use
//!
//! | Consumer | Use |
//! |---|---|
//! | future `vyre-libs::ml::ot` | Wasserstein loss / EMD |
//! | future `vyre-libs::ml::alignment` | distribution alignment / GAN training |
//! | `vyre-libs::parsing::c::sema` (#5 typedef classification) | identifier→typedef bipartite as soft assignment via Sinkhorn |
//!
//! Self-consumer is weak today; revisit when an internal soft-
//! assignment use materializes (e.g. dispatch-graph clustering
//! via Sinkhorn-OT distance between cost-vector distributions).
//!
//! # Fixed-point convention
//!
//! u32 16.16 fixed-point everywhere. The `K` matrix is precomputed
//! by the caller (typically `K[i,j] = exp_fp(-C[i,j] / eps_scaled)`
//! using a separate elementwise op). Numerical floor: `Kv` cells
//! near zero saturate to 1 to avoid divide-by-zero (callers tighten
//! ε to control floor activation).

use vyre_foundation::ir::{DataType, Expr, Program};

/// Op id for the scaling-update primitive.
pub const OP_ID: &str = "vyre-primitives::math::sinkhorn_scale";

/// Numerical floor for the divisor  -  values below saturate to this so
/// the divide doesn't return MAX. 1 in 16.16 = 65_536 / 65_536; here
/// we just guard against zero exactly.
pub const DIVISOR_FLOOR: u32 = 1;

/// Emit `out[i] = target[i] / max(divisor[i], FLOOR)` for `count`
/// lanes. Callers chain two of these with two matvec dispatches per
/// Sinkhorn iteration:
///   (1) Kv = semiring_gemm(K, v, n, n, 1, Real)
///   (2) u  = sinkhorn_scale(a, Kv)
///   (3) Ktu = semiring_gemm(K.T, u, n, m, 1, Real)
///   (4) v  = sinkhorn_scale(b, Ktu)
#[must_use]
pub fn sinkhorn_scale(target: &str, divisor: &str, out: &str, count: u32) -> Program {
    if count == 0 {
        return crate::invalid_output_program(
            OP_ID,
            out,
            DataType::U32,
            format!("Fix: sinkhorn_scale requires count > 0, got {count}."),
        );
    }

    crate::math::u32_binary_map::u32_binary_map_program(
        OP_ID,
        target,
        divisor,
        out,
        count,
        |target_value, divisor_value| {
            // d_safe = max(divisor[i], FLOOR)  -  done as select(d == 0, FLOOR, d)
            // (assuming we mostly want to guard against literal zero; fixed-point
            // small-positive values pass through).
            let d_safe = Expr::select(
                Expr::eq(divisor_value.clone(), Expr::u32(0)),
                Expr::u32(DIVISOR_FLOOR),
                divisor_value,
            );
            Expr::div(target_value, d_safe)
        },
    )
}

/// CPU reference operating in f64 for numerical clarity. Returns
/// `(u, v)` after one full Sinkhorn iteration starting from
/// `(u_init, v_init)`.
///
/// `k` is the kernel `exp(-C/ε)` of shape `m × n` row-major.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn sinkhorn_iter_cpu(
    k: &[f64],
    a: &[f64],
    b: &[f64],
    u: &mut [f64],
    v: &mut [f64],
    m: u32,
    n: u32,
) {
    let mut kv = Vec::new();
    let mut ktu = Vec::new();
    try_sinkhorn_iter_cpu_into(k, a, b, u, v, m, n, &mut kv, &mut ktu)
        .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - sinkhorn_iter_cpu failed: invalid Sinkhorn shape");
}

/// CPU reference using caller-owned temporary vectors.
#[allow(clippy::too_many_arguments)]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn sinkhorn_iter_cpu_into(
    k: &[f64],
    a: &[f64],
    b: &[f64],
    u: &mut [f64],
    v: &mut [f64],
    m: u32,
    n: u32,
    kv: &mut Vec<f64>,
    ktu: &mut Vec<f64>,
) {
    try_sinkhorn_iter_cpu_into(k, a, b, u, v, m, n, kv, ktu)
        .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - sinkhorn_iter_cpu_into failed: invalid Sinkhorn shape");
}

/// Fallible CPU reference using caller-owned temporary vectors.
#[allow(clippy::too_many_arguments)]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_sinkhorn_iter_cpu_into(
    k: &[f64],
    a: &[f64],
    b: &[f64],
    u: &mut [f64],
    v: &mut [f64],
    m: u32,
    n: u32,
    kv: &mut Vec<f64>,
    ktu: &mut Vec<f64>,
) -> Result<(), String> {
    let m = usize::try_from(m)
        .map_err(|_| format!("sinkhorn_iter CPU oracle m={m} does not fit usize."))?;
    let n = usize::try_from(n)
        .map_err(|_| format!("sinkhorn_iter CPU oracle n={n} does not fit usize."))?;
    m.checked_mul(n)
        .ok_or_else(|| format!("sinkhorn_iter CPU oracle K shape overflows: m={m}, n={n}."))?;

    reserve_sinkhorn_tmp(kv, m, "K*v temporary")?;
    kv.clear();
    kv.resize(m, 0.0);
    for i in 0..m {
        for j in 0..n {
            let k_ij = k.get(i * n + j).copied().unwrap_or(0.0);
            let v_j = v.get(j).copied().unwrap_or(0.0);
            kv[i] += k_ij * v_j;
        }
    }
    for i in 0..m {
        if let Some(u_i) = u.get_mut(i) {
            *u_i = a.get(i).copied().unwrap_or(0.0) / kv[i].max(1e-30);
        }
    }
    reserve_sinkhorn_tmp(ktu, n, "K^T*u temporary")?;
    ktu.clear();
    ktu.resize(n, 0.0);
    for j in 0..n {
        for i in 0..m {
            let k_ij = k.get(i * n + j).copied().unwrap_or(0.0);
            let u_i = u.get(i).copied().unwrap_or(0.0);
            ktu[j] += k_ij * u_i;
        }
    }
    for j in 0..n {
        if let Some(v_j) = v.get_mut(j) {
            *v_j = b.get(j).copied().unwrap_or(0.0) / ktu[j].max(1e-30);
        }
    }
    Ok(())
}

#[cfg(any(test, feature = "cpu-parity"))]
fn reserve_sinkhorn_tmp(out: &mut Vec<f64>, len: usize, name: &str) -> Result<(), String> {
    if len > out.capacity() {
        crate::graph::scratch::reserve_graph_items(
            out,
            len - out.len(),
            "Sinkhorn CPU oracle",
            name,
        )?;
    }
    Ok(())
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        OP_ID,
        || {
            sinkhorn_scale("a", "b", "out", 4)
        },
        Some(|| {
            vec![vec![
                crate::wire::pack_u32_slice(&[8, 9, 10, 11]),
                crate::wire::pack_u32_slice(&[2, 3, 0, 5]),
                crate::wire::pack_u32_slice(&[0; 4]),
            ]]
        }),
        Some(|| {
            vec![vec![crate::wire::pack_u32_slice(&[4, 3, 10, 2])]]
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPS: f64 = 1e-6;

    fn approx_eq(a: f64, b: f64) -> bool {
        (a - b).abs() < EPS * (1.0 + a.abs() + b.abs())
    }

    #[test]
    fn cpu_uniform_marginals_converge_to_uniform_plan() {
        // m=n=2, K = [[1, 1], [1, 1]] (cost matrix all zero, exp(0)=1).
        // a = b = [0.5, 0.5]. Expected u = v = [0.5, 0.5] after iter.
        let k = vec![1.0, 1.0, 1.0, 1.0];
        let a = vec![0.5, 0.5];
        let b = vec![0.5, 0.5];
        let mut u = vec![1.0, 1.0];
        let mut v = vec![1.0, 1.0];
        sinkhorn_iter_cpu(&k, &a, &b, &mut u, &mut v, 2, 2);
        // After one iter: Kv = [2.0, 2.0]; u = [0.25, 0.25]; Ktu = [0.5, 0.5]; v = [1.0, 1.0]
        assert!(approx_eq(u[0], 0.25));
        assert!(approx_eq(u[1], 0.25));
        assert!(approx_eq(v[0], 1.0));
        assert!(approx_eq(v[1], 1.0));
    }

    #[test]
    fn cpu_iterations_converge_to_balanced_plan() {
        // Repeated iterations should drive (u, v) toward a fixed point
        // where T = diag(u) K diag(v) is doubly stochastic.
        let k = vec![1.0, 0.5, 0.5, 1.0];
        let a = vec![0.5, 0.5];
        let b = vec![0.5, 0.5];
        let mut u = vec![1.0, 1.0];
        let mut v = vec![1.0, 1.0];
        for _ in 0..50 {
            sinkhorn_iter_cpu(&k, &a, &b, &mut u, &mut v, 2, 2);
        }
        // T marginals should match a and b.
        let row_sum_0 = u[0] * (k[0] * v[0] + k[1] * v[1]);
        let row_sum_1 = u[1] * (k[2] * v[0] + k[3] * v[1]);
        let col_sum_0 = v[0] * (k[0] * u[0] + k[2] * u[1]);
        let col_sum_1 = v[1] * (k[1] * u[0] + k[3] * u[1]);
        assert!(approx_eq(row_sum_0, a[0]));
        assert!(approx_eq(row_sum_1, a[1]));
        assert!(approx_eq(col_sum_0, b[0]));
        assert!(approx_eq(col_sum_1, b[1]));
    }

    #[test]
    fn cpu_zero_in_divisor_handled() {
        // If a row of K · v is 0, the floor in the GPU primitive saves
        // us. CPU ref uses .max(1e-30) which approximates the same.
        let k = vec![0.0, 0.0, 1.0, 1.0];
        let a = vec![0.5, 0.5];
        let b = vec![0.5, 0.5];
        let mut u = vec![1.0, 1.0];
        let mut v = vec![1.0, 1.0];
        sinkhorn_iter_cpu(&k, &a, &b, &mut u, &mut v, 2, 2);
        assert!(u[0].is_finite());
        assert!(u[1].is_finite());
    }

    #[test]
    fn cpu_into_reuses_sinkhorn_temporaries() {
        let k = vec![1.0, 1.0, 1.0, 1.0];
        let a = vec![0.5, 0.5];
        let b = vec![0.5, 0.5];
        let mut u = vec![1.0, 1.0];
        let mut v = vec![1.0, 1.0];
        let mut kv = Vec::new();
        let mut ktu = Vec::new();

        sinkhorn_iter_cpu_into(&k, &a, &b, &mut u, &mut v, 2, 2, &mut kv, &mut ktu);
        let kv_ptr = kv.as_ptr();
        let ktu_ptr = ktu.as_ptr();
        sinkhorn_iter_cpu_into(&k, &a, &b, &mut u, &mut v, 2, 2, &mut kv, &mut ktu);

        assert_eq!(kv.as_ptr(), kv_ptr);
        assert_eq!(ktu.as_ptr(), ktu_ptr);
    }

    #[test]
    fn cpu_into_truncates_stale_temporaries_without_reallocating() {
        let k = vec![1.0, 1.0, 1.0, 1.0];
        let a = vec![0.5, 0.5];
        let b = vec![0.5, 0.5];
        let mut u = vec![1.0, 1.0];
        let mut v = vec![1.0, 1.0];
        let mut kv = Vec::with_capacity(8);
        let mut ktu = Vec::with_capacity(8);
        kv.extend([99.0; 8]);
        ktu.extend([99.0; 8]);
        let kv_ptr = kv.as_ptr();
        let ktu_ptr = ktu.as_ptr();

        try_sinkhorn_iter_cpu_into(&k, &a, &b, &mut u, &mut v, 2, 2, &mut kv, &mut ktu).unwrap();

        assert_eq!(kv.len(), 2);
        assert_eq!(ktu.len(), 2);
        assert_eq!(kv.as_ptr(), kv_ptr);
        assert_eq!(ktu.as_ptr(), ktu_ptr);
    }

    #[test]
    fn generated_cpu_iter_matches_independent_reference() {
        for case in 0..48 {
            let m = 1 + (case % 5);
            let n = 1 + (case % 4);
            let k: Vec<f64> = (0..m * n)
                .map(|idx| 0.1 + (idx + case) as f64 * 0.01)
                .collect();
            let a: Vec<f64> = (0..m).map(|idx| 0.25 + idx as f64 * 0.05).collect();
            let b: Vec<f64> = (0..n).map(|idx| 0.5 + idx as f64 * 0.025).collect();
            let mut u = vec![1.0; m];
            let mut v = vec![1.0; n];
            let mut kv = Vec::with_capacity(m + 2);
            let mut ktu = Vec::with_capacity(n + 2);

            try_sinkhorn_iter_cpu_into(
                &k, &a, &b, &mut u, &mut v, m as u32, n as u32, &mut kv, &mut ktu,
            )
            .unwrap();

            for i in 0..m {
                let expected_kv: f64 = (0..n).map(|j| k[i * n + j]).sum();
                assert!(approx_eq(kv[i], expected_kv), "case {case} kv[{i}]");
                assert!(
                    approx_eq(u[i], a[i] / expected_kv.max(1e-30)),
                    "case {case} u[{i}]"
                );
            }
            for j in 0..n {
                let expected_ktu: f64 = (0..m).map(|i| k[i * n + j] * u[i]).sum();
                assert!(approx_eq(ktu[j], expected_ktu), "case {case} ktu[{j}]");
                assert!(
                    approx_eq(v[j], b[j] / expected_ktu.max(1e-30)),
                    "case {case} v[{j}]"
                );
            }
        }
    }

    #[test]
    fn cpu_short_inputs_update_available_lanes_only() {
        let mut u = vec![1.0];
        let mut v = vec![1.0];
        let mut kv = Vec::new();
        let mut ktu = Vec::new();
        sinkhorn_iter_cpu_into(&[1.0], &[], &[], &mut u, &mut v, 2, 2, &mut kv, &mut ktu);
        assert_eq!(u.len(), 1);
        assert_eq!(v.len(), 1);
    }

    #[test]
    fn ir_program_buffer_layout() {
        let p = sinkhorn_scale("a", "kv", "u", 32);
        assert_eq!(p.workgroup_size, [256, 1, 1]);
        let names: Vec<&str> = p.buffers.iter().map(|b| b.name()).collect();
        assert_eq!(names, vec!["a", "kv", "u"]);
        for buf in p.buffers.iter() {
            assert_eq!(buf.count(), 32);
        }
    }

    #[test]
    fn zero_count_traps() {
        let p = sinkhorn_scale("a", "kv", "u", 0);
        assert!(p.stats().trap());
    }
}
