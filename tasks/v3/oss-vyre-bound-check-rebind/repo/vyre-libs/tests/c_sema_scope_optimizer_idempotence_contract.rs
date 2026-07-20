//! Optimizer idempotence contract for the GPU C semantic-scope program.

#![cfg(feature = "c-parser")]

use vyre::ir::Expr;
use vyre_foundation::optimizer::pre_lowering::optimize;
use vyre_libs::parsing::c::sema::registry::c_sema_scope;

#[test]
fn c_sema_scope_pre_lowering_optimizer_is_idempotent() {
    let program = c_sema_scope(
        "tok_types",
        "tok_starts",
        "tok_lens",
        "haystack",
        Expr::u32(16),
        Expr::u32(14),
        "out_scope_tree",
    );

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
