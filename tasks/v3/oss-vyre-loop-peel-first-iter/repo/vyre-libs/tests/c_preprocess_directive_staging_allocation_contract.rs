//! Source contracts for C GPU-preprocess directive staging allocation.

mod support;

#[test]
fn directive_staging_uses_checked_fallible_allocation_paths() {
    let directives = support::crate_file("src/parsing/c/preprocess/gpu_pipeline/directives.rs");
    support::assert_contains_all(
        &directives,
        &["fn directive_word_bytes(", "fn reserve_directive_vec<T>("],
        "directive extraction must centralize checked byte sizing and fallible reserve paths.",
    );
    support::assert_contains_all(
        &directives,
        &[
            "fn prepare_zero_init(&mut self, byte_len: usize) -> Result<(), String>",
            "try_reserve_exact(byte_len)",
        ],
        "directive zero-init staging must reserve fallibly before resize.",
    );
    support::assert_contains_all(
        &directives,
        &["u32::try_from(scratch.macro_names.len())"],
        "directive macro-name offsets must reject values outside the GPU u32 address space.",
    );
    support::assert_contains_none(
        &directives,
        &[
            "prepare_zero_init(n_pad * 4)",
            ".reserve((count + builtin_hashes.len()) * 4)",
            "Vec::with_capacity(defined_macros.len() + 1)",
            "scratch.macro_names.len() as u32",
            "fn directive_padded_u32_bytes(",
        ],
        "directive staging must not use dead macro-name padding, unchecked reserve, or offset arithmetic.",
    );
}
