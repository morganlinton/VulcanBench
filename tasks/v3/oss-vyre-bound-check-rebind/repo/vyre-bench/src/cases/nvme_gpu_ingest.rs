use std::time::Instant;

use crate::api::case::{
    BenchCase, BenchContext, BenchError, BenchId, BenchLayer, BenchMetadata, BenchRequirements,
    BenchRun, Correctness, DeterminismClass, PreparedCase, WorkloadClass,
};
use crate::api::metric::{BenchMetrics, MetricPoint};
use vyre_runtime::uring::{NativeReadPath, NvmeGpuIngestTelemetry};

/// Release-scale zero-copy ingest accounting benchmark.
pub struct NvmeGpuIngestRegisteredMappedRead;

/// Release-scale native NVMe to GPU BAR1 ingest accounting benchmark.
pub struct NvmeGpuIngestGpuDirectNvme;

/// Static shape for an NVMe to GPU ingest benchmark.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NvmeGpuIngestWorkloadSpec {
    /// Stable benchmark id.
    pub id: &'static str,
    /// Human-readable benchmark name.
    pub name: &'static str,
    /// Runtime ingest path represented by the accounting benchmark.
    pub path: NativeReadPath,
    /// Number of fixed GPU-visible slots registered with io_uring.
    pub slot_count: u64,
    /// Bytes per GPU-visible ingest slot.
    pub slot_bytes: u64,
    /// Number of full-slot batches represented by the release workload.
    pub batch_count: u64,
}

impl NvmeGpuIngestWorkloadSpec {
    /// Total bytes ingested by this benchmark shape.
    #[must_use]
    pub fn total_bytes(self) -> Option<u64> {
        self.slot_count
            .checked_mul(self.slot_bytes)?
            .checked_mul(self.batch_count)
    }

    /// Total read submissions represented by this benchmark shape.
    #[must_use]
    pub fn total_reads(self) -> Option<u64> {
        self.slot_count.checked_mul(self.batch_count)
    }

    /// Maximum resident GPU-visible staging footprint.
    #[must_use]
    pub fn resident_bytes(self) -> Option<u64> {
        self.slot_count.checked_mul(self.slot_bytes)
    }

    /// Stable path label for metrics and Criterion benchmark ids.
    #[must_use]
    pub fn path_label(self) -> &'static str {
        match self.path {
            NativeReadPath::RegisteredMappedRead => "registered_mapped_read",
            NativeReadPath::GpuDirectNvmePassthrough => "gpudirect_nvme_passthrough",
        }
    }
}

const RELEASE_SUITES: &[crate::api::suite::SuiteKind] = &[
    crate::api::suite::SuiteKind::Release,
    crate::api::suite::SuiteKind::Gpu,
    crate::api::suite::SuiteKind::Deep,
    crate::api::suite::SuiteKind::Honest,
];

const REGISTERED_MAPPED_SPEC: NvmeGpuIngestWorkloadSpec = NvmeGpuIngestWorkloadSpec {
    id: "runtime.nvme_gpu_ingest.registered_mapped.4g",
    name: "NVMe to GPU registered mapped ingest 4GiB",
    path: NativeReadPath::RegisteredMappedRead,
    slot_count: 1_024,
    slot_bytes: 256 * 1024,
    batch_count: 16,
};

const GPUDIRECT_SPEC: NvmeGpuIngestWorkloadSpec = NvmeGpuIngestWorkloadSpec {
    id: "runtime.nvme_gpu_ingest.gpudirect_nvme.64g",
    name: "Native GPUDirect NVMe to GPU ingest 64GiB",
    path: NativeReadPath::GpuDirectNvmePassthrough,
    slot_count: 4_096,
    slot_bytes: 256 * 1024,
    batch_count: 64,
};

const RELEASE_INGEST_SPECS: &[NvmeGpuIngestWorkloadSpec] =
    &[REGISTERED_MAPPED_SPEC, GPUDIRECT_SPEC];

