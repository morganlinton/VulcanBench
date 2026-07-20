//! Frozen conformance-layer tags used by operation metadata.

/// Conformance layer declared by operation metadata in the frozen contract.
///
/// Example: `Layer::L2` records that an operation belongs to the byte-oriented
/// library-operation layer.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize)]
#[non_exhaustive]
pub enum Layer {
    /// L0: published wire-format and data-model contracts.
    L0,
    /// L1: primitive scalar and bit-level operations.
    L1,
    /// L2: byte-oriented library operations.
    L2,
    /// L3: structured algorithms and graph-like operations.
    L3,
    /// L4: mutation-gated composition surfaces.
    L4,
    /// L5: adversarial and stability-hardened operations.
    L5,
}

impl Layer {
    /// Stable layer identifier for generated documentation.
    #[must_use]
    pub const fn id(&self) -> &'static str {
        match self {
            Self::L0 => "L0",
            Self::L1 => "L1",
            Self::L2 => "L2",
            Self::L3 => "L3",
            Self::L4 => "L4",
            Self::L5 => "L5",
        }
    }

    /// Human-readable layer description for generated documentation.
    #[must_use]
    pub const fn layer_description(&self) -> &'static str {
        match self {
            Self::L0 => "Wire-format and data-model contracts",
            Self::L1 => "Primitive scalar and bit-level operations",
            Self::L2 => "Byte-oriented library operations",
            Self::L3 => "Structured algorithms and graph-like operations",
            Self::L4 => "Mutation-gated composition surfaces",
            Self::L5 => "Adversarial and stability-hardened operations",
        }
    }
}
