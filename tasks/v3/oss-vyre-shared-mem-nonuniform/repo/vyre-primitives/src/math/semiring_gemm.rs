//! Generic-semiring matrix multiply  -  the spine of the LEGO substrate.
//!
//! `semiring_gemm` is one Program builder parameterized by a closed semiring
//! choice. It emits IR specialized to that semiring at build time  -  the
//! emitted body contains zero runtime branches over the semiring tag, so
//! Tensor Cores and subgroup-mat-mul intrinsics see the same shape they
//! would for a standard `(×, +)` gemm.
//!
//! # Why this primitive is dual-use
//!
//! Same Program is consumed by user-dialect callers (Tier 3 `vyre-libs`) AND
//! by vyre's own substrate (`vyre-foundation::transform`):
//!
//! | Semiring | User-dialect consumer | vyre-self consumer |
//! |---|---|---|
//! | `Real` (×, +) | every numeric workload | dispatch-cost matrix products |
//! | `MinPlus` (+, min) | shortest-path graphs in `vyre-libs::security` | dependency-graph longest-path for #19 polyhedral fusion |
//! | `MaxPlus` (+, max) | scheduling, rate analysis | critical-path of dispatch graph for #22 megakernel scheduler |
//! | `BoolOr` (∧, ∨) | reachability in `vyre-libs::dataflow` | Region-tree reachability for #26 dataflow fixpoint |
//! | `MaxTimes` (×, max) | Viterbi/HMM forward in ML consumers | rule-conflict probability resolution |
//! | `Provenance` | `vyre-libs::scallop_join` (#39) | rule provenance tracking in external analyzer |
//! | `Gf2` (∧, ⊕) | crypto / linear-code dialects | bitset adjacency under XOR closure |
//!
//! Six self-consumers, six user-dialect consumers  -  clears the recursion-thesis
//! bar from day 1.
//!
//! # Algorithm
//!
//! ```text
//! C[i,j] = ⊕_k (A[i,k] ⊗ B[k,j])
//! ```
//!
//! where `⊕` is the additive (accumulate) op, `⊗` is the multiplicative
//! (combine) op, and the accumulator initializes to the additive identity.
//! The flat invocation `t = i*N + j` covers `M*N` output cells; the inner
//! `k` loop runs serially per lane.
//!
//! # Variant Boundaries
//!
//! Block-tiled, sparse-adjacency, and user-supplied combine/accumulate
//! forms are distinct registered ops. This module's contract is the
//! dense enum-specialized semiring GEMM over the seven well-known
//! semirings.

use vyre_foundation::ir::{DataType, Expr, Program};
pub use vyre_spec::Semiring;

use crate::fixed_u32_matmul::u32_matmul_program;

/// Canonical op id.
pub const OP_ID: &str = "vyre-primitives::math::semiring_gemm";

fn semiring_combine_expr(semiring: Semiring, a: Expr, b: Expr) -> Expr {
    match semiring {
        Semiring::Real | Semiring::MaxTimes => Expr::mul(a, b),
        Semiring::MinPlus => {
            // saturating add: if either operand is MAX, result is MAX,
            // otherwise a + b. Keeps MAX absorbing under min-plus.
            let max_const = Expr::u32(u32::MAX);
            let either_inf = Expr::or(
                Expr::eq(a.clone(), max_const.clone()),
                Expr::eq(b.clone(), max_const.clone()),
            );
            Expr::select(either_inf, max_const, Expr::add(a, b))
        }
        Semiring::MaxPlus => Expr::add(a, b),
        Semiring::BoolOr | Semiring::Gf2 => Expr::bitand(a, b),
        Semiring::BoolAnd => Expr::bitor(a, b),
        Semiring::Lineage => {
            // Zero-absorbing OR: if either operand is 0 (no edge),
            // the join is 0. Otherwise OR the fact bitsets along
            // the path step. Distinguishes "no edge" from
            // "edge with empty fact-set"  -  single-u32 lineage.
            let either_zero = Expr::or(
                Expr::eq(a.clone(), Expr::u32(0)),
                Expr::eq(b.clone(), Expr::u32(0)),
            );
            Expr::select(either_zero, Expr::u32(0), Expr::bitor(a, b))
        }
    }
}

