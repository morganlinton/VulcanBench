//! Differential preprocessing benchmark report model.

/// GPU counter block for a preprocessing benchmark run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreprocessBenchmarkGpuCounters {
    /// Kernel dispatches observed during vyre preprocessing.
    pub kernel_launch_count: u64,
    /// Host-to-device bytes observed during vyre preprocessing.
    pub host_write_bytes: u64,
    /// Device-to-host bytes observed during vyre preprocessing.
    pub host_readback_bytes: u64,
    /// Host synchronization points observed during vyre preprocessing.
    pub host_sync_points: u64,
}

/// One translation-unit row in a differential preprocessing benchmark.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreprocessBenchmarkTranslationUnit {
    /// Translation unit path relative to the benchmark source root.
    pub path: String,
    /// Input source bytes measured for this translation unit.
    pub input_bytes: u64,
    /// clang preprocessed output bytes.
    pub clang_output_bytes: u64,
    /// vyre preprocessed output bytes.
    pub vyre_output_bytes: u64,
    /// clang wall time in nanoseconds.
    pub clang_wall_ns: u64,
    /// vyre wall time in nanoseconds.
    pub vyre_wall_ns: u64,
}

/// Differential clang-vs-vyre preprocessing benchmark report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreprocessDifferentialBenchmarkReport {
    /// Frozen target identifier, such as `linux-lib-math-v6.8`.
    pub target_id: String,
    /// Exact upstream source commit measured by this report.
    pub source_commit: String,
    /// Target triple used for the run.
    pub target_triple: String,
    /// clang version string used as oracle.
    pub clang_version: String,
    /// vyre build/version identity.
    pub vyre_version: String,
    /// Translation-unit benchmark rows.
    pub translation_units: Vec<PreprocessBenchmarkTranslationUnit>,
    /// Aggregate GPU counters for the vyre run.
    pub gpu: PreprocessBenchmarkGpuCounters,
}

impl PreprocessBenchmarkTranslationUnit {
    /// Returns clang throughput in bytes/second.
    #[must_use]
    pub fn clang_bytes_per_second(&self) -> u64 {
        bytes_per_second(self.input_bytes, self.clang_wall_ns)
    }

    /// Returns vyre throughput in bytes/second.
    #[must_use]
    pub fn vyre_bytes_per_second(&self) -> u64 {
        bytes_per_second(self.input_bytes, self.vyre_wall_ns)
    }
}

impl PreprocessDifferentialBenchmarkReport {
    /// Returns aggregate input bytes across all translation units.
    #[must_use]
    pub fn total_input_bytes(&self) -> u64 {
        self.translation_units
            .iter()
            .map(|unit| unit.input_bytes)
            .sum()
    }

    /// Returns aggregate clang wall time in nanoseconds.
    #[must_use]
    pub fn total_clang_wall_ns(&self) -> u64 {
        self.translation_units
            .iter()
            .map(|unit| unit.clang_wall_ns)
            .sum()
    }

    /// Returns aggregate vyre wall time in nanoseconds.
    #[must_use]
    pub fn total_vyre_wall_ns(&self) -> u64 {
        self.translation_units
            .iter()
            .map(|unit| unit.vyre_wall_ns)
            .sum()
    }

    /// Returns aggregate clang throughput in bytes/second.
    #[must_use]
    pub fn clang_bytes_per_second(&self) -> u64 {
        bytes_per_second(self.total_input_bytes(), self.total_clang_wall_ns())
    }

    /// Returns aggregate vyre throughput in bytes/second.
    #[must_use]
    pub fn vyre_bytes_per_second(&self) -> u64 {
        bytes_per_second(self.total_input_bytes(), self.total_vyre_wall_ns())
    }

    /// Validates that this report is usable as release evidence.
    pub fn validate_release_evidence(
        &self,
        expected_target_id: &str,
        expected_commit: &str,
        expected_translation_units: usize,
    ) -> Result<(), String> {
        if self.target_id != expected_target_id {
            return Err(format!(
                "preprocess benchmark target mismatch: got {}, expected {}",
                self.target_id, expected_target_id
            ));
        }
        if self.source_commit != expected_commit {
            return Err(format!(
                "preprocess benchmark commit mismatch: got {}, expected {}",
                self.source_commit, expected_commit
            ));
        }
        if self.translation_units.len() != expected_translation_units {
            return Err(format!(
                "preprocess benchmark TU count mismatch: got {}, expected {}",
                self.translation_units.len(),
                expected_translation_units
            ));
        }
        if self.target_triple.trim().is_empty() {
            return Err("preprocess benchmark missing target triple".to_string());
        }
        if self.clang_version.trim().is_empty() {
            return Err("preprocess benchmark missing clang version".to_string());
        }
        if self.vyre_version.trim().is_empty() {
            return Err("preprocess benchmark missing vyre version".to_string());
        }
        if self.gpu.kernel_launch_count == 0 {
            return Err("preprocess benchmark recorded zero GPU launches".to_string());
        }
        if self.gpu.host_write_bytes == 0 {
            return Err("preprocess benchmark recorded zero host write bytes".to_string());
        }
        if self.gpu.host_readback_bytes == 0 {
            return Err("preprocess benchmark recorded zero host readback bytes".to_string());
        }
        for unit in &self.translation_units {
            if unit.path.trim().is_empty() {
                return Err("preprocess benchmark has an empty TU path".to_string());
            }
            if unit.input_bytes == 0 {
                return Err(format!(
                    "preprocess benchmark {} has zero input bytes",
                    unit.path
                ));
            }
            if unit.clang_output_bytes == 0 {
                return Err(format!(
                    "preprocess benchmark {} has zero clang output bytes",
                    unit.path
                ));
            }
            if unit.vyre_output_bytes == 0 {
                return Err(format!(
                    "preprocess benchmark {} has zero vyre output bytes",
                    unit.path
                ));
            }
            if unit.clang_wall_ns == 0 {
                return Err(format!(
                    "preprocess benchmark {} has zero clang wall time",
                    unit.path
                ));
            }
            if unit.vyre_wall_ns == 0 {
                return Err(format!(
                    "preprocess benchmark {} has zero vyre wall time",
                    unit.path
                ));
            }
        }
        Ok(())
    }
}

fn bytes_per_second(bytes: u64, wall_ns: u64) -> u64 {
    ((bytes as u128 * 1_000_000_000_u128) / wall_ns.max(1) as u128) as u64
}
