const EXPR_VARIANTS: &[&str] = &[
    "LitU32",
    "LitI32",
    "LitF32",
    "LitBool",
    "Var",
    "Load",
    "BufLen",
    "InvocationId",
    "WorkgroupId",
    "LocalId",
    "BinOp",
    "UnOp",
    "Call",
    "Select",
    "Cast",
    "Fma",
    "Atomic",
    "SubgroupBallot",
    "SubgroupShuffle",
    "SubgroupReduce",
    "Opaque",
];

const LAW_CATALOG: &[&str] = &[
    "commutative",
    "associative",
    "identity",
    "left-identity",
    "right-identity",
    "self-inverse",
    "idempotent",
    "absorbing",
    "left-absorbing",
    "right-absorbing",
    "involution",
    "de-morgan",
    "monotone",
    "monotonic",
    "bounded",
    "complement",
    "distributive",
    "lattice-absorption",
    "inverse-of",
    "trichotomy",
    "zero-product",
    "custom",
];

/// Return the frozen catalog of core `Expr` variant names.
#[must_use]
pub fn expr_variants() -> &'static [&'static str] {
    EXPR_VARIANTS
}

/// Return the catalog of all algebraic-law variant fingerprints.
#[must_use]
pub fn law_catalog() -> &'static [&'static str] {
    LAW_CATALOG
}