fn semiring_accumulate_expr(semiring: Semiring, acc: Expr, val: Expr) -> Expr {
    match semiring {
        Semiring::Real | Semiring::MaxPlus => Expr::add(acc, val),
        Semiring::MinPlus => Expr::min(acc, val),
        Semiring::MaxTimes => Expr::max(acc, val),
        Semiring::BoolOr | Semiring::Lineage => Expr::bitor(acc, val),
        Semiring::BoolAnd => Expr::bitand(acc, val),
        Semiring::Gf2 => Expr::bitxor(acc, val),
    }
}

/// Emit a generic-semiring `M × K · K × N → M × N` matmul Program.
///
/// `a` is laid out row-major with stride `k` (`A[i, kk] = a[i*k + kk]`).
/// `b` is laid out row-major with stride `n` (`B[kk, j] = b[kk*n + j]`).
/// `c` is laid out row-major with stride `n` (`C[i, j] = c[i*n + j]`).
/// All buffers are `u32`. For non-integer semirings, callers encode their
/// own fixed-point scaling on top.
///
/// # Panics
///
/// Panics if any of `m`, `n`, `k` is zero.
#[must_use]
pub fn semiring_gemm(
    a: &str,
    b: &str,
    c: &str,
    m: u32,
    n: u32,
    k: u32,
    semiring: Semiring,
) -> Program {
    if m == 0 {
        return crate::invalid_output_program(
            OP_ID,
            c,
            DataType::U32,
            format!("Fix: semiring_gemm requires m > 0, got {m}."),
        );
    }
    if n == 0 {
        return crate::invalid_output_program(
            OP_ID,
            c,
            DataType::U32,
            format!("Fix: semiring_gemm requires n > 0, got {n}."),
        );
    }
    if k == 0 {
        return crate::invalid_output_program(
            OP_ID,
            c,
            DataType::U32,
            format!("Fix: semiring_gemm requires k > 0, got {k}."),
        );
    }

    let Some(cell_count) = m.checked_mul(n) else {
        return crate::invalid_output_program(
            OP_ID,
            c,
            DataType::U32,
            format!("Fix: semiring_gemm output cells overflow u32: m={m}, n={n}."),
        );
    };
    let Some(a_count) = m.checked_mul(k) else {
        return crate::invalid_output_program(
            OP_ID,
            c,
            DataType::U32,
            format!("Fix: semiring_gemm A buffer cells overflow u32: m={m}, k={k}."),
        );
    };
    let Some(b_count) = k.checked_mul(n) else {
        return crate::invalid_output_program(
            OP_ID,
            c,
            DataType::U32,
            format!("Fix: semiring_gemm B buffer cells overflow u32: k={k}, n={n}."),
        );
    };
    u32_matmul_program(
        OP_ID,
        a,
        b,
        c,
        m,
        k,
        n,
        a_count,
        b_count,
        cell_count,
        semiring.identity(),
        |lhs, rhs| semiring_combine_expr(semiring, lhs, rhs),
        |acc, value| semiring_accumulate_expr(semiring, acc, value),
    )
}

/// CPU reference  -  exact byte-for-byte target the GPU dispatch must hit.
///
/// `a` is `m × k`, `b` is `k × n`, output is `m × n`, all row-major.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn semiring_gemm_cpu(
    a: &[u32],
    b: &[u32],
    m: u32,
    n: u32,
    k: u32,
    semiring: Semiring,
) -> Vec<u32> {
    let mut c = Vec::new();
    try_semiring_gemm_cpu_into(a, b, m, n, k, semiring, &mut c)
        .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - semiring_gemm_cpu failed: invalid GEMM shape");
    c
}

/// Fallible CPU reference.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_semiring_gemm_cpu(
    a: &[u32],
    b: &[u32],
    m: u32,
    n: u32,
    k: u32,
    semiring: Semiring,
) -> Result<Vec<u32>, String> {
    let mut c = Vec::new();
    try_semiring_gemm_cpu_into(a, b, m, n, k, semiring, &mut c)?;
    Ok(c)
}

/// CPU reference using a caller-owned output buffer.
///
/// This is the hot-path oracle for higher-level fixpoint primitives:
/// callers can keep one scratch allocation across thousands of GEMM
/// rounds instead of allocating a fresh result per iteration.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn semiring_gemm_cpu_into(
    a: &[u32],
    b: &[u32],
    m: u32,
    n: u32,
    k: u32,
    semiring: Semiring,
    c: &mut Vec<u32>,
) {
    try_semiring_gemm_cpu_into(a, b, m, n, k, semiring, c)
        .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - semiring_gemm_cpu_into failed: invalid GEMM shape");
}

