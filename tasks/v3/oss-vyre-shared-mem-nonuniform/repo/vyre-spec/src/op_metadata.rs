//! Frozen operation metadata records consumed by catalogs and docs generators.

use crate::{layer::Layer, metadata_category::MetadataCategory, op_contract::OperationContract};

/// Location-agnostic metadata declared inside each operation file.
///
/// Example: a `primitive.math.add` record stores its stable id, layer,
/// category, semantic version, signature text, and strictness policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpMetadata {
    /// Stable operation identifier.
    pub id: &'static str,
    /// Logical conformance layer. This is metadata, not a filesystem rule.
    pub layer: Layer,
    /// Location-agnostic conformance category for the operation catalog.
    pub category: MetadataCategory,
    /// Behavior version. Increment when semantics change.
    pub version: u32,
    /// Short operation description.
    pub description: &'static str,
    /// Human-readable operation signature.
    pub signature: &'static str,
    /// Strictness policy label.
    pub strictness: &'static str,
    /// Archetype signature from the conform vocabulary.
    pub archetype_signature: &'static str,
    /// Optional capability and execution contract annotations.
    pub contract: Option<OperationContract>,
}
