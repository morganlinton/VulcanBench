//! Welford's sum-of-squares algorithm.
//!
//! Category-A composition that emits a single-invocation loop to compute
//! the sum and sum-of-squares stably.

use crate::region::wrap_anonymous;
use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

const OP_ID: &str = "vyre-libs::math::welford_sum_of_squares";
const EMPTY_REDUCTION_FIX: &str = "Fix: welford_sum_of_squares n=0 is invalid; pass at least one input element or route empty reductions to a caller-defined identity.";

/// Build a Program that computes the sum and sum of squared deviations
/// of `input` using Welford's algorithm.
///
/// Output is written to `sum_out[0]` and `sum_sq_out[0]`.
#[must_use]
pub fn welford_sum_of_squares(input: &str, sum_out: &str, sum_sq_out: &str, n: u32) -> Program {
    if n == 0 {
        return welford_invalid_program(input, sum_out, sum_sq_out);
    }

    // Welford's online algorithm for computing mean and M2:
    // mean = 0.0
    // m2 = 0.0
    // for i in 0..n {
    //     let count = (i + 1) as f32
    //     let x = input[i]
    //     let delta = x - mean
    //     mean = mean + delta / count
    //     let delta2 = x - mean
    //     m2 = m2 + delta * delta2
    // }
    // sum_out[0] = mean * n
    // sum_sq_out[0] = m2

    let body = vec![
        Node::let_bind("mean", Expr::f32(0.0)),
        Node::let_bind("m2", Expr::f32(0.0)),
        Node::loop_for(
            "i",
            Expr::u32(0),
            Expr::u32(n),
            vec![
                Node::let_bind(
                    "count",
                    Expr::cast(DataType::F32, Expr::add(Expr::var("i"), Expr::u32(1))),
                ),
                Node::let_bind("x", Expr::load(input, Expr::var("i"))),
                Node::let_bind("delta", Expr::sub(Expr::var("x"), Expr::var("mean"))),
                Node::assign(
                    "mean",
                    Expr::add(
                        Expr::var("mean"),
                        Expr::div(Expr::var("delta"), Expr::var("count")),
                    ),
                ),
                Node::let_bind("delta2", Expr::sub(Expr::var("x"), Expr::var("mean"))),
                Node::assign(
                    "m2",
                    Expr::add(
                        Expr::var("m2"),
                        Expr::mul(Expr::var("delta"), Expr::var("delta2")),
                    ),
                ),
            ],
        ),
        Node::Store {
            buffer: sum_out.into(),
            index: Expr::u32(0),
            value: Expr::mul(Expr::var("mean"), Expr::f32(n as f32)),
        },
        Node::Store {
            buffer: sum_sq_out.into(),
            index: Expr::u32(0),
            value: Expr::var("m2"),
        },
    ];

    Program::wrapped(
        vec![
            BufferDecl::storage(input, 0, BufferAccess::ReadOnly, DataType::F32).with_count(n),
            BufferDecl::output(sum_out, 1, DataType::F32).with_count(1),
            BufferDecl::read_write(sum_sq_out, 2, DataType::F32).with_count(1),
        ],
        [1, 1, 1], // Single invocation
        vec![wrap_anonymous(OP_ID, body)],
    )
}

fn welford_invalid_program(input: &str, sum_out: &str, sum_sq_out: &str) -> Program {
    Program::wrapped(
        vec![
            BufferDecl::storage(input, 0, BufferAccess::ReadOnly, DataType::F32).with_count(1),
            BufferDecl::output(sum_out, 1, DataType::F32).with_count(1),
            BufferDecl::read_write(sum_sq_out, 2, DataType::F32).with_count(1),
        ],
        [1, 1, 1],
        vec![wrap_anonymous(
            OP_ID,
            vec![Node::trap(Expr::u32(0), EMPTY_REDUCTION_FIX)],
        )],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::byte_pack::decode_f32_one as decode_one;
    use crate::test_support::byte_pack::f32_bytes;
    use vyre_reference::value::Value;

    #[test]
    fn welford_small_dataset() {
        let input = vec![10.0, 12.0, 23.0, 23.0, 16.0, 23.0, 21.0, 16.0];
        let n = input.len() as u32;
        let program = welford_sum_of_squares("input", "sum", "sum_sq", n);

        let outputs = vyre_reference::reference_eval(
            &program,
            &[
                Value::from(f32_bytes(&input)),
                Value::from(vec![0u8; 4]),
                Value::from(vec![0u8; 4]),
            ],
        )
        .expect("Fix: execution failed");

        let sum = decode_one(&outputs[0].to_bytes());
        let sum_sq = decode_one(&outputs[1].to_bytes());

        let expected_mean = 18.0;
        let expected_sum = expected_mean * 8.0; // 144.0
        let expected_m2 = 192.0; // Sum of (x - mean)^2

        assert!(
            (sum - expected_sum).abs() < 1e-4,
            "Expected sum {}, got {}",
            expected_sum,
            sum
        );
        assert!(
            (sum_sq - expected_m2).abs() < 1e-4,
            "Expected M2 {}, got {}",
            expected_m2,
            sum_sq
        );
    }

    #[test]
    fn welford_length_one() {
        let input = vec![42.0];
        let program = welford_sum_of_squares("input", "sum", "sum_sq", 1);

        let outputs = vyre_reference::reference_eval(
            &program,
            &[
                Value::from(f32_bytes(&input)),
                Value::from(vec![0u8; 4]),
                Value::from(vec![0u8; 4]),
            ],
        )
        .expect("Fix: execution failed");

        let sum = decode_one(&outputs[0].to_bytes());
        let sum_sq = decode_one(&outputs[1].to_bytes());

        assert!((sum - 42.0).abs() < 1e-4);
        assert!((sum_sq - 0.0).abs() < 1e-4);
    }

    #[test]
    fn welford_empty_rejected() {
        let program = welford_sum_of_squares("input", "sum", "sum_sq", 0);
        let err = vyre_reference::reference_eval(
            &program,
            &[
                Value::from(vec![0u8; 4]),
                Value::from(vec![0u8; 4]),
                Value::from(vec![0u8; 4]),
            ],
        )
        .expect_err("should trap");

        assert!(err.to_string().contains(EMPTY_REDUCTION_FIX));
    }
}
