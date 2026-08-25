use super::*;

#[test]
fn macro_table_has_exactly_4096_slots() {
    // The arena geometry is a power-of-two so mask-and-wrap is valid.
    assert_eq!(TABLE_SLOTS, 4096);
    assert_eq!(TABLE_MASK, 4095);
    // 4096 is a power of two; verify mask covers all bits.
    assert_eq!(TABLE_MASK.count_ones(), 12);
}

#[test]
fn empty_macro_slot_sentinel_is_u32_max() {
    assert_eq!(EMPTY_SLOT, u32::MAX);
    // Sentinel must not collide with any valid token ID (max real token is ~255).
    const _: () = assert!(EMPTY_SLOT > 255);
}

#[test]
fn dynamic_macro_table_probe_finds_exact_key_match() {
    let mut fixture = DynamicFixture::empty();
    fixture.insert(TOK_IDENTIFIER, 512, &[TOK_INTEGER]);

    let outputs = run_dynamic(&[TOK_IDENTIFIER], &fixture, 8).expect("probe hit must succeed");
    let out = decode_u32_words(&outputs[0].to_bytes());
    let count = decode_u32_words(&outputs[1].to_bytes());
    assert_eq!(&out[..1], &[TOK_INTEGER]);
    assert_eq!(count, vec![1]);
}
