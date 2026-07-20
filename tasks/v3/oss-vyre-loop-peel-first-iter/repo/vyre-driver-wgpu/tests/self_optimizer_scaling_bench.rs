//! Scaling bench: GPU optimizer wall-clock vs CPU optimizer at sizes
//! 10 / 100 / 1000 Expr instances.
//!
//! Runs the full GPU stack (canonicalize → const-fold → DCE) and the
//! equivalent CPU stack (foundation::canonicalize + const_fold + dce)
//! on synthetic Programs of increasing size. Honest comparison  -
//! single-thread sequential GPU kernel today, so small sizes lose to
//! CPU on dispatch overhead. The interesting axis is whether the GPU
//! line stays flat (constant overhead) while CPU grows with input
//! size, foreshadowing the win once the kernels parallelize via
//! level_wave.
//!
//! This is honest measurement, not a flex. The numbers print in the
//! test output; treat them as the raw signal for where the next
//! optimization work goes.

#![cfg(test)]

use std::time::Instant;

use vyre::ir::{Expr, Node, Program};
use vyre::{DispatchConfig, VyreBackend};
use vyre_driver_wgpu::WgpuBackend;
use vyre_self_substrate::optimizer::canonicalize_via_encoded::gpu_canonicalize;
use vyre_self_substrate::optimizer::const_fold_via_encoded::gpu_const_fold;
use vyre_self_substrate::optimizer::dce_via_encoded::gpu_dce;
use vyre_self_substrate::optimizer::dispatcher::{DispatchError, OptimizerDispatcher};

const CPU_ORACLE_STACK_BYTES: usize = 64 * 1024 * 1024;

struct WgpuOptimizerDispatcher<'a> {
    backend: &'a WgpuBackend,
}

impl<'a> OptimizerDispatcher for WgpuOptimizerDispatcher<'a> {
    fn dispatch(
        &self,
        program: &Program,
        inputs: &[Vec<u8>],
        grid_override: Option<[u32; 3]>,
    ) -> Result<Vec<Vec<u8>>, DispatchError> {
        let mut config = DispatchConfig::default();
        config.grid_override = grid_override;
        VyreBackend::dispatch(self.backend, program, inputs, &config)
            .map_err(|err| DispatchError::BackendError(err.to_string()))
    }
}

/// Build a synthetic chain Program with `n` `let`s, each computing
/// `(prev + small_lit) * other_small_lit`. Linear dependency between
/// lets: depth in the Node graph is `n`, but the Expr arena has
/// shallow depth (~2 per let) since `Var(prev)` is a leaf. Worst-case
/// fixture for level parallelism within a single pass.
fn synthetic_chain_program(n: usize) -> Program {
    let mut entry: Vec<Node> = Vec::with_capacity(n + 1);
    for i in 0..n {
        let value = if i == 0 {
            Expr::mul(Expr::add(Expr::u32(1), Expr::u32(2)), Expr::u32(3))
        } else {
            let prev = format!("v{}", i - 1);
            Expr::mul(Expr::add(Expr::u32(5), Expr::var(prev)), Expr::u32(2))
        };
        entry.push(Node::let_bind(format!("v{i}"), value));
    }
    let last = format!("v{}", n.saturating_sub(1));
    entry.push(Node::store("buf", Expr::u32(0), Expr::var(last)));
    Program::wrapped(Vec::new(), [1, 1, 1], entry)
}

/// Build a synthetic wide Program with `n` independent `let`s, each
/// computing `(small_lit + small_lit) * small_lit` with no cross-let
/// data dependency. Best-case fixture for parallel kernels: every
/// pass can act on every let in parallel without ordering.
fn synthetic_wide_program(n: usize) -> Program {
    let mut entry: Vec<Node> = Vec::with_capacity(n + 1);
    for i in 0..n {
        // Each let's RHS uses literals only, so const-fold collapses
        // every one independently. canonicalize: each is already
        // literal-on-right after construction. pattern-match: no
        // identity hits. DCE: drops every let except the last (read
        // by the store).
        let value = Expr::mul(
            Expr::add(
                Expr::u32(((i % 7) + 1) as u32),
                Expr::u32(((i % 13) + 1) as u32),
            ),
            Expr::u32(((i % 5) + 1) as u32),
        );
        entry.push(Node::let_bind(format!("v{i}"), value));
    }
    let last = format!("v{}", n.saturating_sub(1));
    entry.push(Node::store("buf", Expr::u32(0), Expr::var(last)));
    Program::wrapped(Vec::new(), [1, 1, 1], entry)
}

