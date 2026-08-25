use std::cell::RefCell;
use std::mem;

use super::*;

const VAST_DECL_CONTEXT_STRIDE_U32: usize = 4;

#[derive(Default)]
struct DeclContextScratch {
    outputs: Vec<Vec<u8>>,
}

thread_local! {
    static DECL_CONTEXT_SCRATCH: RefCell<DeclContextScratch> =
        RefCell::new(DeclContextScratch::default());
}

pub(super) fn precompute_decl_contexts(
    backend: &dyn VyreBackend,
    path: &Path,
    scoped_vast_blob: &[u8],
    vast_count: u32,
    global_typedef_fast_path: bool,
    cfg: &mut DispatchConfig,
    log: &mut impl FnMut(&str),
) -> Result<Vec<u8>, String> {
    DECL_CONTEXT_SCRATCH.with(|scratch| {
        let mut scratch = scratch.try_borrow_mut().map_err(|_| {
            "VAST declaration-context dispatch scratch was re-entered on the same thread. Fix: call declaration-context precompute from a non-nested parser context or add explicit caller-owned scratch.".to_string()
        })?;
        precompute_decl_contexts_with_scratch(
            backend,
            path,
            scoped_vast_blob,
            vast_count,
            global_typedef_fast_path,
            cfg,
            log,
            &mut scratch,
        )
    })
}

fn precompute_decl_contexts_with_scratch(
    backend: &dyn VyreBackend,
    path: &Path,
    scoped_vast_blob: &[u8],
    vast_count: u32,
    global_typedef_fast_path: bool,
    cfg: &mut DispatchConfig,
    log: &mut impl FnMut(&str),
    scratch: &mut DeclContextScratch,
) -> Result<Vec<u8>, String> {
    cfg.label = Some(format!(
        "vyre-frontend-c vast-decl-contexts {}",
        path.display()
    ));
    let decl_context_key = super::stage_pipeline_cache_key(
        if global_typedef_fast_path {
            "c11_precompute_vast_decl_prefix_starts"
        } else {
            "c11_precompute_vast_decl_contexts"
        },
        &[
            vast_count.max(1) as u64,
            VAST_DECL_CONTEXT_STRIDE_U32 as u64,
            global_typedef_fast_path as u64,
        ],
    );
    let inputs = [scoped_vast_blob];
    scratch.outputs.clear();
    super::dispatch_borrowed_stage_cached_into(
        backend,
        decl_context_key,
        || {
            let context_prog = if global_typedef_fast_path {
                c11_precompute_vast_decl_prefix_starts(
                    "vast_nodes",
                    Expr::u32(vast_count.max(1)),
                    "decl_contexts",
                )
            } else {
                c11_precompute_vast_decl_contexts(
                    "vast_nodes",
                    Expr::u32(vast_count.max(1)),
                    "decl_contexts",
                )
            };
            let context_prog =
                super::buffers::mark_program_outputs(context_prog, &["decl_contexts"]);
            super::validate_internal_stage(
                &context_prog,
                if global_typedef_fast_path {
                    "c11_precompute_vast_decl_prefix_starts"
                } else {
                    "c11_precompute_vast_decl_contexts"
                },
            )?;
            Ok(context_prog)
        },
        &inputs,
        cfg,
        &mut scratch.outputs,
    )
    .map_err(|error| {
        if global_typedef_fast_path {
            format!("c11_precompute_vast_decl_prefix_starts dispatch failed: {error}")
        } else {
            format!("c11_precompute_vast_decl_contexts dispatch failed: {error}")
        }
    })?;
    if global_typedef_fast_path {
        log("dispatch c11_precompute_vast_decl_prefix_starts");
    } else {
        log("dispatch c11_precompute_vast_decl_contexts");
    }
    if scratch.outputs.len() != 1 {
        return Err(format!(
            "c11_precompute_vast_decl_contexts returned {} output buffer(s), expected exactly 1. Fix: backend must return only decl_contexts.",
            scratch.outputs.len()
        ));
    }
    let mut decl_contexts = Vec::new();
    mem::swap(&mut decl_contexts, &mut scratch.outputs[0]);
    Ok(decl_contexts)
}
