//! Failure-oriented tests for VYRE megakernel/runtime.
//!
//! These tests verify error paths, boundary conditions, and protocol
//! invariants without requiring a live GPU adapter.

#![cfg(feature = "megakernel-batch")]

use std::time::Duration;
use vyre_driver_wgpu::megakernel::BatchDispatchConfig;
use vyre_runtime::megakernel::{
    control, default_priority_offsets,
    io::{
        encode_empty_io_queue, io_op, io_status, io_word, try_complete_io_request,
        try_poll_io_requests,
    },
    protocol::slot,
    scheduler::{self, PRIORITY_OFFSETS_BASE, STARVATION_THRESHOLD},
    BatchRuleProgram, Megakernel, MegakernelIoQueue, PriorityRequeueAccounting, IO_SLOT_COUNT,
    IO_SLOT_WORDS,
};

const _: () = assert!(STARVATION_THRESHOLD > 0);
const _: () = assert!(control::TENANT_BASE < control::TENANT_QUOTA_BASE);
const _: () = assert!(control::TENANT_QUOTA_BASE < control::METRICS_BASE);
const _: () = assert!(control::METRICS_BASE + control::METRICS_SLOTS <= control::EPOCH);
const _: () = assert!(control::EPOCH < PRIORITY_OFFSETS_BASE);
const _: () =
    assert!(PRIORITY_OFFSETS_BASE + scheduler::PRIORITY_LEVELS <= control::OBSERVABLE_BASE);
const _: () = assert!(slot::EMPTY < slot::PUBLISHED);
const _: () = assert!(slot::PUBLISHED < slot::CLAIMED);
const _: () = assert!(slot::CLAIMED < slot::DONE);

// --- adaptive launch policy consumption ---

#[test]
fn adaptive_launch_consumes_zero_worker_groups() {
    let config = BatchDispatchConfig::default();
    let limits = wgpu::Limits::default();
    let rec = config
        .launch_recommendation(&limits, 64)
        .expect("zero worker_groups must be adapted into a positive recommendation");
    assert!(
        rec.worker_groups > 0,
        "policy must derive worker_groups from limits"
    );
    assert!(
        rec.hit_capacity > 0,
        "policy must derive hit_capacity from queue shape"
    );
}

#[test]
fn adaptive_launch_preserves_explicit_worker_groups() {
    let config = BatchDispatchConfig {
        worker_groups: 16,
        ..Default::default()
    };
    let limits = wgpu::Limits::default();
    let rec = config
        .launch_recommendation(&limits, 64)
        .expect("explicit worker_groups must be preserved");
    assert_eq!(rec.worker_groups, 16);
}

#[test]
fn adaptive_launch_rejects_zero_workgroup_size() {
    let config = BatchDispatchConfig {
        workgroup_size_x: 0,
        ..Default::default()
    };
    let limits = wgpu::Limits::default();
    let err = config
        .launch_recommendation(&limits, 64)
        .expect_err("zero workgroup_size_x must fail");
    assert!(err.to_string().contains("Fix:"));
}

// --- priority/requeue starvation ---

#[test]
fn priority_offsets_never_overlap() {
    for total_slots in [0, 1, 4, 255, 256, 1000] {
        let offsets = default_priority_offsets(total_slots);
        assert_eq!(offsets.len(), scheduler::PRIORITY_LEVELS as usize + 1);
        for i in 0..scheduler::PRIORITY_LEVELS as usize {
            assert!(
                offsets[i] <= offsets[i + 1],
                "priority partition boundaries must not decrease at slot count {total_slots}"
            );
        }
    }
}

#[test]
fn requeue_accounting_saturates_at_max() {
    let mut acc = PriorityRequeueAccounting {
        requeue_count: u64::MAX,
        ..Default::default()
    };
    acc.record_requeue(99);
    assert_eq!(acc.requeue_count, u64::MAX);
    assert_eq!(acc.max_priority_age, 99);
}

// --- timeout/cancellation semantics ---

#[test]
fn batch_dispatch_default_timeout_is_nonzero() {
    assert!(!BatchDispatchConfig::default().timeout.is_zero());
}

#[test]
fn batch_dispatch_timeout_can_be_overridden() {
    let config = BatchDispatchConfig {
        timeout: Duration::from_secs(5),
        ..Default::default()
    };
    assert_eq!(config.timeout, Duration::from_secs(5));
}

#[test]
fn persistent_dispatch_wait_does_not_use_fixed_millisecond_sleep() {
    let src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/megakernel/dispatcher.rs"
    ))
    .expect("dispatcher source must be readable for wait-loop contract test");
    assert!(
        !src.contains("std::thread::sleep") && !src.contains("Duration::from_millis(1)"),
        "persistent dispatch wait must use adaptive bounded waiting, not a fixed 1ms sleep"
    );
    assert!(
        src.contains("park_timeout") && src.contains("spin_loop"),
        "persistent dispatch wait must combine short CPU pauses with bounded parking"
    );
}

// --- protocol region non-overlap ---

