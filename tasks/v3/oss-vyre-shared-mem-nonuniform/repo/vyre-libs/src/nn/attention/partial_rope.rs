//! Partial RoPE: rotary position embedding on first `rope_dims` of
//! each head, identity on the rest.
//!
//! Category A composition. Recipe rotates first 16 of 64 head dims.
//! Standard RoPE: `[x1*cos - x2*sin, x1*sin + x2*cos]` on pairs.

use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

use crate::region::wrap_anonymous;

const OP_ID: &str = "vyre-libs::nn::partial_rope";

/// Build a Program applying partial RoPE (F32).
///
/// Applies RoPE to the first `rope_dims` channels of each head and
/// copies the remaining channels unchanged.
#[must_use]
pub fn partial_rope(
    input: &str,
    cos_table: &str,
    sin_table: &str,
    output: &str,
    num_heads: u32,
    seq_len: u32,
    head_dim: u32,
    rope_dims: u32,
) -> Program {
    if num_heads == 0 || seq_len == 0 || head_dim == 0 {
        return crate::builder::invalid_output_program(
            OP_ID,
            output,
            DataType::F32,
            format!(
                "Fix: partial_rope requires positive num_heads, seq_len, and head_dim; got num_heads={num_heads}, seq_len={seq_len}, head_dim={head_dim}."
            ),
        );
    }
    if rope_dims > head_dim || rope_dims % 2 != 0 {
        return crate::builder::invalid_output_program(
            OP_ID,
            output,
            DataType::F32,
            format!(
                "Fix: partial_rope requires an even rope_dims <= head_dim; got rope_dims={rope_dims}, head_dim={head_dim}."
            ),
        );
    }
    let total = match num_heads
        .checked_mul(seq_len)
        .and_then(|value| value.checked_mul(head_dim))
    {
        Some(total) => total,
        None => {
            return crate::builder::invalid_output_program(
                OP_ID,
                output,
                DataType::F32,
                format!(
                    "Fix: partial_rope total element count overflows u32 for num_heads={num_heads}, seq_len={seq_len}, head_dim={head_dim}."
                ),
            );
        }
    };
    let half_rope = rope_dims / 2;
    let table_count = match seq_len.checked_mul(half_rope) {
        Some(count) => count,
        None => {
            return crate::builder::invalid_output_program(
                OP_ID,
                output,
                DataType::F32,
                format!(
                    "Fix: partial_rope table element count overflows u32 for seq_len={seq_len}, rope_dims={rope_dims}."
                ),
            );
        }
    };

    let i = Expr::var("i");
    let dim = Expr::rem(i.clone(), Expr::u32(head_dim));
    let token = Expr::rem(
        Expr::div(i.clone(), Expr::u32(head_dim)),
        Expr::u32(seq_len),
    );
    let pair = Expr::div(dim.clone(), Expr::u32(2));
    let parity = Expr::rem(dim.clone(), Expr::u32(2));
    let pair_base = Expr::sub(i.clone(), parity.clone());
    let x0 = Expr::load(input, pair_base.clone());
    let x1 = Expr::load(input, Expr::add(pair_base, Expr::u32(1)));
    let table_idx = Expr::add(Expr::mul(token, Expr::u32(half_rope)), pair);
    let cos_v = Expr::load(cos_table, table_idx.clone());
    let sin_v = Expr::load(sin_table, table_idx);
    let rotated_even = Expr::sub(
        Expr::mul(x0.clone(), cos_v.clone()),
        Expr::mul(x1.clone(), sin_v.clone()),
    );
    let rotated_odd = Expr::add(Expr::mul(x0, sin_v), Expr::mul(x1, cos_v));
    let rotated = Expr::select(Expr::eq(parity, Expr::u32(0)), rotated_even, rotated_odd);
    let value = Expr::select(
        Expr::lt(dim, Expr::u32(rope_dims)),
        rotated,
        Expr::load(input, i.clone()),
    );

    let body = vec![
        Node::let_bind("i", Expr::InvocationId { axis: 0 }),
        Node::if_then(
            Expr::lt(i.clone(), Expr::u32(total)),
            vec![Node::Store {
                buffer: output.into(),
                index: i,
                value,
            }],
        ),
    ];

    Program::wrapped(
        vec![
            BufferDecl::storage(input, 0, BufferAccess::ReadOnly, DataType::F32).with_count(total),
            BufferDecl::storage(cos_table, 1, BufferAccess::ReadOnly, DataType::F32)
                .with_count(table_count),
            BufferDecl::storage(sin_table, 2, BufferAccess::ReadOnly, DataType::F32)
                .with_count(table_count),
            BufferDecl::output(output, 3, DataType::F32).with_count(total),
        ],
        [64, 1, 1],
        vec![wrap_anonymous(OP_ID, body)],
    )
}

