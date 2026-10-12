//! Full iterative Sinkhorn balance.
//!
//! Alternates row-normalize and column-normalize until converged.
//! Composes `sinkhorn_scale` + `semiring_gemm` + `persistent_fixpoint`.

use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Node, Program};

use crate::math::semiring_gemm::{semiring_gemm, Semiring};
use crate::math::sinkhorn::sinkhorn_scale;

/// Stable registry id for the iterative Sinkhorn primitive.
pub const OP_ID: &str = "vyre-primitives::math::sinkhorn_iterate";

/// Sinkhorn full iteration.
///
/// Runs Sinkhorn matrix-scaling iterations to convergence.
///
/// # Buffers
/// - `k`: `m x n` kernel matrix.
/// - `k_t`: `n x m` transposed kernel matrix.
/// - `a`: `m` target marginals.
/// - `b`: `n` target marginals.
/// - `u_curr`: `m` elements, ping-pong state for u.
/// - `u_next`: `m` elements, ping-pong state for u.
/// - `v`: `n` elements, current state for v.
/// - `kv`: `m` elements scratch.
/// - `ktu`: `n` elements scratch.
/// - `changed`: 1 element convergence flag.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn sinkhorn_iterate(
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
    if m == 0 {
        return crate::invalid_output_program(
            OP_ID,
            u_curr,
            DataType::U32,
            "Fix: sinkhorn_iterate requires m > 0, got 0.".to_string(),
        );
    }
    if n == 0 {
        return crate::invalid_output_program(
            OP_ID,
            u_curr,
            DataType::U32,
            "Fix: sinkhorn_iterate requires n > 0, got 0.".to_string(),
        );
    }
    let Some(matrix_cells) = m.checked_mul(n) else {
        return crate::invalid_output_program(
            OP_ID,
            u_curr,
            DataType::U32,
            format!("Fix: sinkhorn_iterate m*n overflows u32: {m}*{n}."),
        );
    };

    let mut transfer_body = Vec::new();

    let extract_body = |p: Program| -> Vec<Node> {
        let mut body = Vec::new();
        for n in p.entry() {
            if let Node::Region {
                body: region_body, ..
            } = n
            {
                body.extend(region_body.iter().cloned());
            }
        }
        body
    };

    // 1. Kv = K * v (m x n * n x 1 -> m x 1)
    let p1 = semiring_gemm(k, v, kv, m, 1, n, Semiring::Real);
    transfer_body.extend(extract_body(p1));
    transfer_body.push(Node::Barrier {
        ordering: vyre_foundation::MemoryOrdering::SeqCst,
    });

    // 2. u_next = a ./ Kv
    let p2 = sinkhorn_scale(a, kv, u_next, m);
    transfer_body.extend(extract_body(p2));
    transfer_body.push(Node::Barrier {
        ordering: vyre_foundation::MemoryOrdering::SeqCst,
    });

    // 3. Ktu = K_T * u_next (n x m * m x 1 -> n x 1)
    let p3 = semiring_gemm(k_t, u_next, ktu, n, 1, m, Semiring::Real);
    transfer_body.extend(extract_body(p3));
    transfer_body.push(Node::Barrier {
        ordering: vyre_foundation::MemoryOrdering::SeqCst,
    });

    // 4. v = b ./ Ktu
    let p4 = sinkhorn_scale(b, ktu, v, n);
    transfer_body.extend(extract_body(p4));
    transfer_body.push(Node::Barrier {
        ordering: vyre_foundation::MemoryOrdering::SeqCst,
    });

    let inner = crate::fixpoint::persistent_fixpoint::persistent_fixpoint(
        transfer_body,
        u_curr,
        u_next,
        changed,
        m,
        max_iterations,
    );

    let entry: Vec<Node> = vec![Node::Region {
        generator: Ident::from(OP_ID),
        source_region: None,
        body: Arc::new(inner.entry().to_vec()),
    }];

    Program::wrapped(
        vec![
            BufferDecl::storage(u_curr, 0, BufferAccess::ReadWrite, DataType::U32).with_count(m),
            BufferDecl::storage(u_next, 1, BufferAccess::ReadWrite, DataType::U32).with_count(m),
            BufferDecl::storage(changed, 2, BufferAccess::ReadWrite, DataType::U32).with_count(1),
            BufferDecl::storage(k, 3, BufferAccess::ReadOnly, DataType::U32)
                .with_count(matrix_cells),
            BufferDecl::storage(k_t, 4, BufferAccess::ReadOnly, DataType::U32)
                .with_count(matrix_cells),
            BufferDecl::storage(a, 5, BufferAccess::ReadOnly, DataType::U32).with_count(m),
            BufferDecl::storage(b, 6, BufferAccess::ReadOnly, DataType::U32).with_count(n),
            BufferDecl::storage(v, 7, BufferAccess::ReadWrite, DataType::U32).with_count(n),
            BufferDecl::storage(kv, 8, BufferAccess::ReadWrite, DataType::U32).with_count(m),
            BufferDecl::storage(ktu, 9, BufferAccess::ReadWrite, DataType::U32).with_count(n),
        ],
        [256, 1, 1],
        entry,
    )
}

