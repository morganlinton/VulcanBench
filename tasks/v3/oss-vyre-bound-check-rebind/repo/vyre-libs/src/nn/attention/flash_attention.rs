//! Flash-attention tiled fusion  -  `softmax(Q·Kᵀ / √d) · V` computed
//! in a single pass per query row via online-softmax tiling.
//!
//! ROADMAP H4. The standard `attention` primitive in this crate
//! materialises three passes per row (max-reduction, sum-reduction,
//! write) and re-evaluates the dot-product score in each pass. Each
//! re-evaluation reloads `d` Q-values and `d * s` K-values from
//! global memory. For `s = 4096, d = 128` this is roughly
//! `3 * 4096 * 128 * 4 bytes = 6 MiB` of redundant reads per row.
//!
//! Flash-attention's contribution is the **online-softmax** trick:
//! maintain a running `(m, l, o)` state  -  running max, running
//! softmax denominator, running weighted-V sum  -  and update them
//! per-K-row in a single pass:
//!
//! ```text
//! For each query row i in [0, s):
//!   m = -INF; l = 0; o = [0; d]
//!   For each j in [0, s):
//!     score = scale * dot(Q[i,:], K[j,:])
//!     m_new = max(m, score)
//!     rescale = exp(m - m_new)
//!     l_new = rescale * l + exp(score - m_new)
//!     For t in [0, d):
//!       o[t] = rescale * o[t] + exp(score - m_new) * V[j, t]
//!     m = m_new; l = l_new
//!   For t in [0, d):
//!     out[i, t] = o[t] / l
//! ```
//!
//! Soundness: this is the standard online-softmax recurrence; for
//! every i, the final `(m, l, o)` after processing all j is
//! mathematically equivalent to the offline softmax-then-weighted-
//! sum that the reference attention computes.
//!
//! Cost direction: monotone-down on global-memory traffic. Each
//! `Q[i,k]` is loaded once across the j-loop (constant within the
//! per-row online pass) and each `K[j,k]` / `V[j,t]` is loaded
//! exactly once instead of three times. The per-row online-state
//! (`m, l, o[d]`) is held in workgroup-shared scratch.
//!
//! ## Implementation note
//!
//! This builder ships the per-row scalar online-softmax shape (one
//! invocation per row, scalar k-loop). The fully tiled variant that
//! parallelises the K-block scan + uses cooperative-warp reductions
//! over `d` lanes lands on top of this substrate; the algorithmic
//! correctness gate is the per-row reference.

use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program, UnOp};

use super::attention::direct_attention_program;
use super::planner::plan_flash_attention_scalar;
use crate::region::wrap_anonymous;
use vyre_primitives::nn::attention_stability::{
    bounded_exp_arg, bounded_score, flush_tiny, positive_denominator,
};

const OP_ID: &str = "vyre-libs::nn::flash_attention";

