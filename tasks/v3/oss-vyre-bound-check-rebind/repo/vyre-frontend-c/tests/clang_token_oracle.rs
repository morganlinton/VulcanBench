//! Tests for clang preprocessed-token oracle extraction.

mod support;

use std::fs;

use support::clang_tokens::{clang_preprocessed_token_facts, clang_preprocessed_tokens_required};

#[test]
fn clang_token_oracle_records_macro_spelling_origin() {
    let path =
        std::env::temp_dir().join(format!("vyrec-clang-token-oracle-{}.c", std::process::id()));
    fs::write(&path, "#define X 1\nint f(void){return X;}\n")
        .expect("test source must be writable");

    let tokens = clang_preprocessed_tokens_required(&path);
    fs::remove_file(&path).expect("test source must be removable");

    let kinds = tokens
        .iter()
        .map(|token| token.kind.as_str())
        .collect::<Vec<_>>();
    assert!(kinds.contains(&"int"));
    assert!(kinds.contains(&"identifier"));
    assert!(kinds.contains(&"numeric_constant"));

    let expanded = tokens
        .iter()
        .find(|token| token.kind == "numeric_constant" && token.spelling == "1")
        .expect("macro-expanded numeric token must be present");
    assert!(
        expanded.location.contains(":2:20"),
        "expanded token must report expansion location: {expanded:?}"
    );
    assert_eq!(expanded.source_location.line, Some(2));
    assert_eq!(expanded.source_location.column, Some(20));
    assert!(
        expanded
            .spelling_location
            .as_deref()
            .is_some_and(|loc| loc.contains(":1:11")),
        "expanded token must report macro spelling location: {expanded:?}"
    );
    assert_eq!(
        expanded
            .macro_spelling_location
            .as_ref()
            .and_then(|loc| loc.line),
        Some(1)
    );
    assert_eq!(
        expanded
            .macro_spelling_location
            .as_ref()
            .and_then(|loc| loc.column),
        Some(11)
    );
    assert!(expanded.has_leading_space);
}

#[test]
fn clang_token_oracle_records_include_origin() {
    let dir =
        std::env::temp_dir().join(format!("vyrec-clang-token-include-{}", std::process::id()));
    fs::create_dir_all(&dir).expect("test directory must be creatable");
    let header = dir.join("h.h");
    let source = dir.join("main.c");
    fs::write(&header, "static int from_header;\n").expect("header must be writable");
    fs::write(&source, "#include \"h.h\"\nint from_source;\n").expect("source must be writable");

    let tokens = clang_preprocessed_tokens_required(&source);
    fs::remove_file(&source).expect("source must be removable");
    fs::remove_file(&header).expect("header must be removable");
    fs::remove_dir(&dir).expect("test directory must be removable");

    let header_token = tokens
        .iter()
        .find(|token| token.kind == "identifier" && token.spelling == "from_header")
        .expect("included header identifier token must be present");
    assert!(
        header_token
            .include_origin
            .as_deref()
            .is_some_and(|origin| origin.ends_with("h.h")),
        "header token must record include origin: {header_token:?}"
    );

    let source_token = tokens
        .iter()
        .find(|token| token.kind == "identifier" && token.spelling == "from_source")
        .expect("main source identifier token must be present");
    assert!(
        source_token.include_origin.is_none(),
        "main-file token must not be classified as an include-origin token: {source_token:?}"
    );
}

#[test]
fn clang_token_oracle_records_diagnostic_mapping() {
    let path = std::env::temp_dir().join(format!(
        "vyrec-clang-token-diagnostic-{}.c",
        std::process::id()
    ));
    fs::write(&path, "#include \"missing.h\"\n").expect("test source must be writable");

    let facts = clang_preprocessed_token_facts(&path)
        .expect("clang diagnostics should be parseable even when source is invalid");
    fs::remove_file(&path).expect("test source must be removable");

    let diagnostic = facts
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.severity == "fatal error")
        .expect("missing include must produce a clang fatal-error diagnostic");
    assert!(
        diagnostic.message.contains("file not found"),
        "diagnostic message should identify the preprocessing failure: {diagnostic:?}"
    );
    assert_eq!(
        diagnostic.location.as_ref().and_then(|loc| loc.line),
        Some(1)
    );
    assert_eq!(
        diagnostic.location.as_ref().and_then(|loc| loc.column),
        Some(10)
    );
}
