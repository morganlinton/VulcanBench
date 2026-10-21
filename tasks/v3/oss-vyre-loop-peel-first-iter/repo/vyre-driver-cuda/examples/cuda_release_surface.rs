//! CUDA release-surface and megakernel speedup evidence verifier.

const RESIDENT_GRAPH_SESSION_INPUT_HEADER: &str = "backend_id,device_ordinal,device_memory_bytes,compute_capability_major,compute_capability_minor,graph_nodes,graph_edges,graph_layout_hash,graph_bytes,run_count,per_run_frontier_bytes,reusable_scratch_bytes,per_run_output_bytes,budget_bytes,host_orchestrated_ns,resident_megakernel_ns,setup_ns";

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let Some(first) = args.next() else {
        print_release_surface();
        return Ok(());
    };
    match first.as_str() {
        "--verify-megakernel-speedup-csv" => {
            let path = args.next().ok_or_else(|| {
                "Fix: pass a CSV evidence path after --verify-megakernel-speedup-csv.".to_string()
            })?;
            let threshold = match args.next() {
                Some(value) => value.parse::<f64>().map_err(|_| {
                    format!(
                        "Fix: required speedup threshold `{value}` is not a finite numeric value."
                    )
                })?,
                None => 100.0,
            };
            if args.next().is_some() {
                return Err(
                    "Fix: usage is cuda_release_surface --verify-megakernel-speedup-csv <path> [required_speedup_x]."
                        .to_string(),
                );
            }
            let csv = std::fs::read_to_string(&path).map_err(|error| {
                format!("Fix: failed to read CUDA speedup evidence `{path}`: {error}")
            })?;
            let proof =
                vyre_driver_cuda::validate_cuda_megakernel_speedup_evidence_csv(&csv, threshold)
                    .map_err(|error| error.to_string())?;
            println!(
                "validated_cuda_megakernel_speedup,min_x={:.3},max_x={:.3},samples={},repetitions={}",
                proof.min_speedup_x,
                proof.max_speedup_x,
                proof.sample_count,
                proof.total_repetitions
            );
            Ok(())
        }
        "--format-resident-graph-speedup-csv" => {
            let path = args.next().ok_or_else(|| {
                "Fix: pass a resident graph evidence CSV path after --format-resident-graph-speedup-csv.".to_string()
            })?;
            let threshold = match args.next() {
                Some(value) => value.parse::<f64>().map_err(|_| {
                    format!(
                        "Fix: required speedup threshold `{value}` is not a finite numeric value."
                    )
                })?,
                None => 100.0,
            };
            if args.next().is_some() {
                return Err(
                    "Fix: usage is cuda_release_surface --format-resident-graph-speedup-csv <path> [required_speedup_x]."
                        .to_string(),
                );
            }
            let csv = std::fs::read_to_string(&path).map_err(|error| {
                format!("Fix: failed to read CUDA resident graph evidence `{path}`: {error}")
            })?;
            let evidence = parse_resident_graph_session_evidence_csv(&csv)?;
            let (_proof, formatted) =
                vyre_driver_cuda::format_validated_cuda_resident_graph_session_evidence_csv(
                    &evidence, threshold,
                )
                .map_err(|error| error.to_string())?;
            print!("{formatted}");
            Ok(())
        }
        "--print-megakernel-speedup-csv-header" => {
            if args.next().is_some() {
                return Err(
                    "Fix: --print-megakernel-speedup-csv-header does not accept extra arguments."
                        .to_string(),
                );
            }
            println!(
                "{}",
                vyre_driver_cuda::MEGAKERNEL_SPEEDUP_EVIDENCE_CSV_HEADER
            );
            Ok(())
        }
        "--print-cuda-device-evidence-prefix" => {
            let ordinal = match args.next() {
                Some(value) => value.parse::<usize>().map_err(|_| {
                    format!("Fix: CUDA device ordinal `{value}` is not an unsigned integer.")
                })?,
                None => 0,
            };
            if args.next().is_some() {
                return Err(
                    "Fix: usage is cuda_release_surface --print-cuda-device-evidence-prefix [ordinal]."
                        .to_string(),
                );
            }
            let handle = vyre_driver_cuda::CudaDeviceHandle::acquire_ordinal(ordinal)
                .map_err(|error| format!("Fix: failed to acquire CUDA release device: {error}"))?;
            println!(
                "{},{},{},{},{}",
                vyre_driver_cuda::CUDA_BACKEND_ID,
                handle.caps.ordinal,
                handle.caps.total_memory,
                handle.caps.compute_capability.0,
                handle.caps.compute_capability.1
            );
            Ok(())
        }
        "--help" | "-h" => {
            print_usage();
            Ok(())
        }
        other => Err(format!(
            "Fix: unknown argument `{other}`. Run with --help for supported release checks."
        )),
    }
}

