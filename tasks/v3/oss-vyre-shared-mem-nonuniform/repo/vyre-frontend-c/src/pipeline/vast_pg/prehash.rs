use std::cell::RefCell;
use std::mem;

use super::*;

#[derive(Default)]
struct PrehashVastScratch {
    outputs: Vec<Vec<u8>>,
}

thread_local! {
    static PREHASH_VAST_SCRATCH: RefCell<PrehashVastScratch> =
        RefCell::new(PrehashVastScratch::default());
}

pub(super) fn prehash_vast_identifiers(
    backend: &dyn VyreBackend,
    path: &Path,
    raw_vast_blob: &[u8],
    haystack: &[u8],
    haystack_len: u32,
    vast_count: u32,
    packed_haystack: bool,
    cfg: &mut DispatchConfig,
    log: &mut impl FnMut(&str),
) -> Result<Vec<u8>, String> {
    PREHASH_VAST_SCRATCH.with(|scratch| {
        let mut scratch = scratch.try_borrow_mut().map_err(|_| {
            "VAST prehash dispatch scratch was re-entered on the same thread. Fix: call VAST prehashing from a non-nested parser context or add explicit caller-owned scratch.".to_string()
        })?;
        prehash_vast_identifiers_with_scratch(
            backend,
            path,
            raw_vast_blob,
            haystack,
            haystack_len,
            vast_count,
            packed_haystack,
            cfg,
            log,
            &mut scratch,
        )
    })
}

fn prehash_vast_identifiers_with_scratch(
    backend: &dyn VyreBackend,
    path: &Path,
    raw_vast_blob: &[u8],
    haystack: &[u8],
    haystack_len: u32,
    vast_count: u32,
    packed_haystack: bool,
    cfg: &mut DispatchConfig,
    log: &mut impl FnMut(&str),
    scratch: &mut PrehashVastScratch,
) -> Result<Vec<u8>, String> {
    cfg.label = Some(format!("vyre-frontend-c vast-prehash {}", path.display()));
    let prehash_key = super::stage_pipeline_cache_key(
        "c11_prehash_vast_identifiers",
        &[
            haystack_len.max(1) as u64,
            vast_count.max(1) as u64,
            packed_haystack as u64,
        ],
    );
    scratch.outputs.clear();
    super::dispatch_borrowed_stage_cached_into(
        backend,
        prehash_key,
        || {
            let prehash_prog = if packed_haystack {
                c11_prehash_vast_identifiers_packed_haystack(
                    "vast_nodes",
                    "haystack",
                    Expr::u32(haystack_len.max(1)),
                    Expr::u32(vast_count.max(1)),
                    "hashed_vast",
                )
            } else {
                c11_prehash_vast_identifiers(
                    "vast_nodes",
                    "haystack",
                    Expr::u32(haystack_len.max(1)),
                    Expr::u32(vast_count.max(1)),
                    "hashed_vast",
                )
            };
            let prehash_prog = super::buffers::mark_program_outputs(prehash_prog, &["hashed_vast"]);
            super::validate_internal_stage(&prehash_prog, "c11_prehash_vast_identifiers")?;
            Ok(prehash_prog)
        },
        &[raw_vast_blob, haystack],
        cfg,
        &mut scratch.outputs,
    )
    .map_err(|error| format!("c11_prehash_vast_identifiers dispatch failed: {error}"))?;
    log("dispatch c11_prehash_vast_identifiers");
    if scratch.outputs.len() != 1 {
        return Err(format!(
            "c11_prehash_vast_identifiers returned {} output buffer(s), expected exactly 1. Fix: backend must return only hashed_vast.",
            scratch.outputs.len()
        ));
    }
    let mut hashed = Vec::new();
    mem::swap(&mut hashed, &mut scratch.outputs[0]);
    Ok(hashed)
}
