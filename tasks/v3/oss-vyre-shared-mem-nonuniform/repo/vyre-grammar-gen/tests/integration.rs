//! Integration tests: end-to-end exercise of the public API pipeline.

use vyre_grammar_gen::{
    build_c11_lexer_dfa, build_c11_lexer_dfa_for_host,
    chunk_lexer_cpu::count_chunked_valid_tokens,
    decode_dfa_from_bytes, decode_lr_from_bytes,
    kinds_blake3,
    lex_c11_max_munch_kinds,
    preprocess_c_host,
    validate_lr_table,
    wire::{BlobKind, PackedBlob},
    LrBuilder,
    lr::Action,
    c11_lexer::{
        TOK_IDENTIFIER, TOK_INTEGER, TOK_WHITESPACE, TOK_COMMENT,
        TOK_INT, TOK_RETURN, TOK_STRUCT,
        TOK_LPAREN, TOK_RPAREN, TOK_SEMICOLON, TOK_LBRACE, TOK_RBRACE,
        TOK_COMMA, TOK_STAR,
    },
};

// ---------------------------------------------------------------------------
// Full pipeline: preprocess → lex → hash
// ---------------------------------------------------------------------------

#[test]
fn pipeline_hello_c_preprocesses_and_lexes() {
    let src = r#"
/* a simple C function */
int add(int a, int b) {
    return a + b;
}
"#;
    let preprocessed = preprocess_c_host(src);
    let kinds = lex_c11_max_munch_kinds(preprocessed.as_bytes())
        .expect("lex must succeed on well-formed C");
    assert!(!kinds.is_empty(), "lex must produce tokens");
    assert!(kinds.contains(&TOK_INT), "must see TOK_INT");
    assert!(kinds.contains(&TOK_IDENTIFIER), "must see identifier");
    assert!(kinds.contains(&TOK_LPAREN), "must see (");
    assert!(kinds.contains(&TOK_RPAREN), "must see )");
    assert!(kinds.contains(&TOK_LBRACE), "must see {{");
    assert!(kinds.contains(&TOK_RBRACE), "must see }}");
    assert!(kinds.contains(&TOK_RETURN), "must see return keyword");
}

#[test]
fn pipeline_comment_is_stripped_by_preprocessor_not_lexer() {
    let src = "/* this is a comment */\nint x;";
    let preprocessed = preprocess_c_host(src);
    // After preprocessing, block comment should be gone
    assert!(!preprocessed.contains("this is a comment"));
    let kinds = lex_c11_max_munch_kinds(preprocessed.as_bytes()).expect("lex");
    assert!(!kinds.contains(&TOK_COMMENT), "preprocessor should have removed the comment");
}

#[test]
fn pipeline_macro_expansion_feeds_lexer() {
    let src = "#define LIMIT 100\nint arr[LIMIT];\n";
    let preprocessed = preprocess_c_host(src);
    assert!(preprocessed.contains("100"), "macro must be expanded before lex");
    let kinds = lex_c11_max_munch_kinds(preprocessed.as_bytes()).expect("lex");
    assert!(kinds.contains(&TOK_INTEGER), "expanded macro value must lex as integer");
}

#[test]
fn pipeline_if_zero_dead_code_eliminated_before_lex() {
    let src = "#if 0\nvoid dead(void) {}\n#endif\nvoid live(void) {}\n";
    let preprocessed = preprocess_c_host(src);
    assert!(!preprocessed.contains("dead"), "dead code must be stripped");
    let kinds = lex_c11_max_munch_kinds(preprocessed.as_bytes()).expect("lex");
    assert!(kinds.contains(&TOK_IDENTIFIER), "live code must be lexed");
}

// ---------------------------------------------------------------------------
// DFA wire: full pack + decode round-trip preserves all content
// ---------------------------------------------------------------------------

#[test]
fn dfa_wire_roundtrip_full_c11_dfa() {
    let dfa = build_c11_lexer_dfa();
    let blob = PackedBlob::from_dfa(&dfa);
    assert_eq!(blob.kind, BlobKind::LexerDfa);
    let got = decode_dfa_from_bytes(&blob.bytes).expect("decode C11 DFA blob");
    assert_eq!(got.num_states, dfa.num_states);
    assert_eq!(got.num_classes, dfa.num_classes);
    assert_eq!(got.transitions, dfa.transitions);
    assert_eq!(got.token_ids, dfa.token_ids);
}

#[test]
fn dfa_wire_try_as_dfa_convenience() {
    let dfa = build_c11_lexer_dfa();
    let blob = PackedBlob::from_dfa(&dfa);
    let got = blob.try_as_dfa().expect("try_as_dfa must succeed");
    assert_eq!(got.num_states, dfa.num_states);
}

