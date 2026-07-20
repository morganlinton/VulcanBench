//! Statistics helpers: percentile + per-sample summary, plus the
//! scaled-metric formatters used by `report.rs`.

use crate::api::metric::MetricStats;

pub(super) fn compute_stats(samples: &[u64]) -> MetricStats {
    assert!(
        !samples.is_empty(),
        "compute_stats requires at least one real benchmark sample"
    );
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();

    let n = sorted.len();
    let min = sorted[0];
    let max = sorted[n - 1];
    let p50 = percentile(&sorted, 50.0);
    let p90 = percentile(&sorted, 90.0);
    let p95 = percentile(&sorted, 95.0);
    let p99 = percentile(&sorted, 99.0);
    let p999 = percentile(&sorted, 99.9);
    let p9999 = percentile(&sorted, 99.99);
    let sum: u128 = sorted.iter().map(|&sample| u128::from(sample)).sum();
    let mean = sum as f64 / n as f64;
    let variance = sorted
        .iter()
        .map(|&sample| {
            let diff = sample as f64 - mean;
            diff * diff
        })
        .sum::<f64>()
        / n as f64;

    MetricStats {
        min,
        p50,
        p90,
        p95,
        p99,
        p999,
        p9999,
        max,
        mean,
        stddev: variance.sqrt(),
        samples: n as u32,
        determinism_cv: (mean > 0.0).then_some(variance.sqrt() / mean),
    }
}

pub(super) fn percentile(sorted: &[u64], p: f64) -> u64 {
    let n = sorted.len();
    if n == 0 {
        return 0;
    }
    if n == 1 {
        return sorted[0];
    }
    let last = n - 1;
    let index = ((last as f64 * p) / 100.0).ceil() as usize;
    sorted[index.min(last)]
}

pub(super) fn format_scaled_metric(value_x1000: Option<u64>) -> String {
    value_x1000
        .map(|value| format!("{:.3}", value as f64 / 1000.0))
        .unwrap_or_else(|| "-".to_string())
}

pub(super) fn format_scaled_percent(value_x1000: Option<u64>) -> String {
    value_x1000
        .map(|value| format!("{:.2}%", value as f64 / 1000.0))
        .unwrap_or_else(|| "-".to_string())
}
