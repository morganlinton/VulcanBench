//! Sparse lexer compact capacity contracts.

use std::fs;
use std::path::Path;

fn read(path: &str) -> String {
    fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(path))
        .unwrap_or_else(|error| panic!("failed to read {path}: {error}"))
}

fn stage_key_window<'a>(source: &'a str, stage: &str) -> &'a str {
    let mut search_from = 0;
    while let Some(relative) = source[search_from..].find(stage) {
        let start = search_from + relative;
        let prefix_start = start.saturating_sub(96);
        if source[prefix_start..start].contains("stage_pipeline_cache_key") {
            let end = source.len().min(start + 512);
            return &source[start..end];
        }
        search_from = start + stage.len();
    }
    panic!("stage cache key not found for {stage}");
}

#[test]
fn sparse_compact_capacity_is_token_count_driven_across_compaction_paths() {
    let compaction = read("src/pipeline/sparse_compaction.rs");
    let programs = read("src/pipeline/sparse_compaction/programs.rs");
    let megakernel = read("src/pipeline/sparse_lexer_megakernel.rs");
    let resident = read("src/pipeline/sparse_lexer_megakernel/resident_stages.rs");
    let collect = read("src/pipeline/sparse_lexer_megakernel/output_collect.rs");

    assert!(
        compaction.matches("compact_output_capacity_from_inclusive_offsets(").count() >= 3,
        "block-total and one-block sparse compaction must both size dense outputs from scanned inclusive offsets"
    );
    assert!(
        compaction.contains("c11_compact_sparse_tokens(")
            && compaction.contains("compact_capacity,"),
        "one-block sparse compaction must pass scanned token count capacity into dense output allocation"
    );

    assert!(
        programs.contains("pass_c_rescan_compact_sparse_tokens_with_capacity("),
        "block-total sparse compaction must expose a capacity-aware rescan compact builder"
    );
    assert_eq!(
        programs
            .matches(".with_count(bounded_output_capacity)")
            .count(),
        3,
        "capacity-aware block-total compaction must shrink all three dense token output columns"
    );
    assert!(
        programs.contains("Expr::saturating_sub(Expr::var(\"rank\"), Expr::u32(1))")
            && programs.contains("Expr::u32(bounded_output_capacity)"),
        "capacity-aware block-total compaction must guard dense output stores by compact capacity"
    );

    let block_total_key =
        stage_key_window(&megakernel, "\"syntax_sparse_block_total_stage_compact\"");
    assert!(block_total_key.contains("haystack_len as u64"));
    assert!(block_total_key.contains("num_blocks as u64"));
    assert!(
        block_total_key.contains("compact_capacity as u64"),
        "block-total sparse lexer compact cache key must include token-count capacity"
    );

    let staged_key = stage_key_window(&megakernel, "\"syntax_sparse_lexer_stage_compact\"");
    assert!(staged_key.contains("haystack_len as u64"));
    assert!(
        staged_key.contains("compact_capacity as u64"),
        "staged sparse lexer compact cache key must include token-count capacity"
    );

    let resident_compact_start = resident
        .find("\"syntax_sparse_lexer_stage_compact_resident\"")
        .expect("resident sparse lexer compact stage must exist");
    let resident_compact_end = resident.len().min(resident_compact_start + 2_500);
    let resident_compact = &resident[resident_compact_start..resident_compact_end];
    assert!(
        resident_compact.contains("dispatch_resident_stage_cached("),
        "resident sparse lexer compact must stay GPU-resident before exact ranged readback"
    );
    assert!(
        !resident_compact.contains("dispatch_resident_stage_readback_cached_into("),
        "resident sparse lexer compact must not full-readback source-length token columns"
    );
    assert!(
        resident_compact.contains("collect_resident_compact_lexer_output_exact_readback("),
        "resident sparse lexer compact must use count-first exact readback"
    );
    assert!(
        collect.contains("download_resident_range_into(&counts.resource, 0, 4")
            && collect.contains("download_resident_ranges_into(&ranges"),
        "resident exact compact collection must read count first, then only live token columns"
    );
}
