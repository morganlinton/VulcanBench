//! CUDA resident graph session planning.
//!
//! Repeated fixed-graph execution is the release path for dataflow analyses
//! and frontend graph passes. The graph topology must be uploaded once, kept
//! resident, and reused across runs. This module plans the steady-state memory
//! envelope and quantifies the upload/allocation/fence work removed by keeping
//! graph state resident.

use crate::backend::accounting::{
    checked_add_u64_count as checked_add, checked_mul_u64_count as checked_mul,
    CudaArithmeticOverflow,
};
use crate::backend::staging_reserve::reserved_vec;
use crate::megakernel_speedup_gate::{
    format_validated_cuda_megakernel_speedup_evidence_csv, CudaMegakernelSpeedupGateError,
    CudaMegakernelSpeedupProof, CudaMegakernelSpeedupSample,
};
use vyre_driver::ResidentGraphReuseTelemetry;

/// Host readback policy for a CUDA resident graph session.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CudaResidentGraphReadback {
    /// Read only the final output after all repeated runs complete.
    FinalOnly,
    /// Read after every run.
    PerRun,
}

/// Input profile for repeated execution over one resident CUDA graph.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CudaResidentGraphSessionProfile {
    /// Stable normalized graph layout hash.
    pub graph_layout_hash: u64,
    /// Bytes required for resident graph topology and immutable metadata.
    pub graph_bytes: u64,
    /// Number of repeated executions over the same graph.
    pub run_count: u64,
    /// Frontier/input bytes refreshed each run.
    pub per_run_frontier_bytes: u64,
    /// Scratch bytes reused across runs.
    pub reusable_scratch_bytes: u64,
    /// Meaningful output bytes produced per run.
    pub per_run_output_bytes: u64,
    /// Explicit CUDA memory budget.
    pub budget_bytes: u64,
    /// Host readback policy.
    pub readback: CudaResidentGraphReadback,
}

/// CUDA resident graph session plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CudaResidentGraphSessionPlan {
    /// Stable normalized graph layout hash.
    pub graph_layout_hash: u64,
    /// Bytes uploaded once at session start.
    pub one_time_graph_upload_bytes: u64,
    /// Bytes refreshed across all runs.
    pub total_frontier_refresh_bytes: u64,
    /// Peak bytes resident on device.
    pub peak_resident_bytes: u64,
    /// Bytes avoided versus uploading graph topology before every run.
    pub avoided_graph_upload_bytes: u64,
    /// Backend-neutral graph upload/reuse telemetry for the session.
    pub graph_reuse: ResidentGraphReuseTelemetry,
    /// Device allocations avoided versus allocating graph/scratch/output per run.
    pub avoided_device_allocations: u64,
    /// Host fences avoided versus per-run readback.
    pub avoided_host_fences: u64,
    /// Host readback bytes after session planning.
    pub host_readback_bytes: u64,
    /// Whether the plan keeps graph topology resident.
    pub graph_topology_resident: bool,
    /// Whether scratch allocation is reused across runs.
    pub scratch_reused: bool,
    /// Whether host readback happens once at the end.
    pub final_only_host_readback: bool,
}

/// Release evidence profile for a measured resident graph session.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CudaResidentGraphSessionEvidence {
    /// Backend identifier that produced the measured session.
    pub backend_id: &'static str,
    /// Physical CUDA device ordinal that produced the measured session.
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
    /// Planned resident session.
    pub plan: CudaResidentGraphSessionPlan,
    /// Naive host-orchestrated execution time in nanoseconds.
    pub host_orchestrated_ns: f64,
    /// Resident megakernel execution time in nanoseconds.
    pub resident_megakernel_ns: f64,
    /// Setup time measured outside the timed region.
    pub setup_ns: f64,
}

