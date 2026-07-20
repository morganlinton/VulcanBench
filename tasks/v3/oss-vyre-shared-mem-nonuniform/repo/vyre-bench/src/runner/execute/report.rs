//! JSON / textual report formatting. Public entry point exported via
//! `super::print_report`.

use crate::report::json::{generate_json_report, ReportSchema};

use super::stats::{format_scaled_metric, format_scaled_percent};

pub fn print_report(
    report: &ReportSchema,
    format: &str,
    roofline_only: bool,
) -> Result<(), serde_json::Error> {
    if format == "json" {
        println!("{}", generate_json_report(report)?);
        return Ok(());
    }

    if roofline_only {
        println!(
            "{:<30} | {:<10} | {:<10} | {:<10}",
            "Benchmark", "Status", "GB/s", "Roofline%"
        );
        println!("---------------------------------------------------------------------");
    } else {
        println!(
            "{:<30} | {:<10} | {:<12} | {:<12} | {:<12} | {:<13} | {:<12} | {:<12} | {:<10} | {:<10} | {:<10} | {:<10}",
            "Benchmark",
            "Status",
            "GPU p50(ns)",
            "GPU p99(ns)",
            "GPU p99.9(ns)",
            "GPU p99.99(ns)",
            "GPU Max(ns)",
            "CPU p50(ns)",
            "Speedup",
            "GB/s",
            "GFLOP/s",
            "Roofline%"
        );
        println!(
            "------------------------------------------------------------------------------------------------------------------------------------------------------------"
        );
    }
    for case in &report.cases {
        let gpu_stats = case
            .metrics
            .get("dispatch_ns")
            .or_else(|| case.metrics.get("wall_ns"));
        let gpu_p50 = gpu_stats.map(|stats| stats.p50);
        let gpu_p99 = gpu_stats.map(|stats| stats.p99);
        let gpu_p999 = gpu_stats.map(|stats| stats.p999);
        let gpu_p9999 = gpu_stats.map(|stats| stats.p9999);
        let gpu_max = gpu_stats.map(|stats| stats.max);
        let cpu_p50 = case.metrics.get("baseline_wall_ns").map(|stats| stats.p50);

        let gpu_p50_str = gpu_p50
            .map(|value| value.to_string())
            .unwrap_or_else(|| "-".to_string());
        let gpu_p99_str = gpu_p99
            .map(|value| value.to_string())
            .unwrap_or_else(|| "-".to_string());
        let gpu_p999_str = gpu_p999
            .map(|value| value.to_string())
            .unwrap_or_else(|| "-".to_string());
        let gpu_p9999_str = gpu_p9999
            .map(|value| value.to_string())
            .unwrap_or_else(|| "-".to_string());
        let gpu_max_str = gpu_max
            .map(|value| value.to_string())
            .unwrap_or_else(|| "-".to_string());
        let cpu_p50_str = cpu_p50
            .map(|value| value.to_string())
            .unwrap_or_else(|| "-".to_string());
        let speedup_str = match (gpu_p50, cpu_p50) {
            (Some(gpu), Some(cpu)) if gpu > 0 => format!("{:.1}x", cpu as f64 / gpu as f64),
            _ => "-".to_string(),
        };
        let gb_s = format_scaled_metric(
            case.metrics
                .get("device_gb_s_x1000")
                .or_else(|| case.metrics.get("wall_gb_s_x1000"))
                .map(|stats| stats.p50),
        );
        let gflops = format_scaled_metric(case.metrics.get("gflops_x1000").map(|stats| stats.p50));
        let roofline = format_scaled_percent(
            case.metrics
                .get("roofline_mem_pct_x1000")
                .map(|stats| stats.p50),
        );
        if roofline_only {
            println!(
                "{:<30} | {:<10} | {:<10} | {:<10}",
                case.id, case.status, gb_s, roofline
            );
        } else {
            println!(
                "{:<30} | {:<10} | {:<12} | {:<12} | {:<12} | {:<13} | {:<12} | {:<12} | {:<10} | {:<10} | {:<10} | {:<10}",
                case.id,
                case.status,
                gpu_p50_str,
                gpu_p99_str,
                gpu_p999_str,
                gpu_p9999_str,
                gpu_max_str,
                cpu_p50_str,
                speedup_str,
                gb_s,
                gflops,
                roofline
            );
        }
    }
    println!(
        "------------------------------------------------------------------------------------------------------------------------------------------------------------"
    );
    if let Some(rate) = report.summary.cache_hit_rate {
        println!(
            "Passed: {}, Failed: {}, Cache Hit Rate: {:.1}%",
            report.summary.passed,
            report.summary.failed,
            rate * 100.0
        );
    } else {
        println!(
            "Passed: {}, Failed: {}",
            report.summary.passed, report.summary.failed
        );
    }
    Ok(())
}
