use crate::api::metric::MetricPoint;

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct ScanAcStats {
    pub(super) haystack_bytes: u32,
    pub(super) packed_haystack_words: u32,
    pub(super) patterns: u32,
    pub(super) dfa_states: u32,
    pub(super) max_pattern_len: u32,
    pub(super) output_records: u32,
    pub(super) expected_matches: u32,
    pub(super) max_matches: u32,
    pub(super) planted_matches: u32,
    pub(super) candidate_end_bytes: u32,
    pub(super) candidate_end_lanes: u32,
    pub(super) candidate_suffix2_lanes: u32,
    pub(super) candidate_suffix3_lanes: u32,
}

pub(super) fn scan_ac_metric_points(
    stats: ScanAcStats,
    baseline_wall_ns: u64,
    wall_ns: u64,
    resident_used: bool,
    resident_reset_bytes: u64,
    device_reset_sequence: bool,
    workgroup_size_x: u32,
) -> Vec<MetricPoint> {
    let mut metrics = scan_ac_baseline_metric_points(stats);
    metrics.push(metric(
        "scan_ac_irregular_resident_buffers",
        u64::from(resident_used),
    ));
    metrics.push(metric(
        "scan_ac_irregular_resident_reset_bytes",
        resident_reset_bytes,
    ));
    metrics.push(metric(
        "scan_ac_irregular_device_reset_sequence",
        u64::from(device_reset_sequence),
    ));
    metrics.push(metric(
        "scan_ac_irregular_workgroup_size_x",
        u64::from(workgroup_size_x),
    ));
    if wall_ns > 0 {
        metrics.push(metric(
            "scan_ac_irregular_speedup_x1000",
            (u128::from(baseline_wall_ns) * 1000 / u128::from(wall_ns)).min(u128::from(u64::MAX))
                as u64,
        ));
    }
    metrics
}

pub(super) fn scan_ac_bounded_ranges_metric_points(
    stats: ScanAcStats,
    baseline_wall_ns: u64,
    wall_ns: u64,
    resident_used: bool,
    resident_reset_bytes: u64,
    device_reset_sequence: bool,
    workgroup_size_x: u32,
) -> Vec<MetricPoint> {
    let mut metrics = scan_ac_metric_points(
        stats,
        baseline_wall_ns,
        wall_ns,
        resident_used,
        resident_reset_bytes,
        device_reset_sequence,
        workgroup_size_x,
    );
    metrics.push(metric("scan_ac_irregular_bounded_ranges_prefilter", 1));
    metrics.push(metric(
        "scan_ac_irregular_bounded_ranges_suffix3_prefilter",
        1,
    ));
    metrics.push(metric(
        "scan_ac_irregular_candidate_end_bytes",
        u64::from(stats.candidate_end_bytes),
    ));
    metrics.push(metric(
        "scan_ac_irregular_candidate_end_lanes",
        u64::from(stats.candidate_end_lanes),
    ));
    metrics.push(metric(
        "scan_ac_irregular_candidate_suffix2_lanes",
        u64::from(stats.candidate_suffix2_lanes),
    ));
    metrics.push(metric(
        "scan_ac_irregular_candidate_suffix3_lanes",
        u64::from(stats.candidate_suffix3_lanes),
    ));
    if stats.haystack_bytes > 0 {
        metrics.push(metric(
            "scan_ac_irregular_bounded_ranges_prefilter_skipped_lanes_x1000",
            (u128::from(
                stats
                    .haystack_bytes
                    .saturating_sub(stats.candidate_suffix3_lanes),
            ) * 1000
                / u128::from(stats.haystack_bytes)) as u64,
        ));
        metrics.push(metric(
            "scan_ac_irregular_suffix2_extra_skipped_lanes_x1000",
            (u128::from(
                stats
                    .candidate_end_lanes
                    .saturating_sub(stats.candidate_suffix2_lanes),
            ) * 1000
                / u128::from(stats.haystack_bytes)) as u64,
        ));
        metrics.push(metric(
            "scan_ac_irregular_suffix3_extra_skipped_lanes_x1000",
            (u128::from(
                stats
                    .candidate_suffix2_lanes
                    .saturating_sub(stats.candidate_suffix3_lanes),
            ) * 1000
                / u128::from(stats.haystack_bytes)) as u64,
        ));
    }
    metrics
}

