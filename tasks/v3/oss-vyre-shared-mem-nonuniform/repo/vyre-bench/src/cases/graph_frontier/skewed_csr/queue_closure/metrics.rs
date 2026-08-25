use crate::api::metric::MetricPoint;

use super::{
    graph_queue_closure_delta_lanes_per_source, GraphCsrSkewedQueueClosurePrepared,
    GRAPH_QUEUE_CLOSURE_MAX_ITERS, GRAPH_QUEUE_CLOSURE_WORKGROUP_SIZE,
};
use crate::cases::graph_frontier::skewed_csr::metrics::skewed_csr_baseline_metric_points;
use crate::cases::queue_closure_profile::{
    queue_closure_launch_lanes_per_wave, QueueClosureLaneProfile,
};

pub(super) fn queue_closure_metric_points(
    prepared: &GraphCsrSkewedQueueClosurePrepared,
    wall_ns: u64,
    resident_used: bool,
) -> Vec<MetricPoint> {
    let mut metrics = skewed_csr_baseline_metric_points(prepared.stats);
    metrics.push(metric(
        "graph_csr_resident_buffers",
        u64::from(resident_used),
    ));
    metrics.push(metric(
        "graph_csr_workgroup_size_x",
        u64::from(GRAPH_QUEUE_CLOSURE_WORKGROUP_SIZE[0]),
    ));
    if wall_ns > 0 {
        metrics.push(metric(
            "graph_csr_queue_closure_speedup_x1000",
            (u128::from(prepared.baseline_wall_ns) * 1000 / u128::from(wall_ns))
                .min(u128::from(u64::MAX)) as u64,
        ));
    }
    append_queue_closure_points(prepared, &mut metrics);
    metrics
}

pub(super) fn queue_closure_baseline_metric_points(
    prepared: &GraphCsrSkewedQueueClosurePrepared,
) -> Vec<MetricPoint> {
    let mut metrics = skewed_csr_baseline_metric_points(prepared.stats);
    append_queue_closure_points(prepared, &mut metrics);
    metrics
}

fn append_queue_closure_points(
    prepared: &GraphCsrSkewedQueueClosurePrepared,
    metrics: &mut Vec<MetricPoint>,
) {
    metrics.push(metric(
        "graph_csr_queue_closure_capacity",
        u64::from(prepared.queue_capacity),
    ));
    metrics.push(metric(
        "graph_csr_queue_closure_capacity_reduction_x1000",
        (u128::from(prepared.stats.node_count) * 1000 / u128::from(prepared.queue_capacity))
            .min(u128::from(u64::MAX)) as u64,
    ));
    metrics.push(metric(
        "graph_csr_queue_closure_seed_len",
        u64::from(prepared.seed_queue_len),
    ));
    metrics.push(metric(
        "graph_csr_queue_closure_iterations",
        u64::from(prepared.closure_iterations),
    ));
    metrics.push(metric(
        "graph_csr_queue_closure_changed",
        u64::from(prepared.closure_changed),
    ));
    metrics.push(metric(
        "graph_csr_queue_closure_max_iters",
        u64::from(GRAPH_QUEUE_CLOSURE_MAX_ITERS),
    ));
    metrics.push(metric(
        "graph_csr_queue_closure_dispatch_count",
        u64::from(1 + prepared.closure_iterations.saturating_mul(2)),
    ));
    metrics.push(metric(
        "graph_csr_queue_closure_total_queue_pops",
        prepared.total_queue_pops,
    ));
    metrics.push(metric(
        "graph_csr_queue_closure_max_wave_len",
        u64::from(prepared.max_wave_queue_len),
    ));
    metrics.push(metric("graph_csr_queue_closure_delta", 1));
    metrics.push(metric(
        "graph_csr_queue_closure_row_strided_delta",
        u64::from(prepared.row_strided_delta),
    ));
    let launch_lanes_per_wave = queue_closure_launch_lanes_per_wave(
        prepared.delta_grid,
        GRAPH_QUEUE_CLOSURE_WORKGROUP_SIZE,
    );
    let lane_profile = QueueClosureLaneProfile::from_wave_lengths_with_launch_lanes(
        prepared.queue_capacity,
        &prepared.wave_queue_lengths,
        graph_queue_closure_delta_lanes_per_source(prepared.row_strided_delta),
        launch_lanes_per_wave,
    );
    metrics.push(metric("graph_csr_queue_closure_wave_profiled", 1));
    metrics.push(metric(
        "graph_csr_queue_closure_fixed_delta_source_slots",
        lane_profile.fixed_delta_source_slots,
    ));
    metrics.push(metric(
        "graph_csr_queue_closure_profiled_delta_source_slots",
        lane_profile.profiled_delta_source_slots,
    ));
    metrics.push(metric(
        "graph_csr_queue_closure_elided_delta_source_slots",
        lane_profile.elided_delta_source_slots,
    ));
    metrics.push(metric(
        "graph_csr_queue_closure_fixed_delta_lanes",
        lane_profile.fixed_delta_lanes,
    ));
    metrics.push(metric(
        "graph_csr_queue_closure_profiled_delta_lanes",
        lane_profile.profiled_delta_lanes,
    ));
    metrics.push(metric(
        "graph_csr_queue_closure_elided_delta_lanes",
        lane_profile.elided_delta_lanes,
    ));
    metrics.push(metric(
        "graph_csr_queue_closure_delta_lane_elision_x1000",
        lane_profile.delta_lane_elision_x1000,
    ));
    metrics.push(metric(
        "graph_csr_queue_closure_launch_delta_lanes",
        lane_profile.launched_delta_lanes,
    ));
    metrics.push(metric(
        "graph_csr_queue_closure_launch_elided_delta_lanes",
        lane_profile.launch_elided_delta_lanes,
    ));
    metrics.push(metric(
        "graph_csr_queue_closure_launch_lane_elision_x1000",
        lane_profile.launch_lane_elision_x1000,
    ));
}

fn metric(name: &str, value: u64) -> MetricPoint {
    MetricPoint {
        name: name.to_string(),
        value,
    }
}
