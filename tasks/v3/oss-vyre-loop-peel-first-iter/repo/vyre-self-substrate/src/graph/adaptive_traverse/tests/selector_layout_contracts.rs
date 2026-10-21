use super::super::state::{adaptive_four_russians_layout_hash, adaptive_traversal_layout_hash};
use super::super::*;

#[test]
fn selector_uses_queue_for_tiny_sparse_frontier() {
    assert_eq!(
        select_adaptive_traversal_mode(1_000, 10_000, 1, 25),
        AdaptiveTraversalMode::SparseQueue
    );
}

#[test]
fn selector_uses_sparse_dense_at_dense_cutover() {
    assert_eq!(
        select_adaptive_traversal_mode(1_000, 10_000, 260, 25),
        AdaptiveTraversalMode::SparseDense
    );
}

#[test]
fn selector_exports_four_russians_dense_kernel_choice() {
    assert_eq!(
        select_dense_traversal_kernel(1_024, 900, 2),
        DenseTraversalKernel::FourRussiansByteTile
    );
}

#[test]
fn layout_hash_distinguishes_dense_rows() {
    let offsets = [0, 0];
    let targets = [];
    let masks = [];
    let a = adaptive_traversal_layout_hash(1, &offsets, &targets, &masks, &[1]);
    let b = adaptive_traversal_layout_hash(1, &offsets, &targets, &masks, &[2]);
    assert_ne!(a, b);
}

#[test]
fn four_russians_layout_hash_distinguishes_dense_rows() {
    let a = adaptive_four_russians_layout_hash(8, &[0b0000_0001, 0, 0, 0, 0, 0, 0, 0]);
    let b = adaptive_four_russians_layout_hash(8, &[0b0000_0010, 0, 0, 0, 0, 0, 0, 0]);
    assert_ne!(a, b);
}