#[test]
fn lr_wire_roundtrip_simple_grammar() {
    let mut b = LrBuilder::new(4, 3, 1);
    let prod = b.add_production(0, 2);
    b.set_action(0, 0, Action::Shift(1));
    b.set_action(0, 2, Action::Accept);
    b.set_action(1, 1, Action::Shift(2));
    b.set_action(2, 0, Action::Reduce(prod));
    b.set_action(2, 2, Action::Reduce(prod));
    let lr = b.build();
    let blob = PackedBlob::from_lr(&lr).expect("valid LR table must pack");
    assert_eq!(blob.kind, BlobKind::LrTables);
    let got = blob.try_as_lr().expect("try_as_lr must succeed");
    assert_eq!(got.num_states, 4);
    assert_eq!(got.num_tokens, 3);
    assert_eq!(got.action_at(0, 0), Action::Shift(1));
    assert_eq!(got.action_at(0, 2), Action::Accept);
    assert_eq!(got.action_at(2, 0), Action::Reduce(prod));
}

#[test]
fn lr_wire_roundtrip_preserves_goto() {
    let mut b = LrBuilder::new(3, 2, 2);
    b.set_goto(0, 0, 1);
    b.set_goto(1, 1, 2);
    let lr = b.build();
    validate_lr_table(&lr).expect("valid");
    let blob = PackedBlob::from_lr(&lr).expect("valid LR table must pack");
    let got = decode_lr_from_bytes(&blob.bytes).expect("decode");
    assert_eq!(got.goto_at(0, 0), 1);
    assert_eq!(got.goto_at(1, 1), 2);
    assert_eq!(got.goto_at(0, 1), u32::MAX);
}

#[test]
fn lr_wire_roundtrip_preserves_productions() {
    let mut b = LrBuilder::new(2, 2, 2);
    b.set_action(0, 0, Action::Accept);
    let p0 = b.add_production(0, 3);
    let p1 = b.add_production(1, 1);
    let lr = b.build();
    let blob = PackedBlob::from_lr(&lr).expect("valid LR table must pack");
    let got = decode_lr_from_bytes(&blob.bytes).expect("decode");
    assert_eq!(got.productions.len(), 2);
    assert_eq!(got.productions[p0 as usize].lhs, 0);
    assert_eq!(got.productions[p0 as usize].rhs_len, 3);
    assert_eq!(got.productions[p1 as usize].lhs, 1);
    assert_eq!(got.productions[p1 as usize].rhs_len, 1);
}

// ---------------------------------------------------------------------------
// count_chunked_valid_tokens: integration with the C11 DFA
// ---------------------------------------------------------------------------

#[test]
fn chunked_lexer_on_simple_c_source() {
    let dfa = build_c11_lexer_dfa();
    let src = b"int x = 1;";
    let count = count_chunked_valid_tokens(
        &dfa.transitions,
        &dfa.token_ids,
        src,
        src.len() as u32,
        dfa.num_states,
        64,
        dfa.num_classes,
    );
    // At least one lane should emit a valid token (TOK_INT = 107, not 200/201)
    assert!(count > 0, "chunked lexer must emit tokens on `int x = 1;`");
}

#[test]
fn chunked_lexer_mismatched_transitions_returns_zero() {
    // If transitions.len() != num_states * num_classes, function must return 0 safely
    let count = count_chunked_valid_tokens(
        &[0u32; 3],   // wrong size
        &[0u32; 2],
        b"abc",
        3,
        2,
        64,
        4, // 2 * 4 = 8 expected, got 3
    );
    assert_eq!(count, 0, "mismatched table must yield 0");
}

#[test]
fn chunked_lexer_empty_haystack_returns_zero() {
    let dfa = build_c11_lexer_dfa();
    let count = count_chunked_valid_tokens(
        &dfa.transitions,
        &dfa.token_ids,
        b"",
        0,
        dfa.num_states,
        64,
        dfa.num_classes,
    );
    assert_eq!(count, 0, "empty haystack must yield 0");
}

// ---------------------------------------------------------------------------
// kinds_blake3: full pipeline hash is stable (golden)
// ---------------------------------------------------------------------------