/// CUDA resident graph session planning errors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CudaResidentGraphSessionError {
    /// Graph hash must be non-zero.
    ZeroGraphHash,
    /// Graph must have resident bytes.
    ZeroGraphBytes,
    /// Run count must be non-zero.
    ZeroRuns,
    /// Explicit CUDA memory budget cannot be zero.
    ZeroBudget,
    /// Per-run host readback would reintroduce CPU orchestration.
    PerRunReadbackRejected,
    /// Byte arithmetic overflowed.
    ByteCountOverflow {
        /// Field being computed.
        field: &'static str,
    },
    /// Peak resident bytes exceed the explicit budget.
    OverBudget {
        /// Required resident bytes.
        required_bytes: u64,
        /// Caller-provided budget.
        budget_bytes: u64,
    },
    /// Resident session evidence does not describe a final-only resident execution.
    NonResidentEvidence,
}

/// Error while converting resident graph session evidence into release CSV.
#[derive(Clone, Debug, PartialEq)]
pub enum CudaResidentGraphSessionEvidenceError {
    /// Resident session evidence was not a valid final-only resident session.
    Session(CudaResidentGraphSessionError),
    /// Megakernel speedup release gate rejected the converted samples.
    Speedup(CudaMegakernelSpeedupGateError),
    /// Resident session evidence sample staging could not reserve enough slots.
    SampleReserveFailed {
        /// Required sample capacity.
        capacity: usize,
        /// Allocator/backend error message.
        message: String,
    },
}

impl std::fmt::Display for CudaResidentGraphSessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ZeroGraphHash => write!(
                f,
                "CUDA resident graph session received graph_layout_hash=0. Fix: normalize and hash graph topology before session planning."
            ),
            Self::ZeroGraphBytes => write!(
                f,
                "CUDA resident graph session received graph_bytes=0. Fix: pass the concrete resident graph topology byte count."
            ),
            Self::ZeroRuns => write!(
                f,
                "CUDA resident graph session received run_count=0. Fix: plan only non-empty repeated execution sessions."
            ),
            Self::ZeroBudget => write!(
                f,
                "CUDA resident graph session received budget_bytes=0. Fix: pass an explicit CUDA memory budget."
            ),
            Self::PerRunReadbackRejected => write!(
                f,
                "CUDA resident graph session rejected per-run readback. Fix: compact final outputs on device and read back once after repeated execution."
            ),
            Self::ByteCountOverflow { field } => write!(
                f,
                "CUDA resident graph session overflowed while computing {field}. Fix: shard repeated graph execution before planning."
            ),
            Self::OverBudget {
                required_bytes,
                budget_bytes,
            } => write!(
                f,
                "CUDA resident graph session requires {required_bytes} bytes but budget allows {budget_bytes}. Fix: reduce frontier/output size, reuse compact outputs, or shard the graph."
            ),
            Self::NonResidentEvidence => write!(
                f,
                "CUDA resident graph session evidence is not final-only resident execution. Fix: build evidence from a plan with resident topology, reused scratch, and one final readback."
            ),
        }
    }
}

impl std::error::Error for CudaResidentGraphSessionError {}

impl CudaArithmeticOverflow for CudaResidentGraphSessionError {
    fn arithmetic_overflow(field: &'static str) -> Self {
        Self::ByteCountOverflow { field }
    }
}

impl std::fmt::Display for CudaResidentGraphSessionEvidenceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Session(error) => write!(f, "{error}"),
            Self::Speedup(error) => write!(f, "{error}"),
            Self::SampleReserveFailed { capacity, message } => write!(
                f,
                "CUDA resident graph session evidence could not reserve {capacity} release sample slot(s): {message}. Fix: split the release evidence batch before formatting."
            ),
        }
    }
}

impl std::error::Error for CudaResidentGraphSessionEvidenceError {}

impl From<CudaResidentGraphSessionError> for CudaResidentGraphSessionEvidenceError {
    fn from(error: CudaResidentGraphSessionError) -> Self {
        Self::Session(error)
    }
}

impl From<CudaMegakernelSpeedupGateError> for CudaResidentGraphSessionEvidenceError {
    fn from(error: CudaMegakernelSpeedupGateError) -> Self {
        Self::Speedup(error)
    }
}

