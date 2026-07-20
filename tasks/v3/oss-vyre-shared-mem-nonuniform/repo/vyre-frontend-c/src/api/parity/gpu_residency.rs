/// GPU residency proof for one vyrec release-target run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParityGpuResidencyProof {
    /// GPU name reported by the runtime probe.
    pub gpu_name: String,
    /// GPU driver/runtime identity used for the run.
    pub driver: String,
    /// Number of production host-reference escape events observed during the run.
    pub production_host_escape_events: u32,
    /// Number of GPU-required tests or stages skipped because of a no-GPU report.
    pub false_no_gpu_skips: u32,
    /// Kernel launches observed during the run.
    pub kernel_launch_count: u64,
    /// Host-to-device bytes observed during the run.
    pub host_write_bytes: u64,
    /// Device-to-host bytes observed during the run.
    pub host_readback_bytes: u64,
    /// Host synchronization points observed during the run.
    pub host_sync_points: u64,
    /// Device allocation bytes observed during the run.
    pub device_allocation_bytes: u64,
    /// GPU occupancy evidence captured by the backend profiler or runtime instrumentation.
    pub gpu_occupancy_evidence: String,
    /// Device memory pressure high-water mark in bytes.
    pub memory_pressure_bytes: u64,
}

impl ParityGpuResidencyProof {
    /// Creates a GPU residency proof.
    #[must_use]
    pub fn new(gpu_name: impl Into<String>, driver: impl Into<String>) -> Self {
        Self {
            gpu_name: gpu_name.into(),
            driver: driver.into(),
            production_host_escape_events: 0,
            false_no_gpu_skips: 0,
            kernel_launch_count: 0,
            host_write_bytes: 0,
            host_readback_bytes: 0,
            host_sync_points: 0,
            device_allocation_bytes: 0,
            gpu_occupancy_evidence: String::new(),
            memory_pressure_bytes: 0,
        }
    }

    /// Sets production host-reference escape count.
    #[must_use]
    pub const fn with_production_host_escape_events(mut self, count: u32) -> Self {
        self.production_host_escape_events = count;
        self
    }

    /// Sets false no-GPU skip count.
    #[must_use]
    pub const fn with_false_no_gpu_skips(mut self, count: u32) -> Self {
        self.false_no_gpu_skips = count;
        self
    }

    /// Sets kernel launch count.
    #[must_use]
    pub const fn with_kernel_launch_count(mut self, count: u64) -> Self {
        self.kernel_launch_count = count;
        self
    }

    /// Sets host-to-device bytes.
    #[must_use]
    pub const fn with_host_write_bytes(mut self, bytes: u64) -> Self {
        self.host_write_bytes = bytes;
        self
    }

    /// Sets device-to-host bytes.
    #[must_use]
    pub const fn with_host_readback_bytes(mut self, bytes: u64) -> Self {
        self.host_readback_bytes = bytes;
        self
    }

    /// Sets host synchronization point count.
    #[must_use]
    pub const fn with_host_sync_points(mut self, count: u64) -> Self {
        self.host_sync_points = count;
        self
    }

    /// Sets device allocation bytes.
    #[must_use]
    pub const fn with_device_allocation_bytes(mut self, bytes: u64) -> Self {
        self.device_allocation_bytes = bytes;
        self
    }

    /// Sets GPU occupancy evidence.
    #[must_use]
    pub fn with_gpu_occupancy_evidence(mut self, evidence: impl Into<String>) -> Self {
        self.gpu_occupancy_evidence = evidence.into();
        self
    }

    /// Sets device memory pressure high-water mark in bytes.
    #[must_use]
    pub const fn with_memory_pressure_bytes(mut self, bytes: u64) -> Self {
        self.memory_pressure_bytes = bytes;
        self
    }

    /// Returns whether this proof satisfies the release residency contract.
    #[must_use]
    pub fn passes_contract(&self) -> bool {
        self.contract_failures().is_empty()
    }

    /// Returns actionable residency-contract failures.
    ///
    /// An empty list means the proof is acceptable for release gating. Any
    /// returned item is intentionally phrased as a fixable blocker so callers
    /// do not have to interpret a silent boolean failure.
    #[must_use]
    pub fn contract_failures(&self) -> Vec<String> {
        let mut failures = Vec::new();
        if self.gpu_name.trim().is_empty() {
            failures.push(
                "missing GPU name. Fix: record the live adapter/device identity from the GPU probe before accepting residency evidence.".to_string(),
            );
        }
        if self.driver.trim().is_empty() {
            failures.push(
                "missing GPU driver/runtime identity. Fix: record driver/runtime details from nvidia-smi, CUDA, or WGPU adapter diagnostics.".to_string(),
            );
        }
        if self.gpu_occupancy_evidence.trim().is_empty() {
            failures.push(
                "missing GPU occupancy evidence. Fix: attach backend profiler, telemetry counter, or runtime instrumentation evidence for the run.".to_string(),
            );
        }
        if self.production_host_escape_events != 0 {
            failures.push(format!(
                "observed {} production host-reference escape event(s). Fix: remove the production host path or mark it as an explicit parity-test adapter outside release execution.",
                self.production_host_escape_events
            ));
        }
        if self.false_no_gpu_skips != 0 {
            failures.push(format!(
                "observed {} false no-GPU skip event(s). Fix: fail loudly with adapter/device probe diagnostics instead of skipping GPU-required work.",
                self.false_no_gpu_skips
            ));
        }
        failures
    }
}
