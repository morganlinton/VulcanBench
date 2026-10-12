//! API-boundary regression tests for frontend reference-only helpers.

use std::fs;
use std::path::{Path, PathBuf};

const REFERENCE_FRONTEND_CALLS: &[&str] = &[
    "reference_prepare_translation_unit_source(",
    "reference_prepare_resident_translation_unit_source(",
    "reference_expand_preprocessor_macros(",
];

#[test]
fn production_source_does_not_call_reference_frontend_paths() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    collect_rust_files(&src, &mut files);

    let mut violations = Vec::new();
    for path in files {
        let text = fs::read_to_string(&path).expect("frontend source file must be readable");
        scan_source_file(&path, &text, &mut violations);
    }

    assert!(
        violations.is_empty(),
        "C frontend reference preprocessor calls are feature-gated parity surfaces only; \
         production must call `prepare_resident_translation_unit_source_gpu` or the full GPU pipeline.\n{}",
        violations.join("\n")
    );
}

#[test]
fn production_resident_preprocessor_rejects_mixed_macro_transport() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/tu_host/resident_prepare.rs");
    let text = fs::read_to_string(&path).expect("tu_host source file must be readable");
    let body = function_body(&text, "prepare_resident_translation_unit_source_gpu");
    assert!(
        body.contains("reject_mixed_macro_transport(options)?"),
        "production resident preprocessing must reject mixed ordered macro_actions and legacy macros/undefs before GPU dispatch"
    );
}

#[test]
fn legacy_macro_fields_are_used_only_when_ordered_actions_are_empty() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/tu_host/cli_macros.rs");
    let text = fs::read_to_string(&path).expect("tu_host source file must be readable");
    let body = function_body(&text, "cli_macro_actions");
    assert!(
        body.contains("if !options.macro_actions.is_empty()"),
        "cli_macro_actions must prefer ordered macro_actions over legacy unordered macro fields"
    );
    assert!(
        body.contains("return actions;"),
        "cli_macro_actions must not merge legacy macros/undefs after ordered macro_actions"
    );
}

fn collect_rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("frontend src directory must be readable") {
        let entry = entry.expect("frontend src entry must be readable");
        let path = entry.path();
        if path.is_dir() {
            collect_rust_files(&path, out);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

fn function_body<'a>(text: &'a str, name: &str) -> &'a str {
    let start = text
        .find(&format!("fn {name}"))
        .unwrap_or_else(|| panic!("{name} must exist"));
    let after_start = &text[start + "fn ".len() + name.len()..];
    let end = after_start
        .find("\npub ")
        .or_else(|| after_start.find("\npub(crate) "))
        .or_else(|| after_start.find("\nfn "))
        .unwrap_or(after_start.len());
    &after_start[..end]
}

fn scan_source_file(path: &Path, text: &str, violations: &mut Vec<String>) {
    let reference_module = path
        .components()
        .any(|component| component.as_os_str() == "preprocess");
    let mut pending_cfg_cpu_oracle = false;
    let mut cfg_cpu_oracle_depth: Option<i32> = None;
    for (idx, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("#[cfg(") && trimmed.contains("feature = \"cpu-oracle\"") {
            pending_cfg_cpu_oracle = true;
        }
        let is_reference_definition = REFERENCE_FRONTEND_CALLS.iter().any(|needle| {
            trimmed.starts_with("pub fn ") && trimmed.contains(needle.trim_end_matches('('))
        });
        let allowed = reference_module
            || pending_cfg_cpu_oracle
            || cfg_cpu_oracle_depth.is_some()
            || is_reference_definition;
        if !allowed
            && REFERENCE_FRONTEND_CALLS
                .iter()
                .any(|needle| line.contains(needle))
        {
            violations.push(format!("{}:{}: {}", path.display(), idx + 1, trimmed));
        }

        let delta = brace_delta(line);
        if pending_cfg_cpu_oracle
            && !trimmed.starts_with("#[")
            && !trimmed.is_empty()
            && !trimmed.starts_with("//")
        {
            if trimmed.ends_with(';') {
                pending_cfg_cpu_oracle = false;
            } else {
                cfg_cpu_oracle_depth = Some(delta);
                if delta <= 0 && line.contains('}') {
                    cfg_cpu_oracle_depth = None;
                }
                pending_cfg_cpu_oracle = false;
            }
        } else if let Some(depth) = cfg_cpu_oracle_depth.as_mut() {
            *depth += delta;
            if *depth <= 0 && line.contains('}') {
                cfg_cpu_oracle_depth = None;
            }
        }
    }
}

fn brace_delta(line: &str) -> i32 {
    let mut delta = 0i32;
    for ch in line.chars() {
        match ch {
            '{' => delta += 1,
            '}' => delta -= 1,
            _ => {}
        }
    }
    delta
}

#[test]
fn production_imacros_are_gpu_macro_imports_not_forced_includes() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let api = std::fs::read_to_string(manifest_dir.join("src/api/compile_options.rs"))
        .expect("vyre-frontend-c api/compile_options.rs must be readable");
    let resident_prepare =
        std::fs::read_to_string(manifest_dir.join("src/tu_host/resident_prepare.rs"))
            .expect("vyre-frontend-c tu_host/resident_prepare.rs must be readable");
    let resident_cache =
        std::fs::read_to_string(manifest_dir.join("src/tu_host/resident_cache.rs"))
            .expect("vyre-frontend-c tu_host/resident_cache.rs must be readable");
    assert!(api.contains("pub imacro_files: Vec<PathBuf>"));
    assert!(api.contains("discarded and only the live"));
    assert!(resident_prepare.contains("for imacro in &options.imacro_files"));
    assert!(resident_prepare.contains("gpu_preprocess_translation_unit("));
    assert!(resident_prepare.contains("active_macros = imacro_res.macros"));
    assert!(
        resident_cache.contains("hash_path_class(&mut hash, b\"imacros\", &options.imacro_files)")
    );
    let cli_macro_defs = resident_prepare
        .find("let mut active_macros: Vec<MacroDef> = cli_macro_defs(options)")
        .unwrap();
    let imacros = resident_prepare
        .find("for imacro in &options.imacro_files")
        .unwrap();
    let main_tu = resident_prepare.rfind("prefixed.as_bytes()").unwrap();
    assert!(cli_macro_defs < imacros);
    assert!(imacros < main_tu);
}

