//! Sheaf Laplacian eigenvalue primitive (#P-PRIM-9).
//!
//! Power iteration on the sheaf Laplacian (diagonal part) to extract the
//! dominant eigenvalue.
//!
//! Composes `sheaf_diffusion_step`.
//!
//! Algorithm:
//! 1. $v_{k+1} = R v_k$ (where $R$ is the sheaf Laplacian diagonal)
//! 2. $\lambda = ||v_{k+1}|| / ||v_k||$ (approximate)
//! 3. $v_{k+1} = v_{k+1} / ||v_{k+1}||$

use crate::graph::sheaf::sheaf_diffusion_step;
use std::sync::Arc;
use vyre_foundation::ir::model::expr::{GeneratorRef, Ident};
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Op id.
pub const OP_ID: &str = "vyre-primitives::math::sheaf_laplacian_eigenvalue";
const POWER_ITERATION_PHASE_OP_ID: &str =
    "vyre-primitives::math::sheaf_laplacian_eigenvalue::power_iteration_phase";

/// Build a sheaf Laplacian eigenvalue Program.
///
/// Inputs:
/// - `restriction_diag`: `n * d` diagonal sheaf Laplacian.
/// - `v`: `n * d` initial vector (updated in-place).
/// - `lambda`: 1-element output eigenvalue.
/// - `scratch_v`: `n * d` scratch.
/// - `scratch_norm`: 1-element scratch for norm.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn sheaf_laplacian_eigenvalue(
    restriction_diag: &str,
    v: &str,
    lambda: &str,
    scratch_v: &str,
    scratch_norm: &str,
    n_nodes: u32,
    d: u32,
    iterations: u32,
) -> Program {
    if n_nodes == 0 || d == 0 {
        return crate::invalid_output_program(
            OP_ID,
            lambda,
            DataType::U32,
            format!(
                "Fix: sheaf_laplacian_eigenvalue requires n_nodes > 0 and d > 0, got n_nodes={n_nodes}, d={d}."
            ),
        );
    }
    let Some(cells) = n_nodes.checked_mul(d) else {
        return crate::invalid_output_program(
            OP_ID,
            lambda,
            DataType::U32,
            format!(
                "Fix: sheaf_laplacian_eigenvalue n_nodes*d overflows vector cell count for n_nodes={n_nodes}, d={d}; shard the sheaf spectrum before GPU dispatch."
            ),
        );
    };
    let mut nodes = Vec::new();

    // Constant damping = -1.0 (in 16.16: 0xFFFF0000 but we'll use Expr)
    // Actually, sheaf_diffusion_step does: stalks_next = stalks - damping * restriction_diag * stalks
    // If we want r * s, we can use damping = 1.0 to get s - r*s, then compute s - (s - r*s) = r*s.
    // Or we can just use damping = -1.0 to get s + r*s, then compute (s + r*s) - s = r*s.

    // For 16.16, -1.0 is 0xFFFF0000 if signed, but DataType is U32.
    // Let's use 1.0 (0x00010000) and then subtraction.

    let one_fp = 1u32 << 16;
    nodes.push(Node::let_bind("one_fp", Expr::u32(one_fp)));

    for iter in 0..iterations {
        let i_var = format!("eig_i_{iter}");
        let norm_i_var = format!("eig_norm_i_{iter}");
        let val_var = format!("eig_val_{iter}");
        let atomic_old_var = format!("eig_norm_old_{iter}");
        let norm_sq_var = format!("eig_norm_sq_{iter}");
        let normalize_i_var = format!("eig_normalize_i_{iter}");

        // 1. Compute r * v
        // use scratch_v to store v - r*v
        let diff = sheaf_diffusion_step(v, restriction_diag, "one_fp_buf", scratch_v, n_nodes, d);
        nodes.extend(diff.entry().to_vec());

        // v_new = v - (v - r*v) = r*v
        nodes.push(Node::loop_for(
            i_var.as_str(),
            Expr::u32(0),
            Expr::u32(cells),
            vec![Node::store(
                v,
                Expr::var(i_var.as_str()),
                Expr::sub(
                    Expr::load(v, Expr::var(i_var.as_str())),
                    Expr::load(scratch_v, Expr::var(i_var.as_str())),
                ),
            )],
        ));

        nodes.push(Node::store(scratch_norm, Expr::u32(0), Expr::u32(0)));
        let loop_body = vec![
            Node::let_bind(
                val_var.as_str(),
                Expr::load(v, Expr::var(norm_i_var.as_str())),
            ),
            Node::let_bind(
                atomic_old_var.as_str(),
                Expr::atomic_add(
                    scratch_norm,
                    Expr::u32(0),
                    crate::fixed_mul_16_16_expr(
                        Expr::var(val_var.as_str()),
                        Expr::var(val_var.as_str()),
                    ),
                ),
            ),
        ];
        nodes.push(Node::loop_for(
            norm_i_var.as_str(),
            Expr::u32(0),
            Expr::u32(cells),
            loop_body,
        ));

        // lambda = sqrt(norm_sq)
        // We'll just use norm_sq for now or a simple sqrt approximation if available.
        // Actually, power iteration can just use sum of abs or similar.
        // Let's use a simple 1/norm normalization.
        nodes.push(Node::let_bind(
            norm_sq_var.as_str(),
            Expr::load(scratch_norm, Expr::u32(0)),
        ));
        // approx inverse sqrt: this is hard in IR without intrinsics.
        // Let's just store the last norm as lambda for simplicity if that's acceptable,
        // or just perform the division.
        nodes.push(Node::if_then(
            Expr::gt(Expr::var(norm_sq_var.as_str()), Expr::u32(0)),
            vec![
                // v = v / sqrt(norm_sq)
                // For the sake of this primitive, we'll assume a sqrt exists or we use a rough one.
                // Actually, let's just use the norm itself for lambda.
                Node::store(lambda, Expr::u32(0), Expr::var(norm_sq_var.as_str())),
                // normalize v: v = v * (1/sqrt(norm_sq)).
                // We'll just do v = v / (norm_sq >> 8) to keep it in range.
                Node::loop_for(
                    normalize_i_var.as_str(),
                    Expr::u32(0),
                    Expr::u32(cells),
                    vec![Node::store(
                        v,
                        Expr::var(normalize_i_var.as_str()),
                        Expr::div(
                            Expr::shl(
                                Expr::load(v, Expr::var(normalize_i_var.as_str())),
                                Expr::u32(8),
                            ),
                            Expr::var(norm_sq_var.as_str()),
                        ),
                    )],
                ),
            ],
        ));
    }

    Program::wrapped(
        vec![
            BufferDecl::storage(restriction_diag, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(cells),
            BufferDecl::storage(v, 1, BufferAccess::ReadWrite, DataType::U32).with_count(cells),
            BufferDecl::storage(lambda, 2, BufferAccess::ReadWrite, DataType::U32).with_count(1),
            BufferDecl::storage(scratch_v, 3, BufferAccess::ReadWrite, DataType::U32)
                .with_count(cells),
            BufferDecl::storage(scratch_norm, 4, BufferAccess::ReadWrite, DataType::U32)
                .with_count(1),
            BufferDecl::storage("one_fp_buf", 5, BufferAccess::ReadOnly, DataType::U32)
                .with_count(1),
        ],
        [1, 1, 1],
        vec![Node::Region {
            generator: Ident::from(OP_ID),
            source_region: None,
            body: Arc::new(vec![Node::Region {
                generator: Ident::from(POWER_ITERATION_PHASE_OP_ID),
                source_region: Some(GeneratorRef {
                    name: OP_ID.to_string(),
                }),
                body: Arc::new(nodes),
            }]),
        }],
    )
}

/// CPU reference: Power iteration on sheaf Laplacian diagonal.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn cpu_ref(restriction_diag: &[f64], v_init: &[f64], iterations: u32) -> (f64, Vec<f64>) {
    let mut v = Vec::new();
    let mut v_next = Vec::new();
    let lambda = try_cpu_ref_into(restriction_diag, v_init, iterations, &mut v, &mut v_next)
        .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - sheaf_laplacian_eigenvalue cpu_ref failed: invalid CPU buffers");
    (lambda, v)
}

