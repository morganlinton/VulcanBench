//! Protocol/security edge cases for the megakernel ring and control ABI.
//!
//! Covers:
//! - Slot publish bounds (exact boundary, empty ring)
//! - Packed slot overflow (12-word boundary, u8 opcode_count overflow)
//! - Done/epoch/metrics readback with short buffers
//! - Queue packing validation (BatchDescriptor/WindowDescriptor overflow)
//! - No silent CPU fallback (runtime-level explicit GPU mode selection)

use vyre_runtime::megakernel::{
    descriptor::{BatchDescriptor, BuiltinOpcode, SlotDescriptor, SlotOpcode, WindowDescriptor},
    protocol::{self, control, slot, ARGS_PER_SLOT, STATUS_WORD},
    Megakernel, MegakernelExecutionMode,
};
use vyre_runtime::PipelineError;

fn write_word(bytes: &mut [u8], word_idx: usize, value: u32) {
    let off = word_idx * 4;
    bytes[off..off + 4].copy_from_slice(&value.to_le_bytes());
}

// ---------------------------------------------------------------------------
// 1. Slot publish bounds
// ---------------------------------------------------------------------------

#[test]
fn slot_publish_exact_boundary_last_slot_ok_next_fails() {
    let mut ring = Megakernel::encode_empty_ring(4).unwrap();
    Megakernel::publish_slot(&mut ring, 3, 0, protocol::opcode::NOP, &[])
        .expect("last slot (slot_count - 1) must be publishable");
    let err = Megakernel::publish_slot(&mut ring, 4, 0, protocol::opcode::NOP, &[])
        .expect_err("slot_idx == slot_count must be rejected");
    assert!(matches!(err, PipelineError::QueueFull { .. }));
}

#[test]
fn slot_publish_empty_ring_rejects_any_slot() {
    let mut ring = Megakernel::encode_empty_ring(0).unwrap();
    let err = Megakernel::publish_slot(&mut ring, 0, 0, protocol::opcode::NOP, &[])
        .expect_err("empty ring must reject any slot publish");
    assert!(matches!(err, PipelineError::QueueFull { .. }));
}

// ---------------------------------------------------------------------------
// 2. Packed slot overflow
// ---------------------------------------------------------------------------

#[test]
fn packed_slot_exact_12_word_boundary_succeeds() {
    let mut ring = Megakernel::encode_empty_ring(1).unwrap();
    let args = vec![0u32; 11];
    Megakernel::publish_packed_slot(&mut ring, 0, 0, &[(1u8, args)])
        .expect("packed slot with exactly 12 words must succeed");
    let base = (STATUS_WORD as usize) * 4;
    let status = u32::from_le_bytes(ring[base..base + 4].try_into().unwrap());
    assert_eq!(status, slot::PUBLISHED);
}

#[test]
fn packed_slot_13_word_boundary_fails() {
    let mut ring = Megakernel::encode_empty_ring(1).unwrap();
    let args = vec![0u32; 12];
    let err = Megakernel::publish_packed_slot(&mut ring, 0, 0, &[(1u8, args)])
        .expect_err("packed slot with 13 words must fail");
    assert!(matches!(err, PipelineError::QueueFull { .. }));
    let msg = err.to_string();
    assert!(
        msg.contains("12-word") || msg.contains("exceeds") || msg.contains("budget"),
        "error must mention slot argument budget: {msg}"
    );
}

#[test]
fn packed_slot_256_ops_rejects_u8_opcode_count_overflow() {
    let mut ring = Megakernel::encode_empty_ring(1).unwrap();
    let ops: Vec<_> = (0..256).map(|_| (0u8, vec![])).collect();
    let err = Megakernel::publish_packed_slot(&mut ring, 0, 0, &ops)
        .expect_err("256 inner ops must fail u8 opcode_count overflow");
    assert!(matches!(err, PipelineError::QueueFull { .. }));
    assert!(
        err.to_string().contains("255"),
        "error must mention u8 limit: {err}"
    );
}

// ---------------------------------------------------------------------------
// 3. Done/epoch/metrics readback with short buffers
// ---------------------------------------------------------------------------

#[test]
fn try_read_done_count_rejects_buffer_missing_word() {
    // DONE_COUNT is at word 1; 4 bytes only covers word 0.
    let short = vec![0u8; 4];
    let err = protocol::try_read_done_count(&short)
        .expect_err("buffer missing DONE_COUNT word must fail");
    assert!(err.to_string().contains("Fix:"));
}

