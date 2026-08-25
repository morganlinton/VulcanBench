use super::{
    ResidentCsrQueueBatchProgramShape, ResidentCsrQueueBatchQueryHandles,
    ResidentCsrQueueBatchScratch, ResidentCsrQueueBatchShape,
};
use vyre_primitives::bitset::zero::bitset_zero;
use vyre_primitives::graph::csr_frontier_queue::{
    csr_queue_forward_traverse, frontier_queue_len_init, frontier_word_block_offsets_in_place,
    frontier_word_block_offsets_to_queue_parallel, frontier_word_block_prefix_to_queue_parallel,
    frontier_word_counts_scan_pass_a, frontier_words_to_queue_clear_out_parallel,
    validate_frontier_queue_batch,
};
use vyre_primitives::graph::csr_queue_split::csr_queue_split_low_forward_traverse;
use vyre_primitives::graph::csr_queue_strided::csr_queue_strided_forward_traverse;

use crate::csr_frontier_queue_batch_memory::ResidentCsrQueueBatchMemoryPlan;
use crate::csr_frontier_queue_resident::ResidentCsrQueueGraph;
use crate::dispatch_buffers::u32_word_bytes;
use crate::graph::csr_frontier_queue_scratch::{
    frontier_word_dispatch_grid, frontier_word_prefix_scratch,
    frontier_word_prefix_uses_precomputed_offsets, resident_csr_queue_frontier_stats,
    resident_csr_queue_materializer_for_stats,
    resident_csr_queue_scratch_bytes_per_query_for_materializer_and_traverse,
    resident_csr_queue_split_low_grid, resident_csr_queue_traverse_grid,
    resident_csr_queue_traverse_kind_for_graph_stats, FrontierWordPrefixScratch,
    ResidentCsrQueueMaterializer, ResidentCsrQueueTraverseKind, STRIDED_FORWARD_MIN_ROW_DEGREE,
};
use crate::graph::dispatch_bridge::alloc_resident_buffers;
use crate::hardware::scratch::reserve_vec as reserve_graph_vec;
use crate::optimizer::dispatcher::{
    DispatchError, OptimizerDispatcher, ResidentDispatchStep, ResidentReadRange,
};

