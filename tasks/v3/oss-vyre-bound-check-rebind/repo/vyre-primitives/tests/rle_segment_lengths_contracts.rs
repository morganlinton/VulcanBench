//! RLE segment-length CPU parity and reusable-storage contracts.
#![cfg(all(feature = "cpu-parity", feature = "decode"))]

use vyre_primitives::decode::rle_segment_lengths::{
    pack_rle_segments, rle_decode_cpu, rle_segment_lengths, try_rle_decode_cpu_into,
    try_rle_segment_lengths_cpu_into, try_rle_segment_start_offsets_cpu_into,
};

#[test]
fn rle_program_rejects_zero_segments() {
    let program = rle_segment_lengths(0);
    assert!(program.stats().trap());
}

#[test]
fn rle_reusable_cpu_paths_truncate_stale_storage() {
    let packed = pack_rle_segments(&[(2, b'A'), (0, b'X'), (3, b'B')]).unwrap();
    let mut lengths = Vec::with_capacity(16);
    let mut values = Vec::with_capacity(16);
    let mut offsets = Vec::with_capacity(16);
    let mut decoded = Vec::with_capacity(16);
    lengths.extend([99u32; 16]);
    values.extend([99u32; 16]);
    offsets.extend([99u32; 16]);
    decoded.extend([0xFFu8; 16]);
    let lengths_ptr = lengths.as_ptr();
    let values_ptr = values.as_ptr();
    let offsets_ptr = offsets.as_ptr();
    let decoded_ptr = decoded.as_ptr();

    try_rle_segment_lengths_cpu_into(&packed, &mut lengths, &mut values).unwrap();
    let total = try_rle_segment_start_offsets_cpu_into(&lengths, &mut offsets).unwrap();
    try_rle_decode_cpu_into(&packed, &mut decoded).unwrap();

    assert_eq!(lengths, vec![2, 0, 3]);
    assert_eq!(
        values,
        vec![u32::from(b'A'), u32::from(b'X'), u32::from(b'B')]
    );
    assert_eq!(offsets, vec![0, 2, 2]);
    assert_eq!(total, 5);
    assert_eq!(decoded, b"AABBB".to_vec());
    assert_eq!(lengths.as_ptr(), lengths_ptr);
    assert_eq!(values.as_ptr(), values_ptr);
    assert_eq!(offsets.as_ptr(), offsets_ptr);
    assert_eq!(decoded.as_ptr(), decoded_ptr);
}

#[test]
fn generated_rle_pack_unpack_offsets_decode_match_independent_reference() {
    for case in 0..128usize {
        let segment_count = case % 19;
        let segments: Vec<(u32, u8)> = (0..segment_count)
            .map(|idx| (((idx * 5 + case) % 11) as u32, (idx * 17 + case) as u8))
            .collect();
        let packed = pack_rle_segments(&segments).unwrap();
        let mut lengths = Vec::with_capacity(segment_count + 4);
        let mut values = Vec::with_capacity(segment_count + 4);
        let mut offsets = Vec::with_capacity(segment_count + 4);
        let mut decoded = Vec::with_capacity(
            segments
                .iter()
                .map(|(length, _)| *length as usize)
                .sum::<usize>()
                + 4,
        );

        try_rle_segment_lengths_cpu_into(&packed, &mut lengths, &mut values).unwrap();
        let total = try_rle_segment_start_offsets_cpu_into(&lengths, &mut offsets).unwrap();
        try_rle_decode_cpu_into(&packed, &mut decoded).unwrap();

        let mut expected_offsets = Vec::with_capacity(segment_count);
        let mut expected_total = 0u32;
        let mut expected_decoded = Vec::new();
        for &(length, value) in &segments {
            expected_offsets.push(expected_total);
            expected_total = expected_total.saturating_add(length);
            expected_decoded.extend(std::iter::repeat(value).take(length as usize));
        }

        assert_eq!(
            lengths,
            segments
                .iter()
                .map(|(length, _)| *length)
                .collect::<Vec<_>>(),
            "case {case}"
        );
        assert_eq!(
            values,
            segments
                .iter()
                .map(|(_, value)| u32::from(*value))
                .collect::<Vec<_>>(),
            "case {case}"
        );
        assert_eq!(offsets, expected_offsets, "case {case}");
        assert_eq!(total, expected_total, "case {case}");
        assert_eq!(decoded, expected_decoded, "case {case}");
        assert_eq!(rle_decode_cpu(&packed), expected_decoded, "case {case}");
    }
}
