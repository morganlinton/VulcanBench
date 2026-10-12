use super::*;

#[test]
fn dynamic_macro_table_probe_skips_empty_slots_then_matches() {
    // Force a collision chain by occupying slots base and base+1 with
    // non-matching keys so the probe walks to base+2.
    let mut fixture = DynamicFixture::empty();
    let tok = TOK_IDENTIFIER;
    let base_slot = hash_token(tok);
    fixture.keys[base_slot] = TOK_PLUS; // occupied but different
    fixture.keys[(base_slot + 1) & (TABLE_SLOTS - 1)] = TOK_MINUS; // occupied but different
    let target_slot = (base_slot + 2) & (TABLE_SLOTS - 1);
    fixture.keys[target_slot] = tok;
    fixture.vals[target_slot] = 512;
    fixture.sizes[512] = 1;
    fixture.vals[512] = TOK_INTEGER;

    let outputs = run_dynamic(&[tok], &fixture, 8).expect("two-probe chain must succeed");
    let out = decode_u32_words(&outputs[0].to_bytes());
    assert_eq!(out[0], TOK_INTEGER);
}

#[test]
fn dynamic_macro_table_probe_terminates_at_empty_slot_for_missing_key() {
    // No entry for TOK_IDENTIFIER; the probe should see EMPTY_SLOT and stop.
    let fixture = DynamicFixture::empty();
    let outputs = run_dynamic(&[TOK_IDENTIFIER], &fixture, 8).expect("passthrough must succeed");
    let out = decode_u32_words(&outputs[0].to_bytes());
    let count = decode_u32_words(&outputs[1].to_bytes());
    assert_eq!(out[0], TOK_IDENTIFIER);
    assert_eq!(count, vec![1]);
}

#[test]
fn dynamic_macro_table_full_without_empty_slot_fails_loudly() {
    let mut fixture = DynamicFixture::empty();
    // Fill every slot with a non-matching key so the probe never sees EMPTY_SLOT.
    for slot in 0..TABLE_SLOTS {
        fixture.keys[slot] = (1000 + slot) as u32; // None of these will match TOK_IDENTIFIER
        fixture.vals[slot] = 0;
    }
    let result = catch_unwind(AssertUnwindSafe(|| {
        run_dynamic(&[TOK_IDENTIFIER], &fixture, 8)
    }));
    let eval = result.expect("full-table probe must return an error, not panic");
    let err = eval.expect_err("expected reference evaluation failure");
        assert!(
            err.to_string().contains("reference dispatch trapped"),
            "unexpected error: {err}"
        );
}

