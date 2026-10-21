//! Unit tests: focused behavior of individual public functions and types.

use vyre_grammar_gen::{
    build_c11_lexer_dfa, build_c11_lexer_dfa_for_host,
    decode_dfa_from_bytes, decode_lr_from_bytes,
    kinds_blake3,
    lex_c11_max_munch_kinds,
    preprocess_c_host,
    validate_lr_table,
    wire::{BlobKind, PackedBlob, MAGIC, VERSION},
    DfaBuilder, LrBuilder,
    c11_lexer::{
        TOK_IDENTIFIER, TOK_INTEGER, TOK_WHITESPACE, TOK_COMMENT,
        TOK_IF, TOK_INT, TOK_RETURN, TOK_STRUCT,
        TOK_LPAREN, TOK_RPAREN, TOK_SEMICOLON, TOK_LBRACE, TOK_RBRACE,
        TOK_EQ, TOK_NE,
        TOK_AND, TOK_OR, TOK_INC, TOK_DEC, TOK_COMMA, TOK_ELLIPSIS,
        TOK_ARROW, TOK_DOT, TOK_COLON, TOK_QUESTION,
        C11_PATTERNS,
    },
    lr::{Action, Production},
};

// ---------------------------------------------------------------------------
// dfa::Action pack/unpack
// ---------------------------------------------------------------------------

#[test]
fn dfa_action_pack_unpack_continue() {
    let t = vyre_grammar_gen::dfa::Transition {
        next_state: 7,
        action: vyre_grammar_gen::dfa::Action::Continue,
    };
    let got = vyre_grammar_gen::dfa::Transition::unpack(t.pack());
    assert_eq!(got.next_state, 7);
    assert_eq!(got.action, vyre_grammar_gen::dfa::Action::Continue);
}

#[test]
fn dfa_action_pack_unpack_emit_token() {
    let t = vyre_grammar_gen::dfa::Transition {
        next_state: 0,
        action: vyre_grammar_gen::dfa::Action::EmitToken,
    };
    let got = vyre_grammar_gen::dfa::Transition::unpack(t.pack());
    assert_eq!(got.action, vyre_grammar_gen::dfa::Action::EmitToken);
}

#[test]
fn dfa_action_pack_unpack_push_back() {
    let t = vyre_grammar_gen::dfa::Transition {
        next_state: 255,
        action: vyre_grammar_gen::dfa::Action::PushBack,
    };
    let got = vyre_grammar_gen::dfa::Transition::unpack(t.pack());
    assert_eq!(got.next_state, 255);
    assert_eq!(got.action, vyre_grammar_gen::dfa::Action::PushBack);
}

#[test]
fn dfa_action_pack_unpack_error() {
    let t = vyre_grammar_gen::dfa::Transition {
        next_state: 0,
        action: vyre_grammar_gen::dfa::Action::Error,
    };
    let got = vyre_grammar_gen::dfa::Transition::unpack(t.pack());
    assert_eq!(got.action, vyre_grammar_gen::dfa::Action::Error);
}

#[test]
fn dfa_action_high_bits_parse_as_error() {
    // Any unknown low-bits value (3+) -> Error
    let word = 0xFFFF_FFFF_u32; // next_state=0xFFFF, action bits = 0xFFFF -> Error
    let t = vyre_grammar_gen::dfa::Transition::unpack(word);
    assert_eq!(t.action, vyre_grammar_gen::dfa::Action::Error);
}

// ---------------------------------------------------------------------------
// DfaBuilder
// ---------------------------------------------------------------------------

#[test]
fn dfa_builder_new_allocates_correct_dimensions() {
    let b = DfaBuilder::new(3, 5);
    let t = b.build().expect("empty pattern set must succeed");
    assert_eq!(t.num_states, 3);
    assert_eq!(t.num_classes, 5);
    assert_eq!(t.transitions.len(), 15);
    assert_eq!(t.token_ids.len(), 3);
}

#[test]
fn dfa_builder_all_errors_by_default() {
    let b = DfaBuilder::new(2, 4);
    let t = b.build().expect("empty pattern set must succeed");
    for &w in &t.transitions {
        assert_eq!(
            vyre_grammar_gen::dfa::Transition::unpack(w).action,
            vyre_grammar_gen::dfa::Action::Error
        );
    }
}

