//! Op versioning, attribute migration, and deprecation registration.
//!
//! Ops evolve. `math.add@1` may gain an `overflow_behavior` attribute
//! in `math.add@2` and rename `mode` in the process. Payloads encoded
//! against v1 must still decode on a runtime that only knows v2.
//!
//! This module carries three inventory-collected registries:
//!
//! * [`Migration`]  -  a one-step rewrite from `(op_id, from_version)`
//!   to `(op_id, to_version)` operating on an [`AttrMap`]. Migrations
//!   chain automatically: if v1→v2 and v2→v3 are registered, a v1
//!   payload decodes as v3.
//! * [`Deprecation`]  -  marks an op as deprecated since a specific
//!   version, with a note that becomes part of the
//!   [`deprecation_diagnostic`] warning surfaced to the caller.
//! * Decoders consult these tables before validating an op against the
//!   final schema. This module ships the registries and public API so
//!   dialect crates register migrations next to the evolving op.
//!
//! Design notes:
//!
//! * Attribute values are typed (see [`AttrValue`]). A migration can
//!   inspect the existing shape before rewriting  -  no stringly-typed
//!   dance inside the hot decode path.
//! * Migrations are `fn` pointers, not closures. This keeps
//!   `Migration` `'static` and safe to stash behind `inventory::iter`.
//! * Chain resolution stops at the highest version reachable. An
//!   absent further migration is a terminal state, not an error.

use std::borrow::Cow;
use std::sync::OnceLock;

use crate::diagnostics::{Diagnostic, OpLocation, Severity};
use rustc_hash::FxHashMap;

/// Semantic version triple used for op versioning.
///
/// The registry's current `Dialect::version` is still a single `u32`;
/// the triple form is the canonical representation for per-op
/// evolution: minor bumps are backward-compatible additions and patch
/// bumps are bug fixes. The `Ord` impl is lexicographic major→minor→
/// patch so ordinary comparison works for chain resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Semver {
    /// Breaking-change counter.
    pub major: u32,
    /// Backwards-compatible-feature counter.
    pub minor: u32,
    /// Patch counter.
    pub patch: u32,
}

impl Semver {
    /// Construct a new semver triple.
    #[must_use]
    pub const fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }
}

impl std::fmt::Display for Semver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Typed attribute value carried in an [`AttrMap`].
///
/// The tags match [`crate::AttrType`] one-to-one so
/// a migration can round-trip an attribute through the op's schema
/// without losing type information.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum AttrValue {
    /// Unsigned 32-bit integer.
    U32(u32),
    /// Signed 32-bit integer.
    I32(i32),
    /// 32-bit float.
    F32(f32),
    /// Boolean flag.
    Bool(bool),
    /// Opaque byte blob.
    Bytes(Vec<u8>),
    /// UTF-8 string.
    String(String),
}

/// Mutable attribute bag passed to [`Migration::rewrite`].
///
/// The migration typically renames keys, coerces values, or
/// inserts defaults for newly-introduced attributes. The wire
/// decoder constructs one of these per decoded op, hands it to the
/// migration chain, and then validates against the final op's
/// schema.
#[derive(Debug, Default, Clone)]
pub struct AttrMap {
    attrs: FxHashMap<String, AttrValue>,
}

impl AttrMap {
    /// Construct an empty attribute map.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert an attribute, returning the previous value if one
    /// existed.
    pub fn insert(&mut self, key: impl Into<String>, value: AttrValue) -> Option<AttrValue> {
        self.attrs.insert(key.into(), value)
    }

    /// Remove an attribute by key, returning its value if present.
    pub fn remove(&mut self, key: &str) -> Option<AttrValue> {
        self.attrs.remove(key)
    }

    /// Fetch a reference to an attribute value.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&AttrValue> {
        self.attrs.get(key)
    }

    /// Rename an attribute key. No-op when the source key is absent.
    /// Returns `true` when a rename occurred.
    pub fn rename(&mut self, from: &str, to: impl Into<String>) -> bool {
        match self.attrs.remove(from) {
            Some(v) => {
                self.attrs.insert(to.into(), v);
                true
            }
            None => false,
        }
    }

    /// Number of attributes in the map.
    #[must_use]
    pub fn len(&self) -> usize {
        self.attrs.len()
    }

    /// `true` when the map contains no attributes.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.attrs.is_empty()
    }

    /// Iterate `(key, value)` pairs in arbitrary order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &AttrValue)> {
        self.attrs.iter().map(|(k, v)| (k.as_str(), v))
    }
}

