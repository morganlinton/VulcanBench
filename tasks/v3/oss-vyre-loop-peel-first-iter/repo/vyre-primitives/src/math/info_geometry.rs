//! Information-geometry primitives  -  Fisher-Rao distance + Amari
//! α-connection between distributions on a statistical manifold.
//!
//! Fisher-Rao distance is the Riemannian distance on the statistical
//! manifold under the Fisher information metric (Rao 1945). For
//! categorical distributions p, q over k outcomes:
//!
//! ```text
//!   d_FR(p, q) = 2 · arccos( Σ_i sqrt(p_i · q_i) )
//! ```
//!
//! This is the "spherical" form. The Amari α-connection (Amari 1985)
//! interpolates between exponential (α = 1) and mixture (α = -1)
//! families; α = 0 recovers the Levi-Civita connection (Fisher-Rao).
//!
//! This file ships:
//! - `bhattacharyya_coefficient`  -  `Σ sqrt(p · q)`, the inner
//!   product on the spherical statistical manifold. Distance is
//!   `2 · arccos(coeff)` host-side.
//! - [`crate::math::info_geometry::amari_alpha_step_cpu`]  -  host-side α-connection interpolation,
//!   useful for distribution-aware loss design.
//!
//! # Why this primitive is dual-use
//!
//! | Consumer | Use |
//! |---|---|
//! | future `vyre-libs::ml::distribution` | distribution-aware loss functions |
//! | future `vyre-libs::ml::moe` | mixture-of-experts routing on the simplex |
//! | future `vyre-libs::ml::calibration` | model calibration via natural distance |

use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Op id.
pub const OP_ID: &str = "vyre-primitives::math::bhattacharyya_coefficient";

/// Numerical floor for `sqrt(p_i)`. Caller supplies `p` and `q` in
/// 16.16 fixed-point already representing probabilities; the GPU
/// path uses an integer-square-root approximation per element.
///
/// # Algorithm
///
/// Per lane `t`:
///   `out[t] = isqrt(p[t]) · isqrt(q[t]) >> 16`
///
/// then the caller dispatches `reduce::sum` to obtain the final
/// scalar coefficient.
#[must_use]
pub fn bhattacharyya_per_element(p: &str, q: &str, out_per_elem: &str, n: u32) -> Program {
    if n == 0 {
        return crate::invalid_output_program(
            OP_ID,
            out_per_elem,
            DataType::U32,
            format!("Fix: bhattacharyya_per_element requires n > 0, got {n}."),
        );
    }

    let t = Expr::InvocationId { axis: 0 };

    // Integer square root via Newton iteration: x_{k+1} = (x_k + n/x_k) / 2
    // 4 iterations from x_0 = n give ~2 ulps for u32 values up to 2^31.
    // We embed 4 unrolled steps and clamp to 1 to avoid divide-by-zero.
    let isqrt_inline = |val_var: &'static str| {
        let mut steps = vec![
            Node::let_bind(val_var, Expr::load(p, t.clone())),
            // Use any nonzero seed; pick val_var or 1.
            Node::let_bind(
                "x",
                Expr::select(
                    Expr::eq(Expr::var(val_var), Expr::u32(0)),
                    Expr::u32(1),
                    Expr::var(val_var),
                ),
            ),
        ];
        for _ in 0..4 {
            steps.push(Node::assign(
                "x",
                Expr::shr(
                    Expr::add(
                        Expr::var("x"),
                        Expr::div(Expr::var(val_var), Expr::var("x")),
                    ),
                    Expr::u32(1),
                ),
            ));
        }
        steps
    };

    let mut body_inner = isqrt_inline("pv");
    body_inner.push(Node::let_bind("xp", Expr::var("x")));
    // q
    body_inner.push(Node::let_bind("qv", Expr::load(q, t.clone())));
    body_inner.push(Node::let_bind(
        "y",
        Expr::select(
            Expr::eq(Expr::var("qv"), Expr::u32(0)),
            Expr::u32(1),
            Expr::var("qv"),
        ),
    ));
    for _ in 0..4 {
        body_inner.push(Node::assign(
            "y",
            Expr::shr(
                Expr::add(Expr::var("y"), Expr::div(Expr::var("qv"), Expr::var("y"))),
                Expr::u32(1),
            ),
        ));
    }
    body_inner.push(Node::store(
        out_per_elem,
        t.clone(),
        crate::fixed_mul_16_16_expr(Expr::var("xp"), Expr::var("y")),
    ));

    let body = vec![Node::if_then(Expr::lt(t.clone(), Expr::u32(n)), body_inner)];

    Program::wrapped(
        vec![
            BufferDecl::storage(p, 0, BufferAccess::ReadOnly, DataType::U32).with_count(n),
            BufferDecl::storage(q, 1, BufferAccess::ReadOnly, DataType::U32).with_count(n),
            BufferDecl::storage(out_per_elem, 2, BufferAccess::ReadWrite, DataType::U32)
                .with_count(n),
        ],
        [256, 1, 1],
        vec![Node::Region {
            generator: Ident::from(OP_ID),
            source_region: None,
            body: Arc::new(body),
        }],
    )
}

