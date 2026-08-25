//! Regression for FINDING-PRIM-1: softmax / layer_norm must run as
//! workgroup-cooperative *tiled* reductions (workgroup_size_x > 1 with a
//! workgroup barrier), not as scalar `[1, 1, 1]` dispatches. The finding tracked
//! the gap where these ops fell back to single-lane workgroups because the
//! cooperative reduction primitive (shared memory + barrier) was not wired.

#![cfg(all(feature = "nn-attention", feature = "nn-norm"))]

use vyre::ir::Node;

fn has_barrier(nodes: &[Node]) -> bool {
    nodes.iter().any(|node| match node {
        Node::Barrier { .. } => true,
        Node::If { then, otherwise, .. } => has_barrier(then) || has_barrier(otherwise),
        Node::Loop { body, .. } | Node::Block(body) => has_barrier(body),
        Node::Region { body, .. } => has_barrier(body),
        _ => false,
    })
}

#[test]
fn softmax_runs_cooperatively_tiled_not_scalar() {
    let program = vyre_libs::nn::softmax("input", "output", 1024);
    let wg = program.workgroup_size();
    assert!(
        wg[0] > 1,
        "softmax must dispatch a cooperative tile (workgroup_size_x > 1), got {wg:?}"
    );
    // SOFTMAX_TILE is 256; pin it so a regression to [1,1,1] is caught loudly.
    assert_eq!(wg, [256, 1, 1], "softmax should tile at SOFTMAX_TILE lanes");
    assert!(
        has_barrier(program.entry()),
        "a cooperative softmax must use a workgroup barrier to synchronize its reduction"
    );
}

#[test]
fn layer_norm_runs_cooperatively_tiled_not_scalar() {
    let program = vyre_libs::nn::layer_norm("input", "output", 1024, 1e-5);
    let wg = program.workgroup_size();
    assert!(
        wg[0] > 1,
        "layer_norm must dispatch a cooperative tile (workgroup_size_x > 1), got {wg:?}"
    );
    assert_eq!(wg, [256, 1, 1], "layer_norm should tile at LAYER_NORM_TILE lanes");
    assert!(
        has_barrier(program.entry()),
        "a cooperative layer_norm must use a workgroup barrier to synchronize its reduction"
    );
}
