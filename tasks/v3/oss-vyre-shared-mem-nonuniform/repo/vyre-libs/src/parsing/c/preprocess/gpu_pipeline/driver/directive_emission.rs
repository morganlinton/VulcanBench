use std::path::Path;

use super::active_segments::emit_active_token_range;
use super::conditional_directives::conditionals_active;
use super::directive_state::DirectiveWalkState;
use super::PreprocessRun;
use crate::parsing::c::preprocess::gpu_pipeline::macro_expansion::flush_active_macro_segment;
use crate::parsing::c::preprocess::gpu_pipeline::segments::append_active_segment;
use crate::parsing::c::preprocess::gpu_pipeline::source_spans::token_row_span;
use crate::parsing::c::preprocess::gpu_pipeline::tokenization::ClassifiedTokens;

pub(super) fn row_token_span(
    classified: &ClassifiedTokens,
    row: usize,
) -> Result<(usize, usize), String> {
    token_row_span(classified, row)
}

pub(super) fn flush_before_directive(
    run: &mut PreprocessRun<'_>,
    file_path: &Path,
    classified: &ClassifiedTokens,
    state: &mut DirectiveWalkState,
) -> Result<(), String> {
    if state.active_segment.is_empty() {
        return Ok(());
    }
    flush_active_bytes(run, file_path, classified, state)?;
    state.active_segment_start = None;
    Ok(())
}

pub(super) fn emit_active_row(
    classified: &ClassifiedTokens,
    state: &mut DirectiveWalkState,
    tok_start: usize,
    tok_end: usize,
) -> Result<(), String> {
    if !conditionals_active(&state.conditionals) {
        return Ok(());
    }
    emit_active_token_range(
        &classified.source,
        &mut state.active_segment,
        &mut state.active_segment_start,
        &mut state.last_emit_end,
        tok_start,
        tok_end,
    )
}

pub(super) fn emit_trailing_active_bytes(
    classified: &ClassifiedTokens,
    state: &mut DirectiveWalkState,
) -> Result<(), String> {
    if conditionals_active(&state.conditionals) && state.last_emit_end < classified.source.len() {
        append_active_segment(
            &mut state.active_segment,
            &mut state.active_segment_start,
            &classified.source,
            state.last_emit_end,
            classified.source.len(),
            "trailing emission",
        )?;
    }
    Ok(())
}

pub(super) fn flush_final_active_bytes(
    run: &mut PreprocessRun<'_>,
    file_path: &Path,
    classified: &ClassifiedTokens,
    state: &mut DirectiveWalkState,
) -> Result<(), String> {
    if state.active_segment.is_empty() {
        return Ok(());
    }
    flush_active_bytes(run, file_path, classified, state)
}

fn flush_active_bytes(
    run: &mut PreprocessRun<'_>,
    file_path: &Path,
    classified: &ClassifiedTokens,
    state: &mut DirectiveWalkState,
) -> Result<(), String> {
    let segment_start = state.active_segment_start.ok_or_else(|| {
        "vyre-libs::gpu_pipeline: active segment bytes existed without a source start. Fix: preserve segment source offsets during directive walking.".to_string()
    })?;
    flush_active_macro_segment(
        run.dispatcher,
        file_path,
        &run.stack,
        classified,
        segment_start,
        &run.macros,
        &mut run.macro_events,
        &mut state.macro_expansion_cache,
        &mut state.active_segment,
        &mut run.output,
        &mut run.macro_expansion_events,
        &mut run.token_provenance_events,
    )
}
