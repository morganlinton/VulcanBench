//! CUDA megakernel steady-state speedup release gate.

const MIN_RELEASE_GRAPH_NODES: u64 = 10_000;
const MIN_RELEASE_GRAPH_EDGES: u64 = 80_000;
const MIN_RELEASE_REPETITIONS: u64 = 64;
const MIN_RELEASE_SAMPLE_COUNT: usize = 2;
const MIN_RELEASE_DEVICE_MEMORY_BYTES: u64 = 16 * 1024 * 1024 * 1024;
const MIN_RELEASE_COMPUTE_CAPABILITY_MAJOR: u32 = 8;
const MIN_RELEASE_COMPUTE_CAPABILITY_MINOR: u32 = 0;

/// One measured benchmark sample for resident megakernel release gating.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CudaMegakernelSpeedupSample {
    /// Backend identifier that produced the measured sample.
    pub backend_id: &'static str,
    /// Physical CUDA device ordinal that produced the measured sample.
    pub device_ordinal: u64,
    /// Probed CUDA device memory in bytes.
    pub device_memory_bytes: u64,
    /// Probed CUDA compute capability major version.
    pub compute_capability_major: u32,
    /// Probed CUDA compute capability minor version.
    pub compute_capability_minor: u32,
    /// Logical graph nodes in the measured workload.
    pub graph_nodes: u64,
    /// Logical graph edges in the measured workload.
    pub graph_edges: u64,
    /// Number of repeated evaluations inside the timed region.
    pub repetitions: u64,
    /// Naive host-orchestrated execution time in nanoseconds.
    pub host_orchestrated_ns: f64,
    /// Resident megakernel execution time in nanoseconds.
    pub resident_megakernel_ns: f64,
    /// Setup time measured outside the timed region.
    pub setup_ns: f64,
    /// Graph uploads observed inside the timed region.
    pub timed_graph_uploads: u64,
    /// Host allocations observed inside the timed region.
    pub timed_host_allocations: u64,
    /// Host synchronization points observed inside the timed region.
    pub timed_host_syncs: u64,
    /// Resident borrowed escape-hatch dispatches observed inside the timed region.
    pub resident_borrowed_fallback_dispatches: u64,
}

/// Validated megakernel speedup proof.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CudaMegakernelSpeedupProof {
    /// Minimum observed speedup across accepted samples.
    pub min_speedup_x: f64,
    /// Maximum observed speedup across accepted samples.
    pub max_speedup_x: f64,
    /// Accepted sample count.
    pub sample_count: usize,
    /// Total repeated resident evaluations covered by the proof.
    pub total_repetitions: u64,
}

/// Failure reason for megakernel speedup release gating.
#[derive(Clone, Debug, PartialEq)]
pub enum CudaMegakernelSpeedupGateError {
    /// No samples were provided.
    EmptySamples,
    /// Required speedup threshold must be positive and finite.
    InvalidRequiredSpeedup {
        /// Caller-provided threshold.
        required_x: f64,
    },
    /// Evidence was not produced by the CUDA backend.
    NonCudaEvidence {
        /// Sample index.
        index: usize,
        /// Observed backend identifier.
        backend_id: String,
    },
    /// Evidence was produced on a device below the release evidence floor.
    InsufficientCudaDevice {
        /// Sample index.
        index: usize,
        /// Probed device memory in bytes.
        device_memory_bytes: u64,
        /// Probed compute capability major version.
        compute_capability_major: u32,
        /// Probed compute capability minor version.
        compute_capability_minor: u32,
    },
    /// Sample does not represent a repeated fixed-graph workload.
    NotRepeatedFixedGraph {
        /// Sample index.
        index: usize,
    },
    /// Sample is too small to prove release-scale megakernel behavior.
    InsufficientScale {
        /// Sample index.
        index: usize,
        /// Logical graph nodes in the sample.
        graph_nodes: u64,
        /// Logical graph edges in the sample.
        graph_edges: u64,
        /// Repetitions in the timed steady-state region.
        repetitions: u64,
    },
    /// Proof does not contain enough independent accepted benchmark samples.
    InsufficientSampleCount {
        /// Accepted sample count.
        sample_count: usize,
    },
    /// Timed region includes setup, graph upload, allocation, or host-sync pollution.
    PollutedTimedRegion {
        /// Sample index.
        index: usize,
        /// Graph uploads in timed region.
        graph_uploads: u64,
        /// Host allocations in timed region.
        host_allocations: u64,
        /// Host synchronization points in timed region.
        host_syncs: u64,
        /// Resident borrowed escape-hatch dispatches in timed region.
        borrowed_fallback_dispatches: u64,
    },
    /// Timings must be positive finite values.
    InvalidTiming {
        /// Sample index.
        index: usize,
    },
    /// Sample speedup is below release threshold.
    SpeedupBelowThreshold {
        /// Sample index.
        index: usize,
        /// Observed speedup.
        observed_x: f64,
        /// Required speedup.
        required_x: f64,
    },
    /// Total repeated evaluations exceeded the proof counter width.
    RepetitionCountOverflow {
        /// Sample index that overflowed the aggregate.
        index: usize,
    },
    /// Release evidence CSV header is missing or does not match the contract.
    InvalidEvidenceHeader {
        /// Header observed in the evidence artifact.
        observed: String,
    },
    /// Release evidence CSV row does not have the required field count.
    InvalidEvidenceRow {
        /// One-based line number in the evidence artifact.
        line: usize,
    },
    /// Release evidence CSV row contains a field that cannot be parsed.
    InvalidEvidenceValue {
        /// One-based line number in the evidence artifact.
        line: usize,
        /// Field name.
        field: &'static str,
        /// Raw field value.
        value: String,
    },
    /// Release evidence CSV capacity planning overflowed.
    EvidenceCsvCapacityOverflow,
    /// Release evidence CSV allocation failed.
    EvidenceCsvReserveFailed {
        /// Requested total byte capacity.
        requested: usize,
        /// Allocator failure text.
        message: String,
    },
}