fn print_release_surface() {
    println!("CUDA backend id: {}", vyre_driver_cuda::CUDA_BACKEND_ID);
    println!(
        "CUDA caps type: {}",
        std::any::type_name::<vyre_driver_cuda::CudaDeviceCaps>()
    );
    println!("Acquire with vyre_driver_cuda::CudaBackend::acquire() on a CUDA host.");
    println!(
        "Megakernel speedup CSV header: {}",
        vyre_driver_cuda::MEGAKERNEL_SPEEDUP_EVIDENCE_CSV_HEADER
    );
    println!("Resident graph evidence CSV header: {RESIDENT_GRAPH_SESSION_INPUT_HEADER}");
    println!("Live CUDA device evidence prefix: run --print-cuda-device-evidence-prefix [ordinal]");
}

fn print_usage() {
    println!("Usage:");
    println!("  cuda_release_surface");
    println!("  cuda_release_surface --verify-megakernel-speedup-csv <path> [required_speedup_x]");
    println!(
        "  cuda_release_surface --format-resident-graph-speedup-csv <path> [required_speedup_x]"
    );
    println!("  cuda_release_surface --print-megakernel-speedup-csv-header");
    println!("  cuda_release_surface --print-cuda-device-evidence-prefix [ordinal]");
    println!(
        "CSV header: {}",
        vyre_driver_cuda::MEGAKERNEL_SPEEDUP_EVIDENCE_CSV_HEADER
    );
    println!("Resident graph input header: {RESIDENT_GRAPH_SESSION_INPUT_HEADER}");
}

fn parse_resident_graph_session_evidence_csv(
    csv: &str,
) -> Result<Vec<vyre_driver_cuda::CudaResidentGraphSessionEvidence>, String> {
    let mut lines = csv.lines().filter(|line| !line.trim().is_empty());
    let header = lines
        .next()
        .ok_or_else(|| "Fix: resident graph evidence CSV is empty.".to_string())?;
    if header.trim() != RESIDENT_GRAPH_SESSION_INPUT_HEADER {
        return Err(format!(
            "Fix: resident graph evidence CSV header must be `{RESIDENT_GRAPH_SESSION_INPUT_HEADER}` but found `{}`.",
            header.trim()
        ));
    }
    let mut evidence = Vec::new();
    for (row_index, line) in lines.enumerate() {
        let line_number = row_index + 2;
        let fields: Vec<&str> = line.split(',').map(str::trim).collect();
        if fields.len() != 17 {
            return Err(format!(
                "Fix: resident graph evidence line {line_number} must have 17 fields."
            ));
        }
        let backend_id = parse_cuda_backend_id(fields[0], line_number)?;
        let device_ordinal = parse_u64(fields[1], line_number, "device_ordinal")?;
        let device_memory_bytes = parse_u64(fields[2], line_number, "device_memory_bytes")?;
        let compute_capability_major =
            parse_u32(fields[3], line_number, "compute_capability_major")?;
        let compute_capability_minor =
            parse_u32(fields[4], line_number, "compute_capability_minor")?;
        let graph_nodes = parse_u64(fields[5], line_number, "graph_nodes")?;
        let graph_edges = parse_u64(fields[6], line_number, "graph_edges")?;
        let graph_layout_hash = parse_u64(fields[7], line_number, "graph_layout_hash")?;
        let graph_bytes = parse_u64(fields[8], line_number, "graph_bytes")?;
        let run_count = parse_u64(fields[9], line_number, "run_count")?;
        let per_run_frontier_bytes = parse_u64(fields[10], line_number, "per_run_frontier_bytes")?;
        let reusable_scratch_bytes = parse_u64(fields[11], line_number, "reusable_scratch_bytes")?;
        let per_run_output_bytes = parse_u64(fields[12], line_number, "per_run_output_bytes")?;
        let budget_bytes = parse_u64(fields[13], line_number, "budget_bytes")?;
        let host_orchestrated_ns = parse_f64(fields[14], line_number, "host_orchestrated_ns")?;
        let resident_megakernel_ns = parse_f64(fields[15], line_number, "resident_megakernel_ns")?;
        let setup_ns = parse_f64(fields[16], line_number, "setup_ns")?;
        let plan = vyre_driver_cuda::plan_cuda_resident_graph_session(
            vyre_driver_cuda::CudaResidentGraphSessionProfile {
                graph_layout_hash,
                graph_bytes,
                run_count,
                per_run_frontier_bytes,
                reusable_scratch_bytes,
                per_run_output_bytes,
                budget_bytes,
                readback: vyre_driver_cuda::CudaResidentGraphReadback::FinalOnly,
            },
        )
        .map_err(|error| format!("Fix: resident graph evidence line {line_number}: {error}"))?;
        evidence.push(vyre_driver_cuda::CudaResidentGraphSessionEvidence {
            backend_id,
            device_ordinal,
            device_memory_bytes,
            compute_capability_major,
            compute_capability_minor,
            graph_nodes,
            graph_edges,
            plan,
            host_orchestrated_ns,
            resident_megakernel_ns,
            setup_ns,
        });
    }
    Ok(evidence)
}

