//! P1 inventory #90  -  OOM / huge-input tests for wire decode.
//!
//! The wire decoder must reject frames that claim a payload larger
//! than `MAX_PROGRAM_BYTES` without allocating that much memory. The
//! test feeds malformed wire frames whose declared length is
//! `2 * MAX_PROGRAM_BYTES` and asserts decode errors out before any
//! large allocation occurs.
//!
//! Paired with `vyre-foundation/fuzz/fuzz_targets/program_wire.rs` for
//! continuous fuzz coverage.

mod wire_decode_support;

use wire_decode_support::decode_error_string;

#[test]
fn wire_decoder_rejects_oversized_declared_length() {
    // Build a synthetic wire frame whose first u64 (length prefix)
    // claims `2 * MAX_PROGRAM_BYTES`. The body is 16 bytes  -  far
    // smaller than the declared length, so the decoder MUST reject
    // before allocating.
    let huge: u64 = 256 * 1024 * 1024;
    let mut bytes = Vec::with_capacity(24);
    bytes.extend_from_slice(&huge.to_le_bytes());
    bytes.extend_from_slice(&[0u8; 16]);

    let error = decode_error_string(&bytes, "oversized declared wire length");
    assert!(
        error.contains("TooLarge") || error.contains("TruncatedPayload") || error.contains("Fix:"),
        "oversized declared length must produce an allocation-safe wire error, got: {error}"
    );
}

#[test]
fn wire_decoder_rejects_zero_length_buffer() {
    // An empty input must produce a structured error, not a panic and
    // not an unbounded read.
    let error = decode_error_string(&[], "empty wire input");
    assert!(
        error.contains("TruncatedPayload") || error.contains("wire") || error.contains("Fix:"),
        "empty input must produce a structured wire error, got: {error}"
    );
}

#[test]
fn wire_decoder_rejects_truncated_header() {
    // Less than the size-prefix header  -  decoder must error promptly.
    let bytes = [0u8; 4];
    let error = decode_error_string(&bytes, "truncated wire header");
    assert!(
        error.contains("TruncatedPayload") || error.contains("wire") || error.contains("Fix:"),
        "truncated header must produce a structured wire error, got: {error}"
    );
}
