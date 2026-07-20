//! Per-translation-unit object compilation orchestration.
//!
//! This file sequences named stage duties only. Stage implementation lives in
//! one-duty files under `pipeline/compile_unit/`.

#[path = "compile_unit/abi_layout.rs"]
mod abi_layout;
#[path = "compile_unit/ast_stage.rs"]
mod ast_stage;
#[path = "compile_unit/cfg_stage.rs"]
mod cfg_stage;
#[path = "compile_unit/keyword_stage.rs"]
mod keyword_stage;
#[path = "compile_unit/lex_stage.rs"]
mod lex_stage;
#[path = "compile_unit/object_carrier.rs"]
mod object_carrier;
#[path = "compile_unit/preproc_mask.rs"]
mod preproc_mask;
#[path = "compile_unit/semantic_stage.rs"]
mod semantic_stage;
#[path = "compile_unit/structure_stage.rs"]
mod structure_stage;
#[path = "compile_unit/token_decode.rs"]
mod token_decode;
#[path = "compile_unit/trace.rs"]
mod trace;

use super::*;

pub(super) fn compile_translation_unit(
    backend: &dyn VyreBackend,
    prepared: &PreparedTranslationUnit,
    target_abi: CTargetAbi,
) -> Result<(), String> {
    let path = prepared.path.as_path();
    let dest = prepared.dest.as_path();
    let mut trace = trace::CompileTrace::new();
    trace.log("compile_translation_unit start");

    let mut dcfg = DispatchConfig::default();
    let mut expanded_haystack_cache = None;

    let mut lexed = lex_stage::lex_object_tokens(
        backend,
        path,
        &prepared.source,
        &mut dcfg,
        &mut expanded_haystack_cache,
        &mut trace,
    )?;
    keyword_stage::promote_object_keywords(
        backend,
        path,
        &prepared.source,
        &mut dcfg,
        &mut expanded_haystack_cache,
        &mut lexed,
        &mut trace,
    )?;
    let decoded = token_decode::decode_object_tokens(path, &prepared.source, &lexed, &mut trace)?;
    let preproc_mask = preproc_mask::build_preproc_mask(
        backend,
        path,
        &prepared.source,
        &lexed,
        &mut dcfg,
        &mut trace,
    )?;
    let structure =
        structure_stage::build_object_structure(backend, path, &decoded, &mut dcfg, &mut trace)?;
    let abi_blob = abi_layout::build_object_abi_layout(
        backend, path, target_abi, &decoded, &mut dcfg, &mut trace,
    )?;
    let ast_blob = ast_stage::build_object_ast(backend, path, &decoded, &mut dcfg, &mut trace)?;
    let semantic = semantic_stage::build_object_semantics(
        backend,
        path,
        &prepared.source,
        &lexed,
        &decoded,
        &mut expanded_haystack_cache,
        &mut trace,
    )?;
    let cfg_blob =
        cfg_stage::build_object_cfg(backend, path, &semantic.vast_blob, &mut dcfg, &mut trace)?;
    object_carrier::emit_object_carrier(
        backend,
        path,
        dest,
        &decoded,
        structure,
        preproc_mask,
        abi_blob,
        ast_blob,
        semantic,
        cfg_blob,
        &mut trace,
    )
}
