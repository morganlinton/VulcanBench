//! Megakernel host-protocol contracts: encode-decode round-trips and
//! strict boundary checks for control, ring, debug-log, metrics, and
//! observable reads.
//!
//! Covers:
//! - control/ring encode-decode exact boundaries
//! - metrics/read observables bounds

use vyre_runtime::megakernel::{
    protocol::{self, control, debug},
    Megakernel,
};
use vyre_runtime::PipelineError;

fn write_word(bytes: &mut [u8], word_idx: usize, value: u32) {
    let off = word_idx * 4;
    bytes[off..off + 4].copy_from_slice(&value.to_le_bytes());
}

// ---------------------------------------------------------------------------
// 1. control/ring encode-decode exact boundaries
// ---------------------------------------------------------------------------

#[test]
fn try_encode_empty_ring_zero_slots_produces_empty_vec() {
    let ring = protocol::try_encode_empty_ring(0).expect("zero slots must encode");
    assert!(ring.is_empty(), "zero-slot ring must be empty");
}

#[test]
fn publish_slot_rejects_zero_slot_ring() {
    let mut ring = protocol::encode_empty_ring(0).unwrap();
    let err = Megakernel::publish_slot(&mut ring, 0, 0, protocol::opcode::NOP, &[])
        .expect_err("empty ring must reject publish");
    assert!(matches!(err, PipelineError::QueueFull { .. }));
}

#[test]
fn encode_control_zero_observables_exactly_min_words() {
    let ctrl = Megakernel::encode_control(false, 0, 0).unwrap();
    let expected = (protocol::CONTROL_MIN_WORDS as usize) * 4;
    assert_eq!(
        ctrl.len(),
        expected,
        "control with zero observables must equal CONTROL_MIN_WORDS * 4"
    );
}

#[test]
fn try_read_observable_rejects_buffer_ending_exactly_at_observable_base() {
    // Buffer has words [0 .. OBSERVABLE_BASE-1], so word OBSERVABLE_BASE is missing.
    let buf = vec![0u8; (control::OBSERVABLE_BASE as usize) * 4];
    let err = Megakernel::try_read_observable(&buf, 0)
        .expect_err("buffer ending at observable base must reject index 0");
    let msg = err.to_string();
    assert!(msg.contains("Fix:"), "error must be actionable: {msg}");
}

#[test]
fn try_read_observable_accepts_boundary_when_buffer_has_one_extra_word() {
    let mut buf = vec![0u8; (control::OBSERVABLE_BASE as usize + 1) * 4];
    write_word(&mut buf, control::OBSERVABLE_BASE as usize, 0xCAFE_BABE);
    let val = Megakernel::try_read_observable(&buf, 0)
        .expect("observable 0 must be readable with exactly one word past base");
    assert_eq!(val, 0xCAFE_BABE);
}

#[test]
fn try_read_debug_log_rejects_zero_byte_buffer() {
    let err = Megakernel::try_read_debug_log(&[]).expect_err("zero-byte debug-log must reject");
    let msg = err.to_string();
    assert!(msg.contains("Fix:"), "error must be actionable: {msg}");
}

#[test]
fn try_encode_empty_debug_log_zero_capacity_roundtrips() {
    let log = protocol::try_encode_empty_debug_log(0).expect("zero capacity must encode");
    assert_eq!(log.len(), (debug::RECORDS_BASE as usize) * 4);
    let records = Megakernel::try_read_debug_log(&log).expect("zero cursor must decode");
    assert!(records.is_empty());
}

// ---------------------------------------------------------------------------
// 2. metrics/read observables bounds
// ---------------------------------------------------------------------------

#[test]
fn read_metrics_on_empty_buffer_returns_empty() {
    let metrics = Megakernel::read_metrics(&[]);
    assert!(metrics.is_empty(), "empty buffer must yield empty metrics");
}

#[test]
fn try_read_metrics_rejects_buffer_ending_exactly_at_metrics_base() {
    let buf = vec![0u8; (control::METRICS_BASE as usize) * 4];
    let err =
        Megakernel::try_read_metrics(&buf).expect_err("buffer ending at metrics base must reject");
    let msg = err.to_string();
    assert!(msg.contains("Fix:"), "error must be actionable: {msg}");
}

#[test]
fn read_observable_returns_zero_for_out_of_bounds_on_minimal_buffer() {
    let ctrl = Megakernel::encode_control(false, 1, 0).unwrap();
    // Minimal control has no observable words; any index is out of bounds.
    assert_eq!(Megakernel::read_observable(&ctrl, 0), 0);
    assert_eq!(Megakernel::read_observable(&ctrl, 99), 0);
}

#[test]
fn strict_metrics_window_exact_boundary_succeeds() {
    let mut buf = vec![0u8; ((control::METRICS_BASE + control::METRICS_SLOTS) as usize) * 4];
    write_word(&mut buf, (control::METRICS_BASE + 5) as usize, 42);
    let metrics = Megakernel::try_read_metrics(&buf).expect("exact boundary must succeed");
    assert_eq!(metrics, vec![(5, 42)]);
}
