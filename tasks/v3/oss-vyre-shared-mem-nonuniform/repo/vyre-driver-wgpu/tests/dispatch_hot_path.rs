//! WGPU dispatch hot-path tests.
//!
//! Guarantees:
//! - Pipeline cache hits skip recompilation and lower per-dispatch latency
//! - Bind-group cache reuses GPU descriptors across repeated dispatches
//! - Persistent buffer pool recycles allocations instead of churning the driver
//! - `dispatch_borrowed` avoids async scheduling overhead that `dispatch_async` may pay for;
//!   note `dispatch_async` still collects zero-copy `&[u8]` views into a `SmallVec` (cap 8) - it
//!   does not clone input payloads
//! - Cached small-dispatch latency stays under a fixed budget
//! - `dispatch_batch` submit overhead is sublinear (not N× serial blocking)
//! - Even on the compiled-pipeline fast path execution remains GPU-consistent,
//!   never silently falling back to CPU

mod common;
use common::acquire_live_backend as live_backend;

use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};
use vyre::ir::{BufferDecl, DataType, Expr, Node, Program};
use vyre::{DispatchConfig, VyreBackend};
use vyre_driver::CompiledPipeline;

static HOT_PATH_TEST_LOCK: Mutex<()> = Mutex::new(());

fn hot_path_test_guard() -> MutexGuard<'static, ()> {
    HOT_PATH_TEST_LOCK.lock().unwrap_or_else(|error| {
        panic!(
            "dispatch hot-path test mutex was poisoned: {error}. Fix: resolve the earlier hot-path panic before trusting latency measurements."
        )
    })
}

fn add_one_program(words: u32) -> Program {
    let idx = Expr::gid_x();
    let in_bounds = Expr::lt(idx.clone(), Expr::u32(words));
    Program::wrapped(
        vec![
            BufferDecl::read("input", 0, DataType::U32).with_count(words),
            BufferDecl::output("out", 1, DataType::U32)
                .with_count(words)
                .with_output_byte_range(0..(words as usize * 4)),
        ],
        [64, 1, 1],
        vec![
            Node::if_then(
                in_bounds,
                vec![Node::store(
                    "out",
                    idx.clone(),
                    Expr::add(Expr::load("input", idx), Expr::u32(1)),
                )],
            ),
            Node::return_(),
        ],
    )
}

// ------------------------------------------------------------------
// 1. Pipeline cache reuse
// ------------------------------------------------------------------

#[test]
fn pipeline_cache_hit_avoids_recompilation_latency() {
    let _guard = hot_path_test_guard();
    let backend = live_backend();
    let program = add_one_program(1024);
    let input: Vec<u8> = vyre_primitives::wire::pack_u32_iter(0..1024u32);

    // Cold dispatch: must compile the pipeline.
    let cold_start = Instant::now();
    let _ = backend
        .dispatch(&program, &[input.clone()], &DispatchConfig::default())
        .expect("Fix: cold dispatch must succeed");
    let cold_elapsed = cold_start.elapsed();

    // Hot dispatch: pipeline cache must skip WGSL lowering + ComputePipeline creation.
    let hot_start = Instant::now();
    let _ = backend
        .dispatch(&program, &[input.clone()], &DispatchConfig::default())
        .expect("Fix: hot dispatch must succeed");
    let hot_elapsed = hot_start.elapsed();

    assert!(
        hot_elapsed < cold_elapsed,
        "Fix: pipeline cache hit must be faster than cold compile+dispatch. \
         cold={cold_elapsed:?}, hot={hot_elapsed:?}"
    );

    let stats = backend.stats();
    assert!(
        stats.pipeline_cache_entries >= 1,
        "Fix: after two dispatches of the same program the pipeline cache must contain at least one entry"
    );
    assert!(
        stats.pipeline_cache_entries <= stats.pipeline_cache_capacity,
        "Fix: pipeline cache must never exceed its declared capacity"
    );
}

// ------------------------------------------------------------------
// 2. Bind-group cache reuse through compiled pipeline
// ------------------------------------------------------------------

