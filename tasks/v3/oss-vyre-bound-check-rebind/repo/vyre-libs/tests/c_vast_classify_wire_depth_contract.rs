//! Wire-depth contract for the GPU C VAST classifier program.

#![cfg(feature = "c-parser")]

use vyre::ir::{Expr, Program};
use vyre_foundation::optimizer::pre_lowering::optimize;
use vyre_libs::parsing::c::parse::vast::c11_classify_vast_node_kinds;

#[test]
fn c_vast_classifier_wire_roundtrip_respects_decode_depth_contract() {
    let program = c11_classify_vast_node_kinds("vast_nodes", Expr::u32(9), "out_typed_vast_nodes");
    let wire = program
        .to_wire()
        .expect("VAST classifier must encode to the canonical wire format");
    let decoded = Program::from_wire(&wire)
        .expect("VAST classifier wire form must stay within canonical decode depth");
    assert_eq!(program, decoded);
}

#[test]
fn c_vast_classifier_optimizer_is_idempotent() {
    let program = c11_classify_vast_node_kinds("vast_nodes", Expr::u32(9), "out_typed_vast_nodes");
    let optimized_once = optimize(program);
    let optimized_twice = optimize(optimized_once.clone());
    assert_eq!(
        optimized_once,
        optimized_twice,
        "{}",
        first_debug_difference(&optimized_once, &optimized_twice)
    );
}

fn first_debug_difference(left: &impl std::fmt::Debug, right: &impl std::fmt::Debug) -> String {
    let left = format!("{left:?}");
    let right = format!("{right:?}");
    let first_diff = left
        .bytes()
        .zip(right.bytes())
        .position(|(left, right)| left != right)
        .unwrap_or_else(|| left.len().min(right.len()));
    let left_start = first_diff.saturating_sub(240);
    let right_start = first_diff.saturating_sub(240);
    let left_end = left.len().min(first_diff + 520);
    let right_end = right.len().min(first_diff + 520);
    format!(
        "first debug diff at byte {first_diff}\nleft: {}\nright: {}",
        &left[left_start..left_end],
        &right[right_start..right_end],
    )
}