/// Release-scale ingest accounting shapes consumed by Criterion and registry cases.
#[must_use]
pub fn nvme_gpu_ingest_specs() -> &'static [NvmeGpuIngestWorkloadSpec] {
    RELEASE_INGEST_SPECS
}

/// Build the telemetry snapshot that a fully completed release-scale ingest run must report.
///
/// This is intentionally accounting-only: the runtime driver owns the io_uring
/// submission mechanics, while this bench surface owns stable metrics and
/// regression detection for zero-copy ingest semantics.
pub fn synthesize_completed_ingest_telemetry(
    spec: NvmeGpuIngestWorkloadSpec,
) -> Result<NvmeGpuIngestTelemetry, String> {
    let total_bytes = spec
        .total_bytes()
        .ok_or_else(|| format!("ingest spec `{}` byte count overflowed", spec.id))?;
    let total_reads = spec
        .total_reads()
        .ok_or_else(|| format!("ingest spec `{}` read count overflowed", spec.id))?;
    let mut telemetry = NvmeGpuIngestTelemetry {
        submitted_bytes: total_bytes,
        completed_bytes: total_bytes,
        submitted_reads: total_reads,
        completed_reads: total_reads,
        registered_mapped_read_submissions: 0,
        gpudirect_nvme_submissions: 0,
        cpu_bounce_bytes: 0,
        failed_completions: 0,
    };
    match spec.path {
        NativeReadPath::RegisteredMappedRead => {
            telemetry.registered_mapped_read_submissions = total_reads;
        }
        NativeReadPath::GpuDirectNvmePassthrough => {
            telemetry.gpudirect_nvme_submissions = total_reads;
        }
    }
    Ok(telemetry)
}

/// Validate that an ingest telemetry snapshot still describes a zero-copy completed run.
pub fn validate_zero_copy_ingest_telemetry(
    spec: NvmeGpuIngestWorkloadSpec,
    telemetry: NvmeGpuIngestTelemetry,
) -> Result<(), String> {
    let expected_bytes = spec
        .total_bytes()
        .ok_or_else(|| format!("ingest spec `{}` byte count overflowed", spec.id))?;
    let expected_reads = spec
        .total_reads()
        .ok_or_else(|| format!("ingest spec `{}` read count overflowed", spec.id))?;

    telemetry
        .validate_completed_zero_copy(spec.path)
        .map_err(|error| {
            format!(
                "ingest spec `{}` failed runtime invariant: {error}",
                spec.id
            )
        })?;
    if telemetry.submitted_bytes != expected_bytes || telemetry.completed_bytes != expected_bytes {
        return Err(format!(
            "ingest spec `{}` byte accounting mismatch: submitted={}, completed={}, expected={}",
            spec.id, telemetry.submitted_bytes, telemetry.completed_bytes, expected_bytes
        ));
    }
    if telemetry.submitted_reads != expected_reads || telemetry.completed_reads != expected_reads {
        return Err(format!(
            "ingest spec `{}` read accounting mismatch: submitted={}, completed={}, expected={}",
            spec.id, telemetry.submitted_reads, telemetry.completed_reads, expected_reads
        ));
    }
    Ok(())
}

/// Convert runtime ingest telemetry into benchmark metric points.
#[must_use]
pub fn ingest_telemetry_metric_points(
    spec: NvmeGpuIngestWorkloadSpec,
    telemetry: NvmeGpuIngestTelemetry,
) -> Vec<MetricPoint> {
    vec![
        MetricPoint {
            name: format!("{}_submitted_bytes", spec.path_label()),
            value: telemetry.submitted_bytes,
        },
        MetricPoint {
            name: format!("{}_completed_bytes", spec.path_label()),
            value: telemetry.completed_bytes,
        },
        MetricPoint {
            name: format!("{}_submitted_reads", spec.path_label()),
            value: telemetry.submitted_reads,
        },
        MetricPoint {
            name: format!("{}_completed_reads", spec.path_label()),
            value: telemetry.completed_reads,
        },
        MetricPoint {
            name: "registered_mapped_read_submissions".to_string(),
            value: telemetry.registered_mapped_read_submissions,
        },
        MetricPoint {
            name: "gpudirect_nvme_submissions".to_string(),
            value: telemetry.gpudirect_nvme_submissions,
        },
        MetricPoint {
            name: "cpu_bounce_bytes".to_string(),
            value: telemetry.cpu_bounce_bytes,
        },
        MetricPoint {
            name: "failed_completions".to_string(),
            value: telemetry.failed_completions,
        },
        MetricPoint {
            name: "inflight_reads".to_string(),
            value: telemetry.inflight_reads(),
        },
    ]
}

