//! Self-substrate wrappers for advanced scientific and numerical kernels.
//!
//! This module is the dispatch glue for the scientific-math side of the
//! recursion thesis: information geometry, tensor trains, FMM, QSVT,
//! p-adics, SOS certificates, bigint carry propagation, tensor networks,
//! ODE integration, Sinkhorn scaling, score denoising, conformal intervals,
//! semiring GEMM, wide lineage joins, and Mori-Zwanzig projection. The
//! primitive crate owns the executable semantics; this crate owns only the
//! self-consumer dispatch surface and reusable CPU parity adapters.

use vyre_foundation::ir::Program;
use vyre_primitives::math::{
    bigint_add_carry::bigint_add_carry,
    conformal::conformal_threshold,
    fmm::{l2p_zeroth_f32_step, m2l_zeroth_f32_step, p2m_step, p2m_zeroth_f32_step},
    info_geometry::bhattacharyya_per_element,
    mori_zwanzig::mz_project_step,
    ode_step::rk4_step,
    padic::hensel_lift_step,
    qsvt::qsvt_block_encode,
    scallop_join_wide::semiring_gemm_wide,
    score_denoise::score_denoise_step,
    semiring_gemm::{semiring_gemm, Semiring},
    sinkhorn::sinkhorn_scale,
    sos_certificate::sos_gram_construct,
    tensor_network::tn_pair_contract,
    tensor_train::tt_contract_step,
};

#[cfg(any(test, feature = "cpu-parity"))]
use vyre_primitives::math::{
    bigint_add_carry::{
        bigint_add_carry_cpu, bigint_add_carry_cpu_into, resolve_carry_chain_cpu,
        resolve_carry_chain_cpu_into, BigIntAddCarryError,
    },
    conformal::{conformal_rank, predict_interval},
    info_geometry::{amari_alpha_step_cpu, bhattacharyya_coefficient_cpu, fisher_rao_distance_cpu},
    mori_zwanzig::{mz_project_step_cpu, mz_project_step_cpu_into},
    ode_step::rk4_step_cpu,
    padic::hensel_lift_step_cpu,
    qsvt::{
        qsvt_apply_cpu, qsvt_apply_cpu_into, qsvt_block_encode_cpu, qsvt_block_encode_cpu_into,
    },
    score_denoise::score_denoise_step_cpu,
    semiring_gemm::{semiring_gemm_cpu, semiring_gemm_cpu_into},
    sinkhorn::{sinkhorn_iter_cpu, sinkhorn_iter_cpu_into},
    sos_certificate::{is_psd_cpu, sos_gram_construct_cpu, sos_gram_construct_cpu_into},
    tensor_network::{greedy_contract_order_cpu, tn_pair_contract_cpu},
    tensor_train::{tt_contract_step_cpu_into, tt_full_chain_cpu, tt_full_chain_cpu_with_scratch},
};

/// Build a Bhattacharyya per-element information-geometry dispatch.
#[must_use]
pub fn dispatch_bhattacharyya_per_element(p: &str, q: &str, out_per_elem: &str, n: u32) -> Program {
    bhattacharyya_per_element(p, q, out_per_elem, n)
}

/// Build one tensor-train contraction step.
#[must_use]
pub fn dispatch_tt_contract_step(
    acc_in: &str,
    core_slice: &str,
    acc_out: &str,
    r_prev: u32,
    r_next: u32,
) -> Program {
    tt_contract_step(acc_in, core_slice, acc_out, r_prev, r_next)
}

/// Build an FMM particle-to-multipole dispatch.
#[must_use]
pub fn dispatch_p2m_step(
    particles: &str,
    cell_assignment: &str,
    cell_centers: &str,
    multipoles: &str,
    n_particles: u32,
    n_cells: u32,
) -> Program {
    p2m_step(
        particles,
        cell_assignment,
        cell_centers,
        multipoles,
        n_particles,
        n_cells,
    )
}

/// Build a zeroth-moment FMM scatter dispatch.
#[must_use]
pub fn dispatch_p2m_zeroth_f32_step(
    scores: &str,
    cell_assignment: &str,
    moments: &str,
    n_regions: u32,
    n_cells: u32,
) -> Program {
    p2m_zeroth_f32_step(scores, cell_assignment, moments, n_regions, n_cells)
}

