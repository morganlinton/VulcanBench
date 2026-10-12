//! P-CONFORM-1: every consumed self-substrate module gets a smoke
//! test entry in the conform-runner matrix.
//!
//! Each test exercises the module's primary entry point on a small
//! synthetic input and asserts the call doesn't panic / returns the
//! expected shape. The point isn't algorithmic correctness (the
//! primitive's own unit tests cover that)  -  it's "this self-consumer
//! is reachable and callable end-to-end through the public crate
//! surface that user dialects will actually go through."

#![allow(missing_docs)]

use vyre_foundation::pass_substrate::multigrid_matroid_solver;
use vyre_self_substrate::{
    amg_pass_solver, do_calculus_change_impact, matroid_exact_megakernel,
    matroid_megakernel_scheduler, megakernel_schedule, observability, scallop_provenance,
    sheaf_heterophilic_dispatch, sheaf_spectral_clustering, submodular_cache_eviction,
    tensor_train_compression,
};

#[test]
fn matroid_megakernel_scheduler_is_callable() {
    let n = 4;
    let seed = vec![0u32, 0, 0, 0];
    let exchange_adj = vec![0u32; 16];
    let result =
        matroid_megakernel_scheduler::max_fusion_subset(&seed, &exchange_adj, n, 4).unwrap();
    assert_eq!(result.len(), n);
}

#[test]
fn matroid_exact_megakernel_is_callable() {
    let n = 3;
    let exchange_adj = vec![0u32; 9];
    let sources = vec![1u32, 0, 0];
    let sinks = vec![0u32, 0, 1];
    let seed = vec![0u32; 3];
    let result = matroid_exact_megakernel::reference_select_optimal_subset(
        &exchange_adj,
        &sources,
        &sinks,
        &seed,
        n,
        4,
    )
    .unwrap();
    assert_eq!(result.len(), n);
}

#[test]
fn megakernel_schedule_is_callable() {
    let costs = vec![1.0, 2.0, 3.0];
    let result = megakernel_schedule::schedule_via_homotopy(&costs, 3, 4, 0.1);
    assert_eq!(result.len(), 3);
}

#[test]
fn multigrid_matroid_solver_is_callable() {
    let n: u32 = 3;
    let mut a = vec![0.0f64; 9];
    for i in 0..3 {
        a[i * 3 + i] = 1.0;
    }
    let b = vec![1.0, 2.0, 3.0];
    let x_in = vec![0.0; 3];
    let result = multigrid_matroid_solver::matroid_solve_step(&a, &b, &x_in, 0.66, n);
    assert_eq!(result.len(), 3);
}

#[test]
fn sheaf_heterophilic_dispatch_is_callable() {
    let stalks = vec![1.0, 2.0, 3.0];
    let restriction_diag = vec![0.5; 3];
    let result = sheaf_heterophilic_dispatch::reference_diffuse_dispatch_stalks(
        &stalks,
        &restriction_diag,
        0.5,
    );
    assert_eq!(result.len(), 3);
}

#[test]
fn sheaf_spectral_clustering_is_callable() {
    let restriction_diag = vec![0.7; 4];
    let (lambda, v) = sheaf_spectral_clustering::dominant_spectrum(&restriction_diag, 16);
    assert!(lambda.is_finite());
    assert_eq!(v.len(), 4);
}

#[test]
fn submodular_cache_eviction_is_callable() {
    let mut gains = vec![5u32, 3, 7, 1];
    let result = submodular_cache_eviction::select_retention_set(&mut gains, 4, 2);
    assert_eq!(result.len(), 4);
}

#[test]
fn do_calculus_change_impact_is_callable() {
    let adj = vec![0u32; 9];
    let intervention_mask = vec![1u32, 0, 0];
    let result = do_calculus_change_impact::predict_impact(&adj, &intervention_mask, 3);
    assert_eq!(result.len(), 3);
}

#[test]
fn scallop_provenance_is_callable() {
    let state = vec![0u32; 4];
    let join_rules = vec![0u32; 4];
    let result = scallop_provenance::reference_provenance_closure(&state, &join_rules, 2, 4);
    assert_eq!(result.len(), 4);
}

#[test]
fn amg_pass_solver_is_callable() {
    let n_fine = 2;
    let n_coarse = 1;
    let a = vec![1.0f64, 0.0, 0.0, 1.0];
    let b = vec![1.0, 2.0];
    let x = vec![0.0; 2];
    let r_mat = vec![0.5, 0.5];
    let p_mat = vec![1.0, 1.0];
    let a_c = vec![1.0];
    let result = amg_pass_solver::reference_smooth_matroid_flow(
        &a, &b, &x, &r_mat, &p_mat, &a_c, n_fine, n_coarse,
    );
    assert_eq!(result.len(), 2);
}

#[test]
fn tensor_train_compression_is_callable() {
    let dims = vec![2u32, 2];
    let target_ranks = vec![1u32, 2, 1];
    let tensor = vec![1.0; 4];
    let compressed =
        tensor_train_compression::reference_compress_cost_tensor(&tensor, &dims, &target_ranks);
    assert_eq!(compressed.dims, dims);
}

#[test]
fn observability_counters_increment_on_substrate_calls() {
    let before = observability::total_calls();
    let _ = matroid_megakernel_scheduler::max_fusion_subset(&[0u32; 0], &[], 0, 4).unwrap();
    let _ = sheaf_spectral_clustering::dominant_spectrum(&[0.5; 2], 4);
    let after = observability::total_calls();
    assert!(
        after > before,
        "substrate call counters did not advance: before={before} after={after}"
    );
}
