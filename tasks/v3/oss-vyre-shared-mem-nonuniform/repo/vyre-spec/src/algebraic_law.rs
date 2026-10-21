//! Frozen algebraic-law declarations that conformance engines verify per operation.

use crate::monotonic_direction::MonotonicDirection;

/// Function pointer used by custom algebraic law checks.
///
/// The first argument is the operation under test. The second argument is the
/// witness tuple encoded as `u32` values. Returning `true` means the law holds
/// for that witness.
pub type LawCheckFn = fn(fn(&[u8]) -> Vec<u8>, &[u32]) -> bool;

/// An algebraic law that an operation must satisfy in the frozen data contract.
///
/// Laws are declared per-operation in the registry. The algebra checker
/// verifies each law exhaustively on small domains and with witnesses on full
/// domains. Example: `AlgebraicLaw::Commutative` records that `add(a, b)` and
/// `add(b, a)` must produce the same bytes.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum AlgebraicLaw {
    /// Standard notation: `forall a b . f(a,b) = f(b,a)`.
    ///
    /// ```
    /// use vyre_spec::AlgebraicLaw;
    /// let _law = AlgebraicLaw::Commutative;
    /// ```
    Commutative,
    /// Standard notation: `forall a b c . f(f(a,b),c) = f(a,f(b,c))`.
    ///
    /// ```
    /// use vyre_spec::AlgebraicLaw;
    /// let _law = AlgebraicLaw::Associative;
    /// ```
    Associative,
    /// Standard notation: `forall a . f(a,e) = a`.
    ///
    /// ```
    /// use vyre_spec::AlgebraicLaw;
    /// let _law = AlgebraicLaw::Identity { element: 0 };
    /// ```
    Identity {
        /// The identity element as a `u32` value.
        element: u32,
    },
    /// Standard notation: `forall a . f(e,a) = a`.
    ///
    /// ```
    /// use vyre_spec::AlgebraicLaw;
    /// let _law = AlgebraicLaw::LeftIdentity { element: 0 };
    /// ```
    LeftIdentity {
        /// The left identity element as a `u32` value.
        element: u32,
    },
    /// Standard notation: `forall a . f(a,e) = a`.
    ///
    /// ```
    /// use vyre_spec::AlgebraicLaw;
    /// let _law = AlgebraicLaw::RightIdentity { element: 0 };
    /// ```
    RightIdentity {
        /// The right identity element as a `u32` value.
        element: u32,
    },
    /// Standard notation: `forall a . f(a,a) = e`.
    ///
    /// ```
    /// use vyre_spec::AlgebraicLaw;
    /// let _law = AlgebraicLaw::SelfInverse { result: 0 };
    /// ```
    SelfInverse {
        /// The result of `f(a, a)` as a `u32` value.
        result: u32,
    },
    /// Standard notation: `forall a . f(a,a) = a`.
    ///
    /// ```
    /// use vyre_spec::AlgebraicLaw;
    /// let _law = AlgebraicLaw::Idempotent;
    /// ```
    Idempotent,
    /// Standard notation: `forall a . f(a,z) = z and f(z,a) = z`.
    ///
    /// ```
    /// use vyre_spec::AlgebraicLaw;
    /// let _law = AlgebraicLaw::Absorbing { element: 0 };
    /// ```
    Absorbing {
        /// The absorbing element.
        element: u32,
    },
    /// Standard notation: `forall a . f(z,a) = z`.
    ///
    /// ```
    /// use vyre_spec::AlgebraicLaw;
    /// let _law = AlgebraicLaw::LeftAbsorbing { element: 0 };
    /// ```
    LeftAbsorbing {
        /// The left absorbing argument.
        element: u32,
    },
    /// Standard notation: `forall a . f(a,z) = z`.
    ///
    /// ```
    /// use vyre_spec::AlgebraicLaw;
    /// let _law = AlgebraicLaw::RightAbsorbing { element: 0 };
    /// ```
    RightAbsorbing {
        /// The right absorbing argument.
        element: u32,
    },
    /// Standard notation: `forall a . f(f(a)) = a`.
    ///
    /// ```
    /// use vyre_spec::AlgebraicLaw;
    /// let _law = AlgebraicLaw::Involution;
    /// ```
    Involution,
    /// Standard notation: `forall a b . f(g(a,b)) = h(f(a),f(b))`.
    ///
    /// ```
    /// use vyre_spec::AlgebraicLaw;
    /// let _law = AlgebraicLaw::DeMorgan { inner_op: "and", dual_op: "or" };
    /// ```
    DeMorgan {
        /// The operation on the left side.
        inner_op: &'static str,
        /// The dual operation on the right side.
        dual_op: &'static str,
    },
    /// Standard notation: `forall a b . a <= b -> f(a) <= f(b)`.
    ///
    /// ```
    /// use vyre_spec::AlgebraicLaw;
    /// let _law = AlgebraicLaw::Monotone;
    /// ```
    Monotone,
    /// Standard notation: `forall a b . a <= b -> f(a) <= f(b)` or
    /// `forall a b . a <= b -> f(a) >= f(b)`.
    ///
    /// ```
    /// use vyre_spec::{AlgebraicLaw, MonotonicDirection};
    /// let _law = AlgebraicLaw::Monotonic {
    ///     direction: MonotonicDirection::NonDecreasing,
    /// };
    /// ```
    Monotonic {
        /// Direction of monotonicity.
        direction: MonotonicDirection,
    },
    /// Standard notation: `forall a b . lo <= f(a,b) <= hi`.
    ///
    /// ```
    /// use vyre_spec::AlgebraicLaw;
    /// let _law = AlgebraicLaw::Bounded { lo: 0, hi: 32 };
    /// ```
    Bounded {
        /// Inclusive lower bound.
        lo: u32,
        /// Inclusive upper bound.
        hi: u32,
    },
    /// Standard notation: `forall a . f(a,g(a)) = universe`.
    ///
    /// ```
    /// use vyre_spec::AlgebraicLaw;
    /// let _law = AlgebraicLaw::Complement {
    ///     complement_op: "not",
    ///     universe: u32::MAX,
    /// };
    /// ```
    Complement {
        /// The complementary operation.
        complement_op: &'static str,
        /// The constant they sum or combine to.
        universe: u32,
    },
    /// Standard notation: `forall a b c . f(a,g(b,c)) = g(f(a,b),f(a,c))`.
    ///
    /// ```
    /// use vyre_spec::AlgebraicLaw;
    /// let _law = AlgebraicLaw::DistributiveOver { over_op: "add" };
    /// ```
    DistributiveOver {
        /// The operation that this law distributes over.
        over_op: &'static str,
    },
    /// Standard notation: `forall a b . f(a,g(a,b)) = a`.
    ///
    /// ```
    /// use vyre_spec::AlgebraicLaw;
    /// let _law = AlgebraicLaw::LatticeAbsorption { dual_op: "min" };
    /// ```
    LatticeAbsorption {
        /// The dual lattice operation.
        dual_op: &'static str,
    },
    /// Standard notation: `forall a b . f(g(a,b),b) = a`.
    ///
    /// ```
    /// use vyre_spec::AlgebraicLaw;
    /// let _law = AlgebraicLaw::InverseOf { op: "add" };
    /// ```
    InverseOf {
        /// The operation this operation inverts.
        op: &'static str,
    },
    /// Standard notation: `forall a b . exactly_one(lt(a,b), eq(a,b), gt(a,b))`.
    ///
    /// ```
    /// use vyre_spec::AlgebraicLaw;
    /// let _law = AlgebraicLaw::Trichotomy {
    ///     less_op: "lt",
    ///     equal_op: "eq",
    ///     greater_op: "gt",
    /// };
    /// ```
    Trichotomy {
        /// Strict less-than operation id.
        less_op: &'static str,
        /// Equality operation id.
        equal_op: &'static str,
        /// Strict greater-than operation id.
        greater_op: &'static str,
    },
    /// Standard notation: `forall a b . f(a,b) = 0 -> a = 0 or b = 0`.
    ///
    /// ```
    /// use vyre_spec::AlgebraicLaw;
    /// let _law = AlgebraicLaw::ZeroProduct { holds: true };
    /// ```
    ZeroProduct {
        /// Whether this law actually holds.
        holds: bool,
    },
    /// Standard notation: `forall x0 ... xn . predicate(x0, ..., xn)`.
    ///
    /// ```
    /// use vyre_spec::AlgebraicLaw;
    /// fn check(_op: fn(&[u8]) -> Vec<u8>, _args: &[u32]) -> bool { true }
    /// let _law = AlgebraicLaw::Custom {
    ///     name: "custom",
    ///     description: "custom predicate",
    ///     arity: 1,
    ///     check,
    /// };
    /// ```
    Custom {
        /// Human-readable name for this law.
        name: &'static str,
        /// Description of what the law asserts.
        description: &'static str,
        /// Number of `u32` witness values passed to the predicate.
        arity: usize,
        /// Predicate function that returns true when the law holds.
        check: LawCheckFn,
    },
    /// Categorical-IR contract law: the operation participates as an
    /// arrow in the dispatch-graph monoidal category. Composition with
    /// the identity arrow on either side leaves the operation
    /// unchanged (`f ∘ id = id ∘ f = f`). P-SPEC-1: vyre-spec
    /// invariants reflect the categorical-IR contract; conformance
    /// engines verify this law for every op tagged as a category
    /// arrow.
    CategoricalIdentity,
    /// Categorical-IR contract law: the operation composes
    /// associatively as a category arrow (`(h ∘ g) ∘ f = h ∘ (g ∘ f)`).
    /// P-SPEC-1.
    CategoricalAssociative,
}

