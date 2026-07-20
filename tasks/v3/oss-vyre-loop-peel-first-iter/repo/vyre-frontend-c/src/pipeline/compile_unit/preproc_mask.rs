use std::cell::RefCell;
use std::mem;

use super::*;

#[derive(Default)]
struct PreprocMaskScratch {
    mask_init: Vec<u8>,
    outputs: Vec<Vec<u8>>,
}

thread_local! {
    static PREPROC_MASK_SCRATCH: RefCell<PreprocMaskScratch> =
        RefCell::new(PreprocMaskScratch::default());
}

fn all_enabled_preproc_mask_bytes(n_tokens: u32) -> Result<Vec<u8>, String> {
    let byte_len = usize::try_from(n_tokens.max(1))
        .ok()
        .and_then(|count| count.checked_mul(4))
        .ok_or_else(|| {
            format!(
                "preprocessor all-enabled mask byte length overflows host indexing for n_tokens={n_tokens}. Fix: shard the token stream before mask construction."
            )
        })?;
    let mut mask = vec![0u8; byte_len];
    for chunk in mask.chunks_exact_mut(4) {
        chunk[0] = 1;
    }
    Ok(mask)
}

pub(super) fn build_preproc_mask(
    backend: &dyn VyreBackend,
    path: &Path,
    source: &str,
    lexed: &lex_stage::ObjectLexTokens,
    dcfg: &mut DispatchConfig,
    trace: &mut trace::CompileTrace,
) -> Result<Vec<u8>, String> {
    PREPROC_MASK_SCRATCH.with(|scratch| {
        let mut scratch = scratch.try_borrow_mut().map_err(|_| {
            "preprocessor mask dispatch scratch was re-entered on the same thread. Fix: call mask construction from a non-nested compile-unit context or add explicit caller-owned scratch.".to_string()
        })?;
        build_preproc_mask_with_scratch(backend, path, source, lexed, dcfg, trace, &mut scratch)
    })
}

fn build_preproc_mask_with_scratch(
    backend: &dyn VyreBackend,
    path: &Path,
    source: &str,
    lexed: &lex_stage::ObjectLexTokens,
    dcfg: &mut DispatchConfig,
    trace: &mut trace::CompileTrace,
    scratch: &mut PreprocMaskScratch,
) -> Result<Vec<u8>, String> {
    let types_prefix_len = usize::try_from(lexed.n_tokens.max(1))
        .ok()
        .and_then(|count| count.checked_mul(4))
        .ok_or_else(|| {
            format!(
                "preprocessor mask token prefix byte length overflows host indexing for n_tokens={}. Fix: shard the token stream before mask construction.",
                lexed.n_tokens
            )
        })?;
    if types_prefix_len > lexed.types.len() {
        return Err(format!(
            "preprocessor token types: need {types_prefix_len} bytes for {} u32 words, have {}",
            lexed.n_tokens.max(1),
            lexed.types.len()
        ));
    }
    let types_prefix = &lexed.types[..types_prefix_len];
    if source.as_bytes().contains(&b'#') {
        scratch.mask_init.clear();
        scratch.mask_init.resize(types_prefix_len, 0);
        dcfg.label = Some(format!("vyre-frontend-c cpp-mask {}", path.display()));
        let mask_key =
            stage_pipeline_cache_key("opt_conditional_mask", &[lexed.n_tokens.max(1) as u64]);
        let inputs = [types_prefix, scratch.mask_init.as_slice()];
        dispatch_borrowed_stage_cached_into(
            backend,
            mask_key,
            || {
                let mask_prog =
                    opt_conditional_mask("tok_types", "mask", Expr::u32(lexed.n_tokens.max(1)));
                validate_internal_stage(&mask_prog, "opt_conditional_mask")?;
                Ok(mask_prog)
            },
            &inputs,
            dcfg,
            &mut scratch.outputs,
        )
        .map_err(|error| format!("opt_conditional_mask dispatch failed: {error}"))?;
        trace.log("dispatch cpp-mask");
        if scratch.outputs.len() != 1 {
            return Err(format!(
                "opt_conditional_mask returned {} output buffer(s), expected exactly 1. Fix: backend must return only mask.",
                scratch.outputs.len()
            ));
        }
        let mut mask = Vec::new();
        mem::swap(&mut mask, &mut scratch.outputs[0]);
        return Ok(mask);
    }
    trace.log("skip cpp-mask; no directives");
    all_enabled_preproc_mask_bytes(lexed.n_tokens)
}
