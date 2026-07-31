//! Tensor-train (TT) contraction primitive.
//!
//! TT decomposition (Oseledets 2011) compresses an n-mode tensor as a
//! chain of 3-mode "cores":
//!
//! ```text
//!   T(i_1, ..., i_n) = G_1[1, i_1, :] · G_2[:, i_2, :] · ... · G_n[:, i_n, 1]
//! ```
//!
//! Each `G_k` has shape `(r_{k-1}, n_k, r_k)`. Storage drops from
//! exponential `Π n_k` to linear `Σ r_{k-1} · n_k · r_k` for bounded
//! ranks `r_k`. Substrate for: compressing massive context windows
//! (Khrulkov 2018), low-rank attention (Choromanski 2020), parameter
//! compression in NN (Novikov 2015), high-dim PDE solvers.
//!
//! This file ships the **single-step contraction** primitive  -  given
//! current accumulator `acc` (shape `r_{prev}`), one core
//! `G_k[:, i_k, :]` slice (shape `r_{prev} × r_k`), and the chosen
//! index `i_k`, emit the next accumulator (shape `r_k`):
//!
//! ```text
//!   acc_next[b] = Σ_a acc[a] · G[a, i, b]
//! ```
//!
//! Composes via repeated dispatch with `n` calls for an `n`-mode
//! contraction.
//!
//! # Why this primitive is dual-use
//!
//! | Consumer | Use |
//! |---|---|
//! | future `vyre-libs::ml::compress` | low-rank weight compression |
//! | future `vyre-libs::ml::attention` | TT-attention for long context |
//! | future `vyre-libs::sci::pde` | high-dim PDE state representation |
//! | `vyre-foundation::transform` region compression | the Region tree is a tensor network; TT contraction order is the optimal fusion order for chain-shaped Region compositions |

use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Op id.
pub const OP_ID: &str = "vyre-primitives::math::tt_contract_step";

/// Emit one TT contraction step.
///
/// Inputs:
/// - `acc_in`: length-`r_prev` u32 (16.16)  -  current accumulator.
/// - `core_slice`: `r_prev × r_next` u32  -  the 2D slice `G_k[:, i_k, :]`
///   from a TT core (caller selects the index `i_k` and provides the
///   already-sliced 2D matrix).
///
/// Output:
/// - `acc_out`: length-`r_next` u32.
///
/// Per-cell rule: `acc_out[b] = Σ_a acc_in[a] · core_slice[a, b]`.
#[must_use]
pub fn tt_contract_step(
    acc_in: &str,
    core_slice: &str,
    acc_out: &str,
    r_prev: u32,
    r_next: u32,
) -> Program {
    if r_prev == 0 {
        return crate::invalid_output_program(
            OP_ID,
            acc_out,
            DataType::U32,
            format!("Fix: tt_contract_step requires r_prev > 0, got {r_prev}."),
        );
    }
    if r_next == 0 {
        return crate::invalid_output_program(
            OP_ID,
            acc_out,
            DataType::U32,
            format!("Fix: tt_contract_step requires r_next > 0, got {r_next}."),
        );
    }
    let Some(core_count) = r_prev.checked_mul(r_next) else {
        return crate::invalid_output_program(
            OP_ID,
            acc_out,
            DataType::U32,
            format!("Fix: tt_contract_step r_prev*r_next overflows u32: {r_prev}*{r_next}."),
        );
    };

    let t = Expr::InvocationId { axis: 0 };
    let body = vec![Node::if_then(
        Expr::lt(t.clone(), Expr::u32(r_next)),
        vec![
            Node::let_bind("acc", Expr::u32(0)),
            Node::loop_for(
                "a",
                Expr::u32(0),
                Expr::u32(r_prev),
                vec![Node::assign(
                    "acc",
                    Expr::add(
                        Expr::var("acc"),
                        crate::fixed_mul_16_16_expr(
                            Expr::load(acc_in, Expr::var("a")),
                            Expr::load(
                                core_slice,
                                Expr::add(Expr::mul(Expr::var("a"), Expr::u32(r_next)), t.clone()),
                            ),
                        ),
                    ),
                )],
            ),
            Node::store(acc_out, t, Expr::var("acc")),
        ],
    )];

    Program::wrapped(
        vec![
            BufferDecl::storage(acc_in, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(r_prev),
            BufferDecl::storage(core_slice, 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(core_count),
            BufferDecl::storage(acc_out, 2, BufferAccess::ReadWrite, DataType::U32)
                .with_count(r_next),
        ],
        [256, 1, 1],
        vec![Node::Region {
            generator: Ident::from(OP_ID),
            source_region: None,
            body: Arc::new(body),
        }],
    )
}

/// CPU reference: f64.
#[cfg(any(test, feature = "cpu-parity"))]
#[must_use]
pub fn tt_contract_step_cpu(
    acc_in: &[f64],
    core_slice: &[f64],
    r_prev: u32,
    r_next: u32,
) -> Vec<f64> {
    let mut out = Vec::new();
    try_tt_contract_step_cpu_into(acc_in, core_slice, r_prev, r_next, &mut out)
        .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - tt_contract_step_cpu failed: invalid TT contraction shape");
    out
}

/// Fallible CPU reference: f64.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_tt_contract_step_cpu(
    acc_in: &[f64],
    core_slice: &[f64],
    r_prev: u32,
    r_next: u32,
) -> Result<Vec<f64>, String> {
    let mut out = Vec::new();
    try_tt_contract_step_cpu_into(acc_in, core_slice, r_prev, r_next, &mut out)?;
    Ok(out)
}

/// CPU reference: f64 into caller-owned output storage.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn tt_contract_step_cpu_into(
    acc_in: &[f64],
    core_slice: &[f64],
    r_prev: u32,
    r_next: u32,
    out: &mut Vec<f64>,
) {
    try_tt_contract_step_cpu_into(acc_in, core_slice, r_prev, r_next, out)
        .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - tt_contract_step_cpu_into failed: invalid TT contraction shape");
}

