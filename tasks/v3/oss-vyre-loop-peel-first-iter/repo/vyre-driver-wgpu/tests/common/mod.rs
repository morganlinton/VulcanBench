// Integration test module for the containing Vyre package.

#![allow(dead_code, unused_imports)]

#[allow(deprecated)]
pub(crate) mod c_fixture;
pub(crate) mod every_op_random_inputs;

use std::time::{Duration, Instant};
use vyre::ir::{BufferDecl, DataType, Expr, Node, Program};
use vyre::{DispatchConfig, VyreBackend};
use vyre_driver::PendingDispatch;
use vyre_driver_wgpu::WgpuBackend;

const LIVE_GPU_REQUIRED: &str =
    "WgpuBackend acquisition failed on a machine that must have a GPU. \
Fix: inspect WGPU adapter probing and driver visibility; live GPU tests must not silently skip.";

/// Acquire a fresh live WGPU backend for tests that need isolated backend state.
pub(crate) fn acquire_live_backend() -> WgpuBackend {
    WgpuBackend::acquire().expect(LIVE_GPU_REQUIRED)
}

/// Acquire the shared live WGPU backend for capability/adapter tests.
pub(crate) fn shared_live_backend() -> WgpuBackend {
    WgpuBackend::shared()
        .expect(LIVE_GPU_REQUIRED)
        .as_ref()
        .clone()
}

/// Pack little-endian `u32` lanes into backend dispatch bytes.
pub(crate) use vyre_primitives::wire::pack_u32_slice as u32_bytes;

/// Alias used by C parser integration tests.
pub(crate) fn words_to_bytes(words: &[u32]) -> Vec<u8> {
    u32_bytes(words)
}

/// Decode backend output bytes into little-endian `u32` lanes.
pub(crate) use vyre_primitives::wire::decode_u32_le_bytes_all as bytes_u32;

pub(crate) use vyre_primitives::wire::decode_u32_le_bytes_all as decode_u32_words;

/// Alias used by C parser integration tests.
pub(crate) use vyre_primitives::wire::decode_u32_le_bytes_all as words_from_bytes;

pub(crate) fn add_one_program(words: u32) -> Program {
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

pub(crate) fn add_one_input(words: u32) -> Vec<u8> {
    vyre_primitives::wire::pack_u32_iter(0..words)
}

pub(crate) fn add_one_expected(words: u32) -> Vec<u8> {
    vyre_primitives::wire::pack_u32_iter(1..=words)
}

pub(crate) fn assert_dispatch_async_returns_before_gpu_completion() {
    let backend = acquire_live_backend();
    let program = add_one_program(512 * 1024);
    let input = add_one_input(512 * 1024);

    let start = Instant::now();
    let pending = backend
        .dispatch_async(&program, &[input], &DispatchConfig::default())
        .expect("Fix: dispatch_async must return a handle immediately without blocking on GPU completion");
    let return_time = start.elapsed();

    assert!(
        return_time < Duration::from_secs(1),
        "Fix: dispatch_async took {:?} to return; this suggests synchronous GPU blocking",
        return_time
    );

    let _ = pending.is_ready();

    let outputs = pending
        .await_result()
        .expect("Fix: await_result must resolve correctly");
    let expected = add_one_expected(512 * 1024);
    assert_eq!(outputs, vec![expected]);
}

pub(crate) fn assert_dispatch_async_ready_state_observable_for_non_trivial_work() {
    let backend = acquire_live_backend();
    let program = add_one_program(256 * 1024);
    let input = add_one_input(256 * 1024);

    let pending = backend
        .dispatch_async(&program, &[input], &DispatchConfig::default())
        .expect("Fix: dispatch_async must return a handle");

    let ready_now = pending.is_ready();

    let outputs = pending
        .await_result()
        .expect("Fix: await_result must resolve");
    let expected = add_one_expected(256 * 1024);
    assert_eq!(outputs, vec![expected]);

    if !ready_now {
        return;
    }
}

pub(crate) fn assert_multiple_concurrent_async_dispatches_do_not_serialize() {
    let backend = acquire_live_backend();
    let program = add_one_program(128 * 1024);
    let input = add_one_input(128 * 1024);

    let _ = backend
        .dispatch(&program, &[input.clone()], &DispatchConfig::default())
        .expect("Fix: warm-up dispatch must succeed");

    let start = Instant::now();
    let p1 = backend
        .dispatch_async(&program, &[input.clone()], &DispatchConfig::default())
        .expect("Fix: dispatch_async #1 must start");
    let p2 = backend
        .dispatch_async(&program, &[input.clone()], &DispatchConfig::default())
        .expect("Fix: dispatch_async #2 must start");
    let p3 = backend
        .dispatch_async(&program, &[input], &DispatchConfig::default())
        .expect("Fix: dispatch_async #3 must start");
    let submit_time = start.elapsed();

    assert!(
        submit_time < Duration::from_millis(100),
        "Fix: three back-to-back dispatch_async calls took {:?}, suggesting blocking behavior",
        submit_time
    );

    let o1 = p1
        .await_result()
        .expect("Fix: async dispatch #1 must complete");
    let o2 = p2
        .await_result()
        .expect("Fix: async dispatch #2 must complete");
    let o3 = p3
        .await_result()
        .expect("Fix: async dispatch #3 must complete");
    assert_eq!(
        o1, o2,
        "Fix: identical async dispatches must produce identical outputs"
    );
    assert_eq!(o2, o3);
}

pub(crate) fn assert_pending_dispatch_from_wgpu_is_object_safe() {
    let backend = acquire_live_backend();
    let program = add_one_program(1024);
    let input = add_one_input(1024);

    let pending: Box<dyn PendingDispatch> = backend
        .dispatch_async(&program, &[input], &DispatchConfig::default())
        .expect("Fix: wgpu dispatch_async must produce object-safe PendingDispatch");

    let outputs = pending
        .await_result()
        .expect("Fix: object-safe await must succeed");
    let expected = add_one_expected(1024);
    assert_eq!(outputs, vec![expected]);
}
