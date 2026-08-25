//! Self-substrate wrappers for numerical optimizer and scientific kernels.
//!
//! These functions give scheduler and optimizer code named self-consumers for
//! math primitives without reimplementing the primitive algorithms here.

use vyre_foundation::ir::Program;
use vyre_primitives::math::{
    dp_accountant::{gaussian_rdp_step, rdp_to_dp},
    fractional::{
        grunwald_letnikov_kernel, grunwald_letnikov_kernel_into, kernel_to_fixed_16_16,
        kernel_to_fixed_16_16_into,
    },
    preconditioner::{newton_schulz_poly5_f32, newton_schulz_y_step},
    randomized_svd::randomized_projection_step,
    sinkhorn_iterate::sinkhorn_iterate,
};

#[cfg(any(test, feature = "cpu-parity"))]
use vyre_primitives::math::{
    dp_accountant::gaussian_rdp_step_cpu,
    fractional::{fractional_derivative_cpu, fractional_derivative_cpu_into},
    preconditioner::{
        newton_schulz_inverse_sqrt_cpu, newton_schulz_inverse_sqrt_cpu_into,
        newton_schulz_y_step_cpu, newton_schulz_y_step_cpu_into, NewtonSchulzScratch,
    },
    randomized_svd::{
        modified_gram_schmidt_cpu, modified_gram_schmidt_cpu_into, randomized_projection_step_cpu,
        randomized_projection_step_cpu_into,
    },
    sinkhorn_iterate::{
        cpu_ref as sinkhorn_cpu_ref, cpu_ref_into as sinkhorn_cpu_ref_into, sinkhorn_col_residual,
        sinkhorn_iterate_f64, sinkhorn_iterate_f64_into, sinkhorn_row_residual,
    },
};

/// Build a randomized projection dispatch for low-rank optimizer telemetry.
#[must_use]
pub fn dispatch_randomized_projection(
    a: &str,
    omega: &str,
    y: &str,
    m: u32,
    n: u32,
    l: u32,
) -> Program {
    randomized_projection_step(a, omega, y, m, n, l)
}

/// Build a Newton-Schulz Y-update dispatch.
#[must_use]
pub fn dispatch_newton_schulz_y_step(y_curr: &str, yzy: &str, y_next: &str, n: u32) -> Program {
    newton_schulz_y_step(y_curr, yzy, y_next, n)
}

/// Build the fused f32 Newton-Schulz quintic polynomial dispatch.
#[must_use]
pub fn dispatch_newton_schulz_poly5_f32(mat: &str, output: &str, rows: u32, cols: u32) -> Program {
    newton_schulz_poly5_f32(mat, output, rows, cols)
}

/// Build a quantized Sinkhorn fixed-point dispatch.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn dispatch_sinkhorn_iterate(
    k: &str,
    k_t: &str,
    a: &str,
    b: &str,
    u_curr: &str,
    u_next: &str,
    v: &str,
    kv: &str,
    ktu: &str,
    changed: &str,
    m: u32,
    n: u32,
    max_iterations: u32,
) -> Program {
    sinkhorn_iterate(
        k,
        k_t,
        a,
        b,
        u_curr,
        u_next,
        v,
        kv,
        ktu,
        changed,
        m,
        n,
        max_iterations,
    )
}

/// Build a Gaussian RDP per-step dispatch.
#[must_use]
pub fn dispatch_gaussian_rdp_step(
    alpha: &str,
    sigma_squared: &str,
    out: &str,
    count: u32,
) -> Program {
    gaussian_rdp_step(alpha, sigma_squared, out, count)
}

/// Generate a fractional Grünwald-Letnikov kernel for host-side conv1d staging.
#[must_use]
pub fn fractional_kernel(alpha: f64, n: u32) -> Vec<f64> {
    grunwald_letnikov_kernel(alpha, n)
}

/// Generate a fractional kernel into caller-owned storage.
pub fn fractional_kernel_into(alpha: f64, n: u32, out: &mut Vec<f64>) {
    grunwald_letnikov_kernel_into(alpha, n, out);
}

/// Convert a fractional kernel to 16.16 fixed-point weights.
#[must_use]
pub fn fractional_kernel_fixed_16_16(kernel: &[f64], step: f64, alpha: f64) -> Vec<u32> {
    kernel_to_fixed_16_16(kernel, step, alpha)
}