/// Fallible CPU reference: f64 into caller-owned output storage.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_tt_contract_step_cpu_into(
    acc_in: &[f64],
    core_slice: &[f64],
    r_prev: u32,
    r_next: u32,
    out: &mut Vec<f64>,
) -> Result<(), String> {
    let (r_prev, r_next, _) = checked_tt_step_shape(r_prev, r_next)?;
    reserve_tt_output(out, r_next, "TT step output")?;
    out.clear();
    out.resize(r_next, 0.0);
    for b in 0..r_next {
        let mut acc = 0.0;
        for a in 0..r_prev {
            let lhs = acc_in.get(a).copied().unwrap_or(0.0);
            let rhs = core_slice.get(a * r_next + b).copied().unwrap_or(0.0);
            acc += lhs * rhs;
        }
        out[b] = acc;
    }
    Ok(())
}

/// CPU reference: full TT chain contraction. Cores given as a Vec of
/// 3D arrays flattened row-major as `(r_prev × n × r_next)`. Indices
/// pick the slice from each core. Boundary ranks must be 1.
#[cfg(any(test, feature = "cpu-parity"))]
#[must_use]
pub fn tt_full_chain_cpu(
    cores: &[Vec<f64>],
    ranks: &[u32],
    mode_dims: &[u32],
    indices: &[u32],
) -> f64 {
    let mut acc = Vec::new();
    let mut next = Vec::new();
    tt_full_chain_cpu_with_scratch(cores, ranks, mode_dims, indices, &mut acc, &mut next)
}

/// CPU reference full chain contraction using caller-owned accumulators.
#[cfg(any(test, feature = "cpu-parity"))]
#[allow(clippy::needless_range_loop)]
pub fn tt_full_chain_cpu_with_scratch(
    cores: &[Vec<f64>],
    ranks: &[u32],
    mode_dims: &[u32],
    indices: &[u32],
    acc: &mut Vec<f64>,
    next: &mut Vec<f64>,
) -> f64 {
    try_tt_full_chain_cpu_with_scratch(cores, ranks, mode_dims, indices, acc, next)
        .expect("Fix: scratch allocation must succeed for declared sizes; shrink test fixture or return Err - tt_full_chain_cpu_with_scratch failed: scratch allocation failed")
}

/// Fallible full-chain contraction using caller-owned accumulators.
#[cfg(any(test, feature = "cpu-parity"))]
#[allow(clippy::needless_range_loop)]
pub fn try_tt_full_chain_cpu_with_scratch(
    cores: &[Vec<f64>],
    ranks: &[u32],
    mode_dims: &[u32],
    indices: &[u32],
    acc: &mut Vec<f64>,
    next: &mut Vec<f64>,
) -> Result<f64, String> {
    let n = cores.len();
    if n == 0 || ranks.first().copied().unwrap_or(0) != 1 || ranks.get(n).copied().unwrap_or(0) != 1
    {
        return Ok(0.0);
    }

    reserve_tt_output(acc, 1, "TT chain accumulator")?;
    acc.clear();
    acc.push(1.0);
    for k in 0..n {
        let r_p = ranks.get(k).copied().unwrap_or(0);
        let r_n = ranks.get(k + 1).copied().unwrap_or(0);
        let nk = mode_dims.get(k).copied().unwrap_or(0);
        let i = indices.get(k).copied().unwrap_or(0);
        if r_p == 0 || r_n == 0 || nk == 0 || i >= nk {
            return Ok(0.0);
        }

        let r_n_usize = usize::try_from(r_n)
            .map_err(|_| format!("TT chain rank r_next={r_n} does not fit usize."))?;
        reserve_tt_output(next, r_n_usize, "TT chain next accumulator")?;
        next.clear();
        next.resize(r_n_usize, 0.0);
        for b in 0..r_n_usize {
            let mut value = 0.0;
            for a in 0..r_p as usize {
                let lhs = acc.get(a).copied().unwrap_or(0.0);
                let idx = ((a as u32 * nk + i) * r_n + b as u32) as usize;
                let rhs = cores[k].get(idx).copied().unwrap_or(0.0);
                value += lhs * rhs;
            }
            next[b] = value;
        }
        std::mem::swap(acc, next);
    }
    Ok(acc.first().copied().unwrap_or(0.0))
}

