use super::*;
use crate::megakernel::protocol::{slot, ARGS_PER_SLOT, SLOT_WORDS, STATUS_WORD};

/// Regression test for the P0 Law-10 silent-fallback bug:
/// WindowDescriptor::into_batch previously returned an empty BatchDescriptor
/// (silently dropping ALL window work) when try_into_batch failed.
///
/// After the fix, into_batch panics loudly on failure so the operator
/// cannot miss the error.  The caller is directed to try_into_batch.
///
/// Concretely: a payload with 11 u32 args plus the 2-word [ticket, class]
/// prefix = 13 words > ARGS_PER_SLOT_USIZE (12), so try_into_batch returns
/// QueueFull.  The old into_batch silently dropped everything; the new one
/// panics.
#[test]
#[should_panic(expected = "WindowDescriptor::into_batch failed")]
fn into_batch_oversized_payload_panics_not_silent_drop() {
    // payload of 11 u32s: ticket(1) + class(1) + payload(11) = 13 > 12 max args
    let window = WindowDescriptor::new(
        0,
        0,
        SlotOpcode::Builtin(BuiltinOpcode::Nop),
        1,
        vec![vec![0u32; 11]],
        vec![],
    );
    // This must panic, not silently return an empty BatchDescriptor.
    let _ = window.into_batch();
}

/// try_into_batch on a well-formed window must return a BatchDescriptor with
/// exactly as many items as required + lookahead entries combined.
#[test]
fn try_into_batch_well_formed_window_returns_all_items() {
    let window = WindowDescriptor::new(
        0,
        0,
        SlotOpcode::Builtin(BuiltinOpcode::Nop),
        99,
        vec![vec![1u32], vec![2u32]],   // 2 required
        vec![vec![3u32]],               // 1 lookahead
    );
    let batch = window
        .try_into_batch()
        .expect("Fix: well-formed window must produce a BatchDescriptor");
    assert_eq!(
        batch.items.len(),
        3,
        "Fix: try_into_batch must return all 3 slot descriptors (2 required + 1 lookahead), not drop any"
    );
}

fn read_word(buf: &[u8], slot_idx: u32, word_idx: u32) -> u32 {
    let base = (slot_idx as usize) * (SLOT_WORDS as usize) * 4;
    let off = base + (word_idx as usize) * 4;
    u32::from_le_bytes(buf[off..off + 4].try_into().unwrap())
}

#[test]
fn single_descriptor_publishes_normal_slot() {
    let mut ring = Megakernel::try_encode_empty_ring(4).unwrap();
    let slot = SlotDescriptor::single(
        7,
        SlotOpcode::Builtin(BuiltinOpcode::StoreU32),
        vec![11, 13],
    );
    slot.publish_into(&mut ring, 1).unwrap();
    assert_eq!(read_word(&ring, 1, STATUS_WORD), slot::PUBLISHED);
}

#[test]
fn packed_descriptor_uses_packed_opcode() {
    let mut ring = Megakernel::try_encode_empty_ring(2).unwrap();
    let slot = SlotDescriptor::packed(
        3,
        vec![
            PackedOpDescriptor::new(9, vec![1, 2, 3]),
            PackedOpDescriptor::new(10, vec![4]),
        ],
    );
    slot.publish_into(&mut ring, 0).unwrap();
    assert_eq!(read_word(&ring, 0, STATUS_WORD), slot::PUBLISHED);
    assert_eq!(
        read_word(&ring, 0, protocol::OPCODE_WORD),
        protocol::opcode::PACKED_SLOT
    );
}

#[test]
fn batch_descriptor_publishes_sequential_slots() {
    let mut ring = Megakernel::try_encode_empty_ring(4).unwrap();
    let batch = BatchDescriptor::new(
        1,
        vec![
            SlotDescriptor::single(0, SlotOpcode::Builtin(BuiltinOpcode::Nop), vec![]),
            SlotDescriptor::single(0, SlotOpcode::Builtin(BuiltinOpcode::AtomicAdd), vec![1, 2]),
        ],
    );
    let consumed = batch.publish_into(&mut ring).unwrap();
    assert_eq!(consumed, 2);
    assert_eq!(read_word(&ring, 1, STATUS_WORD), slot::PUBLISHED);
    assert_eq!(read_word(&ring, 2, STATUS_WORD), slot::PUBLISHED);
}

