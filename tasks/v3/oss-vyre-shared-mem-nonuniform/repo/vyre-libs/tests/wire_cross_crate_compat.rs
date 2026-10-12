//! Cross-crate compatibility: vyre-libs encodes with
//! `vyre_primitives::wire`, vyre-libs (mirroring a vyre-frontend-c
//! consumer) decodes the same bytes. Asserts the wire format is
//! crate-boundary stable - independent re-implementations would
//! show up here as divergent output.

use vyre_libs::scan::dispatch_io::{pack_haystack_u32, pack_u32_slice};
use vyre_primitives::wire::{
    decode_f32_le_bytes_all, decode_u32_le_bytes_all, decode_u64_le_bytes_all, pack_f32_slice,
    pack_u32_slice as wire_pack_u32, pack_u64_slice, unpack_u32_slice_into,
};

#[test]
fn vyre_libs_scan_pack_matches_vyre_primitives_wire() {
    let words: Vec<u32> = (0u32..256).collect();
    let scan_path = pack_u32_slice(&words);
    let wire_path = wire_pack_u32(&words);
    assert_eq!(
        scan_path, wire_path,
        "vyre_libs::scan::pack_u32_slice diverged from vyre_primitives::wire"
    );
}

#[test]
fn round_trip_u32_across_crates() {
    let words: Vec<u32> = (0u32..1024).map(|i| i.wrapping_mul(0x0101_0101)).collect();
    let bytes = pack_u32_slice(&words);
    let decoded = decode_u32_le_bytes_all(&bytes);
    assert_eq!(decoded, words);
    let mut into = Vec::new();
    unpack_u32_slice_into(&bytes, words.len(), "cross-crate", &mut into).unwrap();
    assert_eq!(into, words);
}

#[test]
fn round_trip_f32_across_crates() {
    let values: Vec<f32> = (0..256).map(|i| (i as f32) * 0.125 - 16.0).collect();
    let bytes = pack_f32_slice(&values);
    let decoded = decode_f32_le_bytes_all(&bytes);
    assert_eq!(decoded.len(), values.len());
    for (a, b) in decoded.iter().zip(values.iter()) {
        assert_eq!(a.to_bits(), b.to_bits(), "f32 cross-crate divergence");
    }
}

#[test]
fn round_trip_u64_across_crates() {
    let values: Vec<u64> = (0u64..512)
        .map(|i| i.wrapping_mul(0x0101_0101_0101_0101))
        .collect();
    let bytes = pack_u64_slice(&values);
    let decoded = decode_u64_le_bytes_all(&bytes);
    assert_eq!(decoded, values);
}

#[test]
fn haystack_pack_is_4byte_aligned_and_preserves_input_prefix() {
    let haystack = b"hello, vyre!";
    let packed = pack_haystack_u32(haystack);
    assert_eq!(packed.len() % 4, 0, "haystack pack must be 4-byte aligned");
    assert!(packed.len() >= haystack.len());
    assert_eq!(&packed[..haystack.len()], &haystack[..]);
    for trailing in &packed[haystack.len()..] {
        assert_eq!(*trailing, 0, "tail padding must be zero-filled");
    }
}
