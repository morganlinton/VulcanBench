use super::*;
/// Release-gate report for one frozen clang/vyrec parity target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParityReleaseReport {
    /// Frozen target identifier, such as `linux-lib-math-v6.8`.
    pub target_id: String,
    /// Exact upstream source commit used by the target.
    pub source_commit: String,
    /// clang version used as the oracle for this report.
    pub clang_version: String,
    /// vyrec build identity used for this report.
    pub vyrec_version: String,
    /// GPU identity used for this report.
    pub gpu: String,
    /// Execution mode used for this report: staged, resident-graph, or megakernel.
    pub mode: String,
    findings: Vec<ParityFinding>,
    performance_proofs: Vec<ParityPerformanceProof>,
    gpu_residency_proofs: Vec<ParityGpuResidencyProof>,
}

/// Dashboard summary for one release parity report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParityReleaseDashboard {
    /// Frozen target identifier, such as `linux-lib-math-v6.8`.
    pub target_id: String,
    /// Exact upstream source commit used by the target.
    pub source_commit: String,
    /// Execution mode used for this report: staged, resident-graph, or megakernel.
    pub mode: String,
    /// Total comparator findings in the report.
    pub total_findings: usize,
    /// Release-blocking findings in the report.
    pub blocking_findings: usize,
    /// Matching findings in the report.
    pub matching_findings: usize,
    /// Explicitly approved target differences in the report.
    pub explained_differences: usize,
    /// Number of structured performance proofs attached to the report.
    pub performance_proof_count: usize,
    /// Best measured speedup scaled by 1000, if any performance proof exists.
    pub best_measured_speedup_x1000: Option<u64>,
    /// Number of structured GPU-residency proofs attached to the report.
    pub gpu_residency_proof_count: usize,
    /// Total kernel launches reported by all GPU-residency proofs.
    pub total_kernel_launch_count: u64,
    /// Total host-to-device bytes reported by all GPU-residency proofs.
    pub total_host_write_bytes: u64,
    /// Total device-to-host bytes reported by all GPU-residency proofs.
    pub total_host_readback_bytes: u64,
    /// Total host synchronization points reported by all GPU-residency proofs.
    pub total_host_sync_points: u64,
    /// Total device allocation bytes reported by all GPU-residency proofs.
    pub total_device_allocation_bytes: u64,
    /// Number of GPU-residency proofs with occupancy evidence.
    pub gpu_occupancy_evidence_count: usize,
    /// Total memory pressure high-water bytes reported by all GPU-residency proofs.
    pub total_memory_pressure_bytes: u64,
    /// Whether the underlying report is release-ready.
    pub release_ready: bool,
}

impl ParityReleaseReport {
    /// Creates an empty release-gate report.
    #[must_use]
    pub fn new(
        target_id: impl Into<String>,
        source_commit: impl Into<String>,
        clang_version: impl Into<String>,
        vyrec_version: impl Into<String>,
        gpu: impl Into<String>,
        mode: impl Into<String>,
    ) -> Self {
        Self {
            target_id: target_id.into(),
            source_commit: source_commit.into(),
            clang_version: clang_version.into(),
            vyrec_version: vyrec_version.into(),
            gpu: gpu.into(),
            mode: mode.into(),
            findings: Vec::new(),
            performance_proofs: Vec::new(),
            gpu_residency_proofs: Vec::new(),
        }
    }

    /// Adds one comparator finding.
    pub fn push_finding(&mut self, finding: ParityFinding) {
        self.findings.push(finding);
    }

    /// Adds one matching fact.
    pub fn push_match(
        &mut self,
        category: ParityFactCategory,
        fact_id: impl Into<String>,
        detail: impl Into<String>,
    ) {
        self.push_finding(ParityFinding::new(
            category,
            ParityFindingKind::Match,
            fact_id,
            detail,
        ));
    }

    /// Adds one explicitly approved target difference.
    pub fn push_explained_difference(
        &mut self,
        category: ParityFactCategory,
        fact_id: impl Into<String>,
        detail: impl Into<String>,
    ) {
        self.push_finding(ParityFinding::new(
            category,
            ParityFindingKind::ExplainedTargetDifference,
            fact_id,
            detail,
        ));
    }

    /// Adds a measured performance proof and turns it into a release-gate finding.
    pub fn push_performance_proof(
        &mut self,
        fact_id: impl Into<String>,
        proof: ParityPerformanceProof,
    ) {
        let fact_id = fact_id.into();
        let measured = proof.measured_speedup_x1000();
        let detail = format!(
            "clang_wall_ns={} vyrec_wall_ns={} measured_x1000={} required_x1000={}",
            proof.clang_wall_ns, proof.vyrec_wall_ns, measured, proof.required_speedup_x1000
        );
        let kind = if proof.passes_contract() {
            ParityFindingKind::Match
        } else {
            ParityFindingKind::PerformanceFailure
        };
        self.push_finding(ParityFinding::new(
            ParityFactCategory::Performance,
            kind,
            fact_id,
            detail,
        ));
        self.performance_proofs.push(proof);
    }

