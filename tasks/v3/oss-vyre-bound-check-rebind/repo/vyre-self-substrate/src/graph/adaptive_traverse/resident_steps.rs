use super::state::AdaptiveTraversalResidentScratch;
use super::{
    AdaptiveTraversalMode, ResidentAdaptiveFourRussiansDenseGraph,
    ResidentAdaptiveSparseQueueGraph, ResidentAdaptiveTraversalGraph,
};

use crate::dispatch_buffers::{u32_word_bytes, write_u32_slice_le_bytes};
use crate::graph::csr_frontier_queue_scratch::{
    frontier_word_dispatch_grid, frontier_word_prefix_scratch,
    frontier_word_prefix_uses_precomputed_offsets, resident_csr_queue_materializer_for_stats,
    resident_csr_queue_split_low_grid, resident_csr_queue_traverse_grid,
    resident_csr_queue_traverse_kind_for_graph_stats, FrontierWordPrefixScratch,
    ResidentCsrQueueMaterializer, ResidentCsrQueueTraverseKind,
    RESIDENT_CSR_QUEUE_MULTI_SOURCE_MIN_CAPACITY, STRIDED_FORWARD_MIN_ROW_DEGREE,
};
use crate::graph::dispatch_bridge::{
    alloc_resident_buffers, resident_sequence_single_u32_output_into,
};
use crate::graph::resident_handles::free_unique_resident_handles;
use crate::optimizer::dispatcher::{
    DispatchError, OptimizerDispatcher, ResidentDispatchStep, ResidentReadRange,
};
use vyre_primitives::bitset::zero::bitset_zero;
use vyre_primitives::graph::adaptive_traverse::{
    adaptive_four_russians_dense_step as primitive_adaptive_four_russians_dense_step,
    adaptive_sparse_dense_step as primitive_adaptive_sparse_dense_step,
    plan_adaptive_resident_auto_step, plan_adaptive_resident_frontier_step,
    plan_adaptive_resident_sparse_queue_step, AdaptiveResidentFrontierPlan,
    AdaptiveTraversalPlanCacheKey,
};
use vyre_primitives::graph::csr_frontier_queue::{
    csr_queue_forward_traverse as primitive_csr_queue_forward_traverse,
    frontier_queue_len_init as primitive_frontier_queue_len_init,
    frontier_word_block_offsets_in_place as primitive_frontier_word_block_offsets,
    frontier_word_block_offsets_to_queue_parallel as primitive_frontier_word_block_offsets_queue,
    frontier_word_block_prefix_to_queue_parallel as primitive_frontier_word_prefix_queue,
    frontier_word_counts_scan_pass_a as primitive_frontier_word_counts,
    frontier_words_to_queue_clear_out_parallel as primitive_frontier_words_to_queue_clear_out,
};
use vyre_primitives::graph::csr_queue_split::csr_queue_split_low_forward_traverse as primitive_csr_queue_split_low_forward_traverse;
use vyre_primitives::graph::csr_queue_strided::csr_queue_strided_forward_traverse as primitive_csr_queue_strided_forward_traverse;
use vyre_primitives::reduce::count::reduce_count;

#[derive(Clone, Copy)]
struct AdaptiveSparseQueueGraphView {
    node_count: u32,
    edge_count: u32,
    max_row_degree: u32,
    high_degree_source_count: u32,
    layout_hash: u64,
    handles: [u64; 3],
}

impl AdaptiveSparseQueueGraphView {
    fn from_full_graph(graph: &ResidentAdaptiveTraversalGraph) -> Self {
        let handles = graph.handles;
        Self {
            node_count: graph.node_count,
            edge_count: graph.edge_count,
            max_row_degree: graph.max_row_degree,
            high_degree_source_count: graph.high_degree_source_count,
            layout_hash: graph.layout_hash,
            handles: [handles[0], handles[1], handles[2]],
        }
    }

    fn from_sparse_queue_graph(graph: &ResidentAdaptiveSparseQueueGraph) -> Self {
        Self {
            node_count: graph.node_count,
            edge_count: graph.edge_count,
            max_row_degree: graph.max_row_degree,
            high_degree_source_count: graph.high_degree_source_count,
            layout_hash: graph.layout_hash,
            handles: graph.handles,
        }
    }
}