#[test]
fn dfa_builder_continue_to_sets_correct_cell() {
    let mut b = DfaBuilder::new(4, 8);
    b.continue_to(2, 3, 3).expect("state 3 fits in u16");
    let t = b.build().expect("empty pattern set must succeed");
    let tr = t.transition(2, 3);
    assert_eq!(tr.next_state, 3);
    assert_eq!(tr.action, vyre_grammar_gen::dfa::Action::Continue);
    // Other cells are still Error
    assert_eq!(
        t.transition(0, 0).action,
        vyre_grammar_gen::dfa::Action::Error
    );
}

#[test]
fn dfa_builder_accept_sets_token_id() {
    let mut b = DfaBuilder::new(3, 4);
    b.accept(1, 99);
    let t = b.build().expect("empty pattern set must succeed");
    assert_eq!(t.token_ids[1], 99);
    assert_eq!(t.token_ids[0], 0);
    assert_eq!(t.token_ids[2], 0);
}

// ---------------------------------------------------------------------------
// LR Action pack/unpack
// ---------------------------------------------------------------------------

#[test]
fn lr_action_shift_roundtrip() {
    assert_eq!(Action::unpack(Action::Shift(42).pack()), Action::Shift(42));
}

#[test]
fn lr_action_shift_zero() {
    assert_eq!(Action::unpack(Action::Shift(0).pack()), Action::Shift(0));
}

#[test]
fn lr_action_reduce_roundtrip() {
    assert_eq!(
        Action::unpack(Action::Reduce(7).pack()),
        Action::Reduce(7)
    );
}

#[test]
fn lr_action_accept_roundtrip() {
    assert_eq!(Action::unpack(Action::Accept.pack()), Action::Accept);
}

#[test]
fn lr_action_error_roundtrip() {
    assert_eq!(Action::unpack(Action::Error.pack()), Action::Error);
}

#[test]
fn lr_action_max_payload() {
    // The max payload for shift/reduce is 0x0FFF_FFFF
    let big = 0x0FFF_FFFFu32;
    assert_eq!(Action::unpack(Action::Shift(big).pack()), Action::Shift(big));
    assert_eq!(
        Action::unpack(Action::Reduce(big).pack()),
        Action::Reduce(big)
    );
}

// ---------------------------------------------------------------------------
// LrBuilder
// ---------------------------------------------------------------------------

#[test]
fn lr_builder_empty_table_dimensions() {
    let t = LrBuilder::new(3, 4, 2).build();
    assert_eq!(t.num_states, 3);
    assert_eq!(t.num_tokens, 4);
    assert_eq!(t.num_nonterminals, 2);
    assert_eq!(t.action.len(), 12);
    assert_eq!(t.goto.len(), 6);
}

#[test]
fn lr_builder_default_action_is_error() {
    let t = LrBuilder::new(2, 3, 1).build();
    for &w in &t.action {
        assert_eq!(Action::unpack(w), Action::Error);
    }
}

#[test]
fn lr_builder_default_goto_is_u32_max() {
    let t = LrBuilder::new(2, 3, 1).build();
    for &g in &t.goto {
        assert_eq!(g, u32::MAX);
    }
}

#[test]
fn lr_builder_set_action_and_lookup() {
    let mut b = LrBuilder::new(4, 3, 1);
    b.set_action(1, 2, Action::Shift(3));
    let t = b.build();
    assert_eq!(t.action_at(1, 2), Action::Shift(3));
    assert_eq!(t.action_at(0, 0), Action::Error);
}

#[test]
fn lr_builder_add_production_returns_sequential_ids() {
    let mut b = LrBuilder::new(2, 2, 2);
    let p0 = b.add_production(0, 2);
    let p1 = b.add_production(1, 3);
    let p2 = b.add_production(0, 0);
    assert_eq!(p0, 0);
    assert_eq!(p1, 1);
    assert_eq!(p2, 2);
    let t = b.build();
    assert_eq!(t.productions[0], Production { lhs: 0, rhs_len: 2 });
    assert_eq!(t.productions[1], Production { lhs: 1, rhs_len: 3 });
    assert_eq!(t.productions[2], Production { lhs: 0, rhs_len: 0 });
}

#[test]
fn lr_builder_set_goto_and_lookup() {
    let mut b = LrBuilder::new(3, 2, 2);
    b.set_goto(1, 0, 2);
    let t = b.build();
    assert_eq!(t.goto_at(1, 0), 2);
    assert_eq!(t.goto_at(0, 0), u32::MAX);
}

