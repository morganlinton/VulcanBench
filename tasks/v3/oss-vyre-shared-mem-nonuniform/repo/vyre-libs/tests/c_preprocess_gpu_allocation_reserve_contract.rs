//! Source contracts for checked allocation in GPU C-preprocessor hot paths.

#[test]
fn gpu_preprocess_hot_paths_use_checked_reservation() {
    let macro_values = include_str!("../src/parsing/c/preprocess/gpu_pipeline/macro_values.rs");
    assert!(
        macro_values.contains(
            "pub(super) fn macro_integer_values(macros: &[MacroDef]) -> Result<Vec<u32>, String>"
        ),
        "macro integer folding must propagate allocation errors instead of panicking"
    );
    assert!(
        macro_values.contains("collect_macro_body_identifiers(")
            && macro_values.contains("-> Result<(), String>"),
        "macro dependency collection must propagate allocation errors from edge storage"
    );
    assert!(
        !macro_values.contains("Vec::with_capacity(macros.len())")
            && !macro_values.contains("vec![0u32; macros.len()]")
            && !macro_values.contains("vec![Vec::new(); macros.len()]"),
        "macro integer folding must reserve checked storage before resizing"
    );

    let conditional_eval =
        include_str!("../src/parsing/c/preprocess/gpu_pipeline/conditional_eval.rs");
    assert!(
        conditional_eval.contains("Result<Option<bool>, String>")
            && conditional_eval.contains("try_reserve(macros.len())"),
        "conditional truth fast path must report oversized macro truth tables"
    );
    assert!(
        !conditional_eval.contains("bodies.reserve(macros.len())"),
        "conditional truth table must not use infallible reserve"
    );

    let driver = include_str!("../src/parsing/c/preprocess/gpu_pipeline/driver.rs");
    assert!(
        driver.contains("PreprocessRun::try_new")
            && driver.contains("try_reserve_exact(cli_macros.len())")
            && driver.contains("macro_index.try_reserve(cli_macros.len())"),
        "translation-unit setup must check CLI macro allocation"
    );
    assert!(
        !driver.contains("macro_index.reserve(cli_macros.len())"),
        "translation-unit setup must not use infallible macro-index reserve"
    );

    let expansion_events =
        include_str!("../src/parsing/c/preprocess/gpu_pipeline/expansion_events.rs");
    assert!(
        expansion_events.contains("macros_by_name.try_reserve(macros.len())")
            && expansion_events.contains("macro_expansion_events")
            && expansion_events.contains(".try_reserve(candidate_macros.len())"),
        "macro expansion evidence must check index and event allocation"
    );
    assert!(
        !expansion_events.contains("macros_by_name.reserve(macros.len())"),
        "macro expansion evidence must not use infallible reserve"
    );

    let segments = include_str!("../src/parsing/c/preprocess/gpu_pipeline/segments.rs");
    assert!(
        segments.contains("macro_lookup.try_reserve(macros.len())")
            && segments.contains("self.by_name.try_reserve(macros.len())")
            && segments.contains("self.refresh(macros)?"),
        "macro segment lookup refresh must be fallible and checked"
    );
    assert!(
        !segments.contains("macro_lookup.reserve(macros.len())")
            && !segments.contains("self.by_name.reserve(macros.len())"),
        "macro segment lookup must not use infallible reserve"
    );

    let segment_ranges =
        include_str!("../src/parsing/c/preprocess/gpu_pipeline/macro_expansion/segment_ranges.rs");
    assert!(
        segment_ranges.contains("chunk.try_reserve_exact(additional)"),
        "macro segment range scratch growth must be checked"
    );
    assert!(
        !segment_ranges.contains("chunk.reserve("),
        "macro segment range scratch must not use infallible reserve"
    );

    let flush = include_str!("../src/parsing/c/preprocess/gpu_pipeline/macro_expansion/flush.rs");
    assert!(
        flush.contains("output.try_reserve_exact(source_count)"),
        "macro expansion materialization must check output growth"
    );
    assert!(
        !flush.contains("output.reserve(source_count)"),
        "macro expansion materialization must not use infallible reserve"
    );
}
