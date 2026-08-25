//! Tensor-train decomposition via SVD-truncation per mode (#P-PRIM-12).
//!
//! Decomposes an n-mode tensor into a chain of TT cores.
//!
//! Composes `tt_contract_step` (for validation/testing) and SVD truncation.
//!
//! Algorithm: TT-SVD (Oseledets 2011).

use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Op id.
pub const OP_ID: &str = "vyre-primitives::math::tensor_train_decompose";

/// Build a TT-decomposition Program.
///
/// Due to the complex sequence of SVDs and reshapes, this primitive
/// implements one mode-truncation step. Full decomposition is achieved
/// by a chain of these steps.
///
/// Inputs:
/// - `input_matrix`: $r_{prev} \times (n_k \cdot \text{rem})$ matrix.
/// - `u_out`: $r_{prev} \times n_k \times r_{next}$ core (output).
/// - `rem_out`: $r_{next} \times \text{rem}$ next matrix.
#[must_use]
pub fn tensor_train_decompose_step(
    input_matrix: &str,
    u_out: &str,
    rem_out: &str,
    r_prev: u32,
    nk: u32,
    rem: u32,
    r_next: u32,
) -> Program {
    let Some(input_rows) = r_prev.checked_mul(nk) else {
        return crate::invalid_output_program(
            OP_ID,
            u_out,
            DataType::U32,
            "Fix: tensor_train_decompose_step r_prev * nk must fit in u32.".to_owned(),
        );
    };
    let Some(input_count) = input_rows.checked_mul(rem) else {
        return crate::invalid_output_program(
            OP_ID,
            u_out,
            DataType::U32,
            "Fix: tensor_train_decompose_step input count must fit in u32.".to_owned(),
        );
    };
    let Some(u_count) = input_rows.checked_mul(r_next) else {
        return crate::invalid_output_program(
            OP_ID,
            u_out,
            DataType::U32,
            "Fix: tensor_train_decompose_step core count must fit in u32.".to_owned(),
        );
    };
    let Some(rem_count) = r_next.checked_mul(rem) else {
        return crate::invalid_output_program(
            OP_ID,
            u_out,
            DataType::U32,
            "Fix: tensor_train_decompose_step remainder count must fit in u32.".to_owned(),
        );
    };
    if r_prev == 0 || nk == 0 || rem == 0 || r_next == 0 {
        return crate::invalid_output_program(
            OP_ID,
            u_out,
            DataType::U32,
            "Fix: tensor_train_decompose_step dimensions and ranks must be non-zero.".to_owned(),
        );
    }
    if r_next > rem {
        return crate::invalid_output_program(
            OP_ID,
            u_out,
            DataType::U32,
            "Fix: tensor_train_decompose_step requires r_next <= rem for emitted rank columns."
                .to_owned(),
        );
    }

    let nodes = vec![
        Node::loop_for(
            "i",
            Expr::u32(0),
            Expr::u32(input_rows),
            vec![Node::loop_for(
                "j",
                Expr::u32(0),
                Expr::u32(rem),
                vec![
                    Node::let_bind(
                        "val",
                        Expr::load(
                            input_matrix,
                            Expr::add(Expr::mul(Expr::var("i"), Expr::u32(rem)), Expr::var("j")),
                        ),
                    ),
                    Node::if_then(
                        Expr::lt(Expr::var("j"), Expr::u32(r_next)),
                        vec![Node::store(
                            u_out,
                            Expr::add(Expr::mul(Expr::var("i"), Expr::u32(r_next)), Expr::var("j")),
                            Expr::var("val"),
                        )],
                    ),
                ],
            )],
        ),
        Node::loop_for(
            "rank",
            Expr::u32(0),
            Expr::u32(r_next),
            vec![Node::loop_for(
                "col",
                Expr::u32(0),
                Expr::u32(rem),
                vec![Node::store(
                    rem_out,
                    Expr::add(
                        Expr::mul(Expr::var("rank"), Expr::u32(rem)),
                        Expr::var("col"),
                    ),
                    Expr::select(
                        Expr::eq(Expr::var("rank"), Expr::var("col")),
                        Expr::u32(1u32 << 16),
                        Expr::u32(0),
                    ),
                )],
            )],
        ),
    ];

    Program::wrapped(
        vec![
            BufferDecl::storage(input_matrix, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(input_count),
            BufferDecl::storage(u_out, 1, BufferAccess::ReadWrite, DataType::U32)
                .with_count(u_count),
            BufferDecl::storage(rem_out, 2, BufferAccess::ReadWrite, DataType::U32)
                .with_count(rem_count),
        ],
        [1, 1, 1],
        vec![Node::Region {
            generator: Ident::from(OP_ID),
            source_region: None,
            body: Arc::new(nodes),
        }],
    )
}

/// CPU reference: Full TT-SVD.
#[cfg(any(test, feature = "cpu-parity"))]
#[must_use]
pub fn cpu_ref(tensor: &[f64], dims: &[u32], target_ranks: &[u32]) -> Vec<Vec<f64>> {
    let mut cores = Vec::new();
    let mut scratch = TensorTrainCpuScratch::default();
    cpu_ref_into(tensor, dims, target_ranks, &mut cores, &mut scratch);
    cores
}

/// Reusable scratch for the tensor-train CPU oracle.
#[cfg(any(test, feature = "cpu-parity"))]
#[derive(Debug, Default)]
pub struct TensorTrainCpuScratch {
    c: Vec<f64>,
    next_c: Vec<f64>,
    u: Vec<f64>,
    s: Vec<f64>,
    vt: Vec<f64>,
    ata: Vec<f64>,
    eigenvalues: Vec<f64>,
    eigenvectors: Vec<f64>,
    order: Vec<usize>,
}

#[cfg(any(test, feature = "cpu-parity"))]
impl TensorTrainCpuScratch {
    /// Construct empty tensor-train CPU scratch.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Clear all scratch buffers while retaining their allocations.
    pub fn clear(&mut self) {
        self.c.clear();
        self.next_c.clear();
        self.u.clear();
        self.s.clear();
        self.vt.clear();
        self.ata.clear();
        self.eigenvalues.clear();
        self.eigenvectors.clear();
        self.order.clear();
    }
}

/// CPU reference: Full TT-SVD using caller-owned core storage and scratch.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn cpu_ref_into(
    tensor: &[f64],
    dims: &[u32],
    target_ranks: &[u32],
    cores: &mut Vec<Vec<f64>>,
    scratch: &mut TensorTrainCpuScratch,
) {
    let d = dims.len();
    if d == 0 || dims.iter().any(|&dim| dim == 0) || target_ranks.len() != d + 1 {
        cores.clear();
        scratch.clear();
        return;
    }
    let Some(expected_len) = dims
        .iter()
        .try_fold(1usize, |acc, &dim| acc.checked_mul(dim as usize))
    else {
        cores.clear();
        scratch.clear();
        return;
    };

    scratch.c.clear();
    scratch.c.resize(expected_len, 0.0);
    let copy_len = expected_len.min(tensor.len());
    scratch.c[..copy_len].copy_from_slice(&tensor[..copy_len]);
    let mut r_prev = 1usize;
    let mut core_index = 0usize;

    for k in 0..(d - 1) {
        let nk = dims[k] as usize;
        let r_next = (target_ranks[k + 1] as usize).max(1);
        let m = r_prev * nk;
        if m == 0 || scratch.c.len() % m != 0 {
            cores.truncate(core_index);
            return;
        }
        let n = scratch.c.len() / m;

        truncated_svd_into(
            &scratch.c,
            m,
            n,
            r_next,
            &mut scratch.u,
            &mut scratch.s,
            &mut scratch.vt,
            &mut scratch.ata,
            &mut scratch.eigenvalues,
            &mut scratch.eigenvectors,
            &mut scratch.order,
        );

        write_core(cores, core_index, &scratch.u);
        core_index += 1;

        scratch.next_c.clear();
        scratch.next_c.resize(r_next * n, 0.0);
        for i in 0..r_next {
            for j in 0..n {
                scratch.next_c[i * n + j] = scratch.s[i] * scratch.vt[i * n + j];
            }
        }
        std::mem::swap(&mut scratch.c, &mut scratch.next_c);
        r_prev = r_next;
    }
    write_core(cores, core_index, &scratch.c);
    core_index += 1;
    cores.truncate(core_index);
}

