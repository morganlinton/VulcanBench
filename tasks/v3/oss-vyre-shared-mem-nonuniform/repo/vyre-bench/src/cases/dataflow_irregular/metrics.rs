use crate::api::metric::MetricPoint;

use super::IfdsSkewedStats;

pub(super) fn ifds_skewed_metric_points(
    stats: IfdsSkewedStats,
    baseline_wall_ns: u64,
    wall_ns: u64,
    resident_used: bool,
    workgroup_size_x: u32,
) -> Vec<MetricPoint> {
    let mut metrics = ifds_skewed_baseline_metric_points(stats);
    metrics.push(MetricPoint {
        name: "dataflow_ifds_skewed_resident_buffers".to_string(),
        value: u64::from(resident_used),
    });
    metrics.push(MetricPoint {
        name: "dataflow_ifds_skewed_workgroup_size_x".to_string(),
        value: u64::from(workgroup_size_x),
    });
    if wall_ns > 0 {
        metrics.push(MetricPoint {
            name: "dataflow_ifds_skewed_speedup_x1000".to_string(),
            value: (u128::from(baseline_wall_ns) * 1000 / u128::from(wall_ns))
                .min(u128::from(u64::MAX)) as u64,
        });
    }
    metrics
}

pub(super) fn ifds_skewed_baseline_metric_points(stats: IfdsSkewedStats) -> Vec<MetricPoint> {
    vec![
        MetricPoint {
            name: "dataflow_ifds_skewed_nodes".to_string(),
            value: u64::from(stats.nodes),
        },
        MetricPoint {
            name: "dataflow_ifds_skewed_edges".to_string(),
            value: u64::from(stats.edges),
        },
        MetricPoint {
            name: "dataflow_ifds_skewed_frontier_words".to_string(),
            value: u64::from(stats.frontier_words),
        },
        MetricPoint {
            name: "dataflow_ifds_skewed_active_sources".to_string(),
            value: stats.active_sources,
        },
        MetricPoint {
            name: "dataflow_ifds_skewed_allowed_edges".to_string(),
            value: stats.allowed_edges_from_active,
        },
        MetricPoint {
            name: "dataflow_ifds_skewed_filtered_edges".to_string(),
            value: stats.filtered_edges_from_active,
        },
        MetricPoint {
            name: "dataflow_ifds_skewed_output_words_set".to_string(),
            value: stats.output_words_set,
        },
        MetricPoint {
            name: "dataflow_ifds_skewed_max_degree".to_string(),
            value: u64::from(stats.max_degree),
        },
        MetricPoint {
            name: "dataflow_ifds_skewed_high_degree_sources".to_string(),
            value: stats.high_degree_sources,
        },
    ]
}

pub(super) fn ifds_queue_metric_points(
    stats: IfdsSkewedStats,
    queue_capacity: u32,
    high_degree_queue_capacity: u32,
    traverse_logical_lanes: u64,
    baseline_wall_ns: u64,
    wall_ns: u64,
    resident_used: bool,
    workgroup_size_x: u32,
    parallel_materializer: bool,
    row_strided_traverse: bool,
    split_high_degree_traverse: bool,
    high_degree_threshold: u32,
    fused_frontier_clear: bool,
    reset_grid_lanes: u32,
) -> Vec<MetricPoint> {
    let mut metrics = ifds_queue_baseline_metric_points(stats, queue_capacity);
    metrics.push(MetricPoint {
        name: "dataflow_ifds_queue_resident_buffers".to_string(),
        value: u64::from(resident_used),
    });
    metrics.push(MetricPoint {
        name: "dataflow_ifds_queue_workgroup_size_x".to_string(),
        value: u64::from(workgroup_size_x),
    });
    metrics.push(MetricPoint {
        name: "dataflow_ifds_queue_parallel_materializer".to_string(),
        value: u64::from(parallel_materializer),
    });
    metrics.push(MetricPoint {
        name: "dataflow_ifds_queue_row_strided_traverse".to_string(),
        value: u64::from(row_strided_traverse),
    });
    metrics.push(MetricPoint {
        name: "dataflow_ifds_queue_split_high_degree".to_string(),
        value: u64::from(split_high_degree_traverse),
    });
    metrics.push(MetricPoint {
        name: "dataflow_ifds_queue_high_degree_threshold".to_string(),
        value: u64::from(high_degree_threshold),
    });
    metrics.push(MetricPoint {
        name: "dataflow_ifds_queue_fused_frontier_clear".to_string(),
        value: u64::from(fused_frontier_clear),
    });
    metrics.push(MetricPoint {
        name: "dataflow_ifds_queue_reset_grid_lanes".to_string(),
        value: u64::from(reset_grid_lanes),
    });
    metrics.push(MetricPoint {
        name: "dataflow_ifds_queue_high_degree_capacity".to_string(),
        value: u64::from(high_degree_queue_capacity),
    });
    metrics.push(MetricPoint {
        name: "dataflow_ifds_queue_traverse_logical_lanes".to_string(),
        value: traverse_logical_lanes,
    });
    if traverse_logical_lanes > 0 {
        metrics.push(MetricPoint {
            name: "dataflow_ifds_queue_traverse_lane_reduction_x1000".to_string(),
            value: (u128::from(stats.nodes) * 1000 / u128::from(traverse_logical_lanes))
                .min(u128::from(u64::MAX)) as u64,
        });
    }
    if wall_ns > 0 {
        metrics.push(MetricPoint {
            name: "dataflow_ifds_queue_speedup_x1000".to_string(),
            value: (u128::from(baseline_wall_ns) * 1000 / u128::from(wall_ns))
                .min(u128::from(u64::MAX)) as u64,
        });
    }
    metrics
}