#[cfg(any(test, feature = "cpu-parity"))]
fn checked_tt_step_shape(r_prev: u32, r_next: u32) -> Result<(usize, usize, usize), String> {
    if r_prev == 0 || r_next == 0 {
        return Err(format!(
            "tt_contract_step CPU oracle requires non-zero ranks, got r_prev={r_prev}, r_next={r_next}."
        ));
    }
    let r_prev = usize::try_from(r_prev)
        .map_err(|_| format!("TT step r_prev={r_prev} does not fit usize."))?;
    let r_next = usize::try_from(r_next)
        .map_err(|_| format!("TT step r_next={r_next} does not fit usize."))?;
    let cells = r_prev
        .checked_mul(r_next)
        .ok_or_else(|| "TT step core-slice shape overflows usize.".to_string())?;
    Ok((r_prev, r_next, cells))
}

#[cfg(any(test, feature = "cpu-parity"))]
fn reserve_tt_output(out: &mut Vec<f64>, len: usize, name: &str) -> Result<(), String> {
    if len > out.capacity() {
        crate::graph::scratch::reserve_graph_items(out, len - out.len(), "TT CPU oracle", name)?;
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
    fn cpu_step_identity_core_passes_acc_through() {
        // r_prev = r_next = 2, core = identity.
        let acc = vec![3.0, 5.0];
        let core = vec![1.0, 0.0, 0.0, 1.0];
        let out = tt_contract_step_cpu(&acc, &core, 2, 2);
        assert!(approx_eq(out[0], 3.0));
        assert!(approx_eq(out[1], 5.0));
    }

    #[test]
    fn cpu_step_zero_core_zeros_out() {
        let acc = vec![10.0, 20.0];
        let core = vec![0.0, 0.0, 0.0, 0.0];
        let out = tt_contract_step_cpu(&acc, &core, 2, 2);
        assert!(approx_eq(out[0], 0.0));
        assert!(approx_eq(out[1], 0.0));
    }

    #[test]
    fn cpu_step_rank_change_works() {
        // r_prev = 2, r_next = 3.
        let acc = vec![1.0, 2.0];
        // core 2x3 = [[a, b, c], [d, e, f]] flattened row-major.
        let core = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        // out[0] = 1·1 + 2·4 = 9
        // out[1] = 1·2 + 2·5 = 12
        // out[2] = 1·3 + 2·6 = 15
        let out = tt_contract_step_cpu(&acc, &core, 2, 3);
        assert!(approx_eq(out[0], 9.0));
        assert!(approx_eq(out[1], 12.0));
        assert!(approx_eq(out[2], 15.0));
    }

    #[test]
    fn cpu_step_into_reuses_output_storage() {
        let acc = vec![1.0, 2.0];
        let core = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let expected = tt_contract_step_cpu(&acc, &core, 2, 3);
        let mut out = Vec::with_capacity(expected.len());

        tt_contract_step_cpu_into(&acc, &core, 2, 3, &mut out);
        let ptr = out.as_ptr();
        tt_contract_step_cpu_into(&acc, &core, 2, 3, &mut out);

        assert_eq!(out, expected);
        assert_eq!(out.as_ptr(), ptr);
    }

    #[test]
    fn cpu_step_into_truncates_stale_tail_without_reallocating() {
        let acc = vec![1.0, 2.0];
        let core = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let mut out = Vec::with_capacity(8);
        out.extend([99.0; 8]);
        let ptr = out.as_ptr();

        try_tt_contract_step_cpu_into(&acc, &core, 2, 3, &mut out).unwrap();

        assert_eq!(out.len(), 3);
        assert!(approx_eq(out[0], 9.0));
        assert!(approx_eq(out[1], 12.0));
        assert!(approx_eq(out[2], 15.0));
        assert_eq!(out.as_ptr(), ptr);
    }

    #[test]
    fn generated_cpu_step_matches_independent_reference() {
        for case in 0..72 {
            let r_prev = 1 + (case % 5);
            let r_next = 1 + (case % 6);
            let acc: Vec<f64> = (0..r_prev).map(|idx| idx as f64 * 0.25 + 1.0).collect();
            let core: Vec<f64> = (0..r_prev * r_next)
                .map(|idx| idx as f64 * 0.125 - case as f64 * 0.001)
                .collect();
            let mut out = Vec::with_capacity(r_next + 3);

            try_tt_contract_step_cpu_into(&acc, &core, r_prev as u32, r_next as u32, &mut out)
                .unwrap();

            for b in 0..r_next {
                let expected: f64 = (0..r_prev).map(|a| acc[a] * core[a * r_next + b]).sum();
                assert!(
                    approx_eq(out[b], expected),
                    "case {case} b {b}: expected {expected}, got {}",
                    out[b]
                );
            }
        }
    }

    #[test]
    fn cpu_full_chain_two_cores_recovers_product() {
        // 2-mode tensor, T(i, j) = α(i) · β(j).
        // TT cores: G_1 shape (1, 2, 1) = [α(0); α(1)], G_2 same shape.
        let core1 = vec![3.0, 5.0]; // α = (3, 5)
        let core2 = vec![7.0, 11.0]; // β = (7, 11)
        let cores = vec![core1, core2];
        let ranks = vec![1, 1, 1];
        let dims = vec![2, 2];
        let result = tt_full_chain_cpu(&cores, &ranks, &dims, &[0, 0]);
        // T(0, 0) = 3 · 7 = 21
        assert!(approx_eq(result, 21.0));
        let result_11 = tt_full_chain_cpu(&cores, &ranks, &dims, &[1, 1]);
        // T(1, 1) = 5 · 11 = 55
        assert!(approx_eq(result_11, 55.0));
    }

    #[test]
    fn cpu_full_chain_with_scratch_reuses_accumulators() {
        let cores = vec![vec![3.0, 5.0], vec![7.0, 11.0]];
        let ranks = vec![1, 1, 1];
        let dims = vec![2, 2];
        let mut acc = Vec::with_capacity(1);
        let mut next = Vec::with_capacity(1);

        let first =
            tt_full_chain_cpu_with_scratch(&cores, &ranks, &dims, &[1, 1], &mut acc, &mut next);
        let acc_ptr = acc.as_ptr();
        let next_ptr = next.as_ptr();
        let second =
            tt_full_chain_cpu_with_scratch(&cores, &ranks, &dims, &[1, 1], &mut acc, &mut next);

        assert!(approx_eq(first, 55.0));
        assert!(approx_eq(second, 55.0));
        assert_eq!(acc.as_ptr(), acc_ptr);
        assert_eq!(next.as_ptr(), next_ptr);
    }

    #[test]
    fn cpu_full_chain_with_scratch_truncates_stale_accumulators() {
        let cores = vec![vec![3.0, 5.0], vec![7.0, 11.0]];
        let ranks = vec![1, 1, 1];
        let dims = vec![2, 2];
        let mut acc = Vec::with_capacity(8);
        let mut next = Vec::with_capacity(8);
        acc.extend([99.0; 8]);
        next.extend([99.0; 8]);
        let acc_ptr = acc.as_ptr();
        let next_ptr = next.as_ptr();

        let value =
            try_tt_full_chain_cpu_with_scratch(&cores, &ranks, &dims, &[1, 1], &mut acc, &mut next)
                .unwrap();

        assert!(approx_eq(value, 55.0));
        assert_eq!(acc.len(), 1);
        assert!(next.len() <= 1);
        assert_eq!(acc.as_ptr(), acc_ptr);
        assert_eq!(next.as_ptr(), next_ptr);
    }

    #[test]
    fn ir_program_buffer_layout() {
        let p = tt_contract_step("acc", "core", "out", 4, 8);
        assert_eq!(p.workgroup_size, [256, 1, 1]);
        let names: Vec<&str> = p.buffers.iter().map(|b| b.name()).collect();
        assert_eq!(names, vec!["acc", "core", "out"]);
        assert_eq!(p.buffers[0].count(), 4);
        assert_eq!(p.buffers[1].count(), 32);
        assert_eq!(p.buffers[2].count(), 8);
    }

    #[test]
    fn zero_r_prev_traps() {
        let p = tt_contract_step("a", "c", "o", 0, 1);
        assert!(p.stats().trap());
    }

    #[test]
    fn zero_r_next_traps() {
        let p = tt_contract_step("a", "c", "o", 1, 0);
        assert!(p.stats().trap());
    }

    #[test]
    fn cpu_step_rejects_zero_rank() {
        let err = try_tt_contract_step_cpu(&[1.0], &[1.0], 0, 1).unwrap_err();
        assert!(err.contains("non-zero ranks"), "{err}");
    }
}
