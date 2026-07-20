//! Scaling bench: GPU optimizer wall-clock on real CUDA hardware vs
//! CPU optimizer pipeline at sizes 10 / 100 / 1000 Expr instances.
//! Mirrors the wgpu scaling bench. Honest single-rep measurement.

#![cfg(test)]

mod common;

use common::CudaOptimizerDispatcher;
use std::{thread, time::Instant};

use vyre::ir::{Expr, Node, Program};
use vyre_driver_cuda::CudaBackend;
use vyre_self_substrate::optimizer::canonicalize_via_encoded::gpu_canonicalize;
use vyre_self_substrate::optimizer::const_fold_via_encoded::gpu_const_fold;
use vyre_self_substrate::optimizer::dce_via_encoded::gpu_dce;
use vyre_self_substrate::optimizer::dispatcher::OptimizerDispatcher;

fn synthetic_program(n: usize) -> Program {
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

fn run_gpu_pipeline(p: Program, dispatcher: &dyn OptimizerDispatcher) -> Program {
    let p = gpu_canonicalize(p, dispatcher).expect("canonicalize");
    let p = gpu_const_fold(p, dispatcher).expect("const-fold");
    gpu_dce(p, dispatcher).expect("dce")
}

fn run_cpu_pipeline(p: Program) -> Program {
    use vyre_foundation::optimizer::passes::algebraic::canonicalize_engine::run as cpu_canonicalize;
    use vyre_foundation::optimizer::passes::fusion_cse::dce::engine::dce as cpu_dce;
    let p = cpu_canonicalize(p);
    let p = vyre_foundation::optimizer::pre_lowering::optimize(p);
    cpu_dce(p)
}

#[test]
fn cuda_scaling_bench_gpu_vs_cpu_pipeline() {
    thread::Builder::new()
        .name("cuda_scaling_bench_gpu_vs_cpu_pipeline_worker".into())
        .stack_size(32 * 1024 * 1024)
        .spawn(cuda_scaling_bench_gpu_vs_cpu_pipeline_body)
        .expect("spawn cuda scaling bench worker with expanded stack")
        .join()
        .expect("cuda scaling bench worker panicked");
}

fn cuda_scaling_bench_gpu_vs_cpu_pipeline_body() {
    let backend = CudaBackend::acquire().expect("CudaBackend acquire");
    let dispatcher = CudaOptimizerDispatcher { backend: &backend };

    println!("\n=== self-hosted CUDA optimizer scaling vs CPU pipeline ===");
    println!(
        "{:>8} | {:>14} | {:>14} | {:>10}",
        "n", "gpu_us", "cpu_us", "gpu/cpu"
    );
    println!("{}", "-".repeat(56));

    for &n in &[10usize, 100, 1000] {
        let p = synthetic_program(n);

        // Warm up both paths.
        let _ = run_gpu_pipeline(p.clone(), &dispatcher);
        let _ = run_cpu_pipeline(p.clone());

        let t_gpu = Instant::now();
        let _ = run_gpu_pipeline(p.clone(), &dispatcher);
        let gpu_us = t_gpu.elapsed().as_micros();

        let t_cpu = Instant::now();
        let _ = run_cpu_pipeline(p);
        let cpu_us = t_cpu.elapsed().as_micros();

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
