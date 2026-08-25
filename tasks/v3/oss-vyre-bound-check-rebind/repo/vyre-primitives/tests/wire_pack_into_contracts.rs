//! Contracts for the `_into` and `_min_words_into` variants of
//! `wire::pack_u32_slice`. These primitives replaced the inlined
//! `pack_u32_le_bytes_into` / `pack_u32_le_bytes_min_words_into`
//! duplicates that were sitting in five `vyre-frontend-c` files; the
//! tests below lock the exact contracts those call sites depended
//! on so a future re-divergence is caught immediately.

use vyre_primitives::wire::{
    append_packed_byte_lane, pack_bytes_as_u32_slice, pack_bytes_as_u32_slice_min_words,
    pack_f32_slice, pack_f32_slice_into_uninit, pack_i32_slice, pack_i32_slice_into,
    pack_u16_slice, pack_u16_slice_into, pack_u32_slice, pack_u32_slice_into,
    pack_u32_slice_into_uninit, pack_u32_slice_min_words_into, pack_u64_slice, pack_u64_slice_into,
    unpack_f32_slice, unpack_f32_slice_into, unpack_u32_slice_into,
};

#[test]
fn pack_u32_slice_into_matches_pack_u32_slice() {
    let words: Vec<u32> = (0u32..32).map(|i| i.wrapping_mul(0x0101_0101)).collect();
    let owned = pack_u32_slice(&words);
    let mut into_buf: Vec<u8> = Vec::new();
    pack_u32_slice_into(&words, &mut into_buf);
    assert_eq!(into_buf, owned, "owned and _into byte streams must match");
    assert_eq!(into_buf.len(), words.len() * 4);
}

#[test]
fn pack_u32_slice_into_clears_existing_contents() {
    let mut buf: Vec<u8> = vec![0xff; 256];
    let words = [0x1234_5678u32, 0x9abc_def0];
    pack_u32_slice_into(&words, &mut buf);
    assert_eq!(buf.len(), 8);
    assert_eq!(buf, vec![0x78, 0x56, 0x34, 0x12, 0xf0, 0xde, 0xbc, 0x9a]);
}

#[test]
fn pack_u32_slice_into_emits_little_endian_words() {
    let mut buf: Vec<u8> = Vec::new();
    pack_u32_slice_into(&[0xdead_beefu32], &mut buf);
    assert_eq!(buf, vec![0xef, 0xbe, 0xad, 0xde]);
}

#[test]
fn pack_u32_slice_into_empty_input_yields_empty_buf() {
    let mut buf: Vec<u8> = vec![1, 2, 3];
    pack_u32_slice_into(&[], &mut buf);
    assert!(buf.is_empty());
}

#[test]
fn generated_non_u32_pack_into_variants_match_owned_and_clear_for_4096_cases() {
    let mut i32_buf = vec![0xaa; 64];
    let mut u64_buf = vec![0xbb; 64];
    let mut u16_buf = vec![0xcc; 64];
    let mut state = 0x9e37_79b9_7f4a_7c15u64;

    for case in 0..4096usize {
        state = next_state(state);
        let len = (state as usize) & 31;

        let i32_values = (0..len)
            .map(|index| next_state(state ^ index as u64) as i32)
            .collect::<Vec<_>>();
        let u64_values = (0..len)
            .map(|index| next_state(state.wrapping_add(index as u64)))
            .collect::<Vec<_>>();
        let u16_values = (0..len)
            .map(|index| next_state(state.rotate_left((index & 63) as u32)) as u16)
            .collect::<Vec<_>>();

        pack_i32_slice_into(&i32_values, &mut i32_buf);
        pack_u64_slice_into(&u64_values, &mut u64_buf);
        pack_u16_slice_into(&u16_values, &mut u16_buf);

        assert_eq!(i32_buf, pack_i32_slice(&i32_values), "case {case} i32");
        assert_eq!(u64_buf, pack_u64_slice(&u64_values), "case {case} u64");
        assert_eq!(u16_buf, pack_u16_slice(&u16_values), "case {case} u16");
        assert_eq!(i32_buf.len(), i32_values.len() * 4, "case {case} i32 len");
        assert_eq!(u64_buf.len(), u64_values.len() * 8, "case {case} u64 len");
        assert_eq!(u16_buf.len(), u16_values.len() * 2, "case {case} u16 len");

        state ^= (len as u64).wrapping_mul(0xd6e8_feb8_6659_fd93);
    }
}

