use std::fs;
use std::path::Path;

use serde::Serialize;
use serde_json::Value;

pub(super) fn write_json(path: &Path, value: &impl Serialize) {
    if let Some(parent) = path.parent() {
        if let Err(error) = fs::create_dir_all(parent) {
            eprintln!("Fix: failed to create `{}`: {error}", parent.display());
            std::process::exit(1);
        }
    }
    let json = match serde_json::to_string_pretty(value) {
        Ok(json) => json,
        Err(error) => {
            eprintln!("Fix: failed to serialize `{}`: {error}", path.display());
            std::process::exit(1);
        }
    };
    if let Err(error) = fs::write(path, format!("{json}\n")) {
        eprintln!("Fix: failed to write `{}`: {error}", path.display());
        std::process::exit(1);
    }
}

pub(super) fn release_axis_blockers(reports: &[Value]) -> Vec<String> {
    let mut blockers = Vec::new();
    if reports.is_empty() {
        blockers.push("no benchmark case reports available for release axes".to_string());
    }
    if reports.len() < 12 {
        blockers.push(format!(
            "only {} benchmark report(s) available for release axes; release needs at least 12 workload reports",
            reports.len()
        ));
    }
    if min_metric_p50(reports, "wall_ns").is_none() {
        blockers.push("missing wall_ns metric for warm_us_per_file".to_string());
    }
    if min_first_available_metric_p50(
        reports,
        &[
            "cold_compile_ns",
            "cold_wall_ns",
            "compile_ns",
            "lower_ns",
            "optimize_ns",
        ],
    )
    .is_none()
    {
        blockers.push("missing cold/compile metric for cold_pipeline_build_ms".to_string());
    }
    if max_metric_p50(reports, "wall_gb_s_x1000")
        .or_else(|| max_metric_p50(reports, "device_gb_s_x1000"))
        .is_none()
    {
        blockers.push("missing throughput metric for gbs_scan_throughput".to_string());
    }
    if max_vram_mib(reports).is_none() {
        blockers.push("missing GPU memory evidence for max_vram_mib".to_string());
    }
    blockers
}

pub(super) fn min_first_available_metric_p50(reports: &[Value], keys: &[&str]) -> Option<u64> {
    keys.iter().find_map(|key| min_metric_p50(reports, key))
}

pub(super) fn min_metric_p50(reports: &[Value], key: &str) -> Option<u64> {
    metric_p50_values(reports, key).into_iter().min()
}

pub(super) fn max_metric_p50(reports: &[Value], key: &str) -> Option<u64> {
    metric_p50_values(reports, key).into_iter().max()
}

pub(super) fn metric_p50_values(reports: &[Value], key: &str) -> Vec<u64> {
    let mut values = Vec::new();
    for report in reports {
        let Some(cases) = report.get("cases").and_then(Value::as_array) else {
            continue;
        };
        for case in cases {
            let Some(metrics) = case.get("metrics").and_then(Value::as_object) else {
                continue;
            };
            let Some(value) = metrics
                .get(key)
                .and_then(|metric| metric.get("p50"))
                .and_then(Value::as_u64)
            else {
                continue;
            };
            values.push(value);
        }
    }
    values
}

pub(super) fn max_observed_ulp(reports: &[Value]) -> Option<u32> {
    let mut max_ulp = None::<u32>;
    for report in reports {
        let Some(cases) = report.get("cases").and_then(Value::as_array) else {
            continue;
        };
        for case in cases {
            if let Some(ulp) = case
                .get("correctness")
                .and_then(|correctness| correctness.get("Toleranced"))
                .and_then(|toleranced| toleranced.get("max_observed_ulp"))
                .and_then(Value::as_u64)
            {
                let ulp = ulp.min(u64::from(u32::MAX)) as u32;
                max_ulp = Some(max_ulp.map_or(ulp, |current| current.max(ulp)));
            }
        }
    }
    max_ulp
}

pub(super) fn max_vram_mib(reports: &[Value]) -> Option<u64> {
    let mut max_mib = None::<u64>;
    for report in reports {
        if let Some(devices) = report
            .get("environment")
            .and_then(|environment| environment.get("gpu_devices"))
            .and_then(Value::as_array)
        {
            for device in devices {
                if let Some(mib) = device.get("memory_total_mib").and_then(Value::as_u64) {
                    max_mib = Some(max_mib.map_or(mib, |current| current.max(mib)));
                }
            }
        }
        let Some(cases) = report.get("cases").and_then(Value::as_array) else {
            continue;
        };
        for case in cases {
            if let Some(mib) = case
                .get("metrics")
                .and_then(|metrics| metrics.get("memory_total_mib"))
                .and_then(|metric| metric.get("p50"))
                .and_then(Value::as_u64)
            {
                max_mib = Some(max_mib.map_or(mib, |current| current.max(mib)));
            }
        }
    }
    max_mib
}
