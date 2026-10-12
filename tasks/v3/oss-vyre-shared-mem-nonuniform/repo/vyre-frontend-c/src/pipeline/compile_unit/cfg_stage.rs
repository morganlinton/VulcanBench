use std::cell::RefCell;
use std::mem;

use super::*;

const CFG_LABEL_MAP_WORDS: usize = 4096;
const CFG_LABEL_MAP_BYTES: usize = CFG_LABEL_MAP_WORDS * 4;
const EMPTY_CFG_PUBLIC_OUTPUT_BYTES: usize = 2 * 4;

fn cfg_label_map_zeroes() -> &'static [u8] {
    static ZEROES: OnceLock<Vec<u8>> = OnceLock::new();
    ZEROES
        .get_or_init(|| vec![0u8; CFG_LABEL_MAP_BYTES])
        .as_slice()
}

#[derive(Default)]
struct ObjectCfgScratch {
    ssa_buf: Vec<u8>,
    cfg_init: Vec<u8>,
    lbl_init: Vec<u8>,
    outputs: Vec<Vec<u8>>,
    cfg_blob: Vec<u8>,
}

thread_local! {
    static OBJECT_CFG_SCRATCH: RefCell<ObjectCfgScratch> =
        RefCell::new(ObjectCfgScratch::default());
}

pub(super) fn build_object_cfg(
    backend: &dyn VyreBackend,
    path: &Path,
    vast_blob: &[u8],
    dcfg: &mut DispatchConfig,
    trace: &mut trace::CompileTrace,
) -> Result<Vec<u8>, String> {
    OBJECT_CFG_SCRATCH.with(|scratch| {
        let mut scratch = scratch.try_borrow_mut().map_err(|_| {
            "object CFG dispatch scratch was re-entered on the same thread. Fix: call CFG construction from a non-nested compile-unit context or add explicit caller-owned scratch.".to_string()
        })?;
        build_object_cfg_with_scratch(backend, path, vast_blob, dcfg, trace, &mut scratch)
    })
}

fn build_object_cfg_with_scratch(
    backend: &dyn VyreBackend,
    path: &Path,
    vast_blob: &[u8],
    dcfg: &mut DispatchConfig,
    trace: &mut trace::CompileTrace,
    scratch: &mut ObjectCfgScratch,
) -> Result<Vec<u8>, String> {
    let cfg_ssa = cfg_ssa_words_from_vast(vast_blob)?;
    if cfg_ssa.as_slice() == [0] {
        trace.log("skip c11_build_cfg_and_gotos; no labels or gotos");
        return Ok(vec![0u8; EMPTY_CFG_PUBLIC_OUTPUT_BYTES]);
    }
    let n_ssa = u32::try_from(cfg_ssa.len())
        .map_err(|_| "CFG SSA stream exceeds u32 count".to_string())?
        .max(1);
    vyre_primitives::wire::pack_u32_slice_into(&cfg_ssa, &mut scratch.ssa_buf);
    let cfg_label_byte_len = usize::try_from(n_ssa)
        .ok()
        .and_then(|count| count.checked_mul(4))
        .ok_or_else(|| {
            format!(
                "c11_build_cfg_and_gotos init buffer length overflows host indexing for n_ssa={n_ssa}. Fix: shard CFG construction before GPU dispatch."
            )
        })?;
    scratch.cfg_init.clear();
    scratch.cfg_init.resize(cfg_label_byte_len, 0);
    scratch.lbl_init.clear();
    scratch.lbl_init.resize(cfg_label_byte_len, 0);
    dcfg.label = Some(format!("vyre-frontend-c cfg {}", path.display()));
    let cfg_key = stage_pipeline_cache_key("c11_build_cfg_and_gotos", &[n_ssa as u64]);
    let inputs = [
        scratch.ssa_buf.as_slice(),
        scratch.cfg_init.as_slice(),
        scratch.lbl_init.as_slice(),
        cfg_label_map_zeroes(),
        cfg_label_map_zeroes(),
    ];
    dispatch_borrowed_stage_cached_into(
        backend,
        cfg_key,
        || {
            let cfg_prog = c11_build_cfg_and_gotos("ssa", "cfg", "labels", Expr::u32(n_ssa));
            validate_internal_stage(&cfg_prog, "c11_build_cfg_and_gotos")?;
            Ok(cfg_prog)
        },
        &inputs,
        dcfg,
        &mut scratch.outputs,
    )
    .map_err(|error| format!("c11_build_cfg_and_gotos dispatch failed: {error}"))?;
    trace.log("dispatch c11_build_cfg_and_gotos");
    if scratch.outputs.len() < 2 {
        return Err(format!(
            "c11_build_cfg_and_gotos: expected at least cfg/labels outputs, got {}. Fix: backend must return the declared GPU CFG ABI outputs.",
            scratch.outputs.len()
        ));
    }
    scratch.cfg_blob.clear();
    scratch
        .cfg_blob
        .reserve(scratch.outputs[..2].iter().map(Vec::len).sum());
    for chunk in &scratch.outputs[..2] {
        scratch.cfg_blob.extend_from_slice(chunk);
    }
    let mut cfg_blob = Vec::new();
    mem::swap(&mut cfg_blob, &mut scratch.cfg_blob);
    Ok(cfg_blob)
}