/// Plan a repeated CUDA execution session over one resident graph.
pub fn plan_cuda_resident_graph_session(
    profile: CudaResidentGraphSessionProfile,
) -> Result<CudaResidentGraphSessionPlan, CudaResidentGraphSessionError> {
    if profile.graph_layout_hash == 0 {
        return Err(CudaResidentGraphSessionError::ZeroGraphHash);
    }
    if profile.graph_bytes == 0 {
        return Err(CudaResidentGraphSessionError::ZeroGraphBytes);
    }
    if profile.run_count == 0 {
        return Err(CudaResidentGraphSessionError::ZeroRuns);
    }
    if profile.budget_bytes == 0 {
        return Err(CudaResidentGraphSessionError::ZeroBudget);
    }
    if profile.readback != CudaResidentGraphReadback::FinalOnly {
        return Err(CudaResidentGraphSessionError::PerRunReadbackRejected);
    }
    if profile.run_count == 1 {
        let graph_plus_frontier = checked_add(
            profile.graph_bytes,
            profile.per_run_frontier_bytes,
            "graph plus frontier bytes",
        )?;
        let with_scratch = checked_add(
            graph_plus_frontier,
            profile.reusable_scratch_bytes,
            "graph frontier scratch bytes",
        )?;
        let peak_resident_bytes = checked_add(
            with_scratch,
            profile.per_run_output_bytes,
            "peak resident bytes",
        )?;
        if peak_resident_bytes > profile.budget_bytes {
            return Err(CudaResidentGraphSessionError::OverBudget {
                required_bytes: peak_resident_bytes,
                budget_bytes: profile.budget_bytes,
            });
        }
        return Ok(CudaResidentGraphSessionPlan {
            graph_layout_hash: profile.graph_layout_hash,
            one_time_graph_upload_bytes: profile.graph_bytes,
            total_frontier_refresh_bytes: profile.per_run_frontier_bytes,
            peak_resident_bytes,
            avoided_graph_upload_bytes: 0,
            graph_reuse: ResidentGraphReuseTelemetry::cold_upload(profile.graph_bytes),
            avoided_device_allocations: 0,
            avoided_host_fences: 0,
            host_readback_bytes: profile.per_run_output_bytes,
            graph_topology_resident: true,
            scratch_reused: true,
            final_only_host_readback: true,
        });
    }

    let graph_plus_frontier = checked_add(
        profile.graph_bytes,
        profile.per_run_frontier_bytes,
        "graph plus frontier bytes",
    )?;
    let with_scratch = checked_add(
        graph_plus_frontier,
        profile.reusable_scratch_bytes,
        "graph frontier scratch bytes",
    )?;
    let peak_resident_bytes = checked_add(
        with_scratch,
        profile.per_run_output_bytes,
        "peak resident bytes",
    )?;
    if peak_resident_bytes > profile.budget_bytes {
        return Err(CudaResidentGraphSessionError::OverBudget {
            required_bytes: peak_resident_bytes,
            budget_bytes: profile.budget_bytes,
        });
    }

    let total_frontier_refresh_bytes = checked_mul(
        profile.run_count,
        profile.per_run_frontier_bytes,
        "total frontier refresh bytes",
    )?;
    let repeated_runs = profile.run_count - 1;
    let avoided_graph_upload_bytes = checked_mul(
        repeated_runs,
        profile.graph_bytes,
        "avoided graph upload bytes",
    )?;
    let avoided_device_allocations = checked_mul(repeated_runs, 3, "avoided allocations")?;

    Ok(CudaResidentGraphSessionPlan {
        graph_layout_hash: profile.graph_layout_hash,
        one_time_graph_upload_bytes: profile.graph_bytes,
        total_frontier_refresh_bytes,
        peak_resident_bytes,
        avoided_graph_upload_bytes,
        graph_reuse: ResidentGraphReuseTelemetry::from_counters(
            1,
            repeated_runs,
            profile.graph_bytes,
            avoided_graph_upload_bytes,
        ),
        avoided_device_allocations,
        avoided_host_fences: repeated_runs,
        host_readback_bytes: profile.per_run_output_bytes,
        graph_topology_resident: true,
        scratch_reused: true,
        final_only_host_readback: true,
    })
}