/// Build a zeroth-moment FMM translate dispatch.
#[must_use]
pub fn dispatch_m2l_zeroth_f32_step(
    cell_moments: &str,
    cell_distances: &str,
    cell_local: &str,
    n_cells: u32,
) -> Program {
    m2l_zeroth_f32_step(cell_moments, cell_distances, cell_local, n_cells)
}

/// Build a zeroth-moment FMM evaluate dispatch.
#[must_use]
pub fn dispatch_l2p_zeroth_f32_step(
    cell_local: &str,
    cell_assignment: &str,
    region_out: &str,
    n_regions: u32,
    n_cells: u32,
) -> Program {
    l2p_zeroth_f32_step(cell_local, cell_assignment, region_out, n_regions, n_cells)
}

/// Build a QSVT block-encoding dispatch.
#[must_use]
pub fn dispatch_qsvt_block_encode(a: &str, norm: &str, a_scaled: &str, n: u32) -> Program {
    qsvt_block_encode(a, norm, a_scaled, n)
}

/// Build a Hensel-lift update dispatch.
#[must_use]
pub fn dispatch_hensel_lift_step(
    x: &str,
    f_x: &str,
    inv_f_prime: &str,
    out: &str,
    n: u32,
) -> Program {
    hensel_lift_step(x, f_x, inv_f_prime, out, n)
}

/// Build an SOS Gram-matrix construction dispatch.
#[must_use]
pub fn dispatch_sos_gram_construct(
    monomial_pairs: &str,
    p_coeffs: &str,
    gram: &str,
    m: u32,
    coeff_count: u32,
) -> Program {
    sos_gram_construct(monomial_pairs, p_coeffs, gram, m, coeff_count)
}

/// Build a limb-wise bigint add/carry dispatch.
#[must_use]
pub fn dispatch_bigint_add_carry(limb_count: u32) -> Program {
    bigint_add_carry(limb_count)
}

/// Build a tensor-network pair contraction dispatch.
#[must_use]
pub fn dispatch_tn_pair_contract(a: &str, b: &str, c: &str, m: u32, k: u32, n: u32) -> Program {
    tn_pair_contract(a, b, c, m, k, n)
}

/// Build one RK4 ODE update dispatch.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn dispatch_rk4_step(
    y_prev: &str,
    k1: &str,
    k2: &str,
    k3: &str,
    k4: &str,
    h_scaled: &str,
    y_next: &str,
    n: u32,
) -> Program {
    rk4_step(y_prev, k1, k2, k3, k4, h_scaled, y_next, n)
}

/// Build one Sinkhorn scale dispatch.
#[must_use]
pub fn dispatch_sinkhorn_scale(target: &str, divisor: &str, out: &str, count: u32) -> Program {
    sinkhorn_scale(target, divisor, out, count)
}

/// Build one score-denoising update dispatch.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn dispatch_score_denoise_step(
    x: &str,
    score: &str,
    noise: &str,
    alpha: &str,
    beta: &str,
    sigma: &str,
    out: &str,
    n: u32,
) -> Program {
    score_denoise_step(x, score, noise, alpha, beta, sigma, out, n)
}

/// Build a conformal threshold dispatch.
#[must_use]
pub fn dispatch_conformal_threshold(scores_sorted: &str, q_hat: &str, n: u32, k: u32) -> Program {
    conformal_threshold(scores_sorted, q_hat, n, k)
}

/// Compute the conformal rank used by self-substrate uncertainty gates.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_conformal_rank(n: u32, alpha: f64) -> u32 {
    conformal_rank(n, alpha)
}

/// Compute a symmetric conformal prediction interval.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_predict_interval(y: u32, q_hat: u32) -> (u32, u32) {
    predict_interval(y, q_hat)
}

/// Build a generic semiring GEMM dispatch.
#[must_use]
pub fn dispatch_semiring_gemm(
    a: &str,
    b: &str,
    c: &str,
    m: u32,
    n: u32,
    k: u32,
    semiring: Semiring,
) -> Program {
    semiring_gemm(a, b, c, m, n, k, semiring)
}

