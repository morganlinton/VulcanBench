//! Sparse recovery  -  Iterative Hard Thresholding (IHT) step.
//!
//! Compressed sensing recovers a k-sparse signal from few linear
//! measurements (Donoho 2006, Candès 2008). IHT (Blumensath-Davies
//! 2009) is the simplest GPU-friendly recovery algorithm:
//!
//! ```text
//!   x_{t+1} = H_k(x_t + Aᵀ (y - A x_t))
//! ```
//!
//! where `H_k(z)` keeps the top-k absolute values and zeros the rest.
//!
//! This file ships the **hard-thresholding step** primitive  -  given
//! the gradient-step output `z = x + Aᵀ(y - Ax)`, find the top-k
//! threshold and zero everything below.
//!
//! The matvec parts (`A x` and `Aᵀ residual`) are
//! [`crate::math::semiring_gemm`] calls in the caller's loop.
//!
//! # Why this primitive is dual-use
//!
//! | Consumer | Use |
//! |---|---|
//! | future `vyre-libs::signal::recovery` | compressed-sensing decoders |
//! | future `vyre-libs::ml::pruning` | structured-sparsity NN pruning |
//! | future `vyre-libs::ml::dictionary` | dictionary learning |
//! | `vyre-foundation::transform` sparse-buffer compaction | when a Region's output is mostly zero, IHT picks the threshold that keeps the top-k non-zeros. The same primitive ships to user signal-recovery dialects. |

use vyre_foundation::ir::{DataType, Expr, Program};

/// Op id.
pub const OP_ID: &str = "vyre-primitives::math::iht_threshold";

/// Emit the hard-threshold Program.
///
/// Inputs:
/// - `z`: length-`n` u32 buffer (signed values in two's complement  -
///   `|z|` is taken at compare time).
/// - `threshold`: single-element u32 buffer; values with absolute
///   value below this are zeroed. Caller computes `threshold` as the
///   k-th largest `|z|` (typically via a sort-then-pick pass).
///
/// Output:
/// - `out`: length-`n` u32 buffer with everything below threshold
///   zeroed.
#[must_use]
pub fn iht_threshold(z: &str, threshold: &str, out: &str, n: u32) -> Program {
    if n == 0 {
        return crate::invalid_output_program(
            OP_ID,
            out,
            DataType::U32,
            format!("Fix: iht_threshold requires n > 0, got {n}."),
        );
    }

    // We treat z as i32 for the sign-aware threshold compare:
    //   abs_z = (z as i32).abs() as u32
    // The fixed-point convention stores the sign in the high bit and
    // the remaining 31 bits hold the magnitude, so masking yields the
    // threshold magnitude used by this primitive.
    crate::math::u32_binary_map::u32_vector_scalar_map_program(
        OP_ID,
        z,
        threshold,
        out,
        n,
        |value, threshold| {
            let abs_z = Expr::bitand(value.clone(), Expr::u32(0x7FFF_FFFF));
            Expr::select(Expr::ge(abs_z, threshold), value, Expr::u32(0))
        },
    )
}

/// CPU reference: keep top-k absolute values; zero the rest. Returns
/// the kept values + the threshold (k-th largest `|z|`).
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn iht_top_k_cpu(z: &[f64], k: usize) -> (Vec<f64>, f64) {
    try_iht_top_k_cpu(z, k).unwrap_or_else(|error| panic!("{error}"))
}

/// Caller-owned workspace for IHT top-k CPU thresholding.
#[cfg(any(test, feature = "cpu-parity"))]
#[derive(Debug, Default, Clone)]
pub struct IhtTopKScratch {
    /// Sorted candidate indices ordered by finite absolute magnitude descending.
    pub order: Vec<usize>,
}

#[cfg(any(test, feature = "cpu-parity"))]
impl IhtTopKScratch {
    /// Create empty reusable IHT top-k scratch.
    pub fn new() -> Self {
        Self::default()
    }
}

/// Fallible CPU reference: keep top-k absolute values; zero the rest.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_iht_top_k_cpu(z: &[f64], k: usize) -> Result<(Vec<f64>, f64), String> {
    let mut out = Vec::new();
    let mut scratch = IhtTopKScratch::new();
    let threshold = try_iht_top_k_cpu_into(z, k, &mut out, &mut scratch)?;
    Ok((out, threshold))
}