    /// Adds a GPU residency proof and turns it into a release-gate finding.
    pub fn push_gpu_residency_proof(
        &mut self,
        fact_id: impl Into<String>,
        proof: ParityGpuResidencyProof,
    ) {
        let failures = proof.contract_failures();
        let detail = format!(
            "gpu={} driver={} production_host_escape_events={} false_no_gpu_skips={} launches={} host_write_bytes={} host_readback_bytes={} host_sync_points={} device_allocation_bytes={} occupancy={} memory_pressure_bytes={} residency_failures={}",
            proof.gpu_name,
            proof.driver,
            proof.production_host_escape_events,
            proof.false_no_gpu_skips,
            proof.kernel_launch_count,
            proof.host_write_bytes,
            proof.host_readback_bytes,
            proof.host_sync_points,
            proof.device_allocation_bytes,
            proof.gpu_occupancy_evidence,
            proof.memory_pressure_bytes,
            if failures.is_empty() {
                "none".to_string()
            } else {
                failures.join(" | ")
            }
        );
        let kind = if failures.is_empty() {
            ParityFindingKind::Match
        } else {
            ParityFindingKind::GpuResidencyFailure
        };
        self.push_finding(ParityFinding::new(
            ParityFactCategory::GpuResidency,
            kind,
            fact_id,
            detail,
        ));
        self.gpu_residency_proofs.push(proof);
    }

    /// Adds unsupported-construct evidence and turns it into a release-gate finding.
    pub fn push_unsupported_construct(&mut self, construct: ParityUnsupportedConstruct) {
        let fact_id = format!("construct:{}:{}", construct.construct, construct.location);
        let detail = format!(
            "construct={} location={} status={:?} detail={}",
            construct.construct, construct.location, construct.status, construct.detail
        );
        let kind = match construct.status {
            ParityConstructStatus::Implemented => ParityFindingKind::Match,
            ParityConstructStatus::ApprovedOutOfScope => {
                ParityFindingKind::ExplainedTargetDifference
            }
            ParityConstructStatus::Unresolved => ParityFindingKind::VyrecMissing,
        };
        self.push_finding(ParityFinding::new(
            construct.category,
            kind,
            fact_id,
            detail,
        ));
    }

    /// Returns every finding in insertion order.
    #[must_use]
    pub fn findings(&self) -> &[ParityFinding] {
        &self.findings
    }

    /// Returns release-blocking findings in insertion order.
    #[must_use]
    pub fn blocking_findings(&self) -> Vec<&ParityFinding> {
        self.findings
            .iter()
            .filter(|finding| finding.blocks_release())
            .collect()
    }

    /// Returns whether this report satisfies the release gate.
    #[must_use]
    pub fn is_release_ready(&self) -> bool {
        !self.findings.is_empty() && self.blocking_findings().is_empty()
    }

    /// Builds a dashboard summary from this report.
    #[must_use]
    pub fn dashboard(&self) -> ParityReleaseDashboard {
        let matching_findings = self
            .findings
            .iter()
            .filter(|finding| finding.kind == ParityFindingKind::Match)
            .count();
        let explained_differences = self
            .findings
            .iter()
            .filter(|finding| finding.kind == ParityFindingKind::ExplainedTargetDifference)
            .count();
        let best_measured_speedup_x1000 = self
            .performance_proofs
            .iter()
            .map(|proof| proof.measured_speedup_x1000())
            .max();
        let total_kernel_launch_count =
            self.gpu_residency_proofs.iter().fold(0_u64, |sum, proof| {
                sum.saturating_add(proof.kernel_launch_count)
            });
        let total_host_write_bytes = self.gpu_residency_proofs.iter().fold(0_u64, |sum, proof| {
            sum.saturating_add(proof.host_write_bytes)
        });
        let total_host_readback_bytes =
            self.gpu_residency_proofs.iter().fold(0_u64, |sum, proof| {
                sum.saturating_add(proof.host_readback_bytes)
            });
        let total_host_sync_points = self.gpu_residency_proofs.iter().fold(0_u64, |sum, proof| {
            sum.saturating_add(proof.host_sync_points)
        });
        let total_device_allocation_bytes =
            self.gpu_residency_proofs.iter().fold(0_u64, |sum, proof| {
                sum.saturating_add(proof.device_allocation_bytes)
            });
        let gpu_occupancy_evidence_count = self
            .gpu_residency_proofs
            .iter()
            .filter(|proof| !proof.gpu_occupancy_evidence.trim().is_empty())
            .count();
        let total_memory_pressure_bytes =
            self.gpu_residency_proofs.iter().fold(0_u64, |sum, proof| {
                sum.saturating_add(proof.memory_pressure_bytes)
            });

        ParityReleaseDashboard {
            target_id: self.target_id.clone(),
            source_commit: self.source_commit.clone(),
            mode: self.mode.clone(),
            total_findings: self.findings.len(),
            blocking_findings: self.blocking_findings().len(),
            matching_findings,
            explained_differences,
            performance_proof_count: self.performance_proofs.len(),
            best_measured_speedup_x1000,
            gpu_residency_proof_count: self.gpu_residency_proofs.len(),
            total_kernel_launch_count,
            total_host_write_bytes,
            total_host_readback_bytes,
            total_host_sync_points,
            total_device_allocation_bytes,
            gpu_occupancy_evidence_count,
            total_memory_pressure_bytes,
            release_ready: self.is_release_ready(),
        }
    }
}
