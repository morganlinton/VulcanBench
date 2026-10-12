//! Parity test: GPU reachability + lineage closures match reference oracle.

#![cfg(test)]

mod common;

use common::with_cuda_optimizer_dispatcher;
use vyre_self_substrate::dataflow_fixpoint::{
    forward_backward_bitsets_for_pivot, forward_backward_bitsets_for_pivot_via, lineage_closure,
    lineage_closure_via, reachability_closure, reachability_closure_via,
    scc_components_via_substrate, scc_components_via_substrate_via, shortest_path_closure,
    shortest_path_closure_via,
};

#[test]
fn cuda_reachability_closure_via_chain() {
    // 4-node chain.
    let adj = vec![0u32, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0];
    let gpu = with_cuda_optimizer_dispatcher("reachability chain", |dispatcher| {
        reachability_closure_via(dispatcher, &adj, 4, 10).expect("dispatch")
    });
    let reference = reachability_closure(&adj, 4, 10);
    assert_eq!(gpu, reference, "reachability closure divergence");
}

#[test]
fn cuda_reachability_closure_via_diamond() {
    // 0 -> 1, 0 -> 2, 1 -> 3, 2 -> 3.
    let adj = vec![0u32, 1, 1, 0, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0];
    let gpu = with_cuda_optimizer_dispatcher("reachability diamond", |dispatcher| {
        reachability_closure_via(dispatcher, &adj, 4, 5).expect("dispatch")
    });
    let reference = reachability_closure(&adj, 4, 5);
    assert_eq!(gpu, reference, "diamond closure divergence");
}

#[test]
fn cuda_scc_components_via_substrate_via_matches_reference() {
    with_cuda_optimizer_dispatcher("scc components", |dispatcher| {
        // 3-cycle 0->1->2->0 makes a single SCC of size 3.
        let adj = vec![0u32, 1, 0, 0, 0, 1, 1, 0, 0];
        let gpu = scc_components_via_substrate_via(dispatcher, &adj, 3).expect("dispatch");
        let reference = scc_components_via_substrate(&adj, 3);
        assert_eq!(gpu, reference, "scc components divergence on 3-cycle");

        // Two disjoint cycles: {0,1} and {2,3}.
        let adj = vec![0u32, 1, 0, 0, 1, 0, 0, 0, 0, 0, 0, 1, 0, 0, 1, 0];
        let gpu = scc_components_via_substrate_via(dispatcher, &adj, 4).expect("dispatch");
        let reference = scc_components_via_substrate(&adj, 4);
        assert_eq!(
            gpu, reference,
            "scc components divergence on disjoint cycles"
        );
    });
}

#[test]
fn cuda_forward_backward_bitsets_for_pivot_via_matches_reference() {
    // 4x4 adjacency: cycle 0->1->2->3->0 plus a chord 0->2.
    let adj = vec![0u32, 1, 1, 0, 0, 0, 1, 0, 0, 0, 0, 1, 1, 0, 0, 0];
    with_cuda_optimizer_dispatcher("forward/backward pivot", |dispatcher| {
        for pivot in [0u32, 1, 2, 3] {
            let (gpu_fwd, gpu_bwd) = {
                forward_backward_bitsets_for_pivot_via(dispatcher, &adj, pivot, 4)
                    .expect("dispatch")
            };
            let (reference_fwd, reference_bwd) = forward_backward_bitsets_for_pivot(&adj, pivot, 4);
            assert_eq!(gpu_fwd, reference_fwd, "fwd divergence for pivot={pivot}");
            assert_eq!(gpu_bwd, reference_bwd, "bwd divergence for pivot={pivot}");
        }
    });
}

#[test]
fn cuda_shortest_path_closure_via_matches_reference() {
    // 4-node weighted graph with u32::MAX as "no-edge".
    let m = u32::MAX;
    let adj = vec![0u32, 5, m, 9, m, 0, 3, m, m, m, 0, 2, m, m, m, 0];
    let gpu = with_cuda_optimizer_dispatcher("shortest path closure", |dispatcher| {
        shortest_path_closure_via(dispatcher, &adj, 4, 6).expect("dispatch")
    });
    let reference = shortest_path_closure(&adj, 4, 6);
    assert_eq!(gpu, reference, "shortest-path closure divergence");
}

#[test]
fn cuda_lineage_closure_via_matches_reference() {
    // 4x4 lineage matrix: each cell = bitset of clauses.
    let adj: Vec<u32> = vec![
        0b0001, 0b0010, 0b0000, 0b0000, 0b0000, 0b0100, 0b1000, 0b0000, 0b0000, 0b0000, 0b0001,
        0b0010, 0b0000, 0b0000, 0b0000, 0b0100,
    ];
    let gpu = with_cuda_optimizer_dispatcher("lineage closure", |dispatcher| {
        lineage_closure_via(dispatcher, &adj, 4, 5).expect("dispatch")
    });
    let reference = lineage_closure(&adj, 4, 5);
    assert_eq!(gpu, reference, "lineage closure divergence");
}
