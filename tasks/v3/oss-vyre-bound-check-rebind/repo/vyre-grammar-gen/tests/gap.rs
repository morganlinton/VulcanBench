//! Gap tests: documented current limitations and edge cases, pinned so a
//! future change must be deliberate.

use vyre_grammar_gen::{
    decode_dfa_from_bytes, decode_lr_from_bytes,
    lex_c11_max_munch_kinds,
    preprocess_c_host,
    wire::{PackedBlob, WireError},
    LrBuilder,
    lr::Action,
    c11_lexer::{
        TOK_IDENTIFIER, TOK_WHITESPACE, TOK_COMMENT,
        TOK_INTEGER, TOK_PREPROC, TOK_HASH,
        TOK_ALIGNAS, TOK_ALIGNOF, TOK_ATOMIC,
        TOK_BOOL, TOK_COMPLEX, TOK_GENERIC,
        TOK_IMAGINARY, TOK_NORETURN, TOK_STATIC_ASSERT, TOK_THREAD_LOCAL,
        TOK_GNU_ASM, TOK_GNU_ATTRIBUTE, TOK_GNU_TYPEOF, TOK_GNU_EXTENSION,
        TOK_BUILTIN_CONSTANT_P,
        TOK_INC, TOK_PLUS,
    },
};

// ---------------------------------------------------------------------------
// GAP: wire format only supports VERSION == 1; future versions are rejected.
// ---------------------------------------------------------------------------

/// If the version bumps, this test must be deliberately updated.
#[test]
fn wire_version_is_pinned_at_one() {
    use vyre_grammar_gen::wire::VERSION;
    assert_eq!(
        VERSION, 1,
        "Wire format version changed. Update all consumers before bumping."
    );
}

/// A blob with version 2 is rejected; this pins the behaviour so a version
/// bump requires fixing both the encoder and this test.
#[test]
fn wire_version_2_is_unsupported() {
    use vyre_grammar_gen::wire::MAGIC;
    let mut bytes = vec![0u8; 40];
    bytes[0..4].copy_from_slice(&MAGIC);
    bytes[4..6].copy_from_slice(&2u16.to_le_bytes()); // version 2
    let err = decode_dfa_from_bytes(&bytes).unwrap_err();
    assert!(
        matches!(err, WireError::UnsupportedVersion(2)),
        "expected UnsupportedVersion(2), got {err:?}"
    );
}

// ---------------------------------------------------------------------------
// GAP: MAGIC constant is pinned; a change breaks GPU-side consumers.
// ---------------------------------------------------------------------------

#[test]
fn magic_constant_is_pinned() {
    use vyre_grammar_gen::wire::MAGIC;
    assert_eq!(MAGIC, *b"SGGC", "MAGIC changed - GPU consumers must be updated");
}

// ---------------------------------------------------------------------------
// GAP: BlobKind discriminants are pinned; GPU shader code uses these values.
// ---------------------------------------------------------------------------

#[test]
fn blob_kind_lexer_dfa_discriminant_pinned() {
    use vyre_grammar_gen::wire::BlobKind;
    assert_eq!(BlobKind::LexerDfa as u16, 0, "LexerDfa discriminant changed");
}

#[test]
fn blob_kind_lr_tables_discriminant_pinned() {
    use vyre_grammar_gen::wire::BlobKind;
    assert_eq!(BlobKind::LrTables as u16, 1, "LrTables discriminant changed");
}

// ---------------------------------------------------------------------------
// GAP: token_id == 0 is reserved for non-accepting states in the GPU DFA.
// No pattern in C11_PATTERNS may use id 0.
// ---------------------------------------------------------------------------

#[test]
fn c11_patterns_never_use_token_id_zero() {
    use vyre_grammar_gen::c11_lexer::C11_PATTERNS;
    for &(id, pat) in C11_PATTERNS {
        assert_ne!(
            id, 0,
            "Pattern `{pat}` uses reserved token id 0 - GPU lexer treats 0 as non-accepting"
        );
    }
}

// ---------------------------------------------------------------------------
// GAP: max_munch lexer strips whitespace (TOK_WHITESPACE = 201) and comments
// (TOK_COMMENT = 200) but still emits them; callers must filter.
// This pins the current semantic: whitespace is not silently dropped.
// ---------------------------------------------------------------------------

#[test]
fn whitespace_is_emitted_not_silently_dropped() {
    let kinds = lex_c11_max_munch_kinds(b"a b").expect("lex");
    assert!(
        kinds.contains(&TOK_WHITESPACE),
        "whitespace must be emitted (not silently dropped), callers filter: {kinds:?}"
    );
}

#[test]
fn line_comment_is_emitted_not_silently_dropped() {
    let kinds = lex_c11_max_munch_kinds(b"//comment").expect("lex");
    assert!(
        kinds.contains(&TOK_COMMENT),
        "comment must be emitted as TOK_COMMENT: {kinds:?}"
    );
}

