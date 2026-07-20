//! Metric-key plumbing: small key/value helpers used by `collect.rs` and
//! `report.rs` to look up metrics by stable name.

#[path = "metric_keys/graph.rs"]
mod graph;

pub(super) fn rate_per_second_x1000(units: u64, wall_ns: u64, scale: u64) -> u64 {
    let numerator = u128::from(units) * 1_000_000_000_000u128;
    let denominator = u128::from(wall_ns) * u128::from(scale);
    (numerator / denominator).min(u128::from(u64::MAX)) as u64
}

pub(super) fn custom_metric_value(
    metrics: &crate::api::metric::BenchMetrics,
    name: &str,
) -> Option<u64> {
    metrics
        .custom
        .iter()
        .find(|point| point.name == name)
        .map(|point| point.value)
}

pub(super) fn gpu_counter_value(
    metrics: &crate::api::metric::BenchMetrics,
    name: &str,
) -> Option<u64> {
    metrics
        .gpu_counter
        .iter()
        .find(|counter| counter.name == name)
        .map(|counter| counter.value)
}

pub(super) fn custom_metric_key(prefix: &'static str, name: &str) -> Option<&'static str> {
    if let Some(key) = graph::custom_graph_metric_key(prefix, name) {
        return Some(key);
    }

    match (prefix, name) {
        ("", "flop_count") => Some("flop_count"),
        ("", "clock_mem_max_mhz") => Some("clock_mem_max_mhz"),
        ("", "clock_mem_current_mhz") => Some("clock_mem_current_mhz"),
        ("", "clock_graphics_max_mhz") => Some("clock_graphics_max_mhz"),
        ("", "clock_graphics_current_mhz") => Some("clock_graphics_current_mhz"),
        ("", "pstate") => Some("pstate"),
        ("", "clock_throttle_reasons_active") => Some("clock_throttle_reasons_active"),
        ("", "power_draw_w") => Some("power_draw_w"),
        ("", "power_limit_w") => Some("power_limit_w"),
        ("", "temperature_c") => Some("temperature_c"),
        ("", "memory_total_mib") => Some("memory_total_mib"),
        ("", "memory_used_mib") => Some("memory_used_mib"),
        ("", "memory_free_mib") => Some("memory_free_mib"),
        ("", "utilization_gpu_pct") => Some("utilization_gpu_pct"),
        ("", "utilization_mem_pct") => Some("utilization_mem_pct"),
        ("", "memory_peak_gb_s_x1000") => Some("memory_peak_gb_s_x1000"),
        ("", "thermal_unstable") => Some("thermal_unstable"),
        ("", "megakernel_slots") => Some("megakernel_slots"),
        ("", "megakernel_dispatch_latency_ns") => Some("megakernel_dispatch_latency_ns"),
        ("", "megakernel_slots_per_sec_x1000") => Some("megakernel_slots_per_sec_x1000"),
        ("", "megakernel_roundtrip_buffers") => Some("megakernel_roundtrip_buffers"),
        ("", "megakernel_speculation_samples") => Some("megakernel_speculation_samples"),
        ("", "megakernel_speculation_adopted") => Some("megakernel_speculation_adopted"),
        ("", "megakernel_speculation_rejected") => Some("megakernel_speculation_rejected"),
        ("", "megakernel_speculation_side_compile_cost_ns") => {
            Some("megakernel_speculation_side_compile_cost_ns")
        }
        ("", "megakernel_speculation_autotune_records") => {
            Some("megakernel_speculation_autotune_records")
        }
        ("", "megakernel_queue_plan_ns") => Some("megakernel_queue_plan_ns"),
        ("", "megakernel_queue_publish_ns") => Some("megakernel_queue_publish_ns"),
        ("", "megakernel_backend_dispatch_ns") => Some("megakernel_backend_dispatch_ns"),
        ("", "megakernel_lineage_ns") => Some("megakernel_lineage_ns"),
        ("", "megakernel_published_items") => Some("megakernel_published_items"),
        ("", "megakernel_lineage_items") => Some("megakernel_lineage_items"),
        ("", "megakernel_deduped_items") => Some("megakernel_deduped_items"),
        ("", "megakernel_items_processed") => Some("megakernel_items_processed"),
        ("", "megakernel_items_remaining") => Some("megakernel_items_remaining"),
        ("", "megakernel_bytes_uploaded") => Some("megakernel_bytes_uploaded"),
        ("", "megakernel_bytes_read_back") => Some("megakernel_bytes_read_back"),
        ("", "megakernel_bytes_moved") => Some("megakernel_bytes_moved"),
        ("", "megakernel_resident_allocations") => Some("megakernel_resident_allocations"),
        ("", "megakernel_kernel_launches") => Some("megakernel_kernel_launches"),
        ("", "megakernel_sync_points") => Some("megakernel_sync_points"),
        ("", "megakernel_occupancy_proxy_bps") => Some("megakernel_occupancy_proxy_bps"),
        ("", "megakernel_frontier_density_bps") => Some("megakernel_frontier_density_bps"),
        ("", "megakernel_readback_buffers") => Some("megakernel_readback_buffers"),
        ("", "megakernel_compiled_pipeline_cache_hit") => {
            Some("megakernel_compiled_pipeline_cache_hit")
        }
        ("", "megakernel_resident_input_cache_hit") => Some("megakernel_resident_input_cache_hit"),
        ("", "megakernel_topology") => Some("megakernel_topology"),
        ("", "megakernel_pressure") => Some("megakernel_pressure"),
        ("", "megakernel_execution_mode") => Some("megakernel_execution_mode"),
        ("", "megakernel_hit_capacity") => Some("megakernel_hit_capacity"),
        ("", "megakernel_estimated_peak_device_bytes") => {
            Some("megakernel_estimated_peak_device_bytes")
        }
        ("", "megakernel_device_memory_budget_bytes") => {
            Some("megakernel_device_memory_budget_bytes")
        }
        ("", "megakernel_condition_slots") => Some("megakernel_condition_slots"),
        ("", "megakernel_condition_fired") => Some("megakernel_condition_fired"),
        ("", "megakernel_condition_slots_per_sec_x1000") => {
            Some("megakernel_condition_slots_per_sec_x1000")
        }
        ("", "conditional_eval_resident_buffers") => Some("conditional_eval_resident_buffers"),
        ("", "conditional_eval_device_reset_sequence") => {
            Some("conditional_eval_device_reset_sequence")
        }
        ("", "conditional_eval_resident_reset_bytes") => {
            Some("conditional_eval_resident_reset_bytes")
        }
        ("", "conditional_batch_resident_buffers") => Some("conditional_batch_resident_buffers"),
        ("", "conditional_batch_device_reset_sequence") => {
            Some("conditional_batch_device_reset_sequence")
        }
        ("", "conditional_batch_resident_reset_bytes") => {
            Some("conditional_batch_resident_reset_bytes")
        }
        ("", "dataflow_nodes") => Some("dataflow_nodes"),
        ("", "dataflow_bitset_words") => Some("dataflow_bitset_words"),
        ("", "weir_nodes") => Some("weir_nodes"),
        ("", "weir_bitset_words") => Some("weir_bitset_words"),
        ("", "dataflow_graph_nodes") => Some("dataflow_graph_nodes"),
        ("", "dataflow_graph_edges") => Some("dataflow_graph_edges"),
        ("", "dataflow_ifds_step") => Some("dataflow_ifds_step"),
        ("", "dataflow_points_to_alias_step") => Some("dataflow_points_to_alias_step"),
        ("", "dataflow_ifds_skewed_nodes") => Some("dataflow_ifds_skewed_nodes"),
        ("", "dataflow_ifds_skewed_edges") => Some("dataflow_ifds_skewed_edges"),
        ("", "dataflow_ifds_skewed_frontier_words") => Some("dataflow_ifds_skewed_frontier_words"),
        ("", "dataflow_ifds_skewed_active_sources") => Some("dataflow_ifds_skewed_active_sources"),
        ("", "dataflow_ifds_skewed_allowed_edges") => Some("dataflow_ifds_skewed_allowed_edges"),
        ("", "dataflow_ifds_skewed_filtered_edges") => Some("dataflow_ifds_skewed_filtered_edges"),
        ("", "dataflow_ifds_skewed_output_words_set") => {
            Some("dataflow_ifds_skewed_output_words_set")
        }
        ("", "dataflow_ifds_skewed_max_degree") => Some("dataflow_ifds_skewed_max_degree"),
        ("", "dataflow_ifds_skewed_high_degree_sources") => {
            Some("dataflow_ifds_skewed_high_degree_sources")
        }
        ("", "dataflow_ifds_skewed_resident_buffers") => {
            Some("dataflow_ifds_skewed_resident_buffers")
        }
        ("", "dataflow_ifds_skewed_workgroup_size_x") => {
            Some("dataflow_ifds_skewed_workgroup_size_x")
        }
        ("", "dataflow_ifds_skewed_speedup_x1000") => Some("dataflow_ifds_skewed_speedup_x1000"),
        ("", "dataflow_ifds_queue_capacity") => Some("dataflow_ifds_queue_capacity"),
        ("", "dataflow_ifds_queue_lane_reduction_x1000") => {
            Some("dataflow_ifds_queue_lane_reduction_x1000")
        }
        ("", "dataflow_ifds_queue_resident_buffers") => {
            Some("dataflow_ifds_queue_resident_buffers")
        }
        ("", "dataflow_ifds_queue_workgroup_size_x") => {
            Some("dataflow_ifds_queue_workgroup_size_x")
        }
        ("", "dataflow_ifds_queue_parallel_materializer") => {
            Some("dataflow_ifds_queue_parallel_materializer")
        }
        ("", "dataflow_ifds_queue_row_strided_traverse") => {
            Some("dataflow_ifds_queue_row_strided_traverse")
        }
        ("", "dataflow_ifds_queue_split_high_degree") => {
            Some("dataflow_ifds_queue_split_high_degree")
        }
        ("", "dataflow_ifds_queue_high_degree_threshold") => {
            Some("dataflow_ifds_queue_high_degree_threshold")
        }
        ("", "dataflow_ifds_queue_fused_frontier_clear") => {
            Some("dataflow_ifds_queue_fused_frontier_clear")
        }
        ("", "dataflow_ifds_queue_reset_grid_lanes") => {
            Some("dataflow_ifds_queue_reset_grid_lanes")
        }
        ("", "dataflow_ifds_queue_high_degree_capacity") => {
            Some("dataflow_ifds_queue_high_degree_capacity")
        }
        ("", "dataflow_ifds_queue_traverse_logical_lanes") => {
            Some("dataflow_ifds_queue_traverse_logical_lanes")
        }
        ("", "dataflow_ifds_queue_traverse_lane_reduction_x1000") => {
            Some("dataflow_ifds_queue_traverse_lane_reduction_x1000")
        }
        ("", "dataflow_ifds_queue_speedup_x1000") => Some("dataflow_ifds_queue_speedup_x1000"),
        ("", "dataflow_ifds_closure_nodes") => Some("dataflow_ifds_closure_nodes"),
        ("", "dataflow_ifds_closure_edges") => Some("dataflow_ifds_closure_edges"),
        ("", "dataflow_ifds_closure_frontier_words") => {
            Some("dataflow_ifds_closure_frontier_words")
        }
        ("", "dataflow_ifds_closure_active_sources") => {
            Some("dataflow_ifds_closure_active_sources")
        }
        ("", "dataflow_ifds_closure_output_words_set") => {
            Some("dataflow_ifds_closure_output_words_set")
        }
        ("", "dataflow_ifds_closure_max_degree") => Some("dataflow_ifds_closure_max_degree"),
        ("", "dataflow_ifds_closure_high_degree_sources") => {
            Some("dataflow_ifds_closure_high_degree_sources")
        }
        ("", "dataflow_ifds_closure_iterations") => Some("dataflow_ifds_closure_iterations"),
        ("", "dataflow_ifds_closure_changed") => Some("dataflow_ifds_closure_changed"),
        ("", "dataflow_ifds_closure_fixpoint_iterations") => {
            Some("dataflow_ifds_closure_fixpoint_iterations")
        }
        ("", "dataflow_ifds_closure_max_iterations") => {
            Some("dataflow_ifds_closure_max_iterations")
        }
        ("", "dataflow_ifds_closure_elided_iterations") => {
            Some("dataflow_ifds_closure_elided_iterations")
        }
        ("", "dataflow_ifds_closure_resident_buffers") => {
            Some("dataflow_ifds_closure_resident_buffers")
        }
        ("", "dataflow_ifds_closure_resident_reset_bytes") => {
            Some("dataflow_ifds_closure_resident_reset_bytes")
        }
        ("", "dataflow_ifds_closure_device_reset_sequence") => {
            Some("dataflow_ifds_closure_device_reset_sequence")
        }
        ("", "dataflow_ifds_closure_workgroup_size_x") => {
            Some("dataflow_ifds_closure_workgroup_size_x")
        }
        ("", "dataflow_ifds_closure_speedup_x1000") => Some("dataflow_ifds_closure_speedup_x1000"),
        ("", "dataflow_ifds_closure_queue_capacity") => {
            Some("dataflow_ifds_closure_queue_capacity")
        }
        ("", "dataflow_ifds_closure_queue_capacity_reduction_x1000") => {
            Some("dataflow_ifds_closure_queue_capacity_reduction_x1000")
        }
        ("", "dataflow_ifds_closure_seed_queue_len") => {
            Some("dataflow_ifds_closure_seed_queue_len")
        }
        ("", "dataflow_ifds_closure_dispatch_count") => {
            Some("dataflow_ifds_closure_dispatch_count")
        }
        ("", "dataflow_ifds_closure_total_queue_pops") => {
            Some("dataflow_ifds_closure_total_queue_pops")
        }
        ("", "dataflow_ifds_closure_max_wave_queue_len") => {
            Some("dataflow_ifds_closure_max_wave_queue_len")
        }
        ("", "dataflow_ifds_closure_queue_delta") => Some("dataflow_ifds_closure_queue_delta"),
        ("", "dataflow_ifds_closure_row_strided_delta") => {
            Some("dataflow_ifds_closure_row_strided_delta")
        }
        ("", "dataflow_ifds_closure_seed_scan_elided") => {
            Some("dataflow_ifds_closure_seed_scan_elided")
        }
        ("", "dataflow_ifds_closure_wave_profiled") => Some("dataflow_ifds_closure_wave_profiled"),
        ("", "dataflow_ifds_closure_fixed_delta_source_slots") => {
            Some("dataflow_ifds_closure_fixed_delta_source_slots")
        }
        ("", "dataflow_ifds_closure_profiled_delta_source_slots") => {
            Some("dataflow_ifds_closure_profiled_delta_source_slots")
        }
        ("", "dataflow_ifds_closure_elided_delta_source_slots") => {
            Some("dataflow_ifds_closure_elided_delta_source_slots")
        }
        ("", "dataflow_ifds_closure_fixed_delta_lanes") => {
            Some("dataflow_ifds_closure_fixed_delta_lanes")
        }
        ("", "dataflow_ifds_closure_profiled_delta_lanes") => {
            Some("dataflow_ifds_closure_profiled_delta_lanes")
        }
        ("", "dataflow_ifds_closure_elided_delta_lanes") => {
            Some("dataflow_ifds_closure_elided_delta_lanes")
        }
        ("", "dataflow_ifds_closure_delta_lane_elision_x1000") => {
            Some("dataflow_ifds_closure_delta_lane_elision_x1000")
        }
        ("", "dataflow_ifds_closure_launch_delta_lanes") => {
            Some("dataflow_ifds_closure_launch_delta_lanes")
        }
        ("", "dataflow_ifds_closure_launch_elided_delta_lanes") => {
            Some("dataflow_ifds_closure_launch_elided_delta_lanes")
        }
        ("", "dataflow_ifds_closure_launch_lane_elision_x1000") => {
            Some("dataflow_ifds_closure_launch_lane_elision_x1000")
        }
        ("", "scan_ac_irregular_haystack_bytes") => Some("scan_ac_irregular_haystack_bytes"),
        ("", "scan_ac_irregular_packed_haystack_words") => {
            Some("scan_ac_irregular_packed_haystack_words")
        }
        ("", "scan_ac_irregular_patterns") => Some("scan_ac_irregular_patterns"),
        ("", "scan_ac_irregular_dfa_states") => Some("scan_ac_irregular_dfa_states"),
        ("", "scan_ac_irregular_max_pattern_len") => Some("scan_ac_irregular_max_pattern_len"),
        ("", "scan_ac_irregular_output_records") => Some("scan_ac_irregular_output_records"),
        ("", "scan_ac_irregular_expected_matches") => Some("scan_ac_irregular_expected_matches"),
        ("", "scan_ac_irregular_max_matches") => Some("scan_ac_irregular_max_matches"),
        ("", "scan_ac_irregular_match_readback_bytes") => {
            Some("scan_ac_irregular_match_readback_bytes")
        }
        ("", "scan_ac_irregular_avoided_match_readback_bytes") => {
            Some("scan_ac_irregular_avoided_match_readback_bytes")
        }
        ("", "scan_ac_irregular_bounded_ranges_prefilter") => {
            Some("scan_ac_irregular_bounded_ranges_prefilter")
        }
        ("", "scan_ac_irregular_bounded_ranges_prefilter_skipped_lanes_x1000") => {
            Some("scan_ac_irregular_bounded_ranges_prefilter_skipped_lanes_x1000")
        }
        ("", "scan_ac_irregular_bounded_ranges_suffix3_prefilter") => {
            Some("scan_ac_irregular_bounded_ranges_suffix3_prefilter")
        }
        ("", "scan_ac_irregular_count_only") => Some("scan_ac_irregular_count_only"),
        ("", "scan_ac_irregular_count_prefilter") => Some("scan_ac_irregular_count_prefilter"),
        ("", "scan_ac_irregular_count_readback_bytes") => {
            Some("scan_ac_irregular_count_readback_bytes")
        }
        ("", "scan_ac_irregular_candidate_end_bytes") => {
            Some("scan_ac_irregular_candidate_end_bytes")
        }
        ("", "scan_ac_irregular_candidate_end_lanes") => {
            Some("scan_ac_irregular_candidate_end_lanes")
        }
        ("", "scan_ac_irregular_candidate_suffix2_lanes") => {
            Some("scan_ac_irregular_candidate_suffix2_lanes")
        }
        ("", "scan_ac_irregular_candidate_suffix3_lanes") => {
            Some("scan_ac_irregular_candidate_suffix3_lanes")
        }
        ("", "scan_ac_irregular_count_prefilter_skipped_lanes_x1000") => {
            Some("scan_ac_irregular_count_prefilter_skipped_lanes_x1000")
        }
        ("", "scan_ac_irregular_suffix2_extra_skipped_lanes_x1000") => {
            Some("scan_ac_irregular_suffix2_extra_skipped_lanes_x1000")
        }
        ("", "scan_ac_irregular_suffix3_extra_skipped_lanes_x1000") => {
            Some("scan_ac_irregular_suffix3_extra_skipped_lanes_x1000")
        }
        ("", "scan_ac_irregular_planted_matches") => Some("scan_ac_irregular_planted_matches"),
        ("", "scan_ac_irregular_resident_buffers") => Some("scan_ac_irregular_resident_buffers"),
        ("", "scan_ac_irregular_resident_reset_bytes") => {
            Some("scan_ac_irregular_resident_reset_bytes")
        }
        ("", "scan_ac_irregular_device_reset_sequence") => {
            Some("scan_ac_irregular_device_reset_sequence")
        }
        ("", "scan_ac_irregular_workgroup_size_x") => Some("scan_ac_irregular_workgroup_size_x"),
        ("", "scan_ac_irregular_speedup_x1000") => Some("scan_ac_irregular_speedup_x1000"),
        ("", "sparse_items") => Some("sparse_items"),
        ("", "callgraph_nodes") => Some("callgraph_nodes"),
        ("", "metadata_records") => Some("metadata_records"),
        ("", "metadata_expected_matches") => Some("metadata_expected_matches"),
        ("", "metadata_resident_buffers") => Some("metadata_resident_buffers"),
        ("", "metadata_resident_reset_bytes") => Some("metadata_resident_reset_bytes"),
        ("", "metadata_resident_batch_wall_ns") => Some("metadata_resident_batch_wall_ns"),
        ("", "metadata_resident_batch_len") => Some("metadata_resident_batch_len"),
        ("", "synthetic_resident_buffers") => Some("synthetic_resident_buffers"),
        ("", "synthetic_resident_reset_bytes") => Some("synthetic_resident_reset_bytes"),
        ("", "synthetic_resident_batch_wall_ns") => Some("synthetic_resident_batch_wall_ns"),
        ("", "synthetic_resident_batch_len") => Some("synthetic_resident_batch_len"),
        ("", "scatter_materialized_words") => Some("scatter_materialized_words"),
        ("", "lower_ops_before") => Some("lower_ops_before"),
        ("", "lower_ops_after") => Some("lower_ops_after"),
        ("", "lower_ops_eliminated") => Some("lower_ops_eliminated"),
        ("", "lower_bindings_dropped") => Some("lower_bindings_dropped"),
        ("", "lower_off_graph_dropped") => Some("lower_off_graph_dropped"),
        ("", "lower_baseline_issue_score") => Some("lower_baseline_issue_score"),
        ("", "lower_optimized_issue_score") => Some("lower_optimized_issue_score"),
        ("", "lower_coalesce_problematic_before") => Some("lower_coalesce_problematic_before"),
        ("", "lower_coalesce_problematic_after") => Some("lower_coalesce_problematic_after"),
        ("", "lower_shared_candidates_before") => Some("lower_shared_candidates_before"),
        ("", "lower_shared_candidates_after") => Some("lower_shared_candidates_after"),
        ("", "lower_bank_critical_before") => Some("lower_bank_critical_before"),
        ("", "lower_bank_critical_after") => Some("lower_bank_critical_after"),
        ("", "lower_vec_pack_chains_before") => Some("lower_vec_pack_chains_before"),
        ("", "lower_vec_pack_chains_after") => Some("lower_vec_pack_chains_after"),
        ("", "lower_vec_pack_ops_eliminable_before") => {
            Some("lower_vec_pack_ops_eliminable_before")
        }
        ("", "lower_vec_pack_ops_eliminable_after") => Some("lower_vec_pack_ops_eliminable_after"),
        ("", "lower_layout_candidates_before") => Some("lower_layout_candidates_before"),
        ("", "lower_layout_candidates_after") => Some("lower_layout_candidates_after"),
        ("", "lower_converged") => Some("lower_converged"),
        ("", "optimizer_input_nodes") => Some("optimizer_input_nodes"),
        ("", "optimizer_output_nodes") => Some("optimizer_output_nodes"),
        ("", "optimizer_nodes_eliminated") => Some("optimizer_nodes_eliminated"),
        ("", "alias_pass_wins") => Some("alias_pass_wins"),
        ("", "alias_fact_count") => Some("alias_fact_count"),
        ("", "alias_cross_binding_fact_count") => Some("alias_cross_binding_fact_count"),
        ("", "reaching_def_fact_count") => Some("reaching_def_fact_count"),
        ("", "alias_total_ops_after") => Some("alias_total_ops_after"),
        ("", "conservative_total_ops_after") => Some("conservative_total_ops_after"),
        ("", "alias_dse_store_count") => Some("alias_dse_store_count"),
        ("", "conservative_dse_store_count") => Some("conservative_dse_store_count"),
        ("", "alias_stlf_final_value_id") => Some("alias_stlf_final_value_id"),
        ("", "conservative_stlf_final_value_id") => Some("conservative_stlf_final_value_id"),
        ("", "alias_licm_loop_loads") => Some("alias_licm_loop_loads"),
        ("", "conservative_licm_loop_loads") => Some("conservative_licm_loop_loads"),
        ("", "alias_fusion_loop_count") => Some("alias_fusion_loop_count"),
        ("", "conservative_fusion_loop_count") => Some("conservative_fusion_loop_count"),
        ("", "alias_fission_loop_count") => Some("alias_fission_loop_count"),
        ("", "conservative_fission_loop_count") => Some("conservative_fission_loop_count"),
        ("", "cpu_digest") => Some("cpu_digest"),
        ("", "gpu_digest") => Some("gpu_digest"),
        ("", "active_time_ns") => Some("active_time_ns"),
        ("", "transfer_bytes") => Some("transfer_bytes"),
        ("", "benchmark_repeats") => Some("benchmark_repeats"),
        ("", "ptx_corpus_kernels") => Some("ptx_corpus_kernels"),
        ("", "ptx_predication_candidates") => Some("ptx_predication_candidates"),
        ("", "ptx_safe_predication_candidates") => Some("ptx_safe_predication_candidates"),
        ("", "ptx_vec_load_candidates") => Some("ptx_vec_load_candidates"),
        ("", "ptx_vec_store_candidates") => Some("ptx_vec_store_candidates"),
        ("", "ptx_async_copy_candidates") => Some("ptx_async_copy_candidates"),
        ("", "ptx_tensor_core_candidates") => Some("ptx_tensor_core_candidates"),
        ("", "ptx_ldmatrix_capable_targets") => Some("ptx_ldmatrix_capable_targets"),
        ("", "ptx_scheduled_fillers") => Some("ptx_scheduled_fillers"),
        ("", "ptx_predicated_stores") => Some("ptx_predicated_stores"),
        ("", "ptx_branch_labels") => Some("ptx_branch_labels"),
        ("", "ptx_cp_async_emitted") => Some("ptx_cp_async_emitted"),
        ("", "ptx_mma_sync_emitted") => Some("ptx_mma_sync_emitted"),
        ("", "ptx_vectorized_loads_emitted") => Some("ptx_vectorized_loads_emitted"),
        ("", "ptx_vectorized_stores_emitted") => Some("ptx_vectorized_stores_emitted"),
        ("", "ptx_vector_kernel_scalar_loads") => Some("ptx_vector_kernel_scalar_loads"),
        ("", "ptx_vector_kernel_scalar_stores") => Some("ptx_vector_kernel_scalar_stores"),
        ("", "ptx_vector_kernel_scalar_index_adds") => Some("ptx_vector_kernel_scalar_index_adds"),
        ("", "ptx_bytes_emitted") => Some("ptx_bytes_emitted"),
        // The CUDA PTX patterns codegen corpus emits its own source-cache stats
        // (entries/hits/misses over the unique kernel set). Without these keys the
        // case's values are silently dropped by `collect_custom_metrics`, and the
        // driver's backend_metric_snapshot (0 for a codegen-only case that never
        // dispatches a module) is the only value left via `or_insert_with` — which
        // zeroed `cuda_ptx_source_cache_entries` in the release artifact and failed
        // the PTX pattern evidence gate. Registering them keeps the corpus values.
        ("", "cuda_ptx_source_cache_entries") => Some("cuda_ptx_source_cache_entries"),
        ("", "cuda_ptx_source_cache_hits") => Some("cuda_ptx_source_cache_hits"),
        ("", "cuda_ptx_source_cache_misses") => Some("cuda_ptx_source_cache_misses"),
        ("", "egraph_case_count") => Some("egraph_case_count"),
        ("", "egraph_bitwise_case_count") => Some("egraph_bitwise_case_count"),
        ("", "egraph_boolean_case_count") => Some("egraph_boolean_case_count"),
        ("", "egraph_input_ops") => Some("egraph_input_ops"),
        ("", "egraph_output_ops") => Some("egraph_output_ops"),
        ("", "egraph_baseline_ops_after") => Some("egraph_baseline_ops_after"),
        ("", "egraph_iterations") => Some("egraph_iterations"),
        ("", "egraph_equality_classes") => Some("egraph_equality_classes"),
        ("", "egraph_applied_rewrites") => Some("egraph_applied_rewrites"),
        ("", "egraph_hit_iteration_limit") => Some("egraph_hit_iteration_limit"),
        ("", "egraph_hit_node_limit") => Some("egraph_hit_node_limit"),
        ("", "egraph_cpu_saturation_ns") => Some("egraph_cpu_saturation_ns"),
        ("", "egraph_gpu_apply_ns") => Some("egraph_gpu_apply_ns"),
        ("", "egraph_gpu_recall_parity") => Some("egraph_gpu_recall_parity"),
        ("", "egraph_gpu_class_id_determinism") => Some("egraph_gpu_class_id_determinism"),
        ("", "rust_frontend_public_pipeline_speedup_x1000") => {
            Some("rust_frontend_public_pipeline_speedup_x1000")
        }
        ("", "rust_frontend_dispatch_speedup_x1000") => {
            Some("rust_frontend_dispatch_speedup_x1000")
        }
        ("", "rust_frontend_lanes") => Some("rust_frontend_lanes"),
        ("", "rust_frontend_gpu_lexer_speedup_x1000") => {
            Some("rust_frontend_gpu_lexer_speedup_x1000")
        }
        ("", "rust_frontend_gpu_lexer_tokens") => Some("rust_frontend_gpu_lexer_tokens"),
        ("", "rust_frontend_gpu_lexer_source_bytes") => {
            Some("rust_frontend_gpu_lexer_source_bytes")
        }
        ("", "rust_frontend_gpu_lexer_batch_speedup_x1000") => {
            Some("rust_frontend_gpu_lexer_batch_speedup_x1000")
        }
        ("", "rust_frontend_gpu_lexer_batch_tokens") => {
            Some("rust_frontend_gpu_lexer_batch_tokens")
        }
        ("", "rust_frontend_gpu_lexer_batch_source_bytes") => {
            Some("rust_frontend_gpu_lexer_batch_source_bytes")
        }
        ("", "rust_frontend_gpu_lexer_batch_sources") => {
            Some("rust_frontend_gpu_lexer_batch_sources")
        }
        ("", "rust_frontend_gpu_lexer_batch_token_stride") => {
            Some("rust_frontend_gpu_lexer_batch_token_stride")
        }
        // SyntheticCountWorkload metric_name values (families.rs) — absent entries cause
        // collect_custom_metrics to silently discard the record-count each iteration.
        ("", "condition_records") => Some("condition_records"),
        ("", "quantified_records") => Some("quantified_records"),
        ("", "scatter_records") => Some("scatter_records"),
        ("", "aggregation_records") => Some("aggregation_records"),
        ("", "entropy_records") => Some("entropy_records"),
        ("", "alias_records") => Some("alias_records"),
        ("", "ifds_records") => Some("ifds_records"),
        ("", "ast_nodes") => Some("ast_nodes"),
        ("", "queued_records") => Some("queued_records"),
        ("", "rewrite_records") => Some("rewrite_records"),
        // CallgraphReachabilityStep case emits this custom point (release_workloads.rs).
        ("", "callgraph_witness_digest") => Some("callgraph_witness_digest"),
        ("baseline_", "flop_count") => Some("baseline_flop_count"),
        ("baseline_", "megakernel_items_processed") => Some("baseline_megakernel_items_processed"),
        _ => None,
    }
}

pub(super) fn derived_metric_key(prefix: &'static str, name: &str) -> Option<&'static str> {
    match (prefix, name) {
        ("", "wall_gb_s_x1000") => Some("wall_gb_s_x1000"),
        ("", "device_gb_s_x1000") => Some("device_gb_s_x1000"),
        ("", "gflops_x1000") => Some("gflops_x1000"),
        ("", "roofline_mem_pct_x1000") => Some("roofline_mem_pct_x1000"),
        ("baseline_", "wall_gb_s_x1000") => Some("baseline_wall_gb_s_x1000"),
        ("baseline_", "device_gb_s_x1000") => Some("baseline_device_gb_s_x1000"),
        ("baseline_", "gflops_x1000") => Some("baseline_gflops_x1000"),
        ("baseline_", "roofline_mem_pct_x1000") => Some("baseline_roofline_mem_pct_x1000"),
        _ => None,
    }
}

pub(super) fn metric_key(prefix: &'static str, name: &'static str) -> Option<&'static str> {
    match (prefix, name) {
        ("", "wall_ns") => Some("wall_ns"),
        ("", "cpu_ns") => Some("cpu_ns"),
        ("", "compile_ns") => Some("compile_ns"),
        ("", "validate_ns") => Some("validate_ns"),
        ("", "optimize_ns") => Some("optimize_ns"),
        ("", "lower_ns") => Some("lower_ns"),
        ("", "cache_lookup_ns") => Some("cache_lookup_ns"),
        ("", "cache_hit") => Some("cache_hit"),
        ("", "upload_ns") => Some("upload_ns"),
        ("", "dispatch_ns") => Some("dispatch_ns"),
        ("", "kernel_queue_submit_ns") => Some("kernel_queue_submit_ns"),
        ("", "kernel_execute_ns") => Some("kernel_execute_ns"),
        ("", "device_sync_ns") => Some("device_sync_ns"),
        ("", "readback_ns") => Some("readback_ns"),
        ("", "verify_ns") => Some("verify_ns"),
        ("", "alloc_count") => Some("alloc_count"),
        ("", "alloc_bytes") => Some("alloc_bytes"),
        ("", "peak_rss_bytes") => Some("peak_rss_bytes"),
        ("", "input_bytes") => Some("input_bytes"),
        ("", "output_bytes") => Some("output_bytes"),
        ("", "bytes_touched") => Some("bytes_touched"),
        ("", "bytes_read") => Some("bytes_read"),
        ("", "bytes_written") => Some("bytes_written"),
        ("", "atomic_op_count") => Some("atomic_op_count"),
        ("", "wire_bytes") => Some("wire_bytes"),
        ("baseline_", "wall_ns") => Some("baseline_wall_ns"),
        ("baseline_", "cpu_ns") => Some("baseline_cpu_ns"),
        ("baseline_", "compile_ns") => Some("baseline_compile_ns"),
        ("baseline_", "validate_ns") => Some("baseline_validate_ns"),
        ("baseline_", "optimize_ns") => Some("baseline_optimize_ns"),
        ("baseline_", "lower_ns") => Some("baseline_lower_ns"),
        ("baseline_", "cache_lookup_ns") => Some("baseline_cache_lookup_ns"),
        ("baseline_", "cache_hit") => Some("baseline_cache_hit"),
        ("baseline_", "upload_ns") => Some("baseline_upload_ns"),
        ("baseline_", "dispatch_ns") => Some("baseline_dispatch_ns"),
        ("baseline_", "kernel_queue_submit_ns") => Some("baseline_kernel_queue_submit_ns"),
        ("baseline_", "kernel_execute_ns") => Some("baseline_kernel_execute_ns"),
        ("baseline_", "device_sync_ns") => Some("baseline_device_sync_ns"),
        ("baseline_", "readback_ns") => Some("baseline_readback_ns"),
        ("baseline_", "verify_ns") => Some("baseline_verify_ns"),
        ("baseline_", "alloc_count") => Some("baseline_alloc_count"),
        ("baseline_", "alloc_bytes") => Some("baseline_alloc_bytes"),
        ("baseline_", "peak_rss_bytes") => Some("baseline_peak_rss_bytes"),
        ("baseline_", "input_bytes") => Some("baseline_input_bytes"),
        ("baseline_", "output_bytes") => Some("baseline_output_bytes"),
        ("baseline_", "bytes_touched") => Some("baseline_bytes_touched"),
        ("baseline_", "bytes_read") => Some("baseline_bytes_read"),
        ("baseline_", "bytes_written") => Some("baseline_bytes_written"),
        ("baseline_", "atomic_op_count") => Some("baseline_atomic_op_count"),
        ("baseline_", "wire_bytes") => Some("baseline_wire_bytes"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::custom_metric_key;

    #[test]
    fn custom_metric_key_keeps_cuda_ptx_source_cache_visible() {
        // Regression: these were absent, so `collect_custom_metrics` dropped the
        // codegen corpus's source-cache stats and the artifact reported 0 entries,
        // failing the CUDA PTX pattern evidence gate. They must stay collectable.
        for name in [
            "cuda_ptx_source_cache_entries",
            "cuda_ptx_source_cache_hits",
            "cuda_ptx_source_cache_misses",
        ] {
            assert_eq!(custom_metric_key("", name), Some(name));
        }
    }

    #[test]
    fn custom_metric_key_keeps_queue_closure_telemetry_visible() {
        for name in [
            "dataflow_ifds_closure_queue_capacity",
            "dataflow_ifds_closure_queue_capacity_reduction_x1000",
            "dataflow_ifds_closure_seed_queue_len",
            "dataflow_ifds_closure_dispatch_count",
            "dataflow_ifds_closure_total_queue_pops",
            "dataflow_ifds_closure_max_wave_queue_len",
            "dataflow_ifds_closure_queue_delta",
            "dataflow_ifds_closure_row_strided_delta",
            "dataflow_ifds_closure_seed_scan_elided",
            "dataflow_ifds_closure_wave_profiled",
            "dataflow_ifds_closure_fixed_delta_source_slots",
            "dataflow_ifds_closure_profiled_delta_source_slots",
            "dataflow_ifds_closure_elided_delta_source_slots",
            "dataflow_ifds_closure_fixed_delta_lanes",
            "dataflow_ifds_closure_profiled_delta_lanes",
            "dataflow_ifds_closure_elided_delta_lanes",
            "dataflow_ifds_closure_delta_lane_elision_x1000",
            "dataflow_ifds_closure_launch_delta_lanes",
            "dataflow_ifds_closure_launch_elided_delta_lanes",
            "dataflow_ifds_closure_launch_lane_elision_x1000",
        ] {
            assert_eq!(custom_metric_key("", name), Some(name));
        }
    }

    #[test]
    fn custom_metric_key_keeps_ifds_queue_split_telemetry_visible() {
        for name in [
            "dataflow_ifds_queue_high_degree_capacity",
            "dataflow_ifds_queue_split_high_degree",
            "dataflow_ifds_queue_high_degree_threshold",
            "dataflow_ifds_queue_traverse_logical_lanes",
            "dataflow_ifds_queue_traverse_lane_reduction_x1000",
            "dataflow_ifds_queue_fused_frontier_clear",
            "dataflow_ifds_queue_reset_grid_lanes",
        ] {
            assert_eq!(custom_metric_key("", name), Some(name));
        }
    }

    #[test]
    fn custom_metric_key_keeps_weir_release_shape_metrics_visible() {
        for name in ["weir_nodes", "weir_bitset_words"] {
            assert_eq!(custom_metric_key("", name), Some(name));
        }
    }

    #[test]
    fn custom_metric_key_keeps_benchmark_evidence_schema_metrics_visible() {
        for name in ["cpu_digest", "gpu_digest", "active_time_ns", "transfer_bytes"] {
            assert_eq!(custom_metric_key("", name), Some(name));
        }
    }

    #[test]
    fn custom_metric_key_keeps_scan_count_prefilter_telemetry_visible() {
        for name in [
            "scan_ac_irregular_bounded_ranges_prefilter",
            "scan_ac_irregular_bounded_ranges_prefilter_skipped_lanes_x1000",
            "scan_ac_irregular_bounded_ranges_suffix3_prefilter",
            "scan_ac_irregular_count_only",
            "scan_ac_irregular_count_prefilter",
            "scan_ac_irregular_count_readback_bytes",
            "scan_ac_irregular_candidate_end_bytes",
            "scan_ac_irregular_candidate_end_lanes",
            "scan_ac_irregular_candidate_suffix2_lanes",
            "scan_ac_irregular_candidate_suffix3_lanes",
            "scan_ac_irregular_count_prefilter_skipped_lanes_x1000",
            "scan_ac_irregular_suffix2_extra_skipped_lanes_x1000",
            "scan_ac_irregular_suffix3_extra_skipped_lanes_x1000",
        ] {
            assert_eq!(custom_metric_key("", name), Some(name));
        }
    }

    /// Regression: all ten SyntheticCountWorkload::metric_name values (families.rs) and
    /// callgraph_witness_digest (release_workloads.rs) were absent from the match table.
    /// collect_custom_metrics routes every custom point through custom_metric_key and drops
    /// the value when it returns None, so each iteration's record count was silently
    /// discarded and the metric never appeared in the JSON artifact.
    #[test]
    fn synthetic_count_workload_metric_names_are_collectable() {
        // These map 1:1 to SyntheticCountWorkload::metric_name in families.rs.
        let names = [
            "condition_records",
            "quantified_records",
            "scatter_records",
            "aggregation_records",
            "entropy_records",
            "alias_records",
            "ifds_records",
            "ast_nodes",
            "queued_records",
            "rewrite_records",
            // CallgraphReachabilityStep emits this in release_workloads.rs.
            "callgraph_witness_digest",
        ];
        for name in names {
            assert_eq!(
                custom_metric_key("", name),
                Some(name),
                "Fix: custom_metric_key must return Some(\"{name}\") so collect_custom_metrics \
                 does not silently discard the SyntheticCountWorkload record count."
            );
        }
    }
}