/// Build a Program that computes scaled dot-product attention via
/// the online-softmax (flash-attention) recurrence. Tensors are
/// `[s, d]` row-major F32; `out` has the same shape.
///
/// # Errors
///
/// Returns `Err` when `s == 0` or `d == 0` (empty reductions).
pub fn flash_attention(
    q: &str,
    k: &str,
    v: &str,
    out: &str,
    s: u32,
    d: u32,
) -> Result<Program, String> {
    if s == 0 {
        return Err("Fix: flash_attention s=0 is invalid: empty sequence".to_string());
    }
    if d == 0 {
        return Err("Fix: flash_attention d=0 is invalid: empty head dimension".to_string());
    }
    if let Some(program) = direct_attention_program(q, k, v, out, s, d, OP_ID)
        .map_err(|error| format!("Fix: flash_attention direct specialization failed: {error}"))?
    {
        return Ok(program);
    }
    let plan = plan_flash_attention_scalar(s, d)?;
    let elements = plan.logical_elements;
    let scratch_elements = plan.o_acc_scratch_elements;
    let scale = 1.0_f32 / (d as f32).sqrt();
    let scale_expr = Expr::f32(scale);
    let scratch_index = |t: Expr| Expr::add(Expr::mul(Expr::var("flash_local"), Expr::u32(d)), t);

    // Per-row online-softmax body. `row` is the query row index.
    let mut per_row = vec![
        // Initial state: m = -INF (use f32::MIN as the finite sentinel
        // the rest of the codebase already uses), l = 0.
        Node::let_bind("flash_m", Expr::f32(f32::MIN)),
        Node::let_bind("flash_l", Expr::f32(0.0)),
        // For each j in [0, s) update (m, l, o). Wrapped in a Region
        // marked with `source_region: Some(...)` so the structural
        // discipline gate treats the j/k_idx/t loop nest as a child
        // composition (`flash_attention_row_accumulate`) and stops
        // counting nodes/loops once it descends past the boundary.
        Node::loop_for(
            "j",
            Expr::u32(0),
            Expr::u32(s),
            vec![
                // score = scale * dot(Q[row, :], K[j, :])
                Node::let_bind("dot_val", Expr::f32(0.0)),
                Node::loop_for(
                    "k_idx",
                    Expr::u32(0),
                    Expr::u32(d),
                    vec![Node::assign(
                        "dot_val",
                        Expr::add(
                            Expr::var("dot_val"),
                            Expr::mul(
                                Expr::load(
                                    q,
                                    Expr::add(
                                        Expr::mul(Expr::var("row"), Expr::u32(d)),
                                        Expr::var("k_idx"),
                                    ),
                                ),
                                Expr::load(
                                    k,
                                    Expr::add(
                                        Expr::mul(Expr::var("j"), Expr::u32(d)),
                                        Expr::var("k_idx"),
                                    ),
                                ),
                            ),
                        ),
                    )],
                ),
                // Clamp ±inf (from Q/K dot-product overflow) to -80
                // BEFORE the online-softmax recurrence. inf would
                // become m_new=inf, then score-m_new=NaN, exp(NaN)=NaN
                // and poison the whole row. Crucially we preserve NaN
                // inputs so the kernel's NaN-input contract still
                // propagates them; only the finite-but-overflowing
                // case is repaired.
                Node::let_bind("score", {
                    let raw = Expr::mul(Expr::var("dot_val"), scale_expr.clone());
                    bounded_score(raw)
                }),
                // m_new = max(m, score)
                Node::let_bind(
                    "flash_m_new",
                    Expr::select(
                        Expr::gt(Expr::var("score"), Expr::var("flash_m")),
                        Expr::var("score"),
                        Expr::var("flash_m"),
                    ),
                ),
                // rescale = exp(m - m_new)  -  clamped to [0, 1]
                Node::let_bind(
                    "flash_rescale",
                    Expr::UnOp {
                        op: UnOp::Exp,
                        operand: Box::new(bounded_exp_arg(Expr::sub(
                            Expr::var("flash_m"),
                            Expr::var("flash_m_new"),
                        ))),
                    },
                ),
                // probability = exp(score - m_new)
                Node::let_bind(
                    "flash_prob",
                    Expr::UnOp {
                        op: UnOp::Exp,
                        operand: Box::new(bounded_exp_arg(Expr::sub(
                            Expr::var("score"),
                            Expr::var("flash_m_new"),
                        ))),
                    },
                ),
                // l_new = rescale * l + prob
                Node::let_bind(
                    "flash_l_new",
                    Expr::add(
                        Expr::mul(Expr::var("flash_rescale"), Expr::var("flash_l")),
                        Expr::var("flash_prob"),
                    ),
                ),
                // o[t] = rescale * o[t] + prob * V[j, t]
                Node::loop_for(
                    "t",
                    Expr::u32(0),
                    Expr::u32(d),
                    vec![Node::store(
                        "flash_o",
                        scratch_index(Expr::var("t")),
                        Expr::add(
                            Expr::mul(
                                Expr::var("flash_rescale"),
                                Expr::select(
                                    Expr::eq(Expr::var("j"), Expr::u32(0)),
                                    Expr::f32(0.0),
                                    Expr::load("flash_o", scratch_index(Expr::var("t"))),
                                ),
                            ),
                            Expr::mul(
                                Expr::var("flash_prob"),
                                Expr::load(
                                    v,
                                    Expr::add(
                                        Expr::mul(Expr::var("j"), Expr::u32(d)),
                                        Expr::var("t"),
                                    ),
                                ),
                            ),
                        ),
                    )],
                ),
                Node::assign("flash_m", Expr::var("flash_m_new")),
                Node::assign("flash_l", Expr::var("flash_l_new")),
            ],
        ),
        // Final: out[row, t] = o[t] / max(l, MIN_POSITIVE)
        Node::let_bind("flash_denom", positive_denominator(Expr::var("flash_l"))),
        Node::loop_for(
            "out_t",
            Expr::u32(0),
            Expr::u32(d),
            vec![Node::store(
                out,
                Expr::add(
                    Expr::mul(Expr::var("row"), Expr::u32(d)),
                    Expr::var("out_t"),
                ),
                flush_tiny(Expr::div(
                    Expr::load("flash_o", scratch_index(Expr::var("out_t"))),
                    Expr::var("flash_denom"),
                )),
            )],
        ),
    ];

    // Wrap the per-row body in `let row = InvocationId.x; if row < s
    // { body }`. One invocation per row.
    let mut body_with_guard = vec![
        Node::let_bind("row", Expr::InvocationId { axis: 0 }),
        Node::let_bind("flash_local", Expr::LocalId { axis: 0 }),
    ];
    body_with_guard.push(Node::if_then(
        Expr::lt(Expr::var("row"), Expr::u32(s)),
        std::mem::take(&mut per_row),
    ));

    Ok(Program::wrapped(
        vec![
            BufferDecl::storage(q, 0, BufferAccess::ReadOnly, DataType::F32).with_count(elements),
            BufferDecl::storage(k, 1, BufferAccess::ReadOnly, DataType::F32).with_count(elements),
            BufferDecl::storage(v, 2, BufferAccess::ReadOnly, DataType::F32).with_count(elements),
            BufferDecl::workgroup("flash_o", scratch_elements, DataType::F32),
            BufferDecl::output(out, 3, DataType::F32).with_count(elements),
        ],
        [plan.workgroup_lanes, 1, 1],
        vec![wrap_anonymous(OP_ID, body_with_guard)],
    ))
}

