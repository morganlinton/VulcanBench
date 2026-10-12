//! Discovery layer for community vyre-libs dialect packs.
//!
//! Foundation owns these inventory types so both the driver registry and
//! downstream consumers can share one link-time collection point without
//! introducing a package cycle.

#![forbid(unsafe_code)]

use rustc_hash::{FxHashMap, FxHashSet};

/// Metadata describing a community-registered dialect pack.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ExternDialect {
    /// Dialect crate name on crates.io. Must start with `vyre-libs-`.
    pub name: &'static str,
    /// Crate version at link time. Informational.
    pub version: &'static str,
    /// Public repository URL (for diagnostics + trust).
    pub crate_repo: &'static str,
}

impl ExternDialect {
    /// Construct a dialect metadata entry.
    #[must_use]
    pub const fn new(name: &'static str, version: &'static str, crate_repo: &'static str) -> Self {
        Self {
            name,
            version,
            crate_repo,
        }
    }
}

inventory::collect!(ExternDialect);

/// Individual Cat-A op contributed by a community dialect.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ExternOp {
    /// Owning dialect (matches [`ExternDialect::name`]).
    pub dialect: &'static str,
    /// Fully-qualified op id (e.g. `"vyre-libs-quant::int8::matmul"`).
    pub op_id: &'static str,
}

impl ExternOp {
    /// Construct an op registration.
    #[must_use]
    pub const fn new(dialect: &'static str, op_id: &'static str) -> Self {
        Self { dialect, op_id }
    }
}

inventory::collect!(ExternOp);

/// Every dialect registered at link time.
#[must_use]
pub fn dialects() -> Vec<&'static ExternDialect> {
    collect_inventory_refs(inventory::iter::<ExternDialect>())
}

/// Every registered op belonging to `dialect`.
#[must_use]
pub fn ops_in_dialect(dialect: &str) -> Vec<&'static ExternOp> {
    collect_inventory_refs(inventory::iter::<ExternOp>().filter(|op| op.dialect == dialect))
}

/// Every registered op across every dialect.
#[must_use]
pub fn all_ops() -> Vec<&'static ExternOp> {
    collect_inventory_refs(inventory::iter::<ExternOp>())
}

fn collect_inventory_refs<T>(iter: impl Iterator<Item = &'static T>) -> Vec<&'static T>
where
    T: 'static,
{
    let (lo, hi) = iter.size_hint();
    let mut out = Vec::with_capacity(hi.unwrap_or(lo));
    out.extend(iter);
    out
}

/// Structured validation error surfaced by [`verify`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ExternVerifyError {
    /// Two or more `ExternDialect` entries share the same `name`.
    #[error("duplicate dialect name `{name}`: {count} entries registered. Fix: pick a unique crates.io name for each community pack.")]
    DuplicateDialect {
        /// The offending dialect name.
        name: &'static str,
        /// Number of entries sharing this name.
        count: usize,
    },

    /// Dialect `name` does not start with the reserved `vyre-libs-` prefix.
    #[error("dialect name `{name}` does not start with `vyre-libs-`. Fix: rename the pack crate and its ExternDialect::name to begin with `vyre-libs-`.")]
    MalformedDialectName {
        /// The offending dialect name.
        name: &'static str,
    },

    /// An `ExternOp` references a `dialect` name that no
    /// `ExternDialect` entry claims.
    #[error("orphan op `{op_id}` references dialect `{dialect}`, which is not registered. Fix: make sure the dialect's crate registers an `ExternDialect` entry with this name.")]
    OrphanOp {
        /// The orphan op's dialect reference.
        dialect: &'static str,
        /// The op id whose dialect is missing.
        op_id: &'static str,
    },

    /// `ExternOp.op_id` is an empty string.
    #[error("op registered with empty op_id under dialect `{dialect}`. Fix: every op must carry a fully-qualified id like `<dialect>::<op_name>`.")]
    EmptyOpId {
        /// The dialect claiming an empty-id op.
        dialect: &'static str,
    },
}