#[test]
fn pack_u32_slice_min_words_into_pads_with_zeros() {
    let mut buf: Vec<u8> = Vec::new();
    pack_u32_slice_min_words_into(&[0x0102_0304u32], 4, &mut buf)
        .expect("min_words >= len must succeed");
    assert_eq!(buf.len(), 16);
    assert_eq!(buf[..4], [0x04, 0x03, 0x02, 0x01]);
    assert!(
        buf[4..].iter().all(|b| *b == 0),
        "trailing bytes must be zero-padded"
    );
}

#[test]
fn pack_u32_slice_min_words_into_rejects_smaller_min_than_input() {
    let mut buf: Vec<u8> = Vec::new();
    let err =
        pack_u32_slice_min_words_into(&[1u32, 2, 3], 2, &mut buf).expect_err("min < len must fail");
    assert!(
        err.contains("input has 12 bytes but minimum buffer only has 8"),
        "error must surface both byte counts: got {err}",
    );
}

#[test]
fn pack_u32_slice_min_words_into_clears_existing_contents() {
    let mut buf: Vec<u8> = vec![0xaa; 32];
    pack_u32_slice_min_words_into(&[0x11_22_33_44u32], 2, &mut buf).expect("ok");
    assert_eq!(buf.len(), 8);
    assert_eq!(buf, vec![0x44, 0x33, 0x22, 0x11, 0x00, 0x00, 0x00, 0x00]);
}

#[test]
fn pack_u32_slice_min_words_into_empty_input_with_min_pads_full() {
    let mut buf: Vec<u8> = Vec::new();
    pack_u32_slice_min_words_into(&[], 3, &mut buf).expect("ok");
    assert_eq!(buf, vec![0u8; 12]);
}

#[test]
fn pack_bytes_as_u32_slice_lifts_each_byte_into_low_lane() {
    let bytes = b"abc";
    let packed = pack_bytes_as_u32_slice(bytes);
    // Each byte → u32 in low 8 bits → 4 LE bytes per source byte.
    assert_eq!(packed, vec![b'a', 0, 0, 0, b'b', 0, 0, 0, b'c', 0, 0, 0]);
}

#[test]
fn unpack_u32_slice_into_round_trips_pack_u32_slice() {
    let words: Vec<u32> = (0u32..64).map(|i| i.wrapping_mul(0x0102_0304)).collect();
    let bytes = pack_u32_slice(&words);
    let mut decoded: Vec<u32> = Vec::new();
    unpack_u32_slice_into(&bytes, words.len(), "round-trip", &mut decoded)
        .expect("round-trip must succeed when buffer is exact length");
    assert_eq!(decoded, words);
}

#[test]
fn unpack_u32_slice_into_clears_existing_contents() {
    let bytes = [0xef, 0xbe, 0xad, 0xde, 0x78, 0x56, 0x34, 0x12];
    let mut out: Vec<u32> = vec![999, 999, 999, 999];
    unpack_u32_slice_into(&bytes, 2, "clear", &mut out).expect("ok");
    assert_eq!(out, vec![0xdead_beefu32, 0x1234_5678]);
}

#[test]
fn unpack_u32_slice_into_rejects_truncated_buffer() {
    let bytes = [0x01u8, 0x02, 0x03]; // need 4 bytes for 1 word
    let mut out = Vec::new();
    let err = unpack_u32_slice_into(&bytes, 1, "truncated", &mut out)
        .expect_err("truncated buffer must fail");
    assert!(err.contains("u32 stream has 3 bytes, needs 4"), "{err}");
    assert!(err.contains("truncated"), "label must be in error: {err}");
}

#[test]
fn unpack_u32_slice_into_ignores_trailing_bytes_past_count() {
    // 12 bytes provided; only first 8 (2 words) requested.
    let bytes = [
        0xef, 0xbe, 0xad, 0xde, 0x78, 0x56, 0x34, 0x12, 0xff, 0xff, 0xff, 0xff,
    ];
    let mut out = Vec::new();
    unpack_u32_slice_into(&bytes, 2, "tail", &mut out).expect("ok");
    assert_eq!(out, vec![0xdead_beefu32, 0x1234_5678]);
}

#[test]
fn unpack_u32_slice_into_zero_count_yields_empty_without_error() {
    let mut out: Vec<u32> = vec![1, 2, 3];
    unpack_u32_slice_into(&[], 0, "zero", &mut out).expect("ok");
    assert!(out.is_empty());
}

