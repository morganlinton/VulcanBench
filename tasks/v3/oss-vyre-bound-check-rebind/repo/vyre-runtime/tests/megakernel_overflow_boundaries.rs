//! Overflow at ring/protocol boundaries  -  cumulative and arithmetic edge cases.

use vyre_runtime::megakernel::{
    descriptor::{BatchDescriptor, BuiltinOpcode, SlotDescriptor, SlotOpcode, WindowDescriptor},
    protocol::{self, control, debug},
    Megakernel,
};
use vyre_runtime::PipelineError;

#[test]
fn cumulative_batch_publish_rejects_overflow_past_ring_end() {
    let mut ring = Megakernel::encode_empty_ring(2).unwrap();
    let batch1 = BatchDescriptor::new(
        0,
        vec![
            SlotDescriptor::single(0, SlotOpcode::Builtin(BuiltinOpcode::Nop), vec![]),
            SlotDescriptor::single(0, SlotOpcode::Builtin(BuiltinOpcode::Nop), vec![]),
        ],
    );
    batch1.publish_into(&mut ring).unwrap();

    let batch2 = BatchDescriptor::new(
        2,
        vec![SlotDescriptor::single(
            0,
            SlotOpcode::Builtin(BuiltinOpcode::Nop),
            vec![],
        )],
    );
    let err = batch2
        .publish_into(&mut ring)
        .expect_err("cumulative overflow past ring end must reject");
    assert!(matches!(err, PipelineError::QueueFull { .. }));
}

#[test]
fn cumulative_window_publish_rejects_overflow_past_ring_end() {
    let mut ring = Megakernel::encode_empty_ring(2).unwrap();
    let w1 = WindowDescriptor::new(0, 0, SlotOpcode::Custom(0xBEEF), 1, vec![vec![]], vec![]);
    w1.publish_into(&mut ring).unwrap();

    let w2 = WindowDescriptor::new(2, 0, SlotOpcode::Custom(0xBEEF), 2, vec![vec![]], vec![]);
    let err = w2
        .publish_into(&mut ring)
        .expect_err("cumulative window overflow past ring end must reject");
    assert!(matches!(err, PipelineError::QueueFull { .. }));
}

#[test]
fn batch_publish_fence_slot_index_overflows_cleanly_near_u32_max() {
    let mut ring = Megakernel::encode_empty_ring(1).unwrap();
    let err = Megakernel::batch_publish(
        &mut ring,
        u32::MAX,
        0,
        &[(protocol::opcode::NOP, vec![])],
        0,
    )
    .expect_err("batch publish at u32::MAX must fail gracefully");
    assert!(matches!(err, PipelineError::QueueFull { .. }));
}

#[test]
fn strict_io_completion_rejects_slot_beyond_u32_max_mapped_index() {
    let mut buf =
        vyre_runtime::megakernel::io::encode_empty_io_queue(1).expect("valid io_queue must encode");
    let err = vyre_runtime::megakernel::io::try_complete_io_request(&mut buf, u32::MAX, true)
        .expect_err("completion at u32::MAX must be rejected on a 1-slot queue");
    assert!(matches!(err, PipelineError::QueueFull { .. }));
}

#[test]
fn ring_byte_len_overflows_at_exact_u32_boundary() {
    let max_slots = u32::MAX / protocol::SLOT_WORDS;
    assert!(
        protocol::ring_byte_len(max_slots).is_some(),
        "max representable slots must produce a byte length"
    );
    assert!(
        protocol::ring_byte_len(max_slots + 1).is_none(),
        "one slot beyond representable must overflow"
    );
}

#[test]
fn control_byte_len_overflows_at_exact_u32_boundary() {
    let max_observable = u32::MAX - control::OBSERVABLE_BASE;
    assert!(
        protocol::control_byte_len(max_observable).is_some(),
        "max observable slots must produce a byte length"
    );
    assert!(
        protocol::control_byte_len(max_observable + 1).is_none(),
        "one observable beyond representable must overflow"
    );
}

#[test]
fn debug_log_byte_len_overflows_at_exact_u32_boundary() {
    let max_records = u32::MAX / debug::RECORD_WORDS;
    assert!(
        protocol::debug_log_byte_len(max_records).is_some(),
        "max record capacity must produce a byte length"
    );
    assert!(
        protocol::debug_log_byte_len(max_records + 1).is_none(),
        "one record beyond representable must overflow"
    );
}