/// Build a wide-lineage semiring GEMM dispatch.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn dispatch_semiring_gemm_wide(
    a: &str,
    b: &str,
    c: &str,
    seed: Option<&str>,
    m: u32,
    n: u32,
    k: u32,
    w: u32,
) -> Program {
    semiring_gemm_wide(a, b, c, seed, m, n, k, w)
}

/// Build a Mori-Zwanzig projection dispatch.
#[must_use]
pub fn dispatch_mz_project_step(p_matrix: &str, f_vec: &str, out: &str, n: u32) -> Program {
    mz_project_step(p_matrix, f_vec, out, n)
}

/// Reference oracle for zeroth-moment FMM scatter.
///
/// The primitive module keeps its FMM CPU helpers test-local; self-substrate
/// needs an exported oracle because clustering and compression passes reuse
/// the same scatter contract in integration tests.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_p2m_zeroth_moment(charges: &[f64], cell_assignment: &[u32]) -> Vec<f64> {
    let cell_count = cell_assignment
        .iter()
        .copied()
        .max()
        .and_then(|cell| usize::try_from(cell).ok())
        .map_or(0, |cell| cell + 1);
    let mut moments = vec![0.0; cell_count];
    for (idx, charge) in charges.iter().copied().enumerate() {
        if let Some(cell) = cell_assignment
            .get(idx)
            .and_then(|cell| usize::try_from(*cell).ok())
        {
            if let Some(moment) = moments.get_mut(cell) {
                *moment += charge;
            }
        }
    }
    moments
}

/// CPU Bhattacharyya coefficient reference.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_bhattacharyya_coefficient(p: &[f64], q: &[f64]) -> f64 {
    bhattacharyya_coefficient_cpu(p, q)
}

/// CPU Fisher-Rao distance reference.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_fisher_rao_distance(p: &[f64], q: &[f64]) -> f64 {
    fisher_rao_distance_cpu(p, q)
}

/// CPU Amari-alpha interpolation reference.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_amari_alpha_step(p: &[f64], q: &[f64], alpha: f64, t: f64) -> Vec<f64> {
    amari_alpha_step_cpu(p, q, alpha, t)
}

/// CPU tensor-train contraction reference using caller-owned storage.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_tt_contract_step_into(
    acc_in: &[f64],
    core_slice: &[f64],
    r_prev: u32,
    r_next: u32,
    out: &mut Vec<f64>,
) {
    tt_contract_step_cpu_into(acc_in, core_slice, r_prev, r_next, out);
}

/// CPU full tensor-train chain reference.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_tt_full_chain(
    cores: &[Vec<f64>],
    ranks: &[u32],
    mode_dims: &[u32],
    indices: &[u32],
) -> f64 {
    tt_full_chain_cpu(cores, ranks, mode_dims, indices)
}

/// CPU full tensor-train chain reference using caller-owned scratch.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_tt_full_chain_with_scratch(
    cores: &[Vec<f64>],
    ranks: &[u32],
    mode_dims: &[u32],
    indices: &[u32],
    acc: &mut Vec<f64>,
    next: &mut Vec<f64>,
) -> f64 {
    tt_full_chain_cpu_with_scratch(cores, ranks, mode_dims, indices, acc, next)
}

/// CPU QSVT block-encoding reference.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_qsvt_block_encode(a: &[f64], n: u32) -> (Vec<f64>, f64) {
    qsvt_block_encode_cpu(a, n)
}

/// CPU QSVT block-encoding reference using caller-owned output storage.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_qsvt_block_encode_into(a: &[f64], n: u32, out: &mut Vec<f64>) -> f64 {
    qsvt_block_encode_cpu_into(a, n, out)
}

/// CPU QSVT polynomial-application reference.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_qsvt_apply(a_scaled: &[f64], v: &[f64], coeffs: &[f64], n: u32) -> Vec<f64> {
    qsvt_apply_cpu(a_scaled, v, coeffs, n)
}

/// CPU QSVT polynomial-application reference using caller-owned scratch.
#[cfg(any(test, feature = "cpu-parity"))]
#[allow(clippy::too_many_arguments)]
pub fn reference_qsvt_apply_into(
    a_scaled: &[f64],
    v: &[f64],
    coeffs: &[f64],
    n: u32,
    out: &mut Vec<f64>,
    t_prev: &mut Vec<f64>,
    t_curr: &mut Vec<f64>,
    t_next: &mut Vec<f64>,
) {
    qsvt_apply_cpu_into(a_scaled, v, coeffs, n, out, t_prev, t_curr, t_next);
}

