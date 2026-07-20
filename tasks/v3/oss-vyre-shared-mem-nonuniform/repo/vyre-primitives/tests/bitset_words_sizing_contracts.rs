//! Bitset word-count sizing contracts.
#![cfg(feature = "bitset")]

use vyre_primitives::bitset::bitset_words;

#[test]
fn bitset_words_zero_elements() {
    assert_eq!(bitset_words(0), 0);
}

#[test]
fn bitset_words_one_element() {
    assert_eq!(bitset_words(1), 1);
}

#[test]
fn bitset_words_exact_word_boundary() {
    assert_eq!(bitset_words(32), 1);
    assert_eq!(bitset_words(64), 2);
    assert_eq!(bitset_words(128), 4);
}

#[test]
fn bitset_words_just_over_boundary() {
    assert_eq!(bitset_words(33), 2);
    assert_eq!(bitset_words(65), 3);
    assert_eq!(bitset_words(97), 4);
}

#[test]
fn bitset_words_just_under_boundary() {
    assert_eq!(bitset_words(31), 1);
    assert_eq!(bitset_words(63), 2);
    assert_eq!(bitset_words(127), 4);
}

#[test]
fn bitset_words_large_values() {
    assert_eq!(bitset_words(1024), 32);
    assert_eq!(bitset_words(1025), 33);
    assert_eq!(bitset_words(1_000_000), 31_250);
}

#[test]
fn bitset_words_u32_max() {
    assert_eq!(bitset_words(u32::MAX), u32::MAX.div_ceil(32));
    assert_eq!(bitset_words(u32::MAX), 134_217_728);
}
