//! Compatibility alias registry for public shim paths.
//!
//! Every compatibility path keeps one metadata row here so facade docs,
//! release gates, and internal import audits agree on the canonical owner.

/// Metadata for one public compatibility alias.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompatibilityAlias {
    /// Deprecated public path retained for transition consumers.
    pub deprecated_path: &'static str,
    /// Canonical path that internal code and new consumers should import.
    pub canonical_path: &'static str,
    /// Module that owns the real implementation.
    pub canonical_owner: &'static str,
    /// Concrete condition that must be true before removing the alias.
    pub removal_condition: &'static str,
}

/// Root compatibility alias for the former matching dialect name.
pub const MATCHING_ALIAS: CompatibilityAlias = CompatibilityAlias {
    deprecated_path: "vyre_libs::matching",
    canonical_path: "vyre_libs::scan",
    canonical_owner: "vyre-libs/src/scan",
    removal_condition: "public-api snapshot and downstream compatibility tests no longer require vyre_libs::matching",
};

/// Leaf compatibility alias for the former substring path.
pub const MATCHING_SUBSTRING_ALIAS: CompatibilityAlias = CompatibilityAlias {
    deprecated_path: "vyre_libs::matching::substring",
    canonical_path: "vyre_libs::scan::substring",
    canonical_owner: "vyre-libs/src/scan/substring",
    removal_condition: "public-api snapshot and substring compatibility test no longer require vyre_libs::matching::substring",
};

/// All compatibility aliases exposed by `vyre-libs`.
pub const COMPATIBILITY_ALIASES: &[CompatibilityAlias] =
    &[MATCHING_ALIAS, MATCHING_SUBSTRING_ALIAS];
