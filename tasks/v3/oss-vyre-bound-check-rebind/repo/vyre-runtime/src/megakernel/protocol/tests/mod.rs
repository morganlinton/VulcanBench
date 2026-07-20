use super::{
    control, count_done_ring_slots, debug, decode_load_miss, encode_load_miss, read_debug_log,
    read_debug_log_into, read_done_count, read_epoch, read_metrics_into, read_observable, slot,
    try_encode_control, try_encode_empty_debug_log, try_encode_empty_ring,
    try_encode_empty_ring_into, try_encode_load_miss, try_encode_load_miss_into,
    try_read_debug_log, try_read_debug_log_into, try_read_done_count, try_read_epoch,
    try_read_metrics_into, try_read_observable, try_slot_byte_len, try_slot_word_base,
    try_slot_word_index, ProtocolError, MAX_ENCODED_DEBUG_RECORDS, MAX_ENCODED_OBSERVABLE_SLOTS,
    MAX_ENCODED_RING_SLOTS, STATUS_WORD,
};

#[test]
#[allow(clippy::assertions_on_constants)]
fn control_regions_do_not_alias() {
    let metrics_end = control::METRICS_BASE + control::METRICS_SLOTS;
    assert!(metrics_end <= control::EPOCH);
    assert!(control::EPOCH < control::OBSERVABLE_BASE);
}

#[test]
fn count_done_ring_slots_counts_only_done_status_words() {
    let mut ring = Vec::new();
    try_encode_empty_ring_into(4, &mut ring).unwrap();
    for (slot_idx, status) in [slot::DONE, slot::CLAIMED, slot::DONE, slot::EMPTY]
        .into_iter()
        .enumerate()
    {
        let word_idx = slot_idx * super::SLOT_WORDS as usize + STATUS_WORD as usize;
        let byte_idx = word_idx * 4;
        ring[byte_idx..byte_idx + 4].copy_from_slice(&status.to_le_bytes());
    }
    assert_eq!(count_done_ring_slots(&ring, 4), Some(2));
    assert_eq!(count_done_ring_slots(&ring, 0), None);
    assert_eq!(count_done_ring_slots(&ring[..8], 4), None);
    let mut unaligned = vec![0xAA];
    unaligned.extend_from_slice(&ring);
    assert_eq!(count_done_ring_slots(&unaligned[1..], 4), Some(2));
}

#[test]
fn allocating_encoders_reject_allocation_cap_before_reserving() {
    let control_err = try_encode_control(false, 1, MAX_ENCODED_OBSERVABLE_SLOTS + 1)
        .expect_err("observable cap exceeded");
    let err_str = control_err.to_string();
    assert!(
        err_str.contains("observable"),
        "control cap error: {}",
        err_str
    );
    let ring_err =
        try_encode_empty_ring(MAX_ENCODED_RING_SLOTS + 1).expect_err("ring cap exceeded");
    let err_str = ring_err.to_string();
    assert!(
        err_str.contains("ring") || err_str.contains("slot"),
        "ring cap error: {}",
        err_str
    );
    let debug_err =
        try_encode_empty_debug_log(MAX_ENCODED_DEBUG_RECORDS + 1).expect_err("debug cap exceeded");
    let err_str = debug_err.to_string();
    assert!(err_str.contains("debug"), "debug cap error: {}", err_str);
}

#[test]
fn allocating_encoders_preallocate_exact_protocol_capacity() {
    let control = try_encode_control(false, 1, 16).unwrap();
    assert_eq!(control.capacity(), control.len());

    let ring = try_encode_empty_ring(16).unwrap();
    assert_eq!(ring.capacity(), ring.len());

    let debug_log = try_encode_empty_debug_log(16).unwrap();
    assert_eq!(debug_log.capacity(), debug_log.len());
}