/// CPU Hensel-lift reference.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_hensel_lift_step(x: f64, f_x: f64, inv_f_prime: f64) -> f64 {
    hensel_lift_step_cpu(x, f_x, inv_f_prime)
}

/// CPU SOS Gram-construction reference.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_sos_gram_construct(monomial_pairs: &[u32], p_coeffs: &[u32], m: u32) -> Vec<u32> {
    sos_gram_construct_cpu(monomial_pairs, p_coeffs, m)
}

/// CPU SOS Gram-construction reference using caller-owned output storage.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_sos_gram_construct_into(
    monomial_pairs: &[u32],
    p_coeffs: &[u32],
    m: u32,
    out: &mut Vec<u32>,
) {
    sos_gram_construct_cpu_into(monomial_pairs, p_coeffs, m, out);
}

/// CPU positive-semidefinite predicate reference.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_is_psd(matrix: &[f64], n: u32) -> bool {
    is_psd_cpu(matrix, n)
}

/// CPU limb add/carry reference.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_bigint_add_carry(
    a: &[u32],
    b: &[u32],
) -> Result<(Vec<u32>, Vec<u32>), BigIntAddCarryError> {
    bigint_add_carry_cpu(a, b)
}

/// CPU limb add/carry reference using caller-owned buffers.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_bigint_add_carry_into(
    a: &[u32],
    b: &[u32],
    sum_partial: &mut Vec<u32>,
    carry_partial: &mut Vec<u32>,
) -> Result<(), BigIntAddCarryError> {
    bigint_add_carry_cpu_into(a, b, sum_partial, carry_partial)
}

/// CPU carry-chain resolution reference.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_resolve_carry_chain(
    sum_partial: &[u32],
    carry_partial: &[u32],
) -> Result<(Vec<u32>, u32), BigIntAddCarryError> {
    resolve_carry_chain_cpu(sum_partial, carry_partial)
}

/// CPU carry-chain resolution reference using caller-owned output storage.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_resolve_carry_chain_into(
    sum_partial: &[u32],
    carry_partial: &[u32],
    final_sum: &mut Vec<u32>,
) -> Result<u32, BigIntAddCarryError> {
    resolve_carry_chain_cpu_into(sum_partial, carry_partial, final_sum)
}

/// CPU tensor-network pair contraction reference.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]

pub fn reference_tn_pair_contract(a: &[f64], b: &[f64], m: u32, k: u32, n: u32) -> Vec<f64> {
    tn_pair_contract_cpu(a, b, m, k, n)
}

/// CPU greedy tensor-network contraction ordering reference.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_greedy_contract_order(dims: &[u32]) -> Vec<usize> {
    greedy_contract_order_cpu(dims)
}

/// CPU RK4 reference.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_rk4_step(
    y_prev: &[f64],
    k1: &[f64],
    k2: &[f64],
    k3: &[f64],
    k4: &[f64],
    h: f64,
) -> Vec<f64> {
    rk4_step_cpu(y_prev, k1, k2, k3, k4, h)
}

/// CPU Sinkhorn iteration reference.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_sinkhorn_iter(
    k: &[f64],
    a: &[f64],
    b: &[f64],
    u: &mut [f64],
    v: &mut [f64],
    m: u32,
    n: u32,
) {
    sinkhorn_iter_cpu(k, a, b, u, v, m, n);
}

/// CPU Sinkhorn iteration reference using caller-owned scratch.
#[cfg(any(test, feature = "cpu-parity"))]
#[allow(clippy::too_many_arguments)]
pub fn reference_sinkhorn_iter_into(
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
    sinkhorn_iter_cpu_into(k, a, b, u, v, m, n, kv, ktu);
}

/// CPU score-denoising reference.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
#[allow(clippy::too_many_arguments)]
pub fn reference_score_denoise_step(
    x: &[f64],
    score: &[f64],
    noise: &[f64],
    alpha: f64,
    beta: f64,
    sigma: f64,
) -> Vec<f64> {
    score_denoise_step_cpu(x, score, noise, alpha, beta, sigma)
}