/// Fallible CPU reference using a caller-owned output buffer.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_semiring_gemm_cpu_into(
    a: &[u32],
    b: &[u32],
    m: u32,
    n: u32,
    k: u32,
    semiring: Semiring,
    c: &mut Vec<u32>,
) -> Result<(), String> {
    let (m_usize, n_usize, k_usize, cell_count) = checked_cpu_gemm_shape(m, n, k)?;
    if cell_count > c.capacity() {
        crate::graph::scratch::reserve_graph_items(
            c,
            cell_count - c.len(),
            "semiring GEMM CPU oracle",
            "output matrix",
        )?;
    }
    c.clear();
    c.resize(cell_count, semiring.identity());
    for i in 0..m_usize {
        for j in 0..n_usize {
            let mut acc = semiring.identity();
            for kk in 0..k_usize {
                let a_v = a
                    .get(i * k_usize + kk)
                    .copied()
                    .unwrap_or(semiring.identity());
                let b_v = b
                    .get(kk * n_usize + j)
                    .copied()
                    .unwrap_or(semiring.identity());
                let combined = semiring_combine_cpu(semiring, a_v, b_v);
                acc = semiring_accumulate_cpu(semiring, acc, combined);
            }
            c[i * n_usize + j] = acc;
        }
    }
    Ok(())
}

#[cfg(any(test, feature = "cpu-parity"))]
fn checked_cpu_gemm_shape(m: u32, n: u32, k: u32) -> Result<(usize, usize, usize, usize), String> {
    if m == 0 || n == 0 || k == 0 {
        return Err(format!(
            "semiring_gemm CPU oracle requires non-zero dimensions, got m={m}, n={n}, k={k}."
        ));
    }
    let m_usize =
        usize::try_from(m).map_err(|_| format!("semiring_gemm m={m} does not fit usize."))?;
    let n_usize =
        usize::try_from(n).map_err(|_| format!("semiring_gemm n={n} does not fit usize."))?;
    let k_usize =
        usize::try_from(k).map_err(|_| format!("semiring_gemm k={k} does not fit usize."))?;
    let cell_count = m_usize
        .checked_mul(n_usize)
        .ok_or_else(|| format!("semiring_gemm CPU oracle output cells overflow: m={m}, n={n}."))?;
    m_usize.checked_mul(k_usize).ok_or_else(|| {
        format!("semiring_gemm CPU oracle A buffer cells overflow: m={m}, k={k}.")
    })?;
    k_usize.checked_mul(n_usize).ok_or_else(|| {
        format!("semiring_gemm CPU oracle B buffer cells overflow: k={k}, n={n}.")
    })?;
    Ok((m_usize, n_usize, k_usize, cell_count))
}

#[inline]
#[cfg(any(test, feature = "cpu-parity"))]
fn semiring_combine_cpu(s: Semiring, a: u32, b: u32) -> u32 {
    match s {
        Semiring::Real | Semiring::MaxTimes => a.wrapping_mul(b),
        Semiring::MinPlus => {
            if a == u32::MAX || b == u32::MAX {
                u32::MAX
            } else {
                a.saturating_add(b)
            }
        }
        Semiring::MaxPlus => a.saturating_add(b),
        Semiring::BoolOr | Semiring::Gf2 => a & b,
        Semiring::BoolAnd => a | b,
        Semiring::Lineage => {
            if a == 0 || b == 0 {
                0
            } else {
                a | b
            }
        }
    }
}

#[inline]
#[cfg(any(test, feature = "cpu-parity"))]
fn semiring_accumulate_cpu(s: Semiring, acc: u32, val: u32) -> u32 {
    match s {
        Semiring::Real | Semiring::MaxPlus => acc.wrapping_add(val),
        Semiring::MinPlus => acc.min(val),
        Semiring::MaxTimes => acc.max(val),
        Semiring::BoolOr | Semiring::Lineage => acc | val,
        Semiring::BoolAnd => acc & val,
        Semiring::Gf2 => acc ^ val,
    }
}

#[cfg(feature = "inventory-registry")]
fn fixture_u32(words: &[u32]) -> Vec<u8> {
    crate::wire::pack_u32_slice(words)
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        OP_ID,
        || semiring_gemm("a", "b", "c", 2, 2, 2, Semiring::Real),
        Some(|| vec![vec![
            fixture_u32(&[1, 2, 3, 4]),
            fixture_u32(&[5, 6, 7, 8]),
            fixture_u32(&[0, 0, 0, 0]),
        ]]),
        Some(|| vec![vec![fixture_u32(&[19, 22, 43, 50])]]),
    )
}

#[cfg(test)]
mod tests;
