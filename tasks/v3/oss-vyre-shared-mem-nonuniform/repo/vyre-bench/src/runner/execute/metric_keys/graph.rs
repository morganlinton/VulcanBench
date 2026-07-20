pub(super) fn custom_graph_metric_key(prefix: &'static str, name: &str) -> Option<&'static str> {
    match (prefix, name) {
        ("", "graph_csr_nodes") => Some("graph_csr_nodes"),
        ("", "graph_csr_edges") => Some("graph_csr_edges"),
        ("", "graph_csr_frontier_words") => Some("graph_csr_frontier_words"),
        ("", "graph_csr_active_sources") => Some("graph_csr_active_sources"),
        ("", "graph_csr_allowed_edges") => Some("graph_csr_allowed_edges"),
        ("", "graph_csr_output_words_set") => Some("graph_csr_output_words_set"),
        ("", "graph_csr_max_degree") => Some("graph_csr_max_degree"),
        ("", "graph_csr_high_degree_sources") => Some("graph_csr_high_degree_sources"),
        ("", "graph_csr_resident_buffers") => Some("graph_csr_resident_buffers"),
        ("", "graph_csr_workgroup_size_x") => Some("graph_csr_workgroup_size_x"),
        ("", "graph_csr_skewed_speedup_x1000") => Some("graph_csr_skewed_speedup_x1000"),
        ("", "graph_csr_queue_materializer") => Some("graph_csr_queue_materializer"),
        ("", "graph_csr_queue_capacity") => Some("graph_csr_queue_capacity"),
        ("", "graph_csr_queue_row_strided") => Some("graph_csr_queue_row_strided"),
        ("", "graph_csr_queue_split_high_degree") => Some("graph_csr_queue_split_high_degree"),
        ("", "graph_csr_queue_high_degree_capacity") => {
            Some("graph_csr_queue_high_degree_capacity")
        }
        ("", "graph_csr_queue_high_degree_threshold") => {
            Some("graph_csr_queue_high_degree_threshold")
        }
        ("", "graph_csr_queue_fused_frontier_clear") => {
            Some("graph_csr_queue_fused_frontier_clear")
        }
        ("", "graph_csr_queue_reset_grid_lanes") => Some("graph_csr_queue_reset_grid_lanes"),
        ("", "graph_csr_queue_traverse_logical_lanes") => {
            Some("graph_csr_queue_traverse_logical_lanes")
        }
        ("", "graph_csr_queue_traverse_lane_reduction_x1000") => {
            Some("graph_csr_queue_traverse_lane_reduction_x1000")
        }
        ("", "graph_csr_queue_lane_reduction_x1000") => {
            Some("graph_csr_queue_lane_reduction_x1000")
        }
        ("", "graph_csr_queue_closure_speedup_x1000") => {
            Some("graph_csr_queue_closure_speedup_x1000")
        }
        ("", "graph_csr_queue_closure_capacity") => Some("graph_csr_queue_closure_capacity"),
        ("", "graph_csr_queue_closure_capacity_reduction_x1000") => {
            Some("graph_csr_queue_closure_capacity_reduction_x1000")
        }
        ("", "graph_csr_queue_closure_seed_len") => Some("graph_csr_queue_closure_seed_len"),
        ("", "graph_csr_queue_closure_iterations") => Some("graph_csr_queue_closure_iterations"),
        ("", "graph_csr_queue_closure_changed") => Some("graph_csr_queue_closure_changed"),
        ("", "graph_csr_queue_closure_max_iters") => Some("graph_csr_queue_closure_max_iters"),
        ("", "graph_csr_queue_closure_dispatch_count") => {
            Some("graph_csr_queue_closure_dispatch_count")
        }
        ("", "graph_csr_queue_closure_total_queue_pops") => {
            Some("graph_csr_queue_closure_total_queue_pops")
        }
        ("", "graph_csr_queue_closure_max_wave_len") => {
            Some("graph_csr_queue_closure_max_wave_len")
        }
        ("", "graph_csr_queue_closure_delta") => Some("graph_csr_queue_closure_delta"),
        ("", "graph_csr_queue_closure_row_strided_delta") => {
            Some("graph_csr_queue_closure_row_strided_delta")
        }
        ("", "graph_csr_queue_closure_wave_profiled") => {
            Some("graph_csr_queue_closure_wave_profiled")
        }
        ("", "graph_csr_queue_closure_fixed_delta_source_slots") => {
            Some("graph_csr_queue_closure_fixed_delta_source_slots")
        }
        ("", "graph_csr_queue_closure_profiled_delta_source_slots") => {
            Some("graph_csr_queue_closure_profiled_delta_source_slots")
        }
        ("", "graph_csr_queue_closure_elided_delta_source_slots") => {
            Some("graph_csr_queue_closure_elided_delta_source_slots")
        }
        ("", "graph_csr_queue_closure_fixed_delta_lanes") => {
            Some("graph_csr_queue_closure_fixed_delta_lanes")
        }
        ("", "graph_csr_queue_closure_profiled_delta_lanes") => {
            Some("graph_csr_queue_closure_profiled_delta_lanes")
        }
        ("", "graph_csr_queue_closure_elided_delta_lanes") => {
            Some("graph_csr_queue_closure_elided_delta_lanes")
        }
        ("", "graph_csr_queue_closure_delta_lane_elision_x1000") => {
            Some("graph_csr_queue_closure_delta_lane_elision_x1000")
        }
        ("", "graph_csr_queue_closure_launch_delta_lanes") => {
            Some("graph_csr_queue_closure_launch_delta_lanes")
        }
        ("", "graph_csr_queue_closure_launch_elided_delta_lanes") => {
            Some("graph_csr_queue_closure_launch_elided_delta_lanes")
        }
        ("", "graph_csr_queue_closure_launch_lane_elision_x1000") => {
            Some("graph_csr_queue_closure_launch_lane_elision_x1000")
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::custom_graph_metric_key;

    #[test]
    fn custom_graph_metric_key_keeps_queue_materialization_visible() {
        for name in [
            "graph_csr_nodes",
            "graph_csr_edges",
            "graph_csr_active_sources",
            "graph_csr_queue_materializer",
            "graph_csr_queue_capacity",
            "graph_csr_queue_row_strided",
            "graph_csr_queue_split_high_degree",
            "graph_csr_queue_high_degree_capacity",
            "graph_csr_queue_high_degree_threshold",
            "graph_csr_queue_fused_frontier_clear",
            "graph_csr_queue_reset_grid_lanes",
            "graph_csr_queue_traverse_logical_lanes",
            "graph_csr_queue_traverse_lane_reduction_x1000",
            "graph_csr_queue_lane_reduction_x1000",
            "graph_csr_queue_closure_capacity",
            "graph_csr_queue_closure_capacity_reduction_x1000",
            "graph_csr_queue_closure_seed_len",
            "graph_csr_queue_closure_iterations",
            "graph_csr_queue_closure_dispatch_count",
            "graph_csr_queue_closure_total_queue_pops",
            "graph_csr_queue_closure_max_wave_len",
            "graph_csr_queue_closure_row_strided_delta",
            "graph_csr_queue_closure_wave_profiled",
            "graph_csr_queue_closure_fixed_delta_source_slots",
            "graph_csr_queue_closure_profiled_delta_source_slots",
            "graph_csr_queue_closure_elided_delta_source_slots",
            "graph_csr_queue_closure_fixed_delta_lanes",
            "graph_csr_queue_closure_profiled_delta_lanes",
            "graph_csr_queue_closure_elided_delta_lanes",
            "graph_csr_queue_closure_delta_lane_elision_x1000",
            "graph_csr_queue_closure_launch_delta_lanes",
            "graph_csr_queue_closure_launch_elided_delta_lanes",
            "graph_csr_queue_closure_launch_lane_elision_x1000",
        ] {
            assert_eq!(custom_graph_metric_key("", name), Some(name));
        }
    }
}