/// CPU reference for iterative Sinkhorn.
#[cfg(any(test, feature = "cpu-parity"))]
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn cpu_ref(
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
    let mut u = Vec::new();
    let mut v_mut = Vec::new();
    let mut u_old = Vec::new();
    let iters = try_cpu_ref_into(
        k,
        k_t,
        a,
        b,
        u_curr,
        v,
        m,
        n,
        max_iterations,
        &mut u,
        &mut v_mut,
        &mut u_old,
    )
    .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - sinkhorn_iterate cpu_ref failed: invalid fixed-point Sinkhorn buffers");
    (u, v_mut, iters)
}

/// Fallible CPU reference for iterative Sinkhorn.
#[cfg(any(test, feature = "cpu-parity"))]
#[allow(clippy::too_many_arguments)]
pub fn try_cpu_ref(
    k: &[u32],
    k_t: &[u32],
    a: &[u32],
    b: &[u32],
    u_curr: &[u32],
    v: &[u32],
    m: u32,
    n: u32,
    max_iterations: u32,
) -> Result<(Vec<u32>, Vec<u32>, u32), String> {
    let mut u = Vec::new();
    let mut v_mut = Vec::new();
    let mut u_old = Vec::new();
    let iters = try_cpu_ref_into(
        k,
        k_t,
        a,
        b,
        u_curr,
        v,
        m,
        n,
        max_iterations,
        &mut u,
        &mut v_mut,
        &mut u_old,
    )?;
    Ok((u, v_mut, iters))
}