// ---------------------------------------------------------------------------
// validate_lr_table
// ---------------------------------------------------------------------------

#[test]
fn validate_accepts_well_formed_table() {
    let mut b = LrBuilder::new(2, 2, 1);
    b.set_action(0, 0, Action::Accept);
    assert!(validate_lr_table(&b.build()).is_ok());
}

#[test]
fn validate_rejects_short_action_vector() {
    let mut t = LrBuilder::new(2, 2, 1).build();
    t.action.pop();
    assert!(validate_lr_table(&t).is_err());
}

#[test]
fn validate_rejects_short_goto_vector() {
    let mut t = LrBuilder::new(2, 2, 1).build();
    t.goto.pop();
    assert!(validate_lr_table(&t).is_err());
}

#[test]
fn validate_error_message_contains_fix() {
    let mut t = LrBuilder::new(2, 2, 1).build();
    t.action.pop();
    let err = validate_lr_table(&t).unwrap_err();
    assert!(err.contains("Fix:"), "error must be actionable: {err}");
}

// ---------------------------------------------------------------------------
// wire: MAGIC and VERSION constants
// ---------------------------------------------------------------------------

#[test]
fn magic_is_sggc() {
    assert_eq!(&MAGIC, b"SGGC");
}

#[test]
fn version_is_one() {
    assert_eq!(VERSION, 1);
}

#[test]
fn blob_kind_lexer_dfa_discriminant() {
    assert_eq!(BlobKind::LexerDfa as u16, 0);
}

#[test]
fn blob_kind_lr_tables_discriminant() {
    assert_eq!(BlobKind::LrTables as u16, 1);
}

// ---------------------------------------------------------------------------
// PackedBlob from_dfa
// ---------------------------------------------------------------------------

#[test]
fn packed_blob_dfa_has_correct_magic() {
    let dfa = DfaBuilder::new(2, 4).build().expect("empty pattern set must succeed");
    let blob = PackedBlob::from_dfa(&dfa);
    assert_eq!(&blob.bytes[0..4], b"SGGC");
}

#[test]
fn packed_blob_dfa_has_correct_version() {
    let dfa = DfaBuilder::new(2, 4).build().expect("empty pattern set must succeed");
    let blob = PackedBlob::from_dfa(&dfa);
    let ver = u16::from_le_bytes([blob.bytes[4], blob.bytes[5]]);
    assert_eq!(ver, 1);
}

#[test]
fn packed_blob_dfa_has_correct_kind_byte() {
    let dfa = DfaBuilder::new(2, 4).build().expect("empty pattern set must succeed");
    let blob = PackedBlob::from_dfa(&dfa);
    let kind = u16::from_le_bytes([blob.bytes[6], blob.bytes[7]]);
    assert_eq!(kind, 0); // LexerDfa
}

#[test]
fn packed_blob_dfa_kind_field_matches() {
    let dfa = DfaBuilder::new(2, 4).build().expect("empty pattern set must succeed");
    let blob = PackedBlob::from_dfa(&dfa);
    assert_eq!(blob.kind, BlobKind::LexerDfa);
}

#[test]
fn packed_blob_lr_kind_field_matches() {
    let mut b = LrBuilder::new(2, 2, 1);
    b.set_action(0, 0, Action::Accept);
    let lr = b.build();
    let blob = PackedBlob::from_lr(&lr).expect("valid LR table must pack");
    assert_eq!(blob.kind, BlobKind::LrTables);
}

// ---------------------------------------------------------------------------
// decode_dfa_from_bytes / decode_lr_from_bytes (minimal parsing)
// ---------------------------------------------------------------------------

#[test]
fn decode_dfa_roundtrips_zero_transitions() {
    let dfa = DfaBuilder::new(1, 1).build().expect("empty pattern set must succeed");
    let blob = PackedBlob::from_dfa(&dfa);
    let got = decode_dfa_from_bytes(&blob.bytes).expect("decode");
    assert_eq!(got.num_states, 1);
    assert_eq!(got.num_classes, 1);
}