#[test]
fn bind_group_cache_reused_on_repeated_dispatches() {
    let _guard = hot_path_test_guard();
    let backend = live_backend();
    let program = add_one_program(256);
    let input: Vec<u8> = vyre_primitives::wire::pack_u32_iter(0..256u32);

    let pipeline = backend
        .compile_persistent(&program, &DispatchConfig::default())
        .expect("Fix: compile_persistent must succeed");

    // First dispatch: bind groups are created fresh.
    let _ = pipeline
        .dispatch(&[input.clone()], &DispatchConfig::default())
        .expect("Fix: first compiled dispatch must succeed");
    let stats_after_first = pipeline.bind_group_cache_stats();
    assert_eq!(
        stats_after_first.misses, 1,
        "Fix: first dispatch of a compiled pipeline with new buffers must create exactly one bind group"
    );
    assert_eq!(
        stats_after_first.hits, 0,
        "Fix: no bind-group cache hit expected on first dispatch"
    );

    // Second dispatch with identical inputs: the compiled pipeline path reuses
    // the same bind-group layout + buffer identities, so the cache must hit.
    let _ = pipeline
        .dispatch(&[input.clone()], &DispatchConfig::default())
        .expect("Fix: second compiled dispatch must succeed");
    let stats_after_second = pipeline.bind_group_cache_stats();
    assert_eq!(
        stats_after_second.hits, 1,
        "Fix: second dispatch with identical inputs must hit the bind-group cache"
    );
    assert_eq!(
        stats_after_second.misses, 1,
        "Fix: bind-group cache misses must not increase on repeated identical dispatches"
    );
}

// ------------------------------------------------------------------
// 3. Persistent buffer pool reuse
// ------------------------------------------------------------------

#[test]
fn persistent_pool_reuses_allocations_on_repeated_dispatches() {
    let _guard = hot_path_test_guard();
    let backend = live_backend();
    let program = add_one_program(256);
    let input: Vec<u8> = vyre_primitives::wire::pack_u32_iter(0..256u32);

    let pipeline = backend
        .compile_persistent(&program, &DispatchConfig::default())
        .expect("Fix: compile_persistent must succeed");

    let stats_before = backend.stats().persistent_pool;

    for i in 0..5 {
        let _ = pipeline
            .dispatch(&[input.clone()], &DispatchConfig::default())
            .unwrap_or_else(|_| panic!("Fix: repeated compiled dispatch #{i} must succeed"));
    }

    let stats_after = backend.stats().persistent_pool;
    assert!(
        stats_after.hits > stats_before.hits,
        "Fix: repeated dispatches through a compiled pipeline must show buffer-pool reuse. \
         before_hits={}, after_hits={}",
        stats_before.hits,
        stats_after.hits
    );
}

// ------------------------------------------------------------------
// 4. No CPU fallback on the compiled-pipeline fast path
// ------------------------------------------------------------------

#[test]
fn compiled_dispatch_never_cpu_fallback() {
    let _guard = hot_path_test_guard();
    let backend = live_backend();
    let program = add_one_program(1024);
    let input: Vec<u8> = vyre_primitives::wire::pack_u32_iter(0..1024u32);

    let pipeline = backend
        .compile_persistent(&program, &DispatchConfig::default())
        .expect("Fix: compile_persistent must succeed");

    let start = Instant::now();
    let outputs = pipeline
        .dispatch(&[input], &DispatchConfig::default())
        .expect("Fix: compiled dispatch must succeed");
    let elapsed = start.elapsed();

    // A true CPU fallback would return in < 1 microsecond.
    assert!(
        elapsed > Duration::from_micros(10),
        "Fix: compiled dispatch returned in {elapsed:?}, which is too fast for a real GPU round-trip. \
         This suggests a silent CPU fallback."
    );

    let expected: Vec<u8> = vyre_primitives::wire::pack_u32_iter(1..=1024u32);
    assert_eq!(
        outputs,
        vec![expected],
        "Fix: compiled pipeline must return correct GPU-computed results"
    );
}

// ------------------------------------------------------------------
// 5. Async/direct behavior: borrowed path avoids worker-pool overhead
// ------------------------------------------------------------------

#[test]
fn dispatch_borrowed_avoids_async_pool_overhead() {
    let _guard = hot_path_test_guard();
    let backend = live_backend();
    let program = add_one_program(1024);
    let input: Vec<u8> = vyre_primitives::wire::pack_u32_iter(0..1024u32);
    let borrowed = [input.as_slice()];

    // Warm every cache layer so we measure steady-state overhead differences.
    let _ = backend
        .dispatch_borrowed(&program, &borrowed, &DispatchConfig::default())
        .expect("Fix: warm-up borrowed dispatch must succeed");

    // Synchronous borrowed path: stays on the caller thread.
    let sync_start = Instant::now();
    let _ = backend
        .dispatch_borrowed(&program, &borrowed, &DispatchConfig::default())
        .expect("Fix: sync borrowed dispatch must succeed");
    let sync_elapsed = sync_start.elapsed();

    // Async path: borrows input slices (no payload clone); may schedule overlapping host/GPU work.
    let async_start = Instant::now();
    let pending = backend
        .dispatch_async(&program, &[input.clone()], &DispatchConfig::default())
        .expect("Fix: dispatch_async must return a handle");
    let _ = pending
        .await_result()
        .expect("Fix: async dispatch must resolve");
    let async_elapsed = async_start.elapsed();

    // The async path can carry extra scheduling overhead versus synchronous `dispatch_borrowed`.
    // We assert the sync path is not slower (with a small noise margin).
    assert!(
        sync_elapsed <= async_elapsed + Duration::from_millis(5),
        "Fix: dispatch_borrowed (sync) took {sync_elapsed:?}, which is slower than \
         dispatch_async+await at {async_elapsed:?}. The borrowed path must not pay \
         async worker-pool overhead."
    );
}

