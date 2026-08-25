//! Macro expansion-origin evidence emitted by the GPU preprocessor driver.

use super::macro_events::{macro_args_are_variadic, stable_macro_symbol_id};
use super::{ClassifiedTokens, MacroDef};
use rustc_hash::FxHashMap as HashMap;
use smallvec::SmallVec;

type MacroBucket<'a> = SmallVec<[&'a MacroDef; 2]>;

/// Macro expansion origin event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroExpansionEvent {
    /// File whose active segment contained the macro use.
    pub file: std::path::PathBuf,
    /// Stable macro symbol ID derived from the macro name bytes.
    pub symbol_id: [u8; 16],
    /// Macro name bytes.
    pub name: Vec<u8>,
    /// Replacement body bytes.
    pub replacement: Vec<u8>,
    /// Invocation argument bytes for function-like macros.
    pub invocation_args: Vec<u8>,
    /// Macro use byte offset in the active segment that was expanded.
    pub use_start: u32,
    /// Macro use byte length in the active segment that was expanded.
    pub use_len: u32,
    /// Include stack active when this expansion was observed.
    pub include_stack: Vec<std::path::PathBuf>,
    /// Whether this expansion used function-like invocation syntax.
    pub is_function_like: bool,
    /// Whether this expansion used a variadic macro definition.
    pub is_variadic: bool,
    /// Whether the expansion was executed by the GPU materialization kernel.
    pub gpu_resident: bool,
}

/// Records macro expansion origin events from the GPU-tokenized active segment.
pub(super) fn record_macro_expansions(
    file_path: &std::path::Path,
    include_stack: &[std::path::PathBuf],
    macros: &[MacroDef],
    classified: &ClassifiedTokens,
    macro_expansion_events: &mut Vec<MacroExpansionEvent>,
) -> Result<(), String> {
    let mut macros_by_name: HashMap<&[u8], MacroBucket<'_>> = HashMap::default();
    macros_by_name.try_reserve(macros.len()).map_err(|error| {
        format!(
            "vyre-libs::gpu_pipeline: could not reserve {} macro-expansion evidence index entries: {error:?}. Fix: shard expansion evidence recording before GPU preprocessing.",
            macros.len()
        )
    })?;
    for mac in macros {
        macros_by_name
            .entry(mac.name.as_slice())
            .or_default()
            .push(mac);
    }
    for (idx, token_kind) in classified.tok_types.iter().enumerate() {
        if *token_kind == 0 {
            continue;
        }
        let start = *classified.tok_starts.get(idx).ok_or_else(|| {
            "vyre-libs::gpu_pipeline: macro expansion token start missing. Fix: repair GPU lexer output column lengths.".to_string()
        })?;
        let len = *classified.tok_lens.get(idx).ok_or_else(|| {
            "vyre-libs::gpu_pipeline: macro expansion token length missing. Fix: repair GPU lexer output column lengths.".to_string()
        })?;
        let start_usize = start as usize;
        let end = start_usize.checked_add(len as usize).ok_or_else(|| {
            "vyre-libs::gpu_pipeline: macro expansion token range overflows usize. Fix: shard preprocessing before expansion evidence export.".to_string()
        })?;
        let token = classified.source.get(start_usize..end).ok_or_else(|| {
            "vyre-libs::gpu_pipeline: macro expansion token range is outside source. Fix: repair GPU lexer spans before expansion evidence export.".to_string()
        })?;
        let Some(candidate_macros) = macros_by_name.get(token) else {
            continue;
        };
        macro_expansion_events
            .try_reserve(candidate_macros.len())
            .map_err(|error| {
                format!(
                    "vyre-libs::gpu_pipeline: could not reserve {} macro-expansion events: {error:?}. Fix: shard expansion evidence recording before GPU preprocessing.",
                    candidate_macros.len()
                )
            })?;
        for mac in candidate_macros {
            if mac.is_function_like {
                if let Some(args) = function_like_invocation_args(&classified.source, end) {
                    macro_expansion_events.push(MacroExpansionEvent {
                        file: file_path.to_path_buf(),
                        symbol_id: stable_macro_symbol_id(&mac.name),
                        name: mac.name.clone(),
                        replacement: mac.body.clone(),
                        invocation_args: args,
                        use_start: start,
                        use_len: len,
                        include_stack: include_stack.to_vec(),
                        is_function_like: true,
                        is_variadic: macro_args_are_variadic(&mac.args),
                        gpu_resident: true,
                    });
                }
            } else {
                macro_expansion_events.push(MacroExpansionEvent {
                    file: file_path.to_path_buf(),
                    symbol_id: stable_macro_symbol_id(&mac.name),
                    name: mac.name.clone(),
                    replacement: mac.body.clone(),
                    invocation_args: Vec::new(),
                    use_start: start,
                    use_len: len,
                    include_stack: include_stack.to_vec(),
                    is_function_like: false,
                    is_variadic: false,
                    gpu_resident: true,
                });
            }
        }
    }
    Ok(())
}

fn function_like_invocation_args(source: &[u8], after_name: usize) -> Option<Vec<u8>> {
    let mut pos = after_name;
    while source
        .get(pos)
        .is_some_and(|byte| byte.is_ascii_whitespace())
    {
        pos += 1;
    }
    if source.get(pos).copied() != Some(b'(') {
        return None;
    }
    let args_start = pos + 1;
    let mut depth = 1_u32;
    pos += 1;
    while let Some(byte) = source.get(pos).copied() {
        match byte {
            b'(' => depth = depth.saturating_add(1),
            b')' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return source.get(args_start..pos).map(ToOwned::to_owned);
                }
            }
            _ => {}
        }
        pos += 1;
    }
    None
}