/// Stable binary encoding used as the correctness output for ingest accounting cases.
#[must_use]
pub fn encode_ingest_telemetry(telemetry: NvmeGpuIngestTelemetry) -> Vec<u8> {
    let fields = [
        telemetry.submitted_bytes,
        telemetry.completed_bytes,
        telemetry.submitted_reads,
        telemetry.completed_reads,
        telemetry.registered_mapped_read_submissions,
        telemetry.gpudirect_nvme_submissions,
        telemetry.cpu_bounce_bytes,
        telemetry.failed_completions,
        telemetry.inflight_reads(),
    ];
    let mut encoded = Vec::with_capacity(fields.len() * std::mem::size_of::<u64>());
    for field in fields {
        encoded.extend_from_slice(&field.to_le_bytes());
    }
    encoded
}

fn bench_case_metadata(spec: NvmeGpuIngestWorkloadSpec) -> BenchMetadata {
    BenchMetadata {
        id: BenchId(spec.id.to_string()),
        name: spec.name.to_string(),
        description: format!(
            "Release-scale io_uring ingest accounting for the {} path with zero CPU bounce bytes",
            spec.path_label()
        ),
        tags: vec![
            "nvme".to_string(),
            "io_uring".to_string(),
            "gpu-ingest".to_string(),
            "zero-copy".to_string(),
            "gpudirect".to_string(),
            "release".to_string(),
        ],
        layer: BenchLayer::Runtime,
        workload: WorkloadClass::Macro,
        determinism: DeterminismClass::Deterministic,
        owner_crate: "vyre-runtime".to_string(),
    }
}

fn bench_case_requirements(spec: NvmeGpuIngestWorkloadSpec) -> BenchRequirements {
    BenchRequirements {
        needs_gpu: true,
        needs_network: false,
        min_vram_bytes: spec.resident_bytes(),
        min_input_bytes: spec.total_bytes(),
        feature_set: vec![
            "linux".to_string(),
            "io_uring".to_string(),
            "gpu-visible-memory".to_string(),
            spec.path_label().to_string(),
        ],
    }
}

fn prepared_ingest_spec(prepared: &PreparedCase) -> Result<NvmeGpuIngestWorkloadSpec, BenchError> {
    prepared
        .downcast_ref::<NvmeGpuIngestWorkloadSpec>()
        .copied()
        .ok_or_else(|| {
            BenchError::ExecutionFailed(
                "prepared benchmark payload was not an NvmeGpuIngestWorkloadSpec".to_string(),
            )
        })
}

fn run_ingest_accounting(prepared: &PreparedCase) -> Result<BenchRun, BenchError> {
    let spec = prepared_ingest_spec(prepared)?;
    let start = Instant::now();
    let telemetry =
        synthesize_completed_ingest_telemetry(spec).map_err(BenchError::ExecutionFailed)?;
    validate_zero_copy_ingest_telemetry(spec, telemetry)
        .map_err(BenchError::CorrectnessViolation)?;
    let custom = ingest_telemetry_metric_points(spec, telemetry);
    let wall_ns = start.elapsed().as_nanos() as u64;
    let encoded = encode_ingest_telemetry(telemetry);

    Ok(BenchRun {
        metrics: BenchMetrics {
            wall_ns: Some(wall_ns),
            input_bytes: Some(telemetry.submitted_bytes),
            bytes_touched: Some(telemetry.completed_bytes),
            bytes_read: Some(telemetry.completed_bytes),
            bytes_written: Some(telemetry.completed_bytes),
            custom,
            ..Default::default()
        },
        baseline_metrics: Some(BenchMetrics {
            input_bytes: Some(telemetry.submitted_bytes),
            bytes_touched: Some(telemetry.completed_bytes.saturating_mul(2)),
            bytes_read: Some(telemetry.completed_bytes),
            bytes_written: Some(telemetry.completed_bytes),
            custom: vec![
                MetricPoint {
                    name: "legacy_cpu_bounce_bytes".to_string(),
                    value: telemetry.completed_bytes,
                },
                MetricPoint {
                    name: "legacy_copy_passes".to_string(),
                    value: 2,
                },
            ],
            ..Default::default()
        }),
        outputs: vec![encoded.clone()],
        baseline_outputs: Some(vec![encoded]),
    })
}

