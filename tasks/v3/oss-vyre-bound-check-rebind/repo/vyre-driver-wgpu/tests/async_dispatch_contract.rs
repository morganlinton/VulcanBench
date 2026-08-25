//! Contract tests for the wgpu backend's non-blocking dispatch entrypoint.

use vyre_driver::{DispatchConfig, VyreBackend};
use vyre_driver_wgpu::WgpuBackend;
use vyre_foundation::ir::{BinOp, BufferDecl, DataType, Expr, Node, Program};

fn add_one_program(words: u32) -> Program {
    let idx = Expr::gid_x();
    let in_bounds = Expr::BinOp {
        op: BinOp::Lt,
        left: Box::new(idx.clone()),
        right: Box::new(Expr::u32(words)),
    };
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
                    Expr::BinOp {
                        op: BinOp::Add,
                        left: Box::new(Expr::load("input", idx)),
                        right: Box::new(Expr::u32(1)),
                    },
                )],
            ),
            Node::return_(),
        ],
    )
}

fn mul_two_program(words: u32) -> Program {
    let idx = Expr::gid_x();
    let in_bounds = Expr::BinOp {
        op: BinOp::Lt,
        left: Box::new(idx.clone()),
        right: Box::new(Expr::u32(words)),
    };
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
                    Expr::BinOp {
                        op: BinOp::Mul,
                        left: Box::new(Expr::load("input", idx)),
                        right: Box::new(Expr::u32(2)),
                    },
                )],
            ),
            Node::return_(),
        ],
    )
}

#[test]
fn dispatch_batch_empty_jobs_returns_empty_results() {
    let backend = WgpuBackend::new()
        .expect("Fix: dispatch_batch contract requires a configured live GPU backend");
    let results = backend
        .dispatch_batch(&[])
        .expect("Fix: empty dispatch_batch must launch cleanly");
    assert!(
        results.is_empty(),
        "Fix: empty dispatch_batch input must produce an empty result list"
    );
}

#[test]
fn dispatch_borrowed_batch_empty_jobs_returns_empty_results() {
    let backend = WgpuBackend::new()
        .expect("Fix: dispatch_borrowed_batch contract requires a configured live GPU backend");
    let results = backend
        .dispatch_borrowed_batch(&[])
        .expect("Fix: empty dispatch_borrowed_batch must launch cleanly");
    assert!(
        results.is_empty(),
        "Fix: empty dispatch_borrowed_batch input must produce an empty result list"
    );
}

#[test]
fn dispatch_borrowed_batch_matches_owned_batch_outputs() {
    let backend = WgpuBackend::new()
        .expect("Fix: borrowed batch contract requires a configured live GPU backend");
    let program = add_one_program(256);
    let config = DispatchConfig::default();
    let input_a: Vec<u8> = vyre_primitives::wire::pack_u32_iter(0..256u32);
    let input_b: Vec<u8> = vyre_primitives::wire::pack_u32_iter(512..768u32);
    let borrowed_a = [input_a.as_slice()];
    let borrowed_b = [input_b.as_slice()];
    let borrowed_jobs = [
        (&program, borrowed_a.as_slice(), &config),
        (&program, borrowed_b.as_slice(), &config),
    ];

    let results = backend
        .dispatch_borrowed_batch(&borrowed_jobs)
        .expect("Fix: borrowed batch must launch without owned input allocation");

    assert_eq!(
        results.len(),
        borrowed_jobs.len(),
        "Fix: borrowed batch must return one result per submitted job"
    );
    let expected_a: Vec<u8> = vyre_primitives::wire::pack_u32_iter(1..=256u32);
    let expected_b: Vec<u8> = vyre_primitives::wire::pack_u32_iter(513..769u32);
    assert_eq!(
        results[0]
            .as_ref()
            .expect("Fix: first borrowed batch dispatch must succeed"),
        &vec![expected_a],
        "Fix: first borrowed batch output must match the owned dispatch contract"
    );
    assert_eq!(
        results[1]
            .as_ref()
            .expect("Fix: second borrowed batch dispatch must succeed"),
        &vec![expected_b],
        "Fix: second borrowed batch output must match the owned dispatch contract"
    );
}

#[test]
fn dispatch_borrowed_batch_coalesces_distinct_programs() {
    let backend = WgpuBackend::new()
        .expect("Fix: distinct-program borrowed batch requires a configured live GPU backend");
    let add = add_one_program(64);
    let mul = mul_two_program(64);
    let config = DispatchConfig::default();
    let input: Vec<u8> = vyre_primitives::wire::pack_u32_iter(0..64u32);
    let borrowed = [input.as_slice()];
    let jobs = [
        (&add, borrowed.as_slice(), &config),
        (&mul, borrowed.as_slice(), &config),
    ];

    let results = backend
        .dispatch_borrowed_batch(&jobs)
        .expect("Fix: distinct-program borrowed batch must record and submit cleanly");

    assert_eq!(results.len(), 2);
    let expected_add: Vec<u8> = vyre_primitives::wire::pack_u32_iter(1..=64u32);
    let expected_mul: Vec<u8> = (0..64u32)
        .map(|v| v * 2)
        .flat_map(u32::to_le_bytes)
        .collect();
    assert_eq!(
        results[0]
            .as_ref()
            .expect("Fix: add program in coalesced batch must succeed"),
        &vec![expected_add]
    );
    assert_eq!(
        results[1]
            .as_ref()
            .expect("Fix: mul program in coalesced batch must succeed"),
        &vec![expected_mul]
    );
}

