use std::path::Path;

use super::PreprocessRun;
use crate::parsing::c::preprocess::gpu_pipeline::conditional_events::push_conditional_event;
use crate::parsing::c::preprocess::gpu_pipeline::conditional_stack::ConditionalFrame;
use crate::parsing::c::preprocess::gpu_pipeline::live_state::{
    cached_live_macro_name_buffers, recompute_if_expr_truth_gpu_with_scratch,
    recompute_ifdef_truth_gpu_with_scratch, LiveMacroNameBuffers,
};
use crate::parsing::c::preprocess::gpu_pipeline::source_spans::token_row_bytes;
use crate::parsing::c::preprocess::gpu_pipeline::{
    ClassifiedTokens, ConditionalEventKind, ConditionalEventResidency,
};

pub(super) fn conditionals_active(conditionals: &[ConditionalFrame]) -> bool {
    conditionals
        .last()
        .map(|frame| frame.current_active)
        .unwrap_or(true)
}

pub(super) fn apply_ifdef(
    run: &mut PreprocessRun<'_>,
    file_path: &Path,
    classified: &ClassifiedTokens,
    conditionals: &mut Vec<ConditionalFrame>,
    live_macro_buffers_cache: &mut Option<LiveMacroNameBuffers>,
    directive_row: usize,
    directive_byte_offset: usize,
    negated: bool,
    guard_name: Option<&[u8]>,
    precomputed_truth: Option<bool>,
    gpu_ifdef: &mut u32,
) -> Result<(), String> {
    let parent = conditionals_active(conditionals);
    let depth_before = conditionals.len();
    let (truth, state_residency) = if !parent {
        (false, ConditionalEventResidency::GpuResidentTruth)
    } else if let Some(truth) = precomputed_truth {
        (truth, ConditionalEventResidency::GpuResidentTruth)
    } else if negated {
        if let Some(guard_name) = guard_name {
            (
                !run.macro_index.contains_key(guard_name),
                ConditionalEventResidency::HostLiveMacroTable,
            )
        } else {
            let row_bytes = token_row_bytes(classified, directive_row)?;
            *gpu_ifdef += 1;
            let live_macro_buffers =
                cached_live_macro_name_buffers(&run.macros, live_macro_buffers_cache)?;
            (
                recompute_ifdef_truth_gpu_with_scratch(
                    run.dispatcher,
                    row_bytes,
                    classified.directive_kinds[directive_row],
                    negated,
                    live_macro_buffers,
                    &mut run.live_conditional_scratch,
                )?,
                ConditionalEventResidency::GpuResidentTruth,
            )
        }
    } else {
        let row_bytes = token_row_bytes(classified, directive_row)?;
        *gpu_ifdef += 1;
        let live_macro_buffers =
            cached_live_macro_name_buffers(&run.macros, live_macro_buffers_cache)?;
        (
            recompute_ifdef_truth_gpu_with_scratch(
                run.dispatcher,
                row_bytes,
                classified.directive_kinds[directive_row],
                negated,
                live_macro_buffers,
                &mut run.live_conditional_scratch,
            )?,
            ConditionalEventResidency::GpuResidentTruth,
        )
    };
    conditionals.push(ConditionalFrame {
        parent_active: parent,
        branch_taken: truth,
        current_active: parent && truth,
        saw_else: false,
    });
    push_conditional_event(
        &mut run.conditional_events,
        file_path,
        if negated {
            ConditionalEventKind::Ifndef
        } else {
            ConditionalEventKind::Ifdef
        },
        directive_row,
        directive_byte_offset,
        depth_before,
        conditionals.len(),
        parent,
        Some(truth),
        parent && truth,
        truth,
        state_residency,
    )
}

pub(super) fn apply_ifexpr(
    run: &mut PreprocessRun<'_>,
    file_path: &Path,
    classified: &ClassifiedTokens,
    conditionals: &mut Vec<ConditionalFrame>,
    live_macro_buffers_cache: &mut Option<LiveMacroNameBuffers>,
    directive_row: usize,
    directive_byte_offset: usize,
    is_elif: bool,
    gpu_if: &mut u32,
) -> Result<(), String> {
    if is_elif {
        apply_elif(
            run,
            file_path,
            classified,
            conditionals,
            live_macro_buffers_cache,
            directive_row,
            directive_byte_offset,
            gpu_if,
        )
    } else {
        apply_if(
            run,
            file_path,
            classified,
            conditionals,
            live_macro_buffers_cache,
            directive_row,
            directive_byte_offset,
            gpu_if,
        )
    }
}