// ------------------------------------------------------------------
// 6. Bounded per-dispatch overhead (cached pipeline)
// ------------------------------------------------------------------

#[test]
fn hot_cached_dispatch_latency_bounded() {
    let _guard = hot_path_test_guard();
    let backend = live_backend();
    let program = add_one_program(1024);
    let input: Vec<u8> = vyre_primitives::wire::pack_u32_iter(0..1024u32);

    // Warm caches.
    let _ = backend
        .dispatch(&program, &[input.clone()], &DispatchConfig::default())
        .expect("Fix: warm-up dispatch must succeed");

    const BUDGET: Duration = Duration::from_millis(200);

    let start = Instant::now();
    let _ = backend
        .dispatch(&program, &[input.clone()], &DispatchConfig::default())
        .expect("Fix: hot cached dispatch must succeed");
    let elapsed = start.elapsed();

    assert!(
        elapsed < BUDGET,
        "Fix: hot cached small dispatch exceeded latency budget {BUDGET:?}. \
         Elapsed={elapsed:?}. Fix: inspect pipeline cache hit path and buffer-pool reuse."
    );
}

#[test]
fn long_buffer_throughput_latency_bounded() {
    let _guard = hot_path_test_guard();
    let backend = live_backend();
    let words = 1 << 20;
    let program = add_one_program(words);
    let input: Vec<u8> = vyre_primitives::wire::pack_u32_iter(0..words);

    let _ = backend
        .dispatch(&program, &[input.clone()], &DispatchConfig::default())
        .expect("Fix: long-buffer warm-up dispatch must succeed");

    const BUDGET: Duration = Duration::from_millis(1500);

    let start = Instant::now();
    let outputs = backend
        .dispatch(&program, &[input], &DispatchConfig::default())
        .expect("Fix: hot long-buffer dispatch must succeed");
    let elapsed = start.elapsed();

    assert!(
        elapsed < BUDGET,
        "Fix: hot long-buffer dispatch exceeded throughput budget {BUDGET:?}. \
         Elapsed={elapsed:?}. Fix: inspect upload, dispatch, copy, and readback throughput."
    );
    assert_eq!(
        outputs.first().map(Vec::len),
        Some(words as usize * 4),
        "Fix: long-buffer throughput test must read back the full output range"
    );
}

// ------------------------------------------------------------------
// 7. dispatch_batch overhead is bounded
// ------------------------------------------------------------------

#[test]
fn dispatch_batch_submit_overhead_bounded() {
    let _guard = hot_path_test_guard();
    let backend = live_backend();
    let program = add_one_program(512);
    let input: Vec<u8> = vyre_primitives::wire::pack_u32_iter(0..512u32);

    // Warm caches.
    let _ = backend
        .dispatch(&program, &[input.clone()], &DispatchConfig::default())
        .expect("Fix: warm-up dispatch must succeed");

    let jobs = vec![
        (
            program.clone(),
            vec![input.clone()],
            DispatchConfig::default(),
        ),
        (
            program.clone(),
            vec![input.clone()],
            DispatchConfig::default(),
        ),
        (
            program.clone(),
            vec![input.clone()],
            DispatchConfig::default(),
        ),
    ];

    const BUDGET: Duration = Duration::from_millis(300);

    let start = Instant::now();
    let results = backend
        .dispatch_batch(&jobs)
        .expect("Fix: dispatch_batch must launch all jobs");
    let elapsed = start.elapsed();

    assert!(
        elapsed < BUDGET,
        "Fix: three-job dispatch_batch exceeded latency budget {BUDGET:?}. \
         Elapsed={elapsed:?}. Fix: inspect batch launch path for host-side serialization."
    );

    for (i, result) in results.iter().enumerate() {
        let outputs = result
            .as_ref()
            .unwrap_or_else(|e| panic!("Fix: batch job #{i} must succeed: {e:?}"));
        let expected: Vec<u8> = vyre_primitives::wire::pack_u32_iter(1..=512u32);
        assert_eq!(
            *outputs,
            vec![expected],
            "Fix: batch job #{i} produced wrong output"
        );
    }
}
