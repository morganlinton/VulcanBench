use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};

use crate::runner::{evaluate_candidate_headless, RunConfig};

#[derive(Debug, Deserialize)]
pub struct EvolveRequest {
    pub case_id: String,
    pub candidate_ir: String,
    pub timeout_ms: u64,
}

#[derive(Debug, Serialize)]
pub struct EvolveResponse {
    pub case_id: String,
    pub fitness: Option<f64>,
    pub error: Option<String>,
}

pub fn run_evolve_server() -> anyhow::Result<()> {
    let registry = crate::registry::collect_all();
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    let mut reader = BufReader::new(stdin.lock());

    let mut line = String::new();
    loop {
        line.clear();
        if reader.read_line(&mut line)? == 0 {
            break; // EOF
        }

        let request: EvolveRequest = match serde_json::from_str(&line) {
            Ok(req) => req,
            Err(e) => {
                let resp = EvolveResponse {
                    case_id: "".to_string(),
                    fitness: None,
                    error: Some(format!("Invalid JSON request: {}", e)),
                };
                serde_json::to_writer(&mut stdout, &resp)?;
                stdout.write_all(b"\n")?;
                stdout.flush()?;
                continue;
            }
        };

        let response = evaluate(&registry, &request);
        serde_json::to_writer(&mut stdout, &response)?;
        stdout.write_all(b"\n")?;
        stdout.flush()?;
    }

    Ok(())
}

fn evaluate(registry: &crate::registry::BenchRegistry, req: &EvolveRequest) -> EvolveResponse {
    let program = match vyre::ir::Program::from_text(&req.candidate_ir) {
        Ok(p) => p,
        Err(e) => {
            return EvolveResponse {
                case_id: req.case_id.clone(),
                fitness: None,
                error: Some(format!("Failed to parse IR: {:?}", e)),
            }
        }
    };

    let config = RunConfig {
        backend_id: None,
        enforce_budgets: true,
        case_ids: vec![req.case_id.clone()],
        warmup_samples: 1,
        measured_samples: Some(3),
        sample_timeout: std::time::Duration::from_millis(req.timeout_ms),
        determinism_runs: 1,
        workgroup_override: None,
        baseline_warmup_runs: 1,
        snapshot_on_pass: false,
    };

    match evaluate_candidate_headless(registry, &req.case_id, program, &config) {
        Ok(report) => {
            // Fitness scalar: Use GFLOPS if compute-bound, or GB/s if memory-bound. Fallback to scaled wall time.
            let fitness = if let Some(gflops) = report.metrics.get("gflops_x1000") {
                gflops.p50 as f64 / 1000.0
            } else if let Some(gb_s) = report.metrics.get("device_gb_s_x1000") {
                gb_s.p50 as f64 / 1000.0
            } else if let Some(wall_stats) = report.metrics.get("wall_ns") {
                1_000_000_000.0 / wall_stats.p50 as f64
            } else {
                return EvolveResponse {
                    case_id: req.case_id.clone(),
                    fitness: None,
                    error: Some("No throughput or wall_ns metric produced".to_string()),
                };
            };
            EvolveResponse {
                case_id: req.case_id.clone(),
                fitness: Some(fitness),
                error: None,
            }
        }
        Err(e) => EvolveResponse {
            case_id: req.case_id.clone(),
            fitness: None,
            error: Some(e),
        },
    }
}
