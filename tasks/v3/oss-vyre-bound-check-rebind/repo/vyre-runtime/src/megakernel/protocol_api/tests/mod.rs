use super::*;
use crate::megakernel::protocol::{
    slot, ARG0_WORD, ARGS_PER_SLOT, OPCODE_WORD, PRIORITY_WORD, SLOT_WORDS, STATUS_WORD,
    TENANT_WORD,
};
use crate::megakernel::scheduler;
use crate::megakernel::MegakernelWorkItem;

mod publish_contracts;

#[test]
fn encode_control_produces_aligned_buffer() {
    let buf = Megakernel::encode_control(false, 1, 4).unwrap();
    assert!(
        buf.len() % 4 == 0,
        "control buffer must be u32-word aligned"
    );
    assert!(
        !buf.is_empty(),
        "control buffer must have at least the fixed header"
    );
}

#[test]
fn encode_control_with_shutdown_sets_flag() {
    let buf = Megakernel::encode_control(true, 1, 0).unwrap();
    // The shutdown word should be non-zero.
    let shutdown_word = u32::from_le_bytes([
        buf[protocol::control::SHUTDOWN as usize * 4],
        buf[protocol::control::SHUTDOWN as usize * 4 + 1],
        buf[protocol::control::SHUTDOWN as usize * 4 + 2],
        buf[protocol::control::SHUTDOWN as usize * 4 + 3],
    ]);
    assert_ne!(shutdown_word, 0, "shutdown flag must be set");
}

#[test]
fn try_encode_control_delegates_to_encode_control() {
    let a = Megakernel::encode_control(false, 2, 8).unwrap();
    let b = Megakernel::try_encode_control(false, 2, 8).unwrap();
    assert_eq!(a, b, "try_encode_control must produce identical output");
}

#[test]
fn encode_into_reuses_and_zeroes_protocol_buffers() {
    let mut control = vec![0xAA; 4096];
    let control_capacity = control.capacity();
    Megakernel::try_encode_control_into(false, 2, 8, &mut control).unwrap();
    assert_eq!(control.capacity(), control_capacity);
    assert_eq!(
        control,
        Megakernel::try_encode_control(false, 2, 8).unwrap()
    );

    let mut ring = vec![0xAA; 4096];
    let ring_capacity = ring.capacity();
    Megakernel::try_encode_empty_ring_into(4, &mut ring).unwrap();
    assert_eq!(ring.capacity(), ring_capacity);
    assert_eq!(ring, Megakernel::try_encode_empty_ring(4).unwrap());

    let mut debug_log = vec![0xAA; 4096];
    let debug_capacity = debug_log.capacity();
    Megakernel::try_encode_empty_debug_log_into(4, &mut debug_log).unwrap();
    assert_eq!(debug_log.capacity(), debug_capacity);
    assert_eq!(
        debug_log,
        Megakernel::try_encode_empty_debug_log(4).unwrap()
    );
}

#[test]
fn encode_empty_ring_respects_slot_count() {
    let buf = Megakernel::encode_empty_ring(16).unwrap();
    let expected_bytes = 16 * SLOT_WORDS as usize * 4;
    assert_eq!(
        buf.len(),
        expected_bytes,
        "ring must be slot_count * SLOT_WORDS * 4 bytes"
    );
}

#[test]
fn encode_empty_ring_zero_slots() {
    let buf = Megakernel::encode_empty_ring(0).unwrap();
    assert!(buf.is_empty(), "0 slots must produce empty buffer");
}

#[test]
fn publish_slot_writes_and_reads_back() {
    let mut ring = Megakernel::encode_empty_ring(4).unwrap();
    Megakernel::publish_slot(&mut ring, 0, 42, protocol::opcode::STORE_U32, &[100, 200]).unwrap();

    // Verify status is PUBLISHED.
    let status = read_word(&ring, 0, STATUS_WORD as usize);
    assert_eq!(status, slot::PUBLISHED);

    // Verify opcode.
    let op = read_word(&ring, 0, OPCODE_WORD as usize);
    assert_eq!(op, protocol::opcode::STORE_U32);

    // Verify tenant.
    let tenant = read_word(&ring, 0, TENANT_WORD as usize);
    assert_eq!(tenant, 42);

    let priority = read_word(&ring, 0, PRIORITY_WORD as usize);
    assert_eq!(priority, scheduler::priority::NORMAL);

    // Verify args.
    let a0 = read_word(&ring, 0, ARG0_WORD as usize);
    let a1 = read_word(&ring, 0, ARG0_WORD as usize + 1);
    assert_eq!(a0, 100);
    assert_eq!(a1, 200);
}

