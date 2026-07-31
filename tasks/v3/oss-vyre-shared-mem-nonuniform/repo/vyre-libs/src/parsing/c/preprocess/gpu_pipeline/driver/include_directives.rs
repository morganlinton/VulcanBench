use std::path::Path;

use super::include_file_cache::IncludeFileResidency;
use super::stage_trace::StageTrace;
use super::PreprocessRun;
use crate::parsing::c::preprocess::gpu_pipeline::{IncludeEvent, IncludeEventResidency};

pub(super) fn apply_include(
    run: &mut PreprocessRun<'_>,
    file_path: &Path,
    path: &[u8],
    is_system: bool,
    is_next: bool,
    directive_row: usize,
    directive_byte_offset: usize,
    depth: u32,
    trace: &StageTrace<'_>,
) -> Result<bool, String> {
    let Some(resolved) = run
        .include_file_cache
        .resolve(run.loader, file_path, path, is_system, is_next)?
    else {
        let include_name = String::from_utf8_lossy(path);
        return Err(format!(
            "vyre-libs::gpu_pipeline: #include `{include_name}` not resolved from {}. Fix: return an error from the loader with the missing search path details instead of Ok(None).",
            file_path.display()
        ));
    };
    let resolution_residency = match resolved.residency {
        IncludeFileResidency::Filesystem => IncludeEventResidency::HostFilesystemMetadata,
        IncludeFileResidency::RunCache => IncludeEventResidency::HostMemoryCache,
    };
    run.include_events.push(IncludeEvent {
        includer: file_path.to_path_buf(),
        requested_path: path.to_vec(),
        resolved_path: resolved.canonical_path.clone(),
        directive_row: checked_event_u32("include directive row", directive_row)?,
        directive_byte_offset: checked_event_u32(
            "include directive byte offset",
            directive_byte_offset,
        )?,
        is_system,
        is_next,
        request_residency: IncludeEventResidency::GpuResidentRequest,
        resolution_residency,
    });
    if trace.enabled() {
        tracing::debug!(
            "[stage-trace] gpu-preprocess include depth={depth} from={} include={} bytes={}",
            file_path.display(),
            resolved.canonical_path.display(),
            resolved.bytes.len()
        );
    }
    if run.stack.contains(&resolved.canonical_path) {
        return Ok(false);
    }
    if let Some(event) = run
        .include_acceleration_state
        .skip_event(&resolved.canonical_path, &run.macro_index)
    {
        run.include_acceleration_events.push(event);
        return Ok(false);
    }
    run.stack.push(resolved.canonical_path.clone());
    let res = run.preprocess_one_file(&resolved.canonical_path, &resolved.bytes, depth + 1);
    run.stack.pop();
    res?;
    Ok(true)
}

fn checked_event_u32(label: &str, value: usize) -> Result<u32, String> {
    u32::try_from(value).map_err(|_| {
        format!(
            "vyre-libs::gpu_pipeline: {label} exceeds u32. Fix: shard preprocessing before event evidence export."
        )
    })
}