impl AlgebraicLaw {
    /// Human-readable name for reporting.
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            Self::Commutative => "commutative",
            Self::Associative => "associative",
            Self::Identity { .. } => "identity",
            Self::LeftIdentity { .. } => "left-identity",
            Self::RightIdentity { .. } => "right-identity",
            Self::SelfInverse { .. } => "self-inverse",
            Self::Idempotent => "idempotent",
            Self::Absorbing { .. } => "absorbing",
            Self::LeftAbsorbing { .. } => "left-absorbing",
            Self::RightAbsorbing { .. } => "right-absorbing",
            Self::Involution => "involution",
            Self::DeMorgan { .. } => "de-morgan",
            Self::Monotone => "monotone",
            Self::Monotonic { .. } => "monotonic",
            Self::Bounded { .. } => "bounded",
            Self::Complement { .. } => "complement",
            Self::DistributiveOver { .. } => "distributive",
            Self::LatticeAbsorption { .. } => "lattice-absorption",
            Self::InverseOf { .. } => "inverse-of",
            Self::Trichotomy { .. } => "trichotomy",
            Self::ZeroProduct { .. } => "zero-product",
            Self::CategoricalIdentity => "categorical-identity",
            Self::CategoricalAssociative => "categorical-associative",
            Self::Custom { name, .. } => name,
        }
    }

    /// Whether this law applies to binary operations.
    #[must_use]
    pub fn is_binary(&self) -> bool {
        matches!(
            self,
            Self::Commutative
                | Self::Associative
                | Self::Identity { .. }
                | Self::LeftIdentity { .. }
                | Self::RightIdentity { .. }
                | Self::SelfInverse { .. }
                | Self::Idempotent
                | Self::Absorbing { .. }
                | Self::LeftAbsorbing { .. }
                | Self::RightAbsorbing { .. }
                | Self::Bounded { .. }
                | Self::Complement { .. }
                | Self::DistributiveOver { .. }
                | Self::LatticeAbsorption { .. }
                | Self::InverseOf { .. }
                | Self::Trichotomy { .. }
                | Self::ZeroProduct { .. }
                | Self::Custom { .. }
        )
    }

    /// Whether this law applies to unary operations.
    #[must_use]
    pub fn is_unary(&self) -> bool {
        matches!(
            self,
            Self::Involution
                | Self::Monotone
                | Self::Monotonic { .. }
                | Self::Bounded { .. }
                | Self::Complement { .. }
                | Self::DeMorgan { .. }
                | Self::Custom { .. }
        )
    }
}

