//! Source contracts for shared C GPU-preprocess buffer staging helpers.

mod support;

use support::{assert_contains_all, assert_contains_none, crate_file};

#[test]
fn shared_gpu_buffer_pack_helpers_are_checked_fallible_and_padding_free() {
    let buffers = crate_file("src/parsing/c/preprocess/gpu_pipeline/buffers.rs");
    assert_contains_all(
        &buffers,
        &[
            "pub(super) fn u32_word_byte_len(",
            "pub(super) fn reserve_gpu_staging_bytes(",
        ],
        "shared GPU staging helpers must centralize checked sizing and fallible reserve",
    );
    assert_contains_all(
        &buffers,
        &[
            "pub(super) fn unpack_u32_words_prefix(bytes: &[u8], count: usize) -> Result<Vec<u32>, String>",
            "could not reserve {take} prefix u32 decode words",
        ],
        "prefix u32 unpacking must reserve fallibly and propagate decode errors"
    );
    assert_contains_none(
        &buffers,
        &[
            "Vec::with_capacity(take)",
            "clamped prefix u32 unpack must not fail",
        ],
        "prefix u32 unpacking must not keep unchecked legacy allocation paths",
    );
    assert_contains_all(
        &buffers,
        &[
            "pub(super) fn pack_u32_words(words: &[u32], pad_len: usize) -> Result<Vec<u8>, String>",
            "pub(super) fn pack_u32_words_into(",
            ") -> Result<(), String>",
        ],
        "u32 table packing must return Result instead of silently saturating or panicking"
    );
    assert_contains_all(
        &buffers,
        &["try_reserve_exact(additional)"],
        "shared GPU staging helpers must not use saturating byte counts or infallible reserve",
    );
    assert_contains_none(
        &buffers,
        &[
            "saturating_mul(4)",
            ".reserve(target)",
            ".reserve(min_words",
            "pub(super) fn padded_u32_byte_len(",
            "pub(super) fn pad_to_u32_words(",
        ],
        "shared GPU staging helpers must not keep dead byte-padding paths, saturating byte counts, or infallible reserve",
    );
}
