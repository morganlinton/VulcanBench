//! Strict host-protocol contracts for the megakernel ring/control ABI.
//!
//! The compatibility readers keep returning zero for legacy callers, but every
//! untrusted readback path needs a strict API that rejects malformed byte views.

use vyre_runtime::megakernel::{protocol, Megakernel};

fn write_word(bytes: &mut [u8], word_idx: usize, value: u32) {
    let off = word_idx * 4;
    bytes[off..off + 4].copy_from_slice(&value.to_le_bytes());
}

#[test]
fn strict_control_readers_reject_misaligned_buffers() {
    let err = protocol::try_read_done_count(&[0u8; 5]).expect_err("misaligned control must fail");
    let msg = err.to_string();
    assert!(
        msg.contains("Fix:") && msg.contains("whole u32"),
        "misaligned protocol error must be actionable: {msg}"
    );
}

#[test]
fn strict_observable_reader_rejects_out_of_range_word() {
    let control = Megakernel::encode_control(false, 1, 0).unwrap();
    assert_eq!(
        Megakernel::read_observable(&control, 0),
        0,
        "legacy observable reader keeps compatibility zero for missing words"
    );
    let err = Megakernel::try_read_observable(&control, 0)
        .expect_err("strict observable reader must reject absent observable word");
    let msg = err.to_string();
    assert!(
        msg.contains("Fix:") && msg.contains("word"),
        "missing observable error must be actionable: {msg}"
    );
}

#[test]
fn strict_metrics_reader_requires_complete_metrics_window() {
    let truncated = vec![0u8; protocol::control::METRICS_BASE as usize * 4];
    let err = Megakernel::try_read_metrics(&truncated)
        .expect_err("strict metrics reader must reject missing metrics window");
    assert!(
        err.to_string().contains("Fix:"),
        "metrics error must be actionable: {err}"
    );
}

#[test]
fn strict_debug_log_rejects_partial_record_cursor() {
    let mut log = Megakernel::encode_empty_debug_log(1).unwrap();
    write_word(&mut log, protocol::debug::CURSOR_WORD as usize, 3);
    let err = Megakernel::try_read_debug_log(&log)
        .expect_err("debug cursor must advance in complete PRINTF records");
    let msg = err.to_string();
    assert!(
        msg.contains("Fix:") && msg.contains("whole PRINTF records"),
        "partial debug-log error must be actionable: {msg}"
    );
}

#[test]
fn strict_debug_log_rejects_cursor_beyond_capacity() {
    let mut log = Megakernel::encode_empty_debug_log(1).unwrap();
    write_word(
        &mut log,
        protocol::debug::CURSOR_WORD as usize,
        protocol::debug::RECORD_WORDS + 1,
    );
    let err = Megakernel::try_read_debug_log(&log)
        .expect_err("debug cursor beyond encoded capacity must be rejected");
    let msg = err.to_string();
    assert!(
        msg.contains("cursor must stay within"),
        "cursor-capacity error must be actionable: {msg}"
    );
}

#[test]
fn strict_encoders_reject_overflow_without_panicking() {
    for error in [
        protocol::try_encode_control(false, 1, u32::MAX).unwrap_err(),
        protocol::try_encode_empty_ring(u32::MAX).unwrap_err(),
        protocol::try_encode_empty_debug_log(u32::MAX).unwrap_err(),
    ] {
        let msg = error.to_string();
        assert!(
            msg.contains("Fix:"),
            "overflow encoder error must be actionable: {msg}"
        );
    }
}

#[test]
fn publish_slot_rejects_every_inflight_status() {
    let mut ring = Megakernel::encode_empty_ring(1).unwrap();
    for status in [
        protocol::slot::PUBLISHED,
        protocol::slot::CLAIMED,
        protocol::slot::WAIT_IO,
        protocol::slot::YIELD,
        protocol::slot::REQUEUE,
        protocol::slot::FAULT,
    ] {
        write_word(&mut ring, protocol::STATUS_WORD as usize, status);
        let err = Megakernel::publish_slot(&mut ring, 0, 0, protocol::opcode::NOP, &[])
            .expect_err("host must not overwrite in-flight slot state");
        assert!(
            err.to_string().contains("not publishable"),
            "slot status {status} must produce actionable publication error: {err}"
        );
    }

    write_word(
        &mut ring,
        protocol::STATUS_WORD as usize,
        protocol::slot::DONE,
    );
    Megakernel::publish_slot(&mut ring, 0, 0, protocol::opcode::NOP, &[])
        .expect("DONE slot may be recycled");
}

#[test]
fn publish_slot_rejects_reserved_non_builtin_opcodes() {
    let mut ring = Megakernel::encode_empty_ring(1).unwrap();
    for opcode in [
        protocol::opcode::SYSTEM_MASK,
        protocol::opcode::RESERVED_MAX_RANGE_MIN,
        protocol::opcode::RESERVED_MAX_RANGE_MIN + 1,
    ] {
        let err = Megakernel::publish_slot(&mut ring, 0, 0, opcode, &[])
            .expect_err("reserved non-builtin opcode must be rejected before publication");
        assert!(
            err.to_string().contains("reserved"),
            "opcode {opcode:#x} must produce actionable opcode validation error: {err}"
        );
    }

    Megakernel::publish_slot(&mut ring, 0, 0, 0x4000_0000, &[])
        .expect("caller-defined opcode outside reserved ranges may be published");
}