inventory::submit! {
    crate::harness::OpEntry {
        id: OP_ID,
        build: || partial_rope("input", "cos", "sin", "output", 1, 2, 4, 2),
        test_inputs: Some(|| {
            let to_f32 = |w: &[f32]| vyre_primitives::wire::pack_f32_slice(w);
            vec![vec![
                to_f32(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]), // input
                to_f32(&[1.0, 1.0]),  // cos table
                to_f32(&[0.0, 0.0]),  // sin table
                vec![0u8; 4 * 8],     // output
            ]]
        }),
        expected_output: Some(|| {
            let to_f32 = |w: &[f32]| vyre_primitives::wire::pack_f32_slice(w);
            vec![vec![to_f32(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0])]]
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
    fn rejects_invalid_rope_dims_without_panicking() {
        let p = partial_rope("input", "cos", "sin", "output", 1, 2, 4, 3);
        assert!(p.stats().trap());
    }

    #[test]
    fn rejects_zero_shape_without_panicking() {
        let p = partial_rope("input", "cos", "sin", "output", 0, 2, 4, 2);
        assert!(p.stats().trap());
    }

    #[test]
    fn rejects_overflow_shape_without_panicking() {
        let p = partial_rope("input", "cos", "sin", "output", u32::MAX, 2, 4, 2);
        assert!(p.stats().trap());
    }

    #[test]
    fn partial_rope_nan_in_input_propagates_nan() {
        let input = [f32::NAN, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        let cos = [1.0f32, 1.0];
        let sin = [0.0f32, 0.0];
        let program = partial_rope("input", "cos", "sin", "output", 1, 2, 4, 2);
        let outputs = vyre_reference::reference_eval(
            &program,
            &[
                Value::from(f32_bytes(&input)),
                Value::from(f32_bytes(&cos)),
                Value::from(f32_bytes(&sin)),
                Value::from(vec![0u8; 32]),
            ],
        )
        .expect("Fix: partial_rope must not panic on NaN input");
        let out = decode_f32(&outputs[0].to_bytes());
        assert!(
            out[0].is_nan(),
            "partial_rope must propagate NaN from input"
        );
        // RoPE pairs lanes (0,1): out[0] = in[0]*cos - in[1]*sin and
        // out[1] = in[0]*sin + in[1]*cos. With NaN at in[0] and sin=0,
        // out[1] computes `NaN*0 + 2*1` which is NaN under IEEE 754
        // (any arithmetic involving NaN returns NaN, including NaN*0).
        // Asserting out[1] == 2.0 would require a non-IEEE shortcut.
        assert!(
            out[1].is_nan(),
            "partial_rope NaN at in[0] poisons the paired lane via NaN*0 = NaN per IEEE 754, got {}",
            out[1]
        );
        // Lanes outside the rotated pair must NOT be poisoned.
        assert_eq!(out[2], 3.0, "partial_rope leaves unrotated lanes untouched");
        assert_eq!(out[3], 4.0, "partial_rope leaves unrotated lanes untouched");
    }

    #[test]
    fn partial_rope_zero_sequence_length_rejected() {
        let p = partial_rope("input", "cos", "sin", "output", 1, 0, 4, 2);
        assert!(p.stats().trap(), "partial_rope seq_len=0 must trap");
    }

    #[test]
    fn partial_rope_single_token() {
        let input = [1.0f32, 2.0, 3.0, 4.0];
        let cos = [1.0f32, 1.0];
        let sin = [0.0f32, 0.0];
        let program = partial_rope("input", "cos", "sin", "output", 1, 1, 4, 2);
        let outputs = vyre_reference::reference_eval(
            &program,
            &[
                Value::from(f32_bytes(&input)),
                Value::from(f32_bytes(&cos)),
                Value::from(f32_bytes(&sin)),
                Value::from(vec![0u8; 16]),
            ],
        )
        .expect("Fix: partial_rope single token must execute");
        let out = decode_f32(&outputs[0].to_bytes());
        // With sin=0, cos=1, RoPE is identity on pairs.
        assert_eq!(out, vec![1.0, 2.0, 3.0, 4.0]);
    }

    #[test]
    fn partial_rope_nan_in_cos_sin_tables_propagates_nan() {
        let input = [1.0f32, 2.0, 3.0, 4.0];
        let cos = [f32::NAN, 1.0];
        let sin = [0.0f32, 0.0];
        let program = partial_rope("input", "cos", "sin", "output", 1, 1, 4, 2);
        let outputs = vyre_reference::reference_eval(
            &program,
            &[
                Value::from(f32_bytes(&input)),
                Value::from(f32_bytes(&cos)),
                Value::from(f32_bytes(&sin)),
                Value::from(vec![0u8; 16]),
            ],
        )
        .expect("Fix: partial_rope must not panic on NaN cos table");
        let out = decode_f32(&outputs[0].to_bytes());
        assert!(
            out[0].is_nan() || out[1].is_nan(),
            "partial_rope NaN in cos table must propagate to rotated pair"
        );
    }
}
