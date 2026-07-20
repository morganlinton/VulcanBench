//! Unbounded allocation rejection for megakernel buffer encoders.
//!
//! Verifies that fallible encoder entrypoints reject inputs that would
//! overflow the protocol byte-length calculations *before* allocating
//! host-side Vec<u8> buffers.

use vyre_runtime::megakernel::{
    protocol::{self, control, debug},
    Megakernel, MegakernelIoQueue, SLOT_WORDS,
};
use vyre_runtime::PipelineError;

#[test]
fn try_encode_empty_ring_rejects_overflowing_slot_count() {
    let too_many = (u32::MAX / SLOT_WORDS) + 1;
    let err = Megakernel::try_encode_empty_ring(too_many)
        .expect_err("overflowing slot count must be rejected before allocation");
    assert!(
        matches!(err, PipelineError::QueueFull { .. }),
        "expected QueueFull, got {err:?}"
    );
    let msg = err.to_string();
    assert!(
        msg.contains("overflow") || msg.contains("shard"),
        "error must mention overflow or sharding: {msg}"
    );
}

#[test]
fn try_encode_empty_debug_log_rejects_overflowing_record_capacity() {
    let too_many = (u32::MAX / debug::RECORD_WORDS) + 1;
    let err = Megakernel::try_encode_empty_debug_log(too_many)
        .expect_err("overflowing record capacity must be rejected before allocation");
    assert!(
        matches!(err, PipelineError::QueueFull { .. }),
        "expected QueueFull, got {err:?}"
    );
}

#[test]
fn try_encode_control_rejects_overflow_at_observable_boundary() {
    let overflow_observable = u32::MAX - control::OBSERVABLE_BASE + 1;
    let err = Megakernel::try_encode_control(false, 1, overflow_observable)
        .expect_err("observable word offset overflow must be rejected");
    assert!(
        matches!(err, PipelineError::QueueFull { .. }),
        "expected QueueFull, got {err:?}"
    );
}

#[test]
fn try_encode_empty_io_queue_rejects_u32_max_before_alloc() {
    let err = vyre_runtime::megakernel::io::try_encode_empty_io_queue(u32::MAX)
        .expect_err("u32::MAX io queue must be rejected before allocation");
    assert!(
        matches!(err, PipelineError::QueueFull { .. }),
        "expected QueueFull, got {err:?}"
    );
}

#[test]
fn megakernel_io_queue_new_rejects_u32_max() {
    let err = MegakernelIoQueue::new(u32::MAX)
        .expect_err("u32::MAX io queue must be rejected before allocation");
    assert!(
        matches!(err, PipelineError::QueueFull { .. }),
        "expected QueueFull, got {err:?}"
    );
}

#[test]
fn batch_publish_rejects_slot_index_overflow_before_allocating_extra_ring() {
    let mut ring = Megakernel::encode_empty_ring(1).unwrap();
    let err = Megakernel::batch_publish(
        &mut ring,
        u32::MAX,
        0,
        &[(protocol::opcode::NOP, vec![])],
        0,
    )
    .expect_err("batch publish slot-index overflow must be rejected");
    assert!(
        matches!(err, PipelineError::QueueFull { .. }),
        "expected QueueFull, got {err:?}"
    );
}
