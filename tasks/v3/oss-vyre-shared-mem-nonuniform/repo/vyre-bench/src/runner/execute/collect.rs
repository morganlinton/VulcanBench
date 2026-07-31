//! Sample collection helpers used by `run_case` to harvest metric data
//! from `BenchMetrics` after each measured iteration.

use std::collections::BTreeMap;

use crate::api::case::BenchRun;

use super::metric_keys::{
    custom_metric_key, custom_metric_value, derived_metric_key, gpu_counter_value, metric_key,
    rate_per_second_x1000,
};

pub(super) fn collect_samples(
    run_result: &BenchRun,
    samples: &mut BTreeMap<&'static str, Vec<u64>>,
    collect_baseline: bool,
) {
    collect_metric_fields("", &run_result.metrics, samples);
    collect_custom_metrics("", &run_result.metrics, samples);
    collect_gpu_counters("", &run_result.metrics, samples);
    collect_derived_metrics("", &run_result.metrics, samples);
    if collect_baseline {
        if let Some(baseline) = &run_result.baseline_metrics {
            collect_metric_fields("baseline_", baseline, samples);
            collect_custom_metrics("baseline_", baseline, samples);
            collect_gpu_counters("baseline_", baseline, samples);
            collect_derived_metrics("baseline_", baseline, samples);
        }
    }
}

pub(super) fn collect_metric_fields(
    prefix: &'static str,
    metrics: &crate::api::metric::BenchMetrics,
    samples: &mut BTreeMap<&'static str, Vec<u64>>,
) {
    // The cold_* fields of BenchMetrics are never populated by case::run() — they
    // are filled by run_case.rs directly from the first warmup wall-clock and the
    // stored cold_metrics snapshot, bypassing collect_metric_fields entirely and
    // inserting single-sample MetricStats rows into the final BTreeMap.  Entries
    // here for cold_* names would be permanently inert: metric_key() has no
    // cold_* arms (returns None), so the inner `if let (Some(_), Some(_))` guard
    // would never fire.  Keeping them misleads anyone adding a new cold_* metric
    // into thinking this table is the right place.
    #[allow(clippy::type_complexity)]
    const FIELDS: [(&str, fn(&crate::api::metric::BenchMetrics) -> Option<u64>); 25] = [
        ("wall_ns", |m| m.wall_ns),
        ("cpu_ns", |m| m.cpu_ns),
        ("compile_ns", |m| m.compile_ns),
        ("validate_ns", |m| m.validate_ns),
        ("optimize_ns", |m| m.optimize_ns),
        ("lower_ns", |m| m.lower_ns),
        ("cache_lookup_ns", |m| m.cache_lookup_ns),
        ("cache_hit", |m| m.cache_hit.map(|b| if b { 1 } else { 0 })),
        ("upload_ns", |m| m.upload_ns),
        ("dispatch_ns", |m| m.dispatch_ns),
        ("kernel_queue_submit_ns", |m| m.kernel_queue_submit_ns),
        ("kernel_execute_ns", |m| m.kernel_execute_ns),
        ("device_sync_ns", |m| m.device_sync_ns),
        ("readback_ns", |m| m.readback_ns),
        ("verify_ns", |m| m.verify_ns),
        ("alloc_count", |m| m.alloc_count),
        ("alloc_bytes", |m| m.alloc_bytes),
        ("peak_rss_bytes", |m| m.peak_rss_bytes),
        ("input_bytes", |m| m.input_bytes),
        ("output_bytes", |m| m.output_bytes),
        ("bytes_touched", |m| m.bytes_touched),
        ("bytes_read", |m| m.bytes_read),
        ("bytes_written", |m| m.bytes_written),
        ("atomic_op_count", |m| m.atomic_op_count),
        ("wire_bytes", |m| m.wire_bytes),
    ];
    for (name, getter) in FIELDS {
        if let (Some(value), Some(key)) = (getter(metrics), metric_key(prefix, name)) {
            samples.entry(key).or_default().push(value);
        }
    }
}

