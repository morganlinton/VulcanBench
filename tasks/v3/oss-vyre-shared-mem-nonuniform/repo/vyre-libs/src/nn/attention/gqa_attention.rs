//! Grouped-Query Attention: n_q Q heads, n_kv KV heads (replicate K/V).
//!
//! Full 3-pass softmax (max, sum, weighted-write) with KV-head broadcasting.

use vyre::ir::{BinOp, BufferAccess, BufferDecl, DataType, Expr, Node, Program, UnOp};
use vyre_foundation::ir::model::expr::GeneratorRef;
use vyre_primitives::nn::attention_passes::{
    ATTENTION_MAX_PASS_OP_ID, ATTENTION_SUM_PASS_OP_ID, ATTENTION_WRITE_PASS_OP_ID,
};

use crate::region::{wrap_anonymous, wrap_child};
use vyre_primitives::nn::attention_stability::{
    bounded_exp_arg, bounded_score, flush_tiny, positive_denominator,
};

const OP_ID: &str = "vyre-libs::nn::gqa_attention";

/// Build GQA attention (F32). n_q_heads must be a multiple of n_kv_heads.
///
/// # Errors
/// Returns `Err` on dimension violations.
#[allow(clippy::too_many_arguments)]
pub fn gqa_attention(
    q: &str,
    k: &str,
    v_buf: &str,
    output: &str,
    n_q_heads: u32,
    n_kv_heads: u32,
    seq_len: u32,
    head_dim: u32,
) -> Result<Program, String> {
    if n_q_heads == 0 || n_kv_heads == 0 || seq_len == 0 || head_dim == 0 {
        return Err("Fix: gqa_attention requires non-zero dims".into());
    }
    if n_q_heads % n_kv_heads != 0 {
        return Err("Fix: n_q_heads must be multiple of n_kv_heads".into());
    }
    let group_size = n_q_heads / n_kv_heads;
    let q_total = n_q_heads * seq_len * head_dim;
    let per_head = seq_len * head_dim;
    let scale = 1.0f32 / (head_dim as f32).sqrt();

    let flat = Expr::var("flat");
    let q_head = Expr::BinOp {
        op: BinOp::Div,
        left: Box::new(flat.clone()),
        right: Box::new(Expr::u32(per_head)),
    };
    let pos_in_head = Expr::sub(flat.clone(), Expr::mul(q_head.clone(), Expr::u32(per_head)));
    let row = Expr::BinOp {
        op: BinOp::Div,
        left: Box::new(pos_in_head.clone()),
        right: Box::new(Expr::u32(head_dim)),
    };
    let col = Expr::sub(pos_in_head, Expr::mul(row.clone(), Expr::u32(head_dim)));
    let kv_head = Expr::BinOp {
        op: BinOp::Div,
        left: Box::new(q_head.clone()),
        right: Box::new(Expr::u32(group_size)),
    };
    let kv_base = Expr::mul(kv_head, Expr::u32(per_head));
    let q_row_base = Expr::add(
        Expr::mul(q_head, Expr::u32(per_head)),
        Expr::mul(row, Expr::u32(head_dim)),
    );

    // Helper: dot(Q[row], K[j]) for a given "j" variable
    let make_dot_loop = |dot_var: &str| -> Vec<Node> {
        let mut body = vec![Node::let_bind(dot_var, Expr::f32(0.0))];
        if head_dim <= 8 {
            body.extend((0..head_dim).map(|lane| {
                Node::assign(
                    dot_var,
                    Expr::fma(
                        Expr::load(q, Expr::add(q_row_base.clone(), Expr::u32(lane))),
                        Expr::load(
                            k,
                            Expr::add(
                                Expr::add(
                                    kv_base.clone(),
                                    Expr::mul(Expr::var("j"), Expr::u32(head_dim)),
                                ),
                                Expr::u32(lane),
                            ),
                        ),
                        Expr::var(dot_var),
                    ),
                )
            }));
        } else {
            body.push(Node::loop_for(
                "d",
                Expr::u32(0),
                Expr::u32(head_dim),
                vec![Node::assign(
                    dot_var,
                    Expr::fma(
                        Expr::load(q, Expr::add(q_row_base.clone(), Expr::var("d"))),
                        Expr::load(
                            k,
                            Expr::add(
                                Expr::add(
                                    kv_base.clone(),
                                    Expr::mul(Expr::var("j"), Expr::u32(head_dim)),
                                ),
                                Expr::var("d"),
                            ),
                        ),
                        Expr::var(dot_var),
                    ),
                )],
            ));
        }
        body
    };

    let exp_expr = |dot_var: &str| -> Expr {
        Expr::UnOp {
            op: UnOp::Exp,
            operand: Box::new(bounded_exp_arg(Expr::sub(
                Expr::mul(Expr::var(dot_var), Expr::f32(scale)),
                Expr::var("max_score"),
            ))),
        }
    };

    let parent = GeneratorRef {
        name: OP_ID.to_string(),
    };

    // Each `wrap_child(...)` below creates a new Region scope, so
    // any `Node::let_bind` inside a pass body dies when that pass's
    // region exits  -  yet `sum_pass`'s `exp_expr` reads `max_score`
    // and the post-pass `let_bind("denom", positive_denominator(sum_exp))`
    // reads `sum_exp`. The outer if_then body now declares both
    // accumulators up front and the per-pass bodies write into
    // them with Node::assign instead of redeclaring.
    let max_pass = {
        let mut nodes = vec![];
        nodes.push(Node::loop_for("j", Expr::u32(0), Expr::u32(seq_len), {
            let mut v = make_dot_loop("dot");
            v.push(Node::let_bind(
                "score",
                // Clamp ±inf (overflow on large Q/K) to -80 BEFORE the
                // softmax recurrence so it doesn't become NaN via inf-inf.
                // Preserve NaN inputs intact  -  the NaN-input contract
                // requires those to flow through to the output.
                {
                    let raw = Expr::mul(Expr::var("dot"), Expr::f32(scale));
                    bounded_score(raw)
                },
            ));
            v.push(Node::assign(
                "max_score",
                Expr::select(
                    Expr::is_nan(Expr::var("score")),
                    Expr::var("score"),
                    Expr::select(
                        Expr::gt(Expr::var("score"), Expr::var("max_score")),
                        Expr::var("score"),
                        Expr::var("max_score"),
                    ),
                ),
            ));
            v
        }));
        nodes
    };

    let sum_pass = {
        let mut nodes = vec![];
        nodes.push(Node::loop_for("j", Expr::u32(0), Expr::u32(seq_len), {
            let mut v = make_dot_loop("dot2");
            v.push(Node::assign(
                "sum_exp",
                Expr::add(Expr::var("sum_exp"), exp_expr("dot2")),
            ));
            v
        }));
        nodes
    };

    let write_pass = {
        let mut nodes = vec![Node::let_bind("val", Expr::f32(0.0))];
        nodes.push(Node::loop_for("j", Expr::u32(0), Expr::u32(seq_len), {
            let mut v = make_dot_loop("dot3");
            v.push(Node::let_bind(
                "w",
                Expr::div(exp_expr("dot3"), Expr::var("denom")),
            ));
            v.push(Node::assign(
                "val",
                Expr::fma(
                    Expr::var("w"),
                    Expr::load(
                        v_buf,
                        Expr::add(
                            Expr::add(
                                kv_base.clone(),
                                Expr::mul(Expr::var("j"), Expr::u32(head_dim)),
                            ),
                            col.clone(),
                        ),
                    ),
                    Expr::var("val"),
                ),
            ));
            v
        }));
        nodes.push(Node::Store {
            buffer: output.into(),
            index: flat.clone(),
            value: flush_tiny(Expr::var("val")),
        });
        nodes
    };

    let body = vec![
        Node::let_bind("flat", Expr::InvocationId { axis: 0 }),
        Node::if_then(
            Expr::lt(flat.clone(), Expr::u32(q_total)),
            vec![
                // `max_score` and `sum_exp` are hoisted to the if_then
                // body so they survive the per-pass `wrap_child(...)`
                // Region scopes (each Region creates a new variable
                // scope; an inner `Node::let_bind` would die on region
                // exit and downstream reads would be V001 undeclared).
                Node::let_bind("max_score", Expr::f32(f32::MIN)),
                Node::let_bind("sum_exp", Expr::f32(0.0)),
                wrap_child(ATTENTION_MAX_PASS_OP_ID, parent.clone(), max_pass),
                wrap_child(ATTENTION_SUM_PASS_OP_ID, parent.clone(), sum_pass),
                Node::let_bind("denom", positive_denominator(Expr::var("sum_exp"))),
                wrap_child(ATTENTION_WRITE_PASS_OP_ID, parent, write_pass),
            ],
        ),
    ];

    let kv_total = n_kv_heads * seq_len * head_dim;
    Ok(Program::wrapped(
        vec![
            BufferDecl::storage(q, 0, BufferAccess::ReadOnly, DataType::F32).with_count(q_total),
            BufferDecl::storage(k, 1, BufferAccess::ReadOnly, DataType::F32).with_count(kv_total),
            BufferDecl::storage(v_buf, 2, BufferAccess::ReadOnly, DataType::F32)
                .with_count(kv_total),
            BufferDecl::output(output, 3, DataType::F32).with_count(q_total),
        ],
        [64, 1, 1],
        vec![wrap_anonymous(OP_ID, body)],
    ))
}