/// Run many sparse frontier queries over one resident CSR graph.
pub fn run_resident_csr_queue_batch_into(
    dispatcher: &dyn OptimizerDispatcher,
    graph: &ResidentCsrQueueGraph,
    scratch: &mut ResidentCsrQueueBatchScratch,
    frontiers: &[&[u32]],
    queue_capacity: u32,
    allow_mask: u32,
    outputs: &mut Vec<Vec<u8>>,
) -> Result<(), DispatchError> {
    validate_frontier_queue_batch(graph.node_count(), frontiers, queue_capacity)
        .map_err(DispatchError::BadInputs)?;
    let frontier_stats =
        resident_csr_queue_frontier_stats(graph.node_count(), frontiers, queue_capacity)
            .map_err(DispatchError::BadInputs)?;
    let effective_queue_capacity = frontier_stats.effective_queue_capacity;
    let materializer = resident_csr_queue_materializer_for_stats(
        graph.words(),
        effective_queue_capacity,
        frontier_stats.max_nonzero_words,
    );
    let traverse_kind = resident_csr_queue_traverse_kind_for_graph_stats(
        graph.node_count(),
        graph.max_row_degree(),
        graph.high_degree_source_count(),
        effective_queue_capacity,
    );
    ensure_batch_scratch(
        dispatcher,
        graph,
        scratch,
        frontiers.len(),
        effective_queue_capacity,
        allow_mask,
        materializer,
        traverse_kind,
    )?;

    let frontier_bytes = u32_word_bytes(graph.words(), "resident CSR queue batch frontier")?;
    if scratch.frontier_payloads.len() < frontiers.len() {
        scratch
            .frontier_payloads
            .resize_with(frontiers.len(), Vec::new);
    }
    scratch.frontier_payloads.truncate(frontiers.len());
    for (payload, frontier) in scratch.frontier_payloads.iter_mut().zip(frontiers) {
        payload.clear();
        vyre_primitives::wire::append_u32_slice_le_bytes(frontier, payload);
    }
    prepare_batch_sequence_tables(
        graph,
        scratch,
        frontiers.len(),
        frontier_bytes,
        materializer,
        traverse_kind,
    )?;

    let mut upload_refs = Vec::new();
    reserve_graph_vec(
        &mut upload_refs,
        frontiers.len(),
        "resident CSR queue batch uploads",
    )?;
    for query_index in 0..frontiers.len() {
        let handles = scratch.handles[query_index];
        upload_refs.push((
            handles.frontier,
            scratch.frontier_payloads[query_index].as_slice(),
        ));
    }

    let queue_program = scratch.queue_program.as_ref().ok_or_else(|| {
        DispatchError::BackendError(
            "batch CSR queue program is missing after ensure_batch_scratch. Fix: rebuild batch scratch before resident CSR queue dispatch.".to_string(),
        )
    })?;
    let traverse_program = scratch.traverse_program.as_ref().ok_or_else(|| {
        DispatchError::BackendError(
            "batch CSR traverse program is missing after ensure_batch_scratch. Fix: rebuild batch scratch before resident CSR traverse dispatch.".to_string(),
        )
    })?;

    let mut steps = Vec::new();
    let traverse_grid = resident_csr_queue_traverse_grid(effective_queue_capacity, traverse_kind);
    let high_traverse_grid = match traverse_kind {
        ResidentCsrQueueTraverseKind::MixedSplit {
            high_queue_capacity,
        } => Some(resident_csr_queue_traverse_grid(
            high_queue_capacity,
            ResidentCsrQueueTraverseKind::RowStrided,
        )),
        ResidentCsrQueueTraverseKind::RowSerial | ResidentCsrQueueTraverseKind::RowStrided => None,
    };
    reserve_graph_vec(
        &mut steps,
        frontiers
            .len()
            .checked_mul(7)
            .ok_or_else(|| DispatchError::BackendError(
                "Fix: resident CSR queue batch step count overflowed while reserving dispatch sequence slots."
                    .to_string(),
            ))?,
        "resident CSR queue batch steps",
    )?;
    macro_rules! append_traverse_steps {
        ($query_index:expr) => {
            if matches!(traverse_kind, ResidentCsrQueueTraverseKind::MixedSplit { .. }) {
                let high_len_init_program =
                    scratch.high_len_init_program.as_ref().ok_or_else(|| {
                        DispatchError::BackendError(
                            "batch CSR high queue length init program is missing after ensure_batch_scratch. Fix: rebuild batch scratch before resident CSR queue dispatch.".to_string(),
                        )
                    })?;
                let split_low_program = scratch.split_low_program.as_ref().ok_or_else(|| {
                    DispatchError::BackendError(
                        "batch CSR split-low program is missing after ensure_batch_scratch. Fix: rebuild batch scratch before resident CSR queue dispatch.".to_string(),
                    )
                })?;
                steps.push(ResidentDispatchStep {
                    program: high_len_init_program,
                    handle_ids: &scratch.high_len_handle_sets[$query_index],
                    grid_override: Some([1, 1, 1]),
                });
                steps.push(ResidentDispatchStep {
                    program: split_low_program,
                    handle_ids: &scratch.split_low_handle_sets[$query_index],
                    grid_override: Some(resident_csr_queue_split_low_grid(
                        effective_queue_capacity,
                    )),
                });
                steps.push(ResidentDispatchStep {
                    program: traverse_program,
                    handle_ids: &scratch.high_traverse_handle_sets[$query_index],
                    grid_override: high_traverse_grid,
                });
            } else {
                steps.push(ResidentDispatchStep {
                    program: traverse_program,
                    handle_ids: &scratch.traverse_handle_sets[$query_index],
                    grid_override: Some(traverse_grid),
                });
            }
        };
    }
    match materializer {
        ResidentCsrQueueMaterializer::AtomicWordScan => {
            let queue_len_init_program = scratch.queue_len_init_program.as_ref().ok_or_else(|| {
                DispatchError::BackendError(
                    "batch CSR queue length init program is missing after ensure_batch_scratch. Fix: rebuild batch scratch before resident CSR queue dispatch.".to_string(),
                )
            })?;
            for query_index in 0..frontiers.len() {
                steps.push(ResidentDispatchStep {
                    program: queue_len_init_program,
                    handle_ids: &scratch.queue_len_handle_sets[query_index],
                    grid_override: Some([1, 1, 1]),
                });
                steps.push(ResidentDispatchStep {
                    program: queue_program,
                    handle_ids: &scratch.atomic_word_queue_handle_sets[query_index],
                    grid_override: Some(frontier_word_grid(graph.words())?),
                });
                append_traverse_steps!(query_index);
            }
        }
        ResidentCsrQueueMaterializer::DeterministicWordPrefix => {
            let word_prefix = word_prefix_scratch(graph.words())?;
            let precompute_block_offsets =
                frontier_word_prefix_uses_precomputed_offsets(word_prefix.block_count);
            let clear_frontier_out_program =
                scratch.clear_frontier_out_program.as_ref().ok_or_else(|| {
                    DispatchError::BackendError(
                        "batch CSR queue output clear program is missing after ensure_batch_scratch. Fix: rebuild batch scratch before resident CSR queue dispatch.".to_string(),
                    )
                })?;
            let word_counts_program = scratch.word_counts_program.as_ref().ok_or_else(|| {
                DispatchError::BackendError(
                    "batch CSR queue word-count scan program is missing after ensure_batch_scratch. Fix: rebuild batch scratch before resident CSR queue dispatch.".to_string(),
                )
            })?;
            let block_offsets_program = scratch.word_block_offsets_program.as_ref();
            for query_index in 0..frontiers.len() {
                steps.push(ResidentDispatchStep {
                    program: clear_frontier_out_program,
                    handle_ids: &scratch.clear_handle_sets[query_index],
                    grid_override: Some(frontier_word_grid(graph.words())?),
                });
                steps.push(ResidentDispatchStep {
                    program: word_counts_program,
                    handle_ids: &scratch.word_count_handle_sets[query_index],
                    grid_override: Some([word_prefix.block_count, 1, 1]),
                });
                if precompute_block_offsets {
                    let block_offsets_program = block_offsets_program.ok_or_else(|| {
                        DispatchError::BackendError(
                            "batch CSR queue block-offset scan program is missing after ensure_batch_scratch. Fix: rebuild batch scratch before resident CSR queue dispatch.".to_string(),
                        )
                    })?;
                    steps.push(ResidentDispatchStep {
                        program: block_offsets_program,
                        handle_ids: &scratch.word_block_offsets_handle_sets[query_index],
                        grid_override: Some([1, 1, 1]),
                    });
                }
                steps.push(ResidentDispatchStep {
                    program: queue_program,
                    handle_ids: &scratch.word_prefix_queue_handle_sets[query_index],
                    grid_override: Some(frontier_word_grid(graph.words())?),
                });
                append_traverse_steps!(query_index);
            }
        }
    }

    dispatcher.upload_resident_many_sequence_read_ranges_into(
        &upload_refs,
        &steps,
        &scratch.read_ranges,
        &mut scratch.readbacks,
    )?;

    if outputs.len() < frontiers.len() {
        outputs.resize_with(frontiers.len(), Vec::new);
    }
    outputs.truncate(frontiers.len());
    for (output, readback) in outputs.iter_mut().zip(&scratch.readbacks) {
        output.clear();
        output.extend_from_slice(readback);
    }
    Ok(())
}

