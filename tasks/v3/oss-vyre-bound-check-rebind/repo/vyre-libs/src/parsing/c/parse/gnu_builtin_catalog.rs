use crate::parsing::c::parse::vast_kinds::{
    C_AST_KIND_BUILTIN_ALLOCA_INTRIN_EXPR, C_AST_KIND_BUILTIN_ASSUME_INTRIN_EXPR,
    C_AST_KIND_BUILTIN_ATOMIC_INTRIN_EXPR, C_AST_KIND_BUILTIN_BIT_INTRIN_EXPR,
    C_AST_KIND_BUILTIN_BPF_CORE_INTRIN_EXPR, C_AST_KIND_BUILTIN_BSWAP_INTRIN_EXPR,
    C_AST_KIND_BUILTIN_CHOOSE_EXPR, C_AST_KIND_BUILTIN_CLASSIFY_TYPE_EXPR,
    C_AST_KIND_BUILTIN_CONSTANT_P_EXPR, C_AST_KIND_BUILTIN_ELEMENTWISE_INTRIN_EXPR,
    C_AST_KIND_BUILTIN_EXPECT_EXPR, C_AST_KIND_BUILTIN_FP_CONST_INTRIN_EXPR,
    C_AST_KIND_BUILTIN_FP_PREDICATE_INTRIN_EXPR, C_AST_KIND_BUILTIN_FRAME_INTRIN_EXPR,
    C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR, C_AST_KIND_BUILTIN_OBJECT_SIZE_EXPR,
    C_AST_KIND_BUILTIN_OFFSETOF_EXPR, C_AST_KIND_BUILTIN_OVERFLOW_EXPR,
    C_AST_KIND_BUILTIN_PREFETCH_EXPR, C_AST_KIND_BUILTIN_SOURCE_LOC_INTRIN_EXPR,
    C_AST_KIND_BUILTIN_SYNC_INTRIN_EXPR, C_AST_KIND_BUILTIN_TRAP_INTRIN_EXPR,
    C_AST_KIND_BUILTIN_TYPES_COMPATIBLE_P_EXPR, C_AST_KIND_BUILTIN_UNREACHABLE_STMT,
    C_AST_KIND_BUILTIN_VA_INTRIN_EXPR,
};
use std::sync::OnceLock;
use vyre_primitives::hash::fnv1a::fnv1a32_const;

/// One GNU builtin spelling and its parser-local VAST kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct GnuBuiltinNameKind {
    /// Exact builtin identifier bytes.
    pub name: &'static [u8],
    /// FNV-1a hash of `name`, matching VAST symbol hashes.
    pub hash: u32,
    /// Parser-local VAST kind emitted for the builtin.
    pub kind: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct GnuBuiltinHashKind {
    hash: u32,
    kind: u32,
}

const fn gnu_builtin(name: &'static [u8], kind: u32) -> GnuBuiltinNameKind {
    GnuBuiltinNameKind {
        name,
        hash: fnv1a32_const(name),
        kind,
    }
}