#[test]
fn dispatch_borrowed_batch_into_reuses_caller_output_slots() {
    let backend = WgpuBackend::new()
        .expect("Fix: borrowed batch-into contract requires a configured live GPU backend");
    let program = add_one_program(128);
    let config = DispatchConfig::default();
    let input_a: Vec<u8> = vyre_primitives::wire::pack_u32_iter(0..128u32);
    let input_b: Vec<u8> = vyre_primitives::wire::pack_u32_iter(256..384u32);
    let borrowed_a = [input_a.as_slice()];
    let borrowed_b = [input_b.as_slice()];
    let borrowed_jobs = [
        (&program, borrowed_a.as_slice(), &config),
        (&program, borrowed_b.as_slice(), &config),
    ];
    let mut outputs = vec![
        vec![Vec::with_capacity(128 * 4)],
        vec![Vec::with_capacity(128 * 4)],
    ];

    let results = backend
        .dispatch_borrowed_batch_into(&borrowed_jobs, &mut outputs)
        .expect("Fix: borrowed batch-into must launch without owned input allocation");

    assert_eq!(
        results.len(),
        borrowed_jobs.len(),
        "Fix: borrowed batch-into must return one status per submitted job"
    );
    for (index, result) in results.into_iter().enumerate() {
        result.unwrap_or_else(|error| {
            panic!("Fix: borrowed batch-into job #{index} must succeed: {error}")
        });
    }
    let expected_a: Vec<u8> = vyre_primitives::wire::pack_u32_iter(1..=128u32);
    let expected_b: Vec<u8> = vyre_primitives::wire::pack_u32_iter(257..385u32);
    assert_eq!(outputs[0], vec![expected_a]);
    assert_eq!(outputs[1], vec![expected_b]);
}

#[test]
fn dispatch_borrowed_batch_into_rejects_output_slot_mismatch() {
    let backend = WgpuBackend::new()
        .expect("Fix: borrowed batch-into mismatch test requires a configured live GPU backend");
    let program = add_one_program(16);
    let config = DispatchConfig::default();
    let input: Vec<u8> = vyre_primitives::wire::pack_u32_iter(0..16u32);
    let borrowed = [input.as_slice()];
    let jobs = [(&program, borrowed.as_slice(), &config)];
    let mut outputs = Vec::new();

    let err = backend
        .dispatch_borrowed_batch_into(&jobs, &mut outputs)
        .expect_err("Fix: batch-into must reject missing output slots before launch");
    assert!(
        err.to_string().contains("one OutputBuffers slot per job"),
        "Fix: mismatch error must tell callers how to size the output slot slice: {err}"
    );
}

#[test]
fn dispatch_borrowed_for_each_mapped_output_visits_trimmed_bytes_without_vec_contract() {
    let backend =
        WgpuBackend::new().expect("Fix: mapped-output contract requires a live GPU backend");
    let program = add_one_program(64);
    let config = DispatchConfig::default();
    let input: Vec<u8> = vyre_primitives::wire::pack_u32_iter(0..64u32);
    let borrowed = [input.as_slice()];
    let mut seen_outputs = 0usize;
    let mut observed = Vec::new();

    backend
        .dispatch_borrowed_for_each_mapped_output(&program, &borrowed, &config, |index, mapped| {
            assert_eq!(
                index, 0,
                "Fix: single-output programs must visit output index zero exactly once"
            );
            seen_outputs += 1;
            observed.extend_from_slice(mapped);
            Ok(())
        })
        .expect("Fix: mapped-output dispatch must expose the GPU readback slice");

    let expected: Vec<u8> = vyre_primitives::wire::pack_u32_iter(1..=64u32);
    assert_eq!(
        seen_outputs, 1,
        "Fix: mapped-output dispatch must visit one callback per declared output"
    );
    assert_eq!(
        observed, expected,
        "Fix: mapped-output dispatch must expose the same trimmed output bytes as dispatch_borrowed"
    );
}

#[test]
fn dispatch_borrowed_for_each_pod_output_views_u32_results() {
    let backend =
        WgpuBackend::new().expect("Fix: typed mapped-output contract requires a live GPU backend");
    let program = add_one_program(32);
    let config = DispatchConfig::default();
    let input: Vec<u8> = vyre_primitives::wire::pack_u32_iter(100..132u32);
    let borrowed = [input.as_slice()];
    let mut observed = Vec::new();

    backend
        .dispatch_borrowed_for_each_pod_output::<u32, _>(
            &program,
            &borrowed,
            &config,
            |index, values| {
                assert_eq!(
                    index, 0,
                    "Fix: typed mapped-output dispatch must preserve output order"
                );
                observed.extend_from_slice(values);
                Ok(())
            },
        )
        .expect("Fix: typed mapped-output dispatch must cast aligned output bytes to POD values");

    let expected: Vec<u32> = (101..133u32).collect();
    assert_eq!(
        observed, expected,
        "Fix: typed mapped-output dispatch must expose the GPU result as a POD slice"
    );
}