pub(super) fn collect_custom_metrics(
    prefix: &'static str,
    metrics: &crate::api::metric::BenchMetrics,
    samples: &mut BTreeMap<&'static str, Vec<u64>>,
) {
    for point in &metrics.custom {
        if let Some(key) = custom_metric_key(prefix, point.name.as_str()) {
            samples.entry(key).or_default().push(point.value);
        }
    }
}

pub(super) fn collect_gpu_counters(
    prefix: &'static str,
    metrics: &crate::api::metric::BenchMetrics,
    samples: &mut BTreeMap<&'static str, Vec<u64>>,
) {
    for counter in &metrics.gpu_counter {
        // use custom_metric_key to leak the names into the standard space safely
        if let Some(key) = custom_metric_key(prefix, counter.name.as_str()) {
            samples.entry(key).or_default().push(counter.value);
        }
    }
}

pub(super) fn collect_derived_metrics(
    prefix: &'static str,
    metrics: &crate::api::metric::BenchMetrics,
    samples: &mut BTreeMap<&'static str, Vec<u64>>,
) {
    let host_bytes = metrics.bytes_touched.unwrap_or_else(|| {
        metrics
            .input_bytes
            .unwrap_or(0)
            .saturating_add(metrics.output_bytes.unwrap_or(0))
    });
    // device_gb_s_x1000 requires explicit bytes_read + bytes_written from the case.
    // If neither is set, omit device_gb_s_x1000 entirely rather than substituting
    // host_bytes — that substitution would report a spurious device bandwidth figure
    // computed from host I/O, which is a metric miscompile (Law 10 silent fallback).
    let device_bytes = match (metrics.bytes_read, metrics.bytes_written) {
        (Some(r), Some(w)) => {
            let total = r.saturating_add(w);
            if total > 0 { Some(total) } else { None }
        }
        (Some(r), None) if r > 0 => Some(r),
        (None, Some(w)) if w > 0 => Some(w),
        _ => None,
    };

    if let Some(wall_ns) = metrics.wall_ns.filter(|ns| *ns > 0) {
        if host_bytes > 0 {
            if let Some(key) = derived_metric_key(prefix, "wall_gb_s_x1000") {
                samples.entry(key).or_default().push(rate_per_second_x1000(
                    host_bytes,
                    wall_ns,
                    1_000_000_000,
                ));
            }
        }
    }

    if let Some(dev_bytes) = device_bytes {
        if let Some(device_ns) = metrics.dispatch_ns.or(metrics.wall_ns).filter(|ns| *ns > 0) {
            if let Some(key) = derived_metric_key(prefix, "device_gb_s_x1000") {
                samples.entry(key).or_default().push(rate_per_second_x1000(
                    dev_bytes,
                    device_ns,
                    1_000_000_000,
                ));
            }
        }
    }

    if let Some(flop_count) = custom_metric_value(metrics, "flop_count") {
        if let Some(active_ns) = metrics.dispatch_ns.or(metrics.wall_ns).filter(|ns| *ns > 0) {
            if let Some(key) = derived_metric_key(prefix, "gflops_x1000") {
                samples.entry(key).or_default().push(rate_per_second_x1000(
                    flop_count,
                    active_ns,
                    1_000_000_000,
                ));
            }
        }
    }

    if let Some(peak_gb_s_x1000) =
        gpu_counter_value(metrics, "memory_peak_gb_s_x1000").filter(|v| *v > 0)
    {
        if let (Some(dev_bytes), Some(wall_ns)) =
            (device_bytes, metrics.wall_ns.filter(|ns| *ns > 0))
        {
            let achieved_gb_s_x1000 = rate_per_second_x1000(dev_bytes, wall_ns, 1_000_000_000);
            if let Some(key) = derived_metric_key(prefix, "roofline_mem_pct_x1000") {
                samples.entry(key).or_default().push(
                    ((u128::from(achieved_gb_s_x1000) * 100_000) / u128::from(peak_gb_s_x1000))
                        .min(u128::from(u64::MAX)) as u64,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::api::metric::{BenchMetrics, MetricPoint};

    use super::{collect_derived_metrics, collect_metric_fields};

    fn metrics_with_host_only(input_bytes: u64, wall_ns: u64) -> BenchMetrics {
        BenchMetrics {
            input_bytes: Some(input_bytes),
            wall_ns: Some(wall_ns),
            ..Default::default()
        }
    }

    fn metrics_with_device(bytes_read: u64, bytes_written: u64, wall_ns: u64) -> BenchMetrics {
        BenchMetrics {
            bytes_read: Some(bytes_read),
            bytes_written: Some(bytes_written),
            wall_ns: Some(wall_ns),
            ..Default::default()
        }
    }

    /// Regression for device-bytes-silent-host-fallback: when a case sets only
    /// input_bytes/output_bytes (host-side I/O) and no bytes_read/bytes_written,
    /// device_gb_s_x1000 must be absent from the sample map.  Before the fix,
    /// device_bytes silently fell back to host_bytes and device_gb_s_x1000 was
    /// emitted as if the GPU had transferred 512 MiB of device memory — a metric
    /// miscompile that cannot be distinguished from a real device bandwidth measurement.
    #[test]
    fn device_gb_s_x1000_absent_when_no_device_bytes_set() {
        // 512 MiB of host input, 1 second wall time — no bytes_read/bytes_written.
        let metrics = metrics_with_host_only(512 * 1024 * 1024, 1_000_000_000);
        let mut samples: BTreeMap<&'static str, Vec<u64>> = BTreeMap::new();
        collect_derived_metrics("", &metrics, &mut samples);

        assert!(
            !samples.contains_key("device_gb_s_x1000"),
            "Fix: device_gb_s_x1000 must NOT be emitted when bytes_read/bytes_written are absent \
             (512 MiB host input must not be misreported as device bandwidth). \
             Got samples: {:?}",
            samples.keys().collect::<Vec<_>>()
        );
        // wall_gb_s_x1000 MUST still be present — host bandwidth is unaffected.
        assert!(
            samples.contains_key("wall_gb_s_x1000"),
            "Fix: wall_gb_s_x1000 must still be emitted when host bytes are present."
        );
        let wall_val = samples["wall_gb_s_x1000"][0];
        // 512 MiB = 536870912 bytes, 1 s = 1_000_000_000 ns, scale = 1_000_000_000.
        // rate_per_second_x1000 = (units * 1e12) / (wall_ns * scale)
        //   = (536870912 * 1_000_000_000_000) / (1_000_000_000 * 1_000_000_000)
        //   = 536870912_000_000_000_000 / 1_000_000_000_000_000_000
        //   = 536 (integer division).
        // 512 MiB / 1 s ≈ 0.537 GB/s → 537 x1000-units; floor = 536.
        assert_eq!(
            wall_val, 536,
            "Fix: wall_gb_s_x1000 must equal 536 for 512 MiB / 1 s; got {wall_val}"
        );
    }

    /// device_gb_s_x1000 MUST be present and correct when both bytes_read and
    /// bytes_written are explicitly set by the case.
    #[test]
    fn device_gb_s_x1000_present_when_device_bytes_set() {
        // 256 MiB read + 256 MiB written = 512 MiB device transfer, 1 second.
        let metrics = metrics_with_device(256 * 1024 * 1024, 256 * 1024 * 1024, 1_000_000_000);
        let mut samples: BTreeMap<&'static str, Vec<u64>> = BTreeMap::new();
        collect_derived_metrics("", &metrics, &mut samples);

        assert!(
            samples.contains_key("device_gb_s_x1000"),
            "Fix: device_gb_s_x1000 must be emitted when bytes_read and bytes_written are set."
        );
        let val = samples["device_gb_s_x1000"][0];
        // 512 MiB device transfer / 1 s → same calculation as above: 536 x1000-units.
        assert_eq!(
            val, 536,
            "Fix: device_gb_s_x1000 must equal 536 for 512 MiB device transfer / 1 s; got {val}"
        );
    }

    /// Regression for dead-cold-fields-in-collect-fields-array: the 7 cold_* names
    /// previously listed in FIELDS are permanently inert because metric_key() returns
    /// None for them.  After the fix the FIELDS array no longer contains cold_* entries,
    /// so even if BenchMetrics.cold_wall_ns is set, collect_metric_fields must not emit
    /// a "cold_wall_ns" sample (the cold path in run_case.rs is the authoritative emitter).
    #[test]
    fn collect_metric_fields_does_not_emit_cold_samples() {
        let mut metrics = BenchMetrics::default();
        metrics.cold_wall_ns = Some(12_345_678);
        metrics.cold_compile_ns = Some(9_000_000);
        metrics.cold_optimize_ns = Some(1_000_000);
        metrics.cold_lower_ns = Some(500_000);
        metrics.cold_cache_lookup_ns = Some(200_000);
        metrics.cold_dispatch_ns = Some(100_000);
        metrics.cold_readback_ns = Some(50_000);

        let mut samples: BTreeMap<&'static str, Vec<u64>> = BTreeMap::new();
        collect_metric_fields("", &metrics, &mut samples);

        for cold_key in [
            "cold_wall_ns",
            "cold_compile_ns",
            "cold_optimize_ns",
            "cold_lower_ns",
            "cold_cache_lookup_ns",
            "cold_dispatch_ns",
            "cold_readback_ns",
        ] {
            assert!(
                !samples.contains_key(cold_key),
                "Fix: collect_metric_fields must not emit `{cold_key}` — cold-path metrics are \
                 populated by run_case.rs directly and do not route through collect_metric_fields."
            );
        }
    }

    /// Verify that the cold_* entries were removed from FIELDS (not just silenced) by
    /// checking the sample map is entirely empty when only cold_* fields are set on
    /// BenchMetrics.  Previously the entries were present but always inert; now they
    /// should simply not be there.
    #[test]
    fn collect_metric_fields_cold_only_metrics_produce_empty_samples() {
        let mut metrics = BenchMetrics::default();
        metrics.cold_wall_ns = Some(999);
        let mut samples: BTreeMap<&'static str, Vec<u64>> = BTreeMap::new();
        collect_metric_fields("", &metrics, &mut samples);
        assert!(
            samples.is_empty(),
            "Fix: a BenchMetrics with only cold_wall_ns set must produce zero samples from \
             collect_metric_fields; got {:?}",
            samples.keys().collect::<Vec<_>>()
        );
    }

    /// collect_custom_metrics must emit a sample for every SyntheticCountWorkload
    /// metric_name that passes through custom_metric_key.
    #[test]
    fn collect_custom_metrics_emits_synthetic_count_workload_metrics() {
        use super::collect_custom_metrics;
        // Verify the collect path: build BenchMetrics with custom points for each
        // synthetic workload name and confirm they appear in the sample map.
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
            "callgraph_witness_digest",
        ];
        let mut metrics = BenchMetrics::default();
        for (i, name) in names.iter().enumerate() {
            metrics.custom.push(MetricPoint {
                name: (*name).to_string(),
                value: (i as u64 + 1) * 1_000_000,
            });
        }
        let mut samples: BTreeMap<&'static str, Vec<u64>> = BTreeMap::new();
        collect_custom_metrics("", &metrics, &mut samples);
        for (i, name) in names.iter().enumerate() {
            let expected_value = (i as u64 + 1) * 1_000_000;
            let actual = samples.get(*name).and_then(|v| v.first()).copied();
            assert_eq!(
                actual,
                Some(expected_value),
                "Fix: custom metric `{name}` must be collected with value {expected_value}; \
                 got {actual:?}"
            );
        }
    }
}
