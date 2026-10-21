//! Integration test crate for the containing Vyre package.

#![cfg(feature = "c-parser")]
//! Contracts for cached directive counts in classified C preprocessing tokens.

use std::sync::Arc;

use vyre_libs::parsing::c::preprocess::gpu_pipeline::ClassifiedTokens;

#[test]
fn classified_tokens_cache_directive_count_for_o1_hot_path_checks() {
    let classified = ClassifiedTokens::from_parts(
        vec![1, 2, 3, 4],
        vec![0, 1, 2, 3],
        vec![1, 1, 1, 1],
        vec![0, 7, 0, 9],
        Arc::from(b"abcd".as_slice()),
    );

    assert_eq!(classified.directive_count, 2);
    assert!(classified.has_directives());
    assert_eq!(
        classified.directive_rows().collect::<Vec<_>>(),
        vec![(1, 7), (3, 9)]
    );
}

#[test]
fn classified_tokens_directive_free_case_is_cached_without_rescanning() {
    let classified = ClassifiedTokens::from_parts(
        vec![1, 2, 3],
        vec![0, 1, 2],
        vec![1, 1, 1],
        vec![0, 0, 0],
        Arc::from(b"abc".as_slice()),
    );

    assert_eq!(classified.directive_count, 0);
    assert!(!classified.has_directives());
    assert_eq!(classified.directive_rows().next(), None);
}