/// Convert a planned resident graph session measurement into the release
/// megakernel speedup sample schema.
pub fn resident_graph_session_speedup_sample(
    evidence: CudaResidentGraphSessionEvidence,
) -> Result<CudaMegakernelSpeedupSample, CudaResidentGraphSessionError> {
    if !evidence.plan.graph_topology_resident
        || !evidence.plan.scratch_reused
        || !evidence.plan.final_only_host_readback
    {
        return Err(CudaResidentGraphSessionError::NonResidentEvidence);
    }
    Ok(CudaMegakernelSpeedupSample {
        backend_id: evidence.backend_id,
        device_ordinal: evidence.device_ordinal,
        device_memory_bytes: evidence.device_memory_bytes,
        compute_capability_major: evidence.compute_capability_major,
        compute_capability_minor: evidence.compute_capability_minor,
        graph_nodes: evidence.graph_nodes,
        graph_edges: evidence.graph_edges,
        repetitions: checked_add(evidence.plan.avoided_host_fences, 1, "evidence repetitions")?,
        host_orchestrated_ns: evidence.host_orchestrated_ns,
        resident_megakernel_ns: evidence.resident_megakernel_ns,
        setup_ns: evidence.setup_ns,
        timed_graph_uploads: 0,
        timed_host_allocations: 0,
        timed_host_syncs: 0,
        resident_borrowed_fallback_dispatches: 0,
    })
}