#[test]
fn publish_slot_rejects_inflight_slot() {
    let mut ring = Megakernel::encode_empty_ring(4).unwrap();
    // Publish once (now status = PUBLISHED).
    Megakernel::publish_slot(&mut ring, 0, 1, protocol::opcode::STORE_U32, &[1]).unwrap();
    // Try to publish again  -  slot is PUBLISHED (not EMPTY/DONE).
    let err = Megakernel::publish_slot(&mut ring, 0, 1, protocol::opcode::STORE_U32, &[2])
        .expect_err("must reject publishing to an in-flight slot");
    assert!(
        err.to_string().contains("not publishable"),
        "unexpected error: {err}"
    );
}

#[test]
fn publish_slot_rejects_out_of_bounds() {
    let mut ring = Megakernel::encode_empty_ring(2).unwrap();
    let err = Megakernel::publish_slot(&mut ring, 99, 1, protocol::opcode::STORE_U32, &[1])
        .expect_err("must reject slot_idx beyond ring capacity");
    assert!(
        err.to_string().contains("slot_idx exceeds ring slot count"),
        "unexpected error: {err}"
    );
}

#[test]
fn publish_slot_rejects_too_many_args() {
    let mut ring = Megakernel::encode_empty_ring(2).unwrap();
    let too_many = vec![0u32; ARGS_PER_SLOT as usize + 1];
    let err = Megakernel::publish_slot(&mut ring, 0, 1, protocol::opcode::STORE_U32, &too_many)
        .expect_err("must reject args exceeding ARGS_PER_SLOT");
    assert!(
        err.to_string().contains("too many args for one slot"),
        "unexpected error: {err}"
    );
}

#[test]
fn publish_slot_allows_republish_after_done() {
    let mut ring = Megakernel::encode_empty_ring(4).unwrap();
    // Publish, then manually mark as DONE.
    Megakernel::publish_slot(&mut ring, 0, 1, protocol::opcode::STORE_U32, &[1]).unwrap();
    write_word(&mut ring, 0, STATUS_WORD as usize, slot::DONE);
    // Should succeed  -  DONE slots are recyclable.
    Megakernel::publish_slot(&mut ring, 0, 1, protocol::opcode::ATOMIC_ADD, &[2]).unwrap();
    let op = read_word(&ring, 0, OPCODE_WORD as usize);
    assert_eq!(op, protocol::opcode::ATOMIC_ADD);
}

#[test]
fn ring_slot_transition_state_machine_accepts_legal_lifecycle() {
    let mut ring = Megakernel::encode_empty_ring(4).unwrap();
    Megakernel::publish_slot(&mut ring, 0, 1, protocol::opcode::STORE_U32, &[1]).unwrap();

    let previous = Megakernel::transition_slot_status(&mut ring, 0, RingSlotTransition::Claim)
        .expect("Fix: PUBLISHED slots must be claimable");
    assert_eq!(previous, slot::PUBLISHED);
    assert_eq!(read_word(&ring, 0, STATUS_WORD as usize), slot::CLAIMED);

    let previous = Megakernel::transition_slot_status(&mut ring, 0, RingSlotTransition::Done)
        .expect("Fix: CLAIMED slots must complete to DONE");
    assert_eq!(previous, slot::CLAIMED);
    assert_eq!(read_word(&ring, 0, STATUS_WORD as usize), slot::DONE);

    Megakernel::publish_slot(&mut ring, 1, 1, protocol::opcode::STORE_U32, &[2]).unwrap();
    Megakernel::transition_slot_status(&mut ring, 1, RingSlotTransition::Cancel)
        .expect("Fix: unclaimed published slots must be cancellable");
    assert_eq!(read_word(&ring, 1, STATUS_WORD as usize), slot::EMPTY);

    Megakernel::publish_slot(&mut ring, 2, 1, protocol::opcode::STORE_U32, &[3]).unwrap();
    Megakernel::transition_slot_status(&mut ring, 2, RingSlotTransition::Fault)
        .expect("Fix: in-flight published slots must transition to FAULT");
    assert_eq!(read_word(&ring, 2, STATUS_WORD as usize), slot::FAULT);
}