/// Run one adaptive sparse/dense traversal step over resident graph buffers.
///
/// # Errors
///
/// Propagates resident dispatch failures and malformed frontier/readback shapes.
#[allow(clippy::too_many_arguments)]
pub fn adaptive_traverse_resident_graph_step_with_scratch_into(
    dispatcher: &dyn OptimizerDispatcher,
    graph: &ResidentAdaptiveTraversalGraph,
    frontier_in: &[u32],
    allow_mask: u32,
    dense_threshold_pct: u32,
    scratch: &mut AdaptiveTraversalResidentScratch,
    frontier_out: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    let resident_plan = plan_adaptive_resident_frontier_step(graph.node_count, frontier_in)
        .map_err(DispatchError::BadInputs)?;
    if !resident_plan.work.has_active_bits {
        write_zero_frontier_result(resident_plan.work.layout.words, frontier_out);
        return Ok(());
    }
    let handles = ensure_frontier_handles(dispatcher, scratch, &resident_plan)?;
    write_u32_slice_le_bytes(&mut scratch.frontier_in_bytes, frontier_in);

    let words_u32 = resident_plan.work.layout.words_u32;
    let device_features = dispatcher.device_feature_cache_key();
    let popcount_program = scratch.plan_cache.get_or_build(
        AdaptiveTraversalPlanCacheKey::popcount(
            graph.layout_hash,
            graph.node_count,
            graph.edge_count,
            words_u32,
            device_features,
        ),
        || reduce_count("frontier_in", "frontier_popcount", words_u32),
    );
    let traverse_program = scratch.plan_cache.get_or_build(
        AdaptiveTraversalPlanCacheKey::sparse_dense(
            graph.layout_hash,
            graph.node_count,
            graph.edge_count,
            words_u32,
            allow_mask,
            dense_threshold_pct,
            device_features,
        ),
        || {
            primitive_adaptive_sparse_dense_step(
                "frontier_in",
                "frontier_out",
                "frontier_popcount",
                "edge_offsets",
                "edge_targets",
                "edge_kind_mask",
                "adj_rows_dense",
                graph.node_count,
                graph.edge_count,
                allow_mask,
                dense_threshold_pct,
            )
        },
    );
    let clear_frontier_out_program = scratch.plan_cache.get_or_build(
        AdaptiveTraversalPlanCacheKey::clear_frontier_out(
            graph.layout_hash,
            graph.node_count,
            graph.edge_count,
            words_u32,
            device_features,
        ),
        || bitset_zero("frontier_out", words_u32),
    );
    let graph_handles = graph.handles;
    let clear_handles = [handles[1]];
    let count_handles = [handles[0], handles[2]];
    let traverse_handles = [
        handles[0],
        handles[1],
        handles[2],
        graph_handles[0],
        graph_handles[1],
        graph_handles[2],
        graph_handles[3],
    ];
    let uploads = [(handles[0], scratch.frontier_in_bytes.as_slice())];
    let steps = [
        ResidentDispatchStep {
            program: &clear_frontier_out_program,
            handle_ids: &clear_handles,
            grid_override: Some(resident_plan.frontier_word_grid),
        },
        ResidentDispatchStep {
            program: &popcount_program,
            handle_ids: &count_handles,
            grid_override: Some([1, 1, 1]),
        },
        ResidentDispatchStep {
            program: &traverse_program,
            handle_ids: &traverse_handles,
            grid_override: Some(resident_plan.node_grid),
        },
    ];
    resident_sequence_single_u32_output_into(
        dispatcher,
        &uploads,
        &steps,
        ResidentReadRange {
            handle_id: handles[1],
            byte_offset: 0,
            byte_len: resident_plan.frontier_bytes,
        },
        &mut scratch.readbacks,
        resident_plan.work.layout.words,
        "adaptive_traverse_resident_graph_step frontier_out",
        frontier_out,
    )
}