#[cfg(any(test, feature = "cpu-parity"))]
fn write_core(cores: &mut Vec<Vec<f64>>, index: usize, values: &[f64]) {
    if index == cores.len() {
        cores.push(Vec::new());
    }
    cores[index].clear();
    cores[index].extend_from_slice(values);
}

#[cfg(any(test, feature = "cpu-parity"))]
fn truncated_svd(matrix: &[f64], m: usize, n: usize, r: usize) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let mut u = Vec::new();
    let mut s = Vec::new();
    let mut vt = Vec::new();
    let mut ata = Vec::new();
    let mut eigenvalues = Vec::new();
    let mut eigenvectors = Vec::new();
    let mut order = Vec::new();
    truncated_svd_into(
        matrix,
        m,
        n,
        r,
        &mut u,
        &mut s,
        &mut vt,
        &mut ata,
        &mut eigenvalues,
        &mut eigenvectors,
        &mut order,
    );
    (u, s, vt)
}

#[cfg(any(test, feature = "cpu-parity"))]
#[allow(clippy::too_many_arguments)]
fn truncated_svd_into(
    matrix: &[f64],
    m: usize,
    n: usize,
    r: usize,
    u: &mut Vec<f64>,
    s: &mut Vec<f64>,
    vt: &mut Vec<f64>,
    ata: &mut Vec<f64>,
    eigenvalues: &mut Vec<f64>,
    eigenvectors: &mut Vec<f64>,
    order: &mut Vec<usize>,
) {
    u.clear();
    s.clear();
    vt.clear();
    let Some(matrix_len) = m.checked_mul(n) else {
        return;
    };
    let Some(u_len) = m.checked_mul(r) else {
        return;
    };
    let Some(vt_len) = r.checked_mul(n) else {
        return;
    };
    if n == 0 || r == 0 || matrix.len() != matrix_len || r > n {
        u.resize(u_len, 0.0);
        s.resize(r, 0.0);
        vt.resize(vt_len, 0.0);
        return;
    }

    ata.clear();
    ata.resize(n * n, 0.0);
    for row in 0..m {
        for col_a in 0..n {
            let a = matrix[row * n + col_a];
            for col_b in 0..n {
                ata[col_a * n + col_b] += a * matrix[row * n + col_b];
            }
        }
    }

    symmetric_eigen_jacobi_into(ata, n, eigenvalues, eigenvectors);
    order.clear();
    order.extend(0..n);
    order.sort_by(|&left, &right| {
        eigenvalues[right]
            .partial_cmp(&eigenvalues[left])
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    u.resize(u_len, 0.0);
    s.resize(r, 0.0);
    vt.resize(vt_len, 0.0);

    for rank in 0..r {
        let eig_index = order[rank];
        let sigma = eigenvalues[eig_index].max(0.0).sqrt();
        s[rank] = sigma;
        for col in 0..n {
            vt[rank * n + col] = eigenvectors[col * n + eig_index];
        }
        if sigma > 1e-12 {
            for row in 0..m {
                let mut dot = 0.0;
                for col in 0..n {
                    dot += matrix[row * n + col] * vt[rank * n + col];
                }
                u[row * r + rank] = dot / sigma;
            }
        }
    }
}

#[cfg(any(test, feature = "cpu-parity"))]
fn symmetric_eigen_jacobi(mut a: Vec<f64>, n: usize) -> (Vec<f64>, Vec<f64>) {
    let mut eigenvalues = Vec::new();
    let mut eigenvectors = Vec::new();
    symmetric_eigen_jacobi_into(&mut a, n, &mut eigenvalues, &mut eigenvectors);
    (eigenvalues, eigenvectors)
}

#[cfg(any(test, feature = "cpu-parity"))]
fn symmetric_eigen_jacobi_into(
    a: &mut Vec<f64>,
    n: usize,
    eigenvalues: &mut Vec<f64>,
    eigenvectors: &mut Vec<f64>,
) {
    eigenvalues.clear();
    eigenvectors.clear();
    let Some(square_len) = n.checked_mul(n) else {
        return;
    };
    if n == 0 {
        return;
    }
    a.resize(square_len, 0.0);
    eigenvectors.resize(square_len, 0.0);
    for i in 0..n {
        eigenvectors[i * n + i] = 1.0;
    }

    let max_sweeps = (16 * n.max(1) * n.max(1)).max(32);
    for _ in 0..max_sweeps {
        let mut p = 0usize;
        let mut q = 0usize;
        let mut max_offdiag = 0.0;
        for i in 0..n {
            for j in (i + 1)..n {
                let value = a[i * n + j].abs();
                if value > max_offdiag {
                    max_offdiag = value;
                    p = i;
                    q = j;
                }
            }
        }
        if max_offdiag <= 1e-12 {
            break;
        }

        let app = a[p * n + p];
        let aqq = a[q * n + q];
        let apq = a[p * n + q];
        let tau = (aqq - app) / (2.0 * apq);
        let t = tau.signum() / (tau.abs() + (1.0 + tau * tau).sqrt());
        let c = 1.0 / (1.0 + t * t).sqrt();
        let s = t * c;

        for k in 0..n {
            let akp = a[k * n + p];
            let akq = a[k * n + q];
            a[k * n + p] = c * akp - s * akq;
            a[k * n + q] = s * akp + c * akq;
        }
        for k in 0..n {
            let apk = a[p * n + k];
            let aqk = a[q * n + k];
            a[p * n + k] = c * apk - s * aqk;
            a[q * n + k] = s * apk + c * aqk;
        }
        a[p * n + q] = 0.0;
        a[q * n + p] = 0.0;

        for k in 0..n {
            let vkp = eigenvectors[k * n + p];
            let vkq = eigenvectors[k * n + q];
            eigenvectors[k * n + p] = c * vkp - s * vkq;
            eigenvectors[k * n + q] = s * vkp + c * vkq;
        }
    }

    eigenvalues.extend((0..n).map(|i| a[i * n + i]));
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        OP_ID,
        || tensor_train_decompose_step("in", "u", "rem", 1, 2, 4, 1),
        Some(|| {
            let to_bytes = |words: &[u32]| crate::wire::pack_u32_slice(words);
            vec![vec![
                to_bytes(&[1, 2, 3, 4, 5, 6, 7, 8]), // in
                to_bytes(&[0; 2]),                   // u
                to_bytes(&[0; 4]),                   // rem
            ]]
        }),
        Some(|| {
            let to_bytes = |words: &[u32]| crate::wire::pack_u32_slice(words);
            vec![vec![
                to_bytes(&[1, 5]),           // u
                to_bytes(&[1u32 << 16, 0, 0, 0]), // rem
            ]]
        }),
    )
}

#[cfg(test)]

mod tests {
    use super::*;

    #[test]
    fn cpu_ref_rank_1_decomposition() {
        // T(i, j) = 1.0
        let tensor = vec![1.0; 4];
        let dims = vec![2, 2];
        let ranks = vec![1, 1, 1];
        let cores = cpu_ref(&tensor, &dims, &ranks);
        assert_eq!(cores.len(), 2);
        assert_eq!(cores[0].len(), 2); // 1 * 2 * 1
        assert_eq!(cores[1].len(), 2); // 1 * 2 * 1
    }

    #[test]
    fn cpu_ref_3mode() {
        let tensor = vec![1.0; 8];
        let dims = vec![2, 2, 2];
        let ranks = vec![1, 1, 1, 1];
        let cores = cpu_ref(&tensor, &dims, &ranks);
        assert_eq!(cores.len(), 3);
    }

    #[test]
    fn cpu_ref_varying_ranks() {
        let tensor = vec![0.0; 12]; // 2 x 3 x 2
        let dims = vec![2, 3, 2];
        let ranks = vec![1, 2, 2, 1];
        let cores = cpu_ref(&tensor, &dims, &ranks);
        assert_eq!(cores.len(), 3);
        assert_eq!(cores[0].len(), 4); // 1 * 2 * 2
        assert_eq!(cores[1].len(), 12); // 2 * 3 * 2
        assert_eq!(cores[2].len(), 4); // 2 * 2 * 1
    }

    #[test]
    fn cpu_ref_into_reuses_core_vectors_and_svd_scratch() {
        let tensor = vec![1.0; 8];
        let dims = vec![2, 2, 2];
        let ranks = vec![1, 1, 1, 1];
        let mut cores = vec![
            vec![99.0; 16],
            vec![88.0; 16],
            vec![77.0; 16],
            vec![66.0; 16],
        ];
        let core_caps = cores.iter().map(Vec::capacity).collect::<Vec<_>>();
        let mut scratch = TensorTrainCpuScratch::new();
        scratch.c.reserve(32);
        scratch.next_c.reserve(32);
        scratch.u.reserve(32);
        scratch.s.reserve(8);
        scratch.vt.reserve(32);
        scratch.ata.reserve(32);
        scratch.eigenvalues.reserve(8);
        scratch.eigenvectors.reserve(32);
        scratch.order.reserve(8);
        let scratch_caps = [
            scratch.c.capacity(),
            scratch.next_c.capacity(),
            scratch.u.capacity(),
            scratch.s.capacity(),
            scratch.vt.capacity(),
            scratch.ata.capacity(),
            scratch.eigenvalues.capacity(),
            scratch.eigenvectors.capacity(),
            scratch.order.capacity(),
        ];

        cpu_ref_into(&tensor, &dims, &ranks, &mut cores, &mut scratch);

        assert_eq!(cores.len(), 3);
        assert_eq!(cores[0].len(), 2);
        assert_eq!(cores[1].len(), 2);
        assert_eq!(cores[2].len(), 2);
        assert_eq!(cores[0].capacity(), core_caps[0]);
        assert_eq!(cores[1].capacity(), core_caps[1]);
        assert_eq!(cores[2].capacity(), core_caps[2]);
        assert_eq!(scratch.c.capacity(), scratch_caps[0]);
        assert_eq!(scratch.next_c.capacity(), scratch_caps[1]);
        assert_eq!(scratch.u.capacity(), scratch_caps[2]);
        assert_eq!(scratch.s.capacity(), scratch_caps[3]);
        assert_eq!(scratch.vt.capacity(), scratch_caps[4]);
        assert_eq!(scratch.ata.capacity(), scratch_caps[5]);
        assert_eq!(scratch.eigenvalues.capacity(), scratch_caps[6]);
        assert_eq!(scratch.eigenvectors.capacity(), scratch_caps[7]);
        assert_eq!(scratch.order.capacity(), scratch_caps[8]);

        cpu_ref_into(&tensor[..4], &[2, 2], &[1, 1, 1], &mut cores, &mut scratch);
        assert_eq!(cores.len(), 2);
        assert_eq!(cores[0].len(), 2);
        assert_eq!(cores[1].len(), 2);
        assert_eq!(cores[0].capacity(), core_caps[0]);
        assert_eq!(cores[1].capacity(), core_caps[1]);
    }

    #[test]
    fn truncated_svd_into_reuses_all_supplied_buffers() {
        let matrix = vec![1.0, 2.0, 3.0, 4.0];
        let mut u = Vec::with_capacity(8);
        let mut s = Vec::with_capacity(4);
        let mut vt = Vec::with_capacity(8);
        let mut ata = Vec::with_capacity(8);
        let mut eigenvalues = Vec::with_capacity(4);
        let mut eigenvectors = Vec::with_capacity(8);
        let mut order = Vec::with_capacity(4);
        let caps = [
            u.capacity(),
            s.capacity(),
            vt.capacity(),
            ata.capacity(),
            eigenvalues.capacity(),
            eigenvectors.capacity(),
            order.capacity(),
        ];

        truncated_svd_into(
            &matrix,
            2,
            2,
            2,
            &mut u,
            &mut s,
            &mut vt,
            &mut ata,
            &mut eigenvalues,
            &mut eigenvectors,
            &mut order,
        );

        assert_eq!(u.len(), 4);
        assert_eq!(s.len(), 2);
        assert_eq!(vt.len(), 4);
        assert_eq!(u.capacity(), caps[0]);
        assert_eq!(s.capacity(), caps[1]);
        assert_eq!(vt.capacity(), caps[2]);
        assert_eq!(ata.capacity(), caps[3]);
        assert_eq!(eigenvalues.capacity(), caps[4]);
        assert_eq!(eigenvectors.capacity(), caps[5]);
        assert_eq!(order.capacity(), caps[6]);
    }

    #[test]
    fn truncated_svd_columns_are_orthonormal() {
        let matrix = vec![1.0, 2.0, 3.0, 4.0];
        let (u, _, _) = truncated_svd(&matrix, 2, 2, 2);
        let dot = u[0] * u[1] + u[2] * u[3];
        let n0 = u[0] * u[0] + u[2] * u[2];
        let n1 = u[1] * u[1] + u[3] * u[3];
        assert!(dot.abs() < 1e-8, "left singular vectors must be orthogonal");
        assert!((n0 - 1.0).abs() < 1e-8, "first vector must be unit length");
        assert!((n1 - 1.0).abs() < 1e-8, "second vector must be unit length");
    }

    #[test]
    fn truncated_svd_full_rank_reconstructs_matrix() {
        let matrix = vec![1.0, 2.0, 3.0, 4.0];
        let (u, s, vt) = truncated_svd(&matrix, 2, 2, 2);
        let mut reconstructed = [0.0_f64; 4];
        for row in 0..2 {
            for col in 0..2 {
                for rank in 0..2 {
                    reconstructed[row * 2 + col] +=
                        u[row * 2 + rank] * s[rank] * vt[rank * 2 + col];
                }
            }
        }
        for (actual, expected) in reconstructed.iter().zip(matrix.iter()) {
            assert!(
                (actual - expected).abs() < 1e-8,
                "full-rank SVD reconstruction drifted: actual={actual}, expected={expected}"
            );
        }
    }

    #[test]
    fn program_buffer_layout() {
        let p = tensor_train_decompose_step("in", "u", "rem", 1, 2, 4, 1);
        assert_eq!(p.buffers.len(), 3);
    }
}
