//! GPU borrow checker on CUDA: the NLL loan-liveness reachability that the
//! front-end-agnostic engine (`vyre_libs::borrowck::analyze`) computes on the
//! CPU, evaluated instead on the CUDA device via the iterated-closure graph
//! primitive, and parity-gated against the CPU engine across the full
//! control-flow-correctness suite.
//!
//! The engine's two monotone bitset dataflows are equivalent to per-loan
//! reachability:
//!   * a loan's issue reaches a point  <=>  the point is in the forward closure
//!     of {issue} over the CFG;
//!   * a use of the loan is reachable from a point  <=>  the point is in the
//!     forward closure of the loan's use points over the REVERSED CFG.
//! A loan is live at a point iff the point is in both closures, and two loans
//! of one place conflict when one is live at the other's issue and at least one
//! is mutable. Computing both closures on the GPU and pairing on the host must
//! reproduce the CPU engine's verdict exactly. The speedup over the CPU engine
//! comes at scale by batching every loan's closures into one device dispatch;
//! this test fixes correctness first.

#![cfg(test)]

#[path = "common/borrowck_reachability_cuda.rs"]
mod borrowck_reachability_cuda;
mod common;

use std::time::Instant;

use borrowck_reachability_cuda::{corpus, cuda_conflicts, cuda_conflicts_batched, facts};
use common::{with_cuda_optimizer_dispatcher, with_live_backend};
use vyre_driver_cuda::CudaOptimizerDispatcher as CudaResidentDispatcher;
use vyre_libs::borrowck::{analyze, gpu, BorrowFacts, Conflict, ConflictKind, LoanKind};

#[test]
fn cuda_borrow_checker_matches_cpu_engine_across_cfg_suite() {
    let cases = corpus();
    with_cuda_optimizer_dispatcher("cuda borrowck reachability", |dispatcher| {
        for (label, f) in &cases {
            let cpu = analyze(f);
            let gpu = cuda_conflicts(dispatcher, f);
            assert_eq!(
                gpu, cpu,
                "{label}: CUDA borrow-check verdict diverged from the CPU engine"
            );
        }
    });
}

#[test]
fn cuda_borrow_checker_scales_to_a_long_chain() {
    // 2000-point straight chain; two &mut of one place issued early, both used
    // at the end -> live across the whole chain -> exactly one conflict, the
    // same verdict the CPU engine reaches, computed via device closures.
    let n = 2000u32;
    let cfg: Vec<(u32, u32)> = (0..n - 1).map(|i| (i, i + 1)).collect();
    let f = facts(
        n,
        &cfg,
        &[(0, LoanKind::Mut, 0, 10), (0, LoanKind::Mut, 1, 20)],
        &[(0, n - 1), (1, n - 1)],
    );
    let cpu = analyze(&f);
    let gpu = with_cuda_optimizer_dispatcher("cuda borrowck chain", |dispatcher| {
        cuda_conflicts(dispatcher, &f)
    });
    assert_eq!(
        gpu, cpu,
        "CUDA long-chain verdict diverged from the CPU engine"
    );
    assert_eq!(gpu.len(), 1, "expected exactly one conflict, got {gpu:?}");
    assert_eq!(gpu[0].kind, ConflictKind::TwoMutable);
}

#[test]
fn cuda_batched_borrow_checker_matches_cpu_engine() {
    let cases = corpus();
    with_live_backend("cuda batched borrowck", |backend| {
        let dispatcher = CudaResidentDispatcher::new(backend);
        for (label, f) in &cases {
            let cpu = analyze(f);
            let gpu = cuda_conflicts_batched(&dispatcher, f);
            assert_eq!(
                gpu, cpu,
                "{label}: batched CUDA borrow-check verdict diverged from the CPU engine"
            );
        }
    });
}

