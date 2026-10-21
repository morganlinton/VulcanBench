use super::*;
/// Comparator result class for one fact or release proof.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ParityFindingKind {
    /// clang and vyrec agree for this fact.
    Match,
    /// Difference is explicitly approved for this release target.
    ExplainedTargetDifference,
    /// vyrec failed to produce a required fact.
    VyrecMissing,
    /// vyrec produced an unsupported or unjustified extra fact.
    VyrecExtra,
    /// Meaning agrees, but source span or provenance differs.
    SpanMismatch,
    /// Meaning differs.
    SemanticMismatch,
    /// Diagnostic severity, category, recovery, or location differs.
    DiagnosticMismatch,
    /// Performance contract failed.
    PerformanceFailure,
    /// GPU residency contract failed.
    GpuResidencyFailure,
}

impl ParityFindingKind {
    /// Returns whether this finding blocks public release.
    #[must_use]
    pub const fn blocks_release(self) -> bool {
        matches!(
            self,
            Self::VyrecMissing
                | Self::VyrecExtra
                | Self::SpanMismatch
                | Self::SemanticMismatch
                | Self::DiagnosticMismatch
                | Self::PerformanceFailure
                | Self::GpuResidencyFailure
        )
    }
}

/// One comparator finding in a release parity report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParityFinding {
    /// Fact category being compared.
    pub category: ParityFactCategory,
    /// Comparator result class.
    pub kind: ParityFindingKind,
    /// Stable fact identifier from the parity manifest or comparator.
    pub fact_id: String,
    /// Human-readable detail that explains the failure or approved difference.
    pub detail: String,
}

impl ParityFinding {
    /// Creates one comparator finding.
    #[must_use]
    pub fn new(
        category: ParityFactCategory,
        kind: ParityFindingKind,
        fact_id: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            category,
            kind,
            fact_id: fact_id.into(),
            detail: detail.into(),
        }
    }

    /// Returns whether this finding blocks public release.
    #[must_use]
    pub const fn blocks_release(&self) -> bool {
        self.kind.blocks_release()
    }
}
