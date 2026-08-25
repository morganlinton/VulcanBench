use super::{reserve_decoded_vec_capacity, LebReader};
use crate::serial::wire::{Reader, MAX_OPAQUE_PAYLOAD_LEN};

#[test]
fn wire_decode_reservation_reports_capacity_overflow() {
    let mut bytes = Vec::<u8>::new();
    let error = reserve_decoded_vec_capacity(&mut bytes, usize::MAX, "test wire bytes")
        .expect_err("Fix: impossible wire reserve must fail before allocation.");

    assert!(
        error.contains("failed to reserve test wire bytes"),
        "Fix: wire decode reserve errors must name the field that failed: {error}"
    );
}

/// LEB128-encode a usize value into a Vec<u8> (little-endian, unsigned).
fn leb_encode_usize(mut value: usize) -> Vec<u8> {
    let mut out = Vec::new();
    loop {
        let mut byte = (value & 0x7F) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if value == 0 {
            break;
        }
    }
    out
}

#[test]
fn node_payload_len_at_limit_is_accepted() {
    // MAX_OPAQUE_PAYLOAD_LEN itself must not be rejected by leb_len.
    let encoded = leb_encode_usize(MAX_OPAQUE_PAYLOAD_LEN);
    let mut reader = Reader {
        bytes: &encoded,
        pos: 0,
        depth: 0,
    };
    let result = reader.leb_len(MAX_OPAQUE_PAYLOAD_LEN, "node payload length");
    assert!(
        result.is_ok(),
        "payload_len == MAX_OPAQUE_PAYLOAD_LEN must be accepted: {:?}",
        result
    );
    assert_eq!(result.unwrap(), MAX_OPAQUE_PAYLOAD_LEN);
}

#[test]
fn node_payload_len_exceeds_limit_rejected() {
    // A payload_len of MAX_OPAQUE_PAYLOAD_LEN + 1 must be rejected with an
    // actionable error naming the field. Previously leb_len was called with
    // usize::MAX, so this would have been silently accepted (producing a
    // "truncated" error only when the subsequent Reader::take ran out of bytes,
    // with no indication of which field was wrong).
    let over_limit = MAX_OPAQUE_PAYLOAD_LEN + 1;
    let encoded = leb_encode_usize(over_limit);
    let mut reader = Reader {
        bytes: &encoded,
        pos: 0,
        depth: 0,
    };
    let err = reader
        .leb_len(MAX_OPAQUE_PAYLOAD_LEN, "node payload length")
        .expect_err(
            "Fix: payload_len beyond MAX_OPAQUE_PAYLOAD_LEN must be rejected at the leb_len gate",
        );
    assert!(
        err.contains("node payload length"),
        "error must name the field 'node payload length': {err}"
    );
    assert!(
        err.contains("exceeds limit"),
        "error must say 'exceeds limit' to be actionable: {err}"
    );
}

#[test]
fn wire_decode_reservation_reuses_existing_capacity() {
    let mut bytes = Vec::<u8>::with_capacity(16);
    let original_capacity = bytes.capacity();

    reserve_decoded_vec_capacity(&mut bytes, 8, "test wire bytes")
        .expect("Fix: lower target capacity should reuse existing decode storage.");

    assert_eq!(bytes.capacity(), original_capacity);
}