/// Fallible CPU reference using caller-owned output and scratch storage.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_iht_top_k_cpu_into(
    z: &[f64],
    k: usize,
    out: &mut Vec<f64>,
    scratch: &mut IhtTopKScratch,
) -> Result<f64, String> {
    let n = z.len();
    if n > out.capacity() {
        crate::graph::scratch::reserve_graph_items(
            out,
            n - out.len(),
            "IHT sparse-recovery CPU oracle",
            "iht_top_k output",
        )?;
    }
    if n > scratch.order.capacity() {
        let additional = n - scratch.order.len();
        crate::graph::scratch::reserve_graph_items(
            &mut scratch.order,
            additional,
            "IHT sparse-recovery CPU oracle",
            "iht_top_k sorted indices",
        )?;
    }
    if k >= n {
        out.clear();
        out.extend_from_slice(z);
        scratch.order.clear();
        return Ok(0.0);
    }
    if k == 0 {
        out.clear();
        out.resize(n, 0.0);
        scratch.order.clear();
        return Ok(f64::INFINITY);
    }
    // Sort indices by |z| descending; threshold = |z[order[k-1]]|.
    scratch.order.clear();
    scratch.order.extend(0..n);
    scratch
        .order
        .sort_by(|&i, &j| finite_abs_score(z[j]).total_cmp(&finite_abs_score(z[i])));
    let threshold = z[scratch.order[k - 1]].abs();
    out.clear();
    out.resize(n, 0.0);
    for &i in &scratch.order[..k] {
        out[i] = z[i];
    }
    Ok(threshold)
}

