//! Adversarial tests: hostile, malformed, boundary, and empty/oversized inputs.

use vyre_grammar_gen::{
    decode_dfa_from_bytes, decode_lr_from_bytes,
    lex_c11_max_munch_kinds,
    preprocess_c_host,
    validate_lr_table,
    wire::{PackedBlob, WireError, MAGIC},
    DfaBuilder, LrBuilder,
    lr::Action,
    LexCpuError,
};

// ---------------------------------------------------------------------------
// decode_dfa_from_bytes: malformed inputs
// ---------------------------------------------------------------------------

#[test]
fn decode_dfa_empty_input_is_too_short() {
    let err = decode_dfa_from_bytes(&[]).unwrap_err();
    assert!(matches!(err, WireError::TooShort { .. }), "{err:?}");
}

#[test]
fn decode_dfa_23_bytes_is_too_short() {
    let err = decode_dfa_from_bytes(&[0u8; 23]).unwrap_err();
    assert!(matches!(err, WireError::TooShort { need: 24, got: 23 }), "{err:?}");
}

#[test]
fn decode_dfa_all_zeros_is_bad_magic() {
    let err = decode_dfa_from_bytes(&[0u8; 40]).unwrap_err();
    assert!(matches!(err, WireError::BadMagic(_)), "{err:?}");
}

#[test]
fn decode_dfa_bad_magic_bytes() {
    let mut bytes = vec![0u8; 40];
    bytes[0..4].copy_from_slice(b"XXXX");
    let err = decode_dfa_from_bytes(&bytes).unwrap_err();
    assert!(matches!(err, WireError::BadMagic(m) if &m == b"XXXX"), "{err:?}");
}

#[test]
fn decode_dfa_wrong_version() {
    let mut bytes = vec![0u8; 40];
    bytes[0..4].copy_from_slice(&MAGIC);
    bytes[4..6].copy_from_slice(&99u16.to_le_bytes()); // wrong version
    let err = decode_dfa_from_bytes(&bytes).unwrap_err();
    assert!(matches!(err, WireError::UnsupportedVersion(99)), "{err:?}");
}

#[test]
fn decode_dfa_version_zero_rejected() {
    let mut bytes = vec![0u8; 40];
    bytes[0..4].copy_from_slice(&MAGIC);
    bytes[4..6].copy_from_slice(&0u16.to_le_bytes());
    let err = decode_dfa_from_bytes(&bytes).unwrap_err();
    assert!(matches!(err, WireError::UnsupportedVersion(0)), "{err:?}");
}

#[test]
fn decode_dfa_rejects_lr_blob_as_wrong_kind() {
    let mut b = LrBuilder::new(2, 2, 1);
    b.set_action(0, 0, Action::Accept);
    let lr = b.build();
    let blob = PackedBlob::from_lr(&lr).expect("valid LR table must pack");
    // Trying to decode an LR blob as DFA must fail with UnsupportedKind
    let err = decode_dfa_from_bytes(&blob.bytes).unwrap_err();
    assert!(
        matches!(err, WireError::UnsupportedKind(1)),
        "expected UnsupportedKind(1), got {err:?}"
    );
}

#[test]
fn decode_lr_rejects_dfa_blob_as_wrong_kind() {
    let dfa = DfaBuilder::new(2, 4).build().expect("empty pattern set must succeed");
    let blob = PackedBlob::from_dfa(&dfa);
    let err = decode_lr_from_bytes(&blob.bytes).unwrap_err();
    assert!(
        matches!(err, WireError::UnsupportedKind(0)),
        "expected UnsupportedKind(0), got {err:?}"
    );
}

#[test]
fn decode_dfa_truncated_payload_is_payload_truncated() {
    let dfa = DfaBuilder::new(2, 4).build().expect("empty pattern set must succeed");
    let blob = PackedBlob::from_dfa(&dfa);
    // Truncate by removing the last 10 bytes
    let truncated = &blob.bytes[..blob.bytes.len().saturating_sub(10)];
    let err = decode_dfa_from_bytes(truncated).unwrap_err();
    assert!(
        matches!(err, WireError::PayloadTruncated { .. }),
        "{err:?}"
    );
}