/// CPU semiring GEMM reference.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_semiring_gemm(
    a: &[u32],
    b: &[u32],
    m: u32,
    n: u32,
    k: u32,
    semiring: Semiring,
) -> Vec<u32> {
    semiring_gemm_cpu(a, b, m, n, k, semiring)
}

/// CPU semiring GEMM reference using caller-owned output storage.
#[cfg(any(test, feature = "cpu-parity"))]
#[allow(clippy::too_many_arguments)]
pub fn reference_semiring_gemm_into(
    a: &[u32],
    b: &[u32],
    m: u32,
    n: u32,
    k: u32,
    semiring: Semiring,
    c: &mut Vec<u32>,
) {
    semiring_gemm_cpu_into(a, b, m, n, k, semiring, c);
}

/// CPU Mori-Zwanzig projection reference.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_mz_project_step(p_matrix: &[f64], f_vec: &[f64], n: u32) -> Vec<f64> {
    mz_project_step_cpu(p_matrix, f_vec, n)
}

/// CPU Mori-Zwanzig projection reference using caller-owned output storage.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_mz_project_step_into(p_matrix: &[f64], f_vec: &[f64], n: u32, out: &mut Vec<f64>) {
    mz_project_step_cpu_into(p_matrix, f_vec, n, out);
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre_foundation::ir::Node;

    fn approx_eq(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-8 * (1.0 + a.abs() + b.abs())
    }

    fn program_generator(program: &Program) -> &str {
        let Some(Node::Region { generator, .. }) = program.entry.first() else {
            panic!("Fix: scientific kernel Program must start with a Region.");
        };
        generator.as_str()
    }

    #[test]
    fn program_builders_emit_expected_scientific_primitives() {
        assert_eq!(
            program_generator(&dispatch_bhattacharyya_per_element("p", "q", "out", 8)),
            "vyre-primitives::math::bhattacharyya_coefficient"
        );
        assert_eq!(
            program_generator(&dispatch_tt_contract_step("acc", "core", "out", 2, 2)),
            "vyre-primitives::math::tt_contract_step"
        );
        assert_eq!(
            program_generator(&dispatch_p2m_step(
                "particles",
                "assign",
                "centers",
                "m",
                4,
                2
            )),
            "vyre-primitives::math::fmm_p2m_step"
        );
        assert_eq!(
            program_generator(&dispatch_p2m_zeroth_f32_step(
                "scores", "assign", "moments", 4, 2
            )),
            "vyre-primitives::math::fmm_p2m_zeroth_f32_step"
        );
        assert_eq!(
            program_generator(&dispatch_m2l_zeroth_f32_step("moments", "dist", "local", 2)),
            "vyre-primitives::math::fmm_m2l_zeroth_f32_step"
        );
        assert_eq!(
            program_generator(&dispatch_l2p_zeroth_f32_step(
                "local", "assign", "out", 4, 2
            )),
            "vyre-primitives::math::fmm_l2p_zeroth_f32_step"
        );
        assert_eq!(
            program_generator(&dispatch_qsvt_block_encode("a", "norm", "scaled", 2)),
            "vyre-primitives::math::qsvt_block_encode"
        );
        assert_eq!(
            program_generator(&dispatch_hensel_lift_step("x", "fx", "df", "out", 2)),
            "vyre-primitives::math::hensel_lift_step"
        );
        assert_eq!(
            program_generator(&dispatch_sos_gram_construct(
                "pairs", "coeffs", "gram", 2, 3
            )),
            "vyre-primitives::math::sos_gram_construct"
        );
        assert_eq!(
            program_generator(&dispatch_bigint_add_carry(4)),
            "vyre-primitives::math::bigint_add_carry"
        );
        assert_eq!(
            program_generator(&dispatch_tn_pair_contract("a", "b", "c", 2, 2, 2)),
            "vyre-primitives::math::tensor_network_pair_contract"
        );
        assert_eq!(
            program_generator(&dispatch_rk4_step(
                "y", "k1", "k2", "k3", "k4", "h", "out", 2
            )),
            "vyre-primitives::math::ode_rk4_step"
        );
        assert_eq!(
            program_generator(&dispatch_sinkhorn_scale("target", "divisor", "out", 2)),
            "vyre-primitives::math::sinkhorn_scale"
        );
        assert_eq!(
            program_generator(&dispatch_score_denoise_step(
                "x", "score", "noise", "alpha", "beta", "sigma", "out", 2
            )),
            "vyre-primitives::math::score_denoise_step"
        );
        assert_eq!(
            program_generator(&dispatch_conformal_threshold("scores", "q", 8, 4)),
            "vyre-primitives::math::conformal_threshold"
        );
        assert_eq!(
            program_generator(&dispatch_semiring_gemm(
                "a",
                "b",
                "c",
                2,
                2,
                2,
                Semiring::Real
            )),
            "vyre-primitives::math::semiring_gemm"
        );
        assert_eq!(
            program_generator(&dispatch_mz_project_step("p", "f", "out", 2)),
            "vyre-primitives::math::mori_zwanzig_project_step"
        );
    }

    #[test]
    fn anonymous_wide_lineage_builder_marks_the_registered_primitive() {
        let program =
            dispatch_semiring_gemm_wide("state", "rules", "next", Some("state"), 2, 2, 2, 2);
        let generator = program_generator(&program);
        assert!(generator.contains("vyre-primitives::math::scallop_join_wide"));
        assert!(generator.contains("semiring_gemm_wide"));
    }

    #[test]
    fn cpu_references_cover_scientific_contracts() {
        assert!(approx_eq(
            reference_bhattacharyya_coefficient(&[0.5, 0.5], &[0.5, 0.5]),
            1.0
        ));
        assert!(approx_eq(
            reference_fisher_rao_distance(&[1.0, 0.0], &[1.0, 0.0]),
            0.0
        ));
        assert_eq!(
            reference_amari_alpha_step(&[1.0, 0.0], &[0.0, 1.0], -1.0, 0.25),
            vec![0.25, 0.75]
        );

        let mut tt_out = Vec::new();
        reference_tt_contract_step_into(&[3.0, 5.0], &[1.0, 0.0, 0.0, 1.0], 2, 2, &mut tt_out);
        assert_eq!(tt_out, vec![3.0, 5.0]);
        let cores = vec![vec![2.0], vec![3.0]];
        assert!(approx_eq(
            reference_tt_full_chain(&cores, &[1, 1, 1], &[1, 1], &[0, 0]),
            6.0
        ));
        let mut acc = Vec::new();
        let mut next = Vec::new();
        assert!(approx_eq(
            reference_tt_full_chain_with_scratch(
                &cores,
                &[1, 1, 1],
                &[1, 1],
                &[0, 0],
                &mut acc,
                &mut next
            ),
            6.0
        ));

        assert_eq!(
            reference_p2m_zeroth_moment(&[1.0, 2.0, 10.0], &[0, 0, 1]),
            vec![3.0, 10.0]
        );

        let (scaled, norm) = reference_qsvt_block_encode(&[3.0, 0.0, 0.0, 4.0], 2);
        assert!(approx_eq(norm, 5.0));
        assert!(approx_eq(scaled[0], 0.6));
        let mut scaled_into = Vec::new();
        assert!(approx_eq(
            reference_qsvt_block_encode_into(&[3.0, 0.0, 0.0, 4.0], 2, &mut scaled_into),
            5.0
        ));
        assert_eq!(scaled_into, scaled);
        assert_eq!(
            reference_qsvt_apply(&[1.0, 0.0, 0.0, 1.0], &[2.0, 3.0], &[0.0, 1.0], 2),
            vec![2.0, 3.0]
        );
        let mut qsvt_out = Vec::new();
        let mut t_prev = Vec::new();
        let mut t_curr = Vec::new();
        let mut t_next = Vec::new();
        reference_qsvt_apply_into(
            &[1.0, 0.0, 0.0, 1.0],
            &[2.0, 3.0],
            &[0.0, 1.0],
            2,
            &mut qsvt_out,
            &mut t_prev,
            &mut t_curr,
            &mut t_next,
        );
        assert_eq!(qsvt_out, vec![2.0, 3.0]);

        assert!(approx_eq(reference_hensel_lift_step(2.5, 0.0, 1.0), 2.5));
        assert_eq!(
            reference_sos_gram_construct(&[0, 1, 1, 2], &[10, 20, 30], 2),
            vec![10, 20, 20, 30]
        );
        let mut gram = Vec::new();
        reference_sos_gram_construct_into(&[0, 1, 1, 2], &[10, 20, 30], 2, &mut gram);
        assert_eq!(gram, vec![10, 20, 20, 30]);
        assert!(reference_is_psd(&[1.0, 0.0, 0.0, 1.0], 2));
    }

    #[test]
    fn cpu_references_cover_dispatch_scale_and_discrete_contracts() {
        let (sum, carry) = reference_bigint_add_carry(&[u32::MAX, u32::MAX], &[1, 0]).unwrap();
        assert_eq!(sum, vec![0, u32::MAX]);
        assert_eq!(carry, vec![1, 0]);
        let mut sum_into = Vec::new();
        let mut carry_into = Vec::new();
        reference_bigint_add_carry_into(
            &[u32::MAX, u32::MAX],
            &[1, 0],
            &mut sum_into,
            &mut carry_into,
        )
        .unwrap();
        assert_eq!(sum_into, sum);
        assert_eq!(carry_into, carry);
        let (resolved, carry_out) = reference_resolve_carry_chain(&sum, &carry).unwrap();
        assert_eq!(resolved, vec![0, 0]);
        assert_eq!(carry_out, 1);
        let mut resolved_into = Vec::new();
        assert_eq!(
            reference_resolve_carry_chain_into(&sum, &carry, &mut resolved_into).unwrap(),
            1
        );
        assert_eq!(resolved_into, resolved);

        assert_eq!(
            reference_tn_pair_contract(&[1.0, 2.0, 3.0, 4.0], &[5.0, 6.0, 7.0, 8.0], 2, 2, 2),
            vec![19.0, 22.0, 43.0, 50.0]
        );
        assert_eq!(reference_greedy_contract_order(&[2, 5, 3]), vec![1, 2, 0]);
        assert_eq!(
            reference_rk4_step(&[5.0], &[1.0], &[1.0], &[1.0], &[1.0], 0.5),
            vec![5.5]
        );

        let mut u = vec![1.0, 1.0];
        let mut v = vec![1.0, 1.0];
        reference_sinkhorn_iter(
            &[1.0, 1.0, 1.0, 1.0],
            &[0.5, 0.5],
            &[0.5, 0.5],
            &mut u,
            &mut v,
            2,
            2,
        );
        assert!(u.iter().all(|value| approx_eq(*value, 0.25)));
        assert!(v.iter().all(|value| approx_eq(*value, 1.0)));
        let mut kv = Vec::new();
        let mut ktu = Vec::new();
        reference_sinkhorn_iter_into(
            &[1.0, 1.0, 1.0, 1.0],
            &[0.5, 0.5],
            &[0.5, 0.5],
            &mut u,
            &mut v,
            2,
            2,
            &mut kv,
            &mut ktu,
        );
        assert_eq!(kv.len(), 2);
        assert_eq!(ktu.len(), 2);

        let denoised =
            reference_score_denoise_step(&[1.0, 2.0], &[0.5, 1.0], &[0.0, 0.0], 0.9, 0.1, 0.0);
        assert!(approx_eq(denoised[0], 0.95));
        assert!(approx_eq(denoised[1], 1.9));
        assert_eq!(reference_conformal_rank(9, 0.5), 5);
        assert_eq!(reference_predict_interval(10, 3), (7, 13));
        assert_eq!(
            reference_semiring_gemm(&[1, 2, 3, 4], &[5, 6, 7, 8], 2, 2, 2, Semiring::Real),
            vec![19, 22, 43, 50]
        );
        let mut c = Vec::new();
        reference_semiring_gemm_into(
            &[1, 2, 3, 4],
            &[5, 6, 7, 8],
            2,
            2,
            2,
            Semiring::Real,
            &mut c,
        );
        assert_eq!(c, vec![19, 22, 43, 50]);
        assert_eq!(
            reference_mz_project_step(&[1.0, 0.0, 0.0, 1.0], &[3.0, 5.0], 2),
            vec![3.0, 5.0]
        );
        let mut mz = Vec::new();
        reference_mz_project_step_into(&[1.0, 0.0, 0.0, 1.0], &[3.0, 5.0], 2, &mut mz);
        assert_eq!(mz, vec![3.0, 5.0]);
    }
}
