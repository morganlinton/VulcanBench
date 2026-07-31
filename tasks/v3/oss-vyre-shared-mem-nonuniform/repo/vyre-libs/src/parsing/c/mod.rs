//! C11 pipeline modules  -  lex / preprocess / parse / pipeline.

/// DFA lexer pipeline (lexer, tokens, keywords).
pub mod lex;
/// Lowering from structural parse to packed graph (PG) nodes.
pub mod lower;
/// Structural parser.
pub mod parse;
/// End-to-end example Programs for the C11 pipeline.
pub mod pipeline;
/// Preprocessor expansion.
pub mod preprocess;
/// Semantic analysis of C structures and declarations.
pub mod sema;
/// Source byte addressing helpers shared by expanded and packed GPU haystacks.
pub mod source_bytes;

#[cfg(test)]
mod architecture_tests {
    use std::path::{Path, PathBuf};

    fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in
            std::fs::read_dir(dir).expect("Fix: C frontend source directory must be readable")
        {
            let entry = entry.expect("Fix: C frontend source directory entries must be readable");
            let path = entry.path();
            if path.is_dir() {
                collect_rs_files(&path, out);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                out.push(path);
            }
        }
    }

    fn is_oracle_or_test_file(path: &Path) -> bool {
        let path_text = path.to_string_lossy();
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        path_text.contains("/tests/")
            || path_text.contains("/test_support/")
            || path_text.contains("/witness")
            || path_text.contains("/ref_")
            || file_name.contains("test")
            || file_name == "mod.rs"
            || file_name == "reference.rs"
            || file_name.starts_with("ref_")
            || file_name.contains("_reference")
    }

    fn uncommented_source(source: &str) -> String {
        source
            .lines()
            .filter_map(|line| {
                line.split_once("//")
                    .map_or(Some(line), |(code, _)| Some(code))
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn production_only_source(source: &str) -> String {
        let mut out = String::with_capacity(source.len());
        let mut pending_cfg_test = false;
        let mut skipping_test_module = false;
        let mut brace_depth = 0i32;

        for line in source.lines() {
            let trimmed = line.trim();
            if skipping_test_module {
                brace_depth += line.matches('{').count() as i32;
                brace_depth -= line.matches('}').count() as i32;
                if brace_depth <= 0 {
                    skipping_test_module = false;
                    brace_depth = 0;
                }
                continue;
            }

            if trimmed.starts_with("#[cfg(test)]") {
                pending_cfg_test = true;
                continue;
            }

            if pending_cfg_test && trimmed.starts_with("mod ") {
                pending_cfg_test = false;
                if trimmed.ends_with(';') {
                    continue;
                }
                skipping_test_module = true;
                brace_depth += line.matches('{').count() as i32;
                brace_depth -= line.matches('}').count() as i32;
                if brace_depth <= 0 {
                    skipping_test_module = false;
                    brace_depth = 0;
                }
                continue;
            }

            if pending_cfg_test && trimmed.is_empty() {
                // A blank line may sit between `#[cfg(test)]` and its `mod`;
                // keep the pending state so the test module is still stripped.
                out.push_str(line);
                out.push('\n');
                continue;
            }
            pending_cfg_test = false;
            out.push_str(line);
            out.push('\n');
        }

        out
    }

    fn line_calls_or_imports_oracle(line: &str) -> bool {
        let trimmed = line.trim();
        if trimmed.is_empty()
            || trimmed.starts_with("#[deprecated")
            || trimmed.starts_with("pub fn reference_")
            || trimmed.starts_with("pub fn try_reference_")
            || trimmed.starts_with("fn reference_")
            || trimmed.starts_with("pub use ")
        {
            return false;
        }
        trimmed.contains("vyre_reference::reference_eval(")
            || trimmed.contains(" reference_")
            || trimmed.contains("::reference_")
            || trimmed.contains(" try_reference_")
            || trimmed.contains("::try_reference_")
    }

    #[test]
    fn production_c_frontend_does_not_call_cpu_oracles_directly() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/parsing/c");
        let mut files = Vec::new();
        collect_rs_files(&root, &mut files);

        let mut violations = Vec::new();
        for path in files {
            if is_oracle_or_test_file(&path) {
                continue;
            }
            let source = std::fs::read_to_string(&path).unwrap_or_else(|error| {
                panic!("Fix: read C frontend source `{}`: {error}", path.display())
            });
            let source = uncommented_source(&source);
            let source = production_only_source(&source);
            if source.lines().any(line_calls_or_imports_oracle) {
                violations.push(
                    path.strip_prefix(&root)
                        .unwrap_or(&path)
                        .display()
                        .to_string(),
                );
            }
        }

        assert!(
            violations.is_empty(),
            "Fix: production C frontend modules must not call CPU reference oracles directly; use GPU builders in production and keep oracles in explicit reference/test/witness modules only. Violations: {violations:?}"
        );
    }
}
