use std::cell::RefCell;
use std::mem;

use super::*;

#[derive(Default)]
struct VastScopesScratch {
    stack_scratch: Vec<u8>,
    outputs: Vec<Vec<u8>>,
}

thread_local! {
    static VAST_SCOPES_SCRATCH: RefCell<VastScopesScratch> =
        RefCell::new(VastScopesScratch::default());
}

pub(super) fn precompute_vast_scopes(
    backend: &dyn VyreBackend,
    path: &Path,
    hashed_vast_blob: Vec<u8>,
    vast_count: u32,
    global_typedef_fast_path: bool,
    cfg: &mut DispatchConfig,
    log: &mut impl FnMut(&str),
) -> Result<Vec<u8>, String> {
    VAST_SCOPES_SCRATCH.with(|scratch| {
        let mut scratch = scratch.try_borrow_mut().map_err(|_| {
            "VAST scope dispatch scratch was re-entered on the same thread. Fix: call VAST scope precompute from a non-nested parser context or add explicit caller-owned scratch.".to_string()
        })?;
        precompute_vast_scopes_with_scratch(
            backend,
            path,
            hashed_vast_blob,
            vast_count,
            global_typedef_fast_path,
            cfg,
            log,
            &mut scratch,
        )
    })
}

fn precompute_vast_scopes_with_scratch(
    backend: &dyn VyreBackend,
    path: &Path,
    hashed_vast_blob: Vec<u8>,
    vast_count: u32,
    global_typedef_fast_path: bool,
    cfg: &mut DispatchConfig,
    log: &mut impl FnMut(&str),
    scratch: &mut VastScopesScratch,
) -> Result<Vec<u8>, String> {
    if global_typedef_fast_path {
        log("skip c11_precompute_vast_scopes");
        return Ok(hashed_vast_blob);
    }
    cfg.label = Some(format!("vyre-frontend-c vast-scopes {}", path.display()));
    let scope_key =
        super::stage_pipeline_cache_key("c11_precompute_vast_scopes", &[vast_count.max(1) as u64]);
    let use_global_stack = c11_precompute_vast_scopes_uses_global_stack(vast_count.max(1));
    if use_global_stack {
        let scratch_len = usize::try_from(vast_count.max(1))
            .ok()
            .and_then(|count| count.checked_mul(4))
            .ok_or_else(|| {
                format!(
                    "c11_precompute_vast_scopes scratch length overflows host indexing for vast_count={vast_count}. Fix: shard VAST scope construction before GPU dispatch."
                )
            })?;
        scratch.stack_scratch.clear();
        scratch.stack_scratch.resize(scratch_len, 0);
    }
    let inputs1 = [hashed_vast_blob.as_slice()];
    let inputs2 = [
        hashed_vast_blob.as_slice(),
        scratch.stack_scratch.as_slice(),
    ];
    let scope_inputs: &[&[u8]] = if use_global_stack { &inputs2 } else { &inputs1 };
    scratch.outputs.clear();
    super::dispatch_borrowed_stage_cached_into(
        backend,
        scope_key,
        || {
            let scope_prog = c11_precompute_vast_scopes(
                "vast_nodes",
                Expr::u32(vast_count.max(1)),
                "scoped_vast",
            );
            let scope_prog = super::buffers::mark_program_outputs(scope_prog, &["scoped_vast"]);
            super::validate_internal_stage(&scope_prog, "c11_precompute_vast_scopes")?;
            Ok(scope_prog)
        },
        scope_inputs,
        cfg,
        &mut scratch.outputs,
    )
    .map_err(|error| {
        format!(
            "c11_precompute_vast_scopes dispatch failed for vast_count={} input_bytes={} global_stack={} scratch_bytes={}: {error}",
            vast_count.max(1),
            hashed_vast_blob.len(),
            use_global_stack,
            scratch.stack_scratch.len()
        )
    })?;
    super::buffers::drop_suppressed_readbacks(&mut scratch.outputs);
    log("dispatch c11_precompute_vast_scopes");
    if scratch.outputs.len() != 1 {
        return Err(format!(
            "c11_precompute_vast_scopes returned {} output buffer(s), expected exactly 1. Fix: backend must return only scoped_vast.",
            scratch.outputs.len()
        ));
    }
    let mut scoped = Vec::new();
    mem::swap(&mut scoped, &mut scratch.outputs[0]);
    Ok(scoped)
}