pub(super) fn ifds_queue_baseline_metric_points(
    stats: IfdsSkewedStats,
    queue_capacity: u32,
) -> Vec<MetricPoint> {
    let mut metrics = ifds_skewed_baseline_metric_points(stats);
    metrics.push(MetricPoint {
        name: "dataflow_ifds_queue_capacity".to_string(),
        value: u64::from(queue_capacity),
    });
    if queue_capacity > 0 {
        metrics.push(MetricPoint {
            name: "dataflow_ifds_queue_lane_reduction_x1000".to_string(),
            value: (u128::from(stats.nodes) * 1000 / u128::from(queue_capacity))
                .min(u128::from(u64::MAX)) as u64,
        });
    }
    metrics
}

#[allow(clippy::too_many_arguments)]
pub(super) fn ifds_closure_metric_points(
    stats: IfdsSkewedStats,
    closure_iterations: u32,
    closure_changed: u32,
    baseline_wall_ns: u64,
    wall_ns: u64,
    resident_used: bool,
    resident_reset_bytes: u64,
    device_reset_sequence: bool,
    dispatch_iterations: u32,
    max_iterations: u32,
    workgroup_size_x: u32,
) -> Vec<MetricPoint> {
    let mut metrics = ifds_closure_baseline_metric_points(
        stats,
        closure_iterations,
        closure_changed,
        dispatch_iterations,
        max_iterations,
    );
    metrics.push(MetricPoint {
        name: "dataflow_ifds_closure_resident_buffers".to_string(),
        value: u64::from(resident_used),
    });
    metrics.push(MetricPoint {
        name: "dataflow_ifds_closure_resident_reset_bytes".to_string(),
        value: resident_reset_bytes,
    });
    metrics.push(MetricPoint {
        name: "dataflow_ifds_closure_device_reset_sequence".to_string(),
        value: u64::from(device_reset_sequence),
    });
    metrics.push(MetricPoint {
        name: "dataflow_ifds_closure_workgroup_size_x".to_string(),
        value: u64::from(workgroup_size_x),
    });
    if wall_ns > 0 {
        metrics.push(MetricPoint {
            name: "dataflow_ifds_closure_speedup_x1000".to_string(),
            value: (u128::from(baseline_wall_ns) * 1000 / u128::from(wall_ns))
                .min(u128::from(u64::MAX)) as u64,
        });
    }
    metrics
}

pub(super) fn ifds_closure_baseline_metric_points(
    stats: IfdsSkewedStats,
    closure_iterations: u32,
    closure_changed: u32,
    dispatch_iterations: u32,
    max_iterations: u32,
) -> Vec<MetricPoint> {
    vec![
        MetricPoint {
            name: "dataflow_ifds_closure_nodes".to_string(),
            value: u64::from(stats.nodes),
        },
        MetricPoint {
            name: "dataflow_ifds_closure_edges".to_string(),
            value: u64::from(stats.edges),
        },
        MetricPoint {
            name: "dataflow_ifds_closure_frontier_words".to_string(),
            value: u64::from(stats.frontier_words),
        },
        MetricPoint {
            name: "dataflow_ifds_closure_active_sources".to_string(),
            value: stats.active_sources,
        },
        MetricPoint {
            name: "dataflow_ifds_closure_output_words_set".to_string(),
            value: stats.output_words_set,
        },
        MetricPoint {
            name: "dataflow_ifds_closure_max_degree".to_string(),
            value: u64::from(stats.max_degree),
        },
        MetricPoint {
            name: "dataflow_ifds_closure_high_degree_sources".to_string(),
            value: stats.high_degree_sources,
        },
        MetricPoint {
            name: "dataflow_ifds_closure_iterations".to_string(),
            value: u64::from(closure_iterations),
        },
        MetricPoint {
            name: "dataflow_ifds_closure_changed".to_string(),
            value: u64::from(closure_changed),
        },
        MetricPoint {
            name: "dataflow_ifds_closure_fixpoint_iterations".to_string(),
            value: u64::from(dispatch_iterations),
        },
        MetricPoint {
            name: "dataflow_ifds_closure_max_iterations".to_string(),
            value: u64::from(max_iterations),
        },
        MetricPoint {
            name: "dataflow_ifds_closure_elided_iterations".to_string(),
            value: u64::from(max_iterations.saturating_sub(dispatch_iterations)),
        },
    ]
}
