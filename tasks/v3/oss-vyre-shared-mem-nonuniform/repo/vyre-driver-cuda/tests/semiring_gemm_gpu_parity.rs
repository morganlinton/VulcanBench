//! Parity test: GPU BoolOr-semiring GEMM matches Reference oracle.

#![cfg(test)]

mod common;

use common::with_cuda_optimizer_dispatcher;
use vyre_self_substrate::dataflow_fixpoint::{
    reference_semiring_gemm, semiring_gemm_via, semiring_gemm_via_bool_or,
    semiring_gemm_via_lineage, semiring_gemm_via_min_plus, Semiring,
};

#[test]
fn cuda_semiring_gemm_bool_or_matches_reference_3x3_identity() {
    // 3x3 identity adjacency.
    let a = vec![1u32, 0, 0, 0, 1, 0, 0, 0, 1];
    let b = a.clone();
    let gpu = with_cuda_optimizer_dispatcher("bool-or identity gemm", |dispatcher| {
        semiring_gemm_via_bool_or(dispatcher, &a, &b, 3, 3, 3).expect("dispatch")
    });
    let reference = reference_semiring_gemm(&a, &b, 3, 3, 3, Semiring::BoolOr);
    assert_eq!(gpu, reference);
}

#[test]
fn cuda_semiring_gemm_bool_or_chain_reach() {
    // Chain: 0->1->2->3 as a 4x4 adjacency. a[i][j] = 1 iff i->j.
    let a = vec![
        0u32, 1, 0, 0, // row 0: 0→1
        0, 0, 1, 0, // row 1: 1→2
        0, 0, 0, 1, // row 2: 2→3
        0, 0, 0, 0, // row 3: terminal
    ];
    let b = a.clone();
    // a*a under BoolOr should give 2-step reach.
    let gpu = with_cuda_optimizer_dispatcher("bool-or chain gemm", |dispatcher| {
        semiring_gemm_via_bool_or(dispatcher, &a, &b, 4, 4, 4).expect("dispatch")
    });
    let reference = reference_semiring_gemm(&a, &b, 4, 4, 4, Semiring::BoolOr);
    assert_eq!(
        gpu, reference,
        "GPU and reference GEMM diverged on chain reach"
    );
}

#[test]
fn cuda_semiring_gemm_via_dispatch_covers_all_semirings() {
    // Small 3x3 inputs that work for every semiring.
    let m = 3u32;
    let n = 3u32;
    let k = 3u32;
    let a: Vec<u32> = vec![1, 2, 0, 0, 1, 1, 1, 0, 1];
    let b: Vec<u32> = vec![0, 1, 1, 1, 0, 1, 1, 1, 0];
    with_cuda_optimizer_dispatcher("multi-semiring gemm", |dispatcher| {
        for semiring in [
            Semiring::Real,
            Semiring::MaxPlus,
            Semiring::MaxTimes,
            Semiring::BoolAnd,
            Semiring::Gf2,
        ] {
            let gpu = semiring_gemm_via(dispatcher, &a, &b, m, n, k, semiring).expect("dispatch");
            let reference = reference_semiring_gemm(&a, &b, m, n, k, semiring);
            assert_eq!(
                gpu, reference,
                "{semiring:?} GEMM divergence: gpu={gpu:?} reference={reference:?}"
            );
        }
    });
}

#[test]
fn cuda_semiring_gemm_min_plus_matches_reference() {
    // 4x4 distance matrix: u32::MAX = no edge.
    let m = u32::MAX;
    let a = vec![0u32, 5, m, m, m, 0, 3, m, m, m, 0, 2, m, m, m, 0];
    let b = a.clone();
    let gpu = with_cuda_optimizer_dispatcher("min-plus gemm", |dispatcher| {
        semiring_gemm_via_min_plus(dispatcher, &a, &b, 4, 4, 4).expect("dispatch")
    });
    let reference = reference_semiring_gemm(&a, &b, 4, 4, 4, Semiring::MinPlus);
    assert_eq!(gpu, reference, "MinPlus GEMM divergence");
}

#[test]
fn cuda_semiring_gemm_lineage_matches_reference() {
    // 4x4 lineage matrix: each cell = bitset of clauses that derive
    // a region from a source. Combine = OR (zero-absorbing); accumulate = OR.
    let a: Vec<u32> = vec![
        0b0001, 0b0000, 0b0010, 0b0000, 0b0000, 0b0100, 0b0000, 0b1000, 0b0001, 0b0010, 0b0100,
        0b1000, 0b0000, 0b0000, 0b0001, 0b0010,
    ];
    let b = a.clone();
    let gpu = with_cuda_optimizer_dispatcher("lineage gemm", |dispatcher| {
        semiring_gemm_via_lineage(dispatcher, &a, &b, 4, 4, 4).expect("dispatch")
    });
    let reference = reference_semiring_gemm(&a, &b, 4, 4, 4, Semiring::Lineage);
    assert_eq!(gpu, reference, "Lineage GEMM divergence");
}

#[test]
fn cuda_semiring_gemm_bool_or_random_8x8() {
    let m = 8u32;
    let n = 8u32;
    let k = 8u32;
    // Pseudo-random bitset matrix using a simple LCG.
    let mut state: u32 = 0xdead_beef;
    let mut next = || {
        state = state.wrapping_mul(1103515245).wrapping_add(12345);
        state
    };
    let a: Vec<u32> = (0..(m * k)).map(|_| next() & 0x0F).collect();
    let b: Vec<u32> = (0..(k * n)).map(|_| next() & 0x0F).collect();
    let gpu = with_cuda_optimizer_dispatcher("bool-or random gemm", |dispatcher| {
        semiring_gemm_via_bool_or(dispatcher, &a, &b, m, n, k).expect("dispatch")
    });
    let reference = reference_semiring_gemm(&a, &b, m, n, k, Semiring::BoolOr);
    assert_eq!(gpu, reference, "GPU/reference 8x8 BoolOr GEMM divergence");
}