fn run_gpu_pipeline(p: Program, dispatcher: &dyn OptimizerDispatcher) -> Program {
    let p = gpu_canonicalize(p, dispatcher).expect("canonicalize");
    let p = gpu_const_fold(p, dispatcher).expect("const-fold");
    gpu_dce(p, dispatcher).expect("dce")
}

fn run_cpu_pipeline(p: Program) -> Program {
    use vyre_foundation::optimizer::passes::algebraic::canonicalize_engine::run as cpu_canonicalize;
    use vyre_foundation::optimizer::passes::fusion_cse::dce::engine::dce as cpu_dce;
    let p = cpu_canonicalize(p);
    // Foundation has multi-step const-fold; the Canonicalize pass's
    // engine runs idempotently. Use the public optimize entry as a
    // proxy for the broader CPU pipeline.
    let p = vyre_foundation::optimizer::pre_lowering::optimize(p);
    cpu_dce(p)
}

fn run_cpu_pipeline_on_bench_stack(p: Program) -> Program {
    std::thread::Builder::new()
        .name("wgpu-scaling-bench-cpu-oracle".to_string())
        .stack_size(CPU_ORACLE_STACK_BYTES)
        .spawn(move || run_cpu_pipeline(p))
        .expect("Fix: scaling bench must be able to spawn the CPU oracle worker thread")
        .join()
        .expect("Fix: CPU optimizer oracle must not panic on scaling bench fixtures")
}

fn time_cpu_pipeline_on_bench_stack(p: Program) -> u128 {
    std::thread::Builder::new()
        .name("wgpu-scaling-bench-cpu-timer".to_string())
        .stack_size(CPU_ORACLE_STACK_BYTES)
        .spawn(move || {
            let t_cpu = Instant::now();
            let _ = run_cpu_pipeline(p);
            t_cpu.elapsed().as_micros()
        })
        .expect("Fix: scaling bench must be able to spawn the timed CPU oracle worker thread")
        .join()
        .expect("Fix: timed CPU optimizer oracle must not panic on scaling bench fixtures")
}

fn bench_one(
    label: &str,
    fixtures: &[usize],
    build: impl Fn(usize) -> Program,
    dispatcher: &WgpuOptimizerDispatcher<'_>,
) {
    println!("\n=== {label} ===");
    println!(
        "{:>8} | {:>14} | {:>14} | {:>10}",
        "n", "gpu_us", "cpu_us", "gpu/cpu"
    );
    println!("{}", "-".repeat(56));
    for &n in fixtures {
        let p = build(n);
        let _ = run_gpu_pipeline(p.clone(), dispatcher);
        let _ = run_cpu_pipeline_on_bench_stack(p.clone());

        let t_gpu = Instant::now();
        let _ = run_gpu_pipeline(p.clone(), dispatcher);
        let gpu_us = t_gpu.elapsed().as_micros();

        let cpu_us = time_cpu_pipeline_on_bench_stack(p);

        let ratio = if cpu_us == 0 {
            f64::INFINITY
        } else {
            gpu_us as f64 / cpu_us as f64
        };
        println!(
            "{:>8} | {:>14} | {:>14} | {:>10.2}x",
            n, gpu_us, cpu_us, ratio
        );
    }
    println!();
}

#[test]
fn scaling_bench_gpu_vs_cpu_pipeline() {
    let backend = WgpuBackend::acquire().expect("WgpuBackend acquire");
    let dispatcher = WgpuOptimizerDispatcher { backend: &backend };

    bench_one(
        "wgpu chain fixture (depth-bound, worst case for parallelism)",
        &[10usize, 100, 1000],
        synthetic_chain_program,
        &dispatcher,
    );
    bench_one(
        "wgpu wide fixture (independent computations, parallelism-friendly)",
        &[10usize, 100, 1000],
        synthetic_wide_program,
        &dispatcher,
    );
}
