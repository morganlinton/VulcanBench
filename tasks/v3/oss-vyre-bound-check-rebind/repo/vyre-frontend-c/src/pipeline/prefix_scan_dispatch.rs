use std::mem;

use vyre::{DispatchConfig, VyreBackend};
use vyre_primitives::math::prefix_scan::{prefix_scan, ScanKind};
use vyre_primitives::reduce::multi_block_prefix_scan::{
    pass_a_local_scan, pass_c_broadcast_offsets, BLOCK_LANES,
};

use super::backend_select::{dispatch_borrowed_stage_cached_into, stage_pipeline_cache_key};
use super::buffers::take_last_output_into;
use super::validate_internal_stage;

#[derive(Default)]
pub(super) struct PrefixScanDispatchScratch {
    single_outputs: Vec<Vec<u8>>,
    pass_a_outputs: Vec<Vec<u8>>,
    pass_c_outputs: Vec<Vec<u8>>,
    partials: Vec<u8>,
    block_totals: Vec<u8>,
    scanned_block_totals: Vec<u8>,
    block_totals_scratch: Option<Box<PrefixScanDispatchScratch>>,
}

pub(super) fn dispatch_borrowed_prefix_scan_u32_into(
    backend: &dyn VyreBackend,
    input_words_le: &[u8],
    n: u32,
    config: &mut DispatchConfig,
    label: &str,
    stage_name: &str,
    output: &mut Vec<u8>,
    scratch: &mut PrefixScanDispatchScratch,
) -> Result<(), String> {
    if n > BLOCK_LANES {
        return dispatch_borrowed_prefix_scan_u32_large_into(
            backend,
            input_words_le,
            n,
            config,
            label,
            stage_name,
            output,
            scratch,
        );
    }

    config.label = Some(format!("{label} {stage_name}"));
    let scan_key = stage_pipeline_cache_key(stage_name, &[n as u64]);
    dispatch_borrowed_stage_cached_into(
        backend,
        scan_key,
        || {
            let scan = prefix_scan("scan_in", "scan_out", n.max(1), ScanKind::InclusiveSum);
            validate_internal_stage(&scan, stage_name)?;
            Ok(scan)
        },
        &[input_words_le],
        config,
        &mut scratch.single_outputs,
    )
    .map_err(|e| format!("{label} {stage_name} dispatch failed: {e}"))?;
    take_last_output_into(&mut scratch.single_outputs, output, || {
        format!("{label} {stage_name}: missing scan output")
    })
}

fn dispatch_borrowed_prefix_scan_u32_large_into(
    backend: &dyn VyreBackend,
    input_words_le: &[u8],
    n: u32,
    config: &mut DispatchConfig,
    label: &str,
    stage_name: &str,
    output: &mut Vec<u8>,
    scratch: &mut PrefixScanDispatchScratch,
) -> Result<(), String> {
    let num_blocks = n.div_ceil(BLOCK_LANES);
    let pass_a_stage = format!("{stage_name}:pass-a");
    config.label = Some(format!("{label} {pass_a_stage}"));
    let pass_a_key = stage_pipeline_cache_key(&pass_a_stage, &[n as u64, num_blocks as u64]);
    dispatch_borrowed_stage_cached_into(
        backend,
        pass_a_key,
        || {
            let pass_a = pass_a_local_scan(
                "scan_in",
                "scan_partials",
                "scan_block_totals",
                n,
                num_blocks,
            );
            validate_internal_stage(&pass_a, &pass_a_stage)?;
            Ok(pass_a)
        },
        &[input_words_le],
        config,
        &mut scratch.pass_a_outputs,
    )
    .map_err(|e| format!("{label} {pass_a_stage} dispatch failed: {e}"))?;
    if scratch.pass_a_outputs.len() != 2 {
        return Err(format!(
            "{label} {pass_a_stage}: expected exactly partials and block totals, got {}. Fix: backend must return the declared prefix-scan pass outputs and no extras.",
            scratch.pass_a_outputs.len()
        ));
    }
    mem::swap(&mut scratch.partials, &mut scratch.pass_a_outputs[0]);
    mem::swap(&mut scratch.block_totals, &mut scratch.pass_a_outputs[1]);

    let block_stage = format!("{stage_name}:block-totals");
    {
        let PrefixScanDispatchScratch {
            block_totals,
            scanned_block_totals,
            block_totals_scratch,
            ..
        } = scratch;
        let nested_scratch = block_totals_scratch
            .get_or_insert_with(|| Box::new(PrefixScanDispatchScratch::default()));
        dispatch_borrowed_prefix_scan_u32_into(
            backend,
            block_totals,
            num_blocks,
            config,
            label,
            &block_stage,
            scanned_block_totals,
            nested_scratch,
        )?;
    }

    let pass_c_stage = format!("{stage_name}:pass-c");
    config.label = Some(format!("{label} {pass_c_stage}"));
    let pass_c_key = stage_pipeline_cache_key(&pass_c_stage, &[n as u64, num_blocks as u64]);
    dispatch_borrowed_stage_cached_into(
        backend,
        pass_c_key,
        || {
            let pass_c = pass_c_broadcast_offsets(
                "scan_partials",
                "scan_block_totals_scanned",
                "scan_out",
                n,
                num_blocks,
            );
            validate_internal_stage(&pass_c, &pass_c_stage)?;
            Ok(pass_c)
        },
        &[
            scratch.partials.as_slice(),
            scratch.scanned_block_totals.as_slice(),
        ],
        config,
        &mut scratch.pass_c_outputs,
    )
    .map_err(|e| format!("{label} {pass_c_stage} dispatch failed: {e}"))?;
    take_last_output_into(&mut scratch.pass_c_outputs, output, || {
        format!("{label} {pass_c_stage}: missing scan output")
    })
}