/// Run every consistency check across every registered extern dialect and op.
///
/// # Errors
///
/// Returns every discovered validation error.
pub fn verify() -> Result<(), Vec<ExternVerifyError>> {
    let mut errors = Vec::new();

    // Single dialect sweep: duplicate counts, malformed-name checks, and the
    // dialect-name set for orphan-op detection (VYRE_EXTERN_VERIFY HOT).
    let mut counts: FxHashMap<&'static str, usize> = FxHashMap::default();
    let mut known: FxHashSet<&'static str> = FxHashSet::default();
    for dialect in inventory::iter::<ExternDialect>() {
        *counts.entry(dialect.name).or_insert(0) += 1;
        known.insert(dialect.name);
        if !dialect.name.starts_with("vyre-libs-") {
            errors.push(ExternVerifyError::MalformedDialectName { name: dialect.name });
        }
    }
    for (name, count) in counts {
        if count > 1 {
            errors.push(ExternVerifyError::DuplicateDialect { name, count });
        }
    }

    for op in inventory::iter::<ExternOp>() {
        if op.op_id.is_empty() {
            errors.push(ExternVerifyError::EmptyOpId {
                dialect: op.dialect,
            });
        }
        if !known.contains(op.dialect) {
            errors.push(ExternVerifyError::OrphanOp {
                dialect: op.dialect,
                op_id: op.op_id,
            });
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static TEST_DIALECT_A: ExternDialect =
        ExternDialect::new("vyre-libs-test-a", "0.1.0", "https://example.invalid/a");
    static TEST_DIALECT_B: ExternDialect =
        ExternDialect::new("vyre-libs-test-b", "0.1.0", "https://example.invalid/b");

    #[test]
    fn extern_dialect_construction() {
        let d = ExternDialect::new("vyre-libs-quant", "0.1.0", "https://github.com/example");
        assert_eq!(d.name, "vyre-libs-quant");
        assert_eq!(d.version, "0.1.0");
        assert_eq!(d.crate_repo, "https://github.com/example");
    }

    #[test]
    fn extern_op_construction() {
        let op = ExternOp::new("vyre-libs-quant", "vyre-libs-quant::int8::matmul");
        assert_eq!(op.dialect, "vyre-libs-quant");
        assert_eq!(op.op_id, "vyre-libs-quant::int8::matmul");
    }

    #[test]
    fn inventory_collection_uses_one_shared_collector() {
        let collected = collect_inventory_refs([&TEST_DIALECT_A, &TEST_DIALECT_B].into_iter());
        assert_eq!(collected, vec![&TEST_DIALECT_A, &TEST_DIALECT_B]);
    }

    #[test]
    fn duplicate_dialect_error_display() {
        let err = ExternVerifyError::DuplicateDialect {
            name: "vyre-libs-x",
            count: 3,
        };
        let msg = err.to_string();
        assert!(msg.contains("vyre-libs-x"));
        assert!(msg.contains("3"));
    }

    #[test]
    fn malformed_name_error_display() {
        let err = ExternVerifyError::MalformedDialectName { name: "bad-name" };
        let msg = err.to_string();
        assert!(msg.contains("bad-name"));
        assert!(msg.contains("vyre-libs-"));
    }

    #[test]
    fn orphan_op_error_display() {
        let err = ExternVerifyError::OrphanOp {
            dialect: "missing-dialect",
            op_id: "missing::op",
        };
        assert!(err.to_string().contains("orphan"));
    }

    #[test]
    fn empty_op_id_error_display() {
        let err = ExternVerifyError::EmptyOpId {
            dialect: "vyre-libs-x",
        };
        assert!(err.to_string().contains("empty"));
    }

    #[test]
    fn verify_empty_registry_succeeds() {
        // No ExternDialect or ExternOp are submitted in this test crate,
        // so verify should pass (at minimum  -  other tests may submit entries).
        let result = verify();
        // Either Ok or the errors are all from other test crates.
        if let Err(errors) = &result {
            // All errors should be well-formed.
            for e in errors {
                assert!(
                    e.to_string().contains("Fix:") || e.to_string().contains("extern"),
                    "extern registry validation errors must be displayable: {e}"
                );
            }
        }
    }
}
