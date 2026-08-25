use std::path::Path;

use super::PreprocessRun;
use crate::parsing::c::preprocess::gpu_pipeline::live_state::{
    remove_live_macro_indexed, replace_live_macro_indexed,
};
use crate::parsing::c::preprocess::gpu_pipeline::macro_events::{
    macro_args_are_variadic, stable_macro_symbol_id,
};
use crate::parsing::c::preprocess::gpu_pipeline::macro_expansion::MacroExpansionCache;
use crate::parsing::c::preprocess::gpu_pipeline::{MacroDef, MacroEvent, MacroEventKind};

pub(super) fn apply_define(
    run: &mut PreprocessRun<'_>,
    file_path: &Path,
    directive_row: usize,
    directive_byte_offset: usize,
    name: &[u8],
    name_range: (u32, u32),
    args: &[u8],
    args_range: (u32, u32),
    body: &[u8],
    body_range: (u32, u32),
    is_function_like: bool,
    macro_expansion_cache: &mut MacroExpansionCache,
    live_macro_buffers_cache: &mut Option<
        crate::parsing::c::preprocess::gpu_pipeline::live_state::LiveMacroNameBuffers,
    >,
) -> Result<(), String> {
    if name.is_empty() {
        return Err(format!(
            "vyre-libs::gpu_pipeline: active #define with empty macro name in {}. Fix: repair malformed preprocessor directive before GPU macro-table packing.",
            file_path.display()
        ));
    }
    replace_live_macro_indexed(
        &mut run.macros,
        &mut run.macro_index,
        MacroDef {
            name: name.to_vec(),
            args: args.to_vec(),
            body: body.to_vec(),
            is_function_like,
        },
    );
    run.invalidate_defines_hash();
    run.macro_events.push(MacroEvent {
        file: file_path.to_path_buf(),
        kind: MacroEventKind::Define,
        directive_row: checked_event_u32("macro directive row", directive_row)?,
        directive_byte_offset: checked_event_u32(
            "macro directive byte offset",
            directive_byte_offset,
        )?,
        symbol_id: stable_macro_symbol_id(name),
        name: name.to_vec(),
        name_range: Some(name_range),
        args: args.to_vec(),
        args_range: (args_range.1 != 0).then_some(args_range),
        replacement: body.to_vec(),
        replacement_range: (body_range.1 != 0).then_some(body_range),
        is_function_like,
        is_variadic: macro_args_are_variadic(args),
        gpu_resident: true,
    });
    macro_expansion_cache.clear();
    *live_macro_buffers_cache = None;
    Ok(())
}

pub(super) fn apply_undef(
    run: &mut PreprocessRun<'_>,
    file_path: &Path,
    directive_row: usize,
    directive_byte_offset: usize,
    name: &[u8],
    macro_expansion_cache: &mut MacroExpansionCache,
    live_macro_buffers_cache: &mut Option<
        crate::parsing::c::preprocess::gpu_pipeline::live_state::LiveMacroNameBuffers,
    >,
) -> Result<(), String> {
    if name.is_empty() {
        return Err(format!(
            "vyre-libs::gpu_pipeline: active #undef with empty macro name in {}. Fix: repair malformed preprocessor directive before GPU macro-table packing.",
            file_path.display()
        ));
    }
    remove_live_macro_indexed(&mut run.macros, &mut run.macro_index, name);
    run.invalidate_defines_hash();
    run.macro_events.push(MacroEvent {
        file: file_path.to_path_buf(),
        kind: MacroEventKind::Undef,
        directive_row: checked_event_u32("macro directive row", directive_row)?,
        directive_byte_offset: checked_event_u32(
            "macro directive byte offset",
            directive_byte_offset,
        )?,
        symbol_id: stable_macro_symbol_id(name),
        name: name.to_vec(),
        name_range: None,
        args: Vec::new(),
        args_range: None,
        replacement: Vec::new(),
        replacement_range: None,
        is_function_like: false,
        is_variadic: false,
        gpu_resident: true,
    });
    macro_expansion_cache.clear();
    *live_macro_buffers_cache = None;
    Ok(())
}

fn checked_event_u32(label: &str, value: usize) -> Result<u32, String> {
    u32::try_from(value).map_err(|_| {
        format!(
            "vyre-libs::gpu_pipeline: {label} exceeds u32. Fix: shard preprocessing before event evidence export."
        )
    })
}