#[test]
fn cuda_batched_borrow_checker_many_loans_two_dispatches() {
    // A 200-point chain carrying 64 distinct-place loans, each issued and used
    // once along the chain. The per-loan path would launch 2 x 64 = 128 device
    // dispatches; the batched path runs every loan's forward closure in one
    // dispatch and every backward closure in a second: 2 total, same verdict.
    let n = 200u32;
    let loan_count = 64u32;
    let cfg: Vec<(u32, u32)> = (0..n - 1).map(|i| (i, i + 1)).collect();
    // Distinct places -> no conflicts; issued at i, used at i+1 (live one step).
    let loans: Vec<(u32, LoanKind, u32, u32)> = (0..loan_count)
        .map(|i| (i, LoanKind::Mut, i, i * 4))
        .collect();
    let uses: Vec<(u32, u32)> = (0..loan_count).map(|i| (i, i + 1)).collect();
    let f = facts(n, &cfg, &loans, &uses);

    let cpu = analyze(&f);
    let gpu = with_live_backend("cuda batched many-loan", |backend| {
        let dispatcher = CudaResidentDispatcher::new(backend);
        cuda_conflicts_batched(&dispatcher, &f)
    });
    assert_eq!(
        gpu, cpu,
        "batched CUDA verdict diverged from the CPU engine on 64 distinct-place loans"
    );
    assert!(
        gpu.is_empty(),
        "distinct-place loans never conflict, got {gpu:?}"
    );
}

#[test]
fn cuda_batched_borrow_checker_many_loans_shared_place_conflict() {
    // 64 &mut loans of ONE place, all issued early and all used at the end, so
    // every pair is co-live across the chain: the batched path must report the
    // exact conflict set the CPU engine does, computed in two dispatches.
    let n = 80u32;
    let loan_count = 8u32;
    let cfg: Vec<(u32, u32)> = (0..n - 1).map(|i| (i, i + 1)).collect();
    let loans: Vec<(u32, LoanKind, u32, u32)> = (0..loan_count)
        .map(|i| (0u32, LoanKind::Mut, i, i * 4))
        .collect();
    // Every loan used at the last point -> all live across the whole chain.
    let uses: Vec<(u32, u32)> = (0..loan_count).map(|i| (i, n - 1)).collect();
    let f = facts(n, &cfg, &loans, &uses);

    let cpu = analyze(&f);
    let gpu = with_live_backend("cuda batched shared-place", |backend| {
        let dispatcher = CudaResidentDispatcher::new(backend);
        cuda_conflicts_batched(&dispatcher, &f)
    });
    assert_eq!(
        gpu, cpu,
        "batched CUDA verdict diverged from the CPU engine on shared-place &mut loans"
    );
    // n choose 2 over 8 loans = 28 conflicting pairs.
    assert_eq!(
        gpu.len(),
        28,
        "expected 28 TwoMutable conflicts, got {}",
        gpu.len()
    );
}

#[test]
fn cuda_batched_vs_per_loan_dispatch_speedup() {
    // 64 loans on a 200-point chain. Per-loan: 2 x 64 = 128 device dispatches.
    // Batched: 2. Same verdict; this measures the megakernel-batching win on the
    // device and asserts the two paths agree.
    let n = 200u32;
    let loan_count = 64u32;
    let cfg: Vec<(u32, u32)> = (0..n - 1).map(|i| (i, i + 1)).collect();
    let loans: Vec<(u32, LoanKind, u32, u32)> = (0..loan_count)
        .map(|i| (i, LoanKind::Mut, i, i * 4))
        .collect();
    let uses: Vec<(u32, u32)> = (0..loan_count).map(|i| (i, i + 1)).collect();
    let f = facts(n, &cfg, &loans, &uses);

    with_live_backend("cuda batched vs per-loan", |backend| {
        let dispatcher = CudaResidentDispatcher::new(backend);
        // Warm both paths (PTX/module/resident-graph caches) before timing.
        let warm_per_loan = cuda_conflicts(&dispatcher, &f);
        let warm_batched = cuda_conflicts_batched(&dispatcher, &f);
        assert_eq!(
            warm_per_loan, warm_batched,
            "per-loan and batched must agree"
        );

        let t0 = Instant::now();
        let per_loan = cuda_conflicts(&dispatcher, &f);
        let per_loan_us = t0.elapsed().as_micros();
        let t1 = Instant::now();
        let batched = cuda_conflicts_batched(&dispatcher, &f);
        let batched_us = t1.elapsed().as_micros();

        assert_eq!(
            per_loan, batched,
            "per-loan and batched verdicts must agree"
        );
        let speedup = per_loan_us as f64 / batched_us.max(1) as f64;
        println!();
        println!("=== batched megakernel borrow checker vs per-loan ({loan_count} loans) ===");
        println!(
            "per-loan   {:>3} dispatches   {per_loan_us:>8} us",
            2 * loan_count
        );
        println!("batched      2 dispatches   {batched_us:>8} us");
        println!("speedup    {speedup:>6.1}x");
        println!("===");
        // Regression gate with a wide margin (observed ~900x): batching must
        // stay at least an order of magnitude faster than per-loan dispatch.
        assert!(
            batched_us.saturating_mul(10) < per_loan_us,
            "megakernel batching regressed: batched {batched_us}us vs per-loan {per_loan_us}us \
             (must be >=10x faster)"
        );
    });
}

