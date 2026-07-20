//! Contract tests for preprocessing hash token lexing.

#![cfg(feature = "c-parser")]

use c_grammar_gen::lex_c11_max_munch_kinds;
use vyre_libs::parsing::c::lex::tokens::{TOK_HASH, TOK_IDENTIFIER, TOK_PREPROC, TOK_WHITESPACE};

#[test]
fn standalone_hash_tokens_are_lexed_as_hash_punctuation() {
    let kinds = lex_c11_max_munch_kinds(b"a # # b").expect("hash fixture must lex");
    assert_eq!(
        kinds,
        vec![
            TOK_IDENTIFIER,
            TOK_WHITESPACE,
            TOK_HASH,
            TOK_WHITESPACE,
            TOK_HASH,
            TOK_WHITESPACE,
            TOK_IDENTIFIER,
        ],
        "standalone # tokens must not be swallowed into identifiers or directives"
    );
}

#[test]
fn directive_lines_are_lexed_as_single_preproc_rows() {
    let kinds = lex_c11_max_munch_kinds(b"#define FOO 1\nx").expect("directive fixture must lex");
    assert_eq!(
        kinds,
        vec![TOK_PREPROC, TOK_WHITESPACE, TOK_IDENTIFIER],
        "full directive lines must remain compact TOK_PREPROC rows for the parser lane"
    );
}

#[test]
fn directive_lines_with_stringize_paste_and_pragma_stay_preproc_rows() {
    for source in [
        b"#define STR(x) #x" as &[u8],
        b"#define CAT(a, b) a ## b",
        b"#pragma once",
    ] {
        let kinds = lex_c11_max_munch_kinds(source).expect("directive fixture must lex");
        assert_eq!(
            kinds,
            vec![TOK_PREPROC],
            "directive source {:?} must stay one compact TOK_PREPROC row",
            std::str::from_utf8(source).unwrap()
        );
    }
}
