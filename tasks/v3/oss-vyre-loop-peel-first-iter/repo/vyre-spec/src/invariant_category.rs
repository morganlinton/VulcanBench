//! Frozen invariant-category tags used for catalog grouping and reporting.

/// Coarse grouping of invariants by concern in the frozen data contract.
///
/// Example: `InvariantCategory::Resource` groups bounded allocation, no panic,
/// and no undefined-behavior invariants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum InvariantCategory {
    /// Properties of a single program run.
    Execution,
    /// Properties of the law and composition layer.
    Algebra,
    /// Bounds on allocations, panics, and undefined behaviour.
    Resource,
    /// Stability guarantees across versions.
    Stability,
}