/// CPU reference for iterative Sinkhorn using caller-owned buffers.
///
/// `u_out` and `v_out` receive the final states. `u_old` is retained
/// as convergence scratch to avoid cloning `u` every iteration.
#[cfg(any(test, feature = "cpu-parity"))]
#[allow(clippy::too_many_arguments)]
pub fn cpu_ref_into(
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
    try_cpu_ref_into(
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
    .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - sinkhorn_iterate cpu_ref_into failed: invalid fixed-point Sinkhorn buffers")
}

/// Fallible CPU reference for iterative Sinkhorn using caller-owned buffers.
#[cfg(any(test, feature = "cpu-parity"))]
#[allow(clippy::too_many_arguments)]
pub fn try_cpu_ref_into(
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
) -> Result<u32, String> {
    let (m_usize, n_usize, matrix_cells) = checked_fixed_sinkhorn_shape(m, n)?;
    require_fixed_len("k", k.len(), matrix_cells)?;
    require_fixed_len("k_t", k_t.len(), matrix_cells)?;
    require_fixed_len("a", a.len(), m_usize)?;
    require_fixed_len("b", b.len(), n_usize)?;
    require_fixed_len("u_curr", u_curr.len(), m_usize)?;
    require_fixed_len("v", v.len(), n_usize)?;
    reserve_u32_vec(u_out, m_usize, "u output")?;
    reserve_u32_vec(v_out, n_usize, "v output")?;
    reserve_u32_vec(u_old, m_usize, "u convergence scratch")?;

    u_out.clear();
    u_out.extend_from_slice(&u_curr[..m_usize]);
    v_out.clear();
    v_out.extend_from_slice(&v[..n_usize]);

    let mut iters = 0;
    for iter in 0..max_iterations {
        u_old.clear();
        u_old.extend_from_slice(u_out);

        // 1 & 2. Kv & u
        for i in 0..m_usize {
            let mut sum = 0u32;
            for j in 0..n_usize {
                sum = sum.wrapping_add(k[i * n_usize + j].wrapping_mul(v_out[j]));
            }
            let divisor = if sum == 0 { 1 } else { sum };
            u_out[i] = a[i] / divisor;
        }

        // 3 & 4. Ktu & v
        for j in 0..n_usize {
            let mut sum = 0u32;
            for i in 0..m_usize {
                sum = sum.wrapping_add(k_t[j * m_usize + i].wrapping_mul(u_out[i]));
            }
            let divisor = if sum == 0 { 1 } else { sum };
            v_out[j] = b[j] / divisor;
        }

        if u_out == u_old {
            return Ok(iter);
        }
        iters = iter + 1;
    }
    Ok(iters)
}

#[cfg(any(test, feature = "cpu-parity"))]
fn checked_fixed_sinkhorn_shape(m: u32, n: u32) -> Result<(usize, usize, usize), String> {
    if m == 0 || n == 0 {
        return Err(format!(
            "sinkhorn_iterate CPU oracle requires non-zero dimensions, got m={m}, n={n}."
        ));
    }
    let m_usize =
        usize::try_from(m).map_err(|_| format!("sinkhorn_iterate m={m} does not fit usize."))?;
    let n_usize =
        usize::try_from(n).map_err(|_| format!("sinkhorn_iterate n={n} does not fit usize."))?;
    let matrix_cells = m_usize.checked_mul(n_usize).ok_or_else(|| {
        format!("sinkhorn_iterate CPU oracle matrix cells overflow: m={m}, n={n}.")
    })?;
    Ok((m_usize, n_usize, matrix_cells))
}

#[cfg(any(test, feature = "cpu-parity"))]
fn require_fixed_len(name: &str, got: usize, need: usize) -> Result<(), String> {
    if got < need {
        Err(format!(
            "sinkhorn_iterate CPU oracle buffer `{name}` is too short: got {got}, need {need}."
        ))
    } else {
        Ok(())
    }
}

#[cfg(any(test, feature = "cpu-parity"))]
fn reserve_u32_vec(out: &mut Vec<u32>, len: usize, name: &str) -> Result<(), String> {
    if len > out.capacity() {
        crate::graph::scratch::reserve_graph_items(
            out,
            len - out.len(),
            "Sinkhorn iterate CPU oracle",
            name,
        )?;
    }
    Ok(())
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        OP_ID,
        || sinkhorn_iterate("k", "kt", "a", "b", "uc", "un", "v", "kv", "ktu", "c", 2, 2, 5),
        Some(|| {
            let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
            vec![vec![
                to_bytes(&[65536, 65536]), // u_curr
                to_bytes(&[0, 0]), // u_next
                to_bytes(&[0]), // changed
                to_bytes(&[65536, 65536, 65536, 65536]), // k
                to_bytes(&[65536, 65536, 65536, 65536]), // k_t
                to_bytes(&[32768, 32768]), // a
                to_bytes(&[32768, 32768]), // b
                to_bytes(&[65536, 65536]), // v
                to_bytes(&[0, 0]), // kv
                to_bytes(&[0, 0]), // ktu
            ]]
        }),
        Some(|| {
            let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
            vec![vec![
                to_bytes(&[32768, 32768]), // u_curr
                to_bytes(&[32768, 32768]), // u_next
                to_bytes(&[0]),            // changed
                to_bytes(&[32768, 32768]), // v
                to_bytes(&[0, 0]),         // kv
                to_bytes(&[0, 0]),         // ktu
            ]]
        }),
    )
}

