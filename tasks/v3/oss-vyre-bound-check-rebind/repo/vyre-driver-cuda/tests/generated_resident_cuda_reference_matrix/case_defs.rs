use super::*;

#[derive(Clone, Copy)]
pub(crate) struct BoolUnaryCase {
    pub(crate) name: &'static str,
    pub(crate) build: fn(Expr) -> Expr,
}

#[derive(Clone, Copy)]
pub(crate) struct BoolBinaryCase {
    pub(crate) name: &'static str,
    pub(crate) build: fn(Expr, Expr) -> Expr,
}

#[derive(Clone, Copy)]
pub(crate) struct F32CompareCase {
    pub(crate) name: &'static str,
    pub(crate) build: fn(Expr, Expr) -> Expr,
}

#[derive(Clone, Copy)]
pub(crate) struct F32BinaryCase {
    pub(crate) name: &'static str,
    pub(crate) rhs: F32RhsKind,
    pub(crate) build: fn(Expr, Expr) -> Expr,
}

#[derive(Clone, Copy)]
pub(crate) struct F32UnaryCase {
    pub(crate) name: &'static str,
    pub(crate) inputs: F32InputKind,
    pub(crate) build: fn(Expr) -> Expr,
}

#[derive(Clone, Copy)]
pub(crate) struct F32ClassifyCase {
    pub(crate) name: &'static str,
    pub(crate) build: fn(Expr) -> Expr,
}

#[derive(Clone, Copy)]
pub(crate) enum F32RhsKind {
    Mixed,
    NonZero,
}

#[derive(Clone, Copy)]
pub(crate) enum F32InputKind {
    Mixed,
    NonZero,
    SqrtDomain,
}

#[derive(Clone, Copy)]
pub(crate) struct ResidentAtomicCase {
    pub(crate) name: &'static str,
    pub(crate) identity: u32,
    pub(crate) value_salt: u32,
    pub(crate) build: fn(&str, Expr, Expr) -> Expr,
}

#[derive(Clone)]
pub(crate) struct CastCase {
    pub(crate) name: &'static str,
    pub(crate) input_type: DataType,
    pub(crate) output_type: DataType,
}

#[derive(Clone, Copy)]
pub(crate) struct ResidentBinaryCase {
    pub(crate) name: &'static str,
    pub(crate) build: fn(Expr, Expr) -> Expr,
}

#[derive(Clone, Copy)]
pub(crate) struct ResidentUnaryCase {
    pub(crate) name: &'static str,
    pub(crate) build: fn(Expr) -> Expr,
}

#[derive(Clone)]
pub(crate) struct ResidentMemoryCase {
    pub(crate) name: &'static str,
    pub(crate) ty: DataType,
    pub(crate) build_value: fn(Expr) -> Expr,
    pub(crate) build_src: fn(Expr) -> Expr,
    pub(crate) build_dst: fn(Expr) -> Expr,
}

pub(crate) fn bool_and(lhs: Expr, rhs: Expr) -> Expr {
    Expr::and(lhs, rhs)
}

pub(crate) fn bool_or(lhs: Expr, rhs: Expr) -> Expr {
    Expr::or(lhs, rhs)
}

pub(crate) fn bool_eq(lhs: Expr, rhs: Expr) -> Expr {
    Expr::eq(lhs, rhs)
}

pub(crate) fn bool_ne(lhs: Expr, rhs: Expr) -> Expr {
    Expr::ne(lhs, rhs)
}

pub(crate) fn isnan_word(value: Expr) -> Expr {
    bool_word(Expr::is_nan(value))
}

pub(crate) fn isinf_word(value: Expr) -> Expr {
    bool_word(Expr::is_inf(value))
}

pub(crate) fn isfinite_word(value: Expr) -> Expr {
    bool_word(Expr::is_finite(value))
}

pub(crate) fn i32_rem_i32(lhs: Expr, rhs: Expr) -> Expr {
    Expr::cast(DataType::I32, Expr::rem(lhs, rhs))
}

pub(crate) fn i32_wrapping_negate(value: Expr) -> Expr {
    Expr::sub(Expr::i32(0), value)
}

pub(crate) fn i32_wrapping_abs(value: Expr) -> Expr {
    Expr::select(
        Expr::lt(value.clone(), Expr::i32(0)),
        i32_wrapping_negate(value.clone()),
        value,
    )
}

pub(crate) fn atomic_add(buffer: &str, index: Expr, value: Expr) -> Expr {
    Expr::atomic_add(buffer, index, value)
}

pub(crate) fn atomic_or(buffer: &str, index: Expr, value: Expr) -> Expr {
    Expr::atomic_or(buffer, index, value)
}

pub(crate) fn atomic_and(buffer: &str, index: Expr, value: Expr) -> Expr {
    Expr::atomic_and(buffer, index, value)
}

pub(crate) fn atomic_xor(buffer: &str, index: Expr, value: Expr) -> Expr {
    Expr::atomic_xor(buffer, index, value)
}

pub(crate) fn atomic_min(buffer: &str, index: Expr, value: Expr) -> Expr {
    Expr::atomic_min(buffer, index, value)
}

pub(crate) fn atomic_max(buffer: &str, index: Expr, value: Expr) -> Expr {
    Expr::atomic_max(buffer, index, value)
}

