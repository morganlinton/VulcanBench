//! Fine-grained CUDA dispatch overhead attribution.
//!
//! `dispatch_overhead_breakdown` reports the steady-state per-dispatch wall
//! time; this test splits that wall time into its phases (host enqueue vs
//! completion wait vs device kernel) so optimization targets the real headroom
//! instead of guessing. A no-op program isolates the fixed per-dispatch cost:
//! the GPU work is ~nothing, so whatever remains is overhead we can cut.

use vyre_driver::DispatchConfig;
use vyre_driver_cuda::CudaBackend;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

fn no_op_program() -> Program {
    Program::wrapped(
        vec![BufferDecl::storage("out", 0, BufferAccess::ReadWrite, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::u32(0))],
    )
}

#[test]
fn cuda_steady_state_phase_attribution() {
    let backend = CudaBackend::acquire()
        .expect("Fix: CUDA backend acquisition must succeed on the GPU-required test host.");
    let program = no_op_program();
    let input = vec![0u8; 4];
    let inputs: [&[u8]; 1] = [input.as_slice()];
    let config = DispatchConfig::default();

    // Warm the PTX/module/launch-resource caches so the steady-state loop below
    // measures only the recurring per-dispatch cost.
    for _ in 0..3 {
        let _ = backend
            .dispatch_borrowed_timed(&program, &inputs, &config)
            .expect("warm dispatch must succeed");
    }

    const RUNS: u64 = 200;
    let (mut wall, mut enqueue, mut wait, mut device) = (0u64, 0u64, 0u64, 0u64);
    let mut enqueue_samples = 0u64;
    let mut wait_samples = 0u64;
    let mut device_samples = 0u64;
    for _ in 0..RUNS {
        let r = backend
            .dispatch_borrowed_timed(&program, &inputs, &config)
            .expect("steady-state dispatch must succeed");
        wall += r.wall_ns;
        if let Some(e) = r.enqueue_ns {
            enqueue += e;
            enqueue_samples += 1;
        }
        if let Some(w) = r.wait_ns {
            wait += w;
            wait_samples += 1;
        }
        if let Some(d) = r.device_ns {
            device += d;
            device_samples += 1;
        }
    }

    let div = |sum: u64, n: u64| if n == 0 { 0 } else { sum / n };
    println!();
    println!("=== CUDA steady-state dispatch phase attribution ({RUNS} runs) ===");
    println!("wall_ns/dispatch     {:>10}", wall / RUNS);
    println!(
        "enqueue_ns/dispatch  {:>10}  (host prep + launch enqueue, {enqueue_samples} samples)",
        div(enqueue, enqueue_samples)
    );
    println!(
        "wait_ns/dispatch     {:>10}  (sync + readback, {wait_samples} samples)",
        div(wait, wait_samples)
    );
    if device_samples > 0 {
        println!(
            "device_ns/dispatch   {:>10}  (GPU kernel, {device_samples} samples)",
            div(device, device_samples)
        );
    } else {
        println!("device_ns/dispatch          n/a  (no device timer exposed on this path)");
    }
    println!("===");
}

/// Quantifies the prep overhead that the plain `dispatch()` API redoes every
/// call but a compiled pipeline computes once: PTX/module cache-key derivation
/// (which normalizes + hashes the whole program), `prepare_host_dispatch`, and
/// collective lowering. The gap is the achievable win from caching prep in the
/// common dispatch path.
#[test]
fn cuda_compiled_pipeline_vs_plain_dispatch_overhead() {
    let backend = CudaBackend::acquire()
        .expect("Fix: CUDA backend acquisition must succeed on the GPU-required test host.");
    let program = no_op_program();
    let inputs: Vec<Vec<u8>> = vec![vec![0u8; 4]];
    let config = DispatchConfig::default();

    for _ in 0..3 {
        let _ = backend
            .dispatch(&program, &inputs, &config)
            .expect("warm plain");
    }
    const RUNS: u64 = 200;
    let t0 = std::time::Instant::now();
    for _ in 0..RUNS {
        let _ = backend
            .dispatch(&program, &inputs, &config)
            .expect("plain dispatch");
    }
    let plain_ns = u64::try_from(t0.elapsed().as_nanos()).unwrap_or(u64::MAX) / RUNS;

    let pipeline = backend
        .compile_native(&program, &config)
        .expect("compile_native must succeed");
    for _ in 0..3 {
        let _ = pipeline.dispatch(&inputs, &config).expect("warm pipeline");
    }
    let t1 = std::time::Instant::now();
    for _ in 0..RUNS {
        let _ = pipeline
            .dispatch(&inputs, &config)
            .expect("pipeline dispatch");
    }
    let pipe_ns = u64::try_from(t1.elapsed().as_nanos()).unwrap_or(u64::MAX) / RUNS;

    // Varying inputs defeat the materialized-output cache, so this is the
    // honest repeated-dispatch win for a workload like the borrow checker:
    // same program shape, different buffer data each call.
    let mut varied: Vec<Vec<Vec<u8>>> = (0..RUNS)
        .map(|i| vec![(i as u32).to_le_bytes().to_vec()])
        .collect();
    let t2 = std::time::Instant::now();
    for v in &varied {
        let _ = pipeline
            .dispatch(v, &config)
            .expect("pipeline varied dispatch");
    }
    let pipe_varied_ns = u64::try_from(t2.elapsed().as_nanos()).unwrap_or(u64::MAX) / RUNS;
    let t3 = std::time::Instant::now();
    for v in &mut varied {
        let _ = backend
            .dispatch(&program, v, &config)
            .expect("plain varied dispatch");
    }
    let plain_varied_ns = u64::try_from(t3.elapsed().as_nanos()).unwrap_or(u64::MAX) / RUNS;

    // Isolate the cache-key derivation cost (normalize + hash the whole program)
    // that plain dispatch redoes every call to look up the PTX/module caches.
    const KRUNS: u64 = 1000;
    let t4 = std::time::Instant::now();
    for _ in 0..KRUNS {
        let _ =
            vyre_driver::pipeline::try_normalized_program_cache_digest(&program).expect("digest");
        let _ = vyre_driver::program_vsa_fingerprint_words(&program);
    }
    let digest_ns = u64::try_from(t4.elapsed().as_nanos()).unwrap_or(u64::MAX) / KRUNS;

    println!();
    println!("=== plain dispatch() vs compiled pipeline ({RUNS} runs) ===");
    println!("plain_dispatch_ns/call          {plain_ns:>10}  (identical inputs)");
    println!("compiled_pipeline_ns/call       {pipe_ns:>10}  (identical inputs, output-cache hit)");
    println!("plain_dispatch_varied_ns/call   {plain_varied_ns:>10}  (varying inputs)");
    println!(
        "compiled_pipeline_varied_ns     {pipe_varied_ns:>10}  (varying inputs, graph replay)"
    );
    println!("cache_key_digest_ns/call        {digest_ns:>10}  (normalize+hash, redone every plain call)");
    let speedup = plain_varied_ns as f64 / pipe_varied_ns.max(1) as f64;
    println!("--> varying-input speedup       {speedup:>9.2}x  (the honest repeated-dispatch win)");
    println!("===");
}
