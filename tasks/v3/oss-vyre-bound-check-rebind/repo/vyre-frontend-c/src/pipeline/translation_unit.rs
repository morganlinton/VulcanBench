use super::*;
pub(super) struct PreparedTranslationUnit {
    pub(super) path: PathBuf,
    pub(super) dest: PathBuf,
    pub(super) source: String,
}

struct LexProgramPlan {
    program: Program,
    sparse_output: bool,
    keyword_promoted: bool,
}

pub(super) fn prepare_translation_unit(
    path: &Path,
    dest: PathBuf,
    options: &VyreCompileOptions,
) -> Result<PreparedTranslationUnit, String> {
    let raw_bytes = read_translation_unit_bounded(path)?;
    prepare_translation_unit_from_bytes(path, dest, &raw_bytes, options)
}

pub(super) fn prepare_translation_unit_from_bytes(
    path: &Path,
    dest: PathBuf,
    raw_bytes: &[u8],
    options: &VyreCompileOptions,
) -> Result<PreparedTranslationUnit, String> {
    let trace = std::env::var("VYRE_STAGE_TRACE").is_ok();
    let prep_start = std::time::Instant::now();
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    if ext != "c" && ext != "h" {
        return Err(format!(
            "vyre-frontend-c: expected .c or .h (got {ext:?} on {}).",
            path.display()
        ));
    }

    let raw = std::str::from_utf8(raw_bytes).map_err(|error| {
        format!(
            "vyre-frontend-c: translation unit {} is not UTF-8 at byte {}: {error}. Fix: normalize the source encoding before invoking the resident GPU frontend.",
            path.display(),
            error.valid_up_to()
        )
    })?;
    if trace {
        eprintln!(
            "[stage-trace] +{}ms: prepare_translation_unit source load ({} bytes)",
            prep_start.elapsed().as_millis(),
            raw_bytes.len()
        );
    }
    reject_c11_source_diagnostics(path, &raw)?;
    let pre_gpu = std::time::Instant::now();
    let source = crate::tu_host::prepare_resident_translation_unit_source_gpu(path, raw, options)?;
    if trace {
        eprintln!(
            "[stage-trace] +{}ms: resident preprocessor ({} → {} bytes)",
            pre_gpu.elapsed().as_millis(),
            raw_bytes.len(),
            source.len()
        );
    }
    if let Some(dump_dir) = std::env::var_os("VYRE_DUMP_PREPROCESSED_DIR") {
        let target = std::path::PathBuf::from(dump_dir).join(format!(
            "{}.preprocessed",
            path.file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("unnamed")
        ));
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "vyre-frontend-c: failed to create preprocessed dump directory {}: {error}. Fix: set VYRE_DUMP_PREPROCESSED_DIR to a writable path or unset it.",
                    parent.display()
                )
            })?;
        }
        std::fs::write(&target, source.as_bytes()).map_err(|error| {
            format!(
                "vyre-frontend-c: failed to write preprocessed dump {}: {error}. Fix: set VYRE_DUMP_PREPROCESSED_DIR to a writable path or unset it.",
                target.display()
            )
        })?;
    }
    reject_c11_source_diagnostics(path, &source)?;
    Ok(PreparedTranslationUnit {
        path: path.to_path_buf(),
        dest,
        source,
    })
}

pub(super) fn read_translation_unit_bounded(path: &Path) -> Result<Vec<u8>, String> {
    use std::io::Read as _;

    let metadata = fs::metadata(path).map_err(|error| {
        format!(
            "vyre-frontend-c: read translation unit metadata {}: {error}",
            path.display()
        )
    })?;
    if metadata.len() > MAX_TRANSLATION_UNIT_BYTES {
        return Err(format!(
            "vyre-frontend-c: translation unit {} is {} bytes; maximum accepted input is {MAX_TRANSLATION_UNIT_BYTES} bytes",
            path.display(),
            metadata.len()
        ));
    }
    let capacity = usize::try_from(metadata.len()).map_err(|_| {
        format!(
            "vyre-frontend-c: translation unit {} is {} bytes and exceeds host addressable memory. Fix: shard or reject this input before GPU preprocessing.",
            path.display(),
            metadata.len()
        )
    })?;
    let mut file = fs::File::open(path).map_err(|error| {
        format!(
            "vyre-frontend-c: open translation unit {}: {error}",
            path.display()
        )
    })?;
    let mut bytes = Vec::with_capacity(capacity);
    file.by_ref()
        .take(MAX_TRANSLATION_UNIT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| {
            format!(
                "vyre-frontend-c: read translation unit {}: {error}",
                path.display()
            )
        })?;
    if bytes.len() as u64 > MAX_TRANSLATION_UNIT_BYTES {
        return Err(format!(
            "vyre-frontend-c: translation unit {} exceeded {MAX_TRANSLATION_UNIT_BYTES} bytes while reading",
            path.display()
        ));
    }
    Ok(bytes)
}