pub(super) fn apply_else(
    run: &mut PreprocessRun<'_>,
    file_path: &Path,
    conditionals: &mut [ConditionalFrame],
    directive_row: usize,
    directive_byte_offset: usize,
) -> Result<(), String> {
    let depth_before = conditionals.len();
    let Some(frame) = conditionals.last_mut() else {
        return Err(format!(
            "vyre-libs::gpu_pipeline: #else without matching #if in {}. Fix: repair conditional directive structure.",
            file_path.display()
        ));
    };
    if frame.saw_else {
        return Err(format!(
            "vyre-libs::gpu_pipeline: duplicate #else in {}. Fix: keep exactly one #else per #if block.",
            file_path.display()
        ));
    }
    let take = !frame.branch_taken;
    frame.current_active = frame.parent_active && take;
    frame.branch_taken = true;
    frame.saw_else = true;
    let parent_active = frame.parent_active;
    let current_active = frame.current_active;
    let branch_taken = frame.branch_taken;
    push_conditional_event(
        &mut run.conditional_events,
        file_path,
        ConditionalEventKind::Else,
        directive_row,
        directive_byte_offset,
        depth_before,
        depth_before,
        parent_active,
        None,
        current_active,
        branch_taken,
        ConditionalEventResidency::HostStackThreading,
    )
}

pub(super) fn apply_endif(
    run: &mut PreprocessRun<'_>,
    file_path: &Path,
    conditionals: &mut Vec<ConditionalFrame>,
    directive_row: usize,
    directive_byte_offset: usize,
) -> Result<(), String> {
    let depth_before = conditionals.len();
    if conditionals.pop().is_none() {
        return Err(format!(
            "vyre-libs::gpu_pipeline: #endif without matching #if in {}. Fix: repair conditional directive structure.",
            file_path.display()
        ));
    }
    let current_active = conditionals_active(conditionals);
    push_conditional_event(
        &mut run.conditional_events,
        file_path,
        ConditionalEventKind::Endif,
        directive_row,
        directive_byte_offset,
        depth_before,
        conditionals.len(),
        current_active,
        None,
        current_active,
        conditionals
            .last()
            .map(|frame| frame.branch_taken)
            .unwrap_or(false),
        ConditionalEventResidency::HostStackThreading,
    )
}

fn apply_elif(
    run: &mut PreprocessRun<'_>,
    file_path: &Path,
    classified: &ClassifiedTokens,
    conditionals: &mut [ConditionalFrame],
    live_macro_buffers_cache: &mut Option<LiveMacroNameBuffers>,
    directive_row: usize,
    directive_byte_offset: usize,
    gpu_if: &mut u32,
) -> Result<(), String> {
    let depth_before = conditionals.len();
    let Some(frame) = conditionals.last_mut() else {
        return Err(format!(
            "vyre-libs::gpu_pipeline: #elif without matching #if in {}. Fix: repair conditional directive structure.",
            file_path.display()
        ));
    };
    if frame.saw_else {
        return Err(format!(
            "vyre-libs::gpu_pipeline: #elif after #else in {}. Fix: place all #elif branches before #else.",
            file_path.display()
        ));
    }
    let should_evaluate = frame.parent_active && !frame.branch_taken;
    let truth = if !should_evaluate {
        false
    } else {
        let row_bytes = token_row_bytes(classified, directive_row)?;
        *gpu_if += 1;
        let live_macro_buffers =
            cached_live_macro_name_buffers(&run.macros, live_macro_buffers_cache)?;
        recompute_if_expr_truth_gpu_with_scratch(
            run.dispatcher,
            row_bytes,
            classified.directive_kinds[directive_row],
            &run.macros,
            live_macro_buffers,
            &mut run.live_conditional_scratch,
        )?
    };
    let take = should_evaluate && truth;
    frame.current_active = frame.parent_active && take;
    frame.branch_taken |= take;
    let parent_active = frame.parent_active;
    let current_active = frame.current_active;
    let branch_taken = frame.branch_taken;
    push_conditional_event(
        &mut run.conditional_events,
        file_path,
        ConditionalEventKind::Elif,
        directive_row,
        directive_byte_offset,
        depth_before,
        depth_before,
        parent_active,
        Some(truth),
        current_active,
        branch_taken,
        ConditionalEventResidency::GpuResidentTruth,
    )
}

fn apply_if(
    run: &mut PreprocessRun<'_>,
    file_path: &Path,
    classified: &ClassifiedTokens,
    conditionals: &mut Vec<ConditionalFrame>,
    live_macro_buffers_cache: &mut Option<LiveMacroNameBuffers>,
    directive_row: usize,
    directive_byte_offset: usize,
    gpu_if: &mut u32,
) -> Result<(), String> {
    let parent = conditionals_active(conditionals);
    let depth_before = conditionals.len();
    let truth = if !parent {
        false
    } else {
        let row_bytes = token_row_bytes(classified, directive_row)?;
        *gpu_if += 1;
        let live_macro_buffers =
            cached_live_macro_name_buffers(&run.macros, live_macro_buffers_cache)?;
        recompute_if_expr_truth_gpu_with_scratch(
            run.dispatcher,
            row_bytes,
            classified.directive_kinds[directive_row],
            &run.macros,
            live_macro_buffers,
            &mut run.live_conditional_scratch,
        )?
    };
    conditionals.push(ConditionalFrame {
        parent_active: parent,
        branch_taken: truth,
        current_active: parent && truth,
        saw_else: false,
    });
    push_conditional_event(
        &mut run.conditional_events,
        file_path,
        ConditionalEventKind::If,
        directive_row,
        directive_byte_offset,
        depth_before,
        conditionals.len(),
        parent,
        Some(truth),
        parent && truth,
        truth,
        ConditionalEventResidency::GpuResidentTruth,
    )
}