/// Run one Four-Russians dense traversal step over a resident dense LUT.
///
/// # Errors
///
/// Propagates resident dispatch failures and malformed frontier/readback shapes.
pub fn adaptive_traverse_resident_graph_four_russians_dense_step_with_scratch_into(
    dispatcher: &dyn OptimizerDispatcher,
    graph: &ResidentAdaptiveFourRussiansDenseGraph,
    frontier_in: &[u32],
    scratch: &mut AdaptiveTraversalResidentScratch,
    frontier_out: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    let resident_plan = plan_adaptive_resident_frontier_step(graph.node_count, frontier_in)
        .map_err(DispatchError::BadInputs)?;
    if !resident_plan.work.has_active_bits {
        write_zero_frontier_result(resident_plan.work.layout.words, frontier_out);
        return Ok(());
    }
    let handles = ensure_frontier_handles(dispatcher, scratch, &resident_plan)?;
    write_u32_slice_le_bytes(&mut scratch.frontier_in_bytes, frontier_in);

    let words_u32 = resident_plan.work.layout.words_u32;
    let device_features = dispatcher.device_feature_cache_key();
    let dense_program = scratch.plan_cache.get_or_build(
        AdaptiveTraversalPlanCacheKey::four_russians_dense(
            graph.layout_hash,
            graph.node_count,
            words_u32,
            device_features,
        ),
        || {
            primitive_adaptive_four_russians_dense_step(
                "frontier_in",
                "four_russians_tile_lut",
                "frontier_out",
                graph.node_count,
            )
        },
    );
    let dense_handles = [handles[0], graph.lut_handle, handles[1]];
    let uploads = [(handles[0], scratch.frontier_in_bytes.as_slice())];
    let steps = [ResidentDispatchStep {
        program: &dense_program,
        handle_ids: &dense_handles,
        grid_override: Some(resident_plan.frontier_word_grid),
    }];
    resident_sequence_single_u32_output_into(
        dispatcher,
        &uploads,
        &steps,
        ResidentReadRange {
            handle_id: handles[1],
            byte_offset: 0,
            byte_len: resident_plan.frontier_bytes,
        },
        &mut scratch.readbacks,
        resident_plan.work.layout.words,
        "adaptive_traverse_resident_graph_four_russians_dense_step frontier_out",
        frontier_out,
    )
}

/// Run one queue-driven sparse traversal step over resident graph buffers.
///
/// The active queue is built and consumed on the GPU. The host uploads the
/// input frontier and reads only `frontier_out`; active source ids and queue
/// length remain device-resident.
///
/// # Errors
///
/// Propagates resident dispatch failures and malformed frontier/readback shapes.
#[allow(clippy::too_many_arguments)]
pub fn adaptive_traverse_resident_graph_sparse_queue_step_with_scratch_into(
    dispatcher: &dyn OptimizerDispatcher,
    graph: &ResidentAdaptiveTraversalGraph,
    frontier_in: &[u32],
    allow_mask: u32,
    scratch: &mut AdaptiveTraversalResidentScratch,
    frontier_out: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    adaptive_traverse_sparse_queue_step_with_graph_view_into(
        dispatcher,
        AdaptiveSparseQueueGraphView::from_full_graph(graph),
        frontier_in,
        allow_mask,
        scratch,
        frontier_out,
    )
}

/// Run one queue-driven adaptive sparse traversal step over CSR-only resident graph buffers.
///
/// The active queue is built and consumed on the GPU. This path uploads and
/// retains only CSR graph buffers, avoiding dense adjacency residency for
/// sparse-queue workloads.
///
/// # Errors
///
/// Propagates resident dispatch failures and malformed frontier/readback shapes.
#[allow(clippy::too_many_arguments)]
pub fn adaptive_traverse_resident_sparse_queue_step_with_scratch_into(
    dispatcher: &dyn OptimizerDispatcher,
    graph: &ResidentAdaptiveSparseQueueGraph,
    frontier_in: &[u32],
    allow_mask: u32,
    scratch: &mut AdaptiveTraversalResidentScratch,
    frontier_out: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    adaptive_traverse_sparse_queue_step_with_graph_view_into(
        dispatcher,
        AdaptiveSparseQueueGraphView::from_sparse_queue_graph(graph),
        frontier_in,
        allow_mask,
        scratch,
        frontier_out,
    )
}