#[cfg(test)]
mod tests;

// ===== P-PRIM-11: Full iterative-balance Sinkhorn (f64) ===========
//
// The fixed-point u32 cpu_ref above is the GPU-targeted reference;
// the math operates on quantized fractions. This block ships an
// f64 reference that performs the canonical Sinkhorn-Knopp iterative
// matrix-balancing algorithm with tolerance-based convergence  -
// the operation many user dialects ask for when they say "balanced
// transport plan."

/// Tolerance-based Sinkhorn-Knopp iterative balancing in f64.
///
/// Inputs:
/// - `k`: kernel matrix `m × n`, row-major. Strictly positive entries.
/// - `a`: target row marginal, length m. Strictly positive entries.
/// - `b`: target column marginal, length n. Strictly positive entries.
/// - `tolerance`: stop when `||u_new - u_old||_∞ < tolerance`.
/// - `max_iterations`: hard cap.
///
/// Returns `(u, v, iterations)` such that `diag(u) · k · diag(v)`
/// has row sums approximately `a` and column sums approximately `b`,
/// up to the supplied tolerance.
///
/// Pre/post conditions:
/// * Caller guarantees `sum(a) == sum(b)` (mass-conservation;
///   Sinkhorn-Knopp converges only on balanced marginals).
/// * Returns the iteration that stopped  -  < `max_iterations` means
///   tolerance reached, == `max_iterations` means cap hit.
///
/// # Panics
///
/// Panics on length mismatch.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn sinkhorn_iterate_f64(
    k: &[f64],
    a: &[f64],
    b: &[f64],
    tolerance: f64,
    max_iterations: u32,
) -> (Vec<f64>, Vec<f64>, u32) {
    let mut u = Vec::new();
    let mut v = Vec::new();
    let mut u_old = Vec::new();
    let iters = sinkhorn_iterate_f64_into(
        k,
        a,
        b,
        tolerance,
        max_iterations,
        &mut u,
        &mut v,
        &mut u_old,
    );
    (u, v, iters)
}

/// Fallible tolerance-based Sinkhorn-Knopp iterative balancing in f64.
#[cfg(any(test, feature = "cpu-parity"))]

pub fn try_sinkhorn_iterate_f64(
    k: &[f64],
    a: &[f64],
    b: &[f64],
    tolerance: f64,
    max_iterations: u32,
) -> Result<(Vec<f64>, Vec<f64>, u32), String> {
    let mut u = Vec::new();
    let mut v = Vec::new();
    let mut u_old = Vec::new();
    let iters = try_sinkhorn_iterate_f64_into(
        k,
        a,
        b,
        tolerance,
        max_iterations,
        &mut u,
        &mut v,
        &mut u_old,
    )?;
    Ok((u, v, iters))
}

/// Tolerance-based Sinkhorn-Knopp iterative balancing in f64 using
/// caller-owned buffers.
#[allow(clippy::too_many_arguments)]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn sinkhorn_iterate_f64_into(
    k: &[f64],
    a: &[f64],
    b: &[f64],
    tolerance: f64,
    max_iterations: u32,
    u: &mut Vec<f64>,
    v: &mut Vec<f64>,
    u_old: &mut Vec<f64>,
) -> u32 {
    match try_sinkhorn_iterate_f64_into(k, a, b, tolerance, max_iterations, u, v, u_old) {
        Ok(iters) => iters,
        // Clearing the buffers and returning 0 iterations on failure makes a
        // GPU-vs-CPU parity assertion pass on empty==empty, silently masking a
        // divergence (Law 10 / Law 6). Fail loud; callers use the try_ variant.
        Err(error) => panic!("vyre-primitives Sinkhorn iterate CPU reference failed: {error}"),
    }
}

