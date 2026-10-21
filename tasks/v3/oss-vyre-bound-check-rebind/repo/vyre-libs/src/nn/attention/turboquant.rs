//! TurboQuant-style 3-bit bit-packed KV scoring.
//!
//! Computes the per-token attention score `score[i] = dot(q, dequant_k(i))`
//! over a 3-bit-packed key matrix, and the linear-attention accumulator
//! `out[d] = sum_i score[i] * dequant_v(i, d)` over a 3-bit-packed value
//! matrix. No softmax  -  the softmax numerator / denominator pair is the job
//! of a separate `softmax_rowwise` composition that can stack on top of this
//! op; keeping this kernel softmax-free makes the witness byte-deterministic.
//!
//! 3-bit packing layout: each u32 holds 10 values, shifted by `bit_idx * 3`
//! from the LSB (the top 2 bits of each u32 are unused). A flat index
//! `flat = i * d_head + d` unpacks as
//! `(packed[flat / 10] >> ((flat % 10) * 3)) & 0x7`, cast value-preserving
//! to f32.

use crate::region::wrap_anonymous;
use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

const OP_ID: &str = "vyre-libs::nn::attention::turboquant";

/// Build a Program that computes `out[d] = Σᵢ dot(q, dequant_k[i,:]) * dequant_v[i,d]`.
///
/// Buffers (binding order):
/// - `q` (ReadOnly, F32, `d_head`)
/// - `k_packed` (ReadOnly, U32, ceil((seq_len·d_head) / 10))
/// - `v_packed` (ReadOnly, U32, ceil((seq_len·d_head) / 10))
/// - `out` (ReadWrite, F32, `d_head`)
#[must_use]
pub fn turboquant_attention(
    q: &str,
    k_packed: &str,
    v_packed: &str,
    out: &str,
    seq_len: u32,
    d_head: u32,
) -> Program {
    // Number of u32 words required to pack seq_len * d_head 3-bit values.
    let total_vals = seq_len.saturating_mul(d_head);
    let packed_words = total_vals.div_ceil(10);

    // Unpack helper  -  emits an Expr that decodes `(flat_idx)`-th 3-bit value
    // from `buf` as an f32.
    //   word = buf[flat / 10]
    //   nib  = (word >> ((flat % 10) * 3)) & 0x7
    //   cast u32→f32 value-preserving
    let unpack_3bit = |buf: &str, flat: Expr| {
        let word = Expr::load(buf, Expr::div(flat.clone(), Expr::u32(10)));
        let shift = Expr::mul(Expr::rem(flat, Expr::u32(10)), Expr::u32(3));
        let nib = Expr::bitand(Expr::shr(word, shift), Expr::u32(0x7));
        Expr::select(
            Expr::eq(nib.clone(), Expr::u32(0)),
            Expr::f32(0.0),
            Expr::select(
                Expr::eq(nib.clone(), Expr::u32(1)),
                Expr::f32(1.0),
                Expr::select(
                    Expr::eq(nib.clone(), Expr::u32(2)),
                    Expr::f32(2.0),
                    Expr::select(
                        Expr::eq(nib.clone(), Expr::u32(3)),
                        Expr::f32(3.0),
                        Expr::select(
                            Expr::eq(nib.clone(), Expr::u32(4)),
                            Expr::f32(4.0),
                            Expr::select(
                                Expr::eq(nib.clone(), Expr::u32(5)),
                                Expr::f32(5.0),
                                Expr::select(
                                    Expr::eq(nib, Expr::u32(6)),
                                    Expr::f32(6.0),
                                    Expr::f32(7.0),
                                ),
                            ),
                        ),
                    ),
                ),
            ),
        )
    };

    if seq_len <= 8 && d_head <= 16 {
        let mut stores = Vec::with_capacity(d_head as usize);
        for dim in 0..d_head {
            let mut acc = Expr::f32(0.0);
            for i in 0..seq_len {
                let mut score = Expr::f32(0.0);
                for e in 0..d_head {
                    score = Expr::add(
                        score,
                        Expr::mul(
                            Expr::load(q, Expr::u32(e)),
                            unpack_3bit(k_packed, Expr::u32(i * d_head + e)),
                        ),
                    );
                }
                acc = Expr::add(
                    acc,
                    Expr::mul(score, unpack_3bit(v_packed, Expr::u32(i * d_head + dim))),
                );
            }
            stores.push(Node::store(out, Expr::u32(dim), acc));
        }
        return Program::wrapped(
            vec![
                BufferDecl::storage(q, 0, BufferAccess::ReadOnly, DataType::F32).with_count(d_head),
                BufferDecl::storage(k_packed, 1, BufferAccess::ReadOnly, DataType::U32)
                    .with_count(packed_words),
                BufferDecl::storage(v_packed, 2, BufferAccess::ReadOnly, DataType::U32)
                    .with_count(packed_words),
                BufferDecl::storage(out, 3, BufferAccess::ReadWrite, DataType::F32)
                    .with_count(d_head),
            ],
            [1, 1, 1],
            vec![wrap_anonymous(
                OP_ID,
                vec![Node::if_then(
                    Expr::eq(Expr::InvocationId { axis: 0 }, Expr::u32(0)),
                    stores,
                )],
            )],
        );
    }

    // Per output lane d: walk i in 0..seq_len, accumulate score*V.
    let t = Expr::InvocationId { axis: 0 };

    let inner_body = vec![
        Node::let_bind("d", t.clone()),
        Node::let_bind("acc", Expr::f32(0.0)),
        Node::loop_for(
            "i",
            Expr::u32(0),
            Expr::u32(seq_len),
            vec![
                // score_i = dot(q, dequant_k[i, :])
                Node::let_bind("score", Expr::f32(0.0)),
                Node::loop_for(
                    "e",
                    Expr::u32(0),
                    Expr::u32(d_head),
                    vec![Node::assign(
                        "score",
                        Expr::add(
                            Expr::var("score"),
                            Expr::mul(
                                Expr::load(q, Expr::var("e")),
                                unpack_3bit(
                                    k_packed,
                                    Expr::add(
                                        Expr::mul(Expr::var("i"), Expr::u32(d_head)),
                                        Expr::var("e"),
                                    ),
                                ),
                            ),
                        ),
                    )],
                ),
                // acc += score * dequant_v[i, d]
                Node::assign(
                    "acc",
                    Expr::add(
                        Expr::var("acc"),
                        Expr::mul(
                            Expr::var("score"),
                            unpack_3bit(
                                v_packed,
                                Expr::add(
                                    Expr::mul(Expr::var("i"), Expr::u32(d_head)),
                                    Expr::var("d"),
                                ),
                            ),
                        ),
                    ),
                ),
            ],
        ),
        Node::store(out, Expr::var("d"), Expr::var("acc")),
    ];

    Program::wrapped(
        vec![
            BufferDecl::storage(q, 0, BufferAccess::ReadOnly, DataType::F32).with_count(d_head),
            BufferDecl::storage(k_packed, 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(packed_words),
            BufferDecl::storage(v_packed, 2, BufferAccess::ReadOnly, DataType::U32)
                .with_count(packed_words),
            BufferDecl::storage(out, 3, BufferAccess::ReadWrite, DataType::F32).with_count(d_head),
        ],
        [64, 1, 1],
        vec![wrap_anonymous(
            OP_ID,
            vec![Node::if_then(
                Expr::lt(t.clone(), Expr::u32(d_head)),
                inner_body,
            )],
        )],
    )
}

inventory::submit! {
    crate::harness::OpEntry {
        id: OP_ID,
        build: || turboquant_attention("q", "kp", "vp", "out", 2, 2),
        test_inputs: Some(|| {
            let to_f32_bytes =
                |w: &[f32]| vyre_primitives::wire::pack_f32_slice(w);

            // seq_len=2, d_head=2 → 4 packed values per buffer, 1 u32 holds them.
            // 3-bit layout (LSB first): shifts 0, 3, 6, 9.
            //
            // K values flat:            [k00=1, k01=2, k10=3, k11=4]
            //   word = 1 | (2<<3) | (3<<6) | (4<<9) = 1 + 16 + 192 + 2048 = 0b1000_0011_0001_0001 = 0x831  (no overflow: 4 fits in 3 bits max 7)
            //   Actually 1 + 16 + 192 + 2048 = 2257 = 0x8D1.
            // V values flat:            [v00=1, v01=0, v10=0, v11=1]
            //   word = 1 + 0 + 0 + (1<<9) = 1 + 512 = 513 = 0x201.
            // q = [1.0, 1.0].
            vec![vec![
                to_f32_bytes(&[1.0, 1.0]),
                crate::test_support::byte_pack::u32_bytes(&[0x8D1u32]),
                crate::test_support::byte_pack::u32_bytes(&[0x201u32]),
                vec![0u8; 2 * 4],
            ]]
        }),
        expected_output: Some(|| {
            let to_f32_bytes =
                |w: &[f32]| vyre_primitives::wire::pack_f32_slice(w);
            // score[0] = dot([1,1], [k00=1, k01=2]) = 3
            // score[1] = dot([1,1], [k10=3, k11=4]) = 7
            // out[0] = score[0]*v00 + score[1]*v10 = 3*1 + 7*0 = 3
            // out[1] = score[0]*v01 + score[1]*v11 = 3*0 + 7*1 = 7
            vec![vec![to_f32_bytes(&[3.0, 7.0])]]
        }),
        category: Some("nn"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::byte_pack::decode_f32;
    use crate::test_support::byte_pack::f32_bytes;
    use vyre_reference::value::Value;

    #[test]
    fn turboquant_nan_in_q_propagates_to_output() {
        let q = [f32::NAN, 1.0];
        // k_packed: 4 values in 1 u32, all zeros
        let kp = crate::test_support::byte_pack::u32_bytes(&[0u32]);
        let vp = crate::test_support::byte_pack::u32_bytes(&[0u32]);
        let program = turboquant_attention("q", "kp", "vp", "out", 2, 2);
        let outputs = vyre_reference::reference_eval(
            &program,
            &[
                Value::from(f32_bytes(&q)),
                Value::from(kp),
                Value::from(vp),
                Value::from(vec![0u8; 8]),
            ],
        )
        .expect("Fix: turboquant must not panic on NaN q");
        let out = decode_f32(&outputs[0].to_bytes());
        assert!(
            out.iter().all(|v| v.is_nan()),
            "turboquant NaN in q must produce NaN output, got {:?}",
            out
        );
    }

    #[test]
    fn turboquant_zero_seq_len() {
        let q = [1.0f32, 1.0];
        let kp = crate::test_support::byte_pack::u32_bytes(&[0u32]);
        let vp = crate::test_support::byte_pack::u32_bytes(&[0u32]);
        let program = turboquant_attention("q", "kp", "vp", "out", 0, 2);
        let outputs = vyre_reference::reference_eval(
            &program,
            &[
                Value::from(f32_bytes(&q)),
                Value::from(kp),
                Value::from(vp),
                Value::from(vec![0u8; 8]),
            ],
        )
        .expect("Fix: turboquant seq_len=0 must not panic");
        let out = decode_f32(&outputs[0].to_bytes());
        assert_eq!(
            out,
            vec![0.0, 0.0],
            "turboquant zero seq_len must produce zeros"
        );
    }

    #[test]
    fn turboquant_single_token() {
        let q = [1.0f32, 1.0];
        // k_packed: 2 values in 1 u32: both 1 (bits 0 and 3)
        // word = 1 | (1<<3) = 1 + 8 = 9
        let kp = crate::test_support::byte_pack::u32_bytes(&[9u32]);
        // v_packed: same
        let vp = crate::test_support::byte_pack::u32_bytes(&[9u32]);
        let program = turboquant_attention("q", "kp", "vp", "out", 1, 2);
        let outputs = vyre_reference::reference_eval(
            &program,
            &[
                Value::from(f32_bytes(&q)),
                Value::from(kp),
                Value::from(vp),
                Value::from(vec![0u8; 8]),
            ],
        )
        .expect("Fix: turboquant single token must execute");
        let out = decode_f32(&outputs[0].to_bytes());
        // score = dot([1,1], [1,1]) = 2
        // out[d] = score * v[0,d] = 2 * 1 = 2 for both lanes
        assert_eq!(out, vec![2.0, 2.0]);
    }
}