/// Run many sparse frontier queries, sharded by resident scratch budget.
pub fn run_resident_csr_queue_batch_budgeted_into(
    dispatcher: &dyn OptimizerDispatcher,
    graph: &ResidentCsrQueueGraph,
    scratch: &mut ResidentCsrQueueBatchScratch,
    frontiers: &[&[u32]],
    queue_capacity: u32,
    allow_mask: u32,
    max_scratch_bytes: usize,
    outputs: &mut Vec<Vec<u8>>,
) -> Result<ResidentCsrQueueBatchMemoryPlan, DispatchError> {
    validate_frontier_queue_batch(graph.node_count(), frontiers, queue_capacity)
        .map_err(DispatchError::BadInputs)?;
    let (plan, chunks) = plan_budgeted_effective_chunks(
        graph,
        graph.words(),
        frontiers,
        queue_capacity,
        max_scratch_bytes,
    )?;
    if outputs.len() < frontiers.len() {
        outputs.resize_with(frontiers.len(), Vec::new);
    }
    outputs.truncate(frontiers.len());

    let mut chunk_outputs = Vec::new();
    for chunk in chunks {
        let frontier_chunk = &frontiers[chunk.start..chunk.end];
        run_resident_csr_queue_batch_into(
            dispatcher,
            graph,
            scratch,
            frontier_chunk,
            chunk.queue_capacity,
            allow_mask,
            &mut chunk_outputs,
        )?;
        let offset = chunk.start;
        for (target, source) in outputs[offset..offset + frontier_chunk.len()]
            .iter_mut()
            .zip(&chunk_outputs)
        {
            target.clear();
            target.extend_from_slice(source);
        }
    }

    Ok(plan)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ResidentCsrQueueBudgetedChunk {
    start: usize,
    end: usize,
    queue_capacity: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ResidentCsrQueueBudgetedQueryStats {
    queue_capacity: u32,
    nonzero_words: usize,
}

fn plan_budgeted_effective_chunks(
    graph: &ResidentCsrQueueGraph,
    frontier_words: usize,
    frontiers: &[&[u32]],
    requested_queue_capacity: u32,
    max_scratch_bytes: usize,
) -> Result<
    (
        ResidentCsrQueueBatchMemoryPlan,
        Vec<ResidentCsrQueueBudgetedChunk>,
    ),
    DispatchError,
> {
    let mut query_stats = Vec::new();
    reserve_graph_vec(
        &mut query_stats,
        frontiers.len(),
        "resident CSR queue budgeted query stats",
    )?;
    for frontier in frontiers {
        let stats = resident_csr_queue_frontier_stats(
            graph.node_count(),
            &[*frontier],
            requested_queue_capacity,
        )
        .map_err(DispatchError::BadInputs)?;
        query_stats.push(ResidentCsrQueueBudgetedQueryStats {
            queue_capacity: stats.effective_queue_capacity,
            nonzero_words: stats.max_nonzero_words,
        });
    }

    let mut chunks = Vec::new();
    reserve_graph_vec(
        &mut chunks,
        frontiers.len(),
        "resident CSR queue budgeted dispatch chunks",
    )?;
    let mut start = 0usize;
    let mut max_queries_per_dispatch = 0usize;
    let mut peak_bytes_per_query = 0usize;
    let mut peak_batch_scratch_bytes = 0usize;

    while start < frontiers.len() {
        let mut end = start + 1;
        let mut chunk_capacity = query_stats[start].queue_capacity;
        let mut chunk_nonzero_words = query_stats[start].nonzero_words;
        let mut bytes_per_query = budgeted_chunk_bytes_per_query(
            graph,
            frontier_words,
            chunk_capacity,
            chunk_nonzero_words,
        )?;
        ensure_one_query_fits_budget(bytes_per_query, max_scratch_bytes)?;

        while end < frontiers.len() {
            let candidate_capacity = chunk_capacity.max(query_stats[end].queue_capacity);
            let candidate_nonzero_words = chunk_nonzero_words.max(query_stats[end].nonzero_words);
            let candidate_bytes = budgeted_chunk_bytes_per_query(
                graph,
                frontier_words,
                candidate_capacity,
                candidate_nonzero_words,
            )?;
            let candidate_queries = end - start + 1;
            let candidate_peak = checked_batch_scratch_bytes(candidate_bytes, candidate_queries)?;
            if candidate_peak > max_scratch_bytes {
                break;
            }
            chunk_capacity = candidate_capacity;
            chunk_nonzero_words = candidate_nonzero_words;
            bytes_per_query = candidate_bytes;
            end += 1;
        }

        let query_count = end - start;
        let chunk_peak = checked_batch_scratch_bytes(bytes_per_query, query_count)?;
        max_queries_per_dispatch = max_queries_per_dispatch.max(query_count);
        peak_bytes_per_query = peak_bytes_per_query.max(bytes_per_query);
        peak_batch_scratch_bytes = peak_batch_scratch_bytes.max(chunk_peak);
        chunks.push(ResidentCsrQueueBudgetedChunk {
            start,
            end,
            queue_capacity: chunk_capacity,
        });
        start = end;
    }

    Ok((
        ResidentCsrQueueBatchMemoryPlan {
            query_count: frontiers.len(),
            max_queries_per_dispatch,
            dispatch_batches: chunks.len(),
            bytes_per_query: peak_bytes_per_query,
            peak_batch_scratch_bytes,
        },
        chunks,
    ))
}

fn budgeted_chunk_bytes_per_query(
    graph: &ResidentCsrQueueGraph,
    frontier_words: usize,
    queue_capacity: u32,
    max_nonzero_words: usize,
) -> Result<usize, DispatchError> {
    let materializer = resident_csr_queue_materializer_for_stats(
        frontier_words,
        queue_capacity,
        max_nonzero_words,
    );
    let traverse_kind = resident_csr_queue_traverse_kind_for_graph_stats(
        graph.node_count(),
        graph.max_row_degree(),
        graph.high_degree_source_count(),
        queue_capacity,
    );
    resident_csr_queue_scratch_bytes_per_query_for_materializer_and_traverse(
        frontier_words,
        queue_capacity,
        materializer,
        traverse_kind,
    )
    .map_err(DispatchError::BadInputs)
}

fn checked_batch_scratch_bytes(
    bytes_per_query: usize,
    query_count: usize,
) -> Result<usize, DispatchError> {
    bytes_per_query.checked_mul(query_count).ok_or_else(|| {
        DispatchError::BadInputs(
            "resident CSR queue budgeted batch scratch byte calculation overflowed. Fix: shard the query batch before planning."
                .to_string(),
        )
    })
}

fn ensure_one_query_fits_budget(
    bytes_per_query: usize,
    max_scratch_bytes: usize,
) -> Result<(), DispatchError> {
    if bytes_per_query <= max_scratch_bytes {
        return Ok(());
    }
    Err(DispatchError::BadInputs(format!(
        "resident CSR queue batch needs {bytes_per_query} scratch bytes per query but budget allows {max_scratch_bytes}. Fix: increase max_scratch_bytes or use a smaller graph shard."
    )))
}

fn prepare_batch_sequence_tables(
    graph: &ResidentCsrQueueGraph,
    scratch: &mut ResidentCsrQueueBatchScratch,
    batch_len: usize,
    frontier_bytes: usize,
    materializer: ResidentCsrQueueMaterializer,
    traverse_kind: ResidentCsrQueueTraverseKind,
) -> Result<(), DispatchError> {
    scratch.clear_handle_sets.clear();
    scratch.queue_len_handle_sets.clear();
    scratch.word_count_handle_sets.clear();
    scratch.word_block_offsets_handle_sets.clear();
    scratch.queue_handle_sets.clear();
    scratch.atomic_word_queue_handle_sets.clear();
    scratch.word_prefix_queue_handle_sets.clear();
    scratch.traverse_handle_sets.clear();
    scratch.high_len_handle_sets.clear();
    scratch.split_low_handle_sets.clear();
    scratch.high_traverse_handle_sets.clear();
    scratch.read_ranges.clear();

    reserve_graph_vec(
        &mut scratch.clear_handle_sets,
        batch_len,
        "resident CSR queue batch output clear handles",
    )?;
    reserve_graph_vec(
        &mut scratch.queue_len_handle_sets,
        batch_len,
        "resident CSR queue batch queue-length init handles",
    )?;
    reserve_graph_vec(
        &mut scratch.word_count_handle_sets,
        batch_len,
        "resident CSR queue batch word-count handles",
    )?;
    reserve_graph_vec(
        &mut scratch.word_block_offsets_handle_sets,
        batch_len,
        "resident CSR queue batch block-offset handles",
    )?;
    reserve_graph_vec(
        &mut scratch.queue_handle_sets,
        batch_len,
        "resident CSR queue batch queue handles",
    )?;
    reserve_graph_vec(
        &mut scratch.atomic_word_queue_handle_sets,
        batch_len,
        "resident CSR queue batch atomic word queue handles",
    )?;
    reserve_graph_vec(
        &mut scratch.word_prefix_queue_handle_sets,
        batch_len,
        "resident CSR queue batch word-prefix queue handles",
    )?;
    reserve_graph_vec(
        &mut scratch.traverse_handle_sets,
        batch_len,
        "resident CSR queue batch traverse handles",
    )?;
    if matches!(
        traverse_kind,
        ResidentCsrQueueTraverseKind::MixedSplit { .. }
    ) {
        reserve_graph_vec(
            &mut scratch.high_len_handle_sets,
            batch_len,
            "resident CSR queue batch high queue length handles",
        )?;
        reserve_graph_vec(
            &mut scratch.split_low_handle_sets,
            batch_len,
            "resident CSR queue batch split-low handles",
        )?;
        reserve_graph_vec(
            &mut scratch.high_traverse_handle_sets,
            batch_len,
            "resident CSR queue batch high-row traverse handles",
        )?;
    }
    reserve_graph_vec(
        &mut scratch.read_ranges,
        batch_len,
        "resident CSR queue batch read ranges",
    )?;

    let precompute_block_offsets =
        if materializer == ResidentCsrQueueMaterializer::DeterministicWordPrefix {
            let word_prefix = word_prefix_scratch(graph.words())?;
            frontier_word_prefix_uses_precomputed_offsets(word_prefix.block_count)
        } else {
            false
        };
    for (query_index, handles) in scratch.handles.iter().take(batch_len).enumerate() {
        scratch.clear_handle_sets.push([handles.frontier_out]);
        scratch.queue_len_handle_sets.push([handles.queue_len]);
        scratch
            .queue_handle_sets
            .push([handles.frontier, handles.active_queue, handles.queue_len]);
        if materializer == ResidentCsrQueueMaterializer::AtomicWordScan {
            scratch.atomic_word_queue_handle_sets.push([
                handles.frontier,
                handles.active_queue,
                handles.queue_len,
                handles.frontier_out,
            ]);
        }
        match (handles.word_partials, handles.block_totals) {
            (Some(word_partials), Some(block_totals)) => {
                scratch.word_count_handle_sets.push([
                    handles.frontier,
                    word_partials,
                    block_totals,
                ]);
                if precompute_block_offsets {
                    scratch.word_block_offsets_handle_sets.push([block_totals]);
                }
                scratch.word_prefix_queue_handle_sets.push([
                    handles.frontier,
                    word_partials,
                    block_totals,
                    handles.active_queue,
                    handles.queue_len,
                ]);
            }
            (None, None) if materializer == ResidentCsrQueueMaterializer::AtomicWordScan => {}
            (None, None) => {
                return Err(DispatchError::BackendError(format!(
                    "Fix: resident CSR queue batch query {query_index} is missing word-prefix scratch handles."
                )));
            }
            _ => {
                return Err(DispatchError::BackendError(format!(
                    "Fix: resident CSR queue batch query {query_index} has incomplete word-prefix scratch handles."
                )));
            }
        }
        scratch.traverse_handle_sets.push([
            handles.active_queue,
            handles.queue_len,
            graph.edge_offsets_handle(),
            graph.edge_targets_handle(),
            graph.edge_kind_mask_handle(),
            handles.frontier_out,
        ]);
        if matches!(
            traverse_kind,
            ResidentCsrQueueTraverseKind::MixedSplit { .. }
        ) {
            let high_queue = handles.high_queue.ok_or_else(|| {
                DispatchError::BackendError(format!(
                    "Fix: resident CSR queue batch query {query_index} is missing high_queue scratch for mixed split traversal."
                ))
            })?;
            let high_len = handles.high_len.ok_or_else(|| {
                DispatchError::BackendError(format!(
                    "Fix: resident CSR queue batch query {query_index} is missing high_len scratch for mixed split traversal."
                ))
            })?;
            scratch.high_len_handle_sets.push([high_len]);
            scratch.split_low_handle_sets.push([
                handles.active_queue,
                handles.queue_len,
                graph.edge_offsets_handle(),
                graph.edge_targets_handle(),
                graph.edge_kind_mask_handle(),
                handles.frontier_out,
                high_queue,
                high_len,
            ]);
            scratch.high_traverse_handle_sets.push([
                high_queue,
                high_len,
                graph.edge_offsets_handle(),
                graph.edge_targets_handle(),
                graph.edge_kind_mask_handle(),
                handles.frontier_out,
            ]);
        }
        scratch.read_ranges.push(ResidentReadRange {
            handle_id: handles.frontier_out,
            byte_offset: 0,
            byte_len: frontier_bytes,
        });
    }

    Ok(())
}

fn ensure_batch_scratch(
    dispatcher: &dyn OptimizerDispatcher,
    graph: &ResidentCsrQueueGraph,
    scratch: &mut ResidentCsrQueueBatchScratch,
    batch_len: usize,
    queue_capacity: u32,
    allow_mask: u32,
    materializer: ResidentCsrQueueMaterializer,
    traverse_kind: ResidentCsrQueueTraverseKind,
) -> Result<(), DispatchError> {
    let frontier_bytes =
        u32_word_bytes(graph.words(), "resident CSR queue batch scratch frontier")?;
    let queue_bytes = u32_word_bytes(
        queue_capacity as usize,
        "resident CSR queue batch scratch active_queue",
    )?;
    let queue_len_bytes = u32_word_bytes(1, "resident CSR queue batch scratch queue_len")?;
    let high_queue_capacity = match traverse_kind {
        ResidentCsrQueueTraverseKind::MixedSplit {
            high_queue_capacity,
        } => high_queue_capacity,
        ResidentCsrQueueTraverseKind::RowSerial | ResidentCsrQueueTraverseKind::RowStrided => 0,
    };
    let has_high_queue = high_queue_capacity > 0;
    let high_queue_bytes = if has_high_queue {
        u32_word_bytes(
            high_queue_capacity as usize,
            "resident CSR queue batch scratch high_queue",
        )?
    } else {
        0
    };
    let high_len_bytes = if has_high_queue {
        u32_word_bytes(1, "resident CSR queue batch scratch high_len")?
    } else {
        0
    };
    let shape = ResidentCsrQueueBatchShape {
        batch_len,
        frontier_bytes,
        queue_capacity,
        high_queue_capacity,
        node_count: graph.node_count(),
        materializer,
    };
    let program_shape = ResidentCsrQueueBatchProgramShape {
        queue_capacity,
        allow_mask,
        node_count: graph.node_count(),
        edge_count: graph.edge_count(),
        materializer,
        traverse_kind,
    };
    if matches!(
        scratch.shape,
        Some(existing)
            if existing.batch_len >= batch_len
                && existing.frontier_bytes == frontier_bytes
                && existing.queue_capacity >= queue_capacity
                && existing.high_queue_capacity >= high_queue_capacity
                && existing.node_count == graph.node_count()
                && existing.materializer == materializer
    ) {
        ensure_batch_programs(scratch, graph, program_shape)?;
        return Ok(());
    }

    scratch.free(dispatcher)?;
    reserve_graph_vec(
        &mut scratch.handles,
        batch_len,
        "resident CSR queue batch scratch handles",
    )?;
    for _ in 0..batch_len {
        match materializer {
            ResidentCsrQueueMaterializer::AtomicWordScan => {
                if !has_high_queue {
                    let [frontier, active_queue, queue_len, frontier_out] =
                        match alloc_resident_buffers(
                            dispatcher,
                            [frontier_bytes, queue_bytes, queue_len_bytes, frontier_bytes],
                            "resident CSR queue batch scratch query",
                        ) {
                            Ok(handles) => handles,
                            Err(error) => {
                                if let Err(free_error) = scratch.free(dispatcher) {
                                    return Err(batch_scratch_allocation_cleanup_error(
                                        error, free_error,
                                    ));
                                }
                                return Err(error);
                            }
                        };
                    scratch.handles.push(ResidentCsrQueueBatchQueryHandles {
                        frontier,
                        active_queue,
                        queue_len,
                        frontier_out,
                        word_partials: None,
                        block_totals: None,
                        high_queue: None,
                        high_len: None,
                    });
                } else {
                    let [frontier, active_queue, queue_len, frontier_out, high_queue, high_len] =
                        match alloc_resident_buffers(
                            dispatcher,
                            [
                                frontier_bytes,
                                queue_bytes,
                                queue_len_bytes,
                                frontier_bytes,
                                high_queue_bytes,
                                high_len_bytes,
                            ],
                            "resident CSR queue batch scratch query",
                        ) {
                            Ok(handles) => handles,
                            Err(error) => {
                                if let Err(free_error) = scratch.free(dispatcher) {
                                    return Err(batch_scratch_allocation_cleanup_error(
                                        error, free_error,
                                    ));
                                }
                                return Err(error);
                            }
                        };
                    scratch.handles.push(ResidentCsrQueueBatchQueryHandles {
                        frontier,
                        active_queue,
                        queue_len,
                        frontier_out,
                        word_partials: None,
                        block_totals: None,
                        high_queue: Some(high_queue),
                        high_len: Some(high_len),
                    });
                }
            }
            ResidentCsrQueueMaterializer::DeterministicWordPrefix => {
                let word_prefix = word_prefix_scratch(graph.words())?;
                let word_partials_bytes = u32_word_bytes(
                    word_prefix.partial_words,
                    "resident CSR queue batch scratch word_partials",
                )?;
                let block_totals_bytes = u32_word_bytes(
                    word_prefix.block_total_words,
                    "resident CSR queue batch scratch block_totals",
                )?;
                if !has_high_queue {
                    let [frontier, active_queue, queue_len, frontier_out, word_partials, block_totals] =
                        match alloc_resident_buffers(
                            dispatcher,
                            [
                                frontier_bytes,
                                queue_bytes,
                                queue_len_bytes,
                                frontier_bytes,
                                word_partials_bytes,
                                block_totals_bytes,
                            ],
                            "resident CSR queue batch scratch query",
                        ) {
                            Ok(handles) => handles,
                            Err(error) => {
                                if let Err(free_error) = scratch.free(dispatcher) {
                                    return Err(batch_scratch_allocation_cleanup_error(
                                        error, free_error,
                                    ));
                                }
                                return Err(error);
                            }
                        };
                    scratch.handles.push(ResidentCsrQueueBatchQueryHandles {
                        frontier,
                        active_queue,
                        queue_len,
                        frontier_out,
                        word_partials: Some(word_partials),
                        block_totals: Some(block_totals),
                        high_queue: None,
                        high_len: None,
                    });
                } else {
                    let [frontier, active_queue, queue_len, frontier_out, word_partials, block_totals, high_queue, high_len] =
                        match alloc_resident_buffers(
                            dispatcher,
                            [
                                frontier_bytes,
                                queue_bytes,
                                queue_len_bytes,
                                frontier_bytes,
                                word_partials_bytes,
                                block_totals_bytes,
                                high_queue_bytes,
                                high_len_bytes,
                            ],
                            "resident CSR queue batch scratch query",
                        ) {
                            Ok(handles) => handles,
                            Err(error) => {
                                if let Err(free_error) = scratch.free(dispatcher) {
                                    return Err(batch_scratch_allocation_cleanup_error(
                                        error, free_error,
                                    ));
                                }
                                return Err(error);
                            }
                        };
                    scratch.handles.push(ResidentCsrQueueBatchQueryHandles {
                        frontier,
                        active_queue,
                        queue_len,
                        frontier_out,
                        word_partials: Some(word_partials),
                        block_totals: Some(block_totals),
                        high_queue: Some(high_queue),
                        high_len: Some(high_len),
                    });
                }
            }
        }
    }
    scratch.shape = Some(shape);
    ensure_batch_programs(scratch, graph, program_shape)
}

fn ensure_batch_programs(
    scratch: &mut ResidentCsrQueueBatchScratch,
    graph: &ResidentCsrQueueGraph,
    program_shape: ResidentCsrQueueBatchProgramShape,
) -> Result<(), DispatchError> {
    if scratch.program_shape == Some(program_shape)
        && batch_programs_available(scratch, graph, program_shape)?
    {
        return Ok(());
    }

    scratch.program_shape = None;
    scratch.word_counts_program = None;
    scratch.word_block_offsets_program = None;
    scratch.queue_len_init_program = None;
    scratch.clear_frontier_out_program = None;
    scratch.queue_program = None;
    scratch.high_len_init_program = None;
    scratch.split_low_program = None;
    scratch.traverse_program = None;
    match program_shape.materializer {
        ResidentCsrQueueMaterializer::AtomicWordScan => {
            scratch.queue_len_init_program = Some(frontier_queue_len_init("queue_len"));
            scratch.queue_program = Some(frontier_words_to_queue_clear_out_parallel(
                "frontier",
                "active_queue",
                "queue_len",
                "frontier_out",
                program_shape.node_count,
                program_shape.queue_capacity,
            ));
        }
        ResidentCsrQueueMaterializer::DeterministicWordPrefix => {
            scratch.clear_frontier_out_program =
                Some(bitset_zero("frontier_out", graph.words() as u32));
            let word_prefix = word_prefix_scratch(graph.words())?;
            scratch.word_counts_program = Some(frontier_word_counts_scan_pass_a(
                "frontier",
                "word_partials",
                "block_totals",
                program_shape.node_count,
            ));
            if frontier_word_prefix_uses_precomputed_offsets(word_prefix.block_count) {
                scratch.word_block_offsets_program = Some(frontier_word_block_offsets_in_place(
                    "block_totals",
                    program_shape.node_count,
                ));
                scratch.queue_program = Some(frontier_word_block_offsets_to_queue_parallel(
                    "frontier",
                    "word_partials",
                    "block_totals",
                    "active_queue",
                    "queue_len",
                    program_shape.node_count,
                    program_shape.queue_capacity,
                ));
            } else {
                scratch.queue_program = Some(frontier_word_block_prefix_to_queue_parallel(
                    "frontier",
                    "word_partials",
                    "block_totals",
                    "active_queue",
                    "queue_len",
                    program_shape.node_count,
                    program_shape.queue_capacity,
                ));
            }
        }
    }
    scratch.traverse_program = Some(match program_shape.traverse_kind {
        ResidentCsrQueueTraverseKind::RowSerial => csr_queue_forward_traverse(
            "active_queue",
            "queue_len",
            "edge_offsets",
            "edge_targets",
            "edge_kind_mask",
            "frontier_out",
            program_shape.node_count,
            program_shape.edge_count,
            program_shape.queue_capacity,
            program_shape.allow_mask,
        ),
        ResidentCsrQueueTraverseKind::RowStrided => csr_queue_strided_forward_traverse(
            "active_queue",
            "queue_len",
            "edge_offsets",
            "edge_targets",
            "edge_kind_mask",
            "frontier_out",
            program_shape.node_count,
            program_shape.edge_count,
            program_shape.queue_capacity,
            program_shape.allow_mask,
        ),
        ResidentCsrQueueTraverseKind::MixedSplit {
            high_queue_capacity,
        } => csr_queue_strided_forward_traverse(
            "high_queue",
            "high_len",
            "edge_offsets",
            "edge_targets",
            "edge_kind_mask",
            "frontier_out",
            program_shape.node_count,
            program_shape.edge_count,
            high_queue_capacity,
            program_shape.allow_mask,
        ),
    });
    if let ResidentCsrQueueTraverseKind::MixedSplit {
        high_queue_capacity,
    } = program_shape.traverse_kind
    {
        scratch.high_len_init_program = Some(frontier_queue_len_init("high_len"));
        scratch.split_low_program = Some(csr_queue_split_low_forward_traverse(
            "active_queue",
            "queue_len",
            "edge_offsets",
            "edge_targets",
            "edge_kind_mask",
            "frontier_out",
            "high_queue",
            "high_len",
            program_shape.node_count,
            program_shape.edge_count,
            program_shape.queue_capacity,
            high_queue_capacity,
            STRIDED_FORWARD_MIN_ROW_DEGREE,
            program_shape.allow_mask,
        ));
    }
    scratch.program_shape = Some(program_shape);
    Ok(())
}

fn batch_programs_available(
    scratch: &ResidentCsrQueueBatchScratch,
    graph: &ResidentCsrQueueGraph,
    program_shape: ResidentCsrQueueBatchProgramShape,
) -> Result<bool, DispatchError> {
    if scratch.queue_program.is_none() || scratch.traverse_program.is_none() {
        return Ok(false);
    }
    let materializer_programs_available = match program_shape.materializer {
        ResidentCsrQueueMaterializer::AtomicWordScan => scratch.queue_len_init_program.is_some(),
        ResidentCsrQueueMaterializer::DeterministicWordPrefix => {
            let word_prefix = word_prefix_scratch(graph.words())?;
            let needs_block_offsets =
                frontier_word_prefix_uses_precomputed_offsets(word_prefix.block_count);
            scratch.clear_frontier_out_program.is_some()
                && scratch.word_counts_program.is_some()
                && (!needs_block_offsets || scratch.word_block_offsets_program.is_some())
        }
    };
    let split_programs_available = !matches!(
        program_shape.traverse_kind,
        ResidentCsrQueueTraverseKind::MixedSplit { .. }
    ) || (scratch.high_len_init_program.is_some()
        && scratch.split_low_program.is_some());
    Ok(materializer_programs_available && split_programs_available)
}

fn word_prefix_scratch(words: usize) -> Result<FrontierWordPrefixScratch, DispatchError> {
    frontier_word_prefix_scratch(words).map_err(DispatchError::BackendError)
}

fn frontier_word_grid(words: usize) -> Result<[u32; 3], DispatchError> {
    frontier_word_dispatch_grid(words).map_err(DispatchError::BackendError)
}

fn batch_scratch_allocation_cleanup_error(
    allocation: DispatchError,
    cleanup: DispatchError,
) -> DispatchError {
    DispatchError::BackendError(format!(
        "Fix: resident CSR queue batch scratch allocation failed and cleanup also failed: allocation={allocation}; cleanup={cleanup}."
    ))
}
