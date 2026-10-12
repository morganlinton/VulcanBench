//! Integration contract for vyre-frontend-c.

use std::fs;

use vyre_frontend_c::api::{compile, VyreCompileOptions};

#[test]
fn default_link_mode_does_not_spawn_host_c_linker() {
    let src = std::env::temp_dir().join(format!(
        "vyre_frontend_c_no_host_link_{}.c",
        std::process::id()
    ));
    fs::write(&src, "int main(void) { return 0; }\n").expect("write C source fixture");

    let mut options = VyreCompileOptions::default();
    options.is_compile_only = false;
    options.input_files = vec![src.clone()];
    let err =
        compile(options).expect_err("default CUDA-first surface must reject host linker mode");

    let _ = fs::remove_file(&src);
    assert!(
        err.contains("link mode is not part of the CUDA-first release path"),
        "error must reject the removed host-linker surface, got: {err}"
    );
    assert!(
        err.contains("does not spawn a host C linker"),
        "error must preserve the no-host-linker contract, got: {err}"
    );
}

#[test]
fn resident_sparse_lexer_terminal_readback_is_count_first_and_caller_owned() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let resident_dispatch =
        fs::read_to_string(manifest_dir.join("src/pipeline/backend_select/resident_dispatch.rs"))
            .expect("resident dispatch source must be readable");
    let resident_stages = fs::read_to_string(
        manifest_dir.join("src/pipeline/sparse_lexer_megakernel/resident_stages.rs"),
    )
    .expect("sparse lexer resident stages source must be readable");
    let sparse_lexer =
        fs::read_to_string(manifest_dir.join("src/pipeline/sparse_lexer_megakernel.rs"))
            .expect("sparse lexer megakernel source must be readable");
    let output_collect = fs::read_to_string(
        manifest_dir.join("src/pipeline/sparse_lexer_megakernel/output_collect.rs"),
    )
    .expect("sparse lexer output collector source must be readable");

    assert!(
        !resident_dispatch.contains("fn dispatch_resident_stage_readback_cached<"),
        "resident terminal readback must not expose the allocation-returning helper; use dispatch_resident_stage_readback_cached_into with caller-owned scratch"
    );
    assert!(
        resident_dispatch.contains("fn dispatch_resident_stage_readback_cached_into<"),
        "resident terminal readback must expose the caller-owned _into API"
    );
    assert!(
        sparse_lexer.contains("resident_compact_outputs: Vec<Vec<u8>>"),
        "sparse lexer scratch must own resident compact readback output slots"
    );
    assert!(
        sparse_lexer.contains("resident_count_readback: Vec<u8>"),
        "sparse lexer scratch must own the resident compact count readback slot"
    );
    assert!(
        resident_stages.contains("dispatch_resident_stage_cached(")
            && resident_stages.contains("collect_resident_compact_lexer_output_exact_readback(")
            && resident_stages.contains("&mut scratch.resident_compact_outputs")
            && resident_stages.contains("&mut scratch.resident_count_readback"),
        "resident sparse lexer terminal stage must keep compact outputs resident, then read exact ranges into SparseLexerMegakernelScratch"
    );
    assert!(
        output_collect.contains("download_resident_range_into(&counts.resource, 0, 4")
            && output_collect.contains("download_resident_ranges_into(&ranges"),
        "resident sparse lexer exact readback must read out_counts first, then only the live dense token ranges"
    );
}

#[test]
fn lexer_cache_preserves_promoted_types_and_cuda_packed_haystack() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let semantic_parse = fs::read_to_string(manifest_dir.join("src/pipeline/semantic_parse.rs"))
        .expect("semantic parse source must be readable");

    assert!(
        semantic_parse.contains("keyword_promoted: true"),
        "warm lexer-cache entries must store post-keyword token types so repeat parses skip keyword promotion"
    );
    assert!(
        semantic_parse.contains("cuda_keyword_haystack: cuda_keyword_haystack")
            && semantic_parse.contains("std::sync::Arc::clone(bytes)"),
        "warm lexer-cache entries must retain the CUDA-packed haystack so semantic stages do not host-repack after cache hits"
    );
}