#[test]
fn ring_slot_transition_state_machine_rejects_illegal_edges_without_mutation() {
    let mut ring = Megakernel::encode_empty_ring(2).unwrap();

    let err = Megakernel::transition_slot_status(&mut ring, 0, RingSlotTransition::Done)
        .expect_err("EMPTY slots cannot complete");
    assert!(
        err.to_string().contains("done requires CLAIMED"),
        "Fix: illegal transition error must name the required source state, got: {err}"
    );
    assert_eq!(read_word(&ring, 0, STATUS_WORD as usize), slot::EMPTY);

    Megakernel::publish_slot(&mut ring, 0, 1, protocol::opcode::STORE_U32, &[1]).unwrap();
    Megakernel::transition_slot_status(&mut ring, 0, RingSlotTransition::Claim).unwrap();
    let before = ring.clone();
    let err = Megakernel::transition_slot_status(&mut ring, 0, RingSlotTransition::Cancel)
        .expect_err("CLAIMED slots are worker-owned and cannot be cancelled by host");
    assert!(
        err.to_string().contains("cancel requires an unclaimed"),
        "Fix: illegal cancel error must name ownership boundary, got: {err}"
    );
    assert_eq!(ring, before);

    let err = Megakernel::transition_slot_status(&mut ring, 1, RingSlotTransition::Publish)
        .expect_err("status-only publish is forbidden");
    assert!(
        err.to_string().contains("publish_slot"),
        "Fix: status-only publish rejection must direct callers to payload-safe APIs, got: {err}"
    );
    assert_eq!(read_word(&ring, 1, STATUS_WORD as usize), slot::EMPTY);
}

#[test]
fn batch_publish_writes_items_plus_fence() {
    let mut ring = Megakernel::encode_empty_ring(8).unwrap();
    let items: Vec<(u32, Vec<u32>)> = vec![
        (protocol::opcode::STORE_U32, vec![10, 20]),
        (protocol::opcode::ATOMIC_ADD, vec![30, 40]),
    ];
    let slots_used = Megakernel::batch_publish(&mut ring, 0, 1, &items, 99).unwrap();
    // 2 items + 1 fence = 3 slots consumed.
    assert_eq!(slots_used, 3);

    // Last slot should be BATCH_FENCE.
    let fence_op = read_word(&ring, 2, OPCODE_WORD as usize);
    assert_eq!(fence_op, protocol::opcode::BATCH_FENCE);
}

#[test]
fn batch_publish_rejects_fence_collision_without_partial_publish() {
    let mut ring = Megakernel::encode_empty_ring(4).unwrap();
    write_word(&mut ring, 1, STATUS_WORD as usize, slot::PUBLISHED);
    let before = ring.clone();
    let items: Vec<(u32, Vec<u32>)> = vec![(protocol::opcode::STORE_U32, vec![10, 20])];

    let result = Megakernel::batch_publish(&mut ring, 0, 1, &items, 99);

    assert!(result.is_err(), "fence collision must reject the batch");
    assert_eq!(ring, before, "rejection must not publish earlier slots");
}

#[test]
fn read_done_count_starts_at_zero() {
    let control = Megakernel::encode_control(false, 1, 0).unwrap();
    assert_eq!(Megakernel::read_done_count(&control), 0);
}

#[test]
fn read_epoch_starts_at_zero() {
    let control = Megakernel::encode_control(false, 1, 0).unwrap();
    assert_eq!(Megakernel::read_epoch(&control), 0);
}

#[test]
fn encode_empty_debug_log_round_trips() {
    let log = Megakernel::encode_empty_debug_log(32).unwrap();
    let records = Megakernel::read_debug_log(&log);
    assert!(
        records.is_empty(),
        "fresh debug log must contain zero records"
    );
}

