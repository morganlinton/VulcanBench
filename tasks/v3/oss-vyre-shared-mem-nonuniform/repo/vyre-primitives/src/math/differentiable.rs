//! Differentiable algorithm primitives  -  smoothed argmax + softmax.
//!
//! The breakthrough wasn't auto-diff  -  it was realizing that **classical
//! algorithms become NN building blocks once you smooth their argmaxes**
//! (Mensch-Blondel 2018, Berthet 2020, Petersen 2022). Every smoothed
//! classical algorithm is a primitive that participates in end-to-end
//! gradient flow.
//!
//! This file ships:
//! - [`crate::math::differentiable::softmax_step`]  -  `out[i] = exp(x[i] - max) / Σ exp(x[j] - max)`
//!   numerically stable, fixed-point.
//! - `differentiable_argmax`  -  uses softmax with high temperature
//!   for sharp argmax-like behavior. Forward and gradient agree on
//!   the soft assignment.
//!
//! # Why this primitive is dual-use
//!
//! | Consumer | Use |
//! |---|---|
//! | `vyre-libs::ml::transformers` consumers | attention softmax |
//! | `vyre-libs::ml::structured` consumers | differentiable beam search, top-k, sorting |
//! | `vyre-libs::parsing::c::sema` consumers | identifier→typedef gradient flow |
//! | `vyre-driver` autotuner consumers | smoothed argmax over the workgroup-size / tile-shape / fusion-threshold tuning grid; gradient descent picks the configuration |
//!
//! # Fixed-point convention
//!
//! u32 16.16 fixed-point with `EXP_TABLE_BITS` mantissa for the inner
//! exp lookup. The exp evaluator is a small Taylor expansion at u32
//! precision; for higher precision, callers compose this primitive
//! with their own f32-based exp evaluator and supply the pre-exp'd
//! values.
//!
//! Separate registered ops own differentiable sorting and top-k
//! selection; this module's contract is softmax plus differentiable
//! argmax.

use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Op id.
pub const OP_ID: &str = "vyre-primitives::math::softmax_step";

/// Emit a numerically-stable softmax Program over a length-`n` vector.
///
/// Inputs:
/// - `x`: length-n u32 buffer (16.16 fixed-point logits, post-temperature scaling).
/// - `pre_exp`: caller-supplied length-n u32 buffer of `exp(x[i] - max)`
///   in 16.16 fixed-point. This file does not embed an exp evaluator
///   on GPU  -  the caller composes a separate exp Program (or uses a
///   precomputed lookup) before calling this.
///
/// Output:
/// - `out`: length-n u32 buffer of softmax probabilities (16.16).
///
/// Single-lane normalization (lane 0 walks the array, sums, then
/// writes normalized values back). For large n compose with
/// `reduce::sum` first to get the partition function, then a separate
/// elementwise divide (which may compose multiple of this primitive's
/// inner divides into one parallel pass).
#[must_use]
pub fn softmax_step(pre_exp: &str, out: &str, n: u32) -> Program {
    if n == 0 {
        return crate::invalid_output_program(
            OP_ID,
            out,
            DataType::U32,
            format!("Fix: softmax_step requires n > 0, got {n}."),
        );
    }

    let t = Expr::InvocationId { axis: 0 };
    let body = vec![Node::if_then(
        Expr::eq(t.clone(), Expr::u32(0)),
        vec![
            // sum = Σ pre_exp[i]
            Node::let_bind("sum", Expr::u32(0)),
            Node::loop_for(
                "i",
                Expr::u32(0),
                Expr::u32(n),
                vec![Node::assign(
                    "sum",
                    Expr::add(Expr::var("sum"), Expr::load(pre_exp, Expr::var("i"))),
                )],
            ),
            // sum_safe = max(sum, 1)
            Node::let_bind(
                "sum_safe",
                Expr::select(
                    Expr::eq(Expr::var("sum"), Expr::u32(0)),
                    Expr::u32(1),
                    Expr::var("sum"),
                ),
            ),
            // out[i] = (pre_exp[i] << 16) / sum_safe  (preserve 16.16
            // by shifting the numerator before dividing).
            Node::loop_for(
                "j",
                Expr::u32(0),
                Expr::u32(n),
                vec![Node::store(
                    out,
                    Expr::var("j"),
                    Expr::div(
                        Expr::shl(Expr::load(pre_exp, Expr::var("j")), Expr::u32(16)),
                        Expr::var("sum_safe"),
                    ),
                )],
            ),
        ],
    )];

    Program::wrapped(
        vec![
            BufferDecl::storage(pre_exp, 0, BufferAccess::ReadOnly, DataType::U32).with_count(n),
            BufferDecl::storage(out, 1, BufferAccess::ReadWrite, DataType::U32).with_count(n),
        ],
        [256, 1, 1],
        vec![Node::Region {
            generator: Ident::from(OP_ID),
            source_region: None,
            body: Arc::new(body),
        }],
    )
}

