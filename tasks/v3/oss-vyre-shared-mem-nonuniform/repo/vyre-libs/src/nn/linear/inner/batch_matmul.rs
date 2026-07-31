//! Batched matrix multiplication: `out[b, i, j] = sum_k a[b, i, k] * b[b, k, j]`.
//!
//! Category A composition. Each invocation computes one output element.

use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

use crate::region::wrap_anonymous;

/// Build a Program that computes batched matmul.
///
/// Shapes: `a: [batch, m, k]`, `b: [batch, k, n]`, `out: [batch, m, n]`.
/// Each invocation computes one `out[b, i, j]` by iterating the `k` dimension.
///
/// # Errors
/// Returns `Err` when any dimension is zero or total elements overflow u32.
pub fn batch_matmul(
    a: &str,
    b: &str,
    out: &str,
    batch: u32,
    m: u32,
    k: u32,
    n: u32,
) -> Result<Program, String> {
    if batch == 0 || m == 0 || k == 0 || n == 0 {
        return Err("Fix: batch_matmul all dims must be > 0".to_string());
    }

    let a_batch_stride = m
        .checked_mul(k)
        .ok_or("Fix: batch_matmul a_batch_stride overflow")?;
    let b_batch_stride = k
        .checked_mul(n)
        .ok_or("Fix: batch_matmul b_batch_stride overflow")?;
    let out_batch_stride = m
        .checked_mul(n)
        .ok_or("Fix: batch_matmul out_batch_stride overflow")?;
    let a_count = batch
        .checked_mul(a_batch_stride)
        .ok_or("Fix: batch_matmul a_count overflow")?;
    let b_count = batch
        .checked_mul(b_batch_stride)
        .ok_or("Fix: batch_matmul b_count overflow")?;
    let out_count = batch
        .checked_mul(out_batch_stride)
        .ok_or("Fix: batch_matmul out_count overflow")?;

    let idx = Expr::var("idx");
    let batch_idx = Expr::var("batch_idx");
    let row = Expr::var("row");
    let col = Expr::var("col");
    let local_idx = Expr::var("local_idx");

    let body = vec![
        Node::let_bind("idx", Expr::InvocationId { axis: 0 }),
        Node::let_bind(
            "batch_idx",
            Expr::div(idx.clone(), Expr::u32(out_batch_stride)),
        ),
        Node::let_bind(
            "local_idx",
            Expr::rem(idx.clone(), Expr::u32(out_batch_stride)),
        ),
        Node::let_bind("row", Expr::div(local_idx.clone(), Expr::u32(n))),
        Node::let_bind("col", Expr::rem(local_idx.clone(), Expr::u32(n))),
        Node::if_then(
            Expr::lt(idx.clone(), Expr::buf_len(out)),
            vec![
                Node::let_bind("acc", Expr::f32(0.0)),
                Node::loop_for(
                    "kk",
                    Expr::u32(0),
                    Expr::u32(k),
                    vec![Node::assign(
                        "acc",
                        Expr::add(
                            Expr::var("acc"),
                            Expr::mul(
                                Expr::load(
                                    a,
                                    Expr::add(
                                        Expr::mul(batch_idx.clone(), Expr::u32(a_batch_stride)),
                                        Expr::add(
                                            Expr::mul(row.clone(), Expr::u32(k)),
                                            Expr::var("kk"),
                                        ),
                                    ),
                                ),
                                Expr::load(
                                    b,
                                    Expr::add(
                                        Expr::mul(batch_idx.clone(), Expr::u32(b_batch_stride)),
                                        Expr::add(
                                            Expr::mul(Expr::var("kk"), Expr::u32(n)),
                                            col.clone(),
                                        ),
                                    ),
                                ),
                            ),
                        ),
                    )],
                ),
                Node::Store {
                    buffer: out.into(),
                    index: idx,
                    value: Expr::var("acc"),
                },
            ],
        ),
    ];

    Ok(Program::wrapped(
        vec![
            BufferDecl::storage(a, 0, BufferAccess::ReadOnly, DataType::F32).with_count(a_count),
            BufferDecl::storage(b, 1, BufferAccess::ReadOnly, DataType::F32).with_count(b_count),
            BufferDecl::output(out, 2, DataType::F32).with_count(out_count),
        ],
        [256, 1, 1],
        vec![wrap_anonymous("vyre-libs::nn::batch_matmul", body)],
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::byte_pack::decode_f32;
    use crate::test_support::byte_pack::f32_bytes;
    use vyre_reference::value::Value;

    #[test]
    fn batch_matmul_single_batch_matches_matmul() {
        // batch=1, m=2, k=3, n=2
        let a = vec![1.0f32, 2.0, 3.0, 4.0, 5.0, 6.0]; // [1, 2, 3]
        let b = vec![1.0f32, 2.0, 3.0, 4.0, 5.0, 6.0]; // [1, 3, 2]
                                                       // out[0,0,0] = 1*1 + 2*3 + 3*5 = 1 + 6 + 15 = 22
                                                       // out[0,0,1] = 1*2 + 2*4 + 3*6 = 2 + 8 + 18 = 28
                                                       // out[0,1,0] = 4*1 + 5*3 + 6*5 = 4 + 15 + 30 = 49
                                                       // out[0,1,1] = 4*2 + 5*4 + 6*6 = 8 + 20 + 36 = 64
        let program = batch_matmul("a", "b", "out", 1, 2, 3, 2).unwrap();
        let outputs = vyre_reference::reference_eval(
            &program,
            &[
                Value::from(f32_bytes(&a)),
                Value::from(f32_bytes(&b)),
                Value::from(vec![0u8; 4 * 4]),
            ],
        )
        .expect("Fix: batch_matmul single batch must execute");
        let out = decode_f32(&outputs[0].to_bytes());
        assert_eq!(out, vec![22.0, 28.0, 49.0, 64.0]);
    }

    #[test]
    fn batch_matmul_two_batches() {
        // batch=2, m=2, k=2, n=2
        let a = vec![
            1.0f32, 0.0, 0.0, 1.0, // batch 0: identity
            2.0f32, 0.0, 0.0, 2.0, // batch 1: 2*identity
        ];
        let b = vec![
            1.0f32, 2.0, 3.0, 4.0, // batch 0
            5.0f32, 6.0, 7.0, 8.0, // batch 1
        ];
        // batch 0: identity @ b[0] = b[0] = [1,2,3,4]
        // batch 1: 2*identity @ b[1] = 2*b[1] = [10,12,14,16]
        let program = batch_matmul("a", "b", "out", 2, 2, 2, 2).unwrap();
        let outputs = vyre_reference::reference_eval(
            &program,
            &[
                Value::from(f32_bytes(&a)),
                Value::from(f32_bytes(&b)),
                Value::from(vec![0u8; 4 * 4 * 2]),
            ],
        )
        .expect("Fix: batch_matmul two batches must execute");
        let out = decode_f32(&outputs[0].to_bytes());
        assert_eq!(out, vec![1.0, 2.0, 3.0, 4.0, 10.0, 12.0, 14.0, 16.0]);
    }

    #[test]
    fn batch_matmul_zero_dim_errors() {
        for (batch, m, k, n) in [(0, 2, 2, 2), (1, 0, 2, 2), (1, 2, 0, 2), (1, 2, 2, 0)] {
            let err =
                batch_matmul("a", "b", "out", batch, m, k, n).expect_err("zero dim must error");
            assert!(
                err.contains("batch_matmul") && err.contains("> 0"),
                "batch_matmul zero-dim error for ({batch},{m},{k},{n}): {err}"
            );
        }
    }
}