inventory::submit! {
    crate::harness::OpEntry {
        id: OP_ID,
        build: || {
            gqa_attention("q", "k", "v", "out", 2, 1, 2, 2)
                .unwrap_or_else(|error| crate::invalid_program(OP_ID, format!("Fix: gqa_attention fixture must build: {error}")))
        },
        test_inputs: Some(|| {
            let f = vyre_primitives::wire::pack_f32_slice;
            vec![vec![
                f(&[1.0, 0.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0]),
                f(&[1.0, 0.0, 0.0, 1.0]),
                f(&[10.0, 20.0, 30.0, 40.0]),
                vec![0u8; 32],
            ]]
        }),
        expected_output: Some(|| {
            vec![vec![vec![
                145, 214, 132, 65, 146, 214, 212, 65, 111, 41, 187, 65, 183, 148, 5, 66, 111,
                41, 187, 65, 183, 148, 5, 66, 145, 214, 132, 65, 146, 214, 212, 65,
            ]]]
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
    fn gqa_attention_zero_sequence_length_rejected() {
        let err =
            gqa_attention("q", "k", "v", "out", 2, 1, 0, 4).expect_err("zero seq_len must error");
        assert!(err.contains("seq_len=0") || err.contains("non-zero"));
    }

    #[test]
    fn gqa_attention_single_token() {
        let n_q = 2u32;
        let n_kv = 1u32;
        let s = 1u32;
        let d = 2u32;
        let q = [1.0f32, 0.0, 0.0, 1.0];
        let k = [1.0f32, 0.0];
        let v = [10.0f32, 20.0];
        let prog = gqa_attention("q", "k", "v", "out", n_q, n_kv, s, d).expect("Fix: build");
        let outputs = vyre_reference::reference_eval(
            &prog,
            &[
                Value::from(f32_bytes(&q)),
                Value::from(f32_bytes(&k)),
                Value::from(f32_bytes(&v)),
                Value::from(vec![0u8; (n_q * s * d) as usize * 4]),
            ],
        )
        .expect("Fix: gqa_attention single token must execute");
        let out = decode_f32(&outputs[0].to_bytes());
        // With one token, softmax is [1.0], so output equals V broadcast.
        for (i, &v) in out.iter().enumerate() {
            let expected = if i % 2 == 0 { 10.0 } else { 20.0 };
            assert!(
                (v - expected).abs() <= 1.0e-4,
                "gqa_attention single token mismatch at {i}: {v} != {expected}"
            );
        }
    }

    #[test]
    fn gqa_attention_very_large_qk_values_stay_finite() {
        let n_q = 1u32;
        let n_kv = 1u32;
        let s = 2u32;
        let d = 2u32;
        let q = [1e20f32; 4];
        let k = [1e20f32; 4];
        let v = [1.0f32; 4];
        let prog = gqa_attention("q", "k", "v", "out", n_q, n_kv, s, d).expect("Fix: build");
        let outputs = vyre_reference::reference_eval(
            &prog,
            &[
                Value::from(f32_bytes(&q)),
                Value::from(f32_bytes(&k)),
                Value::from(f32_bytes(&v)),
                Value::from(vec![0u8; (n_q * s * d) as usize * 4]),
            ],
        )
        .expect("Fix: gqa_attention must not panic on large QK values");
        let out = decode_f32(&outputs[0].to_bytes());
        for (i, &v) in out.iter().enumerate() {
            assert!(
                v.is_finite(),
                "gqa_attention output at {i} must be finite for large QK values, got {v}"
            );
        }
    }

    #[test]
    fn gqa_attention_nan_in_q_k_v_propagates() {
        let n_q = 1u32;
        let n_kv = 1u32;
        let s = 1u32;
        let d = 2u32;
        let q = [f32::NAN, 0.0];
        let k = [0.0f32, 0.0];
        let v = [1.0f32, 2.0];
        let prog = gqa_attention("q", "k", "v", "out", n_q, n_kv, s, d).expect("Fix: build");
        let outputs = vyre_reference::reference_eval(
            &prog,
            &[
                Value::from(f32_bytes(&q)),
                Value::from(f32_bytes(&k)),
                Value::from(f32_bytes(&v)),
                Value::from(vec![0u8; (n_q * s * d) as usize * 4]),
            ],
        )
        .expect("Fix: gqa_attention must not panic on NaN input");
        let out = decode_f32(&outputs[0].to_bytes());
        assert!(
            out.iter().any(|v| v.is_nan()),
            "gqa_attention must propagate NaN in Q/K/V instead of silently producing finite output {:?}",
            out
        );
    }
}