#[test]
fn decode_dfa_checksum_mismatch_on_bit_flip() {
    let dfa = DfaBuilder::new(2, 4).build().expect("empty pattern set must succeed");
    let mut blob = PackedBlob::from_dfa(&dfa);
    // Flip a bit in the payload area (byte 25)
    if blob.bytes.len() > 25 {
        blob.bytes[25] ^= 0x01;
    }
    let err = decode_dfa_from_bytes(&blob.bytes).unwrap_err();
    assert!(
        matches!(err, WireError::ChecksumMismatch { .. }),
        "{err:?}"
    );
}

#[test]
fn decode_dfa_checksum_mismatch_on_last_payload_byte_flip() {
    let dfa = DfaBuilder::new(4, 8).build().expect("empty pattern set must succeed");
    let mut blob = PackedBlob::from_dfa(&dfa);
    // Last payload byte is at index len - 17
    let flip_idx = blob.bytes.len() - 17;
    blob.bytes[flip_idx] ^= 0x80;
    let err = decode_dfa_from_bytes(&blob.bytes).unwrap_err();
    assert!(matches!(err, WireError::ChecksumMismatch { .. }), "{err:?}");
}

// ---------------------------------------------------------------------------
// decode_lr_from_bytes: malformed inputs
// ---------------------------------------------------------------------------

#[test]
fn decode_lr_empty_is_too_short() {
    let err = decode_lr_from_bytes(&[]).unwrap_err();
    assert!(matches!(err, WireError::TooShort { .. }), "{err:?}");
}

#[test]
fn decode_lr_one_byte_is_too_short() {
    let err = decode_lr_from_bytes(&[0u8; 1]).unwrap_err();
    assert!(matches!(err, WireError::TooShort { .. }), "{err:?}");
}

// ---------------------------------------------------------------------------
// validate_lr_table: adversarial dimensions
// ---------------------------------------------------------------------------

#[test]
fn validate_lr_extra_action_word_rejected() {
    let mut t = LrBuilder::new(2, 2, 1).build();
    t.action.push(0);
    let err = validate_lr_table(&t).unwrap_err();
    assert!(err.contains("Fix:"));
}

#[test]
fn validate_lr_extra_goto_word_rejected() {
    let mut t = LrBuilder::new(2, 2, 1).build();
    t.goto.push(u32::MAX);
    let err = validate_lr_table(&t).unwrap_err();
    assert!(err.contains("Fix:"));
}

#[test]
fn validate_lr_empty_action_rejected() {
    let mut t = LrBuilder::new(2, 2, 1).build();
    t.action.clear();
    let err = validate_lr_table(&t).unwrap_err();
    assert!(err.contains("Fix:"));
}

#[test]
fn validate_lr_zero_states_zero_tokens_ok() {
    let t = LrBuilder::new(0, 0, 0).build();
    // 0 x 0 = 0 words expected - should pass
    assert!(validate_lr_table(&t).is_ok());
}

// ---------------------------------------------------------------------------
// lex_c11_max_munch_kinds: hostile inputs
// ---------------------------------------------------------------------------

#[test]
fn lex_empty_input_returns_empty() {
    let kinds = lex_c11_max_munch_kinds(b"").expect("empty lex should succeed");
    assert!(kinds.is_empty());
}

#[test]
fn lex_null_byte_is_error() {
    let err = lex_c11_max_munch_kinds(b"\x00").unwrap_err();
    assert!(matches!(err, LexCpuError::NoTokenAt { offset: 0 }), "{err:?}");
}

#[test]
fn lex_lone_backslash_is_error() {
    let err = lex_c11_max_munch_kinds(b"\\").unwrap_err();
    assert!(
        matches!(err, LexCpuError::NoTokenAt { .. }),
        "{err:?}"
    );
}

#[test]
fn lex_high_byte_is_error() {
    // 0xFF is not a valid C token start
    let err = lex_c11_max_munch_kinds(&[0xFFu8]).unwrap_err();
    assert!(matches!(err, LexCpuError::NoTokenAt { offset: 0 }), "{err:?}");
}

#[test]
fn lex_mid_string_error_offset_is_nonzero() {
    // "valid_ident \x00 more" - error is at offset of the null byte
    let input = b"x \x00";
    let err = lex_c11_max_munch_kinds(input).unwrap_err();
    assert!(
        matches!(err, LexCpuError::NoTokenAt { offset } if offset == 2),
        "{err:?}"
    );
}