impl PartialEq for AlgebraicLaw {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Commutative, Self::Commutative)
            | (Self::Associative, Self::Associative)
            | (Self::Idempotent, Self::Idempotent)
            | (Self::Involution, Self::Involution)
            | (Self::Monotone, Self::Monotone)
            | (Self::CategoricalIdentity, Self::CategoricalIdentity)
            | (Self::CategoricalAssociative, Self::CategoricalAssociative) => true,
            (Self::Identity { element: left }, Self::Identity { element: right })
            | (Self::LeftIdentity { element: left }, Self::LeftIdentity { element: right })
            | (Self::RightIdentity { element: left }, Self::RightIdentity { element: right })
            | (Self::Absorbing { element: left }, Self::Absorbing { element: right })
            | (Self::LeftAbsorbing { element: left }, Self::LeftAbsorbing { element: right })
            | (Self::RightAbsorbing { element: left }, Self::RightAbsorbing { element: right })
            | (Self::SelfInverse { result: left }, Self::SelfInverse { result: right }) => {
                left == right
            }
            (
                Self::DeMorgan {
                    inner_op: left_inner,
                    dual_op: left_dual,
                },
                Self::DeMorgan {
                    inner_op: right_inner,
                    dual_op: right_dual,
                },
            ) => left_inner == right_inner && left_dual == right_dual,
            (Self::Monotonic { direction: left }, Self::Monotonic { direction: right }) => {
                left == right
            }
            (
                Self::Bounded {
                    lo: left_lo,
                    hi: left_hi,
                },
                Self::Bounded {
                    lo: right_lo,
                    hi: right_hi,
                },
            ) => left_lo == right_lo && left_hi == right_hi,
            (
                Self::Complement {
                    complement_op: left_op,
                    universe: left_universe,
                },
                Self::Complement {
                    complement_op: right_op,
                    universe: right_universe,
                },
            ) => left_op == right_op && left_universe == right_universe,
            (
                Self::DistributiveOver { over_op: left },
                Self::DistributiveOver { over_op: right },
            )
            | (
                Self::LatticeAbsorption { dual_op: left },
                Self::LatticeAbsorption { dual_op: right },
            )
            | (Self::InverseOf { op: left }, Self::InverseOf { op: right }) => left == right,
            (
                Self::Trichotomy {
                    less_op: left_less,
                    equal_op: left_equal,
                    greater_op: left_greater,
                },
                Self::Trichotomy {
                    less_op: right_less,
                    equal_op: right_equal,
                    greater_op: right_greater,
                },
            ) => {
                left_less == right_less
                    && left_equal == right_equal
                    && left_greater == right_greater
            }
            (Self::ZeroProduct { holds: left }, Self::ZeroProduct { holds: right }) => {
                left == right
            }
            (
                Self::Custom {
                    name: left_name,
                    arity: left_arity,
                    check: left_check,
                    ..
                },
                Self::Custom {
                    name: right_name,
                    arity: right_arity,
                    check: right_check,
                    ..
                },
            ) => {
                left_name == right_name
                    && left_arity == right_arity
                    && core::ptr::fn_addr_eq(*left_check, *right_check)
            }
            _ => false,
        }
    }
}