/// Canonical GNU builtin catalog shared by byte-name, GPU-hash, and oracle classifiers.
pub(super) const GNU_BUILTIN_NAME_KINDS: &[GnuBuiltinNameKind] = &[
    gnu_builtin(b"__builtin_constant_p", C_AST_KIND_BUILTIN_CONSTANT_P_EXPR),
    gnu_builtin(b"__builtin_choose_expr", C_AST_KIND_BUILTIN_CHOOSE_EXPR),
    gnu_builtin(
        b"__builtin_types_compatible_p",
        C_AST_KIND_BUILTIN_TYPES_COMPATIBLE_P_EXPR,
    ),
    gnu_builtin(b"__builtin_expect", C_AST_KIND_BUILTIN_EXPECT_EXPR),
    gnu_builtin(
        b"__builtin_expect_with_probability",
        C_AST_KIND_BUILTIN_EXPECT_EXPR,
    ),
    gnu_builtin(b"__builtin_offsetof", C_AST_KIND_BUILTIN_OFFSETOF_EXPR),
    gnu_builtin(
        b"__builtin_object_size",
        C_AST_KIND_BUILTIN_OBJECT_SIZE_EXPR,
    ),
    gnu_builtin(
        b"__builtin_dynamic_object_size",
        C_AST_KIND_BUILTIN_OBJECT_SIZE_EXPR,
    ),
    gnu_builtin(b"__builtin_prefetch", C_AST_KIND_BUILTIN_PREFETCH_EXPR),
    gnu_builtin(
        b"__builtin_unreachable",
        C_AST_KIND_BUILTIN_UNREACHABLE_STMT,
    ),
    gnu_builtin(b"__builtin_add_overflow", C_AST_KIND_BUILTIN_OVERFLOW_EXPR),
    gnu_builtin(b"__builtin_sub_overflow", C_AST_KIND_BUILTIN_OVERFLOW_EXPR),
    gnu_builtin(b"__builtin_mul_overflow", C_AST_KIND_BUILTIN_OVERFLOW_EXPR),
    gnu_builtin(
        b"__builtin_add_overflow_p",
        C_AST_KIND_BUILTIN_OVERFLOW_EXPR,
    ),
    gnu_builtin(
        b"__builtin_sub_overflow_p",
        C_AST_KIND_BUILTIN_OVERFLOW_EXPR,
    ),
    gnu_builtin(
        b"__builtin_mul_overflow_p",
        C_AST_KIND_BUILTIN_OVERFLOW_EXPR,
    ),
    gnu_builtin(b"__builtin_sadd_overflow", C_AST_KIND_BUILTIN_OVERFLOW_EXPR),
    gnu_builtin(
        b"__builtin_saddl_overflow",
        C_AST_KIND_BUILTIN_OVERFLOW_EXPR,
    ),
    gnu_builtin(
        b"__builtin_saddll_overflow",
        C_AST_KIND_BUILTIN_OVERFLOW_EXPR,
    ),
    gnu_builtin(b"__builtin_uadd_overflow", C_AST_KIND_BUILTIN_OVERFLOW_EXPR),
    gnu_builtin(
        b"__builtin_uaddl_overflow",
        C_AST_KIND_BUILTIN_OVERFLOW_EXPR,
    ),
    gnu_builtin(
        b"__builtin_uaddll_overflow",
        C_AST_KIND_BUILTIN_OVERFLOW_EXPR,
    ),
    gnu_builtin(b"__builtin_ssub_overflow", C_AST_KIND_BUILTIN_OVERFLOW_EXPR),
    gnu_builtin(
        b"__builtin_ssubl_overflow",
        C_AST_KIND_BUILTIN_OVERFLOW_EXPR,
    ),
    gnu_builtin(
        b"__builtin_ssubll_overflow",
        C_AST_KIND_BUILTIN_OVERFLOW_EXPR,
    ),
    gnu_builtin(b"__builtin_usub_overflow", C_AST_KIND_BUILTIN_OVERFLOW_EXPR),
    gnu_builtin(
        b"__builtin_usubl_overflow",
        C_AST_KIND_BUILTIN_OVERFLOW_EXPR,
    ),
    gnu_builtin(
        b"__builtin_usubll_overflow",
        C_AST_KIND_BUILTIN_OVERFLOW_EXPR,
    ),
    gnu_builtin(b"__builtin_smul_overflow", C_AST_KIND_BUILTIN_OVERFLOW_EXPR),
    gnu_builtin(
        b"__builtin_smull_overflow",
        C_AST_KIND_BUILTIN_OVERFLOW_EXPR,
    ),
    gnu_builtin(
        b"__builtin_smulll_overflow",
        C_AST_KIND_BUILTIN_OVERFLOW_EXPR,
    ),
    gnu_builtin(b"__builtin_umul_overflow", C_AST_KIND_BUILTIN_OVERFLOW_EXPR),
    gnu_builtin(
        b"__builtin_umull_overflow",
        C_AST_KIND_BUILTIN_OVERFLOW_EXPR,
    ),
    gnu_builtin(
        b"__builtin_umulll_overflow",
        C_AST_KIND_BUILTIN_OVERFLOW_EXPR,
    ),
    gnu_builtin(b"__builtin_addcb", C_AST_KIND_BUILTIN_OVERFLOW_EXPR),
    gnu_builtin(b"__builtin_addcs", C_AST_KIND_BUILTIN_OVERFLOW_EXPR),
    gnu_builtin(b"__builtin_addc", C_AST_KIND_BUILTIN_OVERFLOW_EXPR),
    gnu_builtin(b"__builtin_addcl", C_AST_KIND_BUILTIN_OVERFLOW_EXPR),
    gnu_builtin(b"__builtin_addcll", C_AST_KIND_BUILTIN_OVERFLOW_EXPR),
    gnu_builtin(b"__builtin_subcb", C_AST_KIND_BUILTIN_OVERFLOW_EXPR),
    gnu_builtin(b"__builtin_subcs", C_AST_KIND_BUILTIN_OVERFLOW_EXPR),
    gnu_builtin(b"__builtin_subc", C_AST_KIND_BUILTIN_OVERFLOW_EXPR),
    gnu_builtin(b"__builtin_subcl", C_AST_KIND_BUILTIN_OVERFLOW_EXPR),
    gnu_builtin(b"__builtin_subcll", C_AST_KIND_BUILTIN_OVERFLOW_EXPR),
    gnu_builtin(
        b"__builtin_classify_type",
        C_AST_KIND_BUILTIN_CLASSIFY_TYPE_EXPR,
    ),
    gnu_builtin(b"__builtin_bswap16", C_AST_KIND_BUILTIN_BSWAP_INTRIN_EXPR),
    gnu_builtin(b"__builtin_bswap32", C_AST_KIND_BUILTIN_BSWAP_INTRIN_EXPR),
    gnu_builtin(b"__builtin_bswap64", C_AST_KIND_BUILTIN_BSWAP_INTRIN_EXPR),
    gnu_builtin(b"__builtin_ffs", C_AST_KIND_BUILTIN_BIT_INTRIN_EXPR),
    gnu_builtin(b"__builtin_ffsl", C_AST_KIND_BUILTIN_BIT_INTRIN_EXPR),
    gnu_builtin(b"__builtin_ffsll", C_AST_KIND_BUILTIN_BIT_INTRIN_EXPR),
    gnu_builtin(b"__builtin_clz", C_AST_KIND_BUILTIN_BIT_INTRIN_EXPR),
    gnu_builtin(b"__builtin_clzl", C_AST_KIND_BUILTIN_BIT_INTRIN_EXPR),
    gnu_builtin(b"__builtin_clzll", C_AST_KIND_BUILTIN_BIT_INTRIN_EXPR),
    gnu_builtin(b"__builtin_ctz", C_AST_KIND_BUILTIN_BIT_INTRIN_EXPR),
    gnu_builtin(b"__builtin_ctzl", C_AST_KIND_BUILTIN_BIT_INTRIN_EXPR),
    gnu_builtin(b"__builtin_ctzll", C_AST_KIND_BUILTIN_BIT_INTRIN_EXPR),
    gnu_builtin(b"__builtin_popcount", C_AST_KIND_BUILTIN_BIT_INTRIN_EXPR),
    gnu_builtin(b"__builtin_popcountl", C_AST_KIND_BUILTIN_BIT_INTRIN_EXPR),
    gnu_builtin(b"__builtin_popcountll", C_AST_KIND_BUILTIN_BIT_INTRIN_EXPR),
    gnu_builtin(b"__builtin_parity", C_AST_KIND_BUILTIN_BIT_INTRIN_EXPR),
    gnu_builtin(b"__builtin_parityl", C_AST_KIND_BUILTIN_BIT_INTRIN_EXPR),
    gnu_builtin(b"__builtin_parityll", C_AST_KIND_BUILTIN_BIT_INTRIN_EXPR),
    gnu_builtin(b"__builtin_popcountg", C_AST_KIND_BUILTIN_BIT_INTRIN_EXPR),
    gnu_builtin(b"__builtin_clzg", C_AST_KIND_BUILTIN_BIT_INTRIN_EXPR),
    gnu_builtin(b"__builtin_ctzg", C_AST_KIND_BUILTIN_BIT_INTRIN_EXPR),
    gnu_builtin(b"__builtin_clrsb", C_AST_KIND_BUILTIN_BIT_INTRIN_EXPR),
    gnu_builtin(b"__builtin_clrsbl", C_AST_KIND_BUILTIN_BIT_INTRIN_EXPR),
    gnu_builtin(b"__builtin_clrsbll", C_AST_KIND_BUILTIN_BIT_INTRIN_EXPR),
    gnu_builtin(b"__builtin_bitreverse8", C_AST_KIND_BUILTIN_BIT_INTRIN_EXPR),
    gnu_builtin(
        b"__builtin_bitreverse16",
        C_AST_KIND_BUILTIN_BIT_INTRIN_EXPR,
    ),
    gnu_builtin(
        b"__builtin_bitreverse32",
        C_AST_KIND_BUILTIN_BIT_INTRIN_EXPR,
    ),
    gnu_builtin(
        b"__builtin_bitreverse64",
        C_AST_KIND_BUILTIN_BIT_INTRIN_EXPR,
    ),
    gnu_builtin(b"__builtin_rotateleft8", C_AST_KIND_BUILTIN_BIT_INTRIN_EXPR),
    gnu_builtin(
        b"__builtin_rotateleft16",
        C_AST_KIND_BUILTIN_BIT_INTRIN_EXPR,
    ),
    gnu_builtin(
        b"__builtin_rotateleft32",
        C_AST_KIND_BUILTIN_BIT_INTRIN_EXPR,
    ),
    gnu_builtin(
        b"__builtin_rotateleft64",
        C_AST_KIND_BUILTIN_BIT_INTRIN_EXPR,
    ),
    gnu_builtin(
        b"__builtin_rotateright8",
        C_AST_KIND_BUILTIN_BIT_INTRIN_EXPR,
    ),
    gnu_builtin(
        b"__builtin_rotateright16",
        C_AST_KIND_BUILTIN_BIT_INTRIN_EXPR,
    ),
    gnu_builtin(
        b"__builtin_rotateright32",
        C_AST_KIND_BUILTIN_BIT_INTRIN_EXPR,
    ),
    gnu_builtin(
        b"__builtin_rotateright64",
        C_AST_KIND_BUILTIN_BIT_INTRIN_EXPR,
    ),
    gnu_builtin(
        b"__builtin_isnan",
        C_AST_KIND_BUILTIN_FP_PREDICATE_INTRIN_EXPR,
    ),
    gnu_builtin(
        b"__builtin_isinf",
        C_AST_KIND_BUILTIN_FP_PREDICATE_INTRIN_EXPR,
    ),
    gnu_builtin(
        b"__builtin_isfinite",
        C_AST_KIND_BUILTIN_FP_PREDICATE_INTRIN_EXPR,
    ),
    gnu_builtin(
        b"__builtin_isnormal",
        C_AST_KIND_BUILTIN_FP_PREDICATE_INTRIN_EXPR,
    ),
    gnu_builtin(
        b"__builtin_signbit",
        C_AST_KIND_BUILTIN_FP_PREDICATE_INTRIN_EXPR,
    ),
    gnu_builtin(
        b"__builtin_fpclassify",
        C_AST_KIND_BUILTIN_FP_PREDICATE_INTRIN_EXPR,
    ),
    gnu_builtin(
        b"__builtin_isgreater",
        C_AST_KIND_BUILTIN_FP_PREDICATE_INTRIN_EXPR,
    ),
    gnu_builtin(
        b"__builtin_isgreaterequal",
        C_AST_KIND_BUILTIN_FP_PREDICATE_INTRIN_EXPR,
    ),
    gnu_builtin(
        b"__builtin_isless",
        C_AST_KIND_BUILTIN_FP_PREDICATE_INTRIN_EXPR,
    ),
    gnu_builtin(
        b"__builtin_islessequal",
        C_AST_KIND_BUILTIN_FP_PREDICATE_INTRIN_EXPR,
    ),
    gnu_builtin(
        b"__builtin_islessgreater",
        C_AST_KIND_BUILTIN_FP_PREDICATE_INTRIN_EXPR,
    ),
    gnu_builtin(
        b"__builtin_isunordered",
        C_AST_KIND_BUILTIN_FP_PREDICATE_INTRIN_EXPR,
    ),
    gnu_builtin(
        b"__builtin_isfpclass",
        C_AST_KIND_BUILTIN_FP_PREDICATE_INTRIN_EXPR,
    ),
    gnu_builtin(
        b"__builtin_huge_val",
        C_AST_KIND_BUILTIN_FP_CONST_INTRIN_EXPR,
    ),
    gnu_builtin(
        b"__builtin_huge_valf",
        C_AST_KIND_BUILTIN_FP_CONST_INTRIN_EXPR,
    ),
    gnu_builtin(
        b"__builtin_huge_vall",
        C_AST_KIND_BUILTIN_FP_CONST_INTRIN_EXPR,
    ),
    gnu_builtin(b"__builtin_inf", C_AST_KIND_BUILTIN_FP_CONST_INTRIN_EXPR),
    gnu_builtin(b"__builtin_inff", C_AST_KIND_BUILTIN_FP_CONST_INTRIN_EXPR),
    gnu_builtin(b"__builtin_infl", C_AST_KIND_BUILTIN_FP_CONST_INTRIN_EXPR),
    gnu_builtin(b"__builtin_nan", C_AST_KIND_BUILTIN_FP_CONST_INTRIN_EXPR),
    gnu_builtin(b"__builtin_nanf", C_AST_KIND_BUILTIN_FP_CONST_INTRIN_EXPR),
    gnu_builtin(b"__builtin_nanl", C_AST_KIND_BUILTIN_FP_CONST_INTRIN_EXPR),
    gnu_builtin(b"__builtin_nans", C_AST_KIND_BUILTIN_FP_CONST_INTRIN_EXPR),
    gnu_builtin(b"__builtin_nansf", C_AST_KIND_BUILTIN_FP_CONST_INTRIN_EXPR),
    gnu_builtin(b"__builtin_nansl", C_AST_KIND_BUILTIN_FP_CONST_INTRIN_EXPR),
    gnu_builtin(b"__builtin_fabs", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_fabsf", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_fabsl", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_copysign", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_copysignf", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_copysignl", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_sqrt", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_sqrtf", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_sqrtl", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_cbrt", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_cbrtf", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_cbrtl", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_pow", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_powf", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_powl", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_exp", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_expf", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_expl", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_exp2", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_exp2f", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_exp2l", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_expm1", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_expm1f", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_expm1l", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_log", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_logf", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_logl", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_log2", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_log2f", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_log2l", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_log10", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_log10f", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_log10l", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_log1p", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_log1pf", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_log1pl", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_sin", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_sinf", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_sinl", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_cos", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_cosf", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_cosl", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_tan", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_tanf", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_tanl", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_asin", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_asinf", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_asinl", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_acos", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_acosf", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_acosl", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_atan", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_atanf", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_atanl", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_atan2", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_atan2f", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_atan2l", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_sinh", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_sinhf", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_sinhl", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_cosh", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_coshf", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_coshl", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_tanh", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_tanhf", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_tanhl", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_asinh", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_asinhf", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_asinhl", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_acosh", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_acoshf", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_acoshl", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_atanh", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_atanhf", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_atanhl", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_floor", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_floorf", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_floorl", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_ceil", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_ceilf", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_ceill", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_trunc", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_truncf", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_truncl", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_round", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_roundf", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_roundl", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_lround", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_lroundf", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_lroundl", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_llround", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_llroundf", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_llroundl", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_rint", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_rintf", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_rintl", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_lrint", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_lrintf", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_lrintl", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_llrint", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_llrintf", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_llrintl", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_nearbyint", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_nearbyintf", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_nearbyintl", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_fmod", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_fmodf", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_fmodl", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_remainder", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_remainderf", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_remainderl", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_remquo", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_remquof", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_remquol", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_fmin", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_fminf", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_fminl", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_fmax", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_fmaxf", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_fmaxl", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_fdim", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_fdimf", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_fdiml", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_fma", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_fmaf", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_fmal", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_frexp", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_frexpf", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_frexpl", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_ldexp", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_ldexpf", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_ldexpl", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_modf", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_modff", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_modfl", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_ilogb", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_ilogbf", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_ilogbl", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_logb", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_logbf", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_logbl", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_memcpy", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(
        b"__builtin___memcpy_chk",
        C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR,
    ),
    gnu_builtin(
        b"__builtin_memcpy_inline",
        C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR,
    ),
    gnu_builtin(b"__builtin_memmove", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(
        b"__builtin___memmove_chk",
        C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR,
    ),
    gnu_builtin(b"__builtin_memset", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(
        b"__builtin___memset_chk",
        C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR,
    ),
    gnu_builtin(
        b"__builtin_memset_inline",
        C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR,
    ),
    gnu_builtin(b"__builtin_memcmp", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_memchr", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(
        b"__builtin_char_memchr",
        C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR,
    ),
    gnu_builtin(b"__builtin_strlen", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_strnlen", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_strcmp", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_strncmp", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_strcpy", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(
        b"__builtin___strcpy_chk",
        C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR,
    ),
    gnu_builtin(b"__builtin_strncpy", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(
        b"__builtin___strncpy_chk",
        C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR,
    ),
    gnu_builtin(b"__builtin_strcat", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(
        b"__builtin___strcat_chk",
        C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR,
    ),
    gnu_builtin(b"__builtin_strncat", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(
        b"__builtin___strncat_chk",
        C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR,
    ),
    gnu_builtin(b"__builtin_strchr", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_strrchr", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(b"__builtin_strstr", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
    gnu_builtin(
        b"__builtin___clear_cache",
        C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR,
    ),
    gnu_builtin(
        b"__builtin_clear_padding",
        C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR,
    ),
    gnu_builtin(b"__builtin_trap", C_AST_KIND_BUILTIN_TRAP_INTRIN_EXPR),
    gnu_builtin(b"__builtin_debugtrap", C_AST_KIND_BUILTIN_TRAP_INTRIN_EXPR),
    gnu_builtin(
        b"__builtin_verbose_trap",
        C_AST_KIND_BUILTIN_TRAP_INTRIN_EXPR,
    ),
    gnu_builtin(b"__builtin_alloca", C_AST_KIND_BUILTIN_ALLOCA_INTRIN_EXPR),
    gnu_builtin(
        b"__builtin_alloca_with_align",
        C_AST_KIND_BUILTIN_ALLOCA_INTRIN_EXPR,
    ),
    gnu_builtin(
        b"__builtin_alloca_with_align_and_max",
        C_AST_KIND_BUILTIN_ALLOCA_INTRIN_EXPR,
    ),
    gnu_builtin(
        b"__builtin_assume_aligned",
        C_AST_KIND_BUILTIN_ASSUME_INTRIN_EXPR,
    ),
    gnu_builtin(b"__builtin_assume", C_AST_KIND_BUILTIN_ASSUME_INTRIN_EXPR),
    gnu_builtin(
        b"__builtin_unpredictable",
        C_AST_KIND_BUILTIN_ASSUME_INTRIN_EXPR,
    ),
    gnu_builtin(
        b"__builtin_assoc_barrier",
        C_AST_KIND_BUILTIN_ASSUME_INTRIN_EXPR,
    ),
    gnu_builtin(
        b"__builtin_speculation_safe_value",
        C_AST_KIND_BUILTIN_ASSUME_INTRIN_EXPR,
    ),
    gnu_builtin(
        b"__builtin_is_aligned",
        C_AST_KIND_BUILTIN_ASSUME_INTRIN_EXPR,
    ),
    gnu_builtin(b"__builtin_align_up", C_AST_KIND_BUILTIN_ASSUME_INTRIN_EXPR),
    gnu_builtin(
        b"__builtin_align_down",
        C_AST_KIND_BUILTIN_ASSUME_INTRIN_EXPR,
    ),
    gnu_builtin(
        b"__builtin_unwind_init",
        C_AST_KIND_BUILTIN_FRAME_INTRIN_EXPR,
    ),
    gnu_builtin(
        b"__builtin_stack_address",
        C_AST_KIND_BUILTIN_FRAME_INTRIN_EXPR,
    ),
    gnu_builtin(
        b"__builtin_frame_address",
        C_AST_KIND_BUILTIN_FRAME_INTRIN_EXPR,
    ),
    gnu_builtin(
        b"__builtin_return_address",
        C_AST_KIND_BUILTIN_FRAME_INTRIN_EXPR,
    ),
    gnu_builtin(
        b"__builtin_extract_return_addr",
        C_AST_KIND_BUILTIN_FRAME_INTRIN_EXPR,
    ),
    gnu_builtin(
        b"__builtin_frob_return_addr",
        C_AST_KIND_BUILTIN_FRAME_INTRIN_EXPR,
    ),
    gnu_builtin(b"__builtin_va_start", C_AST_KIND_BUILTIN_VA_INTRIN_EXPR),
    gnu_builtin(b"__builtin_va_arg", C_AST_KIND_BUILTIN_VA_INTRIN_EXPR),
    gnu_builtin(b"__builtin_va_end", C_AST_KIND_BUILTIN_VA_INTRIN_EXPR),
    gnu_builtin(b"__builtin_va_copy", C_AST_KIND_BUILTIN_VA_INTRIN_EXPR),
    gnu_builtin(b"__builtin_ms_va_start", C_AST_KIND_BUILTIN_VA_INTRIN_EXPR),
    gnu_builtin(b"__builtin_c23_va_start", C_AST_KIND_BUILTIN_VA_INTRIN_EXPR),
    gnu_builtin(b"__builtin_next_arg", C_AST_KIND_BUILTIN_VA_INTRIN_EXPR),
    gnu_builtin(b"__builtin_FILE", C_AST_KIND_BUILTIN_SOURCE_LOC_INTRIN_EXPR),
    gnu_builtin(
        b"__builtin_FILE_NAME",
        C_AST_KIND_BUILTIN_SOURCE_LOC_INTRIN_EXPR,
    ),
    gnu_builtin(
        b"__builtin_FUNCTION",
        C_AST_KIND_BUILTIN_SOURCE_LOC_INTRIN_EXPR,
    ),
    gnu_builtin(
        b"__builtin_FUNCSIG",
        C_AST_KIND_BUILTIN_SOURCE_LOC_INTRIN_EXPR,
    ),
    gnu_builtin(b"__builtin_LINE", C_AST_KIND_BUILTIN_SOURCE_LOC_INTRIN_EXPR),
    gnu_builtin(
        b"__builtin_COLUMN",
        C_AST_KIND_BUILTIN_SOURCE_LOC_INTRIN_EXPR,
    ),
    gnu_builtin(
        b"__builtin_source_location",
        C_AST_KIND_BUILTIN_SOURCE_LOC_INTRIN_EXPR,
    ),
    gnu_builtin(
        b"__builtin_dump_struct",
        C_AST_KIND_BUILTIN_ASSUME_INTRIN_EXPR,
    ),
    gnu_builtin(b"__sync_fetch_and_add", C_AST_KIND_BUILTIN_SYNC_INTRIN_EXPR),
    gnu_builtin(b"__sync_fetch_and_sub", C_AST_KIND_BUILTIN_SYNC_INTRIN_EXPR),
    gnu_builtin(b"__sync_fetch_and_or", C_AST_KIND_BUILTIN_SYNC_INTRIN_EXPR),
    gnu_builtin(b"__sync_fetch_and_and", C_AST_KIND_BUILTIN_SYNC_INTRIN_EXPR),
    gnu_builtin(b"__sync_fetch_and_xor", C_AST_KIND_BUILTIN_SYNC_INTRIN_EXPR),
    gnu_builtin(
        b"__sync_fetch_and_nand",
        C_AST_KIND_BUILTIN_SYNC_INTRIN_EXPR,
    ),
    gnu_builtin(b"__sync_add_and_fetch", C_AST_KIND_BUILTIN_SYNC_INTRIN_EXPR),
    gnu_builtin(b"__sync_sub_and_fetch", C_AST_KIND_BUILTIN_SYNC_INTRIN_EXPR),
    gnu_builtin(b"__sync_or_and_fetch", C_AST_KIND_BUILTIN_SYNC_INTRIN_EXPR),
    gnu_builtin(b"__sync_and_and_fetch", C_AST_KIND_BUILTIN_SYNC_INTRIN_EXPR),
    gnu_builtin(b"__sync_xor_and_fetch", C_AST_KIND_BUILTIN_SYNC_INTRIN_EXPR),
    gnu_builtin(
        b"__sync_nand_and_fetch",
        C_AST_KIND_BUILTIN_SYNC_INTRIN_EXPR,
    ),
    gnu_builtin(
        b"__sync_bool_compare_and_swap",
        C_AST_KIND_BUILTIN_SYNC_INTRIN_EXPR,
    ),
    gnu_builtin(
        b"__sync_val_compare_and_swap",
        C_AST_KIND_BUILTIN_SYNC_INTRIN_EXPR,
    ),
    gnu_builtin(
        b"__sync_lock_test_and_set",
        C_AST_KIND_BUILTIN_SYNC_INTRIN_EXPR,
    ),
    gnu_builtin(b"__sync_lock_release", C_AST_KIND_BUILTIN_SYNC_INTRIN_EXPR),
    gnu_builtin(b"__sync_synchronize", C_AST_KIND_BUILTIN_SYNC_INTRIN_EXPR),
    gnu_builtin(b"__atomic_load", C_AST_KIND_BUILTIN_ATOMIC_INTRIN_EXPR),
    gnu_builtin(b"__atomic_load_n", C_AST_KIND_BUILTIN_ATOMIC_INTRIN_EXPR),
    gnu_builtin(b"__atomic_store", C_AST_KIND_BUILTIN_ATOMIC_INTRIN_EXPR),
    gnu_builtin(b"__atomic_store_n", C_AST_KIND_BUILTIN_ATOMIC_INTRIN_EXPR),
    gnu_builtin(b"__atomic_exchange", C_AST_KIND_BUILTIN_ATOMIC_INTRIN_EXPR),
    gnu_builtin(
        b"__atomic_exchange_n",
        C_AST_KIND_BUILTIN_ATOMIC_INTRIN_EXPR,
    ),
    gnu_builtin(
        b"__atomic_compare_exchange",
        C_AST_KIND_BUILTIN_ATOMIC_INTRIN_EXPR,
    ),
    gnu_builtin(
        b"__atomic_compare_exchange_n",
        C_AST_KIND_BUILTIN_ATOMIC_INTRIN_EXPR,
    ),
    gnu_builtin(b"__atomic_fetch_add", C_AST_KIND_BUILTIN_ATOMIC_INTRIN_EXPR),
    gnu_builtin(b"__atomic_fetch_sub", C_AST_KIND_BUILTIN_ATOMIC_INTRIN_EXPR),
    gnu_builtin(b"__atomic_fetch_or", C_AST_KIND_BUILTIN_ATOMIC_INTRIN_EXPR),
    gnu_builtin(b"__atomic_fetch_and", C_AST_KIND_BUILTIN_ATOMIC_INTRIN_EXPR),
    gnu_builtin(b"__atomic_fetch_xor", C_AST_KIND_BUILTIN_ATOMIC_INTRIN_EXPR),
    gnu_builtin(
        b"__atomic_fetch_nand",
        C_AST_KIND_BUILTIN_ATOMIC_INTRIN_EXPR,
    ),
    gnu_builtin(b"__atomic_add_fetch", C_AST_KIND_BUILTIN_ATOMIC_INTRIN_EXPR),
    gnu_builtin(b"__atomic_sub_fetch", C_AST_KIND_BUILTIN_ATOMIC_INTRIN_EXPR),
    gnu_builtin(b"__atomic_or_fetch", C_AST_KIND_BUILTIN_ATOMIC_INTRIN_EXPR),
    gnu_builtin(b"__atomic_and_fetch", C_AST_KIND_BUILTIN_ATOMIC_INTRIN_EXPR),
    gnu_builtin(b"__atomic_xor_fetch", C_AST_KIND_BUILTIN_ATOMIC_INTRIN_EXPR),
    gnu_builtin(
        b"__atomic_nand_fetch",
        C_AST_KIND_BUILTIN_ATOMIC_INTRIN_EXPR,
    ),
    gnu_builtin(
        b"__atomic_test_and_set",
        C_AST_KIND_BUILTIN_ATOMIC_INTRIN_EXPR,
    ),
    gnu_builtin(b"__atomic_clear", C_AST_KIND_BUILTIN_ATOMIC_INTRIN_EXPR),
    gnu_builtin(
        b"__atomic_thread_fence",
        C_AST_KIND_BUILTIN_ATOMIC_INTRIN_EXPR,
    ),
    gnu_builtin(
        b"__atomic_signal_fence",
        C_AST_KIND_BUILTIN_ATOMIC_INTRIN_EXPR,
    ),
    gnu_builtin(
        b"__builtin_preserve_access_index",
        C_AST_KIND_BUILTIN_BPF_CORE_INTRIN_EXPR,
    ),
    gnu_builtin(
        b"__builtin_preserve_field_info",
        C_AST_KIND_BUILTIN_BPF_CORE_INTRIN_EXPR,
    ),
    gnu_builtin(
        b"__builtin_preserve_type_info",
        C_AST_KIND_BUILTIN_BPF_CORE_INTRIN_EXPR,
    ),
    gnu_builtin(
        b"__builtin_preserve_enum_value",
        C_AST_KIND_BUILTIN_BPF_CORE_INTRIN_EXPR,
    ),
    gnu_builtin(
        b"__builtin_btf_type_id",
        C_AST_KIND_BUILTIN_BPF_CORE_INTRIN_EXPR,
    ),
    gnu_builtin(
        b"__builtin_preserve_static_offset",
        C_AST_KIND_BUILTIN_BPF_CORE_INTRIN_EXPR,
    ),
    gnu_builtin(
        b"__builtin_vectorelements",
        C_AST_KIND_BUILTIN_ELEMENTWISE_INTRIN_EXPR,
    ),
    gnu_builtin(
        b"__builtin_elementwise_abs",
        C_AST_KIND_BUILTIN_ELEMENTWISE_INTRIN_EXPR,
    ),
    gnu_builtin(
        b"__builtin_elementwise_fma",
        C_AST_KIND_BUILTIN_ELEMENTWISE_INTRIN_EXPR,
    ),
    gnu_builtin(
        b"__builtin_elementwise_popcount",
        C_AST_KIND_BUILTIN_ELEMENTWISE_INTRIN_EXPR,
    ),
    gnu_builtin(
        b"__builtin_elementwise_bitreverse",
        C_AST_KIND_BUILTIN_ELEMENTWISE_INTRIN_EXPR,
    ),
    gnu_builtin(
        b"__builtin_elementwise_add_sat",
        C_AST_KIND_BUILTIN_ELEMENTWISE_INTRIN_EXPR,
    ),
    gnu_builtin(
        b"__builtin_elementwise_sub_sat",
        C_AST_KIND_BUILTIN_ELEMENTWISE_INTRIN_EXPR,
    ),
    gnu_builtin(
        b"__builtin_elementwise_max",
        C_AST_KIND_BUILTIN_ELEMENTWISE_INTRIN_EXPR,
    ),
    gnu_builtin(
        b"__builtin_elementwise_min",
        C_AST_KIND_BUILTIN_ELEMENTWISE_INTRIN_EXPR,
    ),
    gnu_builtin(
        b"__builtin_elementwise_clzg",
        C_AST_KIND_BUILTIN_ELEMENTWISE_INTRIN_EXPR,
    ),
    gnu_builtin(
        b"__builtin_elementwise_ctzg",
        C_AST_KIND_BUILTIN_ELEMENTWISE_INTRIN_EXPR,
    ),
];

/// Classify an exact GNU builtin identifier byte string.
pub(super) fn classify_gnu_builtin_name(name: &[u8]) -> Option<u32> {
    let catalog = sorted_name_catalog();
    catalog
        .binary_search_by(|entry| entry.name.cmp(name))
        .ok()
        .map(|idx| catalog[idx].kind)
}

/// Classify an FNV-1a builtin identifier hash.
pub(super) fn classify_gnu_builtin_hash(hash: u32) -> Option<u32> {
    let catalog = sorted_hash_catalog();
    catalog
        .binary_search_by_key(&hash, |entry| entry.hash)
        .ok()
        .map(|idx| catalog[idx].kind)
}

fn sorted_hash_catalog() -> &'static [GnuBuiltinHashKind] {
    static SORTED_HASH_CATALOG: OnceLock<Box<[GnuBuiltinHashKind]>> = OnceLock::new();
    SORTED_HASH_CATALOG.get_or_init(|| {
        let mut entries = Vec::with_capacity(GNU_BUILTIN_NAME_KINDS.len());
        entries.extend(
            GNU_BUILTIN_NAME_KINDS
                .iter()
                .map(|entry| GnuBuiltinHashKind {
                    hash: entry.hash,
                    kind: entry.kind,
                }),
        );
        entries.sort_unstable_by_key(|entry| entry.hash);
        entries.into_boxed_slice()
    })
}

fn sorted_name_catalog() -> &'static [GnuBuiltinNameKind] {
    static SORTED_NAME_CATALOG: OnceLock<Box<[GnuBuiltinNameKind]>> = OnceLock::new();
    SORTED_NAME_CATALOG.get_or_init(|| {
        let mut entries = Vec::with_capacity(GNU_BUILTIN_NAME_KINDS.len());
        entries.extend_from_slice(GNU_BUILTIN_NAME_KINDS);
        entries.sort_unstable_by(|left, right| left.name.cmp(right.name));
        entries.into_boxed_slice()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn byte_catalog_and_hash_catalog_agree() {
        for entry in GNU_BUILTIN_NAME_KINDS {
            let hash = fnv1a32(entry.name);
            assert_eq!(entry.hash, hash, "{}", String::from_utf8_lossy(entry.name));
            assert_eq!(
                classify_gnu_builtin_hash(hash),
                Some(entry.kind),
                "{}",
                String::from_utf8_lossy(entry.name)
            );
        }
    }

    #[test]
    fn catalog_has_no_duplicate_names_or_hashes() {
        for (idx, left) in GNU_BUILTIN_NAME_KINDS.iter().enumerate() {
            for right in GNU_BUILTIN_NAME_KINDS.iter().skip(idx + 1) {
                assert_ne!(
                    left.name,
                    right.name,
                    "duplicate GNU builtin name {}",
                    String::from_utf8_lossy(left.name)
                );
            }
        }
        for (idx, left) in GNU_BUILTIN_NAME_KINDS.iter().enumerate() {
            for right in GNU_BUILTIN_NAME_KINDS.iter().skip(idx + 1) {
                assert_ne!(
                    left.hash, right.hash,
                    "duplicate GNU builtin hash {:#010x}",
                    left.hash
                );
            }
        }
    }

    #[test]
    fn hash_catalog_is_sorted_for_logarithmic_lookup() {
        let catalog = sorted_hash_catalog();
        assert_eq!(
            catalog.len(),
            GNU_BUILTIN_NAME_KINDS.len(),
            "Fix: hash catalog must index every GNU builtin spelling"
        );
        assert!(
            catalog.windows(2).all(|pair| pair[0].hash < pair[1].hash),
            "Fix: hash catalog must stay strictly sorted so binary search is sound"
        );
    }

    #[test]
    fn name_catalog_is_sorted_for_logarithmic_lookup() {
        let catalog = sorted_name_catalog();
        assert_eq!(
            catalog.len(),
            GNU_BUILTIN_NAME_KINDS.len(),
            "Fix: sorted name catalog must index every GNU builtin spelling"
        );
        assert!(
            catalog.windows(2).all(|pair| pair[0].name < pair[1].name),
            "Fix: name catalog must stay strictly sorted so binary search is sound"
        );
    }

    #[test]
    fn fp_builtin_categories_are_reachable_from_byte_and_hash_classifiers() {
        for (name, kind) in [
            (
                b"__builtin_isnan".as_slice(),
                C_AST_KIND_BUILTIN_FP_PREDICATE_INTRIN_EXPR,
            ),
            (
                b"__builtin_isunordered".as_slice(),
                C_AST_KIND_BUILTIN_FP_PREDICATE_INTRIN_EXPR,
            ),
            (
                b"__builtin_huge_val".as_slice(),
                C_AST_KIND_BUILTIN_FP_CONST_INTRIN_EXPR,
            ),
            (
                b"__builtin_nansl".as_slice(),
                C_AST_KIND_BUILTIN_FP_CONST_INTRIN_EXPR,
            ),
        ] {
            assert_eq!(classify_gnu_builtin_name(name), Some(kind));
            assert_eq!(classify_gnu_builtin_hash(fnv1a32(name)), Some(kind));
        }
    }

    #[test]
    fn libm_builtin_categories_are_reachable_from_byte_and_hash_classifiers() {
        for name in [
            b"__builtin_fabs".as_slice(),
            b"__builtin_sqrtf".as_slice(),
            b"__builtin_powl".as_slice(),
            b"__builtin_exp2".as_slice(),
            b"__builtin_log10l".as_slice(),
            b"__builtin_sinf".as_slice(),
            b"__builtin_coshl".as_slice(),
            b"__builtin_atan2".as_slice(),
            b"__builtin_llroundf".as_slice(),
            b"__builtin_nearbyintl".as_slice(),
            b"__builtin_remainder".as_slice(),
            b"__builtin_remquol".as_slice(),
            b"__builtin_fmaf".as_slice(),
            b"__builtin_frexpl".as_slice(),
            b"__builtin_modff".as_slice(),
            b"__builtin_ilogbl".as_slice(),
        ] {
            assert_eq!(
                classify_gnu_builtin_name(name),
                Some(C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR)
            );
            assert_eq!(
                classify_gnu_builtin_hash(fnv1a32(name)),
                Some(C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR)
            );
        }
    }

    #[test]
    fn modern_bit_builtin_categories_are_reachable_from_byte_and_hash_classifiers() {
        for name in [
            b"__builtin_clrsb".as_slice(),
            b"__builtin_clrsbll".as_slice(),
            b"__builtin_popcountg".as_slice(),
            b"__builtin_clzg".as_slice(),
            b"__builtin_ctzg".as_slice(),
            b"__builtin_bitreverse8".as_slice(),
            b"__builtin_bitreverse64".as_slice(),
            b"__builtin_rotateleft32".as_slice(),
            b"__builtin_rotateright64".as_slice(),
        ] {
            assert_eq!(
                classify_gnu_builtin_name(name),
                Some(C_AST_KIND_BUILTIN_BIT_INTRIN_EXPR)
            );
            assert_eq!(
                classify_gnu_builtin_hash(fnv1a32(name)),
                Some(C_AST_KIND_BUILTIN_BIT_INTRIN_EXPR)
            );
        }
    }

    #[test]
    fn clang_and_gcc_modern_builtin_categories_are_reachable_from_byte_and_hash_classifiers() {
        for (name, kind) in [
            (
                b"__builtin_addcb".as_slice(),
                C_AST_KIND_BUILTIN_OVERFLOW_EXPR,
            ),
            (
                b"__builtin_addcll".as_slice(),
                C_AST_KIND_BUILTIN_OVERFLOW_EXPR,
            ),
            (
                b"__builtin_subcs".as_slice(),
                C_AST_KIND_BUILTIN_OVERFLOW_EXPR,
            ),
            (
                b"__builtin_subcll".as_slice(),
                C_AST_KIND_BUILTIN_OVERFLOW_EXPR,
            ),
            (
                b"__builtin_isfpclass".as_slice(),
                C_AST_KIND_BUILTIN_FP_PREDICATE_INTRIN_EXPR,
            ),
            (
                b"__builtin___clear_cache".as_slice(),
                C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR,
            ),
            (
                b"__builtin_clear_padding".as_slice(),
                C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR,
            ),
            (
                b"__builtin_assoc_barrier".as_slice(),
                C_AST_KIND_BUILTIN_ASSUME_INTRIN_EXPR,
            ),
            (
                b"__builtin_stack_address".as_slice(),
                C_AST_KIND_BUILTIN_FRAME_INTRIN_EXPR,
            ),
            (
                b"__builtin_c23_va_start".as_slice(),
                C_AST_KIND_BUILTIN_VA_INTRIN_EXPR,
            ),
            (
                b"__builtin_FILE_NAME".as_slice(),
                C_AST_KIND_BUILTIN_SOURCE_LOC_INTRIN_EXPR,
            ),
            (
                b"__builtin_FUNCSIG".as_slice(),
                C_AST_KIND_BUILTIN_SOURCE_LOC_INTRIN_EXPR,
            ),
            (
                b"__builtin_COLUMN".as_slice(),
                C_AST_KIND_BUILTIN_SOURCE_LOC_INTRIN_EXPR,
            ),
            (
                b"__builtin_source_location".as_slice(),
                C_AST_KIND_BUILTIN_SOURCE_LOC_INTRIN_EXPR,
            ),
        ] {
            assert_eq!(classify_gnu_builtin_name(name), Some(kind));
            assert_eq!(classify_gnu_builtin_hash(fnv1a32(name)), Some(kind));
        }
    }

    #[test]
    fn clang_elementwise_builtin_categories_are_reachable_from_byte_and_hash_classifiers() {
        for name in [
            b"__builtin_vectorelements".as_slice(),
            b"__builtin_elementwise_abs".as_slice(),
            b"__builtin_elementwise_fma".as_slice(),
            b"__builtin_elementwise_popcount".as_slice(),
            b"__builtin_elementwise_bitreverse".as_slice(),
            b"__builtin_elementwise_add_sat".as_slice(),
            b"__builtin_elementwise_sub_sat".as_slice(),
            b"__builtin_elementwise_max".as_slice(),
            b"__builtin_elementwise_min".as_slice(),
            b"__builtin_elementwise_clzg".as_slice(),
            b"__builtin_elementwise_ctzg".as_slice(),
        ] {
            assert_eq!(
                classify_gnu_builtin_name(name),
                Some(C_AST_KIND_BUILTIN_ELEMENTWISE_INTRIN_EXPR)
            );
            assert_eq!(
                classify_gnu_builtin_hash(fnv1a32(name)),
                Some(C_AST_KIND_BUILTIN_ELEMENTWISE_INTRIN_EXPR)
            );
        }
    }

    use vyre_primitives::hash::fnv1a::fnv1a32;
}
