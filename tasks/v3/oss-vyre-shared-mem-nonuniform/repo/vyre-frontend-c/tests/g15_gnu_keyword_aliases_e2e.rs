//! g15_gnu_keyword_aliases_e2e  -  __restrict / __inline / __volatile__ keyword aliases.
//!
//! Verifies that GNU keyword aliases lex to the same token kinds as their bare
//! counterparts (TOK_RESTRICT, TOK_INLINE, TOK_VOLATILE).
#![allow(deprecated)]

mod support;

use support::*;
use vyre_libs::parsing::c::lex::keyword::reference_c_keyword_types;
use vyre_libs::parsing::c::lex::tokens::{TOK_IDENTIFIER, TOK_INLINE, TOK_RESTRICT, TOK_VOLATILE};

const ALIASES_SOURCE: &str = include_str!("corpus/g15_gnu_keyword_aliases/aliases.c");

#[test]
fn gnu_keyword_aliases_compile_successfully() {
    let object = compile_source("g15_aliases", ALIASES_SOURCE, Vec::new());
    object.assert_elf();
}

#[test]
fn gnu_keyword_aliases_lex_to_same_token_kinds_as_bare_keywords() {
    // Synthetic token stream where each alias is initially classified as
    // TOK_IDENTIFIER. The CPU keyword oracle must reclassify them to the
    // same token kind as the bare keyword.
    let haystack = b"__inline __restrict __volatile__";
    let starts = [0u32, 9, 20];
    let lens = [8u32, 10, 12];
    let tok_types = vec![TOK_IDENTIFIER; 3];

    let result = reference_c_keyword_types(&tok_types, &starts, &lens, haystack);

    assert_eq!(
        result[0], TOK_INLINE,
        "__inline should lex to the same token kind as inline"
    );
    assert_eq!(
        result[1], TOK_RESTRICT,
        "__restrict should lex to the same token kind as restrict"
    );
    assert_eq!(
        result[2], TOK_VOLATILE,
        "__volatile__ should lex to the same token kind as volatile"
    );
}

#[test]
fn typo_negative_twin_rejected_by_gcc() {
    let fixture = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/corpus/g15_gnu_keyword_aliases/negatives/typo.c");

    let output = std::process::Command::new("gcc")
        .args(["-std=gnu11", "-c", "-o", "/dev/null"])
        .arg(&fixture)
        .output()
        .unwrap_or_else(|e| {
            panic!("gcc must be runnable for the GNU keyword negative oracle: {e}")
        });

    assert!(!output.status.success(), "gcc must reject typo __restrct__");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("__restrct__"),
        "gcc error should mention the typo identifier, got: {stderr}"
    );
}
