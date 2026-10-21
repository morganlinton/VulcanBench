use super::*;

pub(crate) const BOOL_UNARY_CASES: &[BoolUnaryCase] = &[BoolUnaryCase {
    name: "resident_bool_not",
    build: Expr::not,
}];

pub(crate) const BOOL_BINARY_CASES: &[BoolBinaryCase] = &[
    BoolBinaryCase {
        name: "resident_bool_and",
        build: bool_and,
    },
    BoolBinaryCase {
        name: "resident_bool_or",
        build: bool_or,
    },
    BoolBinaryCase {
        name: "resident_bool_eq",
        build: bool_eq,
    },
    BoolBinaryCase {
        name: "resident_bool_ne",
        build: bool_ne,
    },
];

pub(crate) const F32_COMPARE_CASES: &[F32CompareCase] = &[
    F32CompareCase {
        name: "resident_f32_eq",
        build: eq_word,
    },
    F32CompareCase {
        name: "resident_f32_ne",
        build: ne_word,
    },
    F32CompareCase {
        name: "resident_f32_lt",
        build: lt_word,
    },
    F32CompareCase {
        name: "resident_f32_le",
        build: le_word,
    },
    F32CompareCase {
        name: "resident_f32_gt",
        build: gt_word,
    },
    F32CompareCase {
        name: "resident_f32_ge",
        build: ge_word,
    },
];

pub(crate) const F32_BINARY_CASES: &[F32BinaryCase] = &[
    F32BinaryCase {
        name: "resident_f32_add",
        rhs: F32RhsKind::Mixed,
        build: Expr::add,
    },
    F32BinaryCase {
        name: "resident_f32_sub",
        rhs: F32RhsKind::Mixed,
        build: Expr::sub,
    },
    F32BinaryCase {
        name: "resident_f32_mul",
        rhs: F32RhsKind::Mixed,
        build: Expr::mul,
    },
    F32BinaryCase {
        name: "resident_f32_div_nonzero",
        rhs: F32RhsKind::NonZero,
        build: Expr::div,
    },
    F32BinaryCase {
        name: "resident_f32_min",
        rhs: F32RhsKind::Mixed,
        build: Expr::min,
    },
    F32BinaryCase {
        name: "resident_f32_max",
        rhs: F32RhsKind::Mixed,
        build: Expr::max,
    },
];

pub(crate) const F32_UNARY_CASES: &[F32UnaryCase] = &[
    F32UnaryCase {
        name: "resident_f32_negate",
        inputs: F32InputKind::Mixed,
        build: Expr::negate,
    },
    F32UnaryCase {
        name: "resident_f32_abs",
        inputs: F32InputKind::Mixed,
        build: Expr::abs,
    },
    F32UnaryCase {
        name: "resident_f32_sqrt",
        inputs: F32InputKind::SqrtDomain,
        build: Expr::sqrt,
    },
    F32UnaryCase {
        name: "resident_f32_reciprocal_nonzero",
        inputs: F32InputKind::NonZero,
        build: Expr::reciprocal,
    },
];

pub(crate) const F32_CLASSIFY_CASES: &[F32ClassifyCase] = &[
    F32ClassifyCase {
        name: "resident_f32_is_nan",
        build: isnan_word,
    },
    F32ClassifyCase {
        name: "resident_f32_is_inf",
        build: isinf_word,
    },
    F32ClassifyCase {
        name: "resident_f32_is_finite",
        build: isfinite_word,
    },
];

