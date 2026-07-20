use super::*;
/// Comparable parity fact emitted by either clang or vyrec.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParityComparableFact {
    /// Fact category.
    pub category: ParityFactCategory,
    /// Stable fact identifier.
    pub fact_id: String,
    /// Stable semantic digest or canonical semantic payload.
    pub semantic_digest: String,
    /// Normalized source provenance.
    pub provenance: Option<ParitySourceProvenance>,
}

impl ParityComparableFact {
    /// Creates a comparable fact.
    #[must_use]
    pub fn new(
        category: ParityFactCategory,
        fact_id: impl Into<String>,
        semantic_digest: impl Into<String>,
        provenance: Option<ParitySourceProvenance>,
    ) -> Self {
        Self {
            category,
            fact_id: fact_id.into(),
            semantic_digest: semantic_digest.into(),
            provenance,
        }
    }
}