pub(super) fn scan_ac_count_metric_points(
    stats: ScanAcStats,
    baseline_wall_ns: u64,
    wall_ns: u64,
    resident_used: bool,
    device_reset_sequence: bool,
    workgroup_size_x: u32,
) -> Vec<MetricPoint> {
    let mut metrics = scan_ac_metric_points(
        stats,
        baseline_wall_ns,
        wall_ns,
        resident_used,
        0,
        device_reset_sequence,
        workgroup_size_x,
    );
    metrics.push(metric("scan_ac_irregular_count_only", 1));
    metrics.push(metric("scan_ac_irregular_count_prefilter", 1));
    metrics.push(metric("scan_ac_irregular_count_readback_bytes", 4));
    metrics.push(metric(
        "scan_ac_irregular_candidate_end_bytes",
        u64::from(stats.candidate_end_bytes),
    ));
    metrics.push(metric(
        "scan_ac_irregular_candidate_end_lanes",
        u64::from(stats.candidate_end_lanes),
    ));
    metrics.push(metric(
        "scan_ac_irregular_candidate_suffix2_lanes",
        u64::from(stats.candidate_suffix2_lanes),
    ));
    metrics.push(metric(
        "scan_ac_irregular_candidate_suffix3_lanes",
        u64::from(stats.candidate_suffix3_lanes),
    ));
    if stats.haystack_bytes > 0 {
        metrics.push(metric(
            "scan_ac_irregular_count_prefilter_skipped_lanes_x1000",
            (u128::from(
                stats
                    .haystack_bytes
                    .saturating_sub(stats.candidate_suffix3_lanes),
            ) * 1000
                / u128::from(stats.haystack_bytes)) as u64,
        ));
        metrics.push(metric(
            "scan_ac_irregular_suffix2_extra_skipped_lanes_x1000",
            (u128::from(
                stats
                    .candidate_end_lanes
                    .saturating_sub(stats.candidate_suffix2_lanes),
            ) * 1000
                / u128::from(stats.haystack_bytes)) as u64,
        ));
        metrics.push(metric(
            "scan_ac_irregular_suffix3_extra_skipped_lanes_x1000",
            (u128::from(
                stats
                    .candidate_suffix2_lanes
                    .saturating_sub(stats.candidate_suffix3_lanes),
            ) * 1000
                / u128::from(stats.haystack_bytes)) as u64,
        ));
    }
    metrics
}

pub(super) fn scan_ac_baseline_metric_points(stats: ScanAcStats) -> Vec<MetricPoint> {
    vec![
        metric(
            "scan_ac_irregular_haystack_bytes",
            u64::from(stats.haystack_bytes),
        ),
        metric(
            "scan_ac_irregular_packed_haystack_words",
            u64::from(stats.packed_haystack_words),
        ),
        metric("scan_ac_irregular_patterns", u64::from(stats.patterns)),
        metric("scan_ac_irregular_dfa_states", u64::from(stats.dfa_states)),
        metric(
            "scan_ac_irregular_max_pattern_len",
            u64::from(stats.max_pattern_len),
        ),
        metric(
            "scan_ac_irregular_output_records",
            u64::from(stats.output_records),
        ),
        metric(
            "scan_ac_irregular_expected_matches",
            u64::from(stats.expected_matches),
        ),
        metric(
            "scan_ac_irregular_max_matches",
            u64::from(stats.max_matches),
        ),
        metric(
            "scan_ac_irregular_match_readback_bytes",
            u64::from(stats.expected_matches) * 12,
        ),
        metric(
            "scan_ac_irregular_avoided_match_readback_bytes",
            u64::from(stats.max_matches.saturating_sub(stats.expected_matches)) * 12,
        ),
        metric(
            "scan_ac_irregular_planted_matches",
            u64::from(stats.planted_matches),
        ),
    ]
}

fn metric(name: &str, value: u64) -> MetricPoint {
    MetricPoint {
        name: name.to_string(),
        value,
    }
}
