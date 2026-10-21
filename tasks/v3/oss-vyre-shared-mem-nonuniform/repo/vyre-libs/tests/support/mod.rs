use std::fs;
use std::path::{Path, PathBuf};

#[cfg(feature = "c-parser")]
pub(crate) mod gpu_if_expression;
#[cfg(feature = "c-parser")]
pub(crate) mod gpu_pipeline_filter;

pub(crate) fn crate_file(path: &str) -> String {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    fs::read_to_string(manifest.join(path)).unwrap_or_else(|error| {
        panic!("failed to read {path}: {error}");
    })
}

pub(crate) fn assert_byte_lru_core_tracks_resident_bytes() {
    let byte_lru_cache = crate_file("src/parsing/c/preprocess/gpu_pipeline/byte_lru_cache.rs");
    assert!(
        byte_lru_cache.contains("bytes: usize") && byte_lru_cache.contains("max_bytes: usize"),
        "Fix: shared GPU preprocessor cache core must track resident bytes and byte limit."
    );
}

pub(crate) fn assert_byte_lru_core_rejects_and_accounts() {
    let byte_lru_cache = crate_file("src/parsing/c/preprocess/gpu_pipeline/byte_lru_cache.rs");
    // Insert overflow is detected with `checked_add` (saturating to usize::MAX
    // forces eviction); the per-entry decrement on remove uses `saturating_sub`
    // so an impossible-by-construction underflow cannot panic. Both are the
    // correct, consistent accounting idioms — assert the decrement by its real
    // form so this contract does not regress to a stale string on idiom changes.
    assert!(
        byte_lru_cache.contains("entry_bytes > self.max_bytes")
            && byte_lru_cache.contains(".checked_add(entry_bytes)")
            && byte_lru_cache.contains("saturating_sub(entry.bytes)"),
        "Fix: shared GPU preprocessor cache core must reject oversized entries, evict to byte budget, and update byte accounting."
    );
}

pub(crate) fn assert_contains_all(source: &str, needles: &[&str], message: &str) {
    let missing = needles
        .iter()
        .copied()
        .filter(|needle| !source.contains(needle))
        .collect::<Vec<_>>();
    assert!(
        missing.is_empty(),
        "{message} Missing required source fragment(s): {}",
        missing.join(" | ")
    );
}

pub(crate) fn assert_contains_none(source: &str, needles: &[&str], message: &str) {
    let present = needles
        .iter()
        .copied()
        .filter(|needle| source.contains(needle))
        .collect::<Vec<_>>();
    assert!(
        present.is_empty(),
        "{message} Forbidden source fragment(s): {}",
        present.join(" | ")
    );
}

pub(crate) fn assert_no_cpu_named_api_exports(
    relative_root: &str,
    read_context: &str,
    extra_trait_markers: &[&str],
    failure_message: &str,
) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative_root);
    let mut files = Vec::new();
    collect_rs_files(&root, read_context, &mut files);

    let mut offenders = Vec::new();
    for path in files {
        let source = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("read {read_context} source file: {error}"));
        for (line_idx, line) in source.lines().enumerate() {
            if is_cpu_named_api_export(line, extra_trait_markers) {
                offenders.push(format!("{}:{}: {line}", path.display(), line_idx + 1));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "{failure_message}:\n{}",
        offenders.join("\n")
    );
}

fn collect_rs_files(dir: &Path, read_context: &str, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("read {read_context} source directory: {error}"))
    {
        let entry =
            entry.unwrap_or_else(|error| panic!("read {read_context} source entry: {error}"));
        let path = entry.path();
        if path.is_dir() {
            collect_rs_files(&path, read_context, files);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            files.push(path);
        }
    }
}

fn is_cpu_named_api_export(line: &str, extra_trait_markers: &[&str]) -> bool {
    let has_cpu_name = line.contains("_cpu") || line.contains("cpu_");
    let public_cpu_fn = line.contains("pub fn ") && has_cpu_name;
    let public_cpu_reexport = line.contains("pub use ") && has_cpu_name;
    let trait_marker = line.trim_start().starts_with("fn ")
        && extra_trait_markers
            .iter()
            .any(|marker| line.contains(marker));

    public_cpu_fn || public_cpu_reexport || trait_marker
}
