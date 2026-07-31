//! Optimizer idempotence contract for BLAKE3 compression IR.

use vyre_foundation::optimizer::pre_lowering::optimize;
use vyre_libs::hash::blake3_compress;

#[test]
fn blake3_compress_pre_lowering_optimizer_is_idempotent() {
    let program = blake3_compress("cv_in", "msg", "params", "cv_out");
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