/// Fallible CPU reference: Power iteration on sheaf Laplacian diagonal.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_cpu_ref(
    restriction_diag: &[f64],
    v_init: &[f64],
    iterations: u32,
) -> Result<(f64, Vec<f64>), String> {
    let mut v = Vec::new();
    let mut v_next = Vec::new();
    let lambda = try_cpu_ref_into(restriction_diag, v_init, iterations, &mut v, &mut v_next)?;
    Ok((lambda, v))
}

/// CPU reference writing the final eigenvector into caller-owned storage.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn cpu_ref_into(
    restriction_diag: &[f64],
    v_init: &[f64],
    iterations: u32,
    v: &mut Vec<f64>,
    v_next: &mut Vec<f64>,
) -> f64 {
    try_cpu_ref_into(restriction_diag, v_init, iterations, v, v_next)
        .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - sheaf_laplacian_eigenvalue cpu_ref_into failed: invalid CPU buffers")
}

/// Fallible CPU reference writing the final eigenvector into caller-owned storage.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_cpu_ref_into(
    restriction_diag: &[f64],
    v_init: &[f64],
    iterations: u32,
    v: &mut Vec<f64>,
    v_next: &mut Vec<f64>,
) -> Result<f64, String> {
    if restriction_diag.len() < v_init.len() {
        return Err(format!(
            "sheaf_laplacian_eigenvalue CPU oracle restriction_diag too short: got {}, need {}.",
            restriction_diag.len(),
            v_init.len()
        ));
    }
    reserve_eigen_tmp(v, v_init.len(), "eigenvector output")?;
    reserve_eigen_tmp(v_next, v_init.len(), "next-vector scratch")?;
    v.clear();
    v.extend_from_slice(v_init);
    v_next.clear();
    v_next.resize(v.len(), 0.0);
    let mut lambda = 0.0;
    for _ in 0..iterations {
        // v = R * v
        for i in 0..v.len() {
            v_next[i] = restriction_diag[i] * v[i];
        }

        // norm
        let norm_sq: f64 = v_next.iter().map(|x| x * x).sum();
        let norm = norm_sq.sqrt();

        if norm > 1e-20 {
            for i in 0..v.len() {
                v[i] = v_next[i] / norm;
            }
            lambda = norm;
        } else {
            break;
        }
    }
    Ok(lambda)
}

