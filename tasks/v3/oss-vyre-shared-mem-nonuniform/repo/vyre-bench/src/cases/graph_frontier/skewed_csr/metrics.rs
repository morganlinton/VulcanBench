use super::support::SkewedCsrStats;
use crate::api::metric::MetricPoint;

pub(super) fn skewed_csr_metric_points(
    stats: SkewedCsrStats,
    baseline_wall_ns: u64,
    wall_ns: u64,
    resident_used: bool,
    workgroup_size_x: u32,
) -> Vec<MetricPoint> {
    let mut metrics = skewed_csr_baseline_metric_points(stats);
    metrics.push(metric(
        "graph_csr_resident_buffers",
        u64::from(resident_used),
    ));
    metrics.push(metric(
        "graph_csr_workgroup_size_x",
        u64::from(workgroup_size_x),
    ));
    if wall_ns > 0 {
        metrics.push(metric(
            "graph_csr_skewed_speedup_x1000",
            (u128::from(baseline_wall_ns) * 1000 / u128::from(wall_ns)).min(u128::from(u64::MAX))
                as u64,
        ));
    }
    metrics
}

pub(super) fn skewed_csr_queue_metric_points(
    stats: SkewedCsrStats,
    queue_capacity: u32,
    high_degree_queue_capacity: u32,
    traverse_logical_lanes: u64,
    baseline_wall_ns: u64,
    wall_ns: u64,
    resident_used: bool,
    workgroup_size_x: u32,
    row_strided: bool,
    split_high_degree: bool,
    high_degree_threshold: u32,
    fused_frontier_clear: bool,
    reset_grid_lanes: u32,
) -> Vec<MetricPoint> {
    let mut metrics = skewed_csr_metric_points(
        stats,
        baseline_wall_ns,
        wall_ns,
        resident_used,
        workgroup_size_x,
    );
    metrics.push(metric("graph_csr_queue_materializer", 1));
    metrics.push(metric(
        "graph_csr_queue_capacity",
        u64::from(queue_capacity),
    ));
    metrics.push(metric(
        "graph_csr_queue_row_strided",
        u64::from(row_strided),
    ));
    metrics.push(metric(
        "graph_csr_queue_split_high_degree",
        u64::from(split_high_degree),
    ));
    metrics.push(metric(
        "graph_csr_queue_high_degree_capacity",
        u64::from(high_degree_queue_capacity),
    ));
    metrics.push(metric(
        "graph_csr_queue_high_degree_threshold",
        u64::from(high_degree_threshold),
    ));
    metrics.push(metric(
        "graph_csr_queue_fused_frontier_clear",
        u64::from(fused_frontier_clear),
    ));
    metrics.push(metric(
        "graph_csr_queue_reset_grid_lanes",
        u64::from(reset_grid_lanes),
    ));
    metrics.push(metric(
        "graph_csr_queue_traverse_logical_lanes",
        traverse_logical_lanes,
    ));
    if queue_capacity > 0 {
        metrics.push(metric(
            "graph_csr_queue_lane_reduction_x1000",
            (u128::from(stats.node_count) * 1000 / u128::from(queue_capacity))
                .min(u128::from(u64::MAX)) as u64,
        ));
    }
    if traverse_logical_lanes > 0 {
        metrics.push(metric(
            "graph_csr_queue_traverse_lane_reduction_x1000",
            (u128::from(stats.node_count) * 1000 / u128::from(traverse_logical_lanes))
                .min(u128::from(u64::MAX)) as u64,
        ));
    }
    metrics
}

pub(super) fn skewed_csr_baseline_metric_points(stats: SkewedCsrStats) -> Vec<MetricPoint> {
    vec![
        metric("graph_csr_nodes", u64::from(stats.node_count)),
        metric("graph_csr_edges", u64::from(stats.edge_count)),
        metric("graph_csr_frontier_words", u64::from(stats.frontier_words)),
        metric("graph_csr_active_sources", stats.active_sources),
        metric("graph_csr_allowed_edges", stats.allowed_edges_from_active),
        metric("graph_csr_output_words_set", stats.output_words_set),
        metric("graph_csr_max_degree", u64::from(stats.max_degree)),
        metric("graph_csr_high_degree_sources", stats.high_degree_sources),
    ]
}

fn metric(name: &str, value: u64) -> MetricPoint {
    MetricPoint {
        name: name.to_string(),
        value,
    }
}
