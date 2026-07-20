use super::super::*;
use super::linear_graph;
use vyre_primitives::graph::persistent_bfs::cpu_ref as reference_persistent_bfs;

#[test]
fn checked_reference_surfaces_bad_frontier_width() {
    let offsets = vec![0u32; 65];
    let err = try_bfs_expand(64, &offsets, &[], &[], &[1], 0xFFFF_FFFF, 1)
        .expect_err("short persistent BFS seed frontier must fail through substrate wrapper");

    assert!(
        err.contains("frontier"),
        "Fix: persistent BFS checked wrapper must preserve primitive frontier diagnostics, got: {err}"
    );
}

#[test]
fn expand_chain_saturates() {
    let (off, tgt, msk) = linear_graph();
    let (out, changed) = bfs_expand(4, &off, &tgt, &msk, &[0b0001], 0xFFFF_FFFF, 8);
    assert_eq!(out, vec![0b1111]);
    assert_eq!(changed, 1);
}

#[test]
fn empty_seed_yields_empty_with_no_change() {
    let (off, tgt, msk) = linear_graph();
    let (out, changed) = bfs_expand(4, &off, &tgt, &msk, &[0u32], 0xFFFF_FFFF, 4);
    assert_eq!(out, vec![0u32]);
    assert_eq!(changed, 0);
}

#[test]
fn saturated_seed_reports_no_change() {
    let (off, tgt, msk) = linear_graph();
    let (out, changed) = bfs_expand(4, &off, &tgt, &msk, &[0b1111], 0xFFFF_FFFF, 4);
    assert_eq!(out, vec![0b1111]);
    assert_eq!(changed, 0);
}

#[test]
fn matches_primitive_directly() {
    let (off, tgt, msk) = linear_graph();
    let seed = vec![0b0001];
    let via_substrate = bfs_expand(4, &off, &tgt, &msk, &seed, 0xFFFF_FFFF, 5);
    let via_primitive = reference_persistent_bfs(4, &off, &tgt, &msk, &seed, 0xFFFF_FFFF, 5);
    assert_eq!(via_substrate, via_primitive);
}

#[test]
fn max_iters_bound_honored() {
    let (off, tgt, msk) = linear_graph();
    let (out, _) = bfs_expand(4, &off, &tgt, &msk, &[0b0001], 0xFFFF_FFFF, 1);
    assert_eq!(out[0] & 0b1111, 0b0011);
}

#[test]
fn allow_mask_filters_all_edges() {
    let (off, tgt, msk) = linear_graph();
    let (out, changed) = bfs_expand(4, &off, &tgt, &msk, &[0b0001], 0b0010, 4);
    assert_eq!(out, vec![0b0001]);
    assert_eq!(changed, 0);
}

#[test]
fn forward_reach_saturates_chain() {
    let (off, tgt, msk) = linear_graph();
    let out = forward_reach(4, &off, &tgt, &msk, &[0b0001], 0xFFFF_FFFF);
    assert_eq!(out, vec![0b1111]);
}

#[test]
fn self_loop_terminates() {
    let off = vec![0, 1, 1];
    let tgt = vec![0];
    let msk = vec![1];
    let (out, _) = bfs_expand(2, &off, &tgt, &msk, &[0b01], 0xFFFF_FFFF, 50);
    assert_eq!(out, vec![0b01]);
}