/// Convert a fractional kernel to 16.16 fixed-point weights in caller storage.
pub fn fractional_kernel_fixed_16_16_into(
    kernel: &[f64],
    step: f64,
    alpha: f64,
    out: &mut Vec<u32>,
) {
    kernel_to_fixed_16_16_into(kernel, step, alpha, out);
}

/// Convert RDP to epsilon for self-substrate private telemetry accounting.
#[must_use]
pub fn privacy_epsilon_from_rdp(rdp: f64, alpha: f64, delta: f64) -> f64 {
    rdp_to_dp(rdp, alpha, delta)
}

/// CPU randomized projection reference.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_randomized_projection(
    a: &[f64],
    omega: &[f64],
    m: u32,
    n: u32,
    l: u32,
) -> Vec<f64> {
    randomized_projection_step_cpu(a, omega, m, n, l)
}

/// CPU randomized projection reference into caller storage.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_randomized_projection_into(
    a: &[f64],
    omega: &[f64],
    m: u32,
    n: u32,
    l: u32,
    y: &mut Vec<f64>,
) {
    randomized_projection_step_cpu_into(a, omega, m, n, l, y);
}

/// CPU modified Gram-Schmidt reference.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_modified_gram_schmidt(y: &[f64], m: u32, l: u32) -> Vec<f64> {
    modified_gram_schmidt_cpu(y, m, l)
}

/// CPU modified Gram-Schmidt reference into caller storage.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_modified_gram_schmidt_into(y: &[f64], m: u32, l: u32, q: &mut Vec<f64>) {
    modified_gram_schmidt_cpu_into(y, m, l, q);
}

/// CPU fractional derivative reference.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_fractional_derivative(f: &[f64], alpha: f64, step: f64) -> Vec<f64> {
    fractional_derivative_cpu(f, alpha, step)
}

/// CPU fractional derivative reference into caller storage.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_fractional_derivative_into(
    f: &[f64],
    alpha: f64,
    step: f64,
    kernel: &mut Vec<f64>,
    out: &mut Vec<f64>,
) {
    fractional_derivative_cpu_into(f, alpha, step, kernel, out);
}

/// CPU Newton-Schulz Y-step reference.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_newton_schulz_y_step(y_curr: &[f64], yzy: &[f64]) -> Vec<f64> {
    newton_schulz_y_step_cpu(y_curr, yzy)
}

/// CPU Newton-Schulz Y-step reference into caller storage.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_newton_schulz_y_step_into(y_curr: &[f64], yzy: &[f64], out: &mut Vec<f64>) {
    newton_schulz_y_step_cpu_into(y_curr, yzy, out);
}

/// CPU Newton-Schulz inverse-square-root reference.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_newton_schulz_inverse_sqrt(m: &[f64], n: usize, iters: u32) -> Vec<f64> {
    newton_schulz_inverse_sqrt_cpu(m, n, iters)
}

/// CPU Newton-Schulz inverse-square-root reference into caller storage.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_newton_schulz_inverse_sqrt_into(
    m: &[f64],
    n: usize,
    iters: u32,
    out: &mut Vec<f64>,
    scratch: &mut NewtonSchulzScratch,
) {
    newton_schulz_inverse_sqrt_cpu_into(m, n, iters, out, scratch);
}

/// CPU quantized Sinkhorn reference.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
#[allow(clippy::too_many_arguments)]
pub fn reference_sinkhorn_quantized(
    k: &[u32],
    k_t: &[u32],
    a: &[u32],
    b: &[u32],
    u_curr: &[u32],
    v: &[u32],
    m: u32,
    n: u32,
    max_iterations: u32,
) -> (Vec<u32>, Vec<u32>, u32) {
    sinkhorn_cpu_ref(k, k_t, a, b, u_curr, v, m, n, max_iterations)
}

/// CPU quantized Sinkhorn reference into caller storage.
#[cfg(any(test, feature = "cpu-parity"))]
#[allow(clippy::too_many_arguments)]
pub fn reference_sinkhorn_quantized_into(
    k: &[u32],
    k_t: &[u32],
    a: &[u32],
    b: &[u32],
    u_curr: &[u32],
    v: &[u32],
    m: u32,
    n: u32,
    max_iterations: u32,
    u_out: &mut Vec<u32>,
    v_out: &mut Vec<u32>,
    u_old: &mut Vec<u32>,
) -> u32 {
    sinkhorn_cpu_ref_into(
        k,
        k_t,
        a,
        b,
        u_curr,
        v,
        m,
        n,
        max_iterations,
        u_out,
        v_out,
        u_old,
    )
}

