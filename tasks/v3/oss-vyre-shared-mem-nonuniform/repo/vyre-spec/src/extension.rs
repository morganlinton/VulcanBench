//! Extension contracts for open IR.
//!
//! Downstream crates ship new `Expr`, `Node`, `DataType`, `BinOp`, `UnOp`,
//! `AtomicOp`, `TernaryOp`, and `RuleCondition` variants by implementing the
//! traits in this module and registering an id with the vyre-core inventory
//! layer.
//!
//! `vyre-spec` is intentionally data-only and carries no dependency on
//! `inventory`. The trait signatures below describe the stable contract;
//! actual registration + resolution lives in `vyre::dialect::extension`
//! (see the vyre-core crate).
//!
//! Every extension id occupies the range `0x8000_0000..=0xFFFF_FFFF`  -  the
//! high bit of the wire tag distinguishes extension ids from the frozen
//! core tag space `0x00..=0x7F`. The `ExtensionDataTypeId::from_name`
//! constructor folds a stable crate-name hash into the reserved range so
//! two independently-authored extensions collide only on deliberate
//! name-clashes.

use core::fmt::Debug;

macro_rules! impl_extension_id {
    ($id:ident) => {
        impl $id {
            /// Reserved range: every extension id has its high bit set.
            ///
            /// Core IR discriminants occupy `0x00..=0x7F`; extensions occupy
            /// `0x80..=0xFFFF_FFFF`. Wire decoders test the high byte to route
            /// decoding between the two.
            pub const EXTENSION_RANGE_MASK: u32 = 0x8000_0000;

            /// Construct an id from a stable extension name.
            ///
            /// The id is derived deterministically with FNV-1a and folded into
            /// the extension range by setting the high bit. Callers that pass
            /// the same `name` always get the same id.
            #[must_use]
            pub const fn from_name(name: &str) -> Self {
                Self(fnv1a_with_high_bit(name))
            }

            /// Return the raw id.
            #[must_use]
            pub const fn as_u32(self) -> u32 {
                self.0
            }

            /// Is this a reserved extension id (high bit set)?
            #[must_use]
            pub const fn is_extension(self) -> bool {
                (self.0 & Self::EXTENSION_RANGE_MASK) != 0
            }
        }
    };
}

/// Stable u32 id for an extension variant.
///
/// Extension ids are generated deterministically from a stable name via
/// [`ExtensionDataTypeId::from_name`]. A crate that never changes its
/// extension name keeps the same id across versions, which is the
/// wire-format contract: a `Program` encoded by v1.0 of an extension
/// decodes identically in v1.1 so long as the name is stable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct ExtensionDataTypeId(pub u32);

impl_extension_id!(ExtensionDataTypeId);

/// The contract for an extension-declared `DataType`.
///
/// An implementer describes the runtime shape of a non-core data type:
/// how many bytes it occupies, whether it participates in the float
/// conformance family, and how it should be displayed.
///
/// vyre-core walks a link-time inventory of `ExtensionDataTypeRegistration`
/// entries to resolve a `DataType::Opaque(id)` back to the trait vtable.
/// The resolver caches `&'static dyn ExtensionDataType` so downstream
/// consumers never re-consult the registry on the hot path.
pub trait ExtensionDataType: Send + Sync + Debug + 'static {
    /// Stable id for this data type.
    fn id(&self) -> ExtensionDataTypeId;
    /// Human-readable name for display / debug.
    fn display_name(&self) -> &'static str;
    /// Minimum byte count to represent one value of this type.
    fn min_bytes(&self) -> usize;
    /// Maximum byte count for one value of this type; `None` when unbounded.
    fn max_bytes(&self) -> Option<usize>;
    /// Fixed element size in bytes, or `None` for variable-size types.
    fn size_bytes(&self) -> Option<usize>;
    /// Whether this type belongs to the IEEE-754 float conformance family.
    fn is_float_family(&self) -> bool {
        false
    }
    /// Whether values can be safely memcpy'd between host and device.
    fn is_host_shareable(&self) -> bool {
        true
    }
}