#[allow(clippy::too_many_arguments)]
fn adaptive_traverse_sparse_queue_step_with_graph_view_into(
    dispatcher: &dyn OptimizerDispatcher,
    graph: AdaptiveSparseQueueGraphView,
    frontier_in: &[u32],
    allow_mask: u32,
    scratch: &mut AdaptiveTraversalResidentScratch,
    frontier_out: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    let sparse_plan = plan_adaptive_resident_sparse_queue_step(graph.node_count, frontier_in)
        .map_err(DispatchError::BadInputs)?;
    if !sparse_plan.frontier.work.has_active_bits {
        write_zero_frontier_result(sparse_plan.frontier.work.layout.words, frontier_out);
        return Ok(());
    }
    let handles = ensure_frontier_handles(dispatcher, scratch, &sparse_plan.frontier)?;
    let requested_queue_capacity = if graph.max_row_degree < STRIDED_FORWARD_MIN_ROW_DEGREE
        && sparse_plan.queue_capacity > 1
    {
        sparse_plan
            .queue_capacity
            .max(RESIDENT_CSR_QUEUE_MULTI_SOURCE_MIN_CAPACITY)
            .min(graph.node_count.max(1))
    } else {
        sparse_plan.queue_capacity
    };
    let requested_queue_bytes = u32_word_bytes(
        requested_queue_capacity as usize,
        "adaptive traversal active-source queue",
    )?;
    let queue_handle = ensure_queue_handle(dispatcher, scratch, requested_queue_bytes)?;
    write_u32_slice_le_bytes(&mut scratch.frontier_in_bytes, frontier_in);

    let words_u32 = sparse_plan.frontier.work.layout.words_u32;
    let words = sparse_plan.frontier.work.layout.words;
    let queue_capacity =
        u32::try_from(scratch.queue_bytes / std::mem::size_of::<u32>()).map_err(|_| {
            DispatchError::BackendError(format!(
                "Fix: adaptive traversal resident queue scratch has {} bytes, exceeding u32 queue capacity metadata.",
                scratch.queue_bytes
            ))
        })?;
    let materializer = resident_csr_queue_materializer_for_stats(
        words,
        queue_capacity,
        sparse_plan.frontier_nonzero_words,
    );
    let traverse_kind = resident_csr_queue_traverse_kind_for_graph_stats(
        graph.node_count,
        graph.max_row_degree,
        graph.high_degree_source_count,
        queue_capacity,
    );
    let traverse_grid = resident_csr_queue_traverse_grid(queue_capacity, traverse_kind);
    let device_features = dispatcher.device_feature_cache_key();
    let traverse_key = match traverse_kind {
        ResidentCsrQueueTraverseKind::RowSerial => AdaptiveTraversalPlanCacheKey::queue_forward(
            graph.layout_hash,
            graph.node_count,
            graph.edge_count,
            words_u32,
            queue_capacity,
            allow_mask,
            device_features,
        ),
        ResidentCsrQueueTraverseKind::RowStrided => {
            AdaptiveTraversalPlanCacheKey::queue_forward_strided(
                graph.layout_hash,
                graph.node_count,
                graph.edge_count,
                words_u32,
                queue_capacity,
                allow_mask,
                device_features,
            )
        }
        ResidentCsrQueueTraverseKind::MixedSplit {
            high_queue_capacity,
        } => AdaptiveTraversalPlanCacheKey::queue_forward_strided(
            graph.layout_hash,
            graph.node_count,
            graph.edge_count,
            words_u32,
            high_queue_capacity,
            allow_mask,
            device_features,
        ),
    };
    let traverse_program = scratch
        .plan_cache
        .get_or_build(traverse_key, || match traverse_kind {
            ResidentCsrQueueTraverseKind::RowSerial => primitive_csr_queue_forward_traverse(
                "active_queue",
                "queue_len",
                "edge_offsets",
                "edge_targets",
                "edge_kind_mask",
                "frontier_out",
                graph.node_count,
                graph.edge_count,
                queue_capacity,
                allow_mask,
            ),
            ResidentCsrQueueTraverseKind::RowStrided => {
                primitive_csr_queue_strided_forward_traverse(
                    "active_queue",
                    "queue_len",
                    "edge_offsets",
                    "edge_targets",
                    "edge_kind_mask",
                    "frontier_out",
                    graph.node_count,
                    graph.edge_count,
                    queue_capacity,
                    allow_mask,
                )
            }
            ResidentCsrQueueTraverseKind::MixedSplit {
                high_queue_capacity,
            } => primitive_csr_queue_strided_forward_traverse(
                "high_queue",
                "high_len",
                "edge_offsets",
                "edge_targets",
                "edge_kind_mask",
                "frontier_out",
                graph.node_count,
                graph.edge_count,
                high_queue_capacity,
                allow_mask,
            ),
        });
    let graph_handles = graph.handles;
    let traverse_handles = [
        queue_handle,
        handles[2],
        graph_handles[0],
        graph_handles[1],
        graph_handles[2],
        handles[1],
    ];
    macro_rules! append_traverse_steps {
        ($steps:ident) => {
            let split_handles;
            let high_traverse_handles;
            let split_low_program;
            let high_len_init_program;
            let high_len_handles;
            let high_traverse_grid;
            if let ResidentCsrQueueTraverseKind::MixedSplit {
                high_queue_capacity,
            } = traverse_kind
            {
                let [high_queue_handle, high_len_handle] =
                    ensure_high_queue_handles(dispatcher, scratch, high_queue_capacity)?;
                high_len_init_program = scratch.plan_cache.get_or_build(
                    AdaptiveTraversalPlanCacheKey::queue_len_init(
                        graph.layout_hash,
                        graph.node_count,
                        graph.edge_count,
                        words_u32,
                        high_queue_capacity,
                        device_features,
                    ),
                    || primitive_frontier_queue_len_init("high_len"),
                );
                high_len_handles = [high_len_handle];
                split_low_program = scratch.plan_cache.get_or_build(
                    AdaptiveTraversalPlanCacheKey::queue_split_low(
                        graph.layout_hash,
                        graph.node_count,
                        graph.edge_count,
                        words_u32,
                        queue_capacity,
                        high_queue_capacity,
                        STRIDED_FORWARD_MIN_ROW_DEGREE,
                        allow_mask,
                        device_features,
                    ),
                    || {
                        primitive_csr_queue_split_low_forward_traverse(
                            "active_queue",
                            "queue_len",
                            "edge_offsets",
                            "edge_targets",
                            "edge_kind_mask",
                            "frontier_out",
                            "high_queue",
                            "high_len",
                            graph.node_count,
                            graph.edge_count,
                            queue_capacity,
                            high_queue_capacity,
                            STRIDED_FORWARD_MIN_ROW_DEGREE,
                            allow_mask,
                        )
                    },
                );
                split_handles = [
                    queue_handle,
                    handles[2],
                    graph_handles[0],
                    graph_handles[1],
                    graph_handles[2],
                    handles[1],
                    high_queue_handle,
                    high_len_handle,
                ];
                high_traverse_handles = [
                    high_queue_handle,
                    high_len_handle,
                    graph_handles[0],
                    graph_handles[1],
                    graph_handles[2],
                    handles[1],
                ];
                high_traverse_grid = resident_csr_queue_traverse_grid(
                    high_queue_capacity,
                    ResidentCsrQueueTraverseKind::RowStrided,
                );
                $steps.push(ResidentDispatchStep {
                    program: &high_len_init_program,
                    handle_ids: &high_len_handles,
                    grid_override: Some([1, 1, 1]),
                });
                $steps.push(ResidentDispatchStep {
                    program: &split_low_program,
                    handle_ids: &split_handles,
                    grid_override: Some(resident_csr_queue_split_low_grid(queue_capacity)),
                });
                $steps.push(ResidentDispatchStep {
                    program: &traverse_program,
                    handle_ids: &high_traverse_handles,
                    grid_override: Some(high_traverse_grid),
                });
            } else {
                $steps.push(ResidentDispatchStep {
                    program: &traverse_program,
                    handle_ids: &traverse_handles,
                    grid_override: Some(traverse_grid),
                });
            }
        };
    }
    match materializer {
        ResidentCsrQueueMaterializer::AtomicWordScan => {
            let queue_len_init_program = scratch.plan_cache.get_or_build(
                AdaptiveTraversalPlanCacheKey::queue_len_init(
                    graph.layout_hash,
                    graph.node_count,
                    graph.edge_count,
                    words_u32,
                    queue_capacity,
                    device_features,
                ),
                || primitive_frontier_queue_len_init("queue_len"),
            );
            let queue_program = scratch.plan_cache.get_or_build(
                AdaptiveTraversalPlanCacheKey::frontier_to_queue(
                    graph.layout_hash,
                    graph.node_count,
                    graph.edge_count,
                    words_u32,
                    queue_capacity,
                    device_features,
                ),
                || {
                    primitive_frontier_words_to_queue_clear_out(
                        "frontier_in",
                        "active_queue",
                        "queue_len",
                        "frontier_out",
                        graph.node_count,
                        queue_capacity,
                    )
                },
            );
            let queue_len_handles = [handles[2]];
            let queue_handles = [handles[0], queue_handle, handles[2], handles[1]];
            let mut steps = vec![
                ResidentDispatchStep {
                    program: &queue_len_init_program,
                    handle_ids: &queue_len_handles,
                    grid_override: Some([1, 1, 1]),
                },
                ResidentDispatchStep {
                    program: &queue_program,
                    handle_ids: &queue_handles,
                    grid_override: Some(sparse_plan.frontier.frontier_word_grid),
                },
            ];
            append_traverse_steps!(steps);
            let uploads = [(handles[0], scratch.frontier_in_bytes.as_slice())];
            resident_sequence_single_u32_output_into(
                dispatcher,
                &uploads,
                steps.as_slice(),
                ResidentReadRange {
                    handle_id: handles[1],
                    byte_offset: 0,
                    byte_len: sparse_plan.frontier.frontier_bytes,
                },
                &mut scratch.readbacks,
                sparse_plan.frontier.work.layout.words,
                "adaptive_traverse_resident_graph_sparse_queue_step frontier_out",
                frontier_out,
            )
        }
        ResidentCsrQueueMaterializer::DeterministicWordPrefix => {
            let clear_frontier_out_program = scratch.plan_cache.get_or_build(
                AdaptiveTraversalPlanCacheKey::clear_frontier_out(
                    graph.layout_hash,
                    graph.node_count,
                    graph.edge_count,
                    words_u32,
                    device_features,
                ),
                || bitset_zero("frontier_out", words_u32),
            );
            let clear_handles = [handles[1]];
            let word_prefix = adaptive_word_prefix_scratch(words)?;
            let [word_partials, block_totals] =
                ensure_word_prefix_handles(dispatcher, scratch, &word_prefix)?;
            let word_counts_program = scratch.plan_cache.get_or_build(
                AdaptiveTraversalPlanCacheKey::frontier_word_counts(
                    graph.layout_hash,
                    graph.node_count,
                    graph.edge_count,
                    words_u32,
                    device_features,
                ),
                || {
                    primitive_frontier_word_counts(
                        "frontier_in",
                        "word_partials",
                        "block_totals",
                        graph.node_count,
                    )
                },
            );
            let word_count_handles = [handles[0], word_partials, block_totals];
            if frontier_word_prefix_uses_precomputed_offsets(word_prefix.block_count) {
                let block_offsets_program = scratch.plan_cache.get_or_build(
                    AdaptiveTraversalPlanCacheKey::frontier_word_block_offsets(
                        graph.layout_hash,
                        graph.node_count,
                        graph.edge_count,
                        words_u32,
                        device_features,
                    ),
                    || primitive_frontier_word_block_offsets("block_totals", graph.node_count),
                );
                let queue_program = scratch.plan_cache.get_or_build(
                    AdaptiveTraversalPlanCacheKey::frontier_word_block_offsets_queue(
                        graph.layout_hash,
                        graph.node_count,
                        graph.edge_count,
                        words_u32,
                        queue_capacity,
                        device_features,
                    ),
                    || {
                        primitive_frontier_word_block_offsets_queue(
                            "frontier_in",
                            "word_partials",
                            "block_totals",
                            "active_queue",
                            "queue_len",
                            graph.node_count,
                            queue_capacity,
                        )
                    },
                );
                let block_offsets_handles = [block_totals];
                let queue_handles = [
                    handles[0],
                    word_partials,
                    block_totals,
                    queue_handle,
                    handles[2],
                ];
                let mut steps = vec![
                    ResidentDispatchStep {
                        program: &clear_frontier_out_program,
                        handle_ids: &clear_handles,
                        grid_override: Some(sparse_plan.frontier.frontier_word_grid),
                    },
                    ResidentDispatchStep {
                        program: &word_counts_program,
                        handle_ids: &word_count_handles,
                        grid_override: Some([word_prefix.block_count, 1, 1]),
                    },
                    ResidentDispatchStep {
                        program: &block_offsets_program,
                        handle_ids: &block_offsets_handles,
                        grid_override: Some([1, 1, 1]),
                    },
                    ResidentDispatchStep {
                        program: &queue_program,
                        handle_ids: &queue_handles,
                        grid_override: Some(adaptive_frontier_word_grid(words)?),
                    },
                ];
                append_traverse_steps!(steps);
                let uploads = [(handles[0], scratch.frontier_in_bytes.as_slice())];
                resident_sequence_single_u32_output_into(
                    dispatcher,
                    &uploads,
                    steps.as_slice(),
                    ResidentReadRange {
                        handle_id: handles[1],
                        byte_offset: 0,
                        byte_len: sparse_plan.frontier.frontier_bytes,
                    },
                    &mut scratch.readbacks,
                    sparse_plan.frontier.work.layout.words,
                    "adaptive_traverse_resident_graph_sparse_queue_step frontier_out",
                    frontier_out,
                )
            } else {
                let queue_program = scratch.plan_cache.get_or_build(
                    AdaptiveTraversalPlanCacheKey::frontier_word_prefix_queue(
                        graph.layout_hash,
                        graph.node_count,
                        graph.edge_count,
                        words_u32,
                        queue_capacity,
                        device_features,
                    ),
                    || {
                        primitive_frontier_word_prefix_queue(
                            "frontier_in",
                            "word_partials",
                            "block_totals",
                            "active_queue",
                            "queue_len",
                            graph.node_count,
                            queue_capacity,
                        )
                    },
                );
                let queue_handles = [
                    handles[0],
                    word_partials,
                    block_totals,
                    queue_handle,
                    handles[2],
                ];
                let mut steps = vec![
                    ResidentDispatchStep {
                        program: &clear_frontier_out_program,
                        handle_ids: &clear_handles,
                        grid_override: Some(sparse_plan.frontier.frontier_word_grid),
                    },
                    ResidentDispatchStep {
                        program: &word_counts_program,
                        handle_ids: &word_count_handles,
                        grid_override: Some([word_prefix.block_count, 1, 1]),
                    },
                    ResidentDispatchStep {
                        program: &queue_program,
                        handle_ids: &queue_handles,
                        grid_override: Some(adaptive_frontier_word_grid(words)?),
                    },
                ];
                append_traverse_steps!(steps);
                let uploads = [(handles[0], scratch.frontier_in_bytes.as_slice())];
                resident_sequence_single_u32_output_into(
                    dispatcher,
                    &uploads,
                    steps.as_slice(),
                    ResidentReadRange {
                        handle_id: handles[1],
                        byte_offset: 0,
                        byte_len: sparse_plan.frontier.frontier_bytes,
                    },
                    &mut scratch.readbacks,
                    sparse_plan.frontier.work.layout.words,
                    "adaptive_traverse_resident_graph_sparse_queue_step frontier_out",
                    frontier_out,
                )
            }
        }
    }
}

