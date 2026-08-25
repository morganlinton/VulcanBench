//! QK-Gain: per-head learnable scalar applied to Q.
//!
//! `q_out[h, s, d] = q_in[h, s, d] * gain[h]`
//!
//! Category A  -  broadcast mul. Recipe uses gain_init=5.25.

use vyre::ir::{BinOp, BufferAccess, BufferDecl, DataType, Expr, Node, Program};

use crate::region::wrap_anonymous;

const OP_ID: &str = "vyre-libs::nn::qk_gain";

/// Build a Program that scales Q tensor by per-head F32 gain.
///
/// Layout: `q[num_heads * seq_len * head_dim]` flat (F32),
/// `gain[num_heads]` (F32).
#[must_use]
pub fn qk_gain(
    q_in: &str,
    q_out: &str,
    gain: &str,
    num_heads: u32,
    seq_len: u32,
    head_dim: u32,
) -> Program {
    let total = num_heads * seq_len * head_dim;
    let per_head = seq_len * head_dim;

    // Empty-tensor short circuit: a zero-sized total or per_head means
    // there is no work to do. Building the IR anyway would inject a
    // statically-zero divisor (V044) for `i / per_head`. Return a noop
    // Program that declares the buffers but writes nothing.
    if total == 0 || per_head == 0 {
        return Program::wrapped(
            vec![
                BufferDecl::storage(q_in, 0, BufferAccess::ReadOnly, DataType::F32)
                    .with_count(total),
                BufferDecl::output(q_out, 1, DataType::F32).with_count(total),
                BufferDecl::storage(gain, 2, BufferAccess::ReadOnly, DataType::F32)
                    .with_count(num_heads),
            ],
            [64, 1, 1],
            vec![wrap_anonymous(OP_ID, vec![])],
        );
    }

    let i = Expr::var("i");
    // head_idx = i / (seq_len * head_dim)
    let head_idx = Expr::BinOp {
        op: BinOp::Div,
        left: Box::new(i.clone()),
        right: Box::new(Expr::u32(per_head)),
    };
    let q_val = Expr::load(q_in, i.clone());
    let gain_val = Expr::load(gain, head_idx);
    let scaled = Expr::mul(q_val, gain_val);

    let body = vec![
        Node::let_bind("i", Expr::InvocationId { axis: 0 }),
        Node::if_then(
            Expr::lt(i.clone(), Expr::u32(total)),
            vec![Node::Store {
                buffer: q_out.into(),
                index: i,
                value: scaled,
            }],
        ),
    ];

    Program::wrapped(
        vec![
            BufferDecl::storage(q_in, 0, BufferAccess::ReadOnly, DataType::F32).with_count(total),
            BufferDecl::output(q_out, 1, DataType::F32).with_count(total),
            BufferDecl::storage(gain, 2, BufferAccess::ReadOnly, DataType::F32)
                .with_count(num_heads),
        ],
        [64, 1, 1],
        vec![wrap_anonymous(OP_ID, body)],
    )
}

inventory::submit! {
    crate::harness::OpEntry {
        id: OP_ID,
        build: || qk_gain("q_in", "q_out", "gain", 2, 1, 2),
        test_inputs: Some(|| {
            let to_f32 = |w: &[f32]| vyre_primitives::wire::pack_f32_slice(w);
            vec![vec![
                to_f32(&[1.0, 2.0, 3.0, 4.0]),  // q: 2 heads × 1 seq × 2 dim
                vec![0u8; 4 * 4],                 // q_out
                to_f32(&[5.25, 3.0]),              // gain per head
            ]]
        }),
        expected_output: Some(|| {
            let to_f32 = |w: &[f32]| vyre_primitives::wire::pack_f32_slice(w);
            // h0: [1*5.25, 2*5.25] = [5.25, 10.5]
            // h1: [3*3.0, 4*3.0] = [9.0, 12.0]
            vec![vec![to_f32(&[5.25, 10.5, 9.0, 12.0])]]
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
    fn qk_gain_nan_in_gain_propagates_nan() {
        let q = [1.0f32, 2.0, 3.0, 4.0];
        let gain = [f32::NAN, 1.0];
        let program = qk_gain("q_in", "q_out", "gain", 2, 1, 2);
        let outputs = vyre_reference::reference_eval(
            &program,
            &[
                Value::from(f32_bytes(&q)),
                Value::from(vec![0u8; 16]),
                Value::from(f32_bytes(&gain)),
            ],
        )
        .expect("Fix: qk_gain must not panic on NaN gain");
        let out = decode_f32(&outputs[0].to_bytes());
        assert!(
            out[0].is_nan(),
            "qk_gain NaN gain head-0 lane-0 must be NaN"
        );
        assert!(
            out[1].is_nan(),
            "qk_gain NaN gain head-0 lane-1 must be NaN"
        );
        assert_eq!(out[2], 3.0, "qk_gain finite gain head-1 lane-0 must be 3.0");
        assert_eq!(out[3], 4.0, "qk_gain finite gain head-1 lane-1 must be 4.0");
    }

    #[test]
    fn qk_gain_inf_in_gain() {
        let q = [1.0f32, 2.0];
        let gain = [f32::INFINITY];
        let program = qk_gain("q_in", "q_out", "gain", 1, 1, 2);
        let outputs = vyre_reference::reference_eval(
            &program,
            &[
                Value::from(f32_bytes(&q)),
                Value::from(vec![0u8; 8]),
                Value::from(f32_bytes(&gain)),
            ],
        )
        .expect("Fix: qk_gain must not panic on Inf gain");
        let out = decode_f32(&outputs[0].to_bytes());
        assert_eq!(out[0], f32::INFINITY, "qk_gain Inf gain must produce Inf");
        assert_eq!(out[1], f32::INFINITY, "qk_gain Inf gain must produce Inf");
    }

    #[test]
    fn qk_gain_zero_seq_len() {
        let q = [];
        let gain = [1.0f32];
        let program = qk_gain("q_in", "q_out", "gain", 1, 0, 2);
        let outputs = vyre_reference::reference_eval(
            &program,
            &[
                Value::from(f32_bytes(&q)),
                Value::from(vec![]),
                Value::from(f32_bytes(&gain)),
            ],
        )
        .expect("Fix: qk_gain seq_len=0 must not panic");
        assert!(outputs[0].to_bytes().is_empty());
    }

    #[test]
    fn qk_gain_single_element() {
        let q = [5.0f32];
        let gain = [2.0f32];
        let program = qk_gain("q_in", "q_out", "gain", 1, 1, 1);
        let outputs = vyre_reference::reference_eval(
            &program,
            &[
                Value::from(f32_bytes(&q)),
                Value::from(vec![0u8; 4]),
                Value::from(f32_bytes(&gain)),
            ],
        )
        .expect("Fix: qk_gain single element must execute");
        let out = decode_f32(&outputs[0].to_bytes());
        assert_eq!(out[0], 10.0, "qk_gain single element mismatch");
    }

    #[test]
    fn qk_gain_empty_tensor() {
        let program = qk_gain("q_in", "q_out", "gain", 1, 0, 2);
        let outputs = vyre_reference::reference_eval(
            &program,
            &[
                Value::from(vec![]),
                Value::from(vec![]),
                Value::from(f32_bytes(&[1.0])),
            ],
        )
        .expect("Fix: qk_gain total=0 must not panic");
        assert!(outputs[0].to_bytes().is_empty());
    }
}