#[test]
fn lex_at_sign_is_error() {
    // '@' is not a C token
    let err = lex_c11_max_munch_kinds(b"@").unwrap_err();
    assert!(matches!(err, LexCpuError::NoTokenAt { offset: 0 }), "{err:?}");
}

// ---------------------------------------------------------------------------
// preprocess_c_host: adversarial / edge cases
// ---------------------------------------------------------------------------

#[test]
fn preprocess_empty_string() {
    let out = preprocess_c_host("");
    assert_eq!(out, "");
}

#[test]
fn preprocess_only_whitespace() {
    let out = preprocess_c_host("   \t\n");
    // Must not panic; result just has whitespace / blank
    let _ = out; // content may vary, but must not panic
}

#[test]
fn preprocess_unterminated_block_comment() {
    // Unterminated /* should not panic; rest of input is consumed
    let out = preprocess_c_host("int x /* unclosed");
    // 'int x ' is kept; the unclosed comment is dropped
    assert!(out.contains("int x "), "got: {out:?}");
}

#[test]
fn preprocess_deeply_nested_if_zero() {
    let src = "#if 0\n#if 0\n#if 0\nbad\n#endif\n#endif\n#endif\ngood\n";
    let out = preprocess_c_host(src);
    assert!(!out.contains("bad"), "got: {out:?}");
    assert!(out.contains("good"), "got: {out:?}");
}

#[test]
fn preprocess_macro_not_expanded_in_string_literal() {
    let src = "#define MAX 99\nchar s[] = \"MAX\";\n";
    let out = preprocess_c_host(src);
    assert!(out.contains("\"MAX\""), "macro must not expand inside string: {out:?}");
}

#[test]
fn preprocess_undef_removes_macro() {
    let src = "#define X 1\nX\n#undef X\nX\n";
    let out = preprocess_c_host(src);
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines[0], "1", "before undef: {out:?}");
    assert_eq!(lines[1], "X", "after undef must be unexpanded: {out:?}");
}

#[test]
fn preprocess_backslash_crlf_is_spliced() {
    let out = preprocess_c_host("a\\\r\nb");
    // line splice removes the \\\r\n and joins lines
    assert!(out.contains("a"), "got: {out:?}");
    assert!(out.contains("b"), "got: {out:?}");
    assert!(!out.contains('\n') || out.trim() != "a\nb", "should be spliced: {out:?}");
}

#[test]
fn preprocess_function_like_macro_is_not_expanded() {
    // Function-like macros (with '(' immediately after name) must not be expanded
    let src = "#define FOO(x) x+1\nFOO(5)\n";
    let out = preprocess_c_host(src);
    // FOO(5) is not expanded (function-like macros are unsupported by design)
    assert!(out.contains("FOO"), "function-like macro must not be expanded: {out:?}");
}

// ---------------------------------------------------------------------------
// DfaBuilder: boundary sizes
// ---------------------------------------------------------------------------

#[test]
fn dfa_builder_single_state_single_class() {
    let b = DfaBuilder::new(1, 1);
    let t = b.build().expect("empty pattern set must succeed");
    assert_eq!(t.num_states, 1);
    assert_eq!(t.num_classes, 1);
    assert_eq!(t.transitions.len(), 1);
}

#[test]
fn dfa_builder_zero_states_zero_classes() {
    let b = DfaBuilder::new(0, 0);
    let t = b.build().expect("empty pattern set must succeed");
    assert_eq!(t.transitions.len(), 0);
    assert_eq!(t.token_ids.len(), 0);
}

#[test]
fn dfa_builder_many_states_round_trips_through_wire() {
    let mut b = DfaBuilder::new(64, 64);
    b.continue_to(0, 0, 1).expect("state 1 fits in u16");
    b.accept(1, 7);
    let dfa = b.build().expect("empty pattern set must succeed");
    let blob = PackedBlob::from_dfa(&dfa);
    let got = decode_dfa_from_bytes(&blob.bytes).expect("decode");
    assert_eq!(got.num_states, 64);
    assert_eq!(got.num_classes, 64);
    assert_eq!(got.token_ids[1], 7);
}
