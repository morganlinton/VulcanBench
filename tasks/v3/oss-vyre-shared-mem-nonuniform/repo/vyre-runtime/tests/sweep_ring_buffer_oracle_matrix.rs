//! Handwritten oracle matrix for megakernel host ring buffer contracts.
//!
//! Compares `HostRing` publish/read/try_done_count behavior against an independent
//! byte-layout oracle across hostile slot indices and encoded payloads.

#![forbid(unsafe_code)]

use vyre_runtime::megakernel::protocol;
use vyre_runtime::megakernel::ring::{HostRing, RingConsumer, RingProducer, SLOT_BYTES};

const RING_CASES: u32 = 256;

#[test]
fn host_ring_publish_read_oracle_matrix_matches_independent_slot_layout() {
    let mut assertions = 0usize;
    for case in 0..RING_CASES {
        let slot_count = 1 + (case % 16);
        let mut ring = HostRing::new(slot_count)
            .unwrap_or_else(|error| panic!("Fix: HostRing case {case} must construct: {error}"));
        let slot_idx = case % slot_count;
        let (encoded, resource_id, prefetch) = hostile_encoded_slot(case);

        RingProducer::publish(&mut ring, slot_idx, &encoded)
            .unwrap_or_else(|error| panic!("Fix: publish case {case} must succeed: {error}"));
        assertions += 1;

        let base = oracle_slot_byte_offset(slot_idx);
        assert_eq!(
            &ring.as_bytes()[base..base + SLOT_BYTES],
            encoded.as_slice(),
            "Fix: ring slot bytes case {case} must match the independent layout oracle."
        );
        assertions += 1;

        let mut read_back = [0u8; SLOT_BYTES];
        RingConsumer::read_slot(&ring, slot_idx, &mut read_back)
            .unwrap_or_else(|error| panic!("Fix: read_slot case {case} must succeed: {error}"));
        assert_eq!(
            read_back.as_slice(),
            encoded.as_slice(),
            "Fix: ring read_slot case {case} must round-trip encoded bytes."
        );
        assertions += 1;

        let decoded = protocol::decode_load_miss(ring.as_bytes(), slot_idx);
        assert_eq!(
            decoded,
            Some((resource_id, prefetch)),
            "Fix: decode_load_miss case {case} must match the independent load-miss oracle."
        );
        assertions += 1;
    }
    assert_eq!(assertions, RING_CASES as usize * 4);
}

#[test]
fn host_ring_done_count_oracle_matrix_matches_independent_status_scan() {
    let mut assertions = 0usize;
    for case in 0..RING_CASES {
        let slot_count = 2 + (case % 8);
        let mut ring =
            HostRing::new(slot_count).expect("Fix: ring must construct for done_count matrix.");
        let expected_done = {
            let bytes = ring.as_bytes_mut();
            let mut done = 0u32;
            for slot in 0..slot_count {
                if (case.wrapping_add(slot)).count_ones() & 1 == 0 {
                    let status_offset = oracle_slot_byte_offset(slot)
                        + usize::try_from(protocol::STATUS_WORD).unwrap() * 4;
                    bytes[status_offset..status_offset + 4]
                        .copy_from_slice(&protocol::slot::DONE.to_le_bytes());
                    done += 1;
                }
            }
            done
        };
        assert_eq!(
            RingConsumer::try_done_count(&ring)
                .unwrap_or_else(|error| panic!("Fix: done_count case {case} must decode: {error}")),
            expected_done,
            "Fix: done_count case {case} must match the independent status-word oracle."
        );
        assertions += 1;
        assert_eq!(
            oracle_done_count(ring.as_bytes(), slot_count),
            expected_done,
            "Fix: independent done_count oracle must agree for case {case}."
        );
        assertions += 1;
    }
    assert_eq!(assertions, RING_CASES as usize * 2);
}

#[test]
fn host_ring_rejects_oracle_documented_misaligned_and_oob_cases() {
    let mut ring = HostRing::new(4).expect("Fix: ring must construct");
    let encoded = protocol::encode_load_miss(7, false);
    let ring_bytes = ring.as_bytes().len();
    let oob_slot = 4u32;
    let oob_word = (oob_slot as usize) * 16;
    assert_eq!(
        RingProducer::publish(&mut ring, oob_slot, &encoded)
            .unwrap_err()
            .to_string(),
        format!(
            "ring slot is missing word {oob_word} in {ring_bytes} bytes. Fix: slot_idx must be < slot_count"
        )
    );
    let huge_slot = u32::MAX;
    let huge_word = (huge_slot as usize) * 16;
    assert_eq!(
        RingProducer::publish(&mut ring, huge_slot, &encoded)
            .unwrap_err()
            .to_string(),
        format!(
            "ring slot is missing word {huge_word} in {ring_bytes} bytes. Fix: slot_idx must be < slot_count"
        )
    );
    let short = [0u8; SLOT_BYTES - 1];
    assert_eq!(
        RingProducer::publish(&mut ring, 0, &short)
            .unwrap_err()
            .to_string(),
        format!(
            "ring slot has {} bytes, not a whole number of u32 words. Fix: encoded slot must be exactly SLOT_BYTES (64) long",
            short.len()
        )
    );
    let mut short_out = [0u8; SLOT_BYTES - 1];
    assert_eq!(
        RingConsumer::read_slot(&ring, 0, &mut short_out)
            .unwrap_err()
            .to_string(),
        format!(
            "ring slot has {} bytes, not a whole number of u32 words. Fix: out slice must be exactly SLOT_BYTES (64) long",
            short_out.len()
        )
    );
}

fn oracle_slot_byte_offset(slot_idx: u32) -> usize {
    slot_idx as usize * SLOT_BYTES
}

fn oracle_done_count(bytes: &[u8], slot_count: u32) -> u32 {
    let status_word_offset = usize::try_from(protocol::STATUS_WORD).unwrap() * 4;
    let mut done = 0u32;
    for slot in 0..slot_count {
        let base = oracle_slot_byte_offset(slot) + status_word_offset;
        let word = u32::from_le_bytes([
            bytes[base],
            bytes[base + 1],
            bytes[base + 2],
            bytes[base + 3],
        ]);
        if word == protocol::slot::DONE {
            done += 1;
        }
    }
    done
}

fn hostile_encoded_slot(seed: u32) -> ([u8; SLOT_BYTES], u32, bool) {
    let resource_id = seed.wrapping_mul(0x9E37_79B9);
    let prefetch = seed & 1 == 1;
    let encoded = protocol::encode_load_miss(resource_id, prefetch);
    let mut slot = [0u8; SLOT_BYTES];
    slot.copy_from_slice(&encoded);
    (slot, resource_id, prefetch)
}