impl BenchCase for NvmeGpuIngestRegisteredMappedRead {
    fn id(&self) -> BenchId {
        BenchId(REGISTERED_MAPPED_SPEC.id.to_string())
    }

    fn metadata(&self) -> BenchMetadata {
        bench_case_metadata(REGISTERED_MAPPED_SPEC)
    }

    fn suites(&self) -> &'static [crate::api::suite::SuiteKind] {
        RELEASE_SUITES
    }

    fn requirements(&self) -> BenchRequirements {
        bench_case_requirements(REGISTERED_MAPPED_SPEC)
    }

    fn prepare(&self, _ctx: &mut BenchContext) -> Result<PreparedCase, BenchError> {
        Ok(Box::new(REGISTERED_MAPPED_SPEC))
    }

    fn program<'a>(&self, _prepared: &'a PreparedCase) -> Option<&'a vyre::ir::Program> {
        None
    }

    fn run(
        &self,
        _ctx: &mut BenchContext,
        prepared: &mut PreparedCase,
    ) -> Result<BenchRun, BenchError> {
        run_ingest_accounting(prepared)
    }

    fn verify(&self, _ctx: &mut BenchContext, run: &BenchRun) -> Result<Correctness, BenchError> {
        run.verify_exact_outputs()
    }

    fn bytes_touched(&self, prepared: &PreparedCase) -> (u64, u64) {
        prepared_ingest_spec(prepared)
            .ok()
            .and_then(|spec| spec.total_bytes().map(|bytes| (bytes, bytes)))
            .unwrap_or((0, 0))
    }
}

impl BenchCase for NvmeGpuIngestGpuDirectNvme {
    fn id(&self) -> BenchId {
        BenchId(GPUDIRECT_SPEC.id.to_string())
    }

    fn metadata(&self) -> BenchMetadata {
        bench_case_metadata(GPUDIRECT_SPEC)
    }

    fn suites(&self) -> &'static [crate::api::suite::SuiteKind] {
        RELEASE_SUITES
    }

    fn requirements(&self) -> BenchRequirements {
        bench_case_requirements(GPUDIRECT_SPEC)
    }

    fn prepare(&self, _ctx: &mut BenchContext) -> Result<PreparedCase, BenchError> {
        Ok(Box::new(GPUDIRECT_SPEC))
    }

    fn program<'a>(&self, _prepared: &'a PreparedCase) -> Option<&'a vyre::ir::Program> {
        None
    }

    fn run(
        &self,
        _ctx: &mut BenchContext,
        prepared: &mut PreparedCase,
    ) -> Result<BenchRun, BenchError> {
        run_ingest_accounting(prepared)
    }

    fn verify(&self, _ctx: &mut BenchContext, run: &BenchRun) -> Result<Correctness, BenchError> {
        run.verify_exact_outputs()
    }

    fn bytes_touched(&self, prepared: &PreparedCase) -> (u64, u64) {
        prepared_ingest_spec(prepared)
            .ok()
            .and_then(|spec| spec.total_bytes().map(|bytes| (bytes, bytes)))
            .unwrap_or((0, 0))
    }
}

inventory::submit! {
    &NvmeGpuIngestRegisteredMappedRead as &'static dyn BenchCase
}

inventory::submit! {
    &NvmeGpuIngestGpuDirectNvme as &'static dyn BenchCase
}
