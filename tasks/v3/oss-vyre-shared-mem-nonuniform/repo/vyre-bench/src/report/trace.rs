use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use crate::report::json::ReportSchema;

#[derive(Debug, Serialize, Deserialize)]
struct TraceEvent {
    name: String,
    cat: String,
    ph: String,
    ts: u64,
    pid: u32,
    tid: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    args: Option<serde_json::Value>,
}

pub fn write_chrome_trace(report: &ReportSchema) -> std::io::Result<()> {
    let db_path = Path::new("vyre_roofline_trace.json");
    let file = File::create(db_path)?;
    let mut writer = BufWriter::new(file);

    writeln!(writer, "[")?;

    let mut events = Vec::new();
    let mut current_ts = 0;

    for (case_idx, case) in report.cases.iter().enumerate() {
        let pid = 1;
        let tid = u32::try_from(case_idx)
            .unwrap_or(u32::MAX)
            .saturating_add(1);

        // Phase: B (Begin), E (End)
        events.push(TraceEvent {
            name: case.id.clone(),
            cat: "benchmark".to_string(),
            ph: "B".to_string(),
            ts: current_ts,
            pid,
            tid,
            args: Some(serde_json::json!({
                "status": case.status,
            })),
        });

        // Compute intensity and roofline
        let mut args = serde_json::Map::new();
        if let Some(wall) = case.metrics.get("wall_ns") {
            args.insert("wall_ns".to_string(), serde_json::json!(wall.p50));
        }
        if let Some(gflops) = case.metrics.get("gflops_x1000") {
            args.insert("gflops_x1000".to_string(), serde_json::json!(gflops.p50));
        }
        if let Some(gb_s) = case.metrics.get("device_gb_s_x1000") {
            args.insert("device_gb_s_x1000".to_string(), serde_json::json!(gb_s.p50));
        }
        if let Some(power) = case.metrics.get("power_draw_w") {
            args.insert("power_draw_w".to_string(), serde_json::json!(power.p50));
        }
        if let Some(util) = case.metrics.get("utilization_gpu_pct") {
            args.insert(
                "utilization_gpu_pct".to_string(),
                serde_json::json!(util.p50),
            );
        }

        // Add a trace point for the metrics
        events.push(TraceEvent {
            name: format!("{}_metrics", case.id),
            cat: "roofline".to_string(),
            ph: "i".to_string(), // instant
            ts: current_ts + 10,
            pid,
            tid,
            args: Some(serde_json::Value::Object(args)),
        });

        let duration_us = case
            .metrics
            .get("wall_ns")
            .map(|s| s.p50 / 1000)
            .unwrap_or(1000);
        current_ts += duration_us;

        events.push(TraceEvent {
            name: case.id.clone(),
            cat: "benchmark".to_string(),
            ph: "E".to_string(),
            ts: current_ts,
            pid,
            tid,
            args: None,
        });

        current_ts += 1000; // gap
    }

    for (i, event) in events.iter().enumerate() {
        let json = serde_json::to_string(event)?;
        if i < events.len() - 1 {
            writeln!(writer, "{},", json)?;
        } else {
            writeln!(writer, "{}", json)?;
        }
    }

    writeln!(writer, "]")?;

    Ok(())
}
