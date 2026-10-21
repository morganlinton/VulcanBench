//! Score-based generative modeling  -  one denoise step.
//!
//! Diffusion models (Song-Ermon 2020, Ho 2020) and flow-matching
//! (Lipman 2023) reduce to "one denoise step":
//!
//! ```text
//!   x_{t-1} = α · x_t + β · score_θ(x_t, t) + σ · noise
//! ```
//!
//! where `α`, `β`, `σ` are step-dependent scalars from the noise
//! schedule, and `score_θ` is the user-provided model output.
//!
//! Each step is one elementwise multiply-add. This file ships the
//! primitive that combines `(x, score, noise)` with the schedule
//! coefficients. The score itself is computed by the caller's NN
//! Program; this primitive does the per-step blend.
//!
//! # Why this primitive is dual-use
//!
//! | Consumer | Use |
//! |---|---|
//! | future `vyre-libs::ml::diffusion` | image / audio / 3D generation |
//! | future `vyre-libs::ml::flow_matching` | continuous-time generative |
//! | future `vyre-libs::sim::particle` | stochastic differential equation simulation |
//!
//! Self-consumer is weak; flagged in `MATH_FRONTIER.md`.
//!
//! # Fixed-point convention
//!
//! u32 16.16 throughout. Coefficients are passed as 1-element buffers
//! so the caller can update them per step without recompilation.

use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Op id.
pub const OP_ID: &str = "vyre-primitives::math::score_denoise_step";

/// Emit `out[i] = (alpha * x[i] + beta * score[i] + sigma * noise[i]) >> 16`.
///
/// Scaling: alpha/beta/sigma are 16.16 fixed-point single-element
/// coefficients (caller updates per step from the schedule).
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn score_denoise_step(
    x: &str,
    score: &str,
    noise: &str,
    alpha: &str,
    beta: &str,
    sigma: &str,
    out: &str,
    n: u32,
) -> Program {
    if n == 0 {
        return crate::invalid_output_program(
            OP_ID,
            out,
            DataType::U32,
            format!("Fix: score_denoise_step requires n > 0, got {n}."),
        );
    }

    let t = Expr::InvocationId { axis: 0 };
    let a = Expr::load(alpha, Expr::u32(0));
    let b = Expr::load(beta, Expr::u32(0));
    let s = Expr::load(sigma, Expr::u32(0));

    let term_x = crate::fixed_mul_16_16_expr(a, Expr::load(x, t.clone()));
    let term_score = crate::fixed_mul_16_16_expr(b, Expr::load(score, t.clone()));
    let term_noise = crate::fixed_mul_16_16_expr(s, Expr::load(noise, t.clone()));
    let value = Expr::add(Expr::add(term_x, term_score), term_noise);

    let body = vec![Node::if_then(
        Expr::lt(t.clone(), Expr::u32(n)),
        vec![Node::store(out, t, value)],
    )];

    Program::wrapped(
        vec![
            BufferDecl::storage(x, 0, BufferAccess::ReadOnly, DataType::U32).with_count(n),
            BufferDecl::storage(score, 1, BufferAccess::ReadOnly, DataType::U32).with_count(n),
            BufferDecl::storage(noise, 2, BufferAccess::ReadOnly, DataType::U32).with_count(n),
            BufferDecl::storage(alpha, 3, BufferAccess::ReadOnly, DataType::U32).with_count(1),
            BufferDecl::storage(beta, 4, BufferAccess::ReadOnly, DataType::U32).with_count(1),
            BufferDecl::storage(sigma, 5, BufferAccess::ReadOnly, DataType::U32).with_count(1),
            BufferDecl::storage(out, 6, BufferAccess::ReadWrite, DataType::U32).with_count(n),
        ],
        [256, 1, 1],
        vec![Node::Region {
            generator: Ident::from(OP_ID),
            source_region: None,
            body: Arc::new(body),
        }],
    )
}

/// CPU reference. f64 for clarity; callers convert at the boundary.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn score_denoise_step_cpu(
    x: &[f64],
    score: &[f64],
    noise: &[f64],
    alpha: f64,
    beta: f64,
    sigma: f64,
) -> Vec<f64> {
    let n = x.len().min(score.len()).min(noise.len());
    let mut out = Vec::with_capacity(n);
    score_denoise_step_cpu_into(x, score, noise, alpha, beta, sigma, &mut out);
    out
}

/// CPU reference into caller-owned output storage.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn score_denoise_step_cpu_into(
    x: &[f64],
    score: &[f64],
    noise: &[f64],
    alpha: f64,
    beta: f64,
    sigma: f64,
    out: &mut Vec<f64>,
) {
    try_score_denoise_step_cpu_into(x, score, noise, alpha, beta, sigma, out)
        .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - score_denoise_step_cpu_into failed: output allocation failed");
}