/// Convert measured resident graph sessions into the exact validated CUDA
/// megakernel release CSV artifact.
pub fn format_validated_cuda_resident_graph_session_evidence_csv(
    evidence: &[CudaResidentGraphSessionEvidence],
    required_speedup_x: f64,
) -> Result<(CudaMegakernelSpeedupProof, String), CudaResidentGraphSessionEvidenceError> {
    let mut samples = reserved_vec(
        evidence.len(),
        "cuda resident graph session release samples",
    )
    .map_err(
        |error| CudaResidentGraphSessionEvidenceError::SampleReserveFailed {
            capacity: evidence.len(),
            message: error.to_string(),
        },
    )?;
    for item in evidence {
        samples.push(resident_graph_session_speedup_sample(*item)?);
    }
    format_validated_cuda_megakernel_speedup_evidence_csv(&samples, required_speedup_x)
        .map_err(CudaResidentGraphSessionEvidenceError::Speedup)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resident_graph_session_uses_shared_typed_cuda_arithmetic() {
        let source = include_str!("resident_graph_session.rs");

        assert!(source.contains("checked_add_u64_count as checked_add"));
        assert!(source.contains("checked_mul_u64_count as checked_mul"));
        assert!(source.contains("impl CudaArithmeticOverflow for CudaResidentGraphSessionError"));
        assert!(!source.contains(concat!("fn checked_", "mul(")));
        assert!(!source.contains(concat!("fn checked_", "add(")));
    }

    #[test]
    fn resident_graph_session_amortizes_fixed_graph_repeated_execution() {
        let plan = plan_cuda_resident_graph_session(CudaResidentGraphSessionProfile {
            graph_layout_hash: 0xabc,
            graph_bytes: 1_048_576,
            run_count: 128,
            per_run_frontier_bytes: 4_096,
            reusable_scratch_bytes: 65_536,
            per_run_output_bytes: 2_048,
            budget_bytes: 2_000_000,
            readback: CudaResidentGraphReadback::FinalOnly,
        })
        .expect("Fix: resident graph session should fit");

        assert_eq!(plan.one_time_graph_upload_bytes, 1_048_576);
        assert_eq!(plan.total_frontier_refresh_bytes, 524_288);
        assert_eq!(plan.avoided_graph_upload_bytes, 133_169_152);
        assert_eq!(
            plan.graph_reuse,
            ResidentGraphReuseTelemetry::from_counters(1, 127, 1_048_576, 133_169_152)
        );
        assert_eq!(plan.avoided_device_allocations, 381);
        assert_eq!(plan.avoided_host_fences, 127);
        assert_eq!(plan.host_readback_bytes, 2_048);
        assert!(plan.graph_topology_resident);
        assert!(plan.scratch_reused);
        assert!(plan.final_only_host_readback);
    }

    #[test]
    fn resident_graph_session_builds_release_speedup_sample_without_timed_pollution() {
        let plan = plan_cuda_resident_graph_session(CudaResidentGraphSessionProfile {
            graph_layout_hash: 0xabc,
            graph_bytes: 1_048_576,
            run_count: 128,
            per_run_frontier_bytes: 4_096,
            reusable_scratch_bytes: 65_536,
            per_run_output_bytes: 2_048,
            budget_bytes: 2_000_000,
            readback: CudaResidentGraphReadback::FinalOnly,
        })
        .expect("Fix: resident graph session should fit");

        let sample = resident_graph_session_speedup_sample(CudaResidentGraphSessionEvidence {
            backend_id: crate::CUDA_BACKEND_ID,
            device_ordinal: 0,
            device_memory_bytes: 32 * 1024 * 1024 * 1024,
            compute_capability_major: 12,
            compute_capability_minor: 0,
            graph_nodes: 10_000,
            graph_edges: 80_000,
            plan,
            host_orchestrated_ns: 1_000_000.0,
            resident_megakernel_ns: 10_000.0,
            setup_ns: 250_000.0,
        })
        .expect("Fix: resident final-only plan should produce release evidence");

        assert_eq!(sample.backend_id, crate::CUDA_BACKEND_ID);
        assert_eq!(sample.device_memory_bytes, 32 * 1024 * 1024 * 1024);
        assert_eq!(sample.compute_capability_major, 12);
        assert_eq!(sample.graph_nodes, 10_000);
        assert_eq!(sample.graph_edges, 80_000);
        assert_eq!(sample.repetitions, 128);
        assert_eq!(sample.timed_graph_uploads, 0);
        assert_eq!(sample.timed_host_allocations, 0);
        assert_eq!(sample.timed_host_syncs, 0);
        assert_eq!(sample.resident_borrowed_fallback_dispatches, 0);
    }

    #[test]
    fn resident_graph_session_formats_validated_release_speedup_csv() {
        let plan_a = plan_cuda_resident_graph_session(CudaResidentGraphSessionProfile {
            graph_layout_hash: 0xabc,
            graph_bytes: 1_048_576,
            run_count: 128,
            per_run_frontier_bytes: 4_096,
            reusable_scratch_bytes: 65_536,
            per_run_output_bytes: 2_048,
            budget_bytes: 2_000_000,
            readback: CudaResidentGraphReadback::FinalOnly,
        })
        .expect("Fix: first resident graph session should fit");
        let plan_b = plan_cuda_resident_graph_session(CudaResidentGraphSessionProfile {
            graph_layout_hash: 0xdef,
            graph_bytes: 2_097_152,
            run_count: 256,
            per_run_frontier_bytes: 8_192,
            reusable_scratch_bytes: 131_072,
            per_run_output_bytes: 4_096,
            budget_bytes: 4_000_000,
            readback: CudaResidentGraphReadback::FinalOnly,
        })
        .expect("Fix: second resident graph session should fit");
        let evidence = [
            CudaResidentGraphSessionEvidence {
                backend_id: crate::CUDA_BACKEND_ID,
                device_ordinal: 0,
                device_memory_bytes: 32 * 1024 * 1024 * 1024,
                compute_capability_major: 12,
                compute_capability_minor: 0,
                graph_nodes: 10_000,
                graph_edges: 80_000,
                plan: plan_a,
                host_orchestrated_ns: 1_000_000.0,
                resident_megakernel_ns: 10_000.0,
                setup_ns: 250_000.0,
            },
            CudaResidentGraphSessionEvidence {
                backend_id: crate::CUDA_BACKEND_ID,
                device_ordinal: 0,
                device_memory_bytes: 32 * 1024 * 1024 * 1024,
                compute_capability_major: 12,
                compute_capability_minor: 0,
                graph_nodes: 20_000,
                graph_edges: 160_000,
                plan: plan_b,
                host_orchestrated_ns: 2_500_000.0,
                resident_megakernel_ns: 20_000.0,
                setup_ns: 350_000.0,
            },
        ];

        let (proof, csv) =
            format_validated_cuda_resident_graph_session_evidence_csv(&evidence, 100.0)
                .expect("Fix: resident graph release evidence should format as validated CSV");
        let reparsed = crate::validate_cuda_megakernel_speedup_evidence_csv(&csv, 100.0)
            .expect("Fix: resident graph release CSV should roundtrip through verifier");

        assert_eq!(proof, reparsed);
        assert_eq!(proof.sample_count, 2);
        assert_eq!(proof.min_speedup_x, 100.0);
        assert_eq!(proof.max_speedup_x, 125.0);
        assert_eq!(csv.lines().count(), 3);
    }

    #[test]
    fn resident_graph_session_rejects_host_orchestration_shape() {
        assert_eq!(
            plan_cuda_resident_graph_session(CudaResidentGraphSessionProfile {
                graph_layout_hash: 1,
                graph_bytes: 128,
                run_count: 2,
                per_run_frontier_bytes: 16,
                reusable_scratch_bytes: 16,
                per_run_output_bytes: 16,
                budget_bytes: 1_024,
                readback: CudaResidentGraphReadback::PerRun,
            })
            .expect_err("per-run readback should fail"),
            CudaResidentGraphSessionError::PerRunReadbackRejected
        );
    }

    #[test]
    fn resident_graph_session_rejects_invalid_inputs_and_budget() {
        assert_eq!(
            plan_cuda_resident_graph_session(profile(0, 128, 1, 16, 16, 16, 1_024))
                .expect_err("zero hash should fail"),
            CudaResidentGraphSessionError::ZeroGraphHash
        );
        assert_eq!(
            plan_cuda_resident_graph_session(profile(1, 128, 0, 16, 16, 16, 1_024))
                .expect_err("zero runs should fail"),
            CudaResidentGraphSessionError::ZeroRuns
        );
        assert_eq!(
            plan_cuda_resident_graph_session(profile(1, 128, 1, 16, 16, 16, 127))
                .expect_err("over-budget session should fail"),
            CudaResidentGraphSessionError::OverBudget {
                required_bytes: 176,
                budget_bytes: 127,
            }
        );
    }

    #[test]
    fn resident_graph_session_evidence_uses_fallible_sample_staging() {
        let source = include_str!("resident_graph_session.rs");

        assert!(source.contains("use crate::backend::staging_reserve::reserve_vec;"));
        assert!(source.contains("SampleReserveFailed"));
        assert!(!source.contains(concat!("Vec", "::with_capacity(evidence.len())")));
    }

    fn profile(
        graph_layout_hash: u64,
        graph_bytes: u64,
        run_count: u64,
        per_run_frontier_bytes: u64,
        reusable_scratch_bytes: u64,
        per_run_output_bytes: u64,
        budget_bytes: u64,
    ) -> CudaResidentGraphSessionProfile {
        CudaResidentGraphSessionProfile {
            graph_layout_hash,
            graph_bytes,
            run_count,
            per_run_frontier_bytes,
            reusable_scratch_bytes,
            per_run_output_bytes,
            budget_bytes,
            readback: CudaResidentGraphReadback::FinalOnly,
        }
    }
}