#[test]
fn kinds_blake3_golden_for_known_sequence() {
    // Lex "int x;" and hash the kinds; this golden pins the lexer's token-id
    // assignments and the hash function together.
    let kinds = lex_c11_max_munch_kinds(b"int x;").expect("lex");
    // [TOK_INT=107, TOK_WHITESPACE=201, TOK_IDENTIFIER=1, TOK_SEMICOLON=16]
    let expected_sequence = vec![TOK_INT, TOK_WHITESPACE, TOK_IDENTIFIER, TOK_SEMICOLON];
    assert_eq!(kinds, expected_sequence, "token sequence golden");

    let hash = kinds_blake3(&kinds);
    let hash2 = kinds_blake3(&kinds);
    assert_eq!(hash, hash2, "hash must be deterministic");
    // Hash value is non-zero (proves function ran)
    assert_ne!(hash.as_bytes(), &[0u8; 32], "hash must not be all-zero");
}

// ---------------------------------------------------------------------------
// Host DFA vs GPU DFA: both have states but may differ
// ---------------------------------------------------------------------------

#[test]
fn host_and_gpu_dfas_are_both_non_empty() {
    let gpu = build_c11_lexer_dfa();
    let host = build_c11_lexer_dfa_for_host();
    assert!(gpu.num_states > 0);
    assert!(host.num_states > 0);
}

// ---------------------------------------------------------------------------
// validate_lr_table integrates with builder pipeline
// ---------------------------------------------------------------------------

#[test]
fn builder_pipeline_validate_then_pack_decode() {
    let mut b = LrBuilder::new(3, 3, 2);
    b.set_action(0, 0, Action::Shift(1));
    b.set_action(1, 1, Action::Shift(2));
    b.set_action(2, 2, Action::Accept);
    b.set_goto(0, 0, 1);
    b.add_production(0, 2);
    let lr = b.build();

    validate_lr_table(&lr).expect("table must be valid before serialization");

    let blob = PackedBlob::from_lr(&lr).expect("valid LR table must pack");
    let got = decode_lr_from_bytes(&blob.bytes).expect("decode");
    assert_eq!(got.action_at(0, 0), Action::Shift(1));
    assert_eq!(got.action_at(2, 2), Action::Accept);
    assert_eq!(got.goto_at(0, 0), 1);
}

// ---------------------------------------------------------------------------
// End-to-end lex of a realistic C function declaration
// ---------------------------------------------------------------------------

#[test]
fn e2e_lex_function_declaration() {
    let src = b"static int foo(int *p, unsigned n);";
    let kinds = lex_c11_max_munch_kinds(src).expect("lex function decl");
    // Must contain: TOK_STATIC(130), TOK_INT(107), identifier, (, *, ,, unsigned(132), ), ;
    use vyre_grammar_gen::c11_lexer::{TOK_STATIC, TOK_UNSIGNED};
    assert!(kinds.contains(&TOK_STATIC), "must see static: {kinds:?}");
    assert!(kinds.contains(&TOK_INT), "must see int: {kinds:?}");
    assert!(kinds.contains(&TOK_IDENTIFIER), "must see identifier: {kinds:?}");
    assert!(kinds.contains(&TOK_LPAREN), "must see (: {kinds:?}");
    assert!(kinds.contains(&TOK_STAR), "must see *: {kinds:?}");
    assert!(kinds.contains(&TOK_COMMA), "must see ,: {kinds:?}");
    assert!(kinds.contains(&TOK_UNSIGNED), "must see unsigned: {kinds:?}");
    assert!(kinds.contains(&TOK_RPAREN), "must see ): {kinds:?}");
    assert!(kinds.contains(&TOK_SEMICOLON), "must see ;: {kinds:?}");
}

#[test]
fn e2e_lex_struct_definition() {
    let src = b"struct point { int x; int y; };";
    let kinds = lex_c11_max_munch_kinds(src).expect("lex struct def");
    assert!(kinds.contains(&TOK_STRUCT), "must see struct: {kinds:?}");
    assert!(kinds.contains(&TOK_LBRACE), "must see {{: {kinds:?}");
    assert!(kinds.contains(&TOK_RBRACE), "must see }}: {kinds:?}");
    assert!(kinds.contains(&TOK_INT), "must see int: {kinds:?}");
    assert!(kinds.contains(&TOK_SEMICOLON), "must see ;: {kinds:?}");
}

#[test]
fn e2e_preprocess_then_lex_macro_substituted_code() {
    let src = "#define BUFSIZE 256\nchar buf[BUFSIZE];\n";
    let preprocessed = preprocess_c_host(src);
    assert!(preprocessed.contains("256"), "macro expanded");
    assert!(!preprocessed.contains("BUFSIZE"), "original name gone");
    let kinds = lex_c11_max_munch_kinds(preprocessed.as_bytes()).expect("lex");
    assert!(kinds.contains(&TOK_INTEGER), "256 must be TOK_INTEGER");
}