#[test]
fn decode_lr_roundtrips_minimal() {
    let mut b = LrBuilder::new(2, 2, 1);
    b.set_action(0, 0, Action::Accept);
    let lr = b.build();
    let blob = PackedBlob::from_lr(&lr).expect("valid LR table must pack");
    let got = decode_lr_from_bytes(&blob.bytes).expect("decode");
    assert_eq!(got.num_states, lr.num_states);
    assert_eq!(got.num_tokens, lr.num_tokens);
    assert_eq!(got.action_at(0, 0), Action::Accept);
}

// ---------------------------------------------------------------------------
// build_c11_lexer_dfa
// ---------------------------------------------------------------------------

#[test]
fn c11_lexer_dfa_has_multiple_states() {
    let dfa = build_c11_lexer_dfa();
    assert!(dfa.num_states > 1, "C11 DFA must have >1 states, got {}", dfa.num_states);
}

#[test]
fn c11_lexer_dfa_for_host_has_multiple_states() {
    let dfa = build_c11_lexer_dfa_for_host();
    assert!(dfa.num_states > 1, "host DFA must have >1 states, got {}", dfa.num_states);
}

#[test]
fn c11_patterns_not_empty() {
    assert!(!C11_PATTERNS.is_empty(), "C11_PATTERNS must not be empty");
}

#[test]
fn c11_patterns_each_has_nonzero_token_id() {
    for &(id, _pat) in C11_PATTERNS {
        assert_ne!(id, 0, "token id 0 is reserved for non-accepting states");
    }
}

// ---------------------------------------------------------------------------
// lex_c11_max_munch_kinds: basic token recognition
// ---------------------------------------------------------------------------

#[test]
fn lex_recognizes_identifier() {
    let kinds = lex_c11_max_munch_kinds(b"foo").expect("lex");
    assert_eq!(kinds, vec![TOK_IDENTIFIER]);
}

#[test]
fn lex_recognizes_integer() {
    let kinds = lex_c11_max_munch_kinds(b"42").expect("lex");
    assert_eq!(kinds, vec![TOK_INTEGER]);
}

#[test]
fn lex_recognizes_keyword_if() {
    let kinds = lex_c11_max_munch_kinds(b"if").expect("lex");
    assert_eq!(kinds, vec![TOK_IF]);
}

#[test]
fn lex_recognizes_keyword_int() {
    let kinds = lex_c11_max_munch_kinds(b"int").expect("lex");
    assert_eq!(kinds, vec![TOK_INT]);
}

#[test]
fn lex_recognizes_keyword_return() {
    let kinds = lex_c11_max_munch_kinds(b"return").expect("lex");
    assert_eq!(kinds, vec![TOK_RETURN]);
}

#[test]
fn lex_recognizes_keyword_struct() {
    let kinds = lex_c11_max_munch_kinds(b"struct").expect("lex");
    assert_eq!(kinds, vec![TOK_STRUCT]);
}

#[test]
fn lex_separates_keywords_and_identifiers() {
    let kinds = lex_c11_max_munch_kinds(b"int x").expect("lex");
    assert_eq!(kinds, vec![TOK_INT, TOK_WHITESPACE, TOK_IDENTIFIER]);
}

#[test]
fn lex_parens_and_semicolon() {
    let kinds = lex_c11_max_munch_kinds(b"();").expect("lex");
    assert_eq!(kinds, vec![TOK_LPAREN, TOK_RPAREN, TOK_SEMICOLON]);
}

#[test]
fn lex_braces() {
    let kinds = lex_c11_max_munch_kinds(b"{}").expect("lex");
    assert_eq!(kinds, vec![TOK_LBRACE, TOK_RBRACE]);
}

#[test]
fn lex_compound_operators_ne_and_eq() {
    let ne = lex_c11_max_munch_kinds(b"!=").expect("lex");
    assert_eq!(ne, vec![TOK_NE]);
    let eq = lex_c11_max_munch_kinds(b"==").expect("lex");
    assert_eq!(eq, vec![TOK_EQ]);
}

#[test]
fn lex_logical_operators() {
    let and = lex_c11_max_munch_kinds(b"&&").expect("lex");
    assert_eq!(and, vec![TOK_AND]);
    let or = lex_c11_max_munch_kinds(b"||").expect("lex");
    assert_eq!(or, vec![TOK_OR]);
}

#[test]
fn lex_inc_dec() {
    let inc = lex_c11_max_munch_kinds(b"++").expect("lex");
    assert_eq!(inc, vec![TOK_INC]);
    let dec = lex_c11_max_munch_kinds(b"--").expect("lex");
    assert_eq!(dec, vec![TOK_DEC]);
}

