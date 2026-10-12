use std::cell::RefCell;
use std::mem;

use super::*;

pub(in crate::pipeline) struct C11AbiStage {
    pub(in crate::pipeline) outputs: Vec<Vec<u8>>,
    pub(in crate::pipeline) byte_len: u64,
}

#[derive(Default)]
struct AbiStageScratch {
    type_defs: Vec<u8>,
    outputs: Vec<Vec<u8>>,
}

thread_local! {
    static ABI_STAGE_SCRATCH: RefCell<AbiStageScratch> =
        RefCell::new(AbiStageScratch::default());
}

pub(in crate::pipeline) fn build_c11_abi_stage(
    backend: &dyn VyreBackend,
    target_abi: CTargetAbi,
    tok_types: &[u32],
    dcfg: &mut DispatchConfig,
    dispatch_label: &str,
    mut log: impl FnMut(&str),
) -> Result<C11AbiStage, String> {
    ABI_STAGE_SCRATCH.with(|scratch| {
        let mut scratch = scratch.try_borrow_mut().map_err(|_| {
            "ABI stage scratch was re-entered on the same thread. Fix: call ABI layout from a non-nested parser context or add explicit caller-owned scratch.".to_string()
        })?;
        build_c11_abi_stage_with_scratch(
            backend,
            target_abi,
            tok_types,
            dcfg,
            dispatch_label,
            &mut log,
            &mut scratch,
        )
    })
}

fn build_c11_abi_stage_with_scratch(
    backend: &dyn VyreBackend,
    target_abi: CTargetAbi,
    tok_types: &[u32],
    dcfg: &mut DispatchConfig,
    dispatch_label: &str,
    mut log: impl FnMut(&str),
    scratch: &mut AbiStageScratch,
) -> Result<C11AbiStage, String> {
    c_abi_type_table_bytes_into(tok_types, &mut scratch.type_defs);
    let type_count = u32::try_from(scratch.type_defs.len() / 4)
        .map_err(|_| "ABI type table exceeds u32 count".to_string())?
        .max(1);
    let pointer_size = target_abi.pointer_size_bytes();
    let long_size = target_abi.long_size_bytes();
    let double_align = target_abi.double_alignment_bytes();
    let abi_key = stage_pipeline_cache_key(
        "c11_compute_alignments",
        &[
            type_count as u64,
            pointer_size as u64,
            long_size as u64,
            double_align as u64,
        ],
    );
    dcfg.label = Some(dispatch_label.to_string());
    let inputs = [scratch.type_defs.as_slice()];
    dispatch_borrowed_stage_cached_into(
        backend,
        abi_key,
        || {
            let align_prog = c11_compute_alignments_for_abi(
                "types",
                "sizes",
                "aligns",
                Expr::u32(type_count),
                pointer_size,
                long_size,
                double_align,
            );
            let align_prog = mark_program_outputs(align_prog, &["sizes", "aligns"]);
            validate_internal_stage(&align_prog, "c11_compute_alignments")?;
            Ok(align_prog)
        },
        &inputs,
        dcfg,
        &mut scratch.outputs,
    )
    .map_err(|error| format!("{dispatch_label} c11_compute_alignments dispatch failed: {error}"))?;
    log("dispatch c11_compute_alignments");
    if scratch.outputs.len() != 2 {
        return Err(format!(
            "c11_compute_alignments returned {} output buffer(s), expected exactly 2. Fix: backend must return sizes and aligns.",
            scratch.outputs.len()
        ));
    }
    let byte_len = scratch.outputs.iter().try_fold(0u64, |acc, output| {
        let len = u64::try_from(output.len()).map_err(|_| {
            "ABI layout output length exceeds u64. Fix: shard ABI layout outputs.".to_string()
        })?;
        acc.checked_add(len).ok_or_else(|| {
            "ABI layout byte accounting overflowed. Fix: shard ABI layout outputs.".to_string()
        })
    })?;
    let mut outputs = Vec::new();
    mem::swap(&mut outputs, &mut scratch.outputs);
    Ok(C11AbiStage { outputs, byte_len })
}