#[test]
fn metrics_decode_into_reuses_capacity_without_overreserve() {
    let mut control = super::try_encode_control(false, 1, 0).unwrap();
    let word_idx = control::METRICS_BASE as usize;
    control[word_idx * 4..word_idx * 4 + 4].copy_from_slice(&9_u32.to_le_bytes());

    let mut out = Vec::with_capacity(control::METRICS_SLOTS as usize);
    let initial_capacity = out.capacity();
    read_metrics_into(&control, &mut out);
    assert_eq!(out, vec![(0, 9)]);
    assert_eq!(out.capacity(), initial_capacity);

    try_read_metrics_into(&control, &mut out).unwrap();
    assert_eq!(out, vec![(0, 9)]);
    assert_eq!(out.capacity(), initial_capacity);
}

#[test]
fn metrics_decode_into_does_not_allocate_for_empty_metrics() {
    let control = super::try_encode_control(false, 1, 0).unwrap();

    let mut out = Vec::new();
    read_metrics_into(&control, &mut out);
    assert!(out.is_empty());
    assert_eq!(
        out.capacity(),
        0,
        "Fix: empty metrics snapshots must not allocate the full metrics window."
    );

    try_read_metrics_into(&control, &mut out).unwrap();
    assert!(out.is_empty());
    assert_eq!(
        out.capacity(),
        0,
        "Fix: strict empty metrics snapshots must not allocate the full metrics window."
    );
}

#[test]
fn metrics_decode_into_reserves_only_nonzero_metrics() {
    let mut control = super::try_encode_control(false, 1, 0).unwrap();
    let word_idx = (control::METRICS_BASE + 7) as usize;
    control[word_idx * 4..word_idx * 4 + 4].copy_from_slice(&13_u32.to_le_bytes());

    let mut out = Vec::new();
    read_metrics_into(&control, &mut out);
    assert_eq!(out, vec![(7, 13)]);
    assert!(
        out.capacity() < control::METRICS_SLOTS as usize,
        "Fix: sparse metrics decode must not reserve the entire metrics window."
    );

    out.clear();
    out.shrink_to_fit();
    try_read_metrics_into(&control, &mut out).unwrap();
    assert_eq!(out, vec![(7, 13)]);
    assert!(
        out.capacity() < control::METRICS_SLOTS as usize,
        "Fix: strict sparse metrics decode must not reserve the entire metrics window."
    );
}

#[test]
fn debug_log_decode_into_reuses_capacity_without_overreserve() {
    let mut debug_log = super::try_encode_empty_debug_log(2).unwrap();
    debug_log[(debug::CURSOR_WORD as usize) * 4..(debug::CURSOR_WORD as usize) * 4 + 4]
        .copy_from_slice(&debug::RECORD_WORDS.to_le_bytes());
    let record_start = debug::RECORDS_BASE as usize * 4;
    for (idx, value) in [7_u32, 1, 2, 3].into_iter().enumerate() {
        let byte_idx = record_start + idx * 4;
        debug_log[byte_idx..byte_idx + 4].copy_from_slice(&value.to_le_bytes());
    }

    let mut out = Vec::with_capacity(1);
    let initial_capacity = out.capacity();
    read_debug_log_into(&debug_log, &mut out);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].fmt_id, 7);
    assert_eq!(out.capacity(), initial_capacity);

    try_read_debug_log_into(&debug_log, &mut out).unwrap();
    assert_eq!(out.len(), 1);
    assert_eq!(out.capacity(), initial_capacity);
}

#[test]
fn debug_log_owned_decode_does_not_allocate_for_empty_log() {
    let debug_log = super::try_encode_empty_debug_log(64).unwrap();

    let records = read_debug_log(&debug_log);
    assert!(records.is_empty());
    assert_eq!(
        records.capacity(),
        0,
        "Fix: empty debug-log decode must not allocate the full record capacity."
    );

    let records = try_read_debug_log(&debug_log).unwrap();
    assert!(records.is_empty());
    assert_eq!(
        records.capacity(),
        0,
        "Fix: strict empty debug-log decode must not allocate the full record capacity."
    );
}

