//! Megakernel long-term innovation edge cases  -  runtime-level invariants.
//!
//! Targets:
//! - Bounded batch packing
//! - Protocol encode/decode rejects short buffers
//! - Async worker dispatch is observable
#![cfg(feature = "legacy-infallible")]

#![allow(clippy::field_reassign_with_default)]

use vyre_driver_wgpu::{megakernel::WgpuMegakernelDispatcher, WgpuBackend};
use vyre_runtime::megakernel::{
    descriptor::{
        BatchDescriptor, BuiltinOpcode, PackedOpDescriptor, SlotDescriptor, SlotOpcode,
        WindowClass, WindowDescriptor,
    },
    protocol::{self, control, opcode, slot, ARGS_PER_SLOT, CONTROL_MIN_WORDS},
    telemetry::{ControlSnapshot, RingTelemetry},
    Megakernel,
};
use vyre_runtime::megakernel::{MegakernelConfig, MegakernelWorkItem};
use vyre_runtime::PipelineError;

// ---------------------------------------------------------------------------
// 1. Bounded batch packing
// ---------------------------------------------------------------------------

#[test]
fn empty_batch_descriptor_consumes_zero_slots_and_leaves_ring_intact() {
    let mut ring = Megakernel::encode_empty_ring(4).unwrap();
    Megakernel::publish_slot(&mut ring, 0, 1, opcode::NOP, &[42]).unwrap();
    let batch = BatchDescriptor::new(1, vec![]);
    let consumed = batch.publish_into(&mut ring).unwrap();
    assert_eq!(consumed, 0, "empty batch must consume zero slots");
    let base = 0;
    let status = u32::from_le_bytes(ring[base..base + 4].try_into().unwrap());
    let arg0 = u32::from_le_bytes(
        ring[base + (protocol::ARG0_WORD as usize) * 4
            ..base + (protocol::ARG0_WORD as usize) * 4 + 4]
            .try_into()
            .unwrap(),
    );
    assert_eq!(status, slot::PUBLISHED);
    assert_eq!(arg0, 42);
}

#[test]
fn empty_window_descriptor_consumes_zero_slots() {
    let mut ring = Megakernel::encode_empty_ring(2).unwrap();
    let window = WindowDescriptor::new(
        0,
        5,
        SlotOpcode::Builtin(BuiltinOpcode::Nop),
        99,
        vec![],
        vec![],
    );
    let consumed = window.publish_into(&mut ring).unwrap();
    assert_eq!(consumed, 0, "empty window must consume zero slots");
    for slot_idx in 0..2 {
        let base = slot_idx * (protocol::SLOT_WORDS as usize) * 4;
        let status = u32::from_le_bytes(ring[base..base + 4].try_into().unwrap());
        assert_eq!(status, slot::EMPTY, "slot {slot_idx} must remain empty");
    }
}

#[test]
fn packed_op_exactly_12_word_boundary_one_op_succeeds() {
    let mut ring = Megakernel::encode_empty_ring(1).unwrap();
    // 1 op with 11 args: metadata = 1 word, args = 11 words, total = 12 = ARGS_PER_SLOT
    let op = PackedOpDescriptor::new(7, vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]);
    let slot = SlotDescriptor::packed(0, vec![op]);
    slot.publish_into(&mut ring, 0)
        .expect("exactly 12 words must succeed");
    let op_word = u32::from_le_bytes(ring[4..8].try_into().unwrap());
    assert_eq!(op_word, opcode::PACKED_SLOT);
}

#[test]
fn normal_slot_exact_12_args_boundary_succeeds() {
    let mut ring = Megakernel::encode_empty_ring(1).unwrap();
    let args: Vec<u32> = (0..ARGS_PER_SLOT).collect();
    let slot = SlotDescriptor::single(0, SlotOpcode::Builtin(BuiltinOpcode::StoreU32), args);
    slot.publish_into(&mut ring, 0)
        .expect("exactly 12 args must succeed");
    let base = (protocol::ARG0_WORD as usize) * 4;
    let arg0 = u32::from_le_bytes(ring[base..base + 4].try_into().unwrap());
    assert_eq!(arg0, 0);
    let arg11 = u32::from_le_bytes(ring[base + 44..base + 48].try_into().unwrap());
    assert_eq!(arg11, 11);
}

#[test]
fn window_descriptor_exact_10_word_payload_succeeds() {
    let mut ring = Megakernel::encode_empty_ring(1).unwrap();
    // Window prefixes [ticket, class_tag] = 2 words, leaving 10 words for payload
    let payload: Vec<u32> = (0..10).collect();
    let window = WindowDescriptor::new(
        0,
        1,
        SlotOpcode::Builtin(BuiltinOpcode::Nop),
        77,
        vec![payload],
        vec![],
    );
    window.publish_into(&mut ring).unwrap();
    let base = (protocol::ARG0_WORD as usize) * 4;
    let ticket = u32::from_le_bytes(ring[base..base + 4].try_into().unwrap());
    let class = u32::from_le_bytes(ring[base + 4..base + 8].try_into().unwrap());
    assert_eq!(ticket, 77);
    assert_eq!(class, WindowClass::Required.into_wire());
}

