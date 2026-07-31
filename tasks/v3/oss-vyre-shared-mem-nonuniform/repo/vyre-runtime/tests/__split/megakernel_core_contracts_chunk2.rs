#[test]
fn read_metrics_returns_nonzero_only() {
    let mut ctrl = vec![0u8; (control::METRICS_BASE as usize + 32) * 4];
    // Set opcode 2 counter to 42
    let off = ((control::METRICS_BASE + 2) as usize) * 4;
    ctrl[off..off + 4].copy_from_slice(&42u32.to_le_bytes());
    // Set opcode 7 counter to 99
    let off7 = ((control::METRICS_BASE + 7) as usize) * 4;
    ctrl[off7..off7 + 4].copy_from_slice(&99u32.to_le_bytes());

    let metrics = Megakernel::read_metrics(&ctrl);
    assert_eq!(metrics.len(), 2);
    assert!(metrics.contains(&(2, 42)));
    assert!(metrics.contains(&(7, 99)));
}

#[test]
fn priority_constants_are_distinct() {
    assert_ne!(slot::PRIORITY_NORMAL, slot::PRIORITY_HIGH);
}

#[test]
fn program_with_new_opcodes_passes_validation() {
    let prog = build_program_sharded(64, &[]);
    let errs = vyre_foundation::validate::validate(&prog);
    assert!(errs.is_empty(), "V6.4 program validation failed: {errs:?}");
}

#[test]
fn packed_slot_round_trips() {
    let mut ring = Megakernel::encode_empty_ring(2).unwrap();
    Megakernel::publish_packed_slot(
        &mut ring,
        0,
        7,
        &[
            (opcodes::STORE_U32 as u8, vec![10, 32]),
            (opcodes::ATOMIC_ADD as u8, vec![1, 64]),
        ],
    )
    .expect("Fix: packed slot publish must succeed");
    let slot_words = ring[..(SLOT_WORDS as usize * 4)]
        .chunks_exact(4)
        .map(|chunk| u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect::<Vec<_>>();
    let decoded = decode_packed_slot_words(&slot_words[ARG0_WORD as usize..]);
    assert_eq!(slot_words[OPCODE_WORD as usize], opcodes::PACKED_SLOT);
    assert_eq!(decoded.opcode_count, 2);
    assert_eq!(
        decoded.entries,
        vec![
            (opcodes::STORE_U32 as u8, 0),
            (opcodes::ATOMIC_ADD as u8, 2),
        ]
    );
    assert_eq!(decoded.packed_args, vec![10, 32, 1, 64]);
}

#[test]
fn exceeds_12_args_rejected() {
    let mut ring = Megakernel::encode_empty_ring(1).unwrap();
    let err = Megakernel::publish_packed_slot(
        &mut ring,
        0,
        0,
        &[
            (opcodes::MEMCPY as u8, vec![0; 12]),
            (opcodes::PRINTF as u8, vec![0; 4]),
        ],
    )
    .expect_err("packed slot must reject payloads that exceed 12 words");
    assert!(matches!(err, PipelineError::QueueFull { .. }));
}

#[test]
fn opcode_count_zero_is_nop() {
    let mut ring = Megakernel::encode_empty_ring(1).unwrap();
    let empty_ops: &[(u8, &[u32])] = &[];
    Megakernel::publish_packed_slot(&mut ring, 0, 0, empty_ops)
        .expect("Fix: empty packed slot must still publish");
    let slot_words = ring[..(SLOT_WORDS as usize * 4)]
        .chunks_exact(4)
        .map(|chunk| u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect::<Vec<_>>();
    let decoded = decode_packed_slot_words(&slot_words[ARG0_WORD as usize..]);
    assert_eq!(slot_words[OPCODE_WORD as usize], opcodes::PACKED_SLOT);
    assert_eq!(decoded.opcode_count, 0);
    assert!(decoded.entries.is_empty());
    assert!(decoded.packed_args.is_empty());
}

#[test]
fn batch_fence_after_packed_slot_increments_epoch() {
    let mut ring = Megakernel::encode_empty_ring(4).unwrap();
    Megakernel::publish_packed_slot(&mut ring, 0, 0, &[(opcodes::STORE_U32 as u8, vec![10, 32])])
        .expect("Fix: packed slot publish must succeed");
    Megakernel::publish_slot(&mut ring, 1, 0, opcodes::BATCH_FENCE, &[1, 0xBEEF])
        .expect("Fix: fence publish after packed slot must succeed");
    let fence_base = SLOT_WORDS as usize * 4;
    let fence_op = u32::from_le_bytes(ring[fence_base + 4..fence_base + 8].try_into().unwrap());
    let fence_tag = u32::from_le_bytes(ring[fence_base + 20..fence_base + 24].try_into().unwrap());
    assert_eq!(fence_op, opcodes::BATCH_FENCE);
    assert_eq!(fence_tag, 0xBEEF);
}