#[test]
fn encode_load_miss_produces_correct_slot_layout() {
    let bytes = encode_load_miss(42, true);
    assert_eq!(bytes.len(), 64);
    assert_eq!(decode_load_miss(&bytes, 0), Some((42, true)));
}

#[test]
fn try_encode_load_miss_produces_correct_slot_layout_and_reuses_storage() {
    let bytes = try_encode_load_miss(42, true).expect("Fix: valid load-miss slot must encode");
    assert_eq!(bytes.len(), try_slot_byte_len().unwrap());
    assert_eq!(decode_load_miss(&bytes, 0), Some((42, true)));

    let mut reused = Vec::with_capacity(128);
    let initial_capacity = reused.capacity();
    try_encode_load_miss_into(7, false, &mut reused)
        .expect("Fix: caller-owned load-miss slot must encode");
    assert_eq!(reused.len(), 64);
    assert_eq!(reused.capacity(), initial_capacity);
    assert_eq!(decode_load_miss(&reused, 0), Some((7, false)));
}

#[test]
fn slot_word_arithmetic_rejects_overflow_without_panic() {
    let base_result = std::panic::catch_unwind(|| try_slot_word_base(u32::MAX));
    assert!(
        base_result.is_ok(),
        "fallible slot word-base arithmetic must return an error instead of panicking"
    );
    let err = base_result.unwrap().unwrap_err().to_string();
    assert!(
        err.contains("slot word base overflows"),
        "Fix: overflow must identify the slot word-base contract, got: {err}"
    );

    let word_result = std::panic::catch_unwind(|| try_slot_word_index(0, super::SLOT_WORDS));
    assert!(
        word_result.is_ok(),
        "fallible slot-local word arithmetic must return an error instead of panicking"
    );
    let err = word_result.unwrap().unwrap_err().to_string();
    assert!(
        err.contains("outside SLOT_WORDS"),
        "Fix: out-of-slot word errors must name the slot layout contract, got: {err}"
    );
}

#[test]
fn decode_load_miss_returns_none_for_wrong_opcode() {
    let mut bytes = encode_load_miss(42, true);
    // Corrupt the opcode word
    bytes[4..8].copy_from_slice(&0_u32.to_le_bytes());
    assert_eq!(decode_load_miss(&bytes, 0), None);
}

#[test]
fn decode_load_miss_returns_none_for_short_buffer() {
    assert_eq!(decode_load_miss(&[0u8; 60], 0), None);
}

#[test]
fn decode_load_miss_uses_slot_index_correctly() {
    let mut ring = vec![0u8; 128];
    let slot0 = encode_load_miss(7, false);
    let slot1 = encode_load_miss(99, true);
    ring[..64].copy_from_slice(&slot0);
    ring[64..128].copy_from_slice(&slot1);
    assert_eq!(decode_load_miss(&ring, 0), Some((7, false)));
    assert_eq!(decode_load_miss(&ring, 1), Some((99, true)));
    assert_eq!(decode_load_miss(&ring, 2), None);
}

// ── Regression tests for the Law-10 / P0 silent-fallback bugs ──────────────────────────────

/// try_read_epoch must return a structured Err on a truncated control buffer;
/// the infallible read_epoch must NOT simply return 0 without any diagnostic
/// (before the fix it did — the pump would stall indefinitely on malformed DMA
/// readbacks with no signal).
#[test]
fn read_epoch_on_truncated_control_returns_err_not_zero() {
    // 4 bytes is too short to contain the epoch word — not a valid control buffer.
    let short = [0u8; 4];
    let err = try_read_epoch(&short)
        .expect_err("Fix: try_read_epoch must return Err on a 4-byte truncated buffer");
    // Confirm the error names the missing word or the misalignment rather than
    // succeeding with a stale zero.
    let msg = err.to_string();
    assert!(
        msg.contains("control") && (msg.contains("missing") || msg.contains("mismatch") || msg.contains("bytes")),
        "Fix: error must describe the control-buffer defect, got: {msg}"
    );
    // read_epoch returns the same 0 it always did, but the infallible path
    // now emits tracing::error! — we cannot assert the log in a unit test,
    // but we CAN assert that the strict counterpart disagrees.
    let silent = read_epoch(&short);
    assert_eq!(
        silent, 0,
        "read_epoch on truncated buffer must return the sentinel 0 (loud, not silent — see tracing::error! emitted above)"
    );
}

