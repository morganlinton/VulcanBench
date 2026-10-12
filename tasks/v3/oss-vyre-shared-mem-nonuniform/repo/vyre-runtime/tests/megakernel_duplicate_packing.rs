//! Duplicate queue packing contracts.
//!
//! Covers behavior when the same opcode, ticket, or task identity appears
//! more than once in a packed batch or window.
#![cfg(feature = "legacy-infallible")]

use vyre_runtime::megakernel::{
    descriptor::{BatchDescriptor, SlotDescriptor, SlotOpcode, WindowDescriptor},
    protocol::{self, slot},
    Megakernel, RingTelemetry,
};
use vyre_runtime::PipelineError;

#[test]
fn batch_descriptor_counts_duplicate_items_independently() {
    let mut ring = Megakernel::encode_empty_ring(4).unwrap();
    let batch = BatchDescriptor::new(
        0,
        vec![
            SlotDescriptor::single(0, SlotOpcode::Custom(0xABCD), vec![1, 2]),
            SlotDescriptor::single(0, SlotOpcode::Custom(0xABCD), vec![1, 2]),
        ],
    );
    let consumed = batch
        .publish_into(&mut ring)
        .expect("duplicate items must publish");
    assert_eq!(consumed, 2);

    let telemetry = RingTelemetry::decode(&Megakernel::encode_control(false, 1, 0).unwrap(), &ring);
    assert_eq!(telemetry.occupancy.published, 2);
}

#[test]
fn packed_slot_accepts_duplicate_inner_opcodes() {
    let mut ring = Megakernel::encode_empty_ring(1).unwrap();
    let ops = vec![
        (protocol::opcode::NOP as u8, vec![1u32]),
        (protocol::opcode::NOP as u8, vec![1u32]),
    ];
    Megakernel::publish_packed_slot(&mut ring, 0, 0, &ops)
        .expect("duplicate inner opcodes must be allowed in packed slot");
    let status = u32::from_le_bytes(ring[..4].try_into().unwrap());
    assert_eq!(status, slot::PUBLISHED);
}

#[test]
fn duplicate_window_tickets_aggregate_in_telemetry() {
    let mut ring = Megakernel::encode_empty_ring(4).unwrap();
    let window_opcode = 0xF102;
    let w1 = WindowDescriptor::new(
        0,
        3,
        SlotOpcode::Custom(window_opcode),
        99,
        vec![vec![10]],
        vec![],
    );
    let w2 = WindowDescriptor::new(
        1,
        3,
        SlotOpcode::Custom(window_opcode),
        99,
        vec![vec![20]],
        vec![],
    );
    w1.publish_into(&mut ring).unwrap();
    w2.publish_into(&mut ring).unwrap();

    let telemetry = RingTelemetry::decode_with_window_opcodes(
        &Megakernel::encode_control(false, 1, 0).unwrap(),
        &ring,
        &[window_opcode],
    );
    assert_eq!(
        telemetry.windows.len(),
        1,
        "duplicate ticket must aggregate into one WindowTelemetry"
    );
    let window = &telemetry.windows[0];
    assert_eq!(window.ticket, 99);
    assert_eq!(window.required_slots, 2);
    assert_eq!(window.published, 2);
}

#[test]
fn duplicate_batch_publish_to_same_inflight_slot_is_rejected() {
    let mut ring = Megakernel::encode_empty_ring(2).unwrap();
    Megakernel::publish_slot(&mut ring, 1, 0, protocol::opcode::NOP, &[]).unwrap();

    let err = Megakernel::batch_publish(
        &mut ring,
        1,
        0,
        &[(protocol::opcode::STORE_U32, vec![42, 7])],
        0,
    )
    .expect_err("double-publishing into an inflight slot must be rejected");
    assert!(matches!(err, PipelineError::QueueFull { .. }));
}