/// Run one adaptive traversal step using the runtime mode selector.
///
/// # Errors
///
/// Propagates resident dispatch failures and malformed frontier/readback shapes.
#[allow(clippy::too_many_arguments)]
pub fn adaptive_traverse_resident_graph_auto_step_with_scratch_into(
    dispatcher: &dyn OptimizerDispatcher,
    graph: &ResidentAdaptiveTraversalGraph,
    frontier_in: &[u32],
    allow_mask: u32,
    dense_threshold_pct: u32,
    scratch: &mut AdaptiveTraversalResidentScratch,
    frontier_out: &mut Vec<u32>,
) -> Result<AdaptiveTraversalMode, DispatchError> {
    let auto_plan = plan_adaptive_resident_auto_step(
        graph.node_count,
        graph.edge_count,
        frontier_in,
        dense_threshold_pct,
    )
    .map_err(DispatchError::BadInputs)?;
    if !auto_plan.frontier.work.has_active_bits {
        write_zero_frontier_result(auto_plan.frontier.work.layout.words, frontier_out);
        return Ok(AdaptiveTraversalMode::SparseQueue);
    }
    let mode = auto_plan.mode;
    match mode {
        AdaptiveTraversalMode::SparseQueue => {
            adaptive_traverse_resident_graph_sparse_queue_step_with_scratch_into(
                dispatcher,
                graph,
                frontier_in,
                allow_mask,
                scratch,
                frontier_out,
            )?;
        }
        AdaptiveTraversalMode::SparseDense => {
            adaptive_traverse_resident_graph_step_with_scratch_into(
                dispatcher,
                graph,
                frontier_in,
                allow_mask,
                dense_threshold_pct,
                scratch,
                frontier_out,
            )?;
        }
    }
    Ok(mode)
}

