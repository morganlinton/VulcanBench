//! Block-diagonal inverse for K-FAC natural gradient.
//!
//! Input is a block-diagonal matrix (vector of blocks each n×n),
//! output is the block-diagonal inverse.
//!
//! Fulfils the "otherwise dense solve" primitive fallback for natural
//! gradient optimization in `vyre-nn`.

use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Canonical op id.
pub const OP_ID: &str = "vyre-primitives::math::kfac_block_inverse";

/// Block-diagonal inverse builder.
///
/// Inverts `num_blocks` matrices of size `n x n` in parallel.
/// Assumes matrices are non-singular and well-conditioned (e.g. PD Fisher blocks).
#[must_use]
pub fn kfac_block_inverse(
    blocks_out: &str,
    blocks_in: &str,
    scratch: &str,
    num_blocks: u32,
    n: u32,
) -> Program {
    if num_blocks == 0 {
        return crate::invalid_output_program(
            OP_ID,
            blocks_out,
            DataType::F32,
            "Fix: kfac_block_inverse requires num_blocks > 0, got 0.".to_string(),
        );
    }
    if n == 0 {
        return crate::invalid_output_program(
            OP_ID,
            blocks_out,
            DataType::F32,
            "Fix: kfac_block_inverse requires n > 0, got 0.".to_string(),
        );
    }
    let Some(block_cells) = n.checked_mul(n) else {
        return crate::invalid_output_program(
            OP_ID,
            blocks_out,
            DataType::F32,
            format!("Fix: kfac_block_inverse n*n overflows u32 for n={n}."),
        );
    };
    let Some(total_cells) = num_blocks.checked_mul(block_cells) else {
        return crate::invalid_output_program(
            OP_ID,
            blocks_out,
            DataType::F32,
            format!("Fix: kfac_block_inverse num_blocks*n*n overflows u32: {num_blocks}*{n}*{n}."),
        );
    };

    let t = Expr::InvocationId { axis: 0 };
    let n_expr = Expr::u32(n);

    // Each thread t handles one block
    let mut iter_body = Vec::new();

    let offset = |i: Expr, j: Expr| {
        Expr::add(
            Expr::mul(t.clone(), Expr::mul(n_expr.clone(), n_expr.clone())),
            Expr::add(Expr::mul(i, n_expr.clone()), j),
        )
    };

    // 1. Copy block to scratch and initialize blocks_out to identity
    iter_body.push(Node::loop_for(
        "i",
        Expr::u32(0),
        n_expr.clone(),
        vec![Node::loop_for(
            "j",
            Expr::u32(0),
            n_expr.clone(),
            vec![
                Node::let_bind("idx", offset(Expr::var("i"), Expr::var("j"))),
                Node::store(
                    scratch,
                    Expr::var("idx"),
                    Expr::load(blocks_in, Expr::var("idx")),
                ),
                Node::store(
                    blocks_out,
                    Expr::var("idx"),
                    Expr::select(
                        Expr::eq(Expr::var("i"), Expr::var("j")),
                        Expr::f32(1.0),
                        Expr::f32(0.0),
                    ),
                ),
            ],
        )],
    ));

    // 2. Gauss-Jordan Elimination (no pivoting, assumes PSD)
    iter_body.push(Node::loop_for(
        "i",
        Expr::u32(0),
        n_expr.clone(),
        vec![
            // pivot = scratch[i, i]
            Node::let_bind(
                "pivot",
                Expr::load(scratch, offset(Expr::var("i"), Expr::var("i"))),
            ),
            // scale row i
            Node::loop_for(
                "j",
                Expr::u32(0),
                n_expr.clone(),
                vec![
                    Node::let_bind("idx_ij", offset(Expr::var("i"), Expr::var("j"))),
                    Node::store(
                        scratch,
                        Expr::var("idx_ij"),
                        Expr::div(Expr::load(scratch, Expr::var("idx_ij")), Expr::var("pivot")),
                    ),
                    Node::store(
                        blocks_out,
                        Expr::var("idx_ij"),
                        Expr::div(
                            Expr::load(blocks_out, Expr::var("idx_ij")),
                            Expr::var("pivot"),
                        ),
                    ),
                ],
            ),
            // eliminate other rows
            Node::loop_for(
                "k",
                Expr::u32(0),
                n_expr.clone(),
                vec![Node::if_then(
                    Expr::ne(Expr::var("k"), Expr::var("i")),
                    vec![
                        Node::let_bind(
                            "factor",
                            Expr::load(scratch, offset(Expr::var("k"), Expr::var("i"))),
                        ),
                        Node::loop_for(
                            "jj",
                            Expr::u32(0),
                            n_expr.clone(),
                            vec![
                                Node::let_bind("idx_kj", offset(Expr::var("k"), Expr::var("jj"))),
                                Node::let_bind("idx_ij", offset(Expr::var("i"), Expr::var("jj"))),
                                Node::store(
                                    scratch,
                                    Expr::var("idx_kj"),
                                    Expr::sub(
                                        Expr::load(scratch, Expr::var("idx_kj")),
                                        Expr::mul(
                                            Expr::var("factor"),
                                            Expr::load(scratch, Expr::var("idx_ij")),
                                        ),
                                    ),
                                ),
                                Node::store(
                                    blocks_out,
                                    Expr::var("idx_kj"),
                                    Expr::sub(
                                        Expr::load(blocks_out, Expr::var("idx_kj")),
                                        Expr::mul(
                                            Expr::var("factor"),
                                            Expr::load(blocks_out, Expr::var("idx_ij")),
                                        ),
                                    ),
                                ),
                            ],
                        ),
                    ],
                )],
            ),
        ],
    ));

    let entry = vec![Node::if_then(
        Expr::lt(t.clone(), Expr::u32(num_blocks)),
        iter_body,
    )];

    Program::wrapped(
        vec![
            BufferDecl::storage(blocks_out, 0, BufferAccess::ReadWrite, DataType::F32)
                .with_count(total_cells),
            BufferDecl::storage(blocks_in, 1, BufferAccess::ReadOnly, DataType::F32)
                .with_count(total_cells),
            BufferDecl::storage(scratch, 2, BufferAccess::ReadWrite, DataType::F32)
                .with_count(total_cells),
        ],
        [256, 1, 1],
        vec![Node::Region {
            generator: Ident::from(OP_ID),
            source_region: None,
            body: Arc::new(entry),
        }],
    )
}

