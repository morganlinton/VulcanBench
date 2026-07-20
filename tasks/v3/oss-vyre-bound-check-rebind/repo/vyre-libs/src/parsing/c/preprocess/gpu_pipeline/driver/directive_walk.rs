use std::path::Path;

use super::conditional_directives::{
    apply_else, apply_endif, apply_ifdef, apply_ifexpr, conditionals_active,
};
use super::directive_diagnostics::{
    reject_active_error_directive, reject_unterminated_conditionals,
};
use super::directive_emission::{
    emit_active_row, emit_trailing_active_bytes, flush_before_directive, flush_final_active_bytes,
    row_token_span,
};
use super::directive_state::DirectiveWalkState;
use super::file_inputs::PreparedFile;
use super::ifdef_truth_batch::ensure_ifdef_truth_batch;
use super::include_directives::apply_include;
use super::macro_directives::{apply_define, apply_undef};
use super::stage_trace::StageTrace;
use super::PreprocessRun;
use crate::parsing::c::preprocess::gpu_pipeline::token_provenance::record_direct_token_provenance;
use crate::parsing::c::preprocess::gpu_pipeline::DirectivePayload;

pub(super) fn walk_directives(
    run: &mut PreprocessRun<'_>,
    file_path: &Path,
    source: &[u8],
    depth: u32,
    prepared: PreparedFile,
    trace: &mut StageTrace<'_>,
) -> Result<(), String> {
    let classified = prepared.classified;
    let payloads = prepared.payloads;
    let classified = classified.as_ref();
    let payloads = payloads.as_ref();
    let directive_free = payloads.is_empty()
        || payloads
            .iter()
            .all(|payload| matches!(payload, DirectivePayload::None));
    if run.macros.is_empty() && directive_free {
        let output_base = run.output.len();
        record_direct_token_provenance(
            file_path,
            &run.stack,
            classified,
            output_base,
            &mut run.token_provenance_events,
        )?;
        run.output.extend_from_slice(&classified.source);
        trace.log("emit directive-free gpu-filtered source");
        return Ok(());
    }
    if payloads.is_empty() {
        let mut state = DirectiveWalkState::new(payloads);
        for i in 0..classified.tok_types.len() {
            let (tok_start, tok_end) = row_token_span(&classified, i)?;
            emit_active_row(&classified, &mut state, tok_start, tok_end)?;
        }
        emit_trailing_active_bytes(&classified, &mut state)?;
        flush_final_active_bytes(run, file_path, &classified, &mut state)?;
        trace.log("walk directive-free tokens");
        return Ok(());
    }
    let mut state = DirectiveWalkState::new(payloads);

    for (i, payload) in payloads.iter().enumerate() {
        let row_active = conditionals_active(&state.conditionals);
        let (tok_start, tok_end) = row_token_span(&classified, i)?;
        if !matches!(payload, DirectivePayload::None) {
            flush_before_directive(run, file_path, &classified, &mut state)?;
        }
        match payload {
            DirectivePayload::None => {
                emit_active_row(&classified, &mut state, tok_start, tok_end)?;
            }
            DirectivePayload::Define {
                name,
                name_start,
                name_len,
                args,
                args_start,
                args_len,
                body,
                body_start,
                body_len,
                is_function_like,
            } => {
                if row_active {
                    apply_define(
                        run,
                        file_path,
                        i,
                        tok_start,
                        name,
                        (*name_start, *name_len),
                        args,
                        (*args_start, *args_len),
                        body,
                        (*body_start, *body_len),
                        *is_function_like,
                        &mut state.macro_expansion_cache,
                        &mut state.live_macro_buffers_cache,
                    )?;
                    state.invalidate_ifdef_truth_cache();
                }
                state.last_emit_end = tok_end;
            }
            DirectivePayload::Undef { name } => {
                if row_active {
                    apply_undef(
                        run,
                        file_path,
                        i,
                        tok_start,
                        name,
                        &mut state.macro_expansion_cache,
                        &mut state.live_macro_buffers_cache,
                    )?;
                    state.invalidate_ifdef_truth_cache();
                }
                state.last_emit_end = tok_end;
            }
            DirectivePayload::Include {
                path,
                is_system,
                is_next,
            } => {
                if row_active
                    && apply_include(
                        run, file_path, path, *is_system, *is_next, i, tok_start, depth, trace,
                    )?
                {
                    state.invalidate_macro_dependent_caches();
                }
                state.last_emit_end = tok_end;
            }
            DirectivePayload::Ifdef { value: _, negated } => {
                let guard_name = state.include_guard_ifndef_names.name_at(i);
                let precomputed_truth = if row_active && guard_name.is_none() {
                    ensure_ifdef_truth_batch(
                        run,
                        &classified,
                        &payloads,
                        &state.include_guard_ifndef_names,
                        &mut state.live_macro_buffers_cache,
                        &mut state.ifdef_truth_cache,
                        i,
                        &mut state.gpu_ifdef,
                    )?;
                    state.ifdef_truth_cache.get(&i).copied()
                } else {
                    None
                };
                apply_ifdef(
                    run,
                    file_path,
                    &classified,
                    &mut state.conditionals,
                    &mut state.live_macro_buffers_cache,
                    i,
                    tok_start,
                    *negated,
                    guard_name,
                    precomputed_truth,
                    &mut state.gpu_ifdef,
                )?;
                state.last_emit_end = tok_end;
            }
            DirectivePayload::IfExpr { value: _, is_elif } => {
                apply_ifexpr(
                    run,
                    file_path,
                    &classified,
                    &mut state.conditionals,
                    &mut state.live_macro_buffers_cache,
                    i,
                    tok_start,
                    *is_elif,
                    &mut state.gpu_if,
                )?;
                state.last_emit_end = tok_end;
            }
            DirectivePayload::Else => {
                apply_else(run, file_path, &mut state.conditionals, i, tok_start)?;
                state.last_emit_end = tok_end;
            }
            DirectivePayload::Endif => {
                apply_endif(run, file_path, &mut state.conditionals, i, tok_start)?;
                state.last_emit_end = tok_end;
            }
            DirectivePayload::Other => {
                if row_active {
                    reject_active_error_directive(&classified, file_path, i, tok_start, tok_end)?;
                }
                state.last_emit_end = tok_end;
            }
        }
    }

    reject_unterminated_conditionals(file_path, &state.conditionals)?;
    emit_trailing_active_bytes(&classified, &mut state)?;
    flush_final_active_bytes(run, file_path, &classified, &mut state)?;
    trace.log("walk directives");
    if std::env::var_os("VYRE_PREPROC_COUNTS").is_some() {
        let elapsed = state.walk_start.elapsed();
        tracing::debug!(
            "[preproc-counts] {} bytes={} payloads={} elapsed={:?} gpu_ifdef={} gpu_if={}",
            file_path.display(),
            source.len(),
            payloads.len(),
            elapsed,
            state.gpu_ifdef,
            state.gpu_if,
        );
    }
    Ok(())
}