// ---------------------------------------------------------------------------
// GAP: max-munch priority - keyword before identifier of same prefix.
// e.g. "int" must lex as TOK_INT (not TOK_IDENTIFIER), but "integer"
// must lex as TOK_IDENTIFIER.
// ---------------------------------------------------------------------------

#[test]
fn keyword_int_wins_over_identifier_prefix() {
    use vyre_grammar_gen::c11_lexer::TOK_INT;
    let kinds = lex_c11_max_munch_kinds(b"int").expect("lex");
    assert_eq!(kinds, vec![TOK_INT], "int must lex as keyword: {kinds:?}");
}

#[test]
fn identifier_integer_does_not_lex_as_keyword_int() {
    let kinds = lex_c11_max_munch_kinds(b"integer").expect("lex");
    assert_eq!(
        kinds,
        vec![TOK_IDENTIFIER],
        "`integer` must be an identifier, not keyword int: {kinds:?}"
    );
}

// ---------------------------------------------------------------------------
// GAP: compound operators take priority over single-char ones.
// "++" must lex as TOK_INC, not two TOK_PLUS tokens.
// ---------------------------------------------------------------------------

#[test]
fn plus_plus_lexes_as_inc_not_two_plus() {
    let kinds = lex_c11_max_munch_kinds(b"++").expect("lex");
    assert_eq!(kinds, vec![TOK_INC], "++ must be TOK_INC: {kinds:?}");
}

#[test]
fn single_plus_lexes_as_plus() {
    let kinds = lex_c11_max_munch_kinds(b"+").expect("lex");
    assert_eq!(kinds, vec![TOK_PLUS], "+ must be TOK_PLUS: {kinds:?}");
}

// ---------------------------------------------------------------------------
// GAP: preprocess_c_host does NOT expand function-like macros.
// This is a documented limitation; pinned to detect if it accidentally gains
// that capability (which would change the output contract).
// ---------------------------------------------------------------------------

#[test]
fn function_like_macro_is_not_expanded_gap() {
    let src = "#define ADD(a,b) ((a)+(b))\nADD(1,2)\n";
    let out = preprocess_c_host(src);
    assert!(
        out.contains("ADD"),
        "function-like macros must not be expanded (documented limitation): {out:?}"
    );
}

// ---------------------------------------------------------------------------
// GAP: LR Action tag bits. The SHIFT tag is 0 (not 1,2,3). Changing this
// would silently corrupt all packed LR tables.
// ---------------------------------------------------------------------------

#[test]
fn lr_shift_tag_is_zero() {
    let packed = Action::Shift(0).pack();
    let tag = packed >> 28;
    assert_eq!(tag, 0, "SHIFT tag must be 0 in packed format; got {tag}");
}

#[test]
fn lr_reduce_tag_is_one() {
    let packed = Action::Reduce(0).pack();
    let tag = packed >> 28;
    assert_eq!(tag, 1, "REDUCE tag must be 1 in packed format; got {tag}");
}

#[test]
fn lr_accept_tag_is_two() {
    let packed = Action::Accept.pack();
    let tag = packed >> 28;
    assert_eq!(tag, 2, "ACCEPT tag must be 2 in packed format; got {tag}");
}

#[test]
fn lr_error_tag_is_three() {
    let packed = Action::Error.pack();
    let tag = packed >> 28;
    assert_eq!(tag, 3, "ERROR tag must be 3 in packed format; got {tag}");
}

// ---------------------------------------------------------------------------
// GAP: GNU extension keywords are pinned to their token ids.
// Any renumbering would break GPU-side vyre parsing.
// ---------------------------------------------------------------------------

#[test]
fn gnu_asm_token_id_pinned() {
    assert_eq!(TOK_GNU_ASM, 144, "TOK_GNU_ASM id changed");
}

#[test]
fn gnu_attribute_token_id_pinned() {
    assert_eq!(TOK_GNU_ATTRIBUTE, 145, "TOK_GNU_ATTRIBUTE id changed");
}

#[test]
fn gnu_typeof_token_id_pinned() {
    assert_eq!(TOK_GNU_TYPEOF, 146, "TOK_GNU_TYPEOF id changed");
}

#[test]
fn gnu_extension_token_id_pinned() {
    assert_eq!(TOK_GNU_EXTENSION, 147, "TOK_GNU_EXTENSION id changed");
}

#[test]
fn builtin_constant_p_token_id_pinned() {
    assert_eq!(TOK_BUILTIN_CONSTANT_P, 150, "TOK_BUILTIN_CONSTANT_P id changed");
}

// ---------------------------------------------------------------------------
// GAP: C11 keyword token ids are pinned.
// ---------------------------------------------------------------------------