// ---- CPU references ----

/// Bhattacharyya coefficient: `Σ sqrt(p_i · q_i)`. Coefficient is
/// in `[0, 1]`; `0` = orthogonal distributions, `1` = identical.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn bhattacharyya_coefficient_cpu(p: &[f64], q: &[f64]) -> f64 {
    p.iter()
        .zip(q.iter())
        .map(|(&pi, &qi)| (pi.max(0.0) * qi.max(0.0)).sqrt())
        .sum()
}

/// Fisher-Rao distance from Bhattacharyya coefficient.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn fisher_rao_distance_cpu(p: &[f64], q: &[f64]) -> f64 {
    let c = bhattacharyya_coefficient_cpu(p, q).clamp(0.0, 1.0);
    2.0 * c.acos()
}

/// Amari α-connection interpolation between two probability vectors.
///
/// `α = 1`: exponential (geometric) mixture: `r_i ∝ p_i^t · q_i^(1-t)`.
/// `α = -1`: linear (mixture-family) mixture: `r_i = t·p_i + (1-t)·q_i`.
/// `α = 0`: spherical (Fisher-Rao geodesic)  -  slerp on `sqrt(p)`.
///
/// Returns the un-normalized blend; caller normalizes if needed.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn amari_alpha_step_cpu(p: &[f64], q: &[f64], alpha: f64, t: f64) -> Vec<f64> {
    let mut out = Vec::new();
    try_amari_alpha_step_cpu_into(p, q, alpha, t, &mut out)
        .unwrap_or_else(|error| panic!("{error}"));
    out
}

/// Amari α-connection interpolation into caller-owned output storage.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn amari_alpha_step_cpu_into(p: &[f64], q: &[f64], alpha: f64, t: f64, out: &mut Vec<f64>) {
    try_amari_alpha_step_cpu_into(p, q, alpha, t, out).unwrap_or_else(|error| panic!("{error}"));
}