#[test]
fn lex_line_comment() {
    let kinds = lex_c11_max_munch_kinds(b"// hello").expect("lex");
    assert_eq!(kinds, vec![TOK_COMMENT]);
}

#[test]
fn lex_block_comment() {
    let kinds = lex_c11_max_munch_kinds(b"/* ok */").expect("lex");
    assert_eq!(kinds, vec![TOK_COMMENT]);
}

#[test]
fn lex_arrow_operator() {
    let kinds = lex_c11_max_munch_kinds(b"->").expect("lex");
    assert_eq!(kinds, vec![TOK_ARROW]);
}

#[test]
fn lex_dot_operator() {
    let kinds = lex_c11_max_munch_kinds(b".").expect("lex");
    assert_eq!(kinds, vec![TOK_DOT]);
}

#[test]
fn lex_ellipsis() {
    let kinds = lex_c11_max_munch_kinds(b"...").expect("lex");
    assert_eq!(kinds, vec![TOK_ELLIPSIS]);
}

#[test]
fn lex_comma() {
    let kinds = lex_c11_max_munch_kinds(b",").expect("lex");
    assert_eq!(kinds, vec![TOK_COMMA]);
}

#[test]
fn lex_colon() {
    let kinds = lex_c11_max_munch_kinds(b":").expect("lex");
    assert_eq!(kinds, vec![TOK_COLON]);
}

#[test]
fn lex_question_mark() {
    let kinds = lex_c11_max_munch_kinds(b"?").expect("lex");
    assert_eq!(kinds, vec![TOK_QUESTION]);
}

#[test]
fn lex_integer_hex() {
    let kinds = lex_c11_max_munch_kinds(b"0xFF").expect("lex");
    assert_eq!(kinds, vec![TOK_INTEGER]);
}

#[test]
fn lex_integer_octal() {
    let kinds = lex_c11_max_munch_kinds(b"0755").expect("lex");
    assert_eq!(kinds, vec![TOK_INTEGER]);
}

// ---------------------------------------------------------------------------
// preprocess_c_host
// ---------------------------------------------------------------------------

#[test]
fn preprocess_identity_for_plain_code() {
    let s = "int x = 1;";
    let out = preprocess_c_host(s);
    assert_eq!(out, "int x = 1;");
}

#[test]
fn preprocess_removes_line_comment() {
    let out = preprocess_c_host("int x; // comment");
    assert!(!out.contains("comment"));
    assert!(out.contains("int x;"));
}

#[test]
fn preprocess_removes_block_comment() {
    let out = preprocess_c_host("int /* block */ x;");
    assert!(!out.contains("block"));
    assert!(out.contains("int"));
    assert!(out.contains("x;"));
}

#[test]
fn preprocess_expands_simple_macro() {
    let out = preprocess_c_host("#define N 10\nint arr[N];");
    assert!(out.contains("int arr[10];"), "got: {out:?}");
    assert!(!out.contains("N]"), "macro not expanded: {out:?}");
}

#[test]
fn preprocess_strips_if_zero() {
    let out = preprocess_c_host("#if 0\ndeadcode();\n#endif\nok();");
    assert!(!out.contains("deadcode"));
    assert!(out.contains("ok()"));
}

#[test]
fn preprocess_splices_backslash_newline() {
    let out = preprocess_c_host("a\\\nb");
    assert!(out.contains("a b"), "got: {out:?}");
}

// ---------------------------------------------------------------------------
// kinds_blake3: stability / determinism
// ---------------------------------------------------------------------------

#[test]
fn kinds_blake3_is_deterministic() {
    let kinds = vec![1u32, 2, 3];
    let h1 = kinds_blake3(&kinds);
    let h2 = kinds_blake3(&kinds);
    assert_eq!(h1, h2);
}

#[test]
fn kinds_blake3_empty_differs_from_nonempty() {
    let h_empty = kinds_blake3(&[]);
    let h_one = kinds_blake3(&[1]);
    assert_ne!(h_empty, h_one);
}

#[test]
fn kinds_blake3_order_sensitive() {
    let h1 = kinds_blake3(&[1, 2]);
    let h2 = kinds_blake3(&[2, 1]);
    assert_ne!(h1, h2);
}