/// Fallible CPU reference into caller-owned output storage.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_score_denoise_step_cpu_into(
    x: &[f64],
    score: &[f64],
    noise: &[f64],
    alpha: f64,
    beta: f64,
    sigma: f64,
    out: &mut Vec<f64>,
) -> Result<(), String> {
    let n = x.len().min(score.len()).min(noise.len());
    if n > out.capacity() {
        crate::graph::scratch::reserve_graph_items(
            out,
            n - out.len(),
            "score-denoise CPU oracle",
            "denoised output",
        )?;
    }
    out.clear();
    for i in 0..n {
        out.push(alpha * x[i] + beta * score[i] + sigma * noise[i]);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-10 * (1.0 + a.abs() + b.abs())
    }

    #[test]
    fn cpu_zero_noise_is_deterministic() {
        let x = vec![1.0, 2.0];
        let score = vec![0.5, 1.0];
        let noise = vec![0.0; 2];
        let out = score_denoise_step_cpu(&x, &score, &noise, 0.9, 0.1, 0.0);
        assert!(approx_eq(out[0], 0.9 * 1.0 + 0.1 * 0.5));
        assert!(approx_eq(out[1], 0.9 * 2.0 + 0.1 * 1.0));
    }

    #[test]
    fn cpu_zero_alpha_no_carry() {
        // alpha=0 means the previous state is forgotten.
        let x = vec![100.0];
        let score = vec![1.0];
        let noise = vec![1.0];
        let out = score_denoise_step_cpu(&x, &score, &noise, 0.0, 0.5, 0.5);
        assert!(approx_eq(out[0], 1.0));
    }

    #[test]
    fn cpu_pure_carry_alpha_one_others_zero() {
        let x = vec![3.5, 2.5];
        let score = vec![100.0, 100.0];
        let noise = vec![100.0, 100.0];
        let out = score_denoise_step_cpu(&x, &score, &noise, 1.0, 0.0, 0.0);
        assert_eq!(out, x);
    }

    #[test]
    fn cpu_mismatched_inputs_truncate_to_complete_triples() {
        let out = score_denoise_step_cpu(&[1.0, 2.0], &[3.0], &[4.0, 5.0], 1.0, 1.0, 1.0);
        assert_eq!(out, vec![8.0]);
    }

    #[test]
    fn cpu_into_reuses_output_and_truncates_stale_tail() {
        let mut out = Vec::with_capacity(8);
        out.extend([99.0; 8]);
        let ptr = out.as_ptr();

        try_score_denoise_step_cpu_into(&[1.0, 2.0], &[3.0], &[4.0, 5.0], 1.0, 1.0, 1.0, &mut out)
            .unwrap();

        assert_eq!(out, vec![8.0]);
        assert_eq!(out.as_ptr(), ptr);
    }

    #[test]
    fn generated_cpu_matches_independent_reference() {
        for case in 0..96 {
            let n = 1 + (case % 13);
            let x: Vec<f64> = (0..n).map(|i| i as f64 * 0.25 - 1.0).collect();
            let score: Vec<f64> = (0..n)
                .map(|i| case as f64 * 0.01 - i as f64 * 0.125)
                .collect();
            let noise: Vec<f64> = (0..n).map(|i| (i * i + case) as f64 * 0.001).collect();
            let alpha = 0.75 + (case % 5) as f64 * 0.01;
            let beta = -0.25 + (case % 7) as f64 * 0.02;
            let sigma = (case % 11) as f64 * 0.005;
            let mut out = Vec::with_capacity(n + 3);

            try_score_denoise_step_cpu_into(&x, &score, &noise, alpha, beta, sigma, &mut out)
                .unwrap();

            for i in 0..n {
                let expected = alpha * x[i] + beta * score[i] + sigma * noise[i];
                assert!(
                    approx_eq(out[i], expected),
                    "case {case} idx {i}: expected {expected}, got {}",
                    out[i]
                );
            }
        }
    }

    #[test]
    fn cpu_iterated_steps_converge_with_decay() {
        // Iterate alpha=0.5, beta=0, sigma=0  -  pure decay.
        // x_n = 0.5 * x_{n-1} = (0.5)^n * x_0
        let mut x = vec![16.0];
        for _ in 0..4 {
            x = score_denoise_step_cpu(&x, &[0.0], &[0.0], 0.5, 0.0, 0.0);
        }
        // 16 * 0.5^4 = 1.0
        assert!(approx_eq(x[0], 1.0));
    }

    #[test]
    fn ir_program_buffer_layout() {
        let p = score_denoise_step("x", "s", "n", "a", "b", "g", "o", 64);
        assert_eq!(p.workgroup_size, [256, 1, 1]);
        let names: Vec<&str> = p.buffers.iter().map(|b| b.name()).collect();
        assert_eq!(names, vec!["x", "s", "n", "a", "b", "g", "o"]);
        // n-sized
        for i in [0, 1, 2, 6] {
            assert_eq!(p.buffers[i].count(), 64);
        }
        // single-element coefficients
        for i in [3, 4, 5] {
            assert_eq!(p.buffers[i].count(), 1);
        }
    }

    #[test]
    fn zero_n_traps() {
        let p = score_denoise_step("x", "s", "n", "a", "b", "g", "o", 0);
        assert!(p.stats().trap());
    }
}