inventory::submit! {
    crate::harness::OpEntry {
        id: "vyre-libs::nn::flash_attention",
        build: || {
            flash_attention("q", "k", "v", "out", 9, 1).unwrap_or_else(|error| {
                crate::builder::invalid_output_program(
                    "vyre-libs::nn::flash_attention",
                    "out",
                    DataType::F32,
                    error,
                )
            })
        },
        test_inputs: Some(|| {
            let q = [0.0_f32; 9];
            let k = [0.0_f32; 9];
            let v = [0.0_f32, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
            vec![vec![
                vyre_primitives::wire::pack_f32_slice(&q),
                vyre_primitives::wire::pack_f32_slice(&k),
                vyre_primitives::wire::pack_f32_slice(&v),
                vec![0u8; 9 * core::mem::size_of::<f32>()],
            ]]
        }),
        // This deliberately uses s=9 so `direct_attention_program` declines
        // the tiny-shape specialization and the registered op covers the real
        // online-softmax flash kernel. With zero Q/K, every row has uniform
        // weights and returns mean(V)=4.0.
        expected_output: Some(|| {
            vec![vec![vyre_primitives::wire::pack_f32_slice(&[4.0_f32; 9])]]
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

    /// Online-softmax flash-attention agrees with the offline 3-pass
    /// `attention_reference` on a non-trivial fixture.
    #[test]
    fn flash_attention_matches_attention_reference() {
        let s = 9_u32;
        let d = 7_u32;
        let elements = (s * d) as usize;
        let q: Vec<f32> = (0..elements)
            .map(|i| ((i as f32) * 0.13).sin() - 0.5)
            .collect();
        let k: Vec<f32> = (0..elements)
            .map(|i| ((i as f32) * 0.07).cos() + 0.25)
            .collect();
        let v: Vec<f32> = (0..elements)
            .map(|i| ((i as f32) * 0.19).sin() * 2.0)
            .collect();
        let run = |program: Program| {
            let out_bytes = program
                .buffers()
                .iter()
                .find(|b| b.name() == "out")
                .map(|b| b.count() as usize * core::mem::size_of::<f32>())
                .expect("Fix: output buffer present");
            let outputs = vyre_reference::reference_eval(
                &program,
                &[
                    Value::from(f32_bytes(&q)),
                    Value::from(f32_bytes(&k)),
                    Value::from(f32_bytes(&v)),
                    Value::from(vec![0u8; out_bytes]),
                ],
            )
            .expect("Fix: flash_attention must execute in the reference interpreter.");
            decode_f32(&outputs[0].to_bytes())
        };
        let actual = run(flash_attention("q", "k", "v", "out", s, d).expect("Fix: build"));
        let expected = run(crate::nn::attention::attention_reference(
            "q", "k", "v", "out", s, d,
        ));
        assert_eq!(actual.len(), expected.len(), "output length must match");
        for (i, (a, e)) in actual.iter().zip(expected.iter()).enumerate() {
            assert!(
                (a - e).abs() <= 1.0e-4,
                "flash_attention vs attention_reference mismatch at index {i}: {a} != {e}"
            );
        }
    }

    #[test]
    fn flash_attention_online_kernel_uniform_scores_return_value_mean() {
        let s = 9_u32;
        let d = 1_u32;
        let q = vec![0.0_f32; s as usize];
        let k = vec![0.0_f32; s as usize];
        let v: Vec<f32> = (0..s).map(|idx| idx as f32).collect();
        let program = flash_attention("q", "k", "v", "out", s, d).expect("Fix: build");
        assert_eq!(
            program.workgroup_size(),
            [128, 1, 1],
            "Fix: s=9 must bypass direct_attention_program and use the online flash kernel."
        );
        let outputs = vyre_reference::reference_eval(
            &program,
            &[
                Value::from(f32_bytes(&q)),
                Value::from(f32_bytes(&k)),
                Value::from(f32_bytes(&v)),
                Value::from(vec![0u8; (s * d) as usize * 4]),
            ],
        )
        .expect("Fix: flash_attention online kernel must execute in the reference interpreter.");
        let actual = decode_f32(&outputs[0].to_bytes());
        assert_eq!(actual.len(), s as usize);
        for (idx, value) in actual.iter().enumerate() {
            assert!(
                (*value - 4.0).abs() <= 1.0e-5,
                "uniform-score flash attention row {idx} should return mean(V)=4.0, got {value}"
            );
        }
    }

    /// `flash_attention(0, _)` rejects empty sequence with an
    /// actionable Fix message.
    #[test]
    fn flash_attention_rejects_empty_seq() {
        let err = flash_attention("q", "k", "v", "out", 0, 4).expect_err("empty s must error");
        assert!(err.contains("s=0"));
    }

    /// `flash_attention(_, 0)` rejects empty head dim.
    #[test]
    fn flash_attention_rejects_empty_head_dim() {
        let err = flash_attention("q", "k", "v", "out", 4, 0).expect_err("empty d must error");
        assert!(err.contains("d=0"));
    }

    /// Single-row (s=1) attention degenerates to V (because softmax
    /// of a length-1 score vector is [1.0]).
    #[test]
    fn flash_attention_single_row_passes_v_through() {
        let d = 4_u32;
        let q = vec![1.0_f32, 2.0, 3.0, 4.0];
        let k = vec![0.5_f32, 1.5, 2.5, 3.5];
        let v = vec![10.0_f32, 20.0, 30.0, 40.0];
        let prog = flash_attention("q", "k", "v", "out", 1, d).expect("Fix: build");
        let outputs = vyre_reference::reference_eval(
            &prog,
            &[
                Value::from(f32_bytes(&q)),
                Value::from(f32_bytes(&k)),
                Value::from(f32_bytes(&v)),
                Value::from(vec![0u8; (d as usize) * 4]),
            ],
        )
        .expect("Fix: eval");
        let actual = decode_f32(&outputs[0].to_bytes());
        for (a, e) in actual.iter().zip(v.iter()) {
            assert!(
                (a - e).abs() <= 1.0e-4,
                "single-row attention should pass V through: {a} != {e}"
            );
        }
    }

    #[test]
    fn flash_attention_very_large_qk_values_stay_finite() {
        // Very large Q and K should produce bounded scores due to bounded_exp_arg.
        let s = 2u32;
        let d = 2u32;
        let q = [1e20f32, 1e20, 1e20, 1e20];
        let k = [1e20f32, 1e20, 1e20, 1e20];
        let v = [1.0f32, 2.0, 3.0, 4.0];
        let prog = flash_attention("q", "k", "v", "out", s, d).expect("Fix: build");
        let outputs = vyre_reference::reference_eval(
            &prog,
            &[
                Value::from(f32_bytes(&q)),
                Value::from(f32_bytes(&k)),
                Value::from(f32_bytes(&v)),
                Value::from(vec![0u8; (s * d) as usize * 4]),
            ],
        )
        .expect("Fix: flash_attention must not panic on very large QK values");
        let out = decode_f32(&outputs[0].to_bytes());
        for (i, &v) in out.iter().enumerate() {
            assert!(
                v.is_finite(),
                "flash_attention output at {i} must be finite for large QK values, got {v}"
            );
        }
    }

    #[test]
    fn flash_attention_nan_in_q_k_v_is_silently_suppressed() {
        let s = 1u32;
        let d = 2u32;
        let q = [f32::NAN, 0.0];
        let k = [0.0f32, 0.0];
        let v = [1.0f32, 2.0];
        let prog = flash_attention("q", "k", "v", "out", s, d).expect("Fix: build");
        let outputs = vyre_reference::reference_eval(
            &prog,
            &[
                Value::from(f32_bytes(&q)),
                Value::from(f32_bytes(&k)),
                Value::from(f32_bytes(&v)),
                Value::from(vec![0u8; 8]),
            ],
        )
        .expect("Fix: flash_attention must not panic on NaN input");
        let out = decode_f32(&outputs[0].to_bytes());
        assert!(
            out.iter().any(|v| v.is_nan()),
            "flash_attention must propagate NaN in Q/K/V instead of silently producing finite output {:?}",
            out
        );
    }
}
