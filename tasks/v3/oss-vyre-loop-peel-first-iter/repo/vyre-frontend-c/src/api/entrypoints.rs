use super::*;
/// Compile C translation units according to [`VyreCompileOptions`].
pub fn compile(options: VyreCompileOptions) -> Result<(), String> {
    if options.input_files.is_empty() {
        return Err(
            "vyre-frontend-c: no input files specified. Fix: pass at least one C translation unit path in VyreCompileOptions::input_files."
                .to_string(),
        );
    }
    if options.output_file.is_some() && options.input_files.len() > 1 {
        return Err(
            "vyre-frontend-c: output_file with multiple compile inputs has no single-output contract. Fix: compile one translation unit at a time or omit output_file for per-input .o emission."
                .to_string(),
        );
    }
    for input in &options.input_files {
        let metadata = input.metadata().map_err(|err| {
            format!(
                "vyre-frontend-c: input file read/metadata validation failed for {}: {err}. Fix: pass an existing regular C translation unit path in VyreCompileOptions::input_files.",
                input.display()
            )
        })?;
        if !metadata.is_file() {
            return Err(format!(
                "vyre-frontend-c: input {} is not a regular file. Fix: pass a regular C translation unit path in VyreCompileOptions::input_files.",
                input.display()
            ));
        }
    }
    if options.is_compile_only {
        crate::pipeline::compile_c11_sources(&options)
    } else {
        crate::pipeline::link_c11_executable(&options)
    }
}

/// Run the full GPU C parser/sema spine and return pre-lowering evidence metrics.
///
/// This is not the syntax-only path: non-empty inputs must carry typed VAST, ProgramGraph,
/// semantic ProgramGraph, and semantic-scope evidence. Use [`parse_syntax_source`] when only
/// token/AST evidence is desired.
pub fn parse_source(source: &str) -> Result<CParseSummary, String> {
    crate::pipeline::parse_c11_source(source)
}

/// Run the full GPU C parser/sema spine for a real translation unit path and
/// return parse evidence metrics without emitting an object file.
pub fn parse_translation_unit(
    path: &Path,
    options: &VyreCompileOptions,
) -> Result<CParseSummary, String> {
    crate::pipeline::parse_c11_translation_unit(path, options)
}

/// Run the full GPU C parser/sema spine for already-loaded translation-unit
/// bytes while preserving real path/include context.
pub fn parse_translation_unit_bytes(
    path: &Path,
    raw_bytes: &[u8],
    options: &VyreCompileOptions,
) -> Result<CParseSummary, String> {
    crate::pipeline::parse_c11_translation_unit_bytes(path, raw_bytes, options)
}

/// Run the GPU C syntax parser only and return token/AST evidence metrics.
pub fn parse_syntax_source(source: &str) -> Result<CParseSummary, String> {
    crate::pipeline::parse_c11_syntax_source(source)
}
