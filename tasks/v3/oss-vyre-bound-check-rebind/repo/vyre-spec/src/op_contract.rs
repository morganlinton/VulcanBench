//! Optional operation-contract metadata shared by signatures and catalogs.

/// Backend capability required by an operation.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize)]
pub struct CapabilityId(pub String);

impl CapabilityId {
    /// Create a capability id from a stable name.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// Return the stable capability name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Determinism contract for an operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize)]
#[non_exhaustive]
pub enum DeterminismClass {
    /// Bit-identical outputs for identical inputs.
    Deterministic,
    /// Deterministic except for backend rounding policy.
    DeterministicModuloRounding,
    /// Backend scheduling or hardware effects may change results.
    NonDeterministic,
}

/// Side-effect class for an operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize)]
#[non_exhaustive]
pub enum SideEffectClass {
    /// Pure value computation.
    Pure,
    /// Reads memory through explicit operands.
    ReadsMemory,
    /// Writes memory through explicit operands.
    WritesMemory,
    /// Performs synchronization.
    Synchronizing,
    /// Performs atomic memory effects.
    Atomic,
}

/// Portable cost hint for planning and diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize)]
#[non_exhaustive]
pub enum CostHint {
    /// Cheap scalar or metadata operation.
    Cheap,
    /// Medium-cost operation.
    Medium,
    /// Expensive operation.
    Expensive,
    /// Cost depends on backend or runtime data.
    Unknown,
}

/// Optional contract annotations for operation declarations.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize)]
pub struct OperationContract {
    /// Required backend capabilities.
    #[serde(default)]
    pub capability_requirements: Option<smallvec::SmallVec<[CapabilityId; 4]>>,
    /// Determinism class.
    #[serde(default)]
    pub determinism: Option<DeterminismClass>,
    /// Side-effect class.
    #[serde(default)]
    pub side_effect: Option<SideEffectClass>,
    /// Portable cost hint.
    #[serde(default)]
    pub cost_hint: Option<CostHint>,
}

impl OperationContract {
    /// Empty contract for declarations that have not been annotated yet.
    #[must_use]
    pub const fn none() -> Self {
        Self {
            capability_requirements: None,
            determinism: None,
            side_effect: None,
            cost_hint: None,
        }
    }
}

impl Default for OperationContract {
    fn default() -> Self {
        Self::none()
    }
}
