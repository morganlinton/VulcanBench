//! Source contracts for C GPU-preprocess prefix-scan staging allocation.

mod support;

#[test]
fn prefix_scan_staging_uses_checked_fallible_byte_sizing() {
    let scan = support::crate_file("src/parsing/c/preprocess/gpu_pipeline/scan.rs");
    support::assert_contains_all(
        &scan,
        &[
            "fn prefix_scan_word_bytes(",
            "fn prefix_scan_product_word_bytes(",
        ],
        "prefix scan staging must centralize checked word-to-byte sizing.",
    );
    support::assert_contains_all(
        &scan,
        &["fn prepare_zero(", ") -> Result<(), String> {"],
        "prefix scan zero staging must return typed errors.",
    );
    support::assert_contains_all(
        &scan,
        &["try_reserve_exact(byte_len)"],
        "prefix scan zero staging must reserve fallibly before resize.",
    );
    support::assert_contains_none(
        &scan,
        &[
            "n as usize * 4",
            "num_blocks as usize * 4",
            "total_partials as usize * 4",
        ],
        "prefix scan staging must not use unchecked u32-to-byte multiplication.",
    );
    support::assert_contains_none(
        &scan,
        &["saturating_mul(BLOCK_LANES)"],
        "prefix scan partial sizing must not silently saturate.",
    );
}