fn write_zero_frontier_result(words: usize, frontier_out: &mut Vec<u32>) {
    frontier_out.clear();
    frontier_out.resize(words, 0);
}

fn ensure_frontier_handles(
    dispatcher: &dyn OptimizerDispatcher,
    scratch: &mut AdaptiveTraversalResidentScratch,
    plan: &AdaptiveResidentFrontierPlan,
) -> Result<[u64; 3], DispatchError> {
    let frontier_bytes = plan.frontier_bytes;
    if scratch.frontier_bytes == frontier_bytes {
        if let Some(handles) = scratch.handles {
            return Ok(handles);
        }
    }
    scratch.free(dispatcher)?;
    let handles = alloc_resident_buffers(
        dispatcher,
        [frontier_bytes, frontier_bytes, plan.popcount_bytes],
        "adaptive traversal frontier scratch",
    )?;
    scratch.handles = Some(handles);
    scratch.frontier_bytes = frontier_bytes;
    Ok(handles)
}

fn ensure_queue_handle(
    dispatcher: &dyn OptimizerDispatcher,
    scratch: &mut AdaptiveTraversalResidentScratch,
    queue_bytes: usize,
) -> Result<u64, DispatchError> {
    if scratch.queue_bytes >= queue_bytes {
        if let Some(handle) = scratch.queue_handle {
            return Ok(handle);
        }
    }
    if let Some(handle) = scratch.queue_handle.take() {
        dispatcher.free_resident(handle)?;
    }
    let [handle] = alloc_resident_buffers(
        dispatcher,
        [queue_bytes],
        "adaptive traversal queue scratch",
    )?;
    scratch.queue_handle = Some(handle);
    scratch.queue_bytes = queue_bytes;
    Ok(handle)
}

