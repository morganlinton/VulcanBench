//! End-to-end + scaling bench: persistent-resident pipeline on CUDA.
//!
//! Runs `gpu_pipeline_resident` (single encode, persistent buffers,
//! all four passes share GPU state) on real CUDA hardware and
//! compares against the foundation CPU pipeline.

#![cfg(test)]

mod common;

use common::live_backend;
use std::{thread, time::Instant};

use vyre::ir::{Expr, Node, Program};
use vyre_driver_cuda::CudaOptimizerDispatcher;
use vyre_self_substrate::optimizer::pipeline_resident::gpu_pipeline_resident;

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

fn synthetic_wide_program(n: usize) -> Program {
    let mut entry: Vec<Node> = Vec::with_capacity(n + 1);
    for i in 0..n {
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

/// Tree shape  -  each let depends on its parent index `i/2`. Diameter
/// is `log2(n)`, so DCE BFS converges in O(log n) iterations even
/// though the program has n lets. This is the parallel-friendly
/// fixture: chain-shaped Programs are bound by per-iter sequential
/// cost, but tree-shaped programs amortise.
fn synthetic_tree_program(n: usize) -> Program {
    assert!(n >= 2, "tree fixture needs at least 2 lets");
    let mut entry: Vec<Node> = Vec::with_capacity(n + 1);
    entry.push(Node::let_bind(
        "v0",
        Expr::mul(Expr::add(Expr::u32(1), Expr::u32(2)), Expr::u32(3)),
    ));
    for i in 1..n {
        let parent = format!("v{}", i / 2);
        let value = Expr::add(Expr::var(parent), Expr::u32(((i % 7) + 1) as u32));
        entry.push(Node::let_bind(format!("v{i}"), value));
    }
    let last = format!("v{}", n - 1);
    entry.push(Node::store("buf", Expr::u32(0), Expr::var(last)));
    Program::wrapped(Vec::new(), [1, 1, 1], entry)
}

fn run_cpu_pipeline(p: Program) -> Program {
    use vyre_foundation::optimizer::passes::algebraic::canonicalize_engine::run as cpu_canonicalize;
    use vyre_foundation::optimizer::passes::fusion_cse::dce::engine::dce as cpu_dce;
    let p = cpu_canonicalize(p);
    let p = vyre_foundation::optimizer::pre_lowering::optimize(p);
    cpu_dce(p)
}

#[test]
fn cuda_persistent_pipeline_correctness() {
    // Smoke test: a simple program goes through the persistent path
    // and produces a correct result.
    let backend = live_backend();
    let dispatcher = CudaOptimizerDispatcher::new(&backend);

    // let dead = 99
    // let live = 1 + 2     // foldable to 3
    // store buf 0 (3 + live)   // canon swaps to (live + 3)
    let p = Program::wrapped(
        Vec::new(),
        [1, 1, 1],
        vec![
            Node::let_bind("dead", Expr::u32(99)),
            Node::let_bind("live", Expr::add(Expr::u32(1), Expr::u32(2))),
            Node::store(
                "buf",
                Expr::u32(0),
                Expr::add(Expr::u32(3), Expr::var("live")),
            ),
        ],
    );

    let out = gpu_pipeline_resident(p, &dispatcher).expect("persistent pipeline runs");
    let body: Vec<Node> = match out.entry() {
        [Node::Region { body, .. }] => body.as_ref().clone(),
        entry => entry.to_vec(),
    };

    // After the full 6-pass pipeline + const-prop + post-prop fold:
    //   `dead` is dropped by DCE (unused).
    //   `live = 1 + 2` folds to `live = 3`.
    //   `store buf 0 (3 + live)` after canon becomes `(live + 3)`,
    //     then const-prop turns Var(live) into LitU32(3), then the
    //     post-prop folder collapses `LitU32(3) + LitU32(3)` to
    //     LitU32(6). DCE drops `let live` (its only use went away).
    // Final body: a single `store buf 0 LitU32(6)`.
    assert_eq!(
        body.len(),
        1,
        "all lets dead after const-prop. body={body:?}"
    );
    match &body[0] {
        Node::Store { value, .. } => {
            assert!(
                matches!(value, Expr::LitU32(6)),
                "expected LitU32(6); got {value:?}"
            );
        }
        other => panic!("expected Store; got {other:?}"),
    }
}

#[test]
fn cuda_persistent_pipeline_reuses_static_buffers_on_warm_run() {
    let backend = live_backend();
    let dispatcher = CudaOptimizerDispatcher::new(&backend);
    let p = synthetic_wide_program(1_000);

    backend.reset_telemetry();
    let _ = gpu_pipeline_resident(p.clone(), &dispatcher).expect("cold resident pipeline");
    let cold_h2d = backend.telemetry_snapshot().host_to_device_bytes;
    assert!(
        cold_h2d > 0,
        "Fix: cold CUDA resident optimizer run must report immutable + mutable H2D traffic."
    );

    backend.reset_telemetry();
    let _ = gpu_pipeline_resident(p, &dispatcher).expect("warm resident pipeline");
    let warm_h2d = backend.telemetry_snapshot().host_to_device_bytes;
    assert!(
        warm_h2d > 0,
        "Fix: warm CUDA resident optimizer run must still report mutable scratch H2D traffic."
    );
    assert!(
        warm_h2d < cold_h2d,
        "Fix: warm CUDA resident optimizer run must reuse immutable static buffers; cold_h2d={cold_h2d}, warm_h2d={warm_h2d}."
    );
}

#[test]
fn cuda_persistent_pipeline_scaling_bench() {
    thread::Builder::new()
        .name("cuda_persistent_pipeline_scaling_bench_worker".to_string())
        .stack_size(32 * 1024 * 1024)
        .spawn(cuda_persistent_pipeline_scaling_bench_body)
        .expect("scaling bench worker thread must spawn")
        .join()
        .expect("scaling bench worker thread must complete");
}

fn cuda_persistent_pipeline_scaling_bench_body() {
    let backend = live_backend();
    let dispatcher = CudaOptimizerDispatcher::new(&backend);

    println!("\n=== CUDA persistent-pipeline scaling vs CPU ===");
    println!(
        "{:>8} | {:>8} | {:>14} | {:>14} | {:>10} | {:>8} | {:>10} | {:>10} | {:>6} | {:>7} | {:>7} | {:>8}",
        "shape",
        "n",
        "gpu_us",
        "cpu_us",
        "gpu/cpu",
        "launch",
        "h2d_kib",
        "d2h_kib",
        "sync",
        "utilbp",
        "wastebp",
        "denbp"
    );
    println!("{}", "-".repeat(146));

    let mut tree_50k_ratio = None;

    for &n in &[10usize, 100, 1000, 5000, 10_000, 20_000, 50_000] {
        for (shape, build) in [
            ("chain", synthetic_chain_program as fn(usize) -> Program),
            ("wide", synthetic_wide_program as fn(usize) -> Program),
            ("tree", synthetic_tree_program as fn(usize) -> Program),
        ] {
            // Skip chain past n=1000. Diameter scales linearly and
            // each BFS iter does sequential per-source work, so the
            // pathological case explodes; wide and tree remain
            // meaningful at scale because their diameter is O(1) /
            // O(log n).
            if shape == "chain" && n >= 5000 {
                continue;
            }
            let p = build(n);
            // Warmup: cache pipeline compile + warm CUDA driver paths.
            let _ = gpu_pipeline_resident(p.clone(), &dispatcher).expect("warmup");
            let _ = run_cpu_pipeline(p.clone());

            backend.reset_telemetry();
            let t_gpu = Instant::now();
            let _ = gpu_pipeline_resident(p.clone(), &dispatcher).expect("gpu pipeline");
            let gpu_us = t_gpu.elapsed().as_micros();
            let telemetry = backend.telemetry_snapshot();

            let t_cpu = Instant::now();
            let _ = run_cpu_pipeline(p);
            let cpu_us = t_cpu.elapsed().as_micros();

            let ratio = if cpu_us == 0 {
                f64::INFINITY
            } else {
                gpu_us as f64 / cpu_us as f64
            };
            if shape == "tree" && n == 50_000 {
                tree_50k_ratio = Some(ratio);
            }
            println!(
                "{:>8} | {:>8} | {:>14} | {:>14} | {:>10.2}x | {:>8} | {:>10} | {:>10} | {:>6} | {:>7} | {:>7} | {:>8}",
                shape,
                n,
                gpu_us,
                cpu_us,
                ratio,
                telemetry.kernel_launches,
                telemetry.host_to_device_bytes / 1024,
                telemetry.readback_bytes / 1024,
                telemetry.sync_points,
                telemetry.logical_thread_utilization_bps,
                telemetry.logical_thread_waste_bps,
                telemetry.logical_elements_per_thread_slot_bps
            );
            assert!(
                telemetry.kernel_launches > 0,
                "Fix: persistent CUDA pipeline scaling evidence must expose launch count for {shape}/{n}."
            );
            assert!(
                telemetry.host_to_device_bytes > 0,
                "Fix: persistent CUDA pipeline scaling evidence must expose H2D bytes for {shape}/{n}."
            );
            assert!(
                telemetry.readback_bytes > 0,
                "Fix: persistent CUDA pipeline scaling evidence must expose final readback bytes for {shape}/{n}."
            );
            assert!(
                telemetry.sync_points > 0,
                "Fix: persistent CUDA pipeline scaling evidence must expose synchronization pressure for {shape}/{n}."
            );
            assert!(
                telemetry.sync_points <= 3,
                "Fix: persistent CUDA pipeline must keep resident orchestration sync-collapsed; observed {} sync point(s) for {shape}/{n}, expected <= 3.",
                telemetry.sync_points
            );
            assert!(
                telemetry.logical_thread_utilization_bps > 0,
                "Fix: persistent CUDA pipeline scaling evidence must expose non-zero logical thread utilization for {shape}/{n}."
            );
            assert!(
                telemetry.logical_thread_waste_bps <= 10_000,
                "Fix: persistent CUDA pipeline waste telemetry must stay in basis points for {shape}/{n}."
            );
            assert!(
                telemetry.logical_elements_per_thread_slot_bps > 0,
                "Fix: persistent CUDA pipeline scaling evidence must expose logical element density for {shape}/{n}."
            );
        }
    }
    let tree_50k_ratio = tree_50k_ratio
        .expect("Fix: scaling bench must include the 50k tree release-performance fixture.");
    assert!(
        tree_50k_ratio < 0.50,
        "Fix: CUDA resident tree-50k release fixture must stay at least 2x faster than CPU; observed gpu/cpu={tree_50k_ratio:.2}x."
    );
    println!();
}