impl std::fmt::Display for CudaMegakernelSpeedupGateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptySamples => write!(
                f,
                "CUDA megakernel speedup gate has no samples. Fix: record steady-state resident and host-orchestrated benchmark samples."
            ),
            Self::InvalidRequiredSpeedup { required_x } => write!(
                f,
                "CUDA megakernel speedup gate received invalid required speedup {required_x}. Fix: pass a positive finite release threshold."
            ),
            Self::NonCudaEvidence { index, backend_id } => write!(
                f,
                "CUDA megakernel speedup sample {index} was produced by backend `{backend_id}`. Fix: collect release speedup evidence through the CUDA backend, not a CPU/WGPU/comparison path."
            ),
            Self::InsufficientCudaDevice {
                index,
                device_memory_bytes,
                compute_capability_major,
                compute_capability_minor,
            } => write!(
                f,
                "CUDA megakernel speedup sample {index} was produced on an insufficient CUDA device: memory_bytes={device_memory_bytes}, compute_capability={compute_capability_major}.{compute_capability_minor}. Fix: collect release evidence on a CUDA GPU with at least {MIN_RELEASE_DEVICE_MEMORY_BYTES} bytes VRAM and compute capability {MIN_RELEASE_COMPUTE_CAPABILITY_MAJOR}.{MIN_RELEASE_COMPUTE_CAPABILITY_MINOR}."
            ),
            Self::NotRepeatedFixedGraph { index } => write!(
                f,
                "CUDA megakernel speedup sample {index} is not a repeated fixed-graph workload. Fix: use repetitions > 1 with non-empty graph nodes and edges."
            ),
            Self::InsufficientScale {
                index,
                graph_nodes,
                graph_edges,
                repetitions,
            } => write!(
                f,
                "CUDA megakernel speedup sample {index} is below release scale: nodes={graph_nodes}, edges={graph_edges}, repetitions={repetitions}. Fix: use at least {MIN_RELEASE_GRAPH_NODES} nodes, {MIN_RELEASE_GRAPH_EDGES} edges, and {MIN_RELEASE_REPETITIONS} steady-state repetitions."
            ),
            Self::InsufficientSampleCount { sample_count } => write!(
                f,
                "CUDA megakernel speedup proof has only {sample_count} accepted sample(s). Fix: provide at least {MIN_RELEASE_SAMPLE_COUNT} independent release-scale samples."
            ),
            Self::PollutedTimedRegion {
                index,
                graph_uploads,
                host_allocations,
                host_syncs,
                borrowed_fallback_dispatches,
            } => write!(
                f,
                "CUDA megakernel speedup sample {index} is polluted: graph_uploads={graph_uploads}, host_allocations={host_allocations}, host_syncs={host_syncs}, resident_borrowed_fallback_dispatches={borrowed_fallback_dispatches}. Fix: measure native steady-state CUDA execution only."
            ),
            Self::InvalidTiming { index } => write!(
                f,
                "CUDA megakernel speedup sample {index} has invalid timing. Fix: provide positive finite host and resident durations."
            ),
            Self::SpeedupBelowThreshold {
                index,
                observed_x,
                required_x,
            } => write!(
                f,
                "CUDA megakernel speedup sample {index} reached {observed_x:.2}x but release requires {required_x:.2}x. Fix: improve resident execution or lower host orchestration overhead before claiming the release speedup."
            ),
            Self::RepetitionCountOverflow { index } => write!(
                f,
                "CUDA megakernel speedup proof overflowed total repetitions at sample {index}. Fix: split the proof set into bounded release evidence groups."
            ),
            Self::InvalidEvidenceHeader { observed } => write!(
                f,
                "CUDA megakernel speedup evidence has invalid CSV header `{observed}`. Fix: emit the exact release header `{MEGAKERNEL_SPEEDUP_EVIDENCE_CSV_HEADER}`."
            ),
            Self::InvalidEvidenceRow { line } => write!(
                f,
                "CUDA megakernel speedup evidence line {line} has the wrong field count. Fix: emit one complete CSV row per benchmark sample."
            ),
            Self::InvalidEvidenceValue { line, field, value } => write!(
                f,
                "CUDA megakernel speedup evidence line {line} has invalid `{field}` value `{value}`. Fix: emit finite numeric benchmark fields without separators or units."
            ),
            Self::EvidenceCsvCapacityOverflow => write!(
                f,
                "CUDA megakernel speedup evidence CSV capacity overflowed usize. Fix: split release evidence into bounded proof artifacts."
            ),
            Self::EvidenceCsvReserveFailed { requested, message } => write!(
                f,
                "CUDA megakernel speedup evidence CSV could not reserve {requested} byte(s): {message}. Fix: split release evidence into bounded proof artifacts."
            ),
        }
    }
}