fn parse_cuda_backend_id(value: &str, line: usize) -> Result<&'static str, String> {
    if value == vyre_driver_cuda::CUDA_BACKEND_ID {
        Ok(vyre_driver_cuda::CUDA_BACKEND_ID)
    } else {
        Err(format!(
            "Fix: resident graph evidence line {line} has backend_id `{value}`; release evidence must be produced by the CUDA backend."
        ))
    }
}

fn parse_u64(value: &str, line: usize, field: &str) -> Result<u64, String> {
    value.parse::<u64>().map_err(|_| {
        format!("Fix: resident graph evidence line {line} has invalid `{field}` value `{value}`.")
    })
}

fn parse_u32(value: &str, line: usize, field: &str) -> Result<u32, String> {
    value.parse::<u32>().map_err(|_| {
        format!("Fix: resident graph evidence line {line} has invalid `{field}` value `{value}`.")
    })
}

fn parse_f64(value: &str, line: usize, field: &str) -> Result<f64, String> {
    value.parse::<f64>().map_err(|_| {
        format!("Fix: resident graph evidence line {line} has invalid `{field}` value `{value}`.")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resident_graph_formatter_parses_release_scale_rows_and_roundtrips() {
        let input = format!(
            "{RESIDENT_GRAPH_SESSION_INPUT_HEADER}\n\
             cuda,0,34359738368,12,0,10000,80000,2748,1048576,128,4096,65536,2048,2000000,1000000,10000,250000\n\
             cuda,0,34359738368,12,0,20000,160000,3567,2097152,256,8192,131072,4096,4000000,2500000,20000,350000\n"
        );
        let evidence = parse_resident_graph_session_evidence_csv(&input)
            .expect("Fix: resident graph release evidence should parse");
        let (proof, csv) =
            vyre_driver_cuda::format_validated_cuda_resident_graph_session_evidence_csv(
                &evidence, 100.0,
            )
            .expect("Fix: resident graph release evidence should format");
        let reparsed = vyre_driver_cuda::validate_cuda_megakernel_speedup_evidence_csv(&csv, 100.0)
            .expect("Fix: formatted resident graph evidence should verify");

        assert_eq!(proof, reparsed);
        assert_eq!(proof.sample_count, 2);
        assert_eq!(proof.total_repetitions, 384);
        assert!(csv.starts_with(vyre_driver_cuda::MEGAKERNEL_SPEEDUP_EVIDENCE_CSV_HEADER));
    }

    #[test]
    fn resident_graph_formatter_rejects_bad_rows_before_release_verifier() {
        let bad_header = "graph_nodes,graph_edges\n";
        let header_error = parse_resident_graph_session_evidence_csv(bad_header)
            .expect_err("bad header should fail");
        assert!(header_error.contains("resident graph evidence CSV header must be"));

        let bad_field = format!(
            "{RESIDENT_GRAPH_SESSION_INPUT_HEADER}\n\
             cuda,0,34359738368,12,0,10000,80000,2748,not_bytes,128,4096,65536,2048,2000000,1000000,10000,250000\n"
        );
        let field_error = parse_resident_graph_session_evidence_csv(&bad_field)
            .expect_err("bad numeric field should fail");
        assert!(field_error.contains("graph_bytes"));

        let wrong_backend = format!(
            "{RESIDENT_GRAPH_SESSION_INPUT_HEADER}\n\
             wgpu,0,34359738368,12,0,10000,80000,2748,1048576,128,4096,65536,2048,2000000,1000000,10000,250000\n"
        );
        let backend_error = parse_resident_graph_session_evidence_csv(&wrong_backend)
            .expect_err("resident graph release formatter must reject non-CUDA rows");
        assert!(backend_error.contains("release evidence must be produced by the CUDA backend"));
    }
}