pub(crate) const RESIDENT_ATOMIC_CASES: &[ResidentAtomicCase] = &[
    ResidentAtomicCase {
        name: "resident_atomic_add_bucketed",
        identity: 0,
        value_salt: 0x1020_3040,
        build: atomic_add,
    },
    ResidentAtomicCase {
        name: "resident_atomic_or_bucketed",
        identity: 0,
        value_salt: 0x3141_5926,
        build: atomic_or,
    },
    ResidentAtomicCase {
        name: "resident_atomic_and_bucketed",
        identity: u32::MAX,
        value_salt: 0x2718_2818,
        build: atomic_and,
    },
    ResidentAtomicCase {
        name: "resident_atomic_xor_bucketed",
        identity: 0,
        value_salt: 0x9e37_79b9,
        build: atomic_xor,
    },
    ResidentAtomicCase {
        name: "resident_atomic_min_bucketed",
        identity: u32::MAX,
        value_salt: 0xa5a5_5a5a,
        build: atomic_min,
    },
    ResidentAtomicCase {
        name: "resident_atomic_max_bucketed",
        identity: 0,
        value_salt: 0x5a5a_a5a5,
        build: atomic_max,
    },
];

pub(crate) const CAST_CASES: &[CastCase] = &[
    CastCase {
        name: "resident_u32_to_i32",
        input_type: DataType::U32,
        output_type: DataType::I32,
    },
    CastCase {
        name: "resident_u32_to_f32",
        input_type: DataType::U32,
        output_type: DataType::F32,
    },
    CastCase {
        name: "resident_u32_to_bool",
        input_type: DataType::U32,
        output_type: DataType::Bool,
    },
    CastCase {
        name: "resident_i32_to_u32",
        input_type: DataType::I32,
        output_type: DataType::U32,
    },
    CastCase {
        name: "resident_i32_to_f32",
        input_type: DataType::I32,
        output_type: DataType::F32,
    },
    CastCase {
        name: "resident_i32_to_bool",
        input_type: DataType::I32,
        output_type: DataType::Bool,
    },
    CastCase {
        name: "resident_f32_to_u32",
        input_type: DataType::F32,
        output_type: DataType::U32,
    },
    CastCase {
        name: "resident_f32_to_i32",
        input_type: DataType::F32,
        output_type: DataType::I32,
    },
    CastCase {
        name: "resident_f32_to_bool",
        input_type: DataType::F32,
        output_type: DataType::Bool,
    },
    CastCase {
        name: "resident_bool_to_u32",
        input_type: DataType::Bool,
        output_type: DataType::U32,
    },
    CastCase {
        name: "resident_bool_to_i32",
        input_type: DataType::Bool,
        output_type: DataType::I32,
    },
    CastCase {
        name: "resident_bool_to_f32",
        input_type: DataType::Bool,
        output_type: DataType::F32,
    },
];

pub(crate) const U32_BINARY_CASES: &[ResidentBinaryCase] = &[
    ResidentBinaryCase {
        name: "resident_u32_add",
        build: Expr::add,
    },
    ResidentBinaryCase {
        name: "resident_u32_sub",
        build: Expr::sub,
    },
    ResidentBinaryCase {
        name: "resident_u32_mul",
        build: Expr::mul,
    },
    ResidentBinaryCase {
        name: "resident_u32_div_total",
        build: Expr::div,
    },
    ResidentBinaryCase {
        name: "resident_u32_rem_total",
        build: Expr::rem,
    },
    ResidentBinaryCase {
        name: "resident_u32_bitand",
        build: Expr::bitand,
    },
    ResidentBinaryCase {
        name: "resident_u32_bitor",
        build: Expr::bitor,
    },
    ResidentBinaryCase {
        name: "resident_u32_bitxor",
        build: Expr::bitxor,
    },
    ResidentBinaryCase {
        name: "resident_u32_shl_masked",
        build: Expr::shl,
    },
    ResidentBinaryCase {
        name: "resident_u32_shr_masked",
        build: Expr::shr,
    },
];

pub(crate) const U32_UNARY_CASES: &[ResidentUnaryCase] = &[
    ResidentUnaryCase {
        name: "resident_u32_bitnot",
        build: Expr::bitnot,
    },
    ResidentUnaryCase {
        name: "resident_u32_reverse_bits",
        build: Expr::reverse_bits,
    },
    ResidentUnaryCase {
        name: "resident_u32_popcount",
        build: Expr::popcount,
    },
    ResidentUnaryCase {
        name: "resident_u32_clz",
        build: Expr::clz,
    },
    ResidentUnaryCase {
        name: "resident_u32_ctz",
        build: Expr::ctz,
    },
];