impl std::error::Error for CudaMegakernelSpeedupGateError {}

/// CSV header required for CUDA megakernel release speedup evidence.
pub const MEGAKERNEL_SPEEDUP_EVIDENCE_CSV_HEADER: &str = "backend_id,device_ordinal,device_memory_bytes,compute_capability_major,compute_capability_minor,graph_nodes,graph_edges,repetitions,host_orchestrated_ns,resident_megakernel_ns,setup_ns,timed_graph_uploads,timed_host_allocations,timed_host_syncs,resident_borrowed_fallback_dispatches";

/// Validate measured samples and emit the exact CSV artifact accepted by the
/// CUDA megakernel release verifier.
pub fn format_validated_cuda_megakernel_speedup_evidence_csv(
    samples: &[CudaMegakernelSpeedupSample],
    required_speedup_x: f64,
) -> Result<(CudaMegakernelSpeedupProof, String), CudaMegakernelSpeedupGateError> {
    let proof = validate_cuda_megakernel_speedup_gate(samples, required_speedup_x)?;
    let capacity = megakernel_speedup_evidence_csv_capacity(samples.len())?;
    let mut csv = String::new();
    vyre_foundation::allocation::try_reserve_string_to_capacity(&mut csv, capacity).map_err(
        |error| CudaMegakernelSpeedupGateError::EvidenceCsvReserveFailed {
            requested: capacity,
            message: error.to_string(),
        },
    )?;
    csv.push_str(MEGAKERNEL_SPEEDUP_EVIDENCE_CSV_HEADER);
    csv.push('\n');
    for sample in samples {
        use std::fmt::Write as _;
        writeln!(
            csv,
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
            sample.backend_id,
            sample.device_ordinal,
            sample.device_memory_bytes,
            sample.compute_capability_major,
            sample.compute_capability_minor,
            sample.graph_nodes,
            sample.graph_edges,
            sample.repetitions,
            sample.host_orchestrated_ns,
            sample.resident_megakernel_ns,
            sample.setup_ns,
            sample.timed_graph_uploads,
            sample.timed_host_allocations,
            sample.timed_host_syncs,
            sample.resident_borrowed_fallback_dispatches
        )
        .map_err(|_| CudaMegakernelSpeedupGateError::InvalidEvidenceValue {
            line: 0,
            field: "csv_string_write",
            value: "fmt::Error".to_string(),
        })?;
    }
    Ok((proof, csv))
}

fn megakernel_speedup_evidence_csv_capacity(
    sample_count: usize,
) -> Result<usize, CudaMegakernelSpeedupGateError> {
    let sample_bytes = sample_count
        .checked_mul(128)
        .ok_or(CudaMegakernelSpeedupGateError::EvidenceCsvCapacityOverflow)?;
    MEGAKERNEL_SPEEDUP_EVIDENCE_CSV_HEADER
        .len()
        .checked_add(1)
        .and_then(|header| header.checked_add(sample_bytes))
        .ok_or(CudaMegakernelSpeedupGateError::EvidenceCsvCapacityOverflow)
}

