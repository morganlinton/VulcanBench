use super::*;

#[test]
fn collision_safe_macro_name_hash_matches_fnv1a32() {
    let names: &[&[u8]] = &[b"FOO", b"BAR", b"__builtin_va_start", b"\x00\x01\x02"];
    for name in names {
        let expected = fnv1a32(name);
        // Recompute with an independent routine.
        let mut hash = 0x811c_9dc5u32;
        for byte in *name {
            hash ^= u32::from(*byte);
            hash = hash.wrapping_mul(0x0100_0193);
        }
        assert_eq!(hash, expected, "FNV-1a32 mismatch for {:?}", name);
    }
}

#[test]
fn collision_safe_macro_name_probes_past_same_hash_different_name() {
    let stream = TokenStream {
        source: b"FOO",
        types: vec![TOK_IDENTIFIER],
        starts: vec![0],
        lens: vec![3],
    };
    let foo_hash = fnv1a32(b"FOO");
    let first_slot = macro_slot(foo_hash);
    let second_slot = (first_slot + 1) & (TABLE_SLOTS - 1);
    let mut fixture = NamedFixture::empty();
    fixture.insert_at_slot_with_hash(
        first_slot,
        foo_hash,
        b"BAR",
        512,
        C_MACRO_KIND_OBJECT_LIKE,
        &[(TOK_PLUS, C_MACRO_REPLACEMENT_LITERAL)],
    );
    fixture.insert_at_slot_with_hash(
        second_slot,
        foo_hash,
        b"FOO",
        520,
        C_MACRO_KIND_OBJECT_LIKE,
        &[(TOK_INTEGER, C_MACRO_REPLACEMENT_LITERAL)],
    );

    let outputs = run_named(&stream, &fixture, 4).expect("collision probe must succeed");
    let out = decode_u32_words(&outputs[0].to_bytes());
    let count = decode_u32_words(&outputs[1].to_bytes());
    assert_eq!(count, vec![1]);
    assert_eq!(out[0], TOK_INTEGER);
}

#[test]
fn collision_safe_macro_name_byte_exact_mismatch_does_not_expand() {
    let stream = TokenStream {
        source: b"FOO",
        types: vec![TOK_IDENTIFIER],
        starts: vec![0],
        lens: vec![3],
    };
    let foo_hash = fnv1a32(b"FOO");
    let slot = macro_slot(foo_hash);
    let mut fixture = NamedFixture::empty();
    fixture.insert_at_slot_with_hash(
        slot,
        foo_hash,
        b"FO", // shorter name, same hash slot but different bytes
        512,
        C_MACRO_KIND_OBJECT_LIKE,
        &[(TOK_PLUS, C_MACRO_REPLACEMENT_LITERAL)],
    );

    let outputs = run_named(&stream, &fixture, 4).expect("byte mismatch must passthrough");
    let out = decode_u32_words(&outputs[0].to_bytes());
    let count = decode_u32_words(&outputs[1].to_bytes());
    assert_eq!(count, vec![1]);
    // Must passthrough as unexpanded identifier.
    assert_eq!(out[0], TOK_IDENTIFIER);
}

