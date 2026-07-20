//! Test crate.

#![cfg(all(
    feature = "math-linalg",
    feature = "math-scan",
    feature = "math-broadcast",
    feature = "nn-linear",
    feature = "nn-attention",
    feature = "nn-norm",
    feature = "nn-activation",
    feature = "matching-substring"
))]

use std::panic::{catch_unwind, AssertUnwindSafe};

use vyre_foundation::optimizer::ctx::AdapterCaps;
use vyre_libs::nn::{Attention, LayerNorm, Softmax};
use vyre_libs::tensor_ref::TensorRef;

const MAX_WORKGROUP_LANES: u32 = AdapterCaps::high_end().max_invocations_per_workgroup;

fn assert_no_panic<F>(label: &str, build: F)
where
    F: FnOnce(),
{
    catch_unwind(AssertUnwindSafe(build))
        .unwrap_or_else(|_| panic!("Fix: {label} must not panic at boundary size"));
}

#[test]
fn relu_boundaries_do_not_panic() {
    for &n in &[0, 1, MAX_WORKGROUP_LANES, MAX_WORKGROUP_LANES + 1] {
        assert_no_panic("relu", || {
            let _ = vyre_libs::nn::relu("input", "output", n);
        });
    }
}

#[test]
fn broadcast_boundaries_do_not_panic() {
    for &n in &[0, 1, MAX_WORKGROUP_LANES, MAX_WORKGROUP_LANES + 1] {
        assert_no_panic("broadcast", || {
            let _ = vyre_libs::math::broadcast("src", "dst", n);
        });
    }
}

#[test]
fn scan_prefix_sum_boundaries_do_not_panic() {
    for &n in &[0, 1, MAX_WORKGROUP_LANES, MAX_WORKGROUP_LANES + 1] {
        assert_no_panic("scan_prefix_sum", || {
            let _ = vyre_libs::math::scan_prefix_sum("input", "output", n);
        });
    }
}

#[test]
fn substring_search_boundaries_do_not_panic() {
    for &n in &[0, 1, MAX_WORKGROUP_LANES, MAX_WORKGROUP_LANES + 1] {
        assert_no_panic("substring_search", || {
            let _ = vyre_libs::scan::substring_search("haystack", "needle", "matches", n, 1);
        });
    }
}

#[test]
fn softmax_zero_returns_actionable_error() {
    let error = Softmax::new(
        TensorRef::f32_1d("input", 0),
        TensorRef::f32_1d("output", 0),
    )
    .build()
    .unwrap_err();
    assert!(
        error.to_string().contains("softmax") || format!("{error:?}").contains("softmax"),
        "Fix: softmax n=0 error must mention the op and be actionable: {error:?}"
    );
}

#[test]
fn layer_norm_zero_returns_actionable_error() {
    let error = LayerNorm::new(
        TensorRef::f32_1d("input", 0),
        TensorRef::f32_1d("output", 0),
        1e-5,
    )
    .build()
    .unwrap_err();
    assert!(
        error.to_string().contains("layer_norm") || format!("{error:?}").contains("layer_norm"),
        "Fix: layer_norm n=0 error must mention the op and be actionable: {error:?}"
    );
}

#[test]
fn attention_zero_returns_actionable_error() {
    let error = Attention::new(
        TensorRef::f32_2d("q", 0, 4),
        TensorRef::f32_2d("k", 0, 4),
        TensorRef::f32_2d("v", 0, 4),
        TensorRef::f32_2d("out", 0, 4),
    )
    .build()
    .unwrap_err();
    assert!(
        error.to_string().contains("attention") || format!("{error:?}").contains("attention"),
        "Fix: attention n=0 error must mention the op and be actionable: {error:?}"
    );
}

#[test]
fn dot_zero_returns_actionable_error() {
    let error = vyre_libs::math::dot("lhs", "rhs", "out", 0).unwrap_err();
    assert!(
        error.contains("dot") || error.contains("empty"),
        "Fix: dot n=0 error must mention the op and be actionable: {error}"
    );
}

#[test]
fn linear_zero_returns_actionable_error() {
    let error = vyre_libs::nn::linear("x", "w", "b", "out", 0, 1).unwrap_err();
    assert!(
        error.contains("linear") || error.contains("empty"),
        "Fix: linear n=0 error must mention the op and be actionable: {error}"
    );
}

#[test]
fn linear_zero_output_dim_returns_actionable_error() {
    let error = vyre_libs::nn::linear("x", "w", "b", "out", 1, 0).unwrap_err();
    assert!(
        error.contains("linear") || error.contains("empty"),
        "Fix: linear out_dim=0 error must mention the op and be actionable: {error}"
    );
}