/// try_read_done_count must return a structured Err on a truncated buffer.
#[test]
fn read_done_count_on_truncated_control_returns_err_not_zero() {
    let short = [0u8; 4];
    let err = try_read_done_count(&short)
        .expect_err("Fix: try_read_done_count must return Err on a 4-byte truncated buffer");
    assert!(
        err.to_string().contains("control"),
        "Fix: error must name the control buffer, got: {}",
        err
    );
    // The infallible read_done_count returns 0 and emits tracing::error!.
    // Prove the infallible path disagrees with try_read_done_count:
    assert_eq!(read_done_count(&short), 0);
}

/// try_read_observable on a truncated buffer must Err; read_observable must not
/// silently return 0 — observable index 0 at truncated size cannot be valid.
#[test]
fn read_observable_on_truncated_control_returns_err_not_zero() {
    let short = [0u8; 4];
    let err = try_read_observable(&short, 0)
        .expect_err("Fix: try_read_observable must return Err on a 4-byte truncated buffer");
    // The error must mention the control buffer and missing data.
    let msg = err.to_string();
    assert!(
        msg.contains("control"),
        "Fix: error must name the control buffer, got: {msg}"
    );
    // Infallible read_observable falls back to 0 loudly.
    assert_eq!(read_observable(&short, 0), 0);
}

/// Strict decode paths parse real epoch values correctly from a well-formed buffer,
/// proving try_read_epoch is not just always-Err.
#[test]
fn try_read_epoch_parses_real_epoch_from_well_formed_control() {
    let control = try_encode_control(false, 1, 0)
        .expect("Fix: well-formed control encode must succeed");
    // Fresh kernel: epoch starts at 0.
    let epoch = try_read_epoch(&control)
        .expect("Fix: try_read_epoch must succeed on a well-formed control buffer");
    assert_eq!(epoch, 0, "Fix: fresh epoch must be 0");

    // Inject a non-zero epoch word to prove the decoder reads the right offset.
    let epoch_word_idx = super::control::EPOCH as usize;
    let mut patched = control.clone();
    patched[epoch_word_idx * 4..epoch_word_idx * 4 + 4]
        .copy_from_slice(&42_u32.to_le_bytes());
    let patched_epoch = try_read_epoch(&patched)
        .expect("Fix: try_read_epoch must succeed on a patched well-formed buffer");
    assert_eq!(patched_epoch, 42, "Fix: patched epoch must be the injected value 42");
}

/// Misaligned control buffers (not a multiple of 4 bytes) must be an Err,
/// not a silent 0.  This is the MisalignedByteLength code path.
#[test]
fn read_epoch_on_misaligned_control_returns_err() {
    // Take a valid control buffer and add one byte to make it misaligned.
    let control = try_encode_control(false, 1, 0)
        .expect("Fix: well-formed control encode must succeed");
    let mut misaligned = control.clone();
    misaligned.push(0xAB);
    let err = try_read_epoch(&misaligned)
        .expect_err("Fix: try_read_epoch must Err on misaligned buffer (len % 4 != 0)");
    assert!(
        matches!(err, ProtocolError::MisalignedByteLength { .. }),
        "Fix: misaligned buffer must produce MisalignedByteLength, got: {err:?}"
    );
}