pub(crate) const I32_BINARY_CASES: &[ResidentBinaryCase] = &[
    ResidentBinaryCase {
        name: "resident_i32_add",
        build: Expr::add,
    },
    ResidentBinaryCase {
        name: "resident_i32_sub",
        build: Expr::sub,
    },
    ResidentBinaryCase {
        name: "resident_i32_mul",
        build: Expr::mul,
    },
    ResidentBinaryCase {
        name: "resident_i32_div_total",
        build: Expr::div,
    },
    ResidentBinaryCase {
        name: "resident_i32_rem_total",
        build: i32_rem_i32,
    },
];

pub(crate) const I32_UNARY_CASES: &[ResidentUnaryCase] = &[
    ResidentUnaryCase {
        name: "resident_i32_wrapping_negate",
        build: i32_wrapping_negate,
    },
    ResidentUnaryCase {
        name: "resident_i32_abs",
        build: i32_wrapping_abs,
    },
];

pub(crate) fn value_identity(value: Expr) -> Expr {
    value
}

pub(crate) fn value_bitnot(value: Expr) -> Expr {
    Expr::bitnot(value)
}

pub(crate) fn value_bool_not(value: Expr) -> Expr {
    Expr::not(value)
}

pub(crate) fn value_f32_negate(value: Expr) -> Expr {
    Expr::negate(value)
}

pub(crate) fn identity_index(idx: Expr) -> Expr {
    idx
}

pub(crate) fn reverse_index(idx: Expr) -> Expr {
    Expr::sub(Expr::u32((LANE_COUNT - 1) as u32), idx)
}

pub(crate) fn stride37_index(idx: Expr) -> Expr {
    Expr::bitand(
        Expr::mul(idx, Expr::u32(37)),
        Expr::u32((LANE_COUNT - 1) as u32),
    )
}

pub(crate) fn stride73_index(idx: Expr) -> Expr {
    Expr::bitand(
        Expr::mul(idx, Expr::u32(73)),
        Expr::u32((LANE_COUNT - 1) as u32),
    )
}

pub(crate) const U32_MEMORY_CASES: &[ResidentMemoryCase] = &[
    ResidentMemoryCase {
        name: "resident_u32_reverse_load_identity_store",
        ty: DataType::U32,
        build_value: value_identity,
        build_src: reverse_index,
        build_dst: identity_index,
    },
    ResidentMemoryCase {
        name: "resident_u32_dual_permutation_bitnot",
        ty: DataType::U32,
        build_value: value_bitnot,
        build_src: stride73_index,
        build_dst: stride37_index,
    },
];

pub(crate) const BOOL_MEMORY_CASES: &[ResidentMemoryCase] = &[
    ResidentMemoryCase {
        name: "resident_bool_reverse_load_identity_store",
        ty: DataType::Bool,
        build_value: value_identity,
        build_src: reverse_index,
        build_dst: identity_index,
    },
    ResidentMemoryCase {
        name: "resident_bool_dual_permutation_not",
        ty: DataType::Bool,
        build_value: value_bool_not,
        build_src: stride73_index,
        build_dst: stride37_index,
    },
];

pub(crate) const F32_MEMORY_CASES: &[ResidentMemoryCase] = &[
    ResidentMemoryCase {
        name: "resident_f32_reverse_load_identity_store",
        ty: DataType::F32,
        build_value: value_identity,
        build_src: reverse_index,
        build_dst: identity_index,
    },
    ResidentMemoryCase {
        name: "resident_f32_dual_permutation_negate",
        ty: DataType::F32,
        build_value: value_f32_negate,
        build_src: stride73_index,
        build_dst: stride37_index,
    },
];