#[test]
fn batch_descriptor_start_slot_u32_max_with_item_overflows_cleanly() {
    let mut ring = Megakernel::encode_empty_ring(1).unwrap();
    let batch = BatchDescriptor::new(
        u32::MAX,
        vec![SlotDescriptor::single(
            0,
            SlotOpcode::Builtin(BuiltinOpcode::Nop),
            vec![],
        )],
    );
    let err = batch
        .publish_into(&mut ring)
        .expect_err("start_slot + index overflow must be rejected cleanly");
    assert!(matches!(err, PipelineError::QueueFull { .. }));
    let msg = err.to_string();
    assert!(msg.contains("Fix:"), "error must be actionable: {msg}");
}

// ---------------------------------------------------------------------------
// 2. Protocol encode/decode rejects short buffers
// ---------------------------------------------------------------------------

#[test]
fn ring_telemetry_try_decode_rejects_control_exactly_one_word_short() {
    let min_bytes = (CONTROL_MIN_WORDS as usize) * 4;
    let short = vec![0u8; min_bytes - 4];
    let ring = Megakernel::encode_empty_ring(1).unwrap();
    let err = RingTelemetry::try_decode(&short, &ring)
        .expect_err("control one word short of minimum must reject");
    assert!(matches!(err, PipelineError::Backend { .. }));
    let msg = err.to_string();
    assert!(msg.contains("Fix:"), "error must be actionable: {msg}");
}

#[test]
fn try_read_observable_rejects_u32_max_index_overflow() {
    let control = Megakernel::encode_control(false, 1, 0).unwrap();
    let err = Megakernel::try_read_observable(&control, u32::MAX)
        .expect_err("u32::MAX observable index must overflow");
    let msg = err.to_string();
    assert!(
        msg.contains("overflow") || msg.contains("Fix:"),
        "error must mention overflow: {msg}"
    );
}

#[test]
fn try_read_debug_log_accepts_zero_capacity_with_zero_cursor() {
    let log = protocol::try_encode_empty_debug_log(0).expect("zero capacity must encode");
    let records =
        Megakernel::try_read_debug_log(&log).expect("zero cursor on zero capacity must succeed");
    assert!(records.is_empty());
}

#[test]
fn dispatch_megakernel_bytes_rejects_one_byte_short_of_workitem() {
    let backend =
        WgpuBackend::new().expect("live GPU backend must initialize for dispatch contract test");
    let dispatcher = WgpuMegakernelDispatcher::new(&backend);
    let short = vec![0u8; std::mem::size_of::<MegakernelWorkItem>() - 1];
    let config = MegakernelConfig::default();
    let err = dispatcher
        .dispatch_megakernel_bytes(&short, &config)
        .expect_err("one byte short of MegakernelWorkItem must reject");
    let msg = err.to_string();
    assert!(msg.contains("Fix:"), "error must be actionable: {msg}");
    assert!(
        msg.contains("MegakernelWorkItem"),
        "error must mention MegakernelWorkItem: {msg}"
    );
}

// ---------------------------------------------------------------------------
// 3. Async worker dispatch is observable
// ---------------------------------------------------------------------------

#[test]
fn priority_accounting_is_observable_from_requeue_occupancy() {
    let control = Megakernel::encode_control(false, 1, 0).unwrap();
    let mut ring = Megakernel::encode_empty_ring(8).unwrap();
    Megakernel::publish_slot(&mut ring, 0, 0, opcode::NOP, &[]).unwrap();
    Megakernel::publish_slot(&mut ring, 1, 0, opcode::NOP, &[]).unwrap();
    Megakernel::publish_slot(&mut ring, 2, 0, opcode::NOP, &[]).unwrap();
    for slot_idx in [1, 2] {
        let base = slot_idx * (protocol::SLOT_WORDS as usize) * 4;
        ring[base..base + 4].copy_from_slice(&slot::REQUEUE.to_le_bytes());
    }
    let telemetry = RingTelemetry::decode(&control, &ring);
    let accounting = telemetry.priority_accounting();
    assert_eq!(
        accounting.requeue_count, 2,
        "requeue occupancy must be observable"
    );
    assert_eq!(accounting.aged_promotions, 0);
    assert_eq!(accounting.max_priority_age, 0);
}

