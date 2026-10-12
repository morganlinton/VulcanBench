//! Parity test: GPU bidirectional CSR step matches the reference oracle.

#![cfg(test)]

mod common;

use common::with_cuda_optimizer_dispatcher;
use vyre_self_substrate::csr_bidirectional::{
    bidirectional_closure_via, bidirectional_step_via, reference_bidirectional_closure,
    reference_bidirectional_step,
};

fn linear_chain() -> (u32, Vec<u32>, Vec<u32>, Vec<u32>) {
    // 0 -> 1 -> 2 -> 3
    (4, vec![0, 1, 2, 3, 3], vec![1, 2, 3], vec![1, 1, 1])
}

fn assert_bidirectional_step_matches_reference(
    label: &str,
    n: u32,
    off: &[u32],
    tgt: &[u32],
    msk: &[u32],
    seed: &[u32],
    allow_mask: u32,
) -> Vec<u32> {
    let reference = reference_bidirectional_step(n, off, tgt, msk, seed, allow_mask);
    with_cuda_optimizer_dispatcher(label, |dispatcher| {
        let gpu = bidirectional_step_via(dispatcher, n, off, tgt, msk, seed, allow_mask)
            .expect("dispatch");
        assert_eq!(gpu, reference, "{label}: bidirectional step divergence");
        gpu
    })
}

fn assert_bidirectional_closure_matches_reference(
    label: &str,
    n: u32,
    off: &[u32],
    tgt: &[u32],
    msk: &[u32],
    seed: &[u32],
    allow_mask: u32,
    max_iters: u32,
) {
    let reference = reference_bidirectional_closure(n, off, tgt, msk, seed, allow_mask, max_iters);
    with_cuda_optimizer_dispatcher(label, |dispatcher| {
        let gpu =
            bidirectional_closure_via(dispatcher, n, off, tgt, msk, seed, allow_mask, max_iters)
                .expect("dispatch");
        assert_eq!(gpu, reference, "{label}: bidirectional closure divergence");
    });
}

#[test]
fn cuda_bidirectional_step_via_matches_reference_chain() {
    let (n, off, tgt, msk) = linear_chain();
    // Seed = {1}. Expected: forward {2} ∪ backward {0} = {0, 2}.
    assert_bidirectional_step_matches_reference(
        "chain step",
        n,
        &off,
        &tgt,
        &msk,
        &[0b0010],
        0xFFFF_FFFF,
    );
}

#[test]
fn cuda_bidirectional_step_via_respects_allow_mask() {
    // 2-node graph: 0 -[k=2]-> 1
    let n = 2;
    let off = vec![0u32, 1, 1];
    let tgt = vec![1u32];
    let msk = vec![0b0010];
    let seed = vec![0b01u32];
    // allow_mask = 0b0001 → no edges match → empty step output.
    assert_bidirectional_step_matches_reference(
        "filtered allow-mask step",
        n,
        &off,
        &tgt,
        &msk,
        &seed,
        0b0001,
    );
    // allow_mask = 0b0010 → forward 0→1 fires.
    assert_bidirectional_step_matches_reference(
        "matching allow-mask step",
        n,
        &off,
        &tgt,
        &msk,
        &seed,
        0b0010,
    );
}

#[test]
fn cuda_bidirectional_closure_via_matches_reference_chain() {
    let (n, off, tgt, msk) = linear_chain();
    assert_bidirectional_closure_matches_reference(
        "chain closure",
        n,
        &off,
        &tgt,
        &msk,
        &[0b0001u32],
        0xFFFF_FFFF,
        n,
    );
}
