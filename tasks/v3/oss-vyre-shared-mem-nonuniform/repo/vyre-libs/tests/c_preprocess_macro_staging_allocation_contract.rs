//! Source contracts for macro-expansion GPU staging allocation hardening.

use std::fs;
use std::path::PathBuf;

fn src_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative)
}

fn read_src(relative: &str) -> String {
    fs::read_to_string(src_path(relative)).unwrap_or_else(|err| {
        panic!("failed to read {relative}: {err}");
    })
}

#[test]
fn macro_expansion_staging_uses_fallible_checked_capacity_paths() {
    let gpu_buffers =
        read_src("src/parsing/c/preprocess/gpu_pipeline/macro_expansion/gpu_buffers.rs");
    assert!(
        gpu_buffers.contains("pub(crate) fn checked_staging_word_bytes("),
        "word-count to byte-count conversion must be centralized and checked"
    );
    assert!(
        gpu_buffers.contains("pub(crate) fn reserve_staging_bytes("),
        "GPU staging vectors must reserve through the shared fallible helper"
    );
    assert!(
        gpu_buffers.contains("try_reserve_exact("),
        "staging reservation must report allocation failure instead of panicking"
    );
    assert!(
        !gpu_buffers.contains("bytes_to_u32_word_bytes_into(")
            && !gpu_buffers.contains("pad_u32_byte_buffer_into("),
        "macro expansion input byte arenas must stay raw U8 instead of staging per-byte U32 words"
    );
    assert!(
        !gpu_buffers.contains(".saturating_mul(4)"),
        "staging byte sizes must not silently saturate"
    );
    assert!(
        !gpu_buffers.contains(".reserve(byte_len)"),
        "staging code must not use infallible reserve on wire-sized buffers"
    );
}

#[test]
fn macro_expansion_scratch_zeroing_reserves_fallibly() {
    let model = read_src("src/parsing/c/preprocess/gpu_pipeline/macro_expansion/model.rs");
    assert!(
        model.contains("fn write_zero_bytes(") && model.contains(") -> Result<(), String> {"),
        "scratch zero-fill staging must report allocation failure"
    );
    assert!(
        model.contains("reserve_staging_bytes("),
        "scratch zero-fill staging must use the shared fallible reserve helper"
    );
}

#[test]
fn macro_expansion_flush_uses_checked_staging_sizes() {
    let flush = read_src("src/parsing/c/preprocess/gpu_pipeline/macro_expansion/flush.rs");
    assert!(
        flush.contains("checked_staging_word_bytes("),
        "flush output staging sizes must use checked word-to-byte conversion"
    );
    assert!(
        flush.contains("write_zero_bytes(") && flush.contains(")?;"),
        "flush zero-output staging must propagate scratch allocation failures"
    );
    assert!(
        !flush.contains("bytes_to_u32_word_bytes_into(")
            && !flush.contains("pad_u32_byte_buffer_into("),
        "flush must dispatch raw macro source/replacement bytes without per-byte U32 staging"
    );
    assert!(
        !flush.contains("max_out_tokens as usize * 4"),
        "token output byte sizing must not use unchecked multiplication"
    );
    assert!(
        !flush.contains("max_out_source_bytes as usize * 4"),
        "source output byte sizing must not use unchecked multiplication"
    );
    assert!(
        !flush.contains("token_count_bucket * 4"),
        "bucket output byte sizing must not use unchecked multiplication"
    );
}