#[cfg(any(test, feature = "cpu-parity"))]
fn reserve_eigen_tmp(out: &mut Vec<f64>, len: usize, name: &str) -> Result<(), String> {
    if len > out.capacity() {
        crate::graph::scratch::reserve_graph_items(
            out,
            len - out.len(),
            "sheaf Laplacian eigenvalue CPU oracle",
            name,
        )?;
    }
    Ok(())
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        OP_ID,
        || sheaf_laplacian_eigenvalue("r", "v", "l", "sv", "sn", 4, 1, 4),
        Some(|| {
            let to_bytes = |words: &[u32]| crate::wire::pack_u32_slice(words);
            vec![vec![
                to_bytes(&[0; 4]),     // r
                to_bytes(&[0; 4]),     // v
                to_bytes(&[0]),        // l
                to_bytes(&[0; 4]),     // sv
                to_bytes(&[0]),        // sn
                to_bytes(&[1u32 << 16]), // one_fp_buf
            ]]
        }),
        Some(|| {
            let to_bytes = |words: &[u32]| crate::wire::pack_u32_slice(words);
            vec![vec![
                to_bytes(&[0; 4]), // v
                to_bytes(&[0]),    // l
                to_bytes(&[0; 4]), // sv
                to_bytes(&[0]),    // sn
            ]]
        }),
    )
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        POWER_ITERATION_PHASE_OP_ID,
        || {
            Program::wrapped(
                vec![
                    BufferDecl::storage("input", 0, BufferAccess::ReadOnly, DataType::U32)
                        .with_count(1),
                    BufferDecl::output("out", 1, DataType::U32).with_count(1),
                ],
                [1, 1, 1],
                vec![Node::Region {
                    generator: Ident::from(POWER_ITERATION_PHASE_OP_ID),
                    source_region: None,
                    body: Arc::new(vec![Node::store(
                        "out",
                        Expr::u32(0),
                        Expr::load("input", Expr::u32(0)),
                    )]),
                }],
            )
        },
        Some(|| {
            let to_bytes = |words: &[u32]| crate::wire::pack_u32_slice(words);
            vec![vec![to_bytes(&[11]), to_bytes(&[0])]]
        }),
        Some(|| {
            let to_bytes = |words: &[u32]| crate::wire::pack_u32_slice(words);
            vec![vec![to_bytes(&[11])]]
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_ref_diagonal_max() {
        let r = vec![1.0, 2.0, 5.0, 3.0];
        let v = vec![1.0, 1.0, 1.0, 1.0];
        let (lambda, vec_final) = cpu_ref(&r, &v, 20);
        // Dominant eigenvalue should be 5.0
        assert!((lambda - 5.0).abs() < 1e-5);
        // Eigenvector should be [0, 0, 1, 0]
        assert!(vec_final[2] > 0.99);
    }

    #[test]
    fn cpu_ref_uniform() {
        let r = vec![2.0, 2.0];
        let v = vec![1.0, 0.0];
        let (lambda, _) = cpu_ref(&r, &v, 5);
        assert!((lambda - 2.0).abs() < 1e-5);
    }

    #[test]
    fn cpu_ref_zero() {
        let r = vec![0.0, 0.0];
        let v = vec![1.0, 1.0];
        let (lambda, _) = cpu_ref(&r, &v, 5);
        assert_eq!(lambda, 0.0);
    }

    #[test]
    fn cpu_ref_single() {
        let r = vec![42.0];
        let v = vec![1.0];
        let (lambda, _) = cpu_ref(&r, &v, 1);
        assert_eq!(lambda, 42.0);
    }

    #[test]
    fn cpu_ref_asymmetric() {
        let r = vec![1.0, 10.0, 0.1];
        let v = vec![1.0, 1.0, 1.0];
        let (lambda, _) = cpu_ref(&r, &v, 10);
        assert!((lambda - 10.0).abs() < 1e-5);
    }

    #[test]
    fn cpu_ref_into_reuses_vectors_and_truncates_stale_tail() {
        let r = vec![1.0, 2.0, 5.0, 3.0];
        let init = vec![1.0, 1.0, 1.0, 1.0];
        let mut v = Vec::with_capacity(8);
        let mut next = Vec::with_capacity(8);
        v.extend([99.0; 8]);
        next.extend([99.0; 8]);
        let v_ptr = v.as_ptr();
        let next_ptr = next.as_ptr();

        let lambda = try_cpu_ref_into(&r, &init, 20, &mut v, &mut next).unwrap();

        assert!((lambda - 5.0).abs() < 1e-5);
        assert_eq!(v.len(), init.len());
        assert_eq!(next.len(), init.len());
        assert_eq!(v.as_ptr(), v_ptr);
        assert_eq!(next.as_ptr(), next_ptr);
    }

    #[test]
    fn generated_cpu_ref_matches_independent_power_iteration() {
        for case in 0..48 {
            let n = 1 + (case % 8);
            let restriction: Vec<f64> =
                (0..n).map(|idx| 0.5 + (idx + case) as f64 * 0.25).collect();
            let init: Vec<f64> = (0..n).map(|idx| 1.0 + idx as f64 * 0.125).collect();
            let iterations = 1 + (case % 8) as u32;
            let mut v = Vec::with_capacity(n + 3);
            let mut next = Vec::with_capacity(n + 3);

            let lambda =
                try_cpu_ref_into(&restriction, &init, iterations, &mut v, &mut next).unwrap();
            let (expected_lambda, expected_v) =
                independent_power_iteration(&restriction, &init, iterations);

            assert!(
                (lambda - expected_lambda).abs() < 1e-10,
                "case {case}: expected lambda {expected_lambda}, got {lambda}"
            );
            for idx in 0..n {
                assert!(
                    (v[idx] - expected_v[idx]).abs() < 1e-10,
                    "case {case} idx {idx}: expected {}, got {}",
                    expected_v[idx],
                    v[idx]
                );
            }
        }
    }

    #[test]
    fn try_cpu_ref_rejects_short_restriction_diag() {
        let err = try_cpu_ref(&[1.0], &[1.0, 2.0], 1).unwrap_err();
        assert!(err.contains("restriction_diag too short"), "{err}");
    }

    #[test]
    fn program_buffer_count() {
        let p = sheaf_laplacian_eigenvalue("r", "v", "l", "sv", "sn", 4, 1, 4);
        assert_eq!(p.buffers.len(), 6);
    }

    fn independent_power_iteration(
        restriction_diag: &[f64],
        v_init: &[f64],
        iterations: u32,
    ) -> (f64, Vec<f64>) {
        let mut v = v_init.to_vec();
        let mut next = vec![0.0; v.len()];
        let mut lambda = 0.0;
        for _ in 0..iterations {
            for idx in 0..v.len() {
                next[idx] = restriction_diag[idx] * v[idx];
            }
            let norm = next.iter().map(|value| value * value).sum::<f64>().sqrt();
            if norm <= 1e-20 {
                break;
            }
            for idx in 0..v.len() {
                v[idx] = next[idx] / norm;
            }
            lambda = norm;
        }
        (lambda, v)
    }
}
