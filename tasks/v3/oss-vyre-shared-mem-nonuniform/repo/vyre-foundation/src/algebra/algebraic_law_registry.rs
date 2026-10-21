//! P4.1  -  AlgebraicLaw inventory + optimizer dispatch.
//!
//! Ops declare their laws at link time via
//! `inventory::submit!(AlgebraicLawRegistration { … })`; optimizer
//! passes (canonical-form, CSE, rewrite) consult
//! `laws_for_op` to decide how to canonicalize operand order,
//! fold identities, and fuse associative chains.
//!
//! **Contract freeze**  -  the registration struct's shape is the
//! semver boundary. External crates pin against the `{ op_id, law }`
//! form; new fields will be `#[non_exhaustive]`-guarded.

pub use vyre_spec::AlgebraicLaw;

use rustc_hash::FxHashMap;
use std::sync::LazyLock;

/// One algebraic law a specific op satisfies. Registered at link
/// time via `inventory::submit!(AlgebraicLawRegistration { … })`.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct AlgebraicLawRegistration {
    /// Stable op id the law applies to (matches the op's conformance
    /// certificate fingerprint).
    pub op_id: &'static str,
    /// The law itself. Optimizer passes match on the variant.
    pub law: AlgebraicLaw,
}

impl AlgebraicLawRegistration {
    /// Construct a registration. Use inside `inventory::submit!`:
    ///
    /// ```ignore
    /// inventory::submit! {
    ///     AlgebraicLawRegistration::new(
    ///         "vyre-ops::math::add",
    ///         AlgebraicLaw::Commutative,
    ///     )
    /// }
    /// ```
    #[must_use]
    pub const fn new(op_id: &'static str, law: AlgebraicLaw) -> Self {
        Self { op_id, law }
    }
}

inventory::collect!(AlgebraicLawRegistration);

static LAWS_BY_OP: LazyLock<FxHashMap<&'static str, Vec<&'static AlgebraicLaw>>> =
    LazyLock::new(|| {
        let mut map: FxHashMap<&'static str, Vec<&'static AlgebraicLaw>> = FxHashMap::default();
        for r in inventory::iter::<AlgebraicLawRegistration>() {
            map.entry(r.op_id).or_default().push(&r.law);
        }
        map
    });

/// Collect every registered law for `op_id`. Optimizer passes use
/// this at pass-scheduling time; per-dispatch callers should cache.
#[must_use]
pub fn laws_for_op(op_id: &str) -> &'static [&'static AlgebraicLaw] {
    LAWS_BY_OP.get(op_id).map_or(&[], Vec::as_slice)
}

/// Whether any registered law for `op_id` matches a predicate.
/// Used by canonical-form to decide "is this op commutative?".
#[must_use]
pub fn has_law<F>(op_id: &str, predicate: F) -> bool
where
    F: Fn(&AlgebraicLaw) -> bool,
{
    laws_for_op(op_id).iter().any(|law| predicate(law))
}

/// Specialized helper for the common "is commutative?" query used
/// by the canonical-form pass.
#[must_use]
pub fn is_commutative(op_id: &str) -> bool {
    has_law(op_id, |l| matches!(l, AlgebraicLaw::Commutative))
}

/// Specialized helper for "is associative?".
#[must_use]
pub fn is_associative(op_id: &str) -> bool {
    has_law(op_id, |l| matches!(l, AlgebraicLaw::Associative))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test-only registrations. Real ops submit! from their module
    // source files.
    inventory::submit! {
        AlgebraicLawRegistration::new("test::commutative_op", AlgebraicLaw::Commutative)
    }
    inventory::submit! {
        AlgebraicLawRegistration::new("test::associative_op", AlgebraicLaw::Associative)
    }

    #[test]
    fn registered_commutative_law_is_queryable() {
        assert!(is_commutative("test::commutative_op"));
        assert!(!is_commutative("test::associative_op"));
    }

    #[test]
    fn registered_associative_law_is_queryable() {
        assert!(is_associative("test::associative_op"));
        assert!(!is_associative("test::commutative_op"));
    }

    #[test]
    fn laws_for_unregistered_op_is_empty() {
        assert!(laws_for_op("test::does_not_exist").is_empty());
    }

    #[test]
    fn laws_are_stable_across_calls() {
        let a = laws_for_op("test::commutative_op").len();
        let b = laws_for_op("test::commutative_op").len();
        assert_eq!(a, b);
        assert_eq!(a, 1);
    }
}
