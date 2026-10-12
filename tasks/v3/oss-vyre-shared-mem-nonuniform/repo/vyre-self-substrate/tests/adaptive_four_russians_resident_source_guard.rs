#[test]
fn four_russians_resident_path_remains_a_primitive_wrapper() {
    let upload_source = include_str!("../src/graph/adaptive_traverse/upload.rs");
    let resident_source = include_str!("../src/graph/adaptive_traverse/resident_steps.rs");
    let release_path = format!("{upload_source}\n{resident_source}");

    for required in [
        "primitive_four_russians_dense_lut_from_adj_rows(",
        "primitive_adaptive_four_russians_dense_step(",
        "FourRussiansDense",
        "four_russians_tile_lut",
        "resident_sequence_single_u32_output_into(",
    ] {
        assert!(
            release_path.contains(required),
            "Fix: resident Four-Russians dense traversal must wire {required}."
        );
    }

    for forbidden in [
        "for neighbor",
        "while let Some",
        "reference_",
        "cpu_sparse_dense",
        "HashMap<AdaptiveTraversalPlanKey",
    ] {
        assert!(
            !release_path.contains(forbidden),
            "Fix: self-substrate must not fork primitive traversal logic through {forbidden}."
        );
    }
}

#[test]
fn resident_four_russians_path_reuses_shared_cache_and_buffers() {
    let upload_source = include_str!("../src/graph/adaptive_traverse/upload.rs");
    let resident_source = include_str!("../src/graph/adaptive_traverse/resident_steps.rs");
    let release_path = format!("{upload_source}\n{resident_source}");

    assert!(
        release_path.contains("scratch.plan_cache.get_or_build("),
        "Fix: resident Four-Russians dense traversal must reuse cached programs."
    );
    assert!(
        release_path.contains("ensure_frontier_handles("),
        "Fix: resident Four-Russians dense traversal must reuse resident frontier scratch."
    );
    assert!(
        !release_path.contains("vec![0; bitset_words"),
        "Fix: resident LUT upload must not allocate a fake frontier to derive layout."
    );
}