#[test]
fn pack_bytes_as_u32_slice_min_words_pads_empty_input_to_floor() {
    let (bytes, words) = pack_bytes_as_u32_slice_min_words(&[], 4).expect("ok");
    assert_eq!(words, 4);
    assert_eq!(bytes, vec![0u8; 16]);
}

#[test]
fn pack_bytes_as_u32_slice_min_words_grows_when_input_exceeds_floor() {
    let (bytes, words) = pack_bytes_as_u32_slice_min_words(&[0xaa, 0xbb, 0xcc], 1).expect("ok");
    assert_eq!(words, 3);
    assert_eq!(bytes, vec![0xaa, 0, 0, 0, 0xbb, 0, 0, 0, 0xcc, 0, 0, 0]);
}

#[test]
fn unpack_f32_slice_into_round_trips_pack_f32_slice() {
    let values = [1.0f32, -2.5, 1e-9, f32::INFINITY, f32::MIN_POSITIVE];
    let bytes = pack_f32_slice(&values);
    let decoded = unpack_f32_slice(&bytes, values.len(), "f32-roundtrip").expect("ok");
    assert_eq!(decoded.len(), values.len());
    for (a, b) in decoded.iter().zip(values.iter()) {
        assert_eq!(a.to_bits(), b.to_bits(), "bit-exact round trip");
    }
}

#[test]
fn unpack_f32_slice_into_rejects_truncated_buffer() {
    let mut out = Vec::new();
    let err = unpack_f32_slice_into(&[0u8; 7], 2, "f32-trunc", &mut out)
        .expect_err("truncated buffer must fail");
    assert!(err.contains("f32 stream has 7 bytes, needs 8"), "{err}");
    assert!(err.contains("f32-trunc"), "label propagates: {err}");
}

#[test]
fn unpack_f32_slice_into_clears_existing_contents() {
    let bytes = pack_f32_slice(&[1.0_f32, 2.0]);
    let mut out: Vec<f32> = vec![999.0; 4];
    unpack_f32_slice_into(&bytes, 2, "clear", &mut out).expect("ok");
    assert_eq!(out, vec![1.0_f32, 2.0]);
}

#[test]
fn pack_u32_slice_into_uninit_matches_pack_u32_slice() {
    let words: Vec<u32> = (0..1024u32).map(|i| i.wrapping_mul(0x0101_0101)).collect();
    let owned = pack_u32_slice(&words);
    let uninit = pack_u32_slice_into_uninit(&words);
    assert_eq!(uninit, owned);
    assert_eq!(uninit.len(), words.len() * 4);
    assert_eq!(uninit.capacity(), words.len() * 4);
}

#[test]
fn pack_u32_slice_into_uninit_empty_returns_empty() {
    let out = pack_u32_slice_into_uninit(&[]);
    assert!(out.is_empty());
}

#[test]
fn pack_f32_slice_into_uninit_bit_exact() {
    let values = [
        0.0f32,
        -0.0,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NAN,
        f32::MIN_POSITIVE,
        f32::MAX,
        f32::MIN,
    ];
    let owned = pack_f32_slice(&values);
    let uninit = pack_f32_slice_into_uninit(&values);
    assert_eq!(uninit, owned, "uninit and owned bytes must agree bit-exact");
}

#[test]
fn append_packed_byte_lane_extends_existing_buffer() {
    let mut out = vec![0xaa, 0xbb, 0xcc, 0xdd];
    let bytes = [0x11u8, 0x22, 0x33];
    append_packed_byte_lane(&bytes, &mut out);
    assert_eq!(
        out,
        vec![
            0xaa, 0xbb, 0xcc, 0xdd, 0x11, 0x00, 0x00, 0x00, 0x22, 0x00, 0x00, 0x00, 0x33, 0x00,
            0x00, 0x00,
        ]
    );
}

#[test]
fn append_packed_byte_lane_matches_pack_bytes_as_u32_slice() {
    let bytes = b"hello, vyre wire";
    let owned = pack_bytes_as_u32_slice(bytes);
    let mut appended = Vec::new();
    append_packed_byte_lane(bytes, &mut appended);
    assert_eq!(appended, owned);
}

#[test]
fn append_packed_byte_lane_empty_is_noop() {
    let mut out = vec![1u8, 2, 3];
    append_packed_byte_lane(&[], &mut out);
    assert_eq!(out, vec![1, 2, 3]);
}

fn next_state(mut state: u64) -> u64 {
    state ^= state << 13;
    state ^= state >> 7;
    state ^= state << 17;
    state
}
