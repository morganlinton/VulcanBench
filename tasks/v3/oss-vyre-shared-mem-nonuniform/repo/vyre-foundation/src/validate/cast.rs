//! Cast validation table for IR expressions.
//!
//! Not every type pair can be cast safely or meaningfully in GPU shaders.
//! This module defines the closed set of supported casts so that the
//! validator can reject programs that would emit invalid conversion
//! instructions on the backend. The table is intentionally conservative:
//! missing casts can be added later without breaking existing programs.

use crate::ir::DataType;

/// Returns true if `source -> target` is a supported cast per `casts.md`.
///
/// The supported cast matrix is frozen: frontends and backends can rely
/// on it remaining stable across minor version bumps.
#[allow(clippy::unnested_or_patterns)]
#[inline]
pub(crate) fn cast_is_valid(source: &DataType, target: &DataType) -> bool {
    if source == target {
        return true;
    }
    if is_integer_like_scalar(source) && is_integer_like_scalar(target) {
        return true;
    }
    matches!(
        (source, target),
        (&DataType::U32, &DataType::Vec2U32)
            | (&DataType::U32, &DataType::Vec4U32)
            | (&DataType::I32, &DataType::Vec2U32)
            | (&DataType::I32, &DataType::Vec4U32)
            | (&DataType::Bool, &DataType::Vec2U32)
            | (&DataType::Bool, &DataType::Vec4U32)
            | (&DataType::U64, &DataType::Vec2U32)
            | (&DataType::Vec2U32, &DataType::U32)
            | (&DataType::Vec2U32, &DataType::I32)
            | (&DataType::Vec2U32, &DataType::U64)
            | (&DataType::Vec2U32, &DataType::Bool)
            | (&DataType::Vec4U32, &DataType::U32)
            | (&DataType::Vec4U32, &DataType::I32)
            | (&DataType::Vec4U32, &DataType::Vec2U32)
            | (&DataType::Vec4U32, &DataType::Bool)
            | (&DataType::Vec4U32, &DataType::U64)
            | (&DataType::U32, &DataType::F32)
            | (&DataType::I32, &DataType::F32)
            | (&DataType::Bool, &DataType::F32)
            | (&DataType::F32, &DataType::U32)
            | (&DataType::F32, &DataType::I32)
            | (&DataType::F32, &DataType::Bool)
    )
}

#[inline]
#[must_use]
fn is_integer_like_scalar(data_type: &DataType) -> bool {
    matches!(
        data_type,
        DataType::U8
            | DataType::U16
            | DataType::U32
            | DataType::U64
            | DataType::I8
            | DataType::I16
            | DataType::I32
            | DataType::I64
            | DataType::Bool
    )
}

#[inline]
#[must_use]
pub(crate) fn cast_is_narrowing(source: &DataType, target: &DataType) -> bool {
    match (integer_width_bits(source), integer_width_bits(target)) {
        (Some(source_bits), Some(target_bits)) => target_bits < source_bits,
        _ => false,
    }
}

#[inline]
#[must_use]
fn integer_width_bits(data_type: &DataType) -> Option<u16> {
    match data_type {
        DataType::U8 | DataType::I8 => Some(8),
        DataType::U16 | DataType::I16 => Some(16),
        DataType::U32 | DataType::I32 => Some(32),
        DataType::U64 | DataType::I64 => Some(64),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_cast_is_valid() {
        assert!(cast_is_valid(&DataType::U32, &DataType::U32));
        assert!(cast_is_valid(&DataType::F32, &DataType::F32));
    }

    #[test]
    fn u32_to_f32_is_valid() {
        assert!(cast_is_valid(&DataType::U32, &DataType::F32));
        assert!(cast_is_valid(&DataType::I32, &DataType::F32));
        assert!(cast_is_valid(&DataType::Bool, &DataType::F32));
        assert!(cast_is_valid(&DataType::F32, &DataType::U32));
        assert!(cast_is_valid(&DataType::F32, &DataType::I32));
        assert!(cast_is_valid(&DataType::F32, &DataType::Bool));
    }

    #[test]
    fn integer_like_scalars_cross_cast() {
        assert!(cast_is_valid(&DataType::U8, &DataType::U32));
        assert!(cast_is_valid(&DataType::I32, &DataType::U64));
        assert!(cast_is_valid(&DataType::Bool, &DataType::U32));
    }

    #[test]
    fn u32_to_vec2u32_is_valid() {
        assert!(cast_is_valid(&DataType::U32, &DataType::Vec2U32));
        assert!(cast_is_valid(&DataType::Vec2U32, &DataType::U32));
    }

    #[test]
    fn bytes_cast_is_invalid() {
        assert!(!cast_is_valid(&DataType::Bytes, &DataType::U32));
        assert!(!cast_is_valid(&DataType::U32, &DataType::Bytes));
    }

    #[test]
    fn f32_to_bytes_is_invalid() {
        assert!(!cast_is_valid(&DataType::F32, &DataType::Bytes));
    }

    #[test]
    fn narrowing_detected() {
        assert!(cast_is_narrowing(&DataType::U64, &DataType::U32));
        assert!(cast_is_narrowing(&DataType::U32, &DataType::U8));
    }

    #[test]
    fn widening_not_narrowing() {
        assert!(!cast_is_narrowing(&DataType::U8, &DataType::U32));
        assert!(!cast_is_narrowing(&DataType::U32, &DataType::U64));
    }

    #[test]
    fn same_width_not_narrowing() {
        assert!(!cast_is_narrowing(&DataType::U32, &DataType::I32));
    }

    #[test]
    fn non_integer_not_narrowing() {
        assert!(!cast_is_narrowing(&DataType::F32, &DataType::U32));
    }
}
