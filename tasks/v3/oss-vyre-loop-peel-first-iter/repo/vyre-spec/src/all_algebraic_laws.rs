//! Frozen representative set used to keep the algebraic-law catalog complete.

use crate::{algebraic_law::AlgebraicLaw, monotonic_direction::MonotonicDirection};

fn all_algebraic_laws_custom_check(_op: fn(&[u8]) -> Vec<u8>, _args: &[u32]) -> bool {
    true
}

/// One canonical representative for every [`AlgebraicLaw`] enum variant.
///
/// Parameterized variants use stable values that exercise the variant shape
/// without claiming to enumerate every possible payload value.
///
/// **V7-CORR-016 clarification**: the op-id strings below
/// (`"primitive.bitwise.and"`, `"primitive.math.add"`, etc.) are
/// representative payload examples. They are NOT references to ops in the inventory, and
/// downstream code must NOT feed them to `laws_for_op` or any op-id
/// lookup. This array is a variant-coverage catalog to guarantee
/// every enum variant has a canonical value; it is not a registry.
static ALL_ALGEBRAIC_LAWS: &[AlgebraicLaw] = &[
    AlgebraicLaw::Commutative,
    AlgebraicLaw::Associative,
    AlgebraicLaw::Identity { element: 0 },
    AlgebraicLaw::LeftIdentity { element: 0 },
    AlgebraicLaw::RightIdentity { element: 0 },
    AlgebraicLaw::SelfInverse { result: 0 },
    AlgebraicLaw::Idempotent,
    AlgebraicLaw::Absorbing { element: 0 },
    AlgebraicLaw::LeftAbsorbing { element: 0 },
    AlgebraicLaw::RightAbsorbing { element: 0 },
    AlgebraicLaw::Involution,
    AlgebraicLaw::DeMorgan {
        inner_op: "primitive.bitwise.and",
        dual_op: "primitive.bitwise.or",
    },
    AlgebraicLaw::Monotone,
    AlgebraicLaw::Monotonic {
        direction: MonotonicDirection::NonDecreasing,
    },
    AlgebraicLaw::Bounded { lo: 0, hi: 32 },
    AlgebraicLaw::Complement {
        complement_op: "primitive.bitwise.not",
        universe: u32::MAX,
    },
    AlgebraicLaw::DistributiveOver {
        over_op: "primitive.math.add",
    },
    AlgebraicLaw::LatticeAbsorption {
        dual_op: "primitive.math.min",
    },
    AlgebraicLaw::InverseOf {
        op: "primitive.math.add",
    },
    AlgebraicLaw::Trichotomy {
        less_op: "primitive.compare.lt",
        equal_op: "primitive.compare.eq",
        greater_op: "primitive.compare.gt",
    },
    AlgebraicLaw::ZeroProduct { holds: true },
    AlgebraicLaw::Custom {
        name: "custom",
        description: "canonical custom law representative",
        arity: 1,
        check: all_algebraic_laws_custom_check,
    },
];

/// Return one canonical representative for every [`AlgebraicLaw`] enum variant.
#[must_use]
pub fn all_algebraic_laws() -> &'static [AlgebraicLaw] {
    ALL_ALGEBRAIC_LAWS
}
