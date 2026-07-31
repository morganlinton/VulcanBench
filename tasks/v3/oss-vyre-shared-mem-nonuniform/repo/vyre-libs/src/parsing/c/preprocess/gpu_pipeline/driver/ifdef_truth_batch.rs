use super::include_guard_scan::IncludeGuardIfndefNames;
use super::PreprocessRun;
use crate::parsing::c::preprocess::gpu_pipeline::live_state::{
    cached_live_macro_name_buffers, recompute_ifdef_truths_gpu_with_scratch, IfdefTruthRow,
    LiveMacroNameBuffers,
};
use crate::parsing::c::preprocess::gpu_pipeline::{ClassifiedTokens, DirectivePayload};
use rustc_hash::FxHashMap as HashMap;

#[derive(Default)]
pub(super) struct IfdefTruthBatchScratch {
    row_indices: Vec<usize>,
}

pub(super) fn ensure_ifdef_truth_batch(
    run: &mut PreprocessRun<'_>,
    classified: &ClassifiedTokens,
    payloads: &[DirectivePayload],
    include_guard_ifndef_names: &IncludeGuardIfndefNames,
    live_macro_buffers_cache: &mut Option<LiveMacroNameBuffers>,
    ifdef_truth_cache: &mut HashMap<usize, bool>,
    start_idx: usize,
    gpu_ifdef_dispatches: &mut u32,
) -> Result<(), String> {
    if ifdef_truth_cache.contains_key(&start_idx) {
        return Ok(());
    }
    run.ifdef_truth_batch_scratch.row_indices.clear();
    for idx in start_idx..payloads.len() {
        match &payloads[idx] {
            DirectivePayload::Define { .. }
            | DirectivePayload::Undef { .. }
            | DirectivePayload::Include { .. }
                if idx != start_idx =>
            {
                break
            }
            DirectivePayload::Ifdef { .. } if include_guard_ifndef_names.name_at(idx).is_none() => {
                run.ifdef_truth_batch_scratch.row_indices.push(idx);
            }
            _ => {}
        }
    }
    if run.ifdef_truth_batch_scratch.row_indices.is_empty() {
        return Ok(());
    }
    let rows = run
        .ifdef_truth_batch_scratch
        .row_indices
        .iter()
        .map(|idx| {
            Ok(IfdefTruthRow {
                row_bytes:
                    crate::parsing::c::preprocess::gpu_pipeline::source_spans::token_row_bytes(
                        classified, *idx,
                    )?,
                directive_kind: classified.directive_kinds[*idx],
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let live_macro_buffers = cached_live_macro_name_buffers(&run.macros, live_macro_buffers_cache)?;
    let truths = recompute_ifdef_truths_gpu_with_scratch(
        run.dispatcher,
        &rows,
        live_macro_buffers,
        &mut run.live_conditional_scratch,
    )?;
    *gpu_ifdef_dispatches = gpu_ifdef_dispatches.saturating_add(1);
    for (idx, truth) in run
        .ifdef_truth_batch_scratch
        .row_indices
        .iter()
        .copied()
        .zip(truths.iter().copied())
    {
        ifdef_truth_cache.insert(idx, truth);
    }
    Ok(())
}