/// Structured error returned by a [`Migration::rewrite`] function.
///
/// Migrations are fallible: a required input attribute may be
/// missing, or a coerced value may not fit a narrower type. The
/// error carries enough context for the decoder to surface a
/// [`Diagnostic`] pinned to the offending op.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum MigrationError {
    /// A required attribute was missing from the input map.
    MissingAttribute {
        /// Name of the missing attribute.
        name: String,
    },
    /// An attribute carried the wrong type for the migration.
    WrongType {
        /// Name of the attribute.
        name: String,
        /// The expected type, as a human-readable tag.
        expected: &'static str,
    },
    /// A coerced numeric value did not fit the narrower target type.
    OutOfRange {
        /// Name of the attribute that overflowed.
        name: String,
    },
    /// The migration rejected the input for any other reason.
    Custom {
        /// Human-readable failure reason.
        reason: String,
    },
}

impl std::fmt::Display for MigrationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MigrationError::MissingAttribute { name } => {
                write!(f, "migration needs attribute `{name}` which is missing")
            }
            MigrationError::WrongType { name, expected } => {
                write!(f, "migration expected `{name}` to be {expected}")
            }
            MigrationError::OutOfRange { name } => {
                write!(f, "migration value for `{name}` is out of range")
            }
            MigrationError::Custom { reason } => f.write_str(reason),
        }
    }
}

impl std::error::Error for MigrationError {}