fn ensure_high_queue_handles(
    dispatcher: &dyn OptimizerDispatcher,
    scratch: &mut AdaptiveTraversalResidentScratch,
    high_queue_capacity: u32,
) -> Result<[u64; 2], DispatchError> {
    let high_queue_bytes =
        u32_word_bytes(high_queue_capacity as usize, "high-degree active queue")?;
    if scratch.high_queue_bytes >= high_queue_bytes {
        if let (Some(high_queue), Some(high_len)) =
            (scratch.high_queue_handle, scratch.high_len_handle)
        {
            return Ok([high_queue, high_len]);
        }
    }
    free_high_queue_handles(dispatcher, scratch)?;
    let [high_queue, high_len] = alloc_resident_buffers(
        dispatcher,
        [high_queue_bytes, std::mem::size_of::<u32>()],
        "adaptive traversal high-degree queue scratch",
    )?;
    scratch.high_queue_handle = Some(high_queue);
    scratch.high_len_handle = Some(high_len);
    scratch.high_queue_bytes = high_queue_bytes;
    Ok([high_queue, high_len])
}

fn free_high_queue_handles(
    dispatcher: &dyn OptimizerDispatcher,
    scratch: &mut AdaptiveTraversalResidentScratch,
) -> Result<(), DispatchError> {
    let mut handles = [0_u64; 2];
    let mut handle_count = 0;
    if let Some(high_queue) = scratch.high_queue_handle.take() {
        handles[handle_count] = high_queue;
        handle_count += 1;
    }
    if let Some(high_len) = scratch.high_len_handle.take() {
        handles[handle_count] = high_len;
        handle_count += 1;
    }
    scratch.high_queue_bytes = 0;
    if handle_count == 0 {
        return Ok(());
    }
    free_unique_resident_handles(
        dispatcher,
        &handles[..handle_count],
        "adaptive traversal high-degree queue scratch",
    )
}