#[test]
fn batch_descriptor_rejects_slot_index_overflow_before_publication() {
    let mut ring = Megakernel::try_encode_empty_ring(4).unwrap();
    let before = ring.clone();
    let batch = BatchDescriptor::new(
        u32::MAX,
        vec![
            SlotDescriptor::single(0, SlotOpcode::Builtin(BuiltinOpcode::Nop), vec![]),
            SlotDescriptor::single(0, SlotOpcode::Builtin(BuiltinOpcode::Nop), vec![]),
        ],
    );

    let err = batch.publish_into(&mut ring).unwrap_err();
    assert!(
        err.to_string().contains("overflows u32"),
        "overflowing descriptor batch must fail with an actionable message: {err}"
    );
    assert_eq!(
        ring, before,
        "overflow preflight must not partially publish slots before failing"
    );
}

#[test]
fn normal_slot_respects_wire_arg_budget() {
    let mut ring = Megakernel::try_encode_empty_ring(1).unwrap();
    let slot = SlotDescriptor::single(
        0,
        SlotOpcode::Builtin(BuiltinOpcode::Memcpy),
        vec![0; ARGS_PER_SLOT as usize + 1],
    );
    let err = slot.publish_into(&mut ring, 0).unwrap_err();
    assert!(matches!(err, PipelineError::QueueFull { .. }));
}

#[test]
fn window_descriptor_publishes_required_then_lookahead() {
    let mut ring = Megakernel::try_encode_empty_ring(4).unwrap();
    let window = WindowDescriptor::new(
        1,
        5,
        SlotOpcode::Custom(0xF101),
        77,
        vec![vec![17], vec![42]],
        vec![vec![99]],
    );
    let consumed = window.publish_into(&mut ring).unwrap();
    assert_eq!(consumed, 3);
    assert_eq!(read_word(&ring, 1, STATUS_WORD), slot::PUBLISHED);
    assert_eq!(read_word(&ring, 2, STATUS_WORD), slot::PUBLISHED);
    assert_eq!(read_word(&ring, 3, STATUS_WORD), slot::PUBLISHED);
    assert_eq!(read_word(&ring, 1, protocol::ARG0_WORD), 77);
    assert_eq!(
        read_word(&ring, 1, protocol::ARG0_WORD + 1),
        WindowClass::Required.into_wire()
    );
    assert_eq!(
        read_word(&ring, 3, protocol::ARG0_WORD + 1),
        WindowClass::Lookahead.into_wire()
    );
}

#[test]
fn window_publish_rejects_overflow_before_publication() {
    let mut ring = Megakernel::try_encode_empty_ring(2).unwrap();
    let before = ring.clone();
    let window = WindowDescriptor::new(
        u32::MAX,
        5,
        SlotOpcode::Builtin(BuiltinOpcode::Nop),
        77,
        vec![vec![], vec![]],
        vec![],
    );
    let err = window.publish_into(&mut ring).unwrap_err();
    assert!(
        err.to_string().contains("overflows u32"),
        "overflowing window must fail with an actionable message: {err}"
    );
    assert_eq!(ring, before);
}

#[test]
fn window_publish_rejects_oversized_payload_before_publication() {
    let mut ring = Megakernel::try_encode_empty_ring(2).unwrap();
    let before = ring.clone();
    let window = WindowDescriptor::new(
        0,
        5,
        SlotOpcode::Builtin(BuiltinOpcode::Nop),
        77,
        vec![vec![0; ARGS_PER_SLOT as usize]],
        vec![],
    );
    let err = window.publish_into(&mut ring).unwrap_err();
    assert!(
        err.to_string().contains("too many args"),
        "oversized window payload must fail with an actionable message: {err}"
    );
    assert_eq!(ring, before);
}
