//! Adversarial packed-slot and batch-packing contracts: opcode-count
//! boundaries, duplicate inner ops, density limits, and recycle semantics.

use vyre_runtime::megakernel::{
    protocol::{self, slot},
    Megakernel,
};
use vyre_runtime::PipelineError;

fn write_word(bytes: &mut [u8], word_idx: usize, value: u32) {
    let off = word_idx * 4;
    bytes[off..off + 4].copy_from_slice(&value.to_le_bytes());
}

// ---------------------------------------------------------------------------
// 1. Packed-slot opcode-count boundary (max empty ops before metadata overflow)
// ---------------------------------------------------------------------------

#[test]
fn packed_slot_with_23_empty_ops_succeeds() {
    let mut ring = Megakernel::encode_empty_ring(1).unwrap();
    let ops: Vec<_> = (0..23)
        .map(|_| (protocol::opcode::NOP as u8, vec![]))
        .collect();
    Megakernel::publish_packed_slot(&mut ring, 0, 0, &ops)
        .expect("23 empty ops (max metadata-fit) must succeed");
    let status = u32::from_le_bytes(ring[..4].try_into().unwrap());
    assert_eq!(status, slot::PUBLISHED);
}

#[test]
fn packed_slot_with_24_empty_ops_rejects_metadata_overflow() {
    let mut ring = Megakernel::encode_empty_ring(1).unwrap();
    let ops: Vec<_> = (0..24)
        .map(|_| (protocol::opcode::NOP as u8, vec![]))
        .collect();
    let err = Megakernel::publish_packed_slot(&mut ring, 0, 0, &ops)
        .expect_err("24 empty ops must exceed 12-word metadata budget");
    assert!(matches!(err, PipelineError::QueueFull { .. }));
}

// ---------------------------------------------------------------------------
// 2. Duplicate inner opcodes at high density
// ---------------------------------------------------------------------------

#[test]
fn packed_slot_with_23_duplicate_nops_succeeds() {
    let mut ring = Megakernel::encode_empty_ring(1).unwrap();
    let ops: Vec<_> = (0..23)
        .map(|_| (protocol::opcode::NOP as u8, vec![]))
        .collect();
    Megakernel::publish_packed_slot(&mut ring, 0, 0, &ops)
        .expect("duplicate NOPs at max density must succeed");
}

#[test]
fn packed_slot_duplicate_opcodes_with_args_exactly_at_budget() {
    let mut ring = Megakernel::encode_empty_ring(1).unwrap();
    // 2 ops: metadata = ceil((2 + 2*2)/4) = 2 words. Args = 10 words -> total 12.
    let ops = vec![
        (protocol::opcode::STORE_U32 as u8, vec![0u32; 5]),
        (protocol::opcode::STORE_U32 as u8, vec![0u32; 5]),
    ];
    Megakernel::publish_packed_slot(&mut ring, 0, 0, &ops)
        .expect("duplicate opcodes with args exactly at 12-word budget must succeed");
}

#[test]
fn packed_slot_accepts_borrowed_arg_slices_without_owned_vecs() {
    let mut ring = Megakernel::encode_empty_ring(1).unwrap();
    let store_args = [7u32, 8, 9];
    let add_args = [10u32, 11];
    let ops = [
        (protocol::opcode::STORE_U32 as u8, store_args.as_slice()),
        (protocol::opcode::ATOMIC_ADD as u8, add_args.as_slice()),
    ];

    Megakernel::publish_packed_slot(&mut ring, 0, 0, &ops)
        .expect("borrowed arg slices must publish without forcing owned Vec args");

    let status = u32::from_le_bytes(ring[..4].try_into().unwrap());
    assert_eq!(status, slot::PUBLISHED);
}

#[test]
fn packed_slot_duplicate_opcodes_with_args_one_over_budget() {
    let mut ring = Megakernel::encode_empty_ring(1).unwrap();
    // 2 ops: metadata = 2 words. Args = 11 words -> total 13.
    let ops = vec![
        (protocol::opcode::STORE_U32 as u8, vec![0u32; 5]),
        (protocol::opcode::STORE_U32 as u8, vec![0u32; 6]),
    ];
    let err = Megakernel::publish_packed_slot(&mut ring, 0, 0, &ops)
        .expect_err("duplicate opcodes one word over budget must reject");
    assert!(matches!(err, PipelineError::QueueFull { .. }));
}

// ---------------------------------------------------------------------------
// 3. Batch-publish duplicate / overlap patterns
// ---------------------------------------------------------------------------

#[test]
fn batch_publish_with_fence_consumes_expected_slot_count() {
    let mut ring = Megakernel::encode_empty_ring(8).unwrap();
    let items = vec![
        (protocol::opcode::STORE_U32, vec![1, 2]),
        (protocol::opcode::STORE_U32, vec![3, 4]),
    ];
    let consumed = Megakernel::batch_publish(&mut ring, 0, 0, &items, 0xABCD).unwrap();
    assert_eq!(consumed, 3, "2 items + 1 fence = 3 slots");
}

#[test]
fn batch_publish_accepts_borrowed_arg_slices() {
    let mut ring = Megakernel::encode_empty_ring(8).unwrap();
    let first = [1u32, 2];
    let second = [3u32, 4];
    let items = [
        (protocol::opcode::STORE_U32, first.as_slice()),
        (protocol::opcode::ATOMIC_ADD, second.as_slice()),
    ];

    let consumed = Megakernel::batch_publish(&mut ring, 0, 0, &items, 0x55AA).unwrap();
    assert_eq!(consumed, 3, "2 borrowed-arg items + 1 fence = 3 slots");
}

#[test]
fn duplicate_batch_publish_to_adjacent_slots_succeeds() {
    let mut ring = Megakernel::encode_empty_ring(4).unwrap();
    let items = vec![(protocol::opcode::NOP, vec![])];
    let c1 = Megakernel::batch_publish(&mut ring, 0, 0, &items, 1).unwrap();
    let c2 = Megakernel::batch_publish(&mut ring, c1, 0, &items, 2).unwrap();
    assert_eq!(c1, 2);
    assert_eq!(c2, 2);
}

#[test]
fn batch_publish_at_u32_max_minus_one_rejects_due_to_overflow() {
    let mut ring = Megakernel::encode_empty_ring(1).unwrap();
    let err = Megakernel::batch_publish(
        &mut ring,
        u32::MAX - 1,
        0,
        &[
            (protocol::opcode::NOP, vec![]),
            (protocol::opcode::NOP, vec![]),
        ],
        0,
    )
    .expect_err("batch publish that would overflow u32 slot index must reject");
    assert!(matches!(err, PipelineError::QueueFull { .. }));
}

// ---------------------------------------------------------------------------
// 4. Packed-slot recycle after DONE
// ---------------------------------------------------------------------------

#[test]
fn packed_slot_can_be_recycled_after_marking_done() {
    let mut ring = Megakernel::encode_empty_ring(1).unwrap();
    let ops = vec![(protocol::opcode::NOP as u8, vec![42])];
    Megakernel::publish_packed_slot(&mut ring, 0, 0, &ops).unwrap();
    write_word(&mut ring, protocol::STATUS_WORD as usize, slot::DONE);

    let ops2 = vec![(protocol::opcode::STORE_U32 as u8, vec![1, 2])];
    Megakernel::publish_packed_slot(&mut ring, 0, 0, &ops2)
        .expect("packed slot must be recyclable after DONE");
}
