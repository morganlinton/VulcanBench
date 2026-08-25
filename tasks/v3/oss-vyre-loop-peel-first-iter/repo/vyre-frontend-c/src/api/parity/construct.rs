use super::*;
/// Release status for a C construct encountered in the frozen subsystem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ParityConstructStatus {
    /// The construct has a production implementation and matched the oracle facts.
    Implemented,
    /// The construct is explicitly approved as out of scope for this release.
    ApprovedOutOfScope,
    /// The construct was encountered without an implementation or release approval.
    Unresolved,
}

/// Unsupported-construct evidence from the subsystem parity run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParityUnsupportedConstruct {
    /// Fact category that needs the construct.
    pub category: ParityFactCategory,
    /// Construct name, such as `gnu_statement_expression` or `asm_goto`.
    pub construct: String,
    /// Stable location or fact identifier for the encountered construct.
    pub location: String,
    /// Release status for the construct.
    pub status: ParityConstructStatus,
    /// Human-readable detail or approval reason.
    pub detail: String,
}

impl ParityUnsupportedConstruct {
    /// Creates construct evidence.
    #[must_use]
    pub fn new(
        category: ParityFactCategory,
        construct: impl Into<String>,
        location: impl Into<String>,
        status: ParityConstructStatus,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            category,
            construct: construct.into(),
            location: location.into(),
            status,
            detail: detail.into(),
        }
    }
}
