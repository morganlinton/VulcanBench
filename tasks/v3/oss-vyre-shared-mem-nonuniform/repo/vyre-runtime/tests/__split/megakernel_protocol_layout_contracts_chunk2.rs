#[test]
fn slot_word_layout_args_start_at_word_4() {
    let mut ring = Megakernel::encode_empty_ring(1).unwrap();
    Megakernel::publish_slot(&mut ring, 0, 0, opcode::NOP, &[0xDEAD_BEEF]).unwrap();
    let arg0 = read_word(&ring, 4);
    assert_eq!(arg0, 0xDEAD_BEEF, "arg0 must be at word 4");
}

// ---------------------------------------------------------------------------
// 5. Epoch / done counters
// ---------------------------------------------------------------------------

#[test]
fn done_count_is_at_word_1() {
    assert_eq!(
        control::DONE_COUNT,
        1,
        "done count must be at control word 1"
    );
}

#[test]
fn epoch_is_after_metrics() {
    let metrics_end = control::METRICS_BASE + control::METRICS_SLOTS;
    assert_eq!(
        control::EPOCH,
        metrics_end,
        "epoch must immediately follow metrics region"
    );
}

#[test]
fn done_count_and_epoch_are_distinct() {
    assert_ne!(
        control::DONE_COUNT,
        control::EPOCH,
        "done count and epoch must be at different words"
    );
}

#[test]
fn read_done_count_from_exact_buffer_succeeds() {
    let mut ctrl = vec![0u8; (control::DONE_COUNT as usize + 1) * 4];
    write_word(&mut ctrl, control::DONE_COUNT as usize, 12345);
    assert_eq!(Megakernel::read_done_count(&ctrl), 12345);
}

#[test]
fn read_epoch_from_exact_buffer_succeeds() {
    let mut ctrl = vec![0u8; (control::EPOCH as usize + 1) * 4];
    write_word(&mut ctrl, control::EPOCH as usize, 0x1234_5678);
    assert_eq!(Megakernel::read_epoch(&ctrl), 0x1234_5678);
}

#[test]
fn strict_done_count_rejects_buffer_ending_at_done_count_word() {
    let short = vec![0u8; (control::DONE_COUNT as usize) * 4];
    let err = protocol::try_read_done_count(&short)
        .expect_err("buffer ending exactly at DONE_COUNT word must reject");
    assert!(err.to_string().contains("Fix:"));
}

#[test]
fn strict_epoch_rejects_buffer_ending_at_epoch_word() {
    let short = vec![0u8; (control::EPOCH as usize) * 4];
    let err = protocol::try_read_epoch(&short)
        .expect_err("buffer ending exactly at EPOCH word must reject");
    assert!(err.to_string().contains("Fix:"));
}

#[test]
fn epoch_word_does_not_overlap_priority_offsets() {
    assert!(
        control::EPOCH < control::PRIORITY_OFFSETS_BASE,
        "epoch word must come before priority offsets"
    );
}

#[test]
fn done_count_word_does_not_overlap_tenant_regions() {
    assert!(
        control::DONE_COUNT < control::TENANT_BASE,
        "done count must come before tenant base"
    );
}

#[test]
fn encode_control_includes_done_count_and_epoch_words() {
    let ctrl = Megakernel::encode_control(false, 1, 0).unwrap();
    assert!(
        ctrl.len() >= (control::DONE_COUNT as usize + 1) * 4,
        "encoded control must include done count word"
    );
    assert!(
        ctrl.len() >= (control::EPOCH as usize + 1) * 4,
        "encoded control must include epoch word"
    );
}

// ---------------------------------------------------------------------------
// 6. Packed slot overflow behavior
// ---------------------------------------------------------------------------

#[test]
fn packed_slot_exact_12_words_succeeds() {
    let mut ring = Megakernel::encode_empty_ring(1).unwrap();
    // 1 metadata word (opcode_count=1, arg_words=11, 2-byte pair) + 11 arg words = 12 words
    let args = vec![0u32; 11];
    Megakernel::publish_packed_slot(&mut ring, 0, 0, &[(1u8, args)])
        .expect("packed slot with exactly 12 words must succeed");
    let status = read_word(&ring, 0);
    assert_eq!(status, slot::PUBLISHED);
}

#[test]
fn packed_slot_13_words_fails() {
    let mut ring = Megakernel::encode_empty_ring(1).unwrap();
    let args = vec![0u32; 12];
    let err = Megakernel::publish_packed_slot(&mut ring, 0, 0, &[(1u8, args)])
        .expect_err("packed slot with 13 words must fail");
    assert!(matches!(err, PipelineError::QueueFull { .. }));
    let msg = err.to_string();
    assert!(
        msg.contains("12-word") || msg.contains("budget") || msg.contains("exceeds"),
        "error must mention 12-word budget: {msg}"
    );
}

#[test]
fn packed_slot_max_ops_without_args_fits_budget() {
    let mut ring = Megakernel::encode_empty_ring(1).unwrap();
    // Metadata: 2 header bytes + 2 bytes per op. Max zero-arg ops that fit in 12 words (48 bytes):
    // floor((48 - 2) / 2) = 23 ops.
    let ops: Vec<_> = (0..23).map(|i| (i as u8, vec![])).collect();
    Megakernel::publish_packed_slot(&mut ring, 0, 0, &ops)
        .expect("23 zero-arg ops must fit in 12-word budget");
}