#[test]
fn production_include_search_keeps_quote_system_and_after_roots_distinct() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let api = std::fs::read_to_string(manifest_dir.join("src/api/compile_options.rs"))
        .expect("vyre-frontend-c api/compile_options.rs must be readable");
    let include_search =
        std::fs::read_to_string(manifest_dir.join("src/tu_host/include_search.rs"))
            .expect("vyre-frontend-c tu_host/include_search.rs must be readable");
    let resident_cache =
        std::fs::read_to_string(manifest_dir.join("src/tu_host/resident_cache.rs"))
            .expect("vyre-frontend-c tu_host/resident_cache.rs must be readable");
    assert!(api.contains("pub quote_include_dirs: Vec<PathBuf>"));
    assert!(api.contains("pub system_include_dirs: Vec<PathBuf>"));
    assert!(api.contains("pub after_include_dirs: Vec<PathBuf>"));
    assert!(include_search.contains("struct IncludeSearchDirs"));
    assert!(include_search.contains("quote_dirs"));
    assert!(include_search.contains("user_dirs"));
    assert!(include_search.contains("system_dirs"));
    assert!(include_search.contains("after_dirs"));
    assert!(resident_cache
        .contains("hash_path_class(&mut hash, b\"quote-include\", &options.quote_include_dirs)"));
    assert!(resident_cache
        .contains("hash_path_class(&mut hash, b\"system-include\", &options.system_include_dirs)"));
    assert!(resident_cache
        .contains("hash_path_class(&mut hash, b\"after-include\", &options.after_include_dirs)"));
}

#[test]
fn resident_prep_cache_domain_separates_include_path_classes() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let resident_cache =
        std::fs::read_to_string(manifest_dir.join("src/tu_host/resident_cache.rs"))
            .expect("vyre-frontend-c tu_host/resident_cache.rs must be readable");
    for required in [
        "hash_path_class(&mut hash, b\"include\"",
        "hash_path_class(&mut hash, b\"quote-include\"",
        "hash_path_class(&mut hash, b\"system-include\"",
        "hash_path_class(&mut hash, b\"after-include\"",
        "hash_path_class(&mut hash, b\"imacros\"",
        "hash_path_class(&mut hash, b\"forced-include\"",
        "fn hash_path_class",
    ] {
        assert!(
            resident_cache.contains(required),
            "resident prep cache identity must include `{required}`"
        );
    }
}

#[test]
fn production_system_includes_do_not_search_quote_only_roots() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let include_search =
        std::fs::read_to_string(manifest_dir.join("src/tu_host/include_search.rs"))
            .expect("vyre-frontend-c tu_host/include_search.rs must be readable");
    let body = function_body(&include_search, "search_system_include_file");
    assert!(body.contains("user_dirs"));
    assert!(body.contains("system_dirs"));
    assert!(body.contains("after_dirs"));
    assert!(
        !body.contains("quote_dirs"),
        "system include search must not consult -iquote roots"
    );
}

#[test]
fn macro_expansion_flush_uses_resident_dispatch_scratch() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let vyre_libs = manifest_dir.join("../vyre-libs/src/parsing/c/preprocess/gpu_pipeline");
    let flush = std::fs::read_to_string(vyre_libs.join("macro_expansion/flush.rs"))
        .expect("macro expansion flush source must be readable");
    let model = std::fs::read_to_string(vyre_libs.join("macro_expansion/model.rs"))
        .expect("macro expansion model source must be readable");
    let gpu_buffers = std::fs::read_to_string(vyre_libs.join("macro_expansion/gpu_buffers.rs"))
        .expect("macro expansion gpu buffer source must be readable");

    assert!(
        model.contains("struct MacroExpansionDispatchScratch"),
        "macro expansion cache must own reusable dispatch scratch"
    );
    assert!(
        model.contains("packed_macro_table_with_dispatch_scratch"),
        "macro expansion cache must return the packed macro table and dispatch scratch through one split-borrow API"
    );
    assert!(
        flush.contains("packed_macro_table_with_dispatch_scratch"),
        "macro expansion flush must use resident dispatch scratch instead of building one-shot inputs"
    );
    for forbidden in [
        "let mut owned_inputs = vec!",
        "let mut input_refs = vec!",
        "bytes_to_u32_word_bytes(",
        "pad_u32_byte_buffer(",
        "pack_u32_words(&classified.tok_types",
        "pack_u32_words(&classified.tok_starts",
        "pack_u32_words(&classified.tok_lens",
        ".flat_map(u32::to_le_bytes)",
    ] {
        assert!(
            !flush.contains(forbidden),
            "macro expansion flush must not rebuild dispatch allocation pattern `{forbidden}`"
        );
    }
    assert!(
        gpu_buffers.contains("bytes_to_u32_word_bytes_into")
            && gpu_buffers.contains("pad_u32_byte_buffer_into"),
        "macro expansion GPU buffer helpers must expose in-place packers"
    );
    assert!(
        !gpu_buffers.contains("fn bytes_to_u32_word_bytes(")
            && !gpu_buffers.contains("fn pad_u32_byte_buffer("),
        "allocation-returning macro buffer packers must stay removed"
    );
}