/// CPU reference: softmax in f64 for clarity.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn softmax_cpu(x: &[f64]) -> Vec<f64> {
    let mut out = Vec::new();
    try_softmax_cpu_into(x, &mut out).unwrap_or_else(|error| panic!("{error}"));
    out
}

/// CPU reference: softmax in f64 using caller-owned output.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn softmax_cpu_into(x: &[f64], out: &mut Vec<f64>) {
    try_softmax_cpu_into(x, out).unwrap_or_else(|error| panic!("{error}"));
}

/// Fallible CPU reference: softmax in f64 using caller-owned output.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_softmax_cpu_into(x: &[f64], out: &mut Vec<f64>) -> Result<(), String> {
    if x.len() > out.capacity() {
        crate::graph::scratch::reserve_graph_items(
            out,
            x.len() - out.len(),
            "differentiable math CPU oracle",
            "softmax_cpu output",
        )?;
    }
    out.clear();
    if x.is_empty() {
        return Ok(());
    }
    let max = x.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let mut sum = 0.0;
    for &value in x {
        let exp = (value - max).exp();
        sum += exp;
        out.push(exp);
    }
    for value in out.iter_mut() {
        *value /= sum;
    }
    Ok(())
}

/// CPU reference: differentiable argmax via temperature-scaled softmax.
/// Higher `temperature` → softer assignment; `temperature → 0+`
/// recovers hard argmax. Returns the soft-assignment vector that sums
/// to 1 (probability over indices).
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn differentiable_argmax_cpu(x: &[f64], temperature: f64) -> Vec<f64> {
    let mut scaled = Vec::new();
    let mut out = Vec::new();
    try_differentiable_argmax_cpu_into(x, temperature, &mut scaled, &mut out)
        .unwrap_or_else(|error| panic!("{error}"));
    out
}

/// CPU reference: differentiable argmax using caller-owned scratch and output.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn differentiable_argmax_cpu_into(
    x: &[f64],
    temperature: f64,
    scaled: &mut Vec<f64>,
    out: &mut Vec<f64>,
) {
    try_differentiable_argmax_cpu_into(x, temperature, scaled, out)
        .unwrap_or_else(|error| panic!("{error}"));
}

