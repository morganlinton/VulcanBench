// Integration test module for the containing Vyre package.

#![allow(clippy::too_many_arguments)]

use super::object::{PG_STRIDE_U32, VAST_STRIDE_U32};

pub(crate) fn token_text<'a>(source: &'a str, starts: &[u32], lens: &[u32], idx: usize) -> &'a str {
    let start = starts[idx] as usize;
    let end = start + lens[idx] as usize;
    &source[start..end]
}

pub(crate) fn find_token_after(
    source: &str,
    starts: &[u32],
    lens: &[u32],
    text: &str,
    after_idx: usize,
) -> usize {
    ((after_idx + 1)..starts.len())
        .find(|&idx| token_text(source, starts, lens, idx) == text)
        .unwrap_or_else(|| panic!("token {text:?} exists after index {after_idx}"))
}

pub(crate) fn find_token(source: &str, starts: &[u32], lens: &[u32], text: &str) -> usize {
    (0..starts.len())
        .find(|&idx| token_text(source, starts, lens, idx) == text)
        .unwrap_or_else(|| panic!("token {text:?} exists"))
}

pub(crate) fn find_kind(tok_types: &[u32], kind: u32) -> usize {
    tok_types
        .iter()
        .position(|&tok| tok == kind)
        .unwrap_or_else(|| {
            panic!(
                "token kind {kind} exists; first token kinds: {:?}",
                &tok_types[..tok_types.len().min(80)]
            )
        })
}

pub(crate) fn find_kind_after(tok_types: &[u32], kind: u32, after_idx: usize) -> usize {
    ((after_idx + 1)..tok_types.len())
        .find(|&idx| tok_types[idx] == kind)
        .unwrap_or_else(|| panic!("token kind {kind} exists after index {after_idx}"))
}

pub(crate) fn find_kind_before(tok_types: &[u32], kind: u32, before_idx: usize) -> usize {
    (0..before_idx)
        .rev()
        .find(|&idx| tok_types[idx] == kind)
        .unwrap_or_else(|| panic!("token kind {kind} exists before index {before_idx}"))
}

pub(crate) fn find_token_before(
    source: &str,
    starts: &[u32],
    lens: &[u32],
    text: &str,
    before_idx: usize,
) -> usize {
    (0..before_idx)
        .rev()
        .find(|&idx| token_text(source, starts, lens, idx) == text)
        .unwrap_or_else(|| panic!("token {text:?} exists before index {before_idx}"))
}

pub(crate) fn find_token_in_context(
    source: &str,
    tok_types: &[u32],
    starts: &[u32],
    lens: &[u32],
    token_type: u32,
    context: &str,
    text: &str,
) -> usize {
    let context_start = source
        .find(context)
        .unwrap_or_else(|| panic!("source contains context {context:?}"));
    let token_start = context_start
        + context
            .find(text)
            .unwrap_or_else(|| panic!("context {context:?} contains token {text:?}"));

    tok_types
        .iter()
        .enumerate()
        .find_map(|(idx, &kind)| {
            let start = starts[idx] as usize;
            let len = lens[idx] as usize;
            (kind == token_type && start == token_start && len == text.len()).then_some(idx)
        })
        .unwrap_or_else(|| panic!("lex section contains token {text:?} at byte {token_start}"))
}

pub(crate) fn vast_kind(words: &[u32], idx: usize) -> u32 {
    words[idx * VAST_STRIDE_U32]
}

pub(crate) fn assert_token_kind(
    source: &str,
    starts: &[u32],
    lens: &[u32],
    vast_words: &[u32],
    token_idx: usize,
    expected: u32,
    label: &str,
) {
    assert_eq!(
        vast_kind(vast_words, token_idx),
        expected,
        "{label}: token {:?} at index {token_idx}",
        token_text(source, starts, lens, token_idx)
    );
}

pub(crate) fn assert_vast_kind_and_span(
    source: &str,
    starts: &[u32],
    lens: &[u32],
    vast_words: &[u32],
    token_idx: usize,
    expected: u32,
    expected_text: &str,
    label: &str,
) {
    assert_token_kind(source, starts, lens, vast_words, token_idx, expected, label);
    let base = token_idx * VAST_STRIDE_U32;
    assert_eq!(
        vast_words[base + 5],
        starts[token_idx],
        "{label}: VAST span_start mirrors lex start"
    );
    assert_eq!(
        vast_words[base + 6],
        lens[token_idx],
        "{label}: VAST span_len mirrors lex len"
    );
    assert_eq!(
        token_text(source, starts, lens, token_idx),
        expected_text,
        "{label}: VAST span points at requested token"
    );
}

pub(crate) fn assert_typed_vast_and_pg_rows(
    tok_types: &[u32],
    starts: &[u32],
    lens: &[u32],
    vast_words: &[u32],
    pg_words: &[u32],
) {
    assert_eq!(vast_words.len(), tok_types.len() * VAST_STRIDE_U32);
    assert_eq!(pg_words.len(), tok_types.len() * PG_STRIDE_U32);
    for idx in 0..tok_types.len() {
        let vast_base = idx * VAST_STRIDE_U32;
        let pg_base = idx * PG_STRIDE_U32;
        assert_eq!(vast_words[vast_base + 5], starts[idx]);
        assert_eq!(vast_words[vast_base + 6], lens[idx]);
        assert_eq!(pg_words[pg_base], vast_words[vast_base]);
        assert_eq!(pg_words[pg_base + 1], starts[idx]);
        assert_eq!(pg_words[pg_base + 2], starts[idx] + lens[idx]);
    }
    assert!(
        vast_words
            .chunks_exact(VAST_STRIDE_U32)
            .enumerate()
            .any(|(idx, row)| row[0] != tok_types[idx]),
        "compiled VAST section contains typed AST node kinds rather than raw token kinds only"
    );
}