#[test]
fn packed_slot_256_ops_fails() {
    let mut ring = Megakernel::encode_empty_ring(1).unwrap();
    let ops: Vec<_> = (0..256).map(|_| (0u8, vec![])).collect();
    let err = Megakernel::publish_packed_slot(&mut ring, 0, 0, &ops)
        .expect_err("256 inner ops must fail u8 opcode_count overflow");
    assert!(matches!(err, PipelineError::QueueFull { .. }));
    assert!(
        err.to_string().contains("255"),
        "error must mention u8 limit: {err}"
    );
}

#[test]
fn packed_slot_arg_offset_overflow_fails() {
    let mut ring = Megakernel::encode_empty_ring(1).unwrap();
    // Each op adds 2 metadata bytes; 256 ops = 512 metadata bytes = 128 metadata words,
    // but also each op gets an arg_offset. If packed_args exceeds 255 words,
    // the arg_offset u8 overflows.
    let ops: Vec<_> = (0..255)
        .map(|_| (0u8, vec![0u32])) // 255 arg words total
        .collect();
    let err = Megakernel::publish_packed_slot(&mut ring, 0, 0, &ops)
        .expect_err("packed slot with >255 arg words must fail arg_offset u8 overflow");
    assert!(matches!(err, PipelineError::QueueFull { .. }));
}

#[test]
fn packed_slot_empty_payload_succeeds() {
    let mut ring = Megakernel::encode_empty_ring(1).unwrap();
    let ops: &[(u8, &[u32])] = &[];
    Megakernel::publish_packed_slot(&mut ring, 0, 0, ops)
        .expect("empty packed slot must publish with zero opcodes");
    let slot_words: Vec<u32> = ring[..(SLOT_WORDS as usize * 4)]
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
        .collect();
    assert_eq!(slot_words[1], opcode::PACKED_SLOT);
    // Arg0 word contains opcode_count=0 in its first byte
    let arg0_bytes = slot_words[ARG0_WORD as usize].to_le_bytes();
    assert_eq!(arg0_bytes[0], 0, "opcode_count must be zero");
}

#[test]
fn packed_slot_metadata_overflow_fails_with_many_small_ops() {
    let mut ring = Megakernel::encode_empty_ring(1).unwrap();
    // 2 bytes header + 2 bytes per op. 7 ops = 16 metadata bytes = 4 metadata words.
    // 8 ops = 18 metadata bytes = 5 metadata words (ceil).
    // With 8 ops each having 1 arg word: 5 + 8 = 13 words > 12.
    let ops: Vec<_> = (0..8).map(|i| (i as u8, vec![0u32; 1])).collect();
    let err = Megakernel::publish_packed_slot(&mut ring, 0, 0, &ops)
        .expect_err("packed slot with 8 ops + args exceeding 12 words must fail");
    assert!(matches!(err, PipelineError::QueueFull { .. }));
}

#[test]
fn packed_slot_mixed_metadata_and_args_exactly_12_words_succeeds() {
    let mut ring = Megakernel::encode_empty_ring(1).unwrap();
    // 3 ops -> 2 + 3*2 = 8 metadata bytes = 2 metadata words.
    // 10 arg words -> total = 12 words exactly.
    let ops = vec![
        (1u8, vec![0u32; 3]),
        (2u8, vec![0u32; 4]),
        (3u8, vec![0u32; 3]),
    ];
    Megakernel::publish_packed_slot(&mut ring, 0, 0, &ops)
        .expect("3 ops + 10 arg words = 12 total words must succeed");
    let status = read_word(&ring, 0);
    assert_eq!(status, slot::PUBLISHED);
}

#[test]
fn packed_slot_rejects_non_publishable_target_slot() {
    let mut ring = Megakernel::encode_empty_ring(1).unwrap();
    Megakernel::publish_slot(&mut ring, 0, 0, opcode::NOP, &[]).unwrap();
    let err = Megakernel::publish_packed_slot(&mut ring, 0, 0, &[(1u8, vec![])])
        .expect_err("packed slot must reject already-PUBLISHED target");
    assert!(matches!(err, PipelineError::QueueFull { .. }));
}

#[test]
fn packed_slot_payload_byte_1_records_total_arg_words() {
    let mut ring = Megakernel::encode_empty_ring(1).unwrap();
    let ops = vec![
        (1u8, vec![0xA1A1_A1A1, 0xB2B2_B2B2]),
        (2u8, vec![0xC3C3_C3C3]),
    ];
    Megakernel::publish_packed_slot(&mut ring, 0, 0, &ops).unwrap();
    let arg0_word = read_word(&ring, ARG0_WORD as usize);
    let bytes = arg0_word.to_le_bytes();
    assert_eq!(bytes[0], 2, "byte 0 must be opcode_count");
    assert_eq!(bytes[1], 3, "byte 1 must be total arg words (2 + 1 = 3)");
}

#[test]
fn packed_slot_status_is_published_after_success() {
    let mut ring = Megakernel::encode_empty_ring(2).unwrap();
    Megakernel::publish_packed_slot(&mut ring, 1, 0, &[(opcode::NOP as u8, vec![])])
        .expect("packed slot publish into slot 1 must succeed");
    let slot1_base = (SLOT_WORDS * 4) as usize;
    let status = u32::from_le_bytes(ring[slot1_base..slot1_base + 4].try_into().unwrap());
    assert_eq!(status, slot::PUBLISHED);
}
