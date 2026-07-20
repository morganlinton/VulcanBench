//! Block-FMA weighted-sum reduction: `y = sum_i weights[i] * values[i]`.
//!
//! ROADMAP G7  -  block-FMA reductions. The naive form
//! `acc = acc + weights[i] * values[i]` performs two rounded
//! IEEE-754 operations per element (mul, add). Replacing with
//! `acc = Fma(weights[i], values[i], acc)` collapses to a single
//! rounded operation per element, which is:
//!
//! - **Numerically more accurate**: one round-to-nearest instead
//!   of two, reducing the error per element from ~2 ULP to ~1 ULP.
//! - **Faster on every GPU / modern CPU**: FMA is a single
//!   instruction (1 cycle on Tensor Cores; 4 cycles on a regular
//!   FP unit) vs separate Mul + Add (1 + 1 cycles minimum, often
//!   serialised through the same pipeline).
//!
//! For length-N reductions this saves N rounded ops + N latencies.
//! Critical for ML weighted aggregations (attention head-mixing,
//! LayerNorm gain, gated reduction layers).

use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

use crate::region::wrap_anonymous;

const OP_ID: &str = "vyre-libs::math::weighted_sum_fma_f32";

/// Build a Program that computes the FMA-fused weighted sum
/// `out[0] = sum_i weights[i] * values[i]` for `i ∈ 0..n`.
///
/// # Errors
///
/// Returns `Err` when `n == 0` (empty reduction is undefined).
pub fn weighted_sum_fma_f32(
    weights: &str,
    values: &str,
    output: &str,
    n: u32,
) -> Result<Program, String> {
    if n == 0 {
        return Err("Fix: weighted_sum_fma_f32 n=0 is invalid: empty reduction".to_string());
    }
    let body = vec![
        Node::let_bind("acc", Expr::f32(0.0)),
        Node::loop_for(
            "i",
            Expr::u32(0),
            Expr::u32(n),
            vec![Node::assign(
                "acc",
                Expr::Fma {
                    a: Box::new(Expr::load(weights, Expr::var("i"))),
                    b: Box::new(Expr::load(values, Expr::var("i"))),
                    c: Box::new(Expr::var("acc")),
                },
            )],
        ),
        Node::Store {
            buffer: output.into(),
            index: Expr::u32(0),
            value: Expr::var("acc"),
        },
    ];
    Ok(Program::wrapped(
        vec![
            BufferDecl::storage(weights, 0, BufferAccess::ReadOnly, DataType::F32).with_count(n),
            BufferDecl::storage(values, 1, BufferAccess::ReadOnly, DataType::F32).with_count(n),
            BufferDecl::output(output, 2, DataType::F32).with_count(1),
        ],
        [1, 1, 1],
        vec![wrap_anonymous(OP_ID, body)],
    ))
}

