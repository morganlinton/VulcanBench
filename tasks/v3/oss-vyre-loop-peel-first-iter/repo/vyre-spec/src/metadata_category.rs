//! Frozen metadata category tags emitted into generated operation catalogs.

/// High-level operation category for generated catalogs in the frozen contract.
///
/// Example: `MetadataCategory::Intrinsic` marks an operation whose implementation is a
/// hardware intrinsic path.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize)]
#[non_exhaustive]
pub enum MetadataCategory {
    /// Category A: zero-overhead composition or handwritten equivalent.
    A,
    /// Category B: guarded tripwire path.
    B,
    /// Category C: hardware intrinsic path.
    C,
    /// Catalog producer did not assign a category.
    Unclassified,
}

impl MetadataCategory {
    /// Stable category label.
    #[must_use]
    pub const fn category_id(&self) -> &'static str {
        match self {
            Self::A => "A",
            Self::B => "B",
            Self::C => "C",
            Self::Unclassified => "unclassified",
        }
    }
}