/// CPU f64 Sinkhorn-Knopp reference.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_sinkhorn_f64(
    k: &[f64],
    a: &[f64],
    b: &[f64],
    tolerance: f64,
    max_iterations: u32,
) -> (Vec<f64>, Vec<f64>, u32) {
    sinkhorn_iterate_f64(k, a, b, tolerance, max_iterations)
}

/// CPU f64 Sinkhorn-Knopp reference into caller storage.
#[cfg(any(test, feature = "cpu-parity"))]
#[allow(clippy::too_many_arguments)]
pub fn reference_sinkhorn_f64_into(
    k: &[f64],
    a: &[f64],
    b: &[f64],
    tolerance: f64,
    max_iterations: u32,
    u: &mut Vec<f64>,
    v: &mut Vec<f64>,
    u_old: &mut Vec<f64>,
) -> u32 {
    sinkhorn_iterate_f64_into(k, a, b, tolerance, max_iterations, u, v, u_old)
}

/// CPU Sinkhorn row residual.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_sinkhorn_row_residual(k: &[f64], u: &[f64], v: &[f64], a: &[f64]) -> f64 {
    sinkhorn_row_residual(k, u, v, a)
}

/// CPU Sinkhorn column residual.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_sinkhorn_col_residual(k: &[f64], u: &[f64], v: &[f64], b: &[f64]) -> f64 {
    sinkhorn_col_residual(k, u, v, b)
}