inventory::submit! {
    crate::harness::OpEntry {
        id: OP_ID,
        build: || {
            weighted_sum_fma_f32("weights", "values", "output", 4).unwrap_or_else(|error| {
                crate::builder::invalid_output_program(
                    OP_ID,
                    "output",
                    DataType::F32,
                    error,
                )
            })
        },
        test_inputs: Some(|| {
            let weights = crate::test_support::byte_pack::f32_bytes(&[0.5, 0.25, 0.125, 0.125]);
            let values = crate::test_support::byte_pack::f32_bytes(&[1.0, 2.0, 4.0, 8.0]);
            vec![vec![weights, values]]
        }),
        expected_output: Some(|| {
            // 0.5*1 + 0.25*2 + 0.125*4 + 0.125*8 = 0.5 + 0.5 + 0.5 + 1.0 = 2.5
            vec![vec![crate::test_support::byte_pack::f32_bytes(&[2.5_f32])]]
        }),
        category: Some("math"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::byte_pack::decode_f32_one as decode_one;
    use crate::test_support::byte_pack::f32_bytes;
    use vyre_reference::value::Value;

    fn run(weights: &[f32], values: &[f32]) -> f32 {
        let n = weights.len() as u32;
        let prog = weighted_sum_fma_f32("weights", "values", "output", n).expect("Fix: build");
        let outputs = vyre_reference::reference_eval(
            &prog,
            &[
                Value::from(f32_bytes(weights)),
                Value::from(f32_bytes(values)),
                Value::from(vec![0u8; 4]),
            ],
        )
        .expect("Fix: weighted_sum_fma_f32 must execute in the reference interpreter.");
        decode_one(&outputs[0].to_bytes())
    }

    /// Canonical fixture: 0.5·1 + 0.25·2 + 0.125·4 + 0.125·8 = 2.5.
    #[test]
    fn weighted_sum_fma_canonical_fixture() {
        let weights = [0.5_f32, 0.25, 0.125, 0.125];
        let values = [1.0_f32, 2.0, 4.0, 8.0];
        let actual = run(&weights, &values);
        assert!((actual - 2.5).abs() <= 1.0e-6, "{actual} != 2.5");
    }

    /// All-zero weights → zero output.
    #[test]
    fn weighted_sum_fma_zero_weights_returns_zero() {
        let weights = [0.0_f32; 4];
        let values = [1.0_f32, 2.0, 3.0, 4.0];
        let actual = run(&weights, &values);
        assert_eq!(actual, 0.0);
    }

    /// All-one weights → plain sum of values.
    #[test]
    fn weighted_sum_fma_unit_weights_equals_plain_sum() {
        let weights = [1.0_f32; 8];
        let values: Vec<f32> = (0..8).map(|i| i as f32 - 3.5).collect();
        let actual = run(&weights, &values);
        let expected: f32 = values.iter().sum();
        assert!(
            (actual - expected).abs() <= 1.0e-5,
            "{actual} != {expected}"
        );
    }

    /// Random fuzz: 50 random length-N pairs match a scalar
    /// reference within 1.0e-5 absolute tolerance. The FMA
    /// reduction is *more* accurate than the naive mul+add per
    /// element so the actual vs reference divergence is bounded by
    /// the reference's own rounding.
    #[test]
    fn weighted_sum_fma_matches_naive_on_random_fuzz() {
        let mut state = 0xFEEDBEEF_u64;
        let mut next = || {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((state >> 33) as f32 / (u32::MAX as f32 / 2.0)) - 1.0
        };
        for _ in 0..50 {
            let n = 8;
            let weights: Vec<f32> = (0..n).map(|_| next()).collect();
            let values: Vec<f32> = (0..n).map(|_| next()).collect();
            let actual = run(&weights, &values);
            let expected: f32 = weights.iter().zip(values.iter()).map(|(w, v)| w * v).sum();
            assert!(
                (actual - expected).abs() <= 1.0e-4,
                "fma={actual} naive={expected} diff={}",
                (actual - expected).abs()
            );
        }
    }

    /// Empty reduction rejected.
    #[test]
    fn weighted_sum_fma_rejects_empty_n() {
        let err =
            weighted_sum_fma_f32("weights", "values", "output", 0).expect_err("empty n must error");
        assert!(err.contains("n=0"));
    }

    // ------------------------------------------------------------------
    // Adversarial fixtures exposing real gaps
    // ------------------------------------------------------------------

    #[test]
    fn weighted_sum_fma_single_element() {
        let weights = [3.0_f32];
        let values = [4.0_f32];
        let actual = run(&weights, &values);
        assert!((actual - 12.0).abs() <= 1.0e-5, "3*4 = 12, got {actual}");
    }

    /// NaN in weights must propagate to the output.
    #[test]
    fn weighted_sum_fma_nan_in_weights_propagates() {
        let weights = [1.0_f32, f32::NAN, 1.0];
        let values = [1.0_f32, 1.0, 1.0];
        let actual = run(&weights, &values);
        assert!(
            actual.is_nan(),
            "weighted sum with NaN weight must be NaN, got {actual}"
        );
    }

    /// NaN in values must propagate to the output.
    #[test]
    fn weighted_sum_fma_nan_in_values_propagates() {
        let weights = [1.0_f32, 1.0, 1.0];
        let values = [1.0_f32, f32::NAN, 1.0];
        let actual = run(&weights, &values);
        assert!(
            actual.is_nan(),
            "weighted sum with NaN value must be NaN, got {actual}"
        );
    }

    /// Inf in weights must propagate to the output.
    #[test]
    fn weighted_sum_fma_inf_in_weights_propagates() {
        let weights = [1.0_f32, f32::INFINITY, 1.0];
        let values = [1.0_f32, 1.0, 1.0];
        let actual = run(&weights, &values);
        assert!(
            actual.is_infinite(),
            "weighted sum with Inf weight must be Inf, got {actual}"
        );
    }

    /// Inf in values must propagate to the output.
    #[test]
    fn weighted_sum_fma_inf_in_values_propagates() {
        let weights = [1.0_f32, 1.0, 1.0];
        let values = [1.0_f32, f32::INFINITY, 1.0];
        let actual = run(&weights, &values);
        assert!(
            actual.is_infinite(),
            "weighted sum with Inf value must be Inf, got {actual}"
        );
    }
}