/// Fallible CPU reference: differentiable argmax using caller-owned scratch and output.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_differentiable_argmax_cpu_into(
    x: &[f64],
    temperature: f64,
    scaled: &mut Vec<f64>,
    out: &mut Vec<f64>,
) -> Result<(), String> {
    if x.len() > scaled.capacity() {
        crate::graph::scratch::reserve_graph_items(
            scaled,
            x.len() - scaled.len(),
            "differentiable math CPU oracle",
            "differentiable_argmax_cpu scaled logits",
        )?;
    }
    scaled.clear();
    if temperature <= 0.0 || !temperature.is_finite() {
        out.clear();
        return Ok(());
    }
    scaled.extend(x.iter().map(|&v| v / temperature));
    try_softmax_cpu_into(scaled, out)
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        OP_ID,
        || {
            softmax_step("pre_exp", "out", 4)
        },
        Some(|| {
            vec![vec![
                crate::wire::pack_u32_slice(&[1; 4]),
                crate::wire::pack_u32_slice(&[0; 4]),
            ]]
        }),
        Some(|| {
            vec![vec![crate::wire::pack_u32_slice(&[16_384; 4])]]
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-10 * (1.0 + a.abs() + b.abs())
    }

    #[test]
    fn cpu_softmax_uniform_input_is_uniform_output() {
        let x = vec![1.0, 1.0, 1.0, 1.0];
        let out = softmax_cpu(&x);
        for v in out {
            assert!(approx_eq(v, 0.25));
        }
    }

    #[test]
    fn cpu_softmax_sums_to_one() {
        let x = vec![0.5, 1.0, 1.5, 2.0, 2.5];
        let out = softmax_cpu(&x);
        let s: f64 = out.iter().sum();
        assert!(approx_eq(s, 1.0));
    }

    #[test]
    fn cpu_softmax_monotone_in_input() {
        let x = vec![0.0, 1.0, 2.0];
        let out = softmax_cpu(&x);
        assert!(out[0] < out[1]);
        assert!(out[1] < out[2]);
    }

    #[test]
    fn cpu_softmax_handles_large_inputs_no_overflow() {
        // Without max-subtraction, exp(1000) would overflow.
        let x = vec![1000.0, 1000.0, 1000.0];
        let out = softmax_cpu(&x);
        for v in out {
            assert!(v.is_finite());
            assert!(approx_eq(v, 1.0 / 3.0));
        }
    }

    #[test]
    fn cpu_softmax_into_reuses_output_and_truncates_stale_tail() {
        let mut out = Vec::with_capacity(4);
        out.extend_from_slice(&[99.0, 98.0, 97.0, 96.0]);
        let capacity = out.capacity();

        try_softmax_cpu_into(&[1.0, 1.0], &mut out)
            .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - softmax CPU oracle should reuse caller-owned output");

        assert_eq!(out.len(), 2);
        assert!(approx_eq(out[0], 0.5));
        assert!(approx_eq(out[1], 0.5));
        assert_eq!(out.capacity(), capacity);

        try_softmax_cpu_into(&[1.0], &mut out)
            .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - softmax CPU oracle should truncate stale output");

        assert_eq!(out, vec![1.0]);
        assert_eq!(out.capacity(), capacity);
    }

    #[test]
    fn cpu_diff_argmax_low_temp_concentrates() {
        // Low temperature → soft argmax should concentrate on the max.
        let x = vec![1.0, 5.0, 2.0];
        let probs = differentiable_argmax_cpu(&x, 0.001);
        // probs[1] should be very close to 1.
        assert!(probs[1] > 0.99);
        assert!(probs[0] < 0.01);
        assert!(probs[2] < 0.01);
    }

    #[test]
    fn cpu_diff_argmax_high_temp_uniform() {
        // High temperature → soft argmax should be near-uniform.
        let x = vec![1.0, 5.0, 2.0];
        let probs = differentiable_argmax_cpu(&x, 1000.0);
        for v in probs {
            assert!((v - 1.0 / 3.0).abs() < 0.01);
        }
    }

    #[test]
    fn cpu_diff_argmax_sums_to_one() {
        let x = vec![0.5, 1.0, 1.5, 2.0];
        let probs = differentiable_argmax_cpu(&x, 1.0);
        let s: f64 = probs.iter().sum();
        assert!(approx_eq(s, 1.0));
    }

    #[test]
    fn cpu_diff_argmax_into_reuses_buffers() {
        let x = vec![1.0, 5.0, 2.0];
        let mut scaled = Vec::with_capacity(8);
        let mut out = Vec::with_capacity(8);
        scaled.extend_from_slice(&[99.0, 98.0, 97.0, 96.0]);
        out.extend_from_slice(&[89.0, 88.0, 87.0, 86.0]);
        let scaled_ptr = scaled.as_ptr();
        let out_ptr = out.as_ptr();
        differentiable_argmax_cpu_into(&x, 1000.0, &mut scaled, &mut out);
        assert_eq!(scaled.as_ptr(), scaled_ptr);
        assert_eq!(out.as_ptr(), out_ptr);
        assert_eq!(scaled.len(), x.len());
        assert_eq!(out.len(), x.len());
        let s: f64 = out.iter().sum();
        assert!(approx_eq(s, 1.0));

        differentiable_argmax_cpu_into(&x[..1], 1000.0, &mut scaled, &mut out);
        assert_eq!(scaled.len(), 1);
        assert_eq!(out, vec![1.0]);
        assert_eq!(scaled.as_ptr(), scaled_ptr);
        assert_eq!(out.as_ptr(), out_ptr);
    }

    #[test]
    fn ir_program_buffer_layout() {
        let p = softmax_step("e", "out", 32);
        assert_eq!(p.workgroup_size, [256, 1, 1]);
        let names: Vec<&str> = p.buffers.iter().map(|b| b.name()).collect();
        assert_eq!(names, vec!["e", "out"]);
        for buf in p.buffers.iter() {
            assert_eq!(buf.count(), 32);
        }
    }

    #[test]
    fn zero_n_traps() {
        let p = softmax_step("e", "out", 0);
        assert!(p.stats().trap());
    }
}
