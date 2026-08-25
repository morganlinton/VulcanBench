use crate::api::metric::MetricPoint;

use super::{
    ifds_queue_closure_delta_lanes_per_source, DataflowIfdsSkewedQueueClosurePrepared,
    QUEUE_CLOSURE_WORKGROUP_SIZE,
};
use crate::cases::dataflow_irregular::closure::CLOSURE_MAX_ITERS;
use crate::cases::dataflow_irregular::metrics::{
    ifds_closure_baseline_metric_points, ifds_closure_metric_points,
};
use crate::cases::queue_closure_profile::{
    queue_closure_launch_lanes_per_wave, QueueClosureLaneProfile,
};

pub(super) fn queue_closure_metric_points(
    prepared: &DataflowIfdsSkewedQueueClosurePrepared,
    wall_ns: u64,
    resident_used: bool,
) -> Vec<MetricPoint> {
    let mut metrics = ifds_closure_metric_points(
        prepared.stats,
        prepared.closure_iterations,
        prepared.closure_changed,
        prepared.baseline_wall_ns,
        wall_ns,
        resident_used,
        0,
        true,
        prepared.closure_iterations,
        CLOSURE_MAX_ITERS,
        QUEUE_CLOSURE_WORKGROUP_SIZE[0],
    );
    append_queue_closure_points(prepared, &mut metrics);
    metrics
}

pub(super) fn queue_closure_baseline_metric_points(
    prepared: &DataflowIfdsSkewedQueueClosurePrepared,
) -> Vec<MetricPoint> {
    let mut metrics = ifds_closure_baseline_metric_points(
        prepared.stats,
        prepared.closure_iterations,
        prepared.closure_changed,
        prepared.closure_iterations,
        CLOSURE_MAX_ITERS,
    );
    append_queue_closure_points(prepared, &mut metrics);
    metrics
}

fn append_queue_closure_points(
    prepared: &DataflowIfdsSkewedQueueClosurePrepared,
    metrics: &mut Vec<MetricPoint>,
) {
    metrics.push(MetricPoint {
        name: "dataflow_ifds_closure_queue_capacity".to_string(),
        value: u64::from(prepared.queue_capacity),
    });
    metrics.push(MetricPoint {
        name: "dataflow_ifds_closure_queue_capacity_reduction_x1000".to_string(),
        value: (u128::from(prepared.stats.nodes) * 1000 / u128::from(prepared.queue_capacity))
            .min(u128::from(u64::MAX)) as u64,
    });
    metrics.push(MetricPoint {
        name: "dataflow_ifds_closure_seed_queue_len".to_string(),
        value: u64::from(prepared.seed_queue_len),
    });
    metrics.push(MetricPoint {
        name: "dataflow_ifds_closure_dispatch_count".to_string(),
        value: u64::from(1 + prepared.closure_iterations.saturating_mul(2)),
    });
    metrics.push(MetricPoint {
        name: "dataflow_ifds_closure_total_queue_pops".to_string(),
        value: prepared.total_queue_pops,
    });
    metrics.push(MetricPoint {
        name: "dataflow_ifds_closure_max_wave_queue_len".to_string(),
        value: u64::from(prepared.max_wave_queue_len),
    });
    metrics.push(MetricPoint {
        name: "dataflow_ifds_closure_queue_delta".to_string(),
        value: 1,
    });
    metrics.push(MetricPoint {
        name: "dataflow_ifds_closure_row_strided_delta".to_string(),
        value: if prepared.row_strided_delta { 1 } else { 0 },
    });
    metrics.push(MetricPoint {
        name: "dataflow_ifds_closure_seed_scan_elided".to_string(),
        value: 1,
    });
    let launch_lanes_per_wave =
        queue_closure_launch_lanes_per_wave(prepared.delta_grid, QUEUE_CLOSURE_WORKGROUP_SIZE);
    let lane_profile = QueueClosureLaneProfile::from_wave_lengths_with_launch_lanes(
        prepared.queue_capacity,
        &prepared.wave_queue_lengths,
        ifds_queue_closure_delta_lanes_per_source(prepared.row_strided_delta),
        launch_lanes_per_wave,
    );
    metrics.push(MetricPoint {
        name: "dataflow_ifds_closure_wave_profiled".to_string(),
        value: 1,
    });
    metrics.push(MetricPoint {
        name: "dataflow_ifds_closure_fixed_delta_source_slots".to_string(),
        value: lane_profile.fixed_delta_source_slots,
    });
    metrics.push(MetricPoint {
        name: "dataflow_ifds_closure_profiled_delta_source_slots".to_string(),
        value: lane_profile.profiled_delta_source_slots,
    });
    metrics.push(MetricPoint {
        name: "dataflow_ifds_closure_elided_delta_source_slots".to_string(),
        value: lane_profile.elided_delta_source_slots,
    });
    metrics.push(MetricPoint {
        name: "dataflow_ifds_closure_fixed_delta_lanes".to_string(),
        value: lane_profile.fixed_delta_lanes,
    });
    metrics.push(MetricPoint {
        name: "dataflow_ifds_closure_profiled_delta_lanes".to_string(),
        value: lane_profile.profiled_delta_lanes,
    });
    metrics.push(MetricPoint {
        name: "dataflow_ifds_closure_elided_delta_lanes".to_string(),
        value: lane_profile.elided_delta_lanes,
    });
    metrics.push(MetricPoint {
        name: "dataflow_ifds_closure_delta_lane_elision_x1000".to_string(),
        value: lane_profile.delta_lane_elision_x1000,
    });
    metrics.push(MetricPoint {
        name: "dataflow_ifds_closure_launch_delta_lanes".to_string(),
        value: lane_profile.launched_delta_lanes,
    });
    metrics.push(MetricPoint {
        name: "dataflow_ifds_closure_launch_elided_delta_lanes".to_string(),
        value: lane_profile.launch_elided_delta_lanes,
    });
    metrics.push(MetricPoint {
        name: "dataflow_ifds_closure_launch_lane_elision_x1000".to_string(),
        value: lane_profile.launch_lane_elision_x1000,
    });
}