/// Fallible tolerance-based Sinkhorn-Knopp iterative balancing in f64 using
/// caller-owned buffers.
#[allow(clippy::too_many_arguments)]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_sinkhorn_iterate_f64_into(
    k: &[f64],
    a: &[f64],
    b: &[f64],
    tolerance: f64,
    max_iterations: u32,
    u: &mut Vec<f64>,
    v: &mut Vec<f64>,
    u_old: &mut Vec<f64>,
) -> Result<u32, String> {
    let m = a.len();
    let n = b.len();
    if k.len() != m * n || tolerance <= 0.0 || !tolerance.is_finite() {
        return Err(format!(
            "sinkhorn_iterate_f64 requires k.len()==a.len()*b.len() and finite positive tolerance, got k={}, m={m}, n={n}, tolerance={tolerance}.",
            k.len()
        ));
    }
    reserve_f64_vec(u, m, "u output")?;
    reserve_f64_vec(v, n, "v output")?;
    reserve_f64_vec(u_old, m, "u convergence scratch")?;

    u.clear();
    v.clear();
    u_old.clear();
    u.resize(m, 1.0_f64);
    v.resize(n, 1.0_f64);

    for iter in 0..max_iterations {
        u_old.clear();
        u_old.extend_from_slice(u);

        // u <- a / (k · v)
        for i in 0..m {
            let mut sum = 0.0_f64;
            for j in 0..n {
                sum += k[i * n + j] * v[j];
            }
            // Guard against division by zero  -  sinkhorn requires k > 0,
            // but defensive callers benefit from a non-NaN result.
            u[i] = if sum == 0.0 { 0.0 } else { a[i] / sum };
        }

        // v <- b / (kᵀ · u)
        for j in 0..n {
            let mut sum = 0.0_f64;
            for i in 0..m {
                sum += k[i * n + j] * u[i];
            }
            v[j] = if sum == 0.0 { 0.0 } else { b[j] / sum };
        }

        // Convergence check on u (Sinkhorn-Knopp stops when one
        // marginal is stable; the other follows by construction).
        let max_delta = u
            .iter()
            .zip(u_old.iter())
            .map(|(new, old)| (new - old).abs())
            .fold(0.0_f64, f64::max);
        if max_delta < tolerance {
            return Ok(iter + 1);
        }
    }
    Ok(max_iterations)
}

#[cfg(any(test, feature = "cpu-parity"))]
fn reserve_f64_vec(out: &mut Vec<f64>, len: usize, name: &str) -> Result<(), String> {
    if len > out.capacity() {
        crate::graph::scratch::reserve_graph_items(
            out,
            len - out.len(),
            "Sinkhorn iterate f64 CPU oracle",
            name,
        )?;
    }
    Ok(())
}

/// Compute the row-sum residual `||row_sum(diag(u) · k · diag(v)) - a||_∞`.
/// Useful for testing convergence of [`sinkhorn_iterate_f64`].
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn sinkhorn_row_residual(k: &[f64], u: &[f64], v: &[f64], a: &[f64]) -> f64 {
    let m = a.len();
    let n = v.len();
    assert_eq!(u.len(), m);
    assert_eq!(k.len(), m * n);
    let mut max_resid = 0.0_f64;
    for i in 0..m {
        let mut row = 0.0_f64;
        for j in 0..n {
            row += u[i] * k[i * n + j] * v[j];
        }
        let delta = (row - a[i]).abs();
        if delta > max_resid {
            max_resid = delta;
        }
    }
    max_resid
}

/// Compute the column-sum residual `||col_sum(diag(u) · k · diag(v)) - b||_∞`.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn sinkhorn_col_residual(k: &[f64], u: &[f64], v: &[f64], b: &[f64]) -> f64 {
    let m = u.len();
    let n = b.len();
    assert_eq!(v.len(), n);
    assert_eq!(k.len(), m * n);
    let mut max_resid = 0.0_f64;
    for j in 0..n {
        let mut col = 0.0_f64;
        for i in 0..m {
            col += u[i] * k[i * n + j] * v[j];
        }
        let delta = (col - b[j]).abs();
        if delta > max_resid {
            max_resid = delta;
        }
    }
    max_resid
}

#[cfg(test)]
mod f64_tests;