/// One-step migration from `(op_id, from)` to `(op_id, to)`.
///
/// Dialect crates register migrations via:
///
/// ```
/// use vyre_driver::registry::{AttrMap, Migration, MigrationError, Semver};
///
/// fn rename_mode(attrs: &mut AttrMap) -> Result<(), MigrationError> {
///     attrs.rename("mode", "overflow_behavior");
///     Ok(())
/// }
///
/// inventory::submit! {
///     Migration::new(
///         ("math.add", Semver::new(1, 0, 0)),
///         ("math.add", Semver::new(2, 0, 0)),
///         rename_mode,
///     )
/// }
/// ```
///
/// Multiple migrations form a chain. [`MigrationRegistry::apply_chain`]
/// follows the chain to completion.
pub struct Migration {
    /// `(op_id, from_version)`  -  the shape on the wire.
    pub from: (&'static str, Semver),
    /// `(op_id, to_version)`  -  the shape after rewrite.
    pub to: (&'static str, Semver),
    /// The attribute-map rewrite function.
    pub rewrite: fn(&mut AttrMap) -> Result<(), MigrationError>,
}

impl Migration {
    /// Const constructor suited to `inventory::submit!` bodies.
    #[must_use]
    pub const fn new(
        from: (&'static str, Semver),
        to: (&'static str, Semver),
        rewrite: fn(&mut AttrMap) -> Result<(), MigrationError>,
    ) -> Self {
        Self { from, to, rewrite }
    }
}

inventory::collect!(Migration);

/// Deprecation marker registered alongside an op.
///
/// The decoder consults the registry after successfully resolving an
/// op; a hit produces a `Severity::Warning` diagnostic surfaced to
/// the caller. Deprecation is a pure warning  -  decoding still
/// succeeds.
pub struct Deprecation {
    /// The op identifier being deprecated.
    pub op_id: &'static str,
    /// The version at which the deprecation begins.
    pub deprecated_since: Semver,
    /// Human-readable migration note surfaced inside the warning.
    pub note: &'static str,
}

impl Deprecation {
    /// Const constructor suited to `inventory::submit!` bodies.
    #[must_use]
    pub const fn new(op_id: &'static str, deprecated_since: Semver, note: &'static str) -> Self {
        Self {
            op_id,
            deprecated_since,
            note,
        }
    }
}

inventory::collect!(Deprecation);

/// Registry indexing migrations and deprecations for fast lookup.
///
/// Construction happens lazily on first `global()` call  -  every
/// `inventory::submit!` in the workspace contributes. The registry
/// is immutable after construction.
pub struct MigrationRegistry {
    // Keyed by (op_id, from_version). Value is the single migration
    // registered for that step. Duplicate registrations collapse to
    // the last-inserted for deterministic behavior.
    forward: FxHashMap<(&'static str, Semver), &'static Migration>,
    deprecations: FxHashMap<&'static str, &'static Deprecation>,
}

impl MigrationRegistry {
    /// Process-wide singleton.
    #[must_use]
    pub fn global() -> &'static MigrationRegistry {
        static REGISTRY: OnceLock<MigrationRegistry> = OnceLock::new();
        REGISTRY.get_or_init(|| {
            let migration_count = inventory::iter::<Migration>().count();
            let mut forward = FxHashMap::default();
            let _ = vyre_foundation::allocation::try_reserve_hash_map_to_capacity(
                &mut forward,
                migration_count,
            );
            let migrations = inventory::iter::<Migration>();
            for m in migrations {
                forward.insert((m.from.0, m.from.1), m);
            }
            let deprecation_count = inventory::iter::<Deprecation>().count();
            let mut deprecations = FxHashMap::default();
            vyre_foundation::allocation::try_reserve_hash_map_to_capacity(
                &mut deprecations,
                deprecation_count,
            )
            .ok();
            let deprecation_defs = inventory::iter::<Deprecation>();
            for d in deprecation_defs {
                deprecations.insert(d.op_id, d);
            }
            MigrationRegistry {
                forward,
                deprecations,
            }
        })
    }

    /// Look up a single-step migration for `(op_id, from)`.
    #[must_use]
    pub fn lookup(&self, op_id: &str, from: Semver) -> Option<&'static Migration> {
        self.forward.get(&(op_id, from)).copied()
    }

    /// Follow the migration chain starting at `(op_id, from)` and
    /// rewrite `attrs` in place.
    ///
    /// Returns `(final_op_id, final_version)`  -  the `(op_id, to)`
    /// pair of the last migration applied, or the input `(op_id,
    /// from)` when no migration is registered. A failing rewrite
    /// short-circuits and surfaces the [`MigrationError`].
    ///
    /// # Errors
    ///
    /// Propagates any [`MigrationError`] returned by a migration in
    /// the chain.
    pub fn apply_chain(
        &self,
        op_id: &'static str,
        from: Semver,
        attrs: &mut AttrMap,
    ) -> Result<(&'static str, Semver), MigrationError> {
        let mut current_op = op_id;
        let mut current_ver = from;
        // A migration's `to` is &'static str so we can keep the
        // return type `&'static str` even after chain traversal.
        loop {
            let Some(m) = self.lookup(current_op, current_ver) else {
                return Ok((current_op, current_ver));
            };
            (m.rewrite)(attrs)?;
            current_op = m.to.0;
            current_ver = m.to.1;
        }
    }

    /// Fetch the deprecation marker for an op if one is registered.
    #[must_use]
    pub fn deprecation(&self, op_id: &str) -> Option<&'static Deprecation> {
        self.deprecations.get(op_id).copied()
    }
}

/// Build a `Severity::Warning` diagnostic for a deprecated op.
///
/// The decoder calls this after resolving a deprecated op and pushes
/// the result onto its diagnostic buffer. The caller sees a
/// machine-readable `W-OP-DEPRECATED` warning with the op location
/// and migration note attached as the suggested fix.
#[must_use]
pub fn deprecation_diagnostic(dep: &Deprecation) -> Diagnostic {
    let message = format!(
        "op `{}` is deprecated since version {}",
        dep.op_id, dep.deprecated_since
    );
    Diagnostic {
        severity: Severity::Warning,
        code: crate::diagnostics::DiagnosticCode::new("W-OP-DEPRECATED"),
        message: Cow::Owned(message),
        location: Some(OpLocation::op(dep.op_id.to_owned())),
        suggested_fix: Some(Cow::Borrowed(dep.note)),
        doc_url: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rename_mode_to_overflow(attrs: &mut AttrMap) -> Result<(), MigrationError> {
        if !attrs.rename("mode", "overflow_behavior") {
            return Err(MigrationError::MissingAttribute {
                name: "mode".into(),
            });
        }
        Ok(())
    }

    // Register test-only migrations via inventory. These live in the
    // test build only  -  no `cfg(test)` gate is needed on the
    // inventory::submit! because the tests module itself is gated.
    inventory::submit! {
        Migration::new(
            ("test.op_rename", Semver::new(1, 0, 0)),
            ("test.op_rename", Semver::new(2, 0, 0)),
            rename_mode_to_overflow,
        )
    }

    inventory::submit! {
        Migration::new(
            ("test.op_chain", Semver::new(1, 0, 0)),
            ("test.op_chain", Semver::new(2, 0, 0)),
            |attrs| { attrs.rename("a", "b"); Ok(()) },
        )
    }

    inventory::submit! {
        Migration::new(
            ("test.op_chain", Semver::new(2, 0, 0)),
            ("test.op_chain", Semver::new(3, 0, 0)),
            |attrs| { attrs.rename("b", "c"); Ok(()) },
        )
    }

    inventory::submit! {
        Deprecation::new(
            "test.op_dep",
            Semver::new(1, 1, 0),
            "migrate to test.op_dep2",
        )

    }

    #[test]
    fn registry_finds_registered_migration() {
        let reg = MigrationRegistry::global();
        let m = reg.lookup("test.op_rename", Semver::new(1, 0, 0));
        assert!(m.is_some(), "registered migration must be reachable");
        let m = m.unwrap();
        assert_eq!(m.to.1, Semver::new(2, 0, 0));
    }

    #[test]
    fn apply_chain_rewrites_attributes() {
        let reg = MigrationRegistry::global();
        let mut attrs = AttrMap::new();
        attrs.insert("mode", AttrValue::String("wrap".into()));
        let (op, ver) = reg
            .apply_chain("test.op_rename", Semver::new(1, 0, 0), &mut attrs)
            .expect("Fix: migration registry missing the expected test op; ensure the #[test] fixture's inventory::submit! block is linked in this binary.");
        assert_eq!(op, "test.op_rename");
        assert_eq!(ver, Semver::new(2, 0, 0));
        assert!(attrs.get("mode").is_none());
        assert_eq!(
            attrs.get("overflow_behavior"),
            Some(&AttrValue::String("wrap".into()))
        );
    }

    #[test]
    fn apply_chain_follows_multiple_steps() {
        let reg = MigrationRegistry::global();
        let mut attrs = AttrMap::new();
        attrs.insert("a", AttrValue::U32(1));
        let (_, ver) = reg
            .apply_chain("test.op_chain", Semver::new(1, 0, 0), &mut attrs)
            .expect("Fix: migration registry missing the expected test op; ensure the #[test] fixture's inventory::submit! block is linked in this binary.");
        assert_eq!(ver, Semver::new(3, 0, 0));
        assert!(attrs.get("a").is_none());
        assert!(attrs.get("b").is_none());
        assert_eq!(attrs.get("c"), Some(&AttrValue::U32(1)));
    }

    #[test]
    fn missing_source_attribute_surfaces_error() {
        let reg = MigrationRegistry::global();
        let mut attrs = AttrMap::new();
        let err = reg
            .apply_chain("test.op_rename", Semver::new(1, 0, 0), &mut attrs)
            .expect_err("missing input must error");
        assert!(matches!(err, MigrationError::MissingAttribute { .. }));
    }

    #[test]
    fn no_migration_returns_input_unchanged() {
        let reg = MigrationRegistry::global();
        let mut attrs = AttrMap::new();
        let (op, ver) = reg
            .apply_chain("test.unregistered", Semver::new(1, 0, 0), &mut attrs)
            .expect("Fix: apply_chain on an unregistered op must return Ok(input); if this errors, the no-migration terminal-state contract has regressed.");
        assert_eq!(op, "test.unregistered");
        assert_eq!(ver, Semver::new(1, 0, 0));
    }

    #[test]
    fn deprecation_lookup_returns_marker() {
        let reg = MigrationRegistry::global();
        let dep = reg
            .deprecation("test.op_dep")
            .expect("Fix: test.op_dep deprecation registration missing; verify the fixture's inventory::submit! block is linked.");
        assert_eq!(dep.deprecated_since, Semver::new(1, 1, 0));
        assert_eq!(dep.note, "migrate to test.op_dep2");
    }

    #[test]
    fn deprecation_diagnostic_has_warning_severity() {
        let reg = MigrationRegistry::global();
        let dep = reg.deprecation("test.op_dep").unwrap();
        let diag = deprecation_diagnostic(dep);
        assert_eq!(diag.severity, Severity::Warning);
        assert_eq!(diag.code.as_str(), "W-OP-DEPRECATED");
        assert!(diag.message.contains("test.op_dep"));
        assert!(diag
            .suggested_fix
            .as_ref()
            .map(|s| s.contains("test.op_dep2"))
            .unwrap_or(false));
    }

    #[test]
    fn attr_map_basic_operations() {
        let mut attrs = AttrMap::new();
        assert!(attrs.is_empty());
        attrs.insert("x", AttrValue::Bool(true));
        assert_eq!(attrs.len(), 1);
        assert_eq!(attrs.get("x"), Some(&AttrValue::Bool(true)));
        let prev = attrs.insert("x", AttrValue::Bool(false));
        assert_eq!(prev, Some(AttrValue::Bool(true)));
        let removed = attrs.remove("x");
        assert_eq!(removed, Some(AttrValue::Bool(false)));
        assert!(attrs.is_empty());
    }

    #[test]
    fn semver_ordering_is_lexicographic() {
        assert!(Semver::new(1, 0, 0) < Semver::new(1, 0, 1));
        assert!(Semver::new(1, 0, 5) < Semver::new(1, 1, 0));
        assert!(Semver::new(1, 5, 5) < Semver::new(2, 0, 0));
        assert_eq!(Semver::new(1, 2, 3).to_string(), "1.2.3");
    }
}
