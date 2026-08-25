//! Parity test: GPU union-find produces the same connected-component
//! partition as the reference. Compares roots after path
//! compression since GPU's atomic-CAS unions can produce different
//! intermediate parent links but must agree on which nodes share a
//! root.

#![cfg(test)]

mod common;

use common::with_cuda_optimizer_dispatcher;
use vyre_primitives::graph::union_find::union_find_dispatch_grid;
use vyre_self_substrate::union_find_emit::{
    canonicalize_parent_to_roots, reference_union_find_alias, union_find_alias_via,
};

fn assert_same_partition(a: &[u32], b: &[u32]) {
    let n = a.len();
    assert_eq!(n, b.len());
    let ra = canonicalize_parent_to_roots(a);
    let rb = canonicalize_parent_to_roots(b);
    for i in 0..n {
        for j in (i + 1)..n {
            let same_a = ra[i] == ra[j];
            let same_b = rb[i] == rb[j];
            assert_eq!(
                same_a, same_b,
                "partition mismatch at ({i}, {j}): gpu_roots={ra:?} reference_roots={rb:?}"
            );
        }
    }
}

fn assert_union_find_matches_partition(
    label: &str,
    parent_init: &[u32],
    edge_a: &[u32],
    edge_b: &[u32],
) -> (Vec<u32>, Vec<u32>) {
    let reference = reference_union_find_alias(parent_init, edge_a, edge_b);
    let gpu = with_cuda_optimizer_dispatcher(label, |dispatcher| {
        union_find_alias_via(dispatcher, parent_init, edge_a, edge_b).expect("dispatch")
    });
    assert_same_partition(&reference, &gpu);
    (reference, gpu)
}

#[test]
fn cuda_union_find_disconnected_pairs() {
    let parent_init: Vec<u32> = (0..6).collect();
    let edge_a = vec![0u32, 2, 4];
    let edge_b = vec![1u32, 3, 5];
    assert_union_find_matches_partition("disconnected pairs", &parent_init, &edge_a, &edge_b);
}

#[test]
fn cuda_union_find_chain() {
    let parent_init: Vec<u32> = (0..5).collect();
    let edge_a = vec![0u32, 1, 2, 3];
    let edge_b = vec![1u32, 2, 3, 4];
    let (reference, _) =
        assert_union_find_matches_partition("chain", &parent_init, &edge_a, &edge_b);
    let reference_roots = canonicalize_parent_to_roots(&reference);
    for i in 1..5 {
        assert_eq!(reference_roots[0], reference_roots[i]);
    }
}

#[test]
fn cuda_union_find_two_disjoint_components() {
    let parent_init: Vec<u32> = (0..6).collect();
    let edge_a = vec![0u32, 1, 3, 4];
    let edge_b = vec![1u32, 2, 4, 5];
    let (_, gpu) = assert_union_find_matches_partition(
        "two disjoint components",
        &parent_init,
        &edge_a,
        &edge_b,
    );
    let roots = canonicalize_parent_to_roots(&gpu);
    assert_eq!(roots[0], roots[1]);
    assert_eq!(roots[0], roots[2]);
    assert_eq!(roots[3], roots[4]);
    assert_eq!(roots[3], roots[5]);
    assert_ne!(roots[0], roots[3]);
}

#[test]
fn cuda_union_find_no_edges_keeps_singletons() {
    let parent_init: Vec<u32> = (0..4).collect();
    let (reference, gpu) = assert_union_find_matches_partition("no edges", &parent_init, &[], &[]);
    assert_eq!(reference, parent_init);
    let roots = canonicalize_parent_to_roots(&gpu);
    for i in 0..4u32 {
        assert_eq!(roots[i as usize], i);
    }
}

#[test]
fn cuda_union_find_multi_block_chain_connects_all_nodes() {
    let node_count = 1026u32;
    let parent_init: Vec<u32> = (0..node_count).collect();
    let edge_a: Vec<u32> = (0..(node_count - 1)).collect();
    let edge_b: Vec<u32> = (1..node_count).collect();

    let (_, gpu) =
        assert_union_find_matches_partition("multi-block chain", &parent_init, &edge_a, &edge_b);
    let roots = canonicalize_parent_to_roots(&gpu);

    assert_eq!(union_find_dispatch_grid(edge_a.len() as u32), [5, 1, 1]);
    assert!(roots.iter().all(|&root| root == 0));
}