/// Validate steady-state megakernel speedup samples against a release threshold.
pub fn validate_cuda_megakernel_speedup_gate(
    samples: &[CudaMegakernelSpeedupSample],
    required_speedup_x: f64,
) -> Result<CudaMegakernelSpeedupProof, CudaMegakernelSpeedupGateError> {
    if samples.is_empty() {
        return Err(CudaMegakernelSpeedupGateError::EmptySamples);
    }
    if !required_speedup_x.is_finite() || required_speedup_x <= 0.0 {
        return Err(CudaMegakernelSpeedupGateError::InvalidRequiredSpeedup {
            required_x: required_speedup_x,
        });
    }

    let mut min_speedup_x = f64::INFINITY;
    let mut max_speedup_x = 0.0_f64;
    let mut total_repetitions = 0_u64;
    let mut sample_count = 0_usize;

    for (index, sample) in samples.iter().copied().enumerate() {
        accumulate_cuda_megakernel_speedup_sample(
            sample,
            index,
            required_speedup_x,
            &mut min_speedup_x,
            &mut max_speedup_x,
            &mut total_repetitions,
        )?;
        sample_count += 1;
    }

    if sample_count == 0 {
        return Err(CudaMegakernelSpeedupGateError::EmptySamples);
    }
    if sample_count < MIN_RELEASE_SAMPLE_COUNT {
        return Err(CudaMegakernelSpeedupGateError::InsufficientSampleCount { sample_count });
    }

    Ok(CudaMegakernelSpeedupProof {
        min_speedup_x,
        max_speedup_x,
        sample_count,
        total_repetitions,
    })
}

/// Parse and validate a CUDA megakernel speedup release evidence CSV artifact.
///
/// The artifact intentionally uses a strict, unit-suffixed-free CSV contract so
/// shell benchmark runners can emit it without JSON/TOML dependencies and CI can
/// fail release claims mechanically.
pub fn validate_cuda_megakernel_speedup_evidence_csv(
    csv: &str,
    required_speedup_x: f64,
) -> Result<CudaMegakernelSpeedupProof, CudaMegakernelSpeedupGateError> {
    let mut lines = csv.lines().filter(|line| !line.trim().is_empty());
    let header = lines
        .next()
        .ok_or(CudaMegakernelSpeedupGateError::EmptySamples)?;
    if header.trim() != MEGAKERNEL_SPEEDUP_EVIDENCE_CSV_HEADER {
        return Err(CudaMegakernelSpeedupGateError::InvalidEvidenceHeader {
            observed: header.to_string(),
        });
    }

    let mut min_speedup_x = f64::INFINITY;
    let mut max_speedup_x = 0.0_f64;
    let mut total_repetitions = 0_u64;
    let mut sample_count = 0_usize;
    for (line_index, line) in lines.enumerate() {
        let line_number = line_index + 2;
        let mut fields = [""; 15];
        let mut field_count = 0_usize;
        for (field_index, field) in line.split(',').map(str::trim).enumerate() {
            if field_index >= fields.len() {
                return Err(CudaMegakernelSpeedupGateError::InvalidEvidenceRow {
                    line: line_number,
                });
            }
            fields[field_index] = field;
            field_count += 1;
        }
        if field_count != fields.len() {
            return Err(CudaMegakernelSpeedupGateError::InvalidEvidenceRow { line: line_number });
        }
        let backend_id = parse_backend_id_field(fields[0], line_number)?;
        let sample = CudaMegakernelSpeedupSample {
            backend_id,
            device_ordinal: parse_u64_field(fields[1], line_number, "device_ordinal")?,
            device_memory_bytes: parse_u64_field(fields[2], line_number, "device_memory_bytes")?,
            compute_capability_major: parse_u32_field(
                fields[3],
                line_number,
                "compute_capability_major",
            )?,
            compute_capability_minor: parse_u32_field(
                fields[4],
                line_number,
                "compute_capability_minor",
            )?,
            graph_nodes: parse_u64_field(fields[5], line_number, "graph_nodes")?,
            graph_edges: parse_u64_field(fields[6], line_number, "graph_edges")?,
            repetitions: parse_u64_field(fields[7], line_number, "repetitions")?,
            host_orchestrated_ns: parse_f64_field(fields[8], line_number, "host_orchestrated_ns")?,
            resident_megakernel_ns: parse_f64_field(
                fields[9],
                line_number,
                "resident_megakernel_ns",
            )?,
            setup_ns: parse_f64_field(fields[10], line_number, "setup_ns")?,
            timed_graph_uploads: parse_u64_field(fields[11], line_number, "timed_graph_uploads")?,
            timed_host_allocations: parse_u64_field(
                fields[12],
                line_number,
                "timed_host_allocations",
            )?,
            timed_host_syncs: parse_u64_field(fields[13], line_number, "timed_host_syncs")?,
            resident_borrowed_fallback_dispatches: parse_u64_field(
                fields[14],
                line_number,
                "resident_borrowed_fallback_dispatches",
            )?,
        };
        accumulate_cuda_megakernel_speedup_sample(
            sample,
            sample_count,
            required_speedup_x,
            &mut min_speedup_x,
            &mut max_speedup_x,
            &mut total_repetitions,
        )?;
        sample_count += 1;
    }
    if sample_count < MIN_RELEASE_SAMPLE_COUNT {
        return Err(CudaMegakernelSpeedupGateError::InsufficientSampleCount { sample_count });
    }
    Ok(CudaMegakernelSpeedupProof {
        min_speedup_x,
        max_speedup_x,
        sample_count,
        total_repetitions,
    })
}