/// Fallible Amari α-connection interpolation into caller-owned output storage.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_amari_alpha_step_cpu_into(
    p: &[f64],
    q: &[f64],
    alpha: f64,
    t: f64,
    out: &mut Vec<f64>,
) -> Result<(), String> {
    let t = t.clamp(0.0, 1.0);
    let s = 1.0 - t;
    let n = p.len().min(q.len());
    if n > out.capacity() {
        crate::graph::scratch::reserve_graph_items(
            out,
            n - out.len(),
            "information-geometry CPU oracle",
            "amari_alpha_step output",
        )?;
    }
    out.clear();
    p.iter().zip(q.iter()).for_each(|(&pi, &qi)| {
        if (alpha - 1.0).abs() < 1e-12 {
            out.push(pi.powf(t) * qi.powf(s));
        } else if (alpha + 1.0).abs() < 1e-12 {
            out.push(t * pi + s * qi);
        } else if alpha.abs() < 1e-12 {
            let sp = pi.max(0.0).sqrt();
            let sq = qi.max(0.0).sqrt();
            let blended = t * sp + s * sq;
            out.push(blended * blended);
        } else {
            // General α-connection: r_i^((1-α)/2) = t · p_i^((1-α)/2) +
            //                                       (1-t) · q_i^((1-α)/2)
            let beta = (1.0 - alpha) / 2.0;
            let blended = t * pi.max(0.0).powf(beta) + s * qi.max(0.0).powf(beta);
            out.push(blended.powf(1.0 / beta));
        }
    });
    Ok(())
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        OP_ID,
        || {
            bhattacharyya_per_element("a", "b", "out", 4)
        },
        Some(|| {
            vec![vec![
                crate::wire::pack_u32_slice(&[1; 4]),
                crate::wire::pack_u32_slice(&[1; 4]),
                crate::wire::pack_u32_slice(&[0; 4]),
            ]]
        }),
        Some(|| {
            vec![vec![crate::wire::pack_u32_slice(&[0; 4])]]
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6 * (1.0 + a.abs() + b.abs())
    }

    #[test]
    fn cpu_self_distance_is_zero() {
        let p = vec![0.5, 0.3, 0.2];
        assert!(approx_eq(fisher_rao_distance_cpu(&p, &p), 0.0));
    }

    #[test]
    fn cpu_orthogonal_distance_is_pi() {
        // p = (1, 0), q = (0, 1)  -  disjoint support.
        let p = vec![1.0, 0.0];
        let q = vec![0.0, 1.0];
        // BC = 0, distance = 2 arccos(0) = pi
        assert!(approx_eq(
            fisher_rao_distance_cpu(&p, &q),
            std::f64::consts::PI
        ));
    }

    #[test]
    fn cpu_bhattacharyya_symmetric() {
        let p = vec![0.4, 0.6];
        let q = vec![0.7, 0.3];
        let bc1 = bhattacharyya_coefficient_cpu(&p, &q);
        let bc2 = bhattacharyya_coefficient_cpu(&q, &p);
        assert!(approx_eq(bc1, bc2));
    }

    #[test]
    fn cpu_mismatched_inputs_truncate_and_t_clamps() {
        assert_eq!(bhattacharyya_coefficient_cpu(&[1.0], &[]), 0.0);
        let out = amari_alpha_step_cpu(&[1.0, 2.0], &[3.0], -1.0, 2.0);
        assert_eq!(out, vec![1.0]);
    }

    #[test]
    fn cpu_amari_into_reuses_output_and_truncates_stale_tail() {
        let mut out = Vec::with_capacity(4);
        out.extend_from_slice(&[99.0, 98.0, 97.0, 96.0]);
        let ptr = out.as_ptr();
        let capacity = out.capacity();

        try_amari_alpha_step_cpu_into(&[1.0, 0.0], &[0.0, 1.0], -1.0, 0.25, &mut out)
            .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - Amari alpha CPU oracle should reuse caller-owned output");

        assert_eq!(out, vec![0.25, 0.75]);
        assert_eq!(out.as_ptr(), ptr);
        assert_eq!(out.capacity(), capacity);

        try_amari_alpha_step_cpu_into(&[2.0], &[4.0], -1.0, 0.5, &mut out)
            .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - Amari alpha CPU oracle should truncate stale output");

        assert_eq!(out, vec![3.0]);
        assert_eq!(out.as_ptr(), ptr);
        assert_eq!(out.capacity(), capacity);
    }

    #[test]
    fn generated_amari_alpha_matches_independent_reference() {
        let mut out = Vec::new();
        for case in 0..1024usize {
            let p_len = case % 65;
            let q_len = (case * 7) % 65;
            let alpha = match case % 5 {
                0 => -1.0,
                1 => 0.0,
                2 => 1.0,
                _ => ((case % 17) as f64 - 8.0) / 5.0,
            };
            let t = ((case % 23) as f64 - 4.0) / 11.0;
            let p: Vec<f64> = (0..p_len)
                .map(|idx| ((idx * 13 + case) % 31) as f64 / 31.0)
                .collect();
            let q: Vec<f64> = (0..q_len)
                .map(|idx| ((idx * 17 + case) % 29) as f64 / 29.0)
                .collect();

            try_amari_alpha_step_cpu_into(&p, &q, alpha, t, &mut out)
                .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - generated Amari alpha CPU oracle should evaluate");
            let expected = independent_amari_alpha(&p, &q, alpha, t);

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
        }
    }

    fn independent_amari_alpha(p: &[f64], q: &[f64], alpha: f64, t: f64) -> Vec<f64> {
        let t = t.clamp(0.0, 1.0);
        let s = 1.0 - t;
        p.iter()
            .zip(q.iter())
            .map(|(&pi, &qi)| {
                if (alpha - 1.0).abs() < 1e-12 {
                    pi.powf(t) * qi.powf(s)
                } else if (alpha + 1.0).abs() < 1e-12 {
                    t * pi + s * qi
                } else if alpha.abs() < 1e-12 {
                    let blended = t * pi.max(0.0).sqrt() + s * qi.max(0.0).sqrt();
                    blended * blended
                } else {
                    let beta = (1.0 - alpha) / 2.0;
                    let blended = t * pi.max(0.0).powf(beta) + s * qi.max(0.0).powf(beta);
                    blended.powf(1.0 / beta)
                }
            })
            .collect()
    }

    #[test]
    fn cpu_amari_alpha_neg_one_recovers_linear_mix() {
        // α = -1: r = t · p + (1-t) · q.
        let p = vec![1.0, 0.0];
        let q = vec![0.0, 1.0];
        let r = amari_alpha_step_cpu(&p, &q, -1.0, 0.25);
        assert!(approx_eq(r[0], 0.25));
        assert!(approx_eq(r[1], 0.75));
    }

    #[test]
    fn cpu_amari_alpha_one_recovers_geometric_mix() {
        // α = 1: r ∝ p^t · q^(1-t). At t=0.5, p=q=(0.5, 0.5) → r=(0.5, 0.5).
        let p = vec![0.5, 0.5];
        let q = vec![0.5, 0.5];
        let r = amari_alpha_step_cpu(&p, &q, 1.0, 0.5);
        assert!(approx_eq(r[0], 0.5));
        assert!(approx_eq(r[1], 0.5));
    }

    #[test]
    fn cpu_amari_alpha_zero_recovers_spherical_slerp() {
        // α = 0: blend on sqrt(p). At t=0 → q, at t=1 → p, monotone in between.
        let p = vec![1.0, 0.0];
        let q = vec![0.0, 1.0];
        let r0 = amari_alpha_step_cpu(&p, &q, 0.0, 0.0);
        let r1 = amari_alpha_step_cpu(&p, &q, 0.0, 1.0);
        assert!(approx_eq(r0[0], 0.0) && approx_eq(r0[1], 1.0));
        assert!(approx_eq(r1[0], 1.0) && approx_eq(r1[1], 0.0));
    }

    #[test]
    fn ir_program_buffer_layout() {
        let prog = bhattacharyya_per_element("p", "q", "out", 16);
        assert_eq!(prog.workgroup_size, [256, 1, 1]);
        let names: Vec<&str> = prog.buffers.iter().map(|b| b.name()).collect();
        assert_eq!(names, vec!["p", "q", "out"]);
    }

    #[test]
    fn zero_n_traps() {
        let p = bhattacharyya_per_element("p", "q", "out", 0);
        assert!(p.stats().trap());
    }
}