#[test]
fn try_read_epoch_rejects_buffer_missing_epoch_word() {
    let short = vec![0u8; (control::EPOCH as usize) * 4];
    let err = protocol::try_read_epoch(&short).expect_err("buffer missing EPOCH word must fail");
    assert!(err.to_string().contains("Fix:"));
}

#[test]
fn try_read_metrics_rejects_short_buffer() {
    let short = vec![0u8; ((control::METRICS_BASE + 1) as usize) * 4];
    let err = Megakernel::try_read_metrics(&short).expect_err("short metrics buffer must fail");
    assert!(err.to_string().contains("Fix:"));
}

#[test]
fn try_read_metrics_accepts_exact_size_buffer() {
    let exact = vec![0u8; ((control::METRICS_BASE + control::METRICS_SLOTS) as usize) * 4];
    let metrics =
        Megakernel::try_read_metrics(&exact).expect("exact-size metrics buffer must succeed");
    assert!(metrics.is_empty());
}

#[test]
fn try_read_done_count_accepts_minimal_buffer() {
    let mut buf = vec![0u8; (control::DONE_COUNT as usize + 1) * 4];
    write_word(&mut buf, control::DONE_COUNT as usize, 42);
    assert_eq!(protocol::try_read_done_count(&buf).unwrap(), 42);
}

#[test]
fn try_read_epoch_accepts_minimal_buffer() {
    let mut buf = vec![0u8; (control::EPOCH as usize + 1) * 4];
    write_word(&mut buf, control::EPOCH as usize, 7);
    assert_eq!(protocol::try_read_epoch(&buf).unwrap(), 7);
}

// ---------------------------------------------------------------------------
// 4. Queue packing validation
// ---------------------------------------------------------------------------

#[test]
fn batch_descriptor_rejects_items_exceeding_ring_capacity() {
    let mut ring = Megakernel::encode_empty_ring(2).unwrap();
    let batch = BatchDescriptor::new(
        0,
        vec![
            SlotDescriptor::single(0, SlotOpcode::Builtin(BuiltinOpcode::Nop), vec![]),
            SlotDescriptor::single(0, SlotOpcode::Builtin(BuiltinOpcode::Nop), vec![]),
            SlotDescriptor::single(0, SlotOpcode::Builtin(BuiltinOpcode::Nop), vec![]),
        ],
    );
    let err = batch
        .publish_into(&mut ring)
        .expect_err("batch exceeding ring must fail");
    assert!(matches!(err, PipelineError::QueueFull { .. }));
}

#[test]
fn batch_publish_rejects_u32_slot_index_overflow() {
    let mut ring = Megakernel::encode_empty_ring(1).unwrap();
    let err = Megakernel::batch_publish(
        &mut ring,
        u32::MAX,
        0,
        &[(protocol::opcode::NOP, vec![])],
        0,
    )
    .expect_err("batch publish at u32::MAX must fail on OOB ring");
    assert!(matches!(err, PipelineError::QueueFull { .. }));
}

#[test]
fn window_descriptor_rejects_prefixed_arg_overflow() {
    let mut ring = Megakernel::encode_empty_ring(1).unwrap();
    // WindowDescriptor prefixes [ticket, class_tag] (2 words) to the payload.
    // A payload of ARGS_PER_SLOT args makes the total 2 + ARGS_PER_SLOT > ARGS_PER_SLOT.
    let window = WindowDescriptor::new(
        0,
        0,
        SlotOpcode::Builtin(BuiltinOpcode::Nop),
        77,
        vec![vec![0u32; ARGS_PER_SLOT as usize]],
        vec![],
    );
    let err = window
        .publish_into(&mut ring)
        .expect_err("prefixed args exceeding budget must fail");
    assert!(matches!(err, PipelineError::QueueFull { .. }));
}

// ---------------------------------------------------------------------------
// 5. No silent CPU fallback
// ---------------------------------------------------------------------------

#[test]
fn execution_mode_variants_are_always_gpu() {
    // Interpreter and Jit are both GPU execution modes; there is no CPU variant.
    for mode in [
        MegakernelExecutionMode::Interpreter,
        MegakernelExecutionMode::Jit,
    ] {
        let name = format!("{mode:?}").to_lowercase();
        assert!(
            !name.contains("cpu"),
            "execution mode {mode:?} must never imply CPU fallback"
        );
    }
}