fn accumulate_cuda_megakernel_speedup_sample(
    sample: CudaMegakernelSpeedupSample,
    index: usize,
    required_speedup_x: f64,
    min_speedup_x: &mut f64,
    max_speedup_x: &mut f64,
    total_repetitions: &mut u64,
) -> Result<(), CudaMegakernelSpeedupGateError> {
    if sample.backend_id != crate::CUDA_BACKEND_ID {
        return Err(CudaMegakernelSpeedupGateError::NonCudaEvidence {
            index,
            backend_id: sample.backend_id.to_string(),
        });
    }
    if sample.device_memory_bytes < MIN_RELEASE_DEVICE_MEMORY_BYTES
        || (
            sample.compute_capability_major,
            sample.compute_capability_minor,
        ) < (
            MIN_RELEASE_COMPUTE_CAPABILITY_MAJOR,
            MIN_RELEASE_COMPUTE_CAPABILITY_MINOR,
        )
    {
        return Err(CudaMegakernelSpeedupGateError::InsufficientCudaDevice {
            index,
            device_memory_bytes: sample.device_memory_bytes,
            compute_capability_major: sample.compute_capability_major,
            compute_capability_minor: sample.compute_capability_minor,
        });
    }
    if sample.graph_nodes == 0 || sample.graph_edges == 0 || sample.repetitions <= 1 {
        return Err(CudaMegakernelSpeedupGateError::NotRepeatedFixedGraph { index });
    }
    if sample.graph_nodes < MIN_RELEASE_GRAPH_NODES
        || sample.graph_edges < MIN_RELEASE_GRAPH_EDGES
        || sample.repetitions < MIN_RELEASE_REPETITIONS
    {
        return Err(CudaMegakernelSpeedupGateError::InsufficientScale {
            index,
            graph_nodes: sample.graph_nodes,
            graph_edges: sample.graph_edges,
            repetitions: sample.repetitions,
        });
    }
    if sample.timed_graph_uploads != 0
        || sample.timed_host_allocations != 0
        || sample.timed_host_syncs != 0
        || sample.resident_borrowed_fallback_dispatches != 0
    {
        return Err(CudaMegakernelSpeedupGateError::PollutedTimedRegion {
            index,
            graph_uploads: sample.timed_graph_uploads,
            host_allocations: sample.timed_host_allocations,
            host_syncs: sample.timed_host_syncs,
            borrowed_fallback_dispatches: sample.resident_borrowed_fallback_dispatches,
        });
    }
    if !sample.host_orchestrated_ns.is_finite()
        || !sample.resident_megakernel_ns.is_finite()
        || sample.host_orchestrated_ns <= 0.0
        || sample.resident_megakernel_ns <= 0.0
        || !sample.setup_ns.is_finite()
        || sample.setup_ns < 0.0
    {
        return Err(CudaMegakernelSpeedupGateError::InvalidTiming { index });
    }
    let speedup_x = sample.host_orchestrated_ns / sample.resident_megakernel_ns;
    if speedup_x < required_speedup_x {
        return Err(CudaMegakernelSpeedupGateError::SpeedupBelowThreshold {
            index,
            observed_x: speedup_x,
            required_x: required_speedup_x,
        });
    }
    *min_speedup_x = (*min_speedup_x).min(speedup_x);
    *max_speedup_x = (*max_speedup_x).max(speedup_x);
    *total_repetitions = total_repetitions
        .checked_add(sample.repetitions)
        .ok_or(CudaMegakernelSpeedupGateError::RepetitionCountOverflow { index })?;
    Ok(())
}