/// Runtime contract for an extension-declared binary operator.
///
/// Vyre-core's resolver caches `&'static dyn ExtensionBinOp` pointers keyed
/// by [`ExtensionBinOpId`]; downstream evaluators / lowerings call through
/// this trait without re-consulting the registry on the hot path.
pub trait ExtensionBinOp: Send + Sync + Debug + 'static {
    /// Stable id of this binary operator.
    fn id(&self) -> ExtensionBinOpId;
    /// Human-readable name for display / debug.
    fn display_name(&self) -> &'static str;
    /// Evaluate on the reference (CPU) backend.
    ///
    /// Returning `None` means "this backend does not support the op"; the
    /// caller surfaces a typed error. Extensions implementing backends
    /// other than reference supply their own lowering via the backend
    /// registry.
    fn eval_u32(&self, _a: u32, _b: u32) -> Option<u32> {
        None
    }
}

/// Runtime contract for an extension-declared unary operator.
pub trait ExtensionUnOp: Send + Sync + Debug + 'static {
    /// Stable id of this unary operator.
    fn id(&self) -> ExtensionUnOpId;
    /// Human-readable name for display / debug.
    fn display_name(&self) -> &'static str;
    /// Evaluate on the reference (CPU) backend. `None` = unsupported.
    fn eval_u32(&self, _a: u32) -> Option<u32> {
        None
    }
}

/// Runtime contract for an extension-declared atomic operator.
pub trait ExtensionAtomicOp: Send + Sync + Debug + 'static {
    /// Stable id of this atomic operator.
    fn id(&self) -> ExtensionAtomicOpId;
    /// Human-readable name for display / debug.
    fn display_name(&self) -> &'static str;
}

/// Runtime contract for an extension-declared ternary operator.
pub trait ExtensionTernaryOp: Send + Sync + Debug + 'static {
    /// Stable id of this ternary operator.
    fn id(&self) -> ExtensionTernaryOpId;
    /// Human-readable name for display / debug.
    fn display_name(&self) -> &'static str;
}

/// Stable u32 id for an extension binary operator.
///
/// Identical discipline to [`ExtensionDataTypeId`]: stable across process
/// runs, high bit set, generated by FNV-1a of the extension name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct ExtensionBinOpId(pub u32);

impl_extension_id!(ExtensionBinOpId);

/// Stable u32 id for an extension unary operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct ExtensionUnOpId(pub u32);

impl_extension_id!(ExtensionUnOpId);

/// Stable u32 id for an extension atomic operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct ExtensionAtomicOpId(pub u32);

impl_extension_id!(ExtensionAtomicOpId);

/// Stable u32 id for an extension ternary operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct ExtensionTernaryOpId(pub u32);

impl_extension_id!(ExtensionTernaryOpId);

/// Stable u32 id for an extension rule condition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct ExtensionRuleConditionId(pub u32);

impl_extension_id!(ExtensionRuleConditionId);

/// FNV-1a 32-bit hash folded into the extension range (high bit set).
///
/// Shared helper backing every `ExtensionXxxId::from_name`. Kept private
/// so callers don't construct raw ids that bypass the high-bit invariant.
#[must_use]
const fn fnv1a_with_high_bit(name: &str) -> u32 {
    let mut hash: u32 = 0x811c_9dc5;
    let bytes = name.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        hash ^= bytes[i] as u32;
        hash = hash.wrapping_mul(0x0100_0193);
        i += 1;
    }
    hash | 0x8000_0000
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_from_name_is_deterministic() {
        assert_eq!(
            ExtensionDataTypeId::from_name("tensor.gather"),
            ExtensionDataTypeId::from_name("tensor.gather"),
        );
    }

    #[test]
    fn id_from_different_names_differ() {
        let a = ExtensionDataTypeId::from_name("tensor.gather");
        let b = ExtensionDataTypeId::from_name("tensor.scatter");
        assert_ne!(a, b);
    }

    #[test]
    fn every_id_is_in_extension_range() {
        let id = ExtensionDataTypeId::from_name("anything");
        assert!(id.is_extension(), "{:#010x} missing high bit", id.as_u32());
        assert!(id.as_u32() & ExtensionDataTypeId::EXTENSION_RANGE_MASK != 0);
    }
}
