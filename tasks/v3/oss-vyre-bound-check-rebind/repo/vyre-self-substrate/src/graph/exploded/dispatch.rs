use super::{CachedIfdsCsrProgram, IfdsCsrGpuScratch};
use vyre_primitives::graph::exploded::{
    canonicalize_csr_within_rows_in_place as primitive_canonicalize_csr_within_rows_in_place,
    plan_ifds_csr_dispatch, validate_ifds_csr_readback, IfdsCsrDispatchPlan,
    IfdsCsrRuleInputFingerprint, IfdsCsrStaticInputKey, IFDS_CSR_COL_IDX_BUFFER,
    IFDS_CSR_COL_LEN_BUFFER, IFDS_CSR_GEN_BLOCK_BUFFER, IFDS_CSR_GEN_FACT_BUFFER,
    IFDS_CSR_GEN_PROC_BUFFER, IFDS_CSR_INTER_DST_BLOCK_BUFFER, IFDS_CSR_INTER_DST_PROC_BUFFER,
    IFDS_CSR_INTER_SRC_BLOCK_BUFFER, IFDS_CSR_INTER_SRC_PROC_BUFFER,
    IFDS_CSR_INTRA_DST_BLOCK_BUFFER, IFDS_CSR_INTRA_PROC_BUFFER, IFDS_CSR_INTRA_SRC_BLOCK_BUFFER,
    IFDS_CSR_KILLED_BUFFER, IFDS_CSR_KILL_BLOCK_BUFFER, IFDS_CSR_KILL_FACT_BUFFER,
    IFDS_CSR_KILL_PROC_BUFFER, IFDS_CSR_ROW_CURSOR_BUFFER, IFDS_CSR_ROW_PTR_BUFFER,
};

use crate::dispatch_buffers::{decode_u32_output_exact, u32_word_bytes};
use crate::graph::dispatch_bridge::{refresh_keyed_dispatch_inputs, DispatchInput};
use crate::optimizer::dispatcher::{DispatchError, OptimizerDispatcher};

/// GPU dispatch wrapper around [`reference_build_ifds_csr`].
///
/// Returns the supergraph CSR in canonical (within-row sorted) form
/// so callers comparing against the reference oracle don't need to
/// re-canonicalise the output.
///
/// # Errors
///
/// Propagates dispatch failures and rejects dimensions or readback
/// shapes that cannot represent an exploded CSR safely.
#[allow(clippy::too_many_arguments)]
pub fn build_ifds_csr_via(
    dispatcher: &dyn OptimizerDispatcher,
    num_procs: u32,
    blocks_per_proc: u32,
    facts_per_proc: u32,
    intra_edges: &[(u32, u32, u32)],
    inter_edges: &[(u32, u32, u32, u32)],
    flow_gen: &[(u32, u32, u32)],
    flow_kill: &[(u32, u32, u32)],
) -> Result<(Vec<u32>, Vec<u32>), DispatchError> {
    let mut row_ptr = Vec::new();
    let mut col_idx = Vec::new();
    build_ifds_csr_via_into(
        dispatcher,
        num_procs,
        blocks_per_proc,
        facts_per_proc,
        intra_edges,
        inter_edges,
        flow_gen,
        flow_kill,
        &mut row_ptr,
        &mut col_idx,
    )?;
    Ok((row_ptr, col_idx))
}

/// GPU dispatch wrapper around [`reference_build_ifds_csr`] into caller-owned CSR buffers.
#[allow(clippy::too_many_arguments)]
pub fn build_ifds_csr_via_into(
    dispatcher: &dyn OptimizerDispatcher,
    num_procs: u32,
    blocks_per_proc: u32,
    facts_per_proc: u32,
    intra_edges: &[(u32, u32, u32)],
    inter_edges: &[(u32, u32, u32, u32)],
    flow_gen: &[(u32, u32, u32)],
    flow_kill: &[(u32, u32, u32)],
    row_ptr_out: &mut Vec<u32>,
    col_idx_out: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    let mut scratch = IfdsCsrGpuScratch::default();
    build_ifds_csr_via_with_scratch_into(
        dispatcher,
        num_procs,
        blocks_per_proc,
        facts_per_proc,
        intra_edges,
        inter_edges,
        flow_gen,
        flow_kill,
        &mut scratch,
        row_ptr_out,
        col_idx_out,
    )
}