fn parse_backend_id_field(
    value: &str,
    line: usize,
) -> Result<&'static str, CudaMegakernelSpeedupGateError> {
    if value == crate::CUDA_BACKEND_ID {
        Ok(crate::CUDA_BACKEND_ID)
    } else {
        Err(CudaMegakernelSpeedupGateError::InvalidEvidenceValue {
            line,
            field: "backend_id",
            value: value.to_string(),
        })
    }
}

fn parse_u64_field(
    value: &str,
    line: usize,
    field: &'static str,
) -> Result<u64, CudaMegakernelSpeedupGateError> {
    value
        .parse()
        .map_err(|_| CudaMegakernelSpeedupGateError::InvalidEvidenceValue {
            line,
            field,
            value: value.to_string(),
        })
}

fn parse_u32_field(
    value: &str,
    line: usize,
    field: &'static str,
) -> Result<u32, CudaMegakernelSpeedupGateError> {
    value
        .parse()
        .map_err(|_| CudaMegakernelSpeedupGateError::InvalidEvidenceValue {
            line,
            field,
            value: value.to_string(),
        })
}

fn parse_f64_field(
    value: &str,
    line: usize,
    field: &'static str,
) -> Result<f64, CudaMegakernelSpeedupGateError> {
    value
        .parse()
        .map_err(|_| CudaMegakernelSpeedupGateError::InvalidEvidenceValue {
            line,
            field,
            value: value.to_string(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speedup_gate_accepts_unpolluted_steady_state_100x_samples() {
        let proof = validate_cuda_megakernel_speedup_gate(
            &[
                sample(1_000_000.0, 10_000.0, 64),
                sample(2_500_000.0, 20_000.0, 128),
            ],
            100.0,
        )
        .expect("Fix: unpolluted 100x samples should pass release gate");

        assert_eq!(proof.sample_count, 2);
        assert_eq!(proof.total_repetitions, 192);
        assert_eq!(proof.min_speedup_x, 100.0);
        assert_eq!(proof.max_speedup_x, 125.0);
    }

    #[test]
    fn speedup_gate_validates_release_evidence_csv_artifact() {
        let csv = format!(
            "{MEGAKERNEL_SPEEDUP_EVIDENCE_CSV_HEADER}\n\
             cuda,0,34359738368,12,0,10000,80000,64,1000000,10000,250000,0,0,0,0\n\
             cuda,0,34359738368,12,0,20000,160000,128,2500000,20000,350000,0,0,0,0\n"
        );

        let proof = validate_cuda_megakernel_speedup_evidence_csv(&csv, 100.0)
            .expect("Fix: release evidence CSV should validate");

        assert_eq!(proof.sample_count, 2);
        assert_eq!(proof.total_repetitions, 192);
        assert_eq!(proof.min_speedup_x, 100.0);
        assert_eq!(proof.max_speedup_x, 125.0);
    }

    #[test]
    fn speedup_gate_formats_validated_release_evidence_csv_artifact() {
        let samples = [
            sample(1_000_000.0, 10_000.0, 64),
            sample(2_500_000.0, 20_000.0, 128),
        ];
        let (proof, csv) = format_validated_cuda_megakernel_speedup_evidence_csv(&samples, 100.0)
            .expect("Fix: validated release samples should format as verifier CSV");
        let reparsed = validate_cuda_megakernel_speedup_evidence_csv(&csv, 100.0)
            .expect("Fix: formatted release CSV should roundtrip through the verifier");

        assert_eq!(proof, reparsed);
        assert!(csv.starts_with(MEGAKERNEL_SPEEDUP_EVIDENCE_CSV_HEADER));
        assert_eq!(csv.lines().count(), 3);
    }

    #[test]
    fn speedup_gate_csv_capacity_planning_is_checked() {
        let capacity = megakernel_speedup_evidence_csv_capacity(2)
            .expect("Fix: two release samples should have bounded CSV capacity");
        assert_eq!(
            capacity,
            MEGAKERNEL_SPEEDUP_EVIDENCE_CSV_HEADER.len() + 1 + 256
        );

        assert_eq!(
            megakernel_speedup_evidence_csv_capacity(usize::MAX / 128 + 1)
                .expect_err("oversized proof artifacts must fail before allocation"),
            CudaMegakernelSpeedupGateError::EvidenceCsvCapacityOverflow
        );
    }

    #[test]
    fn speedup_gate_rejects_malformed_release_evidence_csv() {
        assert_eq!(
            validate_cuda_megakernel_speedup_evidence_csv("nodes,edges\n", 100.0)
                .expect_err("wrong header must fail"),
            CudaMegakernelSpeedupGateError::InvalidEvidenceHeader {
                observed: "nodes,edges".to_string(),
            }
        );

        let missing_field = format!(
            "{MEGAKERNEL_SPEEDUP_EVIDENCE_CSV_HEADER}\n\
             cuda,0,34359738368,12,0,10000,80000,64,1000000,10000,250000,0,0,0\n"
        );
        assert_eq!(
            validate_cuda_megakernel_speedup_evidence_csv(&missing_field, 100.0)
                .expect_err("wrong row width must fail"),
            CudaMegakernelSpeedupGateError::InvalidEvidenceRow { line: 2 }
        );

        let bad_number = format!(
            "{MEGAKERNEL_SPEEDUP_EVIDENCE_CSV_HEADER}\n\
             cuda,0,34359738368,12,0,10000,80000,many,1000000,10000,250000,0,0,0,0\n"
        );
        assert_eq!(
            validate_cuda_megakernel_speedup_evidence_csv(&bad_number, 100.0)
                .expect_err("bad numeric field must fail"),
            CudaMegakernelSpeedupGateError::InvalidEvidenceValue {
                line: 2,
                field: "repetitions",
                value: "many".to_string(),
            }
        );

        let wrong_backend = format!(
            "{MEGAKERNEL_SPEEDUP_EVIDENCE_CSV_HEADER}\n\
             wgpu,0,34359738368,12,0,10000,80000,64,1000000,10000,250000,0,0,0,0\n"
        );
        assert_eq!(
            validate_cuda_megakernel_speedup_evidence_csv(&wrong_backend, 100.0)
                .expect_err("non-CUDA evidence must fail at the artifact boundary"),
            CudaMegakernelSpeedupGateError::InvalidEvidenceValue {
                line: 2,
                field: "backend_id",
                value: "wgpu".to_string(),
            }
        );
    }

    #[test]
    fn speedup_gate_rejects_non_cuda_or_too_small_device_samples() {
        let mut wrong_backend = sample(1_000_000.0, 1_000.0, 64);
        wrong_backend.backend_id = "wgpu";
        assert_eq!(
            validate_cuda_megakernel_speedup_gate(&[wrong_backend], 100.0)
                .expect_err("release samples must be CUDA-produced"),
            CudaMegakernelSpeedupGateError::NonCudaEvidence {
                index: 0,
                backend_id: "wgpu".to_string(),
            }
        );

        let mut too_small = sample(1_000_000.0, 1_000.0, 64);
        too_small.device_memory_bytes = 8 * 1024 * 1024 * 1024;
        too_small.compute_capability_major = 7;
        too_small.compute_capability_minor = 5;
        assert_eq!(
            validate_cuda_megakernel_speedup_gate(&[too_small], 100.0)
                .expect_err("release samples must prove CUDA hardware floor"),
            CudaMegakernelSpeedupGateError::InsufficientCudaDevice {
                index: 0,
                device_memory_bytes: 8 * 1024 * 1024 * 1024,
                compute_capability_major: 7,
                compute_capability_minor: 5,
            }
        );
    }

    #[test]
    fn speedup_gate_rejects_setup_pollution() {
        let mut polluted = sample(1_000_000.0, 1_000.0, 64);
        polluted.timed_graph_uploads = 1;

        assert_eq!(
            validate_cuda_megakernel_speedup_gate(&[polluted], 100.0)
                .expect_err("setup pollution must fail"),
            CudaMegakernelSpeedupGateError::PollutedTimedRegion {
                index: 0,
                graph_uploads: 1,
                host_allocations: 0,
                host_syncs: 0,
                borrowed_fallback_dispatches: 0,
            }
        );

        let mut borrowed_fallback = sample(1_000_000.0, 1_000.0, 64);
        borrowed_fallback.resident_borrowed_fallback_dispatches = 1;
        assert_eq!(
            validate_cuda_megakernel_speedup_gate(&[borrowed_fallback], 100.0)
                .expect_err("borrowed resident escape-hatch dispatches must fail release evidence"),
            CudaMegakernelSpeedupGateError::PollutedTimedRegion {
                index: 0,
                graph_uploads: 0,
                host_allocations: 0,
                host_syncs: 0,
                borrowed_fallback_dispatches: 1,
            }
        );
    }

    #[test]
    fn speedup_gate_rejects_below_threshold_and_non_repeated_samples() {
        assert_eq!(
            validate_cuda_megakernel_speedup_gate(&[sample(50_000.0, 1_000.0, 64)], 100.0)
                .expect_err("below threshold must fail"),
            CudaMegakernelSpeedupGateError::SpeedupBelowThreshold {
                index: 0,
                observed_x: 50.0,
                required_x: 100.0,
            }
        );

        assert_eq!(
            validate_cuda_megakernel_speedup_gate(&[sample(1_000_000.0, 1_000.0, 1)], 100.0)
                .expect_err("single-run sample must fail"),
            CudaMegakernelSpeedupGateError::NotRepeatedFixedGraph { index: 0 }
        );
    }

    #[test]
    fn speedup_gate_rejects_toy_workloads_that_cannot_prove_release_scale() {
        let mut tiny = sample(1_000_000.0, 1_000.0, 64);
        tiny.graph_nodes = 128;
        tiny.graph_edges = 512;

        assert_eq!(
            validate_cuda_megakernel_speedup_gate(&[tiny], 100.0)
                .expect_err("toy workload must not pass the release speedup gate"),
            CudaMegakernelSpeedupGateError::InsufficientScale {
                index: 0,
                graph_nodes: 128,
                graph_edges: 512,
                repetitions: 64,
            }
        );

        assert_eq!(
            validate_cuda_megakernel_speedup_gate(&[sample(1_000_000.0, 1_000.0, 8)], 100.0)
                .expect_err("too few repetitions must not pass the release speedup gate"),
            CudaMegakernelSpeedupGateError::InsufficientScale {
                index: 0,
                graph_nodes: MIN_RELEASE_GRAPH_NODES,
                graph_edges: MIN_RELEASE_GRAPH_EDGES,
                repetitions: 8,
            }
        );
    }

    #[test]
    fn speedup_gate_rejects_single_sample_release_proofs() {
        assert_eq!(
            validate_cuda_megakernel_speedup_gate(&[sample(1_000_000.0, 10_000.0, 64)], 100.0)
                .expect_err("single-sample release proof must fail"),
            CudaMegakernelSpeedupGateError::InsufficientSampleCount { sample_count: 1 }
        );
    }

    #[test]
    fn speedup_gate_rejects_invalid_thresholds_timings_and_repetition_overflow() {
        let threshold_error =
            validate_cuda_megakernel_speedup_gate(&[sample(1_000_000.0, 1_000.0, 64)], f64::NAN)
                .expect_err("NaN release threshold must fail");
        assert!(matches!(
            threshold_error,
            CudaMegakernelSpeedupGateError::InvalidRequiredSpeedup { required_x }
                if required_x.is_nan()
        ));

        let mut invalid_setup = sample(1_000_000.0, 1_000.0, 64);
        invalid_setup.setup_ns = f64::NAN;
        assert_eq!(
            validate_cuda_megakernel_speedup_gate(&[invalid_setup], 100.0)
                .expect_err("NaN setup timing must fail"),
            CudaMegakernelSpeedupGateError::InvalidTiming { index: 0 }
        );

        assert_eq!(
            validate_cuda_megakernel_speedup_gate(
                &[
                    sample(1_000_000.0, 1_000.0, u64::MAX),
                    sample(1_000_000.0, 1_000.0, 64),
                ],
                100.0,
            )
            .expect_err("proof repetition count overflow must fail loudly"),
            CudaMegakernelSpeedupGateError::RepetitionCountOverflow { index: 1 }
        );
    }

    fn sample(
        host_orchestrated_ns: f64,
        resident_megakernel_ns: f64,
        repetitions: u64,
    ) -> CudaMegakernelSpeedupSample {
        CudaMegakernelSpeedupSample {
            backend_id: crate::CUDA_BACKEND_ID,
            device_ordinal: 0,
            device_memory_bytes: 32 * 1024 * 1024 * 1024,
            compute_capability_major: 12,
            compute_capability_minor: 0,
            graph_nodes: 10_000,
            graph_edges: 80_000,
            repetitions,
            host_orchestrated_ns,
            resident_megakernel_ns,
            setup_ns: 250_000.0,
            timed_graph_uploads: 0,
            timed_host_allocations: 0,
            timed_host_syncs: 0,
            resident_borrowed_fallback_dispatches: 0,
        }
    }
}
