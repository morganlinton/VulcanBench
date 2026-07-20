//! Conditional-state evidence emitted by the GPU preprocessor driver.

/// Conditional directive kind observed by the GPU preprocessor driver.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConditionalEventKind {
    /// `#ifdef`.
    Ifdef,
    /// `#ifndef`.
    Ifndef,
    /// `#if`.
    If,
    /// `#elif`.
    Elif,
    /// `#else`.
    Else,
    /// `#endif`.
    Endif,
}

/// Residency class for conditional preprocessing evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConditionalEventResidency {
    /// Directive row and payload were extracted by GPU kernels.
    GpuResidentDirective,
    /// Conditional truth was evaluated by GPU kernels.
    GpuResidentTruth,
    /// Conditional truth came from the live host macro table after GPU payload
    /// extraction identified the macro name.
    HostLiveMacroTable,
    /// Compact conditional stack state was threaded by the host driver.
    HostStackThreading,
}

/// Conditional-state event emitted by the GPU-resident preprocessor driver.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConditionalEvent {
    /// File that contained the conditional directive.
    pub file: std::path::PathBuf,
    /// Conditional directive kind.
    pub kind: ConditionalEventKind,
    /// Directive row in the classified token stream.
    pub directive_row: u32,
    /// Byte offset of the directive token in the filtered source.
    pub directive_byte_offset: u32,
    /// Stack depth before applying this directive.
    pub depth_before: u32,
    /// Stack depth after applying this directive.
    pub depth_after: u32,
    /// Parent active state before applying this directive.
    pub parent_active: bool,
    /// Truth value for `#if`, `#elif`, `#ifdef`, and `#ifndef` when evaluated.
    pub truth: Option<bool>,
    /// Current active state after applying this directive.
    pub current_active: bool,
    /// Whether any branch in this conditional group has been taken after applying this directive.
    pub branch_taken: bool,
    /// Residency of directive row/payload extraction.
    pub directive_residency: ConditionalEventResidency,
    /// Residency of truth evaluation or stack transition.
    pub state_residency: ConditionalEventResidency,
}

/// Pushes one conditional event after validating compact integer fields.
#[allow(clippy::too_many_arguments)]
pub(super) fn push_conditional_event(
    events: &mut Vec<ConditionalEvent>,
    file_path: &std::path::Path,
    kind: ConditionalEventKind,
    directive_row: usize,
    directive_byte_offset: usize,
    depth_before: usize,
    depth_after: usize,
    parent_active: bool,
    truth: Option<bool>,
    current_active: bool,
    branch_taken: bool,
    state_residency: ConditionalEventResidency,
) -> Result<(), String> {
    events.push(ConditionalEvent {
        file: file_path.to_path_buf(),
        kind,
        directive_row: u32::try_from(directive_row).map_err(|_| {
            "vyre-libs::gpu_pipeline: conditional directive row exceeds u32. Fix: shard preprocessing before conditional-event evidence export.".to_string()
        })?,
        directive_byte_offset: u32::try_from(directive_byte_offset).map_err(|_| {
            "vyre-libs::gpu_pipeline: conditional directive byte offset exceeds u32. Fix: shard preprocessing before conditional-event evidence export.".to_string()
        })?,
        depth_before: u32::try_from(depth_before).map_err(|_| {
            "vyre-libs::gpu_pipeline: conditional depth exceeds u32. Fix: reject pathological conditional nesting before evidence export.".to_string()
        })?,
        depth_after: u32::try_from(depth_after).map_err(|_| {
            "vyre-libs::gpu_pipeline: conditional depth exceeds u32. Fix: reject pathological conditional nesting before evidence export.".to_string()
        })?,
        parent_active,
        truth,
        current_active,
        branch_taken,
        directive_residency: ConditionalEventResidency::GpuResidentDirective,
        state_residency,
    });
    Ok(())
}
