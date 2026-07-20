//! Source contracts for C GPU-preprocess macro-table staging allocation.

mod support;

use support::{assert_contains_all, assert_contains_none, crate_file};

#[test]
fn macro_table_staging_uses_checked_fallible_allocation_paths() {
    let macro_table = crate_file("src/parsing/c/preprocess/gpu_pipeline/macro_table.rs");
    assert_contains_all(
        &macro_table,
        &[
            "fn reserve_macro_table_vec<T>(",
            "fn filled_macro_table_vec<T: Clone>(",
            "fn padded_macro_table_byte_len(",
        ],
        "macro table packing must centralize fallible reservation and checked padding",
    );
    assert_contains_all(
        &macro_table,
        &[
            "seen_names.try_reserve(macros.len())",
            "indexes.try_reserve(params.len())",
        ],
        "macro table dedupe and parameter indexes must reserve fallibly",
    );
    assert_contains_all(
        &macro_table,
        &["let total_name_bytes = macros.iter().try_fold"],
        "macro table name staging must precompute byte totals with overflow checks",
    );
    assert_contains_all(
        &macro_table,
        &[
            "let name_byte_slots = macros",
            "let replacement_byte_slots = macros",
            "reserve_macro_table_vec(\n            &mut pending,",
        ],
        "macro expansion table packing must precompute and reserve expansion table storage",
    );
    assert_contains_none(
        &macro_table,
        &[
            "seen_names.reserve(macros.len())",
            "Vec::with_capacity(macros.len() + 1)",
            "Vec::with_capacity(macros.len())",
            "vec![EMPTY_MACRO_SLOT; slots]",
            "vec![false; slots]",
            "let mut names_padded = vec![0u8; names_target]",
            "indexes.reserve(params.len())",
        ],
        "macro table packing must not use unchecked reserve or padded allocation paths",
    );
}
