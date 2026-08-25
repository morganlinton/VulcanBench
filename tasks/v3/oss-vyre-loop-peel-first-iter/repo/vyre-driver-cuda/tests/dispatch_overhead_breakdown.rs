//! Per-phase dispatch-overhead breakdown.
//!
//! Measures end-to-end host-to-host latency on the simplest possible CUDA
//! dispatch and attributes the time to four phases:
//!
//!   1. **`backend_acquire_ns`**  -  CudaBackend::acquire (one-time, includes
//!      device probe + module-cache init + transient-pool bootstrap).
//!   2. **`compile_ns`**  -  first-call PTX compilation + module load (cold).
//!   3. **`compile_warm_ns`**  -  second-call dispatch with the module cached
//!      (warm  -  this is the per-dispatch floor for repeat-dispatch workloads).
//!   4. **`steady_state_ns`**  -  dispatch #3 onward, the per-dispatch floor we
//!      care about for the latency-bound corner of the bench.
//!
//! Outputs the breakdown via `println!` so cargo test --nocapture captures
//! the numbers. Asserts conservative ceilings so the test fails when latency
//! regresses past obviously-bad thresholds.

use std::time::Instant;

use vyre_driver::DispatchConfig;
use vyre_driver_cuda::CudaBackend;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// The smallest possible Program: one thread, one store of a constant. This
/// minimizes every per-dispatch cost EXCEPT the host-side overhead, so the
/// measurement attributes overhead correctly. A larger Program would dilute
/// the host-side overhead with kernel-execute time.
fn no_op_program() -> Program {
    Program::wrapped(
        vec![BufferDecl::storage("out", 0, BufferAccess::ReadWrite, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::u32(0))],
    )
}

fn assert_noop_output(outputs: &[Vec<u8>], phase: &str) {
    let expected = 0u32.to_le_bytes();
    assert_eq!(
        outputs.len(),
        1,
        "{phase}: CUDA no-op dispatch must return exactly one output buffer"
    );
    assert_eq!(
        outputs[0].as_slice(),
        expected.as_slice(),
        "{phase}: CUDA no-op dispatch must write the expected zero word"
    );
}

#[test]
fn dispatch_overhead_breakdown_reports_per_phase_latency() {
    let backend_t0 = Instant::now();
    let backend = CudaBackend::acquire()
        .expect("Fix: CUDA backend acquisition must succeed on the GPU-required test host.");
    let backend_acquire_ns = backend_t0.elapsed().as_nanos();

    let program = no_op_program();
    let inputs: Vec<Vec<u8>> = vec![vec![0u8; 4]];
    let config = DispatchConfig::default();

    // Cold dispatch: includes PTX compile + module load + first-launch overhead.
    let cold_t0 = Instant::now();
    let cold_outputs = backend
        .dispatch(&program, &inputs, &config)
        .expect("Fix: cuda no-op dispatch must succeed");
    let cold_ns = cold_t0.elapsed().as_nanos();
    assert_noop_output(&cold_outputs, "cold dispatch");

    // Warm dispatch (module cached, transient pool warm): the per-dispatch
    // floor for repeat-dispatch workloads.
    let warm_t0 = Instant::now();
    let warm_outputs = backend
        .dispatch(&program, &inputs, &config)
        .expect("warm dispatch must succeed");
    let warm_ns = warm_t0.elapsed().as_nanos();
    assert_noop_output(&warm_outputs, "warm dispatch");

    // Steady state: average over 100 dispatches after warmup. Per-dispatch
    // wall-clock is what the latency-bound corner of the bench cares about.
    const STEADY_RUNS: u32 = 100;
    let steady_t0 = Instant::now();
    for _ in 0..STEADY_RUNS {
        let steady_outputs = backend
            .dispatch(&program, &inputs, &config)
            .expect("steady-state dispatch must succeed");
        assert_noop_output(&steady_outputs, "steady-state dispatch");
    }
    let steady_total_ns = steady_t0.elapsed().as_nanos();
    let steady_per_dispatch_ns = steady_total_ns / u128::from(STEADY_RUNS);

    println!();
    println!("=== CUDA dispatch overhead breakdown ===");
    println!("backend_acquire_ns           {backend_acquire_ns:>12}  (one-time)");
    println!("cold_first_dispatch_ns       {cold_ns:>12}  (incl. PTX compile + module load)");
    println!("warm_second_dispatch_ns      {warm_ns:>12}  (module cached)");
    println!("steady_state_per_dispatch_ns {steady_per_dispatch_ns:>12}  ({STEADY_RUNS}-run avg)");
    println!("===");

    // Conservative ceilings  -  fail the test if latency regresses past these.
    // The numbers are the headline budget for the latency-bound corner.
    assert!(
        backend_acquire_ns < 5_000_000_000, // 5 seconds
        "backend acquire must complete in under 5s; observed {backend_acquire_ns}ns. \
         A regression here means PTX target probing or device init has broken."
    );
    assert!(
        cold_ns < 500_000_000, // 500 ms
        "cold first dispatch must complete in under 500ms; observed {cold_ns}ns. \
         A regression here means PTX compile or module load is broken."
    );
    assert!(
        warm_ns < 50_000_000, // 50 ms
        "warm dispatch must complete in under 50ms; observed {warm_ns}ns. \
         A regression here means module cache lookup or transient-pool reuse is broken."
    );
    assert!(
        steady_per_dispatch_ns < 10_000_000, // 10 ms
        "steady-state per-dispatch must complete in under 10ms; observed \
         {steady_per_dispatch_ns}ns. A regression here means the dispatch hot path picked \
         up a per-call allocation, lock contention, or readback stall."
    );
}