/// CPU reference.
#[cfg(any(test, feature = "cpu-parity"))]
#[must_use]
pub fn cpu_ref(blocks_in: &[f32], num_blocks: u32, n: u32) -> Vec<f32> {
    let n = n as usize;
    let mut out = Vec::new();
    let mut mat = Vec::new();
    let mut inv = Vec::new();
    cpu_ref_into(
        blocks_in, num_blocks, n as u32, &mut out, &mut mat, &mut inv,
    );
    out
}

/// CPU reference using caller-owned output and per-block scratch buffers.
///
/// Uses flat `n*n` scratch matrices rather than allocating nested vectors for
/// every block. `out` is overwritten with one inverse block per input block.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn cpu_ref_into(
    blocks_in: &[f32],
    num_blocks: u32,
    n: u32,
    out: &mut Vec<f32>,
    mat: &mut Vec<f32>,
    inv: &mut Vec<f32>,
) {
    let n = n as usize;
    out.clear();
    out.resize(blocks_in.len(), 0.0);
    let Some(block_cells) = n.checked_mul(n) else {
        panic!(
            "kfac_block_inverse CPU oracle n={n} overflows block cell count. Fix: shard K-FAC blocks before parity comparison."
        );
    };
    mat.clear();
    mat.resize(block_cells, 0.0);
    inv.clear();
    inv.resize(block_cells, 0.0);
    for b in 0..num_blocks as usize {
        let block_offset = b * block_cells;
        for i in 0..n {
            for j in 0..n {
                let idx = i * n + j;
                mat[idx] = blocks_in[block_offset + idx];
                inv[idx] = if i == j { 1.0 } else { 0.0 };
            }
        }
        // Gauss-Jordan
        for i in 0..n {
            let pivot = mat[i * n + i];
            for j in 0..n {
                mat[i * n + j] /= pivot;
                inv[i * n + j] /= pivot;
            }
            for k in 0..n {
                if k != i {
                    let factor = mat[k * n + i];
                    for j in 0..n {
                        mat[k * n + j] -= factor * mat[i * n + j];
                        inv[k * n + j] -= factor * inv[i * n + j];
                    }
                }
            }
        }
        for i in 0..n {
            for j in 0..n {
                let idx = i * n + j;
                out[block_offset + idx] = inv[idx];
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cpu_ref_1x1() {
        let blocks_in = vec![2.0];
        let out = cpu_ref(&blocks_in, 1, 1);
        assert_eq!(out, vec![0.5]);
    }

    #[test]
    fn test_cpu_ref_multi_block() {
        let blocks_in = vec![2.0, 0.0, 0.0, 2.0, 4.0, 0.0, 0.0, 4.0];
        let out = cpu_ref(&blocks_in, 2, 2);
        assert_eq!(out, vec![0.5, 0.0, 0.0, 0.5, 0.25, 0.0, 0.0, 0.25]);
    }

    #[test]
    fn test_cpu_ref_3x3_diag() {
        let blocks_in = vec![1.0, 0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 4.0];
        let out = cpu_ref(&blocks_in, 1, 3);
        assert_eq!(out, vec![1.0, 0.0, 0.0, 0.0, 0.5, 0.0, 0.0, 0.0, 0.25]);
    }

    #[test]
    fn test_cpu_ref_large_blocks() {
        let n = 2;
        let num_blocks = 64;
        let mut blocks_in = vec![0.0; num_blocks * n * n];
        for b in 0..num_blocks {
            blocks_in[b * 4] = 2.0;
            blocks_in[b * 4 + 3] = 2.0;
        }
        let out = cpu_ref(&blocks_in, num_blocks as u32, n as u32);
        assert_eq!(out[0], 0.5);
        assert_eq!(out[out.len() - 1], 0.5);
    }

    #[test]
    fn test_cpu_ref_asymmetric_values() {
        let blocks_in = vec![2.0, 1.0, 1.0, 2.0];
        // det = 4 - 1 = 3. Inv = 1/3 * [[2, -1], [-1, 2]] = [[0.666, -0.333], [-0.333, 0.666]]
        let out = cpu_ref(&blocks_in, 1, 2);
        assert!((out[0] - 0.6666667).abs() < 1e-6);
        assert!((out[1] - (-0.3333333)).abs() < 1e-6);
    }

    #[test]
    fn cpu_ref_into_reuses_output_and_flat_scratch() {
        let blocks_in = vec![2.0, 0.0, 0.0, 2.0, 4.0, 0.0, 0.0, 4.0];
        let mut out = Vec::with_capacity(16);
        let mut mat = Vec::with_capacity(8);
        let mut inv = Vec::with_capacity(8);
        out.extend_from_slice(&[99.0; 12]);
        mat.extend_from_slice(&[77.0; 6]);
        inv.extend_from_slice(&[55.0; 6]);
        let out_capacity = out.capacity();
        let mat_capacity = mat.capacity();
        let inv_capacity = inv.capacity();

        cpu_ref_into(&blocks_in, 2, 2, &mut out, &mut mat, &mut inv);

        assert_eq!(out, vec![0.5, 0.0, 0.0, 0.5, 0.25, 0.0, 0.0, 0.25]);
        assert_eq!(mat.len(), 4);
        assert_eq!(inv.len(), 4);
        assert_eq!(out.capacity(), out_capacity);
        assert_eq!(mat.capacity(), mat_capacity);
        assert_eq!(inv.capacity(), inv_capacity);

        cpu_ref_into(&[2.0], 1, 1, &mut out, &mut mat, &mut inv);
        assert_eq!(out, vec![0.5]);
        assert_eq!(mat, vec![1.0]);
        assert_eq!(inv, vec![0.5]);
        assert_eq!(out.capacity(), out_capacity);
        assert_eq!(mat.capacity(), mat_capacity);
        assert_eq!(inv.capacity(), inv_capacity);
    }

    #[test]
    fn test_parity_2x2() {
        let blocks_in = vec![4.0, 0.0, 0.0, 2.0];
        let p = kfac_block_inverse("bo", "bi", "s", 1, 2);

        let expected_out = cpu_ref(&blocks_in, 1, 2);

        use vyre_reference::reference_eval;
        use vyre_reference::value::Value;

        let to_value = |data: &[f32]| {
            let bytes = crate::wire::pack_f32_slice(data);
            Value::Bytes(Arc::from(bytes))
        };

        let inputs = vec![
            to_value(&[0.0; 4]),  // bo
            to_value(&blocks_in), // bi
            to_value(&[0.0; 4]),  // s
        ];

        let results = reference_eval(&p, &inputs).expect("Fix: interpreter failed");
        let actual_bytes = results[0].to_bytes();
        let actual_out: Vec<f32> = actual_bytes
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
            .collect();

        for (a, b) in actual_out.iter().zip(expected_out.iter()) {
            assert!((a - b).abs() < 1e-6);
        }
    }

    #[test]
    fn program_declares_three_buffers() {
        let p = kfac_block_inverse("bo", "bi", "s", 4, 4);
        assert_eq!(p.buffers().len(), 3);
    }

    #[test]
    fn rejects_zero_num_blocks_with_trap() {
        let p = kfac_block_inverse("bo", "bi", "s", 0, 4);
        assert!(p.stats().trap());
    }

    #[test]
    fn rejects_zero_n_with_trap() {
        let p = kfac_block_inverse("bo", "bi", "s", 4, 0);
        assert!(p.stats().trap());
    }
}