/// GPU dispatch wrapper around [`reference_build_ifds_csr`] into caller-owned
/// dispatch scratch and CSR buffers.
#[allow(clippy::too_many_arguments)]
pub fn build_ifds_csr_via_with_scratch_into(
    dispatcher: &dyn OptimizerDispatcher,
    num_procs: u32,
    blocks_per_proc: u32,
    facts_per_proc: u32,
    intra_edges: &[(u32, u32, u32)],
    inter_edges: &[(u32, u32, u32, u32)],
    flow_gen: &[(u32, u32, u32)],
    flow_kill: &[(u32, u32, u32)],
    scratch: &mut IfdsCsrGpuScratch,
    row_ptr_out: &mut Vec<u32>,
    col_idx_out: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    let plan = plan_ifds_csr_dispatch(
        num_procs,
        blocks_per_proc,
        facts_per_proc,
        intra_edges,
        inter_edges,
        flow_gen,
        flow_kill,
    )
    .map_err(DispatchError::BadInputs)?;
    if plan.layout.empty {
        row_ptr_out.clear();
        row_ptr_out.push(0);
        col_idx_out.clear();
        return Ok(());
    }
    let rule_fingerprint =
        IfdsCsrRuleInputFingerprint::from_rules(intra_edges, inter_edges, flow_gen, flow_kill);
    if scratch.rule_fingerprint != Some(rule_fingerprint) {
        scratch
            .rule_columns
            .prepare(intra_edges, inter_edges, flow_gen, flow_kill)
            .map_err(|error| {
                DispatchError::BackendError(format!(
                    "Fix: exploded IFDS wrapper could not marshal primitive rule columns: {error}"
                ))
            })?;
        scratch.rule_fingerprint = Some(rule_fingerprint);
    }
    let rule_columns = &scratch.rule_columns;
    let static_input_key = plan.static_input_key(rule_fingerprint);
    refresh_ifds_csr_inputs(
        &mut scratch.inputs,
        &mut scratch.static_input_key,
        static_input_key,
        &plan,
        &[
            DispatchInput::u32_slice_or_zero_words(
                &rule_columns.intra_proc,
                plan.intra_field_words,
                IFDS_CSR_INTRA_PROC_BUFFER,
            ),
            DispatchInput::u32_slice_or_zero_words(
                &rule_columns.intra_src_block,
                plan.intra_field_words,
                IFDS_CSR_INTRA_SRC_BLOCK_BUFFER,
            ),
            DispatchInput::u32_slice_or_zero_words(
                &rule_columns.intra_dst_block,
                plan.intra_field_words,
                IFDS_CSR_INTRA_DST_BLOCK_BUFFER,
            ),
            DispatchInput::u32_slice_or_zero_words(
                &rule_columns.inter_src_proc,
                plan.inter_field_words,
                IFDS_CSR_INTER_SRC_PROC_BUFFER,
            ),
            DispatchInput::u32_slice_or_zero_words(
                &rule_columns.inter_src_block,
                plan.inter_field_words,
                IFDS_CSR_INTER_SRC_BLOCK_BUFFER,
            ),
            DispatchInput::u32_slice_or_zero_words(
                &rule_columns.inter_dst_proc,
                plan.inter_field_words,
                IFDS_CSR_INTER_DST_PROC_BUFFER,
            ),
            DispatchInput::u32_slice_or_zero_words(
                &rule_columns.inter_dst_block,
                plan.inter_field_words,
                IFDS_CSR_INTER_DST_BLOCK_BUFFER,
            ),
            DispatchInput::u32_slice_or_zero_words(
                &rule_columns.gen_proc,
                plan.gen_field_words,
                IFDS_CSR_GEN_PROC_BUFFER,
            ),
            DispatchInput::u32_slice_or_zero_words(
                &rule_columns.gen_block,
                plan.gen_field_words,
                IFDS_CSR_GEN_BLOCK_BUFFER,
            ),
            DispatchInput::u32_slice_or_zero_words(
                &rule_columns.gen_fact,
                plan.gen_field_words,
                IFDS_CSR_GEN_FACT_BUFFER,
            ),
            DispatchInput::u32_slice_or_zero_words(
                &rule_columns.kill_proc,
                plan.kill_field_words,
                IFDS_CSR_KILL_PROC_BUFFER,
            ),
            DispatchInput::u32_slice_or_zero_words(
                &rule_columns.kill_block,
                plan.kill_field_words,
                IFDS_CSR_KILL_BLOCK_BUFFER,
            ),
            DispatchInput::u32_slice_or_zero_words(
                &rule_columns.kill_fact,
                plan.kill_field_words,
                IFDS_CSR_KILL_FACT_BUFFER,
            ),
            DispatchInput::zero_u32_words(plan.killed_words, IFDS_CSR_KILLED_BUFFER),
            DispatchInput::zero_u32_words(plan.row_ptr_words, IFDS_CSR_ROW_PTR_BUFFER),
            DispatchInput::zero_u32_words(plan.row_cursor_words, IFDS_CSR_ROW_CURSOR_BUFFER),
            DispatchInput::zero_u32_words(plan.col_idx_words, IFDS_CSR_COL_IDX_BUFFER),
            DispatchInput::zero_u32_words(plan.col_len_words, IFDS_CSR_COL_LEN_BUFFER),
        ],
    )?;
    let cached = scratch
        .program_cache
        .get_or_insert_with(plan.program_cache_key(), || CachedIfdsCsrProgram {
            program: plan.program(),
        });
    dispatch_ifds_csr_outputs_from_prepared_into(
        dispatcher,
        &cached.program,
        &scratch.inputs,
        &plan,
        plan.row_ptr_words,
        IFDS_CSR_ROW_PTR_BUFFER,
        row_ptr_out,
        plan.row_cursor_words,
        IFDS_CSR_ROW_CURSOR_BUFFER,
        &mut scratch.row_cursor,
        plan.col_idx_words,
        IFDS_CSR_COL_IDX_BUFFER,
        col_idx_out,
        plan.col_len_words,
        IFDS_CSR_COL_LEN_BUFFER,
        &mut scratch.col_len_words,
        Some(plan.grid),
    )?;
    let col_len = validate_ifds_csr_readback(
        &plan.layout,
        row_ptr_out,
        col_idx_out,
        scratch.col_len_words[0],
    )
    .map_err(DispatchError::BackendError)?;
    col_idx_out.truncate(col_len);
    canonicalize_csr_within_rows_in_place(row_ptr_out, col_idx_out)?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn dispatch_ifds_csr_outputs_from_prepared_into(
    dispatcher: &dyn OptimizerDispatcher,
    program: &vyre_foundation::ir::Program,
    scratch_inputs: &[Vec<u8>],
    plan: &IfdsCsrDispatchPlan,
    row_ptr_expected_words: usize,
    row_ptr_context: &str,
    row_ptr_out: &mut Vec<u32>,
    row_cursor_expected_words: usize,
    row_cursor_context: &str,
    row_cursor_out: &mut Vec<u32>,
    col_idx_expected_words: usize,
    col_idx_context: &str,
    col_idx_out: &mut Vec<u32>,
    col_len_expected_words: usize,
    col_len_context: &str,
    col_len_out: &mut Vec<u32>,
    grid_override: Option<[u32; 3]>,
) -> Result<(), DispatchError> {
    let outputs = dispatcher.dispatch(program, scratch_inputs, grid_override)?;
    let output_base = match outputs.len() {
        4 => 0,
        5 => {
            let expected_killed_bytes =
                u32_word_bytes(plan.killed_words, "exploded IFDS killed scratch")?;
            if outputs[0].len() != expected_killed_bytes {
                return Err(DispatchError::BackendError(format!(
                    "Fix: {IFDS_CSR_KILLED_BUFFER} expected {expected_killed_bytes} byte(s), got {}.",
                    outputs[0].len()
                )));
            }
            1
        }
        count => {
            return Err(DispatchError::BackendError(format!(
                "Fix: {row_ptr_context} expected four u32 output buffers or killed scratch plus four u32 output buffers, got {count}.",
            )));
        }
    };
    decode_u32_output_exact(
        &outputs[output_base],
        row_ptr_expected_words,
        row_ptr_context,
        row_ptr_out,
    )?;
    decode_u32_output_exact(
        &outputs[output_base + 1],
        row_cursor_expected_words,
        row_cursor_context,
        row_cursor_out,
    )?;
    decode_u32_output_exact(
        &outputs[output_base + 2],
        col_idx_expected_words,
        col_idx_context,
        col_idx_out,
    )?;
    decode_u32_output_exact(
        &outputs[output_base + 3],
        col_len_expected_words,
        col_len_context,
        col_len_out,
    )
}
fn canonicalize_csr_within_rows_in_place(
    row_ptr: &[u32],
    col_idx: &mut [u32],
) -> Result<(), DispatchError> {
    primitive_canonicalize_csr_within_rows_in_place(row_ptr, col_idx)
        .map_err(DispatchError::BackendError)
}

fn refresh_ifds_csr_inputs(
    inputs: &mut Vec<Vec<u8>>,
    static_input_key: &mut Option<IfdsCsrStaticInputKey>,
    next_static_input_key: IfdsCsrStaticInputKey,
    plan: &IfdsCsrDispatchPlan,
    all_inputs: &[DispatchInput<'_>],
) -> Result<(), DispatchError> {
    refresh_keyed_dispatch_inputs(
        inputs,
        static_input_key,
        next_static_input_key,
        all_inputs,
        &[
            (
                13,
                DispatchInput::zero_u32_words(plan.killed_words, IFDS_CSR_KILLED_BUFFER),
            ),
            (
                14,
                DispatchInput::zero_u32_words(plan.row_ptr_words, IFDS_CSR_ROW_PTR_BUFFER),
            ),
            (
                15,
                DispatchInput::zero_u32_words(plan.row_cursor_words, IFDS_CSR_ROW_CURSOR_BUFFER),
            ),
            (
                16,
                DispatchInput::zero_u32_words(plan.col_idx_words, IFDS_CSR_COL_IDX_BUFFER),
            ),
            (
                17,
                DispatchInput::zero_u32_words(plan.col_len_words, IFDS_CSR_COL_LEN_BUFFER),
            ),
        ],
    )
}