#[test]
fn control_snapshot_gracefully_truncates_metrics_at_short_buffer() {
    let mut buf = vec![0u8; ((control::METRICS_BASE + 5) as usize) * 4];
    for i in 0..5 {
        let off = ((control::METRICS_BASE + i) as usize) * 4;
        buf[off..off + 4].copy_from_slice(&(100 + i).to_le_bytes());
    }
    let snapshot = ControlSnapshot::decode(&buf);
    assert_eq!(
        snapshot.metrics.len(),
        5,
        "metrics must stop at buffer end without panic"
    );
    assert!(snapshot.metrics.contains(&(0, 100)));
    assert!(snapshot.metrics.contains(&(4, 104)));
}

#[test]
fn post_dispatch_observability_decodes_done_count_epoch_and_observable() {
    let mut control = Megakernel::encode_control(false, 1, 4).unwrap();
    let done_off = (control::DONE_COUNT as usize) * 4;
    control[done_off..done_off + 4].copy_from_slice(&42u32.to_le_bytes());
    let epoch_off = (control::EPOCH as usize) * 4;
    control[epoch_off..epoch_off + 4].copy_from_slice(&7u32.to_le_bytes());
    let observable_off = (control::OBSERVABLE_BASE as usize) * 4;
    control[observable_off..observable_off + 4].copy_from_slice(&0x1234u32.to_le_bytes());

    let snapshot = ControlSnapshot::decode(&control);
    assert_eq!(
        snapshot.done_count, 42,
        "done_count must be observable after dispatch"
    );
    assert_eq!(snapshot.epoch, 7, "epoch must be observable after dispatch");
    assert_eq!(
        Megakernel::read_observable(&control, 0),
        0x1234,
        "observable results must be readable"
    );
}

#[test]
fn async_dispatch_empty_queue_returns_zeroed_report_without_panic() {
    let backend =
        WgpuBackend::new().expect("live GPU backend must initialize for empty-queue dispatch test");
    let dispatcher = WgpuMegakernelDispatcher::new(&backend);
    let config = MegakernelConfig::default();
    let report = dispatcher
        .dispatch_megakernel(&[], &config)
        .expect("empty queue dispatch must succeed");
    assert_eq!(report.items_processed, 0);
    assert_eq!(report.items_remaining, 0);
    assert_eq!(report.wall_time, std::time::Duration::ZERO);
}

#[test]
fn async_dispatch_wall_time_observable_on_live_backend() {
    let backend = WgpuBackend::new()
        .expect("live GPU backend must initialize for async dispatch observability test");
    let dispatcher = WgpuMegakernelDispatcher::new(&backend);
    let items = vec![MegakernelWorkItem {
        op_handle: opcode::NOP,
        input_handle: 0,
        output_handle: 0,
        param: 0,
    }];
    let mut config = MegakernelConfig::default();
    config.max_wall_time = std::time::Duration::from_secs(5);
    let report = dispatcher
        .dispatch_megakernel(&items, &config)
        .expect("dispatch must succeed on live backend");
    assert!(
        report.wall_time > std::time::Duration::ZERO,
        "wall_time must be observable (greater than zero)"
    );
}

#[test]
fn window_telemetry_aggregates_required_and_lookahead_counts() {
    let control = Megakernel::encode_control(false, 1, 0).unwrap();
    let mut ring = Megakernel::encode_empty_ring(4).unwrap();
    let window_opcode = 0xABCD;
    Megakernel::publish_slot(
        &mut ring,
        0,
        1,
        window_opcode,
        &[99, WindowClass::Required.into_wire(), 1],
    )
    .unwrap();
    Megakernel::publish_slot(
        &mut ring,
        1,
        1,
        window_opcode,
        &[99, WindowClass::Required.into_wire(), 2],
    )
    .unwrap();
    Megakernel::publish_slot(
        &mut ring,
        2,
        1,
        window_opcode,
        &[99, WindowClass::Lookahead.into_wire(), 3],
    )
    .unwrap();
    Megakernel::publish_slot(
        &mut ring,
        3,
        1,
        window_opcode,
        &[77, WindowClass::Required.into_wire(), 4],
    )
    .unwrap();

    let telemetry = RingTelemetry::decode_with_window_opcodes(&control, &ring, &[window_opcode]);
    assert_eq!(telemetry.windows.len(), 2);
    let win99 = telemetry
        .windows
        .iter()
        .find(|w| w.ticket == 99)
        .expect("ticket 99 must be present");
    assert_eq!(win99.required_slots, 2);
    assert_eq!(win99.lookahead_slots, 1);
    let win77 = telemetry
        .windows
        .iter()
        .find(|w| w.ticket == 77)
        .expect("ticket 77 must be present");
    assert_eq!(win77.required_slots, 1);
    assert_eq!(win77.lookahead_slots, 0);
}