fn ensure_word_prefix_handles(
    dispatcher: &dyn OptimizerDispatcher,
    scratch: &mut AdaptiveTraversalResidentScratch,
    word_prefix: &FrontierWordPrefixScratch,
) -> Result<[u64; 2], DispatchError> {
    let word_partials_bytes = u32_word_bytes(word_prefix.partial_words, "word-prefix partials")?;
    let word_block_totals_bytes =
        u32_word_bytes(word_prefix.block_total_words, "word-prefix block totals")?;
    if scratch.word_partials_bytes == word_partials_bytes
        && scratch.word_block_totals_bytes == word_block_totals_bytes
    {
        if let (Some(word_partials), Some(block_totals)) = (
            scratch.word_partials_handle,
            scratch.word_block_totals_handle,
        ) {
            return Ok([word_partials, block_totals]);
        }
    }
    free_word_prefix_handles(dispatcher, scratch)?;
    let [word_partials, block_totals] = alloc_resident_buffers(
        dispatcher,
        [word_partials_bytes, word_block_totals_bytes],
        "adaptive traversal word-prefix queue scratch",
    )?;
    scratch.word_partials_handle = Some(word_partials);
    scratch.word_block_totals_handle = Some(block_totals);
    scratch.word_partials_bytes = word_partials_bytes;
    scratch.word_block_totals_bytes = word_block_totals_bytes;
    Ok([word_partials, block_totals])
}

fn free_word_prefix_handles(
    dispatcher: &dyn OptimizerDispatcher,
    scratch: &mut AdaptiveTraversalResidentScratch,
) -> Result<(), DispatchError> {
    let mut handles = [0_u64; 2];
    let mut handle_count = 0;
    if let Some(word_partials) = scratch.word_partials_handle.take() {
        handles[handle_count] = word_partials;
        handle_count += 1;
    }
    if let Some(block_totals) = scratch.word_block_totals_handle.take() {
        handles[handle_count] = block_totals;
        handle_count += 1;
    }
    scratch.word_partials_bytes = 0;
    scratch.word_block_totals_bytes = 0;
    if handle_count == 0 {
        return Ok(());
    }
    free_unique_resident_handles(
        dispatcher,
        &handles[..handle_count],
        "adaptive traversal word-prefix queue scratch",
    )
}

fn adaptive_word_prefix_scratch(words: usize) -> Result<FrontierWordPrefixScratch, DispatchError> {
    frontier_word_prefix_scratch(words).map_err(DispatchError::BackendError)
}

fn adaptive_frontier_word_grid(words: usize) -> Result<[u32; 3], DispatchError> {
    frontier_word_dispatch_grid(words).map_err(DispatchError::BackendError)
}
