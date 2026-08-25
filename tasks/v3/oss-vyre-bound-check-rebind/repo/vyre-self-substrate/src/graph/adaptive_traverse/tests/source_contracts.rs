#[test]
fn matches_primitive_directly_by_wiring_release_programs() {
    let upload_source = include_str!("../upload.rs");
    let resident_source = include_str!("../resident_steps.rs");
    let release_path = format!("{upload_source}\n{resident_source}");

    for primitive_call in [
        "primitive_adaptive_sparse_dense_step(",
        "primitive_adaptive_four_russians_dense_step(",
        "primitive_four_russians_dense_lut_from_adj_rows(",
        "primitive_frontier_queue_len_init(",
        "primitive_frontier_words_to_queue_clear_out(",
        "primitive_frontier_word_counts(",
        "primitive_frontier_word_block_offsets(",
        "primitive_frontier_word_block_offsets_queue(",
        "primitive_frontier_word_prefix_queue(",
        "primitive_csr_queue_forward_traverse(",
        "primitive_csr_queue_split_low_forward_traverse(",
    ] {
        assert!(
            release_path.contains(primitive_call),
            "adaptive traversal release path must call primitive output wiring {primitive_call}"
        );
    }
}

#[test]
fn release_resident_paths_do_not_call_cpu_or_local_saturating_helpers() {
    let upload_source = include_str!("../upload.rs");
    let resident_source = include_str!("../resident_steps.rs");
    let release_path = format!("{upload_source}\n{resident_source}");

    assert!(!release_path.contains("reference_adaptive_sparse_dense_step("));
    assert!(!release_path.contains("cpu_sparse_dense_step("));
    assert!(!release_path.contains("saturating_mul"));
    assert!(!release_path.contains(concat!("checked_mul", "(std::mem::size_of::<u32>())")));
    assert!(release_path.contains("u32_word_bytes("));
    assert!(!release_path.contains(".div_ceil(256)"));
    assert!(release_path.contains("plan_adaptive_resident_frontier_step"));
    assert!(release_path.contains("plan_adaptive_resident_sparse_queue_step"));
    assert!(release_path.contains("plan_adaptive_resident_auto_step"));
}
