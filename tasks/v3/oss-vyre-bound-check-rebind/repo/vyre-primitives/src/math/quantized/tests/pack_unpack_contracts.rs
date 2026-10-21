use super::*;

#[test]
fn packed_word_count_rounds_up_to_eight_lanes() {
    let cases = [
        (0, 0),
        (1, 1),
        (7, 1),
        (8, 1),
        (9, 2),
        (15, 2),
        (16, 2),
        (17, 3),
    ];
    for (lanes, words) in cases {
        assert_eq!(i4_packed_words(lanes), words, "lanes={lanes}");
    }
}

#[test]
fn pack_unpack_preserves_signed_i4_domain() {
    let values = [-8, -7, -1, 0, 1, 2, 6, 7];
    let packed = pack_i4x8_cpu(&values);
    assert_eq!(packed, vec![0x7621_0F98]);
    assert_eq!(unpack_i4x8_cpu(&packed, values.len() as u32), values);
}

#[test]
fn pack_saturates_out_of_domain_values() {
    let values = [-32, -9, -8, 7, 8, 31];
    let packed = pack_i4x8_cpu(&values);
    assert_eq!(
        unpack_i4x8_cpu(&packed, values.len() as u32),
        [-8, -8, -8, 7, 7, 7]
    );
}

#[test]
fn generated_pack_unpack_round_trip_all_offsets() {
    let pattern = [-8, -3, -1, 0, 1, 3, 7, 6, 5, 4, 2, -2, -4, -6, -7, -5];
    for len in 0..=256 {
        let values = pattern
            .iter()
            .copied()
            .cycle()
            .take(len)
            .collect::<Vec<_>>();
        let packed = pack_i4x8_cpu(&values);
        let unpacked = unpack_i4x8_cpu(&packed, len as u32);
        assert_eq!(unpacked, values, "len={len}");
        assert_eq!(packed.len(), i4_packed_words(len as u32) as usize);
    }
}

#[test]
fn pack_unpack_into_reuses_capacity_and_truncates_stale_tail() {
    let mut packed = Vec::with_capacity(4);
    packed.extend_from_slice(&[0xFFFF_FFFF, 0xAAAA_AAAA, 0x5555_5555, 0]);
    let packed_capacity = packed.capacity();

    try_pack_i4x8_cpu_into(&[-8, -1, 0, 7, 8, -9, 3, -2, 1], &mut packed)
        .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - pack_i4x8 CPU oracle should reuse caller-owned packed storage");

    assert_eq!(packed.len(), 2);
    assert_eq!(packed.capacity(), packed_capacity);

    try_pack_i4x8_cpu_into(&[7], &mut packed)
        .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - pack_i4x8 CPU oracle should truncate stale packed words");

    assert_eq!(packed, vec![7]);
    assert_eq!(packed.capacity(), packed_capacity);

    let mut lanes = Vec::with_capacity(16);
    lanes.extend_from_slice(&[99; 16]);
    let lanes_capacity = lanes.capacity();

    try_unpack_i4x8_cpu_into(&packed, 1, &mut lanes)
        .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - unpack_i4x8 CPU oracle should reuse caller-owned lane storage");

    assert_eq!(lanes, vec![7]);
    assert_eq!(lanes.capacity(), lanes_capacity);
}

#[test]
fn unpack_missing_words_zero_fills_missing_lanes() {
    assert_eq!(unpack_i4x8_cpu(&[], 4), vec![0, 0, 0, 0]);
    assert_eq!(unpack_i4x8_cpu(&[0xF], 4), vec![-1, 0, 0, 0]);
}

#[test]
fn unpack_program_layout_matches_packed_shape() {
    let program = unpack_i4x8("packed", "lanes", 17);
    assert_eq!(program.workgroup_size, [256, 1, 1]);
    assert_eq!(program.buffers[0].name(), "packed");
    assert_eq!(program.buffers[0].count(), 3);
    assert_eq!(program.buffers[1].name(), "lanes");
    assert_eq!(program.buffers[1].count(), 17);
}

#[test]
fn unpack_zero_lanes_traps() {
    assert!(unpack_i4x8("packed", "lanes", 0).stats().trap());
}