#[test]
fn c11_keyword_ids_pinned() {
    assert_eq!(TOK_ALIGNAS, 134);
    assert_eq!(TOK_ALIGNOF, 135);
    assert_eq!(TOK_ATOMIC, 136, "TOK_ATOMIC id changed");
    assert_eq!(TOK_BOOL, 137);
    assert_eq!(TOK_COMPLEX, 138);
    assert_eq!(TOK_GENERIC, 139);
    assert_eq!(TOK_IMAGINARY, 140);
    assert_eq!(TOK_NORETURN, 141);
    assert_eq!(TOK_STATIC_ASSERT, 142);
    assert_eq!(TOK_THREAD_LOCAL, 143);
}

// ---------------------------------------------------------------------------
// GAP: LR wire payload must have an even number of production words (pairs).
// A truncated payload with an odd residual is rejected.
// ---------------------------------------------------------------------------

#[test]
fn lr_blob_with_odd_production_words_is_rejected() {
    // Build a valid LR blob, then add one extra byte so the production
    // section has an odd number of u32 residuals.
    let mut b = LrBuilder::new(2, 2, 1);
    b.set_action(0, 0, Action::Accept);
    b.add_production(0, 1);
    let lr = b.build();
    let valid_blob = PackedBlob::from_lr(&lr).expect("valid LR table must pack");

    // Corrupt the payload_len field (bytes 20..24) to claim 4 extra bytes
    // which adds one extra u32 word (odd residual after action+goto).
    // The real gap: if you hand-craft a blob where action+goto section is
    // correct but production residual is odd, LrPayloadSize is returned.
    // We test that by building one manually.
    let _ = valid_blob; // original blob not used further; we construct the test case below
    use vyre_grammar_gen::wire::{MAGIC, VERSION};
    let num_states = 1u32;
    let num_tokens = 1u32;
    let num_nonterminals = 1u32;
    // action: 1 word, goto: 1 word = 2 words = 8 bytes; add 1 extra u32 = odd residual
    let mut payload = Vec::<u8>::new();
    payload.extend_from_slice(&Action::Error.pack().to_le_bytes()); // action[0,0]
    payload.extend_from_slice(&u32::MAX.to_le_bytes());             // goto[0,0]
    payload.extend_from_slice(&0u32.to_le_bytes());                 // 1 extra word = odd residual

    // Compute checksum
    let digest = blake3::hash(&payload);
    let tag: [u8; 16] = digest.as_bytes()[..16].try_into().unwrap();

    let payload_len = payload.len() as u32;
    let mut blob_bytes = Vec::new();
    blob_bytes.extend_from_slice(&MAGIC);
    blob_bytes.extend_from_slice(&VERSION.to_le_bytes());
    blob_bytes.extend_from_slice(&1u16.to_le_bytes()); // kind = LrTables
    blob_bytes.extend_from_slice(&num_states.to_le_bytes());
    blob_bytes.extend_from_slice(&num_tokens.to_le_bytes());
    blob_bytes.extend_from_slice(&num_nonterminals.to_le_bytes());
    blob_bytes.extend_from_slice(&payload_len.to_le_bytes());
    blob_bytes.extend_from_slice(&payload);
    blob_bytes.extend_from_slice(&tag);

    let err = decode_lr_from_bytes(&blob_bytes).unwrap_err();
    assert!(
        matches!(err, WireError::LrPayloadSize),
        "odd production residual must produce LrPayloadSize, got {err:?}"
    );
}

// ---------------------------------------------------------------------------
// GAP: preprocess strips #if 0 blocks but preserves #if <nonzero>.
// ---------------------------------------------------------------------------

#[test]
fn if_nonzero_block_is_not_stripped() {
    let src = "#if 1\nkept\n#endif\n";
    let out = preprocess_c_host(src);
    assert!(out.contains("kept"), "#if 1 block must be preserved: {out:?}");
}

// ---------------------------------------------------------------------------
// GAP: Integer literal lexing: zero alone is valid (not treated as octal prefix).
// ---------------------------------------------------------------------------

#[test]
fn zero_lexes_as_integer() {
    let kinds = lex_c11_max_munch_kinds(b"0").expect("lex");
    assert_eq!(kinds, vec![TOK_INTEGER], "bare 0 must be TOK_INTEGER");
}

// ---------------------------------------------------------------------------
// GAP: preproc directive only fires at line-start (after optional whitespace).
// A '#' mid-token-stream is TOK_HASH, not TOK_PREPROC.
// ---------------------------------------------------------------------------

#[test]
fn hash_mid_stream_is_hash_not_preproc() {
    // "x # y" - the # is not at a line start
    let kinds = lex_c11_max_munch_kinds(b"x # y").expect("lex");
    assert!(
        kinds.contains(&TOK_HASH) && !kinds.contains(&TOK_PREPROC),
        "# after token must be TOK_HASH not TOK_PREPROC: {kinds:?}"
    );
}

#[test]
fn hash_at_line_start_is_preproc() {
    let kinds = lex_c11_max_munch_kinds(b"#define X 1").expect("lex");
    assert!(
        kinds.contains(&TOK_PREPROC),
        "# at line start must be TOK_PREPROC: {kinds:?}"
    );
}
