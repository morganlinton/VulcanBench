//! Source contracts for C GPU-preprocess token provenance allocation.

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
fn token_provenance_uses_shared_fallible_allocation_paths() {
    let root = read_src("src/parsing/c/preprocess/gpu_pipeline/token_provenance.rs");
    let direct = read_src("src/parsing/c/preprocess/gpu_pipeline/token_provenance/direct.rs");
    let parameter = read_src(
        "src/parsing/c/preprocess/gpu_pipeline/token_provenance/parameter_substitution.rs",
    );
    let object =
        read_src("src/parsing/c/preprocess/gpu_pipeline/token_provenance/object_backfill.rs");
    let missing =
        read_src("src/parsing/c/preprocess/gpu_pipeline/token_provenance/missing_invocation.rs");
    let macro_record =
        read_src("src/parsing/c/preprocess/gpu_pipeline/token_provenance/macro_record.rs");
    let span_dedupe =
        read_src("src/parsing/c/preprocess/gpu_pipeline/token_provenance/span_dedupe.rs");

    assert!(
        root.contains("fn reserve_token_provenance_events(")
            && root.contains("try_reserve_exact(additional)"),
        "token provenance event staging must use the shared fallible reserve helper"
    );
    assert!(
        direct.contains("reserve_token_provenance_events(")
            && parameter.contains("reserve_token_provenance_events(")
            && object.contains("reserve_token_provenance_events(")
            && missing.contains("reserve_token_provenance_events("),
        "all token-provenance event producers must use the shared reserve helper"
    );
    assert!(
        macro_record.contains("macros_by_name.try_reserve(macros.len())"),
        "macro provenance lookup buckets must reserve fallibly"
    );
    assert!(
        span_dedupe.contains("pub(crate) fn try_from_iter(")
            && span_dedupe.contains("pub(crate) fn insert(&mut self, span: (T, T)) -> Result<bool, String>")
            && span_dedupe.contains("overflow.try_reserve(reserve_slots)"),
        "span dedupe must report allocation failure instead of panicking when it spills out of the inline buffer"
    );
    assert!(
        !direct.contains("token_provenance_events.reserve(")
            && !parameter.contains("token_provenance_events.reserve(")
            && !object.contains("token_provenance_events.reserve(")
            && !missing.contains("token_provenance_events.reserve(")
            && !macro_record.contains("macros_by_name.reserve(")
            && !span_dedupe.contains("overflow.reserve(")
            && !span_dedupe.contains("SpanDedupe::from_iter"),
        "token provenance must not reintroduce infallible reserve or infallible span-dedupe construction"
    );
}