/// CPU Gaussian RDP reference.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_gaussian_rdp_step(alpha: &[f64], sigma_squared: &[f64]) -> Vec<f64> {
    gaussian_rdp_step_cpu(alpha, sigma_squared)
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre_foundation::ir::Node;

    fn approx_eq(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6 * (1.0 + a.abs() + b.abs())
    }

    fn program_generator(program: &Program) -> &str {
        let Some(Node::Region { generator, .. }) = program.entry.first() else {
            panic!("Fix: numerical kernel Program must start with a Region.");
        };
        generator.as_str()
    }

    #[test]
    fn program_builders_emit_expected_numerical_primitives() {
        assert_eq!(
            program_generator(&dispatch_randomized_projection("a", "omega", "y", 2, 2, 2)),
            "vyre-primitives::math::randomized_projection_step"
        );
        assert_eq!(
            program_generator(&dispatch_newton_schulz_y_step("y", "yzy", "next", 2)),
            "vyre-primitives::math::newton_schulz_y_step"
        );
        assert_eq!(
            program_generator(&dispatch_newton_schulz_poly5_f32("mat", "out", 2, 2)),
            "vyre-primitives::math::newton_schulz_poly5_f32"
        );
        assert_eq!(
            program_generator(&dispatch_sinkhorn_iterate(
                "k", "kt", "a", "b", "uc", "un", "v", "kv", "ktu", "changed", 2, 2, 3
            )),
            "vyre-primitives::math::sinkhorn_iterate"
        );
        assert_eq!(
            program_generator(&dispatch_gaussian_rdp_step("alpha", "sigma", "out", 4)),
            "vyre-primitives::math::gaussian_rdp_step"
        );
    }

    #[test]
    fn fractional_wrappers_preserve_kernel_and_fixed_point_contracts() {
        let kernel = fractional_kernel(1.0, 3);
        assert!(approx_eq(kernel[0], 1.0));
        assert!(approx_eq(kernel[1], -1.0));
        assert!(approx_eq(kernel[2], 0.0));

        let mut kernel_into = Vec::with_capacity(3);
        fractional_kernel_into(1.0, 3, &mut kernel_into);
        assert_eq!(kernel, kernel_into);

        let fixed = fractional_kernel_fixed_16_16(&[1.0, -0.5], 1.0, 1.0);
        assert_eq!(fixed[0], 65536);
        assert_eq!(fixed[1] as i32, -32768);

        let mut fixed_into = Vec::with_capacity(2);
        fractional_kernel_fixed_16_16_into(&[1.0, -0.5], 1.0, 1.0, &mut fixed_into);
        assert_eq!(fixed, fixed_into);

        let derivative = reference_fractional_derivative(&[0.0, 1.0, 2.0], 1.0, 1.0);
        assert_eq!(derivative, vec![0.0, 1.0, 1.0]);

        let mut derivative_kernel = Vec::new();
        let mut derivative_into = Vec::new();
        reference_fractional_derivative_into(
            &[0.0, 1.0, 2.0],
            1.0,
            1.0,
            &mut derivative_kernel,
            &mut derivative_into,
        );
        assert_eq!(derivative_into, derivative);
    }

    #[test]
    fn randomized_projection_and_qr_references_match_contracts() {
        let a = [1.0, 0.0, 0.0, 1.0];
        let omega = [1.0, 0.0, 0.0, 1.0];
        let projection = reference_randomized_projection(&a, &omega, 2, 2, 2);
        assert_eq!(projection, a);

        let mut projection_into = Vec::with_capacity(4);
        reference_randomized_projection_into(&a, &omega, 2, 2, 2, &mut projection_into);
        assert_eq!(projection_into, projection);

        let q = reference_modified_gram_schmidt(&[1.0, 0.0, 0.0, 1.0], 2, 2);
        assert!(approx_eq(q[0], 1.0));
        assert!(approx_eq(q[3], 1.0));

        let mut q_into = Vec::with_capacity(4);
        reference_modified_gram_schmidt_into(&[1.0, 0.0, 0.0, 1.0], 2, 2, &mut q_into);
        assert_eq!(q_into, q);
    }

    #[test]
    fn newton_schulz_references_match_optimizer_contracts() {
        let y_step = reference_newton_schulz_y_step(&[0.5], &[0.25]);
        assert!(approx_eq(y_step[0], 0.625));

        let mut y_step_into = Vec::with_capacity(1);
        reference_newton_schulz_y_step_into(&[0.5], &[0.25], &mut y_step_into);
        assert_eq!(y_step_into, y_step);

        let inverse = reference_newton_schulz_inverse_sqrt(&[1.0, 0.0, 0.0, 1.0], 2, 12);
        assert!(approx_eq(inverse[0], 1.0));
        assert!(approx_eq(inverse[3], 1.0));

        let mut inverse_into = Vec::with_capacity(4);
        let mut scratch = NewtonSchulzScratch::new();
        reference_newton_schulz_inverse_sqrt_into(
            &[1.0, 0.0, 0.0, 1.0],
            2,
            12,
            &mut inverse_into,
            &mut scratch,
        );
        assert_eq!(inverse_into.len(), inverse.len());
    }

    #[test]
    fn sinkhorn_and_privacy_references_match_contracts() {
        let (u, v, _) = reference_sinkhorn_quantized(
            &[65536],
            &[65536],
            &[65536],
            &[65536],
            &[65536],
            &[65536],
            1,
            1,
            5,
        );
        assert_eq!(u, vec![65536]);
        assert_eq!(v, vec![65536]);

        let mut u_into = Vec::new();
        let mut v_into = Vec::new();
        let mut u_old = Vec::new();
        reference_sinkhorn_quantized_into(
            &[65536],
            &[65536],
            &[65536],
            &[65536],
            &[65536],
            &[65536],
            1,
            1,
            5,
            &mut u_into,
            &mut v_into,
            &mut u_old,
        );
        assert_eq!(u_into, u);
        assert_eq!(v_into, v);

        let (uf, vf, _) = reference_sinkhorn_f64(&[1.0], &[1.0], &[1.0], 1e-12, 10);
        assert!(reference_sinkhorn_row_residual(&[1.0], &uf, &vf, &[1.0]) < 1e-9);
        assert!(reference_sinkhorn_col_residual(&[1.0], &uf, &vf, &[1.0]) < 1e-9);

        let mut uf_into = Vec::new();
        let mut vf_into = Vec::new();
        let mut uf_old = Vec::new();
        reference_sinkhorn_f64_into(
            &[1.0],
            &[1.0],
            &[1.0],
            1e-12,
            10,
            &mut uf_into,
            &mut vf_into,
            &mut uf_old,
        );
        assert!(reference_sinkhorn_row_residual(&[1.0], &uf_into, &vf_into, &[1.0]) < 1e-9);

        let rdp = reference_gaussian_rdp_step(&[2.0], &[1.0]);
        assert!(approx_eq(rdp[0], 1.0));
        assert!(approx_eq(
            privacy_epsilon_from_rdp(0.0, 2.0, std::f64::consts::E.recip()),
            1.0
        ));
    }
}