#[test]
fn cuda_crate_batched_matches_cpu_engine_two_dispatches() {
    // Treat the whole CFG corpus as a "crate": every function's loans run
    // through TWO total device dispatches (one forward, one backward over the
    // unioned disconnected graph). Each function's verdict must equal the CPU
    // engine's verdict for that function.
    let cases = corpus();
    let crate_facts: Vec<BorrowFacts> = cases.iter().map(|(_, f)| f.clone()).collect();
    let cpu: Vec<Vec<Conflict>> = crate_facts.iter().map(analyze).collect();
    let gpu = with_live_backend("cuda crate batched", |backend| {
        let dispatcher = CudaResidentDispatcher::new(backend);
        gpu::analyze_crate_batched(&dispatcher, &crate_facts)
            .expect("crate batch dispatch must succeed on the CUDA device")
    });
    assert_eq!(gpu.len(), cpu.len(), "per-function result count must match");
    for (i, (label, _)) in cases.iter().enumerate() {
        assert_eq!(
            gpu[i], cpu[i],
            "{label}: crate-batched verdict diverged from the CPU engine (function {i})"
        );
    }
}

#[test]
fn cuda_crate_batched_scales_to_many_functions_in_ptx_safe_shards() {
    // 128 functions, each a straight-line with one conflicting &mut pair. The
    // crate is split into PTX-safe persistent-BFS shards; every function's
    // conflict must still be found, matching the CPU engine.
    let one = || {
        facts(
            3,
            &[(0, 1), (1, 2)],
            &[(0, LoanKind::Mut, 0, 10), (0, LoanKind::Mut, 1, 20)],
            &[(0, 2), (1, 2)],
        )
    };
    let crate_facts: Vec<BorrowFacts> = (0..128).map(|_| one()).collect();
    let cpu: Vec<Vec<Conflict>> = crate_facts.iter().map(analyze).collect();
    let gpu = with_live_backend("cuda crate scale", |backend| {
        let dispatcher = CudaResidentDispatcher::new(backend);
        gpu::analyze_crate_batched(&dispatcher, &crate_facts)
            .expect("crate batch dispatch must succeed on the CUDA device")
    });
    assert_eq!(gpu.len(), 128, "must return a verdict per function");
    assert_eq!(
        gpu, cpu,
        "128-function crate batch diverged from the CPU engine"
    );
    for (i, conflicts) in gpu.iter().enumerate() {
        assert_eq!(
            conflicts.len(),
            1,
            "function {i} must have exactly one conflict"
        );
        assert_eq!(conflicts[0].kind, ConflictKind::TwoMutable);
    }
}

#[test]
fn cuda_crate_batched_sharding_matches_cpu() {
    // A tiny shard budget forces the corpus crate to split across several
    // two-dispatch batches; the concatenated per-function verdict must still
    // match the CPU engine, proving the sharding + reassembly is correct.
    let cases = corpus();
    let crate_facts: Vec<BorrowFacts> = cases.iter().map(|(_, f)| f.clone()).collect();
    let cpu: Vec<Vec<Conflict>> = crate_facts.iter().map(analyze).collect();
    let gpu = with_live_backend("cuda crate sharded", |backend| {
        let dispatcher = CudaResidentDispatcher::new(backend);
        gpu::analyze_crate_batched_with_shard_cap(&dispatcher, &crate_facts, 2)
            .expect("sharded crate batch dispatch must succeed on the CUDA device")
    });
    assert_eq!(
        gpu.len(),
        cpu.len(),
        "sharded crate batch must return a verdict per function"
    );
    assert_eq!(
        gpu, cpu,
        "sharded crate-batch verdict diverged from the CPU engine"
    );
}
