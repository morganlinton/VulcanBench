//! Marker types  -  unit structs the reference interpreter and backend
//! emitters dispatch on. Always compiled (zero deps); unrelated to
//! the feature-gated Tier 2.5 subsystems.

/// Stable identifier for a workgroup-shared memory region.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct RegionId(pub u32);

/// Associative reduction operator shared by scan/reduce primitives.
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, Hash)]
#[non_exhaustive]
pub enum CombineOp {
    /// Integer wrap-around addition (default).
    #[default]
    Add,
    /// Integer wrap-around multiplication.
    Mul,
    /// Bitwise AND.
    BitAnd,
    /// Bitwise OR.
    BitOr,
    /// Bitwise XOR.
    BitXor,
    /// Minimum (unsigned).
    Min,
    /// Maximum (unsigned).
    Max,
}

macro_rules! primitive_marker {
    ($(#[$m:meta])* $name:ident) => {
        $(#[$m])*
        #[derive(Debug, Default, Clone, Copy, Eq, PartialEq, Hash)]
        pub struct $name;
    };
}

primitive_marker!(
    /// Integer wrap-around addition primitive.
    ArithAdd
);
primitive_marker!(
    /// Integer wrap-around multiplication primitive.
    ArithMul
);
primitive_marker!(
    /// Bitwise AND primitive.
    BitwiseAnd
);
primitive_marker!(
    /// Bitwise OR primitive.
    BitwiseOr
);
primitive_marker!(
    /// Bitwise XOR primitive.
    BitwiseXor
);
primitive_marker!(
    /// Count-leading-zeros primitive.
    Clz
);
primitive_marker!(
    /// Equality comparison primitive.
    CompareEq
);
primitive_marker!(
    /// Less-than comparison primitive.
    CompareLt
);
primitive_marker!(
    /// Workgroup-local gather primitive.
    Gather
);
primitive_marker!(
    /// BLAKE3 hashing primitive.
    HashBlake3
);
primitive_marker!(
    /// FNV-1a hashing primitive.
    HashFnv1a
);
/// DFA-driven scan primitive.
#[derive(Debug, Default, Clone, Eq, PartialEq, Hash)]
pub struct PatternMatchDfa {
    /// Serialized DFA bytes in the canonical flat format.
    pub dfa: Vec<u8>,
}

/// Literal-string scan primitive.
#[derive(Debug, Default, Clone, Eq, PartialEq, Hash)]
pub struct PatternMatchLiteral {
    /// Literal needle bytes to match.
    pub literal: Vec<u8>,
}
primitive_marker!(
    /// Population-count primitive.
    Popcount
);
/// Associative reduction primitive.
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, Hash)]
pub struct Reduce {
    /// Reduction operator.
    pub combine: CombineOp,
}

/// Inclusive/exclusive prefix scan primitive.
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, Hash)]
pub struct Scan {
    /// Scan combine operator.
    pub combine: CombineOp,
}
primitive_marker!(
    /// Scatter primitive.
    Scatter
);
primitive_marker!(
    /// Logical shift-left primitive.
    ShiftLeft
);
primitive_marker!(
    /// Logical shift-right primitive.
    ShiftRight
);
primitive_marker!(
    /// Workgroup-local shuffle primitive.
    Shuffle
);
