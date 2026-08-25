use std::path::Path;

use crate::parsing::c::lex::tokens::TOK_PP_ERROR;
use crate::parsing::c::preprocess::gpu_pipeline::conditional_stack::ConditionalFrame;
use crate::parsing::c::preprocess::gpu_pipeline::source_spans::checked_source_range;
use crate::parsing::c::preprocess::gpu_pipeline::tokenization::ClassifiedTokens;

pub(super) fn reject_active_error_directive(
    classified: &ClassifiedTokens,
    file_path: &Path,
    row: usize,
    tok_start: usize,
    tok_end: usize,
) -> Result<(), String> {
    if classified.directive_kinds[row] != TOK_PP_ERROR {
        return Ok(());
    }
    let row_text = String::from_utf8_lossy(checked_source_range(
        &classified.source,
        tok_start,
        tok_end,
        "#error diagnostic",
    )?);
    Err(format!(
        "vyre-libs::gpu_pipeline: active #error directive in {}: {}",
        file_path.display(),
        row_text.trim()
    ))
}

pub(super) fn reject_unterminated_conditionals(
    file_path: &Path,
    conditionals: &[ConditionalFrame],
) -> Result<(), String> {
    if conditionals.is_empty() {
        return Ok(());
    }
    Err(format!(
        "vyre-libs::gpu_pipeline: reached end of {} with {} unterminated conditional block(s). Fix: add the missing #endif directive(s).",
        file_path.display(),
        conditionals.len()
    ))
}