#[test]
fn encode_control_clamps_tenant_table() {
    let ctrl = Megakernel::encode_control(false, 1_000_000, 0).unwrap();
    let quota_off = control::TENANT_QUOTA_BASE as usize * 4;
    let quota_first = u32::from_le_bytes(ctrl[quota_off..quota_off + 4].try_into().unwrap());
    assert_eq!(quota_first, 1_000_000);
}

#[test]
fn encode_control_clamps_quota_table() {
    let ctrl = Megakernel::encode_control(false, 1_000_000, 0).unwrap();
    let metrics_off = control::METRICS_BASE as usize * 4;
    let metrics_first = u32::from_le_bytes(ctrl[metrics_off..metrics_off + 4].try_into().unwrap());
    assert_eq!(metrics_first, 0);
}

// --- IO queue truncation/alignment/status ordering ---

#[test]
fn io_queue_rejects_zero_slots() {
    let err = MegakernelIoQueue::new(0).expect_err("zero slot count must fail");
    assert!(err.to_string().contains("Fix:"));
}

#[test]
fn io_queue_rejects_over_max_slots() {
    let err = MegakernelIoQueue::new(IO_SLOT_COUNT + 1).expect_err("overflow slot count must fail");
    assert!(err.to_string().contains("Fix:"));
}

#[test]
fn io_queue_alignment_exact_multiple() {
    let queue = MegakernelIoQueue::new(IO_SLOT_COUNT).unwrap();
    assert_eq!(queue.as_bytes().len() % ((IO_SLOT_WORDS as usize) * 4), 0);
}

#[test]
fn io_status_disjoint_from_slot_states() {
    assert_ne!(io_status::OK, slot::EMPTY);
    assert_ne!(io_status::OK, slot::PUBLISHED);
    assert_ne!(io_status::OK, slot::CLAIMED);
    assert_ne!(io_status::OK, slot::DONE);
    assert_ne!(io_status::ERROR, slot::EMPTY);
    assert_ne!(io_status::ERROR, slot::PUBLISHED);
    assert_ne!(io_status::ERROR, slot::CLAIMED);
    assert_ne!(io_status::ERROR, slot::DONE);
}

#[test]
fn strict_poll_rejects_misaligned_queue_bytes() {
    let mut buf = encode_empty_io_queue(1).expect("valid io_queue must encode");
    let write_word = |buf: &mut Vec<u8>, word: u32, val: u32| {
        let off = word as usize * 4;
        buf[off..off + 4].copy_from_slice(&val.to_le_bytes());
    };
    write_word(&mut buf, io_word::OP_TYPE, io_op::READ);
    write_word(&mut buf, io_word::BYTE_COUNT, 4096);
    write_word(&mut buf, io_word::STATUS, slot::PUBLISHED);
    buf.push(0xAA);

    let err = try_poll_io_requests(&buf).expect_err("misaligned IO queue bytes must fail");
    assert!(err.to_string().contains("Fix:"));
}

#[test]
fn strict_completion_rejects_out_of_bounds_slot() {
    let mut buf = encode_empty_io_queue(1).expect("valid io_queue must encode");
    let err = try_complete_io_request(&mut buf, 1, true)
        .expect_err("completion outside the queue must fail");
    assert!(err.to_string().contains("Fix:"));
}

// --- capability-table resource names ---

#[test]
fn io_polling_uses_capability_table_names() {
    use vyre_foundation::ir::Node;
    use vyre_runtime::megakernel::build_program_sharded_with_io_polling;

    fn collect_async_load_bindings(nodes: &[Node], out: &mut Vec<(String, String)>) {
        for node in nodes {
            match node {
                Node::AsyncLoad {
                    source,
                    destination,
                    ..
                } => {
                    out.push((
                        source.as_str().to_string(),
                        destination.as_str().to_string(),
                    ));
                }
                Node::If {
                    then, otherwise, ..
                } => {
                    collect_async_load_bindings(then, out);
                    collect_async_load_bindings(otherwise, out);
                }
                Node::Loop { body, .. } | Node::Block(body) => {
                    collect_async_load_bindings(body, out)
                }
                Node::Region { body, .. } => collect_async_load_bindings(body, out),
                _ => {}
            }
        }
    }

    let program = build_program_sharded_with_io_polling(64, &[]);
    let mut bindings = Vec::new();
    collect_async_load_bindings(&program.entry, &mut bindings);
    assert!(
        !bindings.is_empty(),
        "IO polling program must contain AsyncLoad nodes"
    );
    for (src, dst) in &bindings {
        assert!(
            src.contains("capability"),
            "source resource `{src}` must be a capability table"
        );
        assert!(
            dst.contains("capability"),
            "destination resource `{dst}` must be a capability table"
        );
    }
}

// --- batch/rule catalog validation ---

#[test]
fn batch_rule_program_rejects_mismatched_transition_table() {
    let err = BatchRuleProgram::new(0, vec![0; 128], vec![0; 2], 2)
        .expect_err("transition table size mismatch must fail");
    assert!(err.to_string().contains("Fix:"));
}

#[test]
fn batch_rule_program_rejects_mismatched_accept_table() {
    let err = BatchRuleProgram::new(0, vec![0; 512], vec![0; 1], 2)
        .expect_err("accept table size mismatch must fail");
    assert!(err.to_string().contains("Fix:"));
}
