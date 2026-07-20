//! Async dispatch observability contracts for synthetic in-flight states.
//!
//! Verifies that telemetry decoders accurately reflect mixed active,
//! terminal, and faulted slot states without requiring a live GPU.

#![cfg(feature = "megakernel-batch")]

use vyre_runtime::megakernel::{
    descriptor::{SlotOpcode, WindowDescriptor},
    protocol::{self, control, slot},
    Megakernel, MegakernelExecutionMode, MegakernelLaunchRequest, RingTelemetry,
};

fn write_slot_status(ring: &mut [u8], slot_idx: u32, status: u32) {
    let base = (slot_idx as usize) * (protocol::SLOT_WORDS as usize) * 4;
    ring[base..base + 4].copy_from_slice(&status.to_le_bytes());
}

#[test]
fn telemetry_decode_counts_mixed_inflight_states() {
    let mut ring = Megakernel::encode_empty_ring(8).unwrap();
    Megakernel::publish_slot(&mut ring, 0, 1, protocol::opcode::NOP, &[]).unwrap();
    Megakernel::publish_slot(&mut ring, 1, 2, protocol::opcode::STORE_U32, &[1, 2]).unwrap();
    Megakernel::publish_slot(&mut ring, 2, 3, protocol::opcode::ATOMIC_ADD, &[3, 4]).unwrap();
    Megakernel::publish_slot(&mut ring, 3, 4, protocol::opcode::LOAD_U32, &[5, 6]).unwrap();
    Megakernel::publish_slot(&mut ring, 4, 5, protocol::opcode::COMPARE_SWAP, &[7, 8, 9]).unwrap();
    Megakernel::publish_slot(&mut ring, 5, 6, protocol::opcode::MEMCPY, &[10, 11, 12]).unwrap();

    write_slot_status(&mut ring, 2, slot::CLAIMED);
    write_slot_status(&mut ring, 3, slot::DONE);
    write_slot_status(&mut ring, 4, slot::WAIT_IO);
    write_slot_status(&mut ring, 5, slot::YIELD);
    write_slot_status(&mut ring, 6, slot::REQUEUE);
    write_slot_status(&mut ring, 7, slot::FAULT);

    let control = Megakernel::encode_control(false, 1, 0).unwrap();
    let telemetry = RingTelemetry::decode(&control, &ring);

    assert_eq!(telemetry.occupancy.published, 2); // slots 0, 1
    assert_eq!(telemetry.occupancy.claimed, 1); // slot 2
    assert_eq!(telemetry.occupancy.done, 1); // slot 3
    assert_eq!(telemetry.occupancy.wait_io, 1); // slot 4
    assert_eq!(telemetry.occupancy.yield_count, 1); // slot 5
    assert_eq!(telemetry.occupancy.requeue, 1); // slot 6
    assert_eq!(telemetry.occupancy.fault, 1); // slot 7
    assert_eq!(telemetry.occupancy.empty, 0);
}

#[test]
fn active_slots_for_opcode_filters_only_inflight() {
    let mut ring = Megakernel::encode_empty_ring(4).unwrap();
    let op = 0xBEEF;
    Megakernel::publish_slot(&mut ring, 0, 0, op, &[1]).unwrap();
    Megakernel::publish_slot(&mut ring, 1, 0, op, &[2]).unwrap();
    Megakernel::publish_slot(&mut ring, 2, 0, op, &[3]).unwrap();
    write_slot_status(&mut ring, 2, slot::DONE);

    let telemetry = RingTelemetry::decode(&Megakernel::encode_control(false, 1, 0).unwrap(), &ring);
    let active = telemetry.active_slots_for_opcode(op);
    assert_eq!(active.len(), 2, "only PUBLISHED slots count as active");
    assert_eq!(active[0].slot_idx, 0);
    assert_eq!(active[1].slot_idx, 1);
}

#[test]
fn active_windows_excludes_fully_terminal_windows() {
    let mut ring = Megakernel::encode_empty_ring(3).unwrap();
    let window_opcode = 0xF103;
    let window = WindowDescriptor::new(
        0,
        7,
        SlotOpcode::Custom(window_opcode),
        42,
        vec![vec![10], vec![20]],
        vec![vec![30]],
    );
    window.publish_into(&mut ring).unwrap();

    write_slot_status(&mut ring, 0, slot::DONE);

    let telemetry = RingTelemetry::decode_with_window_opcodes(
        &Megakernel::encode_control(false, 1, 0).unwrap(),
        &ring,
        &[window_opcode],
    );
    assert_eq!(telemetry.windows.len(), 1);
    assert!(
        telemetry.windows[0].is_active(),
        "partially done window is still active"
    );
    assert_eq!(telemetry.active_windows().len(), 1);

    write_slot_status(&mut ring, 1, slot::DONE);
    write_slot_status(&mut ring, 2, slot::DONE);
    let telemetry2 = RingTelemetry::decode_with_window_opcodes(
        &Megakernel::encode_control(false, 1, 0).unwrap(),
        &ring,
        &[window_opcode],
    );
    assert!(
        !telemetry2.windows[0].is_active(),
        "fully done window is not active"
    );
    assert!(telemetry2.active_windows().is_empty());
}

#[test]
fn priority_accounting_reflects_requeue_pressure() {
    let mut ring = Megakernel::encode_empty_ring(4).unwrap();
    for i in 0..4 {
        Megakernel::publish_slot(&mut ring, i, 0, protocol::opcode::NOP, &[]).unwrap();
        write_slot_status(&mut ring, i, slot::REQUEUE);
    }
    let telemetry = RingTelemetry::decode(&Megakernel::encode_control(false, 1, 0).unwrap(), &ring);
    let accounting = telemetry.priority_accounting();
    assert_eq!(accounting.requeue_count, 4);
}

#[test]
fn recommend_launch_from_mixed_pressure_ring_selects_jit() {
    let mut control = Megakernel::encode_control(false, 1, 0).unwrap();
    for i in 0..8u32 {
        let off = ((control::METRICS_BASE + i) as usize) * 4;
        control[off..off + 4].copy_from_slice(&1u32.to_le_bytes());
    }
    let mut ring = Megakernel::encode_empty_ring(2).unwrap();
    write_slot_status(&mut ring, 0, slot::REQUEUE);
    write_slot_status(&mut ring, 1, slot::YIELD);

    let telemetry = RingTelemetry::decode(&control, &ring);
    let rec = telemetry
        .recommend_launch(MegakernelLaunchRequest::direct(4096, 64, 256))
        .expect("telemetry must produce launch recommendation");
    assert_eq!(rec.execution_mode, MegakernelExecutionMode::Jit);
    assert!(rec.promote_hot_opcodes);
    assert!(rec.age_priority_work);
}
