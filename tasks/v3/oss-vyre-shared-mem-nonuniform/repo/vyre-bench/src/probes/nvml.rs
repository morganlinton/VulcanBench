use crate::api::metric::GpuCounter;
use std::process::Command;

/// Capture NVML telemetry using `nvidia-smi`.
/// A GPU-required run treats missing GPU telemetry as an environment failure
/// before this probe is called; this function records only counters that are
/// successfully reported by the driver.
pub fn capture_nvml_telemetry() -> anyhow::Result<Vec<GpuCounter>> {
    let output = Command::new("nvidia-smi")
        .args([
            "--query-gpu=name,clocks.max.memory,clocks.current.memory,clocks.max.graphics,clocks.current.graphics,pstate,clocks_throttle_reasons.active,power.draw,power.limit,temperature.gpu,memory.total,memory.used,memory.free,utilization.gpu,utilization.memory",
            "--format=csv,noheader,nounits",
        ])
        .output()
        .map_err(|error| anyhow::anyhow!(
            "nvidia-smi telemetry probe failed: {error}. Fix: repair NVIDIA driver visibility before collecting GPU benchmark evidence."
        ))?;

    if !output.status.success() {
        return Err(anyhow::anyhow!(
            "nvidia-smi telemetry probe exited with status {}: {}. Fix: repair NVIDIA driver visibility before collecting GPU benchmark evidence.",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    let stdout = String::from_utf8(output.stdout)
        .map_err(|error| anyhow::anyhow!("nvidia-smi telemetry output was not UTF-8: {error}"))?;
    let line = stdout
        .lines()
        .next()
        .ok_or_else(|| anyhow::anyhow!(
            "nvidia-smi telemetry probe returned no GPU rows. Fix: verify `nvidia-smi --query-gpu=...` reports the benchmark GPU."
        ))?;
    parse_nvml_telemetry_row(line)
}

fn parse_nvml_telemetry_row(line: &str) -> anyhow::Result<Vec<GpuCounter>> {
    let mut counters = Vec::new();
    let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
    if parts.len() != 15 {
        return Err(anyhow::anyhow!(
            "nvidia-smi telemetry row had {} fields, expected 15. Fix: update the NVML parser with the exact query schema before trusting benchmark counters.",
            parts.len()
        ));
    }

    push_u64(&mut counters, "clock_mem_max_mhz", parts[1]);
    push_u64(&mut counters, "clock_mem_current_mhz", parts[2]);
    push_u64(&mut counters, "clock_graphics_max_mhz", parts[3]);
    push_u64(&mut counters, "clock_graphics_current_mhz", parts[4]);
    if let Some(pstate) = parts[5].strip_prefix('P') {
        push_u64(&mut counters, "pstate", pstate);
    }
    push_hex(&mut counters, "clock_throttle_reasons_active", parts[6]);
    push_f64_round(&mut counters, "power_draw_w", parts[7]);
    push_f64_round(&mut counters, "power_limit_w", parts[8]);
    push_u64(&mut counters, "temperature_c", parts[9]);
    push_u64(&mut counters, "memory_total_mib", parts[10]);
    push_u64(&mut counters, "memory_used_mib", parts[11]);
    push_u64(&mut counters, "memory_free_mib", parts[12]);
    push_u64(&mut counters, "utilization_gpu_pct", parts[13]);
    push_u64(&mut counters, "utilization_mem_pct", parts[14]);

    if let Ok(peak_gb_s) = query_peak_memory_bandwidth(parts[0]) {
        counters.push(GpuCounter {
            name: "memory_peak_gb_s_x1000".to_string(),
            value: (peak_gb_s * 1000.0) as u64,
        });
    }
    let thermal_unstable = thermal_or_clock_unstable(&counters);
    counters.push(GpuCounter {
        name: "thermal_unstable".to_string(),
        value: u64::from(thermal_unstable),
    });

    if counters.is_empty() {
        return Err(anyhow::anyhow!(
            "nvidia-smi telemetry probe produced no parseable counters. Fix: update parser coverage before collecting GPU benchmark evidence."
        ));
    }
    // Only capture the first GPU for now to avoid cross-device ambiguity.
    Ok(counters)
}

fn push_u64(counters: &mut Vec<GpuCounter>, name: &str, raw: &str) {
    if let Ok(value) = raw.parse::<u64>() {
        counters.push(GpuCounter {
            name: name.to_string(),
            value,
        });
    }
}

fn push_f64_round(counters: &mut Vec<GpuCounter>, name: &str, raw: &str) {
    if let Ok(value) = raw.parse::<f64>() {
        counters.push(GpuCounter {
            name: name.to_string(),
            value: value.round() as u64,
        });
    }
}

fn push_hex(counters: &mut Vec<GpuCounter>, name: &str, raw: &str) {
    let trimmed = raw.trim_start_matches("0x");
    if let Ok(value) = u64::from_str_radix(trimmed, 16).or_else(|_| raw.parse::<u64>()) {
        counters.push(GpuCounter {
            name: name.to_string(),
            value,
        });
    }
}

fn counter_value(counters: &[GpuCounter], name: &str) -> Option<u64> {
    counters
        .iter()
        .find(|counter| counter.name == name)
        .map(|counter| counter.value)
}

fn thermal_or_clock_unstable(counters: &[GpuCounter]) -> bool {
    let throttled = counter_value(counters, "clock_throttle_reasons_active").unwrap_or(0) != 0;
    let hot = counter_value(counters, "temperature_c").is_some_and(|temp| temp >= 85);
    let mem_clock_low = match (
        counter_value(counters, "clock_mem_current_mhz"),
        counter_value(counters, "clock_mem_max_mhz"),
    ) {
        (Some(current), Some(max)) if max > 0 => current.saturating_mul(100) < max * 90,
        _ => false,
    };
    throttled || hot || mem_clock_low
}

pub fn query_peak_memory_bandwidth(adapter_name: &str) -> anyhow::Result<f64> {
    if adapter_name.contains("RTX 5090") {
        return Ok(1792.0);
    }
    if adapter_name.contains("RTX 4090") {
        return Ok(1008.0);
    }
    if adapter_name.contains("A100") {
        return Ok(2039.0);
    }
    if adapter_name.contains("H100") {
        return Ok(3350.0);
    }
    if adapter_name.contains("MI300X") {
        return Ok(5300.0);
    }

    let output = std::process::Command::new("nvidia-smi")
        .args([
            "--query-gpu=clocks.max.memory",
            "--format=csv,noheader,nounits",
        ])
        .output()?;

    let stdout = String::from_utf8(output.stdout)?;
    let line = stdout
        .lines()
        .next()
        .ok_or_else(|| anyhow::anyhow!("No NVML output"))?;
    let clock_mhz = line.trim().parse::<f64>()?;

    let bus_bits = if adapter_name.contains("RTX 4080") || adapter_name.contains("RTX 5080") {
        256.0
    } else {
        384.0
    };

    // clock_mhz is data-rate for GDDR in NVML
    // bandwidth GB/s = clock_mhz * bus_bits / 8 / 1000
    Ok(clock_mhz * bus_bits / 8.0 / 1000.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nvml_telemetry_row_requires_exact_schema_width() {
        let error = parse_nvml_telemetry_row("RTX 5090, 14001")
            .expect_err("short telemetry row must be rejected");
        assert!(
            error.to_string().contains("expected 15"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn nvml_telemetry_row_parses_required_gpu_counters() {
        let counters = parse_nvml_telemetry_row(
            "NVIDIA GeForce RTX 5090, 14001, 14001, 2500, 2400, P0, 0x0, 420.4, 600.0, 72, 32768, 8192, 24576, 97, 88",
        )
        .expect("Fix: valid RTX 5090 telemetry row must parse");

        for required in [
            "clock_mem_max_mhz",
            "clock_mem_current_mhz",
            "power_draw_w",
            "temperature_c",
            "utilization_gpu_pct",
            "memory_peak_gb_s_x1000",
            "thermal_unstable",
        ] {
            assert!(
                counters.iter().any(|counter| counter.name == required),
                "missing {required}"
            );
        }
        assert_eq!(
            counters
                .iter()
                .find(|counter| counter.name == "memory_peak_gb_s_x1000")
                .map(|counter| counter.value),
            Some(1_792_000)
        );
    }
}
