use std::cell::RefCell;
use std::mem;

use super::*;

#[derive(Default)]
struct RawVastScratch {
    last_child_scratch: Vec<u8>,
    stack_scratch: Vec<u8>,
    outputs: Vec<Vec<u8>>,
}

thread_local! {
    static RAW_VAST_SCRATCH: RefCell<RawVastScratch> =
        RefCell::new(RawVastScratch::default());
}

pub(super) fn build_raw_vast(
    backend: &dyn VyreBackend,
    path: &Path,
    tok_types_bytes: &[u8],
    starts: &[u8],
    lens: &[u8],
    nt: u32,
    cfg: &mut DispatchConfig,
    log: &mut impl FnMut(&str),
) -> Result<(Vec<u8>, u32), String> {
    RAW_VAST_SCRATCH.with(|scratch| {
        let mut scratch = scratch.try_borrow_mut().map_err(|_| {
            "raw VAST dispatch scratch was re-entered on the same thread. Fix: call VAST construction from a non-nested parser context or add explicit caller-owned scratch.".to_string()
        })?;
        build_raw_vast_with_scratch(
            backend,
            path,
            tok_types_bytes,
            starts,
            lens,
            nt,
            cfg,
            log,
            &mut scratch,
        )
    })
}

fn build_raw_vast_with_scratch(
    backend: &dyn VyreBackend,
    path: &Path,
    tok_types_bytes: &[u8],
    starts: &[u8],
    lens: &[u8],
    nt: u32,
    cfg: &mut DispatchConfig,
    log: &mut impl FnMut(&str),
    scratch: &mut RawVastScratch,
) -> Result<(Vec<u8>, u32), String> {
    cfg.label = Some(format!("vyre-frontend-c vast {}", path.display()));
    let vast_key = super::stage_pipeline_cache_key("c11_build_vast_nodes_v2", &[nt as u64]);
    let use_global_last_child = c11_build_vast_nodes_uses_global_last_child(nt);
    if use_global_last_child {
        let scratch_len = usize::try_from(nt.max(1))
            .ok()
            .and_then(|count| count.checked_mul(4))
            .ok_or_else(|| {
                format!(
                    "c11_build_vast_nodes scratch length overflows host indexing for token_count={nt}. Fix: shard VAST construction before GPU dispatch."
                )
            })?;
        scratch.last_child_scratch.clear();
        scratch.last_child_scratch.resize(scratch_len, 0);
        scratch.stack_scratch.clear();
        scratch.stack_scratch.resize(scratch_len, 0);
    }
    let inputs3 = [tok_types_bytes, starts, lens];
    let inputs5 = [
        tok_types_bytes,
        starts,
        lens,
        scratch.last_child_scratch.as_slice(),
        scratch.stack_scratch.as_slice(),
    ];
    let vast_inputs: &[&[u8]] = if use_global_last_child {
        &inputs5
    } else {
        &inputs3
    };
    scratch.outputs.clear();
    super::dispatch_borrowed_stage_cached_into(
        backend,
        vast_key,
        || {
            let vast_prog = c11_build_vast_nodes(
                "tok_types",
                "tok_starts",
                "tok_lens",
                Expr::u32(nt),
                "out_vast_nodes",
                "out_vast_count",
            );
            let vast_prog = super::buffers::mark_program_outputs(
                vast_prog,
                &["out_vast_nodes", "out_vast_count"],
            );
            let vast_prog =
                super::buffers::suppress_readwrite_readback(vast_prog, &["out_vast_count"]);
            super::validate_internal_stage(&vast_prog, "c11_build_vast_nodes")?;
            Ok(vast_prog)
        },
        vast_inputs,
        cfg,
        &mut scratch.outputs,
    )
    .map_err(|error| format!("c11_build_vast_nodes dispatch failed: {error}"))?;
    super::buffers::drop_suppressed_readbacks(&mut scratch.outputs);
    log("dispatch c11_build_vast_nodes");
    if scratch.outputs.len() != 1 {
        return Err(format!(
            "c11_build_vast_nodes returned {} output buffer(s), expected exactly 1. Fix: repair stage output marking or backend readback routing.",
            scratch.outputs.len()
        ));
    }
    let mut vast = Vec::new();
    mem::swap(&mut vast, &mut scratch.outputs[0]);
    Ok((vast, nt.max(1)))
}