#[cfg(any(test, feature = "cpu-parity"))]
fn finite_abs_score(value: f64) -> f64 {
    let abs = value.abs();
    if abs.is_nan() {
        f64::NEG_INFINITY
    } else {
        abs
    }
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        OP_ID,
        || {
            iht_threshold("a", "b", "out", 4)
        },
        Some(|| {
            vec![vec![
                crate::wire::pack_u32_slice(&[1, 2, 3, 4]),
                crate::wire::pack_u32_slice(&[3]),
                crate::wire::pack_u32_slice(&[0; 4]),
            ]]
        }),
        Some(|| {
            vec![vec![crate::wire::pack_u32_slice(&[0, 0, 3, 4])]]
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-10 * (1.0 + a.abs() + b.abs())
    }

    #[test]
    fn cpu_top_2_keeps_largest() {
        let z = vec![0.1, -2.0, 0.5, 3.0, -0.05];
        let (out, thresh) = iht_top_k_cpu(&z, 2);
        // top-2 |z| = 3.0 (idx 3) and 2.0 (idx 1)
        assert!(approx_eq(out[3], 3.0));
        assert!(approx_eq(out[1], -2.0));
        // others zero
        assert!(approx_eq(out[0], 0.0));
        assert!(approx_eq(out[2], 0.0));
        assert!(approx_eq(out[4], 0.0));
        assert!(approx_eq(thresh, 2.0));
    }

    #[test]
    fn cpu_k_equals_n_returns_all() {
        let z = vec![1.0, 2.0, 3.0];
        let (out, _) = iht_top_k_cpu(&z, 3);
        assert_eq!(out, z);
    }

    #[test]
    fn cpu_k_zero_zeros_all() {
        let z = vec![1.0, 2.0, 3.0];
        let (out, thresh) = iht_top_k_cpu(&z, 0);
        for v in out {
            assert!(approx_eq(v, 0.0));
        }
        assert!(thresh.is_infinite());
    }

    #[test]
    fn cpu_preserves_signs() {
        let z = vec![-5.0, 3.0, -7.0];
        let (out, _) = iht_top_k_cpu(&z, 2);
        // top-2 by magnitude: idx 2 (-7) and idx 0 (-5)
        assert!(approx_eq(out[2], -7.0));
        assert!(approx_eq(out[0], -5.0));
        assert!(approx_eq(out[1], 0.0));
    }

    #[test]
    fn cpu_into_reuses_output_and_scratch_and_truncates_stale_tail() {
        let z = vec![0.1, -2.0, 0.5, 3.0, -0.05];
        let mut out = Vec::with_capacity(8);
        out.extend_from_slice(&[99.0, 98.0, 97.0, 96.0, 95.0, 94.0, 93.0, 92.0]);
        let mut scratch = IhtTopKScratch {
            order: Vec::with_capacity(8),
        };
        scratch.order.extend_from_slice(&[7, 6, 5, 4, 3, 2, 1, 0]);
        let out_ptr = out.as_ptr();
        let order_ptr = scratch.order.as_ptr();
        let out_capacity = out.capacity();
        let order_capacity = scratch.order.capacity();

        let threshold = try_iht_top_k_cpu_into(&z, 2, &mut out, &mut scratch)
            .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - IHT top-k CPU oracle should reuse caller-owned storage");

        assert!(approx_eq(threshold, 2.0));
        assert_eq!(out.len(), z.len());
        assert!(approx_eq(out[3], 3.0));
        assert!(approx_eq(out[1], -2.0));
        assert_eq!(out.as_ptr(), out_ptr);
        assert_eq!(scratch.order.as_ptr(), order_ptr);
        assert_eq!(out.capacity(), out_capacity);
        assert_eq!(scratch.order.capacity(), order_capacity);

        let threshold = try_iht_top_k_cpu_into(&[4.0], 1, &mut out, &mut scratch)
            .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - IHT top-k CPU oracle should truncate stale output");

        assert!(approx_eq(threshold, 0.0));
        assert_eq!(out, vec![4.0]);
        assert!(scratch.order.is_empty());
        assert_eq!(out.as_ptr(), out_ptr);
        assert_eq!(scratch.order.as_ptr(), order_ptr);
    }

    #[test]
    fn generated_iht_top_k_matches_independent_reference() {
        let mut out = Vec::new();
        let mut scratch = IhtTopKScratch::new();
        for case in 0..2048usize {
            let len = case % 97;
            let k = (case * 7) % 113;
            let z: Vec<f64> = (0..len)
                .map(|idx| {
                    if (idx + case) % 53 == 0 {
                        f64::NAN
                    } else {
                        ((idx * 17 + case) % 101) as f64 / 9.0 - 5.0
                    }
                })
                .collect();
            let actual_threshold = try_iht_top_k_cpu_into(&z, k, &mut out, &mut scratch)
                .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - generated IHT top-k CPU oracle should evaluate");
            let (expected, expected_threshold) = independent_iht_top_k(&z, k);

            assert_eq!(out.len(), expected.len(), "case {case}: output length");
            for idx in 0..out.len() {
                if expected[idx].is_nan() {
                    assert!(out[idx].is_nan(), "case {case} idx {idx}: expected NaN");
                } else {
                    assert!(
                        approx_eq(out[idx], expected[idx]),
                        "case {case} idx {idx}: expected {}, got {}",
                        expected[idx],
                        out[idx]
                    );
                }
            }
            if expected_threshold.is_nan() {
                assert!(
                    actual_threshold.is_nan(),
                    "case {case}: expected NaN threshold"
                );
            } else if expected_threshold.is_infinite() {
                assert_eq!(
                    actual_threshold, expected_threshold,
                    "case {case}: expected infinite threshold"
                );
            } else {
                assert!(
                    approx_eq(actual_threshold, expected_threshold),
                    "case {case}: threshold"
                );
            }
        }
    }

    fn independent_iht_top_k(z: &[f64], k: usize) -> (Vec<f64>, f64) {
        let n = z.len();
        if k >= n {
            return (z.to_vec(), 0.0);
        }
        if k == 0 {
            return (vec![0.0; n], f64::INFINITY);
        }
        let mut order: Vec<usize> = (0..n).collect();
        order.sort_by(|&i, &j| finite_abs_score(z[j]).total_cmp(&finite_abs_score(z[i])));
        let threshold = z[order[k - 1]].abs();
        let mut out = vec![0.0; n];
        for &idx in &order[..k] {
            out[idx] = z[idx];
        }
        (out, threshold)
    }

    #[test]
    fn ir_program_buffer_layout() {
        let p = iht_threshold("z", "th", "out", 32);
        assert_eq!(p.workgroup_size, [256, 1, 1]);
        let names: Vec<&str> = p.buffers.iter().map(|b| b.name()).collect();
        assert_eq!(names, vec!["z", "th", "out"]);
        assert_eq!(p.buffers[0].count(), 32);
        assert_eq!(p.buffers[1].count(), 1);
        assert_eq!(p.buffers[2].count(), 32);
    }

    #[test]
    fn zero_n_traps() {
        let p = iht_threshold("z", "th", "out", 0);
        assert!(p.stats().trap());
    }
}
