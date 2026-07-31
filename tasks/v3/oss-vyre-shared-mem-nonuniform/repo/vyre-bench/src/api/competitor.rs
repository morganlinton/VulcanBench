use serde::{Deserialize, Serialize};

use super::case::{BenchContext, BenchError, PreparedCase};

/// A pinned competitor implementation for A/B benchmarking.
pub trait CompetitorRun: Send + Sync {
    /// Stable name for the competitor (e.g., "simdjson", "hashbrown").
    fn name(&self) -> &'static str;
    /// Pinned version string (e.g., "3.10.1", "0.16.1").
    fn version(&self) -> &'static str;
    /// Run the competitor workload and return timing + parity data.
    fn run(
        &self,
        ctx: &mut BenchContext,
        prepared: &PreparedCase,
    ) -> Result<CompetitorMetrics, BenchError>;
}

/// Metrics captured from a competitor run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompetitorMetrics {
    /// Wall-clock time in nanoseconds.
    pub wall_ns: u64,
    /// Total bytes processed by the competitor.
    pub bytes_processed: u64,
    /// BLAKE3 hash of the competitor's output for parity checking.
    pub output_hash: String,
}

/// Result of a competitor run, including parity check against Vyre output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompetitorResult {
    /// Competitor name.
    pub name: String,
    /// Competitor version.
    pub version: String,
    /// p50 wall-clock time in nanoseconds.
    pub wall_ns_p50: u64,
    /// Throughput in GB/s.
    pub throughput_gb_s: f64,
    /// BLAKE3 hash of competitor output.
    pub output_hash: String,
    /// Whether the output matches Vyre's output (parity check).
    pub parity: bool,
}