#[test]
fn dispatch_borrowed_for_each_mapped_output_propagates_visitor_error() {
    let backend =
        WgpuBackend::new().expect("Fix: mapped-output error contract requires a live GPU backend");
    let program = add_one_program(8);
    let config = DispatchConfig::default();
    let input: Vec<u8> = vyre_primitives::wire::pack_u32_iter(0..8u32);
    let borrowed = [input.as_slice()];

    let error = backend
        .dispatch_borrowed_for_each_mapped_output(&program, &borrowed, &config, |_, _| {
            Err(vyre_driver::BackendError::new(
                "visitor rejected mapped output",
            ))
        })
        .expect_err("Fix: mapped-output dispatch must propagate visitor failures");
    assert!(
        error.to_string().contains("visitor rejected mapped output"),
        "Fix: visitor errors must not be hidden behind readback plumbing: {error}"
    );
}

#[test]
fn dispatch_profile_gpu_timestamps_executes_with_live_timestamp_queries() {
    let backend =
        WgpuBackend::new().expect("Fix: timestamp profile contract requires a live GPU backend");
    let program = add_one_program(32);
    let input: Vec<u8> = vyre_primitives::wire::pack_u32_iter(0..32u32);
    let borrowed = [input.as_slice()];
    let mut config = DispatchConfig::default();
    config.profile = Some("gpu-timestamps".to_string());

    let outputs = backend
        .dispatch_borrowed(&program, &borrowed, &config)
        .expect("Fix: gpu-timestamps profile must use live TIMESTAMP_QUERY instrumentation");

    let expected: Vec<u8> = vyre_primitives::wire::pack_u32_iter(1..=32u32);
    assert_eq!(
        outputs,
        vec![expected],
        "Fix: timestamp instrumentation must not perturb dispatch outputs"
    );
}

#[test]
fn wgpu_dispatch_async_returns_handle_and_matches_borrowed_dispatch() {
    let adapters = vyre_driver_wgpu::runtime::device::enumerate_adapters();
    assert!(
        !adapters.is_empty(),
        "Fix: async dispatch contract requires the live RTX 5090 adapter."
    );
    let backend =
        WgpuBackend::acquire().expect("Fix: async dispatch contract must acquire live GPU");
    let program = add_one_program(256);
    let input: Vec<u8> = vyre_primitives::wire::pack_u32_iter(0..256u32);
    let inputs = vec![input];

    let pending = backend
        .dispatch_async(&program, &inputs, &DispatchConfig::default())
        .expect("Fix: dispatch_async must start without executing synchronously to completion");
    let async_outputs = pending
        .await_result()
        .expect("Fix: async dispatch handle must return GPU outputs");

    let borrowed = [inputs[0].as_slice()];
    let borrowed_outputs = backend
        .dispatch_borrowed(&program, &borrowed, &DispatchConfig::default())
        .expect("Fix: borrowed dispatch must remain the byte-identity reference path");
    assert_eq!(
        async_outputs, borrowed_outputs,
        "async dispatch must preserve the exact GPU output contract"
    );

    let expected: Vec<u8> = vyre_primitives::wire::pack_u32_iter(1..=256u32);
    assert_eq!(
        async_outputs,
        vec![expected],
        "async dispatch must execute the submitted program, not return a cached or empty result"
    );
}

#[test]
fn wgpu_dispatch_async_ready_state_matches_dispatch_lifecycle() {
    let backend =
        WgpuBackend::acquire().expect("Fix: async lifecycle contract must acquire live GPU");

    let ready = backend
        .dispatch_async(&Program::empty(), &[], &DispatchConfig::default())
        .expect("Fix: explicit noop dispatch_async must return a handle");
    assert!(
        ready.is_ready(),
        "Fix: explicit noop dispatch should return a trivially-ready pending handle"
    );
    assert_eq!(
        ready
            .await_result()
            .expect("Fix: ready noop handle must await cleanly"),
        Vec::<Vec<u8>>::new(),
        "Fix: noop pending handle must resolve to no outputs"
    );

    let program = add_one_program(4096);
    let input: Vec<u8> = vyre_primitives::wire::pack_u32_iter(0..4096u32);
    let inputs = vec![input];
    let mut config = DispatchConfig::default();
    config.fixpoint_iterations = Some(2048);
    let pending = backend
        .dispatch_async(&program, &inputs, &config)
        .expect("Fix: non-noop dispatch_async must submit GPU work and return a pending handle");
    let outputs = pending
        .await_result()
        .expect("Fix: non-noop pending handle must resolve through GPU readback");
    let expected: Vec<u8> = vyre_primitives::wire::pack_u32_iter(1..=4096u32);
    assert_eq!(
        outputs,
        vec![expected],
        "Fix: pending non-noop handle must return the GPU result, not a precomputed synchronous result"
    );
}