#[test]
fn read_metrics_on_fresh_control_returns_empty() {
    let control = Megakernel::encode_control(false, 1, 4).unwrap();
    let metrics = Megakernel::read_metrics(&control);
    assert!(
        metrics.is_empty(),
        "fresh control buffer must have no non-zero metric counters"
    );
}

#[test]
fn validate_control_bytes_rejects_too_short() {
    let err = validate_control_bytes(&[0u8; 4]).expect_err("must reject undersized control buffer");
    assert!(
        err.to_string().contains("expected at least"),
        "unexpected error: {err}"
    );
}

#[test]
fn validate_control_bytes_rejects_misaligned() {
    let err = validate_control_bytes(&[0u8; 101])
        .expect_err("must reject non-4-byte-aligned control buffer");
    assert!(
        err.to_string().contains("4-byte alignment"),
        "unexpected error: {err}"
    );
}

#[test]

fn validate_control_bytes_accepts_valid() {
    let control = Megakernel::encode_control(false, 1, 0).unwrap();
    validate_control_bytes(&control).expect("Fix: valid control buffer must pass validation");
}

#[test]
fn validate_debug_log_bytes_rejects_wrong_size() {
    let err = validate_debug_log_bytes(&[0u8; 4]).expect_err("must reject undersized debug log");
    assert!(
        err.to_string().contains("expected exactly"),
        "unexpected error: {err}"
    );
}

#[test]
fn validate_debug_log_bytes_accepts_valid() {
    let log = Megakernel::encode_empty_debug_log(protocol::debug::RECORD_CAPACITY).unwrap();
    validate_debug_log_bytes(&log).expect("Fix: valid debug log must pass validation");
}

#[test]
fn packed_slot_publish_roundtrips() {
    let mut ring = Megakernel::encode_empty_ring(4).unwrap();
    let ops: Vec<(u8, Vec<u32>)> = vec![
        (protocol::opcode::STORE_U32 as u8, vec![10, 20]),
        (protocol::opcode::ATOMIC_ADD as u8, vec![30]),
    ];
    Megakernel::publish_packed_slot(&mut ring, 0, 1, &ops).unwrap();

    let status = read_word(&ring, 0, STATUS_WORD as usize);
    assert_eq!(status, slot::PUBLISHED);

    let op = read_word(&ring, 0, OPCODE_WORD as usize);
    assert_eq!(op, protocol::opcode::PACKED_SLOT);
}

#[test]
fn packed_slot_rejects_overflow() {
    let mut ring = Megakernel::encode_empty_ring(4).unwrap();
    // Each op gets 3 arg words, so 5 ops × 3 args = 15 words > 12 budget.
    let ops: Vec<(u8, Vec<u32>)> = (0..5).map(|i| (i as u8, vec![1, 2, 3])).collect();
    let err = Megakernel::publish_packed_slot(&mut ring, 0, 1, &ops)
        .expect_err("must reject packed slot exceeding arg budget");
    assert!(
        err.to_string()
            .contains("exceeds the 12-word slot argument budget"),
        "unexpected error: {err}"
    );
}

// Helper: read a u32 word from a ring buffer at (slot_idx, word_idx).
fn read_word(ring: &[u8], slot_idx: usize, word_idx: usize) -> u32 {
    let base = slot_idx * SLOT_WORDS as usize * 4;
    let off = base + word_idx * 4;
    u32::from_le_bytes([ring[off], ring[off + 1], ring[off + 2], ring[off + 3]])
}

// Helper: read a native u32 word from a ring-word buffer at (slot_idx, word_idx).
fn read_word_words(ring: &[u32], slot_idx: usize, word_idx: usize) -> u32 {
    ring[slot_idx * SLOT_WORDS as usize + word_idx]
}

// Helper: write a u32 word into a ring buffer at (slot_idx, word_idx).
fn write_word(ring: &mut [u8], slot_idx: usize, word_idx: usize, value: u32) {
    let base = slot_idx * SLOT_WORDS as usize * 4;
    let off = base + word_idx * 4;
    ring[off..off + 4].copy_from_slice(&value.to_le_bytes());
}
