//! Frozen dialect + op registration surface.
//!
//! **Single registration path.** Every op known to any vyre backend is
//! registered exactly once by submitting an [`OpDefRegistration`] via
//! `inventory::submit!`. There is no legacy registry shim, no parallel macro-generated
//! catalog, no alternative discovery mechanism. If an op isn't in the
//! `DialectRegistry` it doesn't exist; the wire decoder returns
//! [`vyre_foundation::error::Error::UnknownOp`] for it.
//!
//! Dialects register themselves via [`DialectRegistration`]. The per-op
//! [`OpBackendTarget`] rows are a pure metadata index so the coverage
//! matrix can answer "does this op have a primary-text path declared" without
//! loading every backend's lowering table.
//!
//! Extension crates adding new ops do not edit this module. They submit
//! their own `OpDefRegistration` from their own `inventory::submit!` macro
//! call at link time. Vision doc §2 (Open hierarchies) enforces this.

use super::op_def::OpDef;

/// Metadata for a versioned dialect of ops (e.g. `pattern`, `crypto`).
pub struct Dialect {
    /// Stable dialect id (e.g. `"primitive"`, `"hash"`).
    pub id: &'static str,
    /// Dialect schema version.
    pub version: u32,
    /// Optional parent dialect this one extends.
    pub parent: Option<&'static str>,
    /// Op ids this dialect publishes.
    pub ops: &'static [&'static str],
    /// Pass/fail predicate run at registration time.
    pub validator: fn() -> bool,
    /// Backends this dialect requires for conformance claims.
    pub backends_required: &'static [vyre_spec::Backend],
}

/// Default `Dialect::validator` that always succeeds.
pub fn default_validator() -> bool {
    true
}

/// Advertises that a named backend target publishes a lowering for a
/// specific op. This is distinct from [`crate::backend::BackendRegistration`]
/// (which registers a full `VyreBackend` implementation): it is a compact
/// (op, target) pair used by the coverage-matrix introspection path to list
/// "this op has a concrete lowering path declared".
pub struct OpBackendTarget {
    /// Op id (e.g. `primitive.math.add`).
    pub op: &'static str,
    /// Backend-target id.
    pub target: &'static str,
}

/// **The single op registration path.**
///
/// Every op a vyre backend can execute is published by submitting one of
/// these through `inventory::submit!` at link time. The closure is called
/// once during registry freeze to build the stable [`OpDef`] record.
pub struct OpDefRegistration {
    /// Factory returning the op's frozen [`OpDef`].
    pub op: fn() -> OpDef,
}

impl OpDefRegistration {
    /// Construct a registration from an op-factory function.
    pub const fn new(op: fn() -> OpDef) -> Self {
        Self { op }
    }
}

/// Register a dialect (op id namespace + version) via `inventory::submit!`.
pub struct DialectRegistration {
    /// Factory returning the dialect's frozen record.
    pub dialect: fn() -> Dialect,
}

inventory::collect!(OpDefRegistration);
inventory::collect!(DialectRegistration);
inventory::collect!(OpBackendTarget);
