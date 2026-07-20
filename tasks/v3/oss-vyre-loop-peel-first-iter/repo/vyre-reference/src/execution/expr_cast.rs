//! Cast helpers for the expression evaluator.

use crate::ops::{read_u32_prefix, read_u64_prefix};
use crate::value::Value;
use vyre::ir::DataType;
use vyre::Error;

pub(crate) fn spec_output_value(ty: DataType, bytes: &[u8]) -> Value {
    match ty {
        DataType::U32 => Value::U32(read_u32_prefix(bytes)),
        DataType::I32 => Value::I32(read_u32_prefix(bytes) as i32),
        DataType::Bool => Value::Bool(read_u32_prefix(bytes) != 0),
        DataType::U64 => Value::U64(read_u64_prefix(bytes)),
        DataType::F32 => Value::Float(f64::from(crate::execution::typed_ops::canonical_f32(
            f32::from_bits(read_u32_prefix(bytes)),
        ))),
        DataType::Vec2U32 => Value::from(read_fixed_prefix(bytes, 8)),
        DataType::Vec4U32 => Value::from(read_fixed_prefix(bytes, 16)),
        DataType::Bytes => Value::from(bytes),
        _ => Value::from(bytes),
    }
}

pub(crate) fn cast_value(target: &DataType, value: &Value) -> Result<Value, vyre::Error> {
    // A float source converts NUMERICALLY only to U32/I32 (saturating), Bool
    // (truthy), or F32 (identity). It has NO defined ARITHMETIC conversion to a
    // narrow int (U8/U16/I8/I16), a 64-bit int (U64/I64), or any other exotic
    // scalar: the foundation validator (`validate::cast::cast_is_valid`) rejects
    // those, and the naga/PTX emitters fail closed on them. Without this guard
    // the narrow/widen arms (or the `_ => to_bytes()` catch-all) would return a
    // meaningless byte payload (the float's raw bits) for such a cast, silently
    // diverging from every backend. Fail closed so the reference SPEC agrees
    // with the emitters (Law 10 coherence).
    //
    // The fixed-width BYTE-REINTERPRET targets (Vec2U32, Vec4U32, Bytes) are NOT
    // rejected here: `cast_value` is a source-type-agnostic byte primitive for
    // them — a scalar cast to a vector packs the scalar's canonical bytes and
    // zero-fills/truncates to the lane width, identically for u32/i32/bool/f32
    // sources (proven by `vector_cast_generated_matrix`'s 32768 cases and the
    // `cast_to_vec*` unit tests). validate still gates f32->vector out at the
    // program level, so this permissive byte primitive is only ever reached by
    // direct callers that want the byte encoding.
    if matches!(value, Value::Float(_))
        && !matches!(
            target,
            DataType::U32
                | DataType::I32
                | DataType::Bool
                | DataType::F32
                | DataType::Vec2U32
                | DataType::Vec4U32
                | DataType::Bytes
        )
    {
        return Err(Error::interp(format!(
            "cast from f32 to {target:?} has no defined conversion: a float source \
             converts numerically only to u32/i32 (saturating), bool (truthy), or f32, \
             or byte-reinterprets to a fixed-width vector. Fix: cast the f32 to u32 or \
             i32 first, then narrow or widen the integer."
        )));
    }
    match target {
        DataType::U32 => match value {
            Value::I32(v) => Ok(Value::U32(*v as u32)),
            Value::Float(v) => Ok(Value::U32((*v) as u32)),
            _ => value
                .try_as_u32()
                .map(Value::U32)
                .ok_or_else(|| invalid_cast(target, value)),
        },
        DataType::I32 => match value {
            Value::I32(value) => Ok(Value::I32(*value)),
            Value::Float(v) => Ok(Value::I32(*v as i32)),
            _ => value
                .try_as_u32()
                .map(|value| Value::I32(value as i32))
                .ok_or_else(|| invalid_cast(target, value)),
        },
        // 64-bit integer widening. `I64` and `U64` share the `Value::U64`
        // bit-pattern representation (the model has no distinct `I64`). The
        // high bits extend per the SOURCE's signedness so the reference matches
        // the backends (PTX `cvt.s64.s32`, naga sign-replicate) and Rust `as`:
        // a signed `i32` SIGN-extends (`-1i32 -> 0xFFFF_FFFF_FFFF_FFFF`), an
        // unsigned/bool source zero-extends. `try_as_u64`'s `u64::try_from`
        // would instead REJECT negative `i32` — diverging from every backend —
        // so the signed case is handled explicitly here before delegating the
        // rest. Adding `I64` here also removes it from the `_ => to_bytes()`
        // catch-all, which silently produced a 4-byte payload with no extension.
        DataType::U64 | DataType::I64 => match value {
            Value::I32(v) => Ok(Value::U64(*v as i64 as u64)),
            _ => value
                .try_as_u64()
                .map(Value::U64)
                .ok_or_else(|| invalid_cast(target, value)),
        },
        DataType::F32 => match value {
            // Integer → float is a value conversion, not a bit-cast,
            // matching backend `f32(u32_value)` semantics. Without this
            // arm the evaluator dropped through to the bytes-copy
            // fallback and produced f32 bits equal to the raw u32,
            // which aliases u32(5) to 7e-45 instead of 5.0.
            Value::U32(v) => Ok(Value::Float(f64::from(*v as f32))),
            Value::I32(v) => Ok(Value::Float(f64::from(*v as f32))),
            Value::U64(v) => Ok(Value::Float(f64::from(*v as f32))),
            Value::Float(v) => Ok(Value::Float(*v)),
            Value::Bool(b) => Ok(Value::Float(if *b { 1.0 } else { 0.0 })),
            _ => value
                .try_as_u32()
                .map(|v| Value::Float(f64::from(v as f32)))
                .ok_or_else(|| invalid_cast(target, value)),
        },
        DataType::Bool => Ok(Value::Bool(value.truthy())),
        // Narrowing integer casts TRUNCATE the high bits and keep the low
        // `width` bits, matching the documented V035 contract ("narrowing cast
        // may truncate high bits"), Rust `value as u8/u16/i8/i16`, and the masked
        // narrowing the naga/PTX emitters now apply. WGSL/PTX have no native
        // 8/16-bit scalar register, so a narrowed value is held in a 32-bit slot:
        // U8/U16 keep the masked unsigned magnitude (`Value::U32`), while I8/I16
        // SIGN-extend from the new top bit (`Value::I32`) so e.g. `200 as i8`
        // == -56 and `-1i32 as u8` == 255. Before this arm existed these targets
        // fell through to the `_ => to_bytes()` catch-all, which returned the
        // source's full-width raw bytes (a `Value::Bytes`, neither a scalar nor
        // narrowed) — diverging from every backend AND from the V035 contract.
        // The float source is already rejected above (a float has no defined
        // conversion to a narrow int), so `source_low_word` only sees integers.
        DataType::U8 => narrow_low_word(target, value).map(|bits| Value::U32(bits & 0xFF)),
        DataType::U16 => narrow_low_word(target, value).map(|bits| Value::U32(bits & 0xFFFF)),
        DataType::I8 => {
            narrow_low_word(target, value).map(|bits| Value::I32(i32::from(bits as u8 as i8)))
        }
        DataType::I16 => {
            narrow_low_word(target, value).map(|bits| Value::I32(i32::from(bits as u16 as i16)))
        }
        DataType::Bytes => Ok(Value::from(value.to_bytes())),
        DataType::Vec2U32 => Ok(Value::from(widen_to_words(value, 2))),
        DataType::Vec4U32 => Ok(Value::from(widen_to_words(value, 4))),
        _ => Ok(Value::from(value.to_bytes())),
    }
}

/// The raw low 32 bits of an integer-like scalar source, sign-agnostic.
///
/// Distinct from `Value::try_as_u32`, which REJECTS negative `i32`/out-of-range
/// `u64` (it answers "does this value fit losslessly in a u32?"). A narrowing
/// cast instead reinterprets the source's bit pattern and discards the high
/// bits, so it must read the low word even for a negative source: `-1i32` has
/// bits `0xFFFF_FFFF`, and `-1i32 as u8` == `0xFF` == 255. Fails closed for a
/// `Float`/`Array` source (a float is already rejected upstream for narrow
/// targets; an array is not a scalar).
fn narrow_low_word(target: &DataType, value: &Value) -> Result<u32, vyre::Error> {
    match value {
        Value::U32(v) => Ok(*v),
        Value::I32(v) => Ok(*v as u32),
        Value::U64(v) => Ok(*v as u32),
        Value::Bool(b) => Ok(u32::from(*b)),
        Value::Bytes(bytes) => Ok(read_u32_prefix(bytes)),
        Value::Float(_) | Value::Array(_) => Err(invalid_cast(target, value)),
    }
}

fn read_fixed_prefix(bytes: &[u8], width: usize) -> Vec<u8> {
    let mut fixed = vec![0u8; width];
    let len = bytes.len().min(width);
    fixed[..len].copy_from_slice(&bytes[..len]);
    fixed
}

fn invalid_cast(target: &DataType, value: &Value) -> Error {
    Error::interp(format!(
        "cast to {target:?} cannot represent {value:?} losslessly. Fix: cast from an in-range scalar value."
    ))
}

fn widen_to_words(value: &Value, words: usize) -> Vec<u8> {
    let target_bytes = words * 4;
    let mut bytes = vec![0u8; target_bytes];
    value.write_bytes_width_into(&mut bytes);
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cast_to_vec2_pads_scalar_bytes_to_fixed_width() {
        let value = cast_value(&DataType::Vec2U32, &Value::U32(0x0403_0201))
            .expect("Fix: scalar to vec2 cast must be representable.");

        assert_eq!(
            value.to_bytes(),
            vec![1, 2, 3, 4, 0, 0, 0, 0],
            "Fix: vector casts must preserve source bytes and zero-fill the declared lane width."
        );
    }

    #[test]
    fn cast_to_vec4_truncates_oversized_byte_payload_to_fixed_width() {
        let value = cast_value(
            &DataType::Vec4U32,
            &Value::from((0u8..24).collect::<Vec<_>>()),
        )
        .expect("Fix: byte payload to vec4 cast must be representable.");

        assert_eq!(
            value.to_bytes(),
            (0u8..16).collect::<Vec<_>>(),
            "Fix: vector casts must truncate oversized payloads at the declared lane width."
        );
    }

    /// Signed 32-bit widened to a 64-bit integer must SIGN-extend so the
    /// reference matches the backends (PTX `cvt.s64.s32`, naga sign-replicate)
    /// and Rust `i32 as i64`. Before the fix `try_as_u64`'s `u64::try_from`
    /// REJECTED negatives, so a negative `i32 -> u64` cast errored in the oracle
    /// while every backend produced a value — a silent divergence. And `I64`
    /// had no arm at all (fell to the `to_bytes()` catch-all, 4 bytes, no
    /// extension).
    #[test]
    fn cast_i32_to_u64_and_i64_sign_extends_negative() {
        // -1i32 -> 0xFFFF_FFFF_FFFF_FFFF in both U64 and I64 (shared bits).
        assert_eq!(
            cast_value(&DataType::U64, &Value::I32(-1)).expect("i32->u64 must succeed"),
            Value::U64(0xFFFF_FFFF_FFFF_FFFF),
            "negative i32 -> u64 must sign-extend, not reject or zero-extend"
        );
        assert_eq!(
            cast_value(&DataType::I64, &Value::I32(-1)).expect("i32->i64 must succeed"),
            Value::U64(0xFFFF_FFFF_FFFF_FFFF),
            "negative i32 -> i64 must sign-extend (shares the U64 bit pattern)"
        );
        // A different negative proves true sign extension, not a constant.
        assert_eq!(
            cast_value(&DataType::I64, &Value::I32(-2)).expect("i32->i64 must succeed"),
            Value::U64(0xFFFF_FFFF_FFFF_FFFE),
            "i32 -2 -> i64 must be 0xFFFF_FFFF_FFFF_FFFE"
        );
    }

    /// The non-negative twin: u32 and non-negative i32 ZERO-extend into the
    /// 64-bit value (high bits clear).
    #[test]
    fn cast_to_u64_zero_extends_non_negative() {
        assert_eq!(
            cast_value(&DataType::U64, &Value::U32(7)).expect("u32->u64 must succeed"),
            Value::U64(7),
            "u32 -> u64 must zero-extend"
        );
        assert_eq!(
            cast_value(&DataType::I64, &Value::U32(0xDEAD_BEEF)).expect("u32->i64 must succeed"),
            Value::U64(0x0000_0000_DEAD_BEEF),
            "u32 -> i64 must zero-extend (high word clear)"
        );
        assert_eq!(
            cast_value(&DataType::U64, &Value::I32(5)).expect("i32->u64 must succeed"),
            Value::U64(5),
            "non-negative i32 -> u64 must equal the value (sign bit clear)"
        );
    }

    /// A float source has no defined ARITHMETIC conversion to a narrow int
    /// (U8/U16/I8/I16) or a 64-bit int (U64/I64) — the validator rejects these
    /// and the naga/PTX emitters fail closed, so the reference SPEC must too
    /// (rather than returning the float's raw bytes via the narrow/widen arms or
    /// the catch-all). The fixed-width byte-reinterpret targets (Vec2U32/Vec4U32/
    /// Bytes) are intentionally NOT in this list — see
    /// `cast_f32_to_fixed_width_vector_byte_packs` and `vector_cast_generated_matrix`.
    #[test]
    fn cast_f32_to_undefined_target_fails_closed() {
        for target in [
            DataType::U8,
            DataType::U16,
            DataType::I8,
            DataType::I16,
            DataType::U64,
            DataType::I64,
        ] {
            let err = cast_value(&target, &Value::Float(3.5))
                .expect_err(&format!("f32 -> {target:?} must fail closed in the oracle"));
            assert!(
                format!("{err:?}").contains("no defined conversion"),
                "f32 -> {target:?} must fail closed with the float-cast message, got: {err:?}"
            );
        }
    }

    /// A float source byte-reinterprets to a fixed-width vector exactly like any
    /// other scalar: the f32's canonical 4 bytes occupy the low lane and the
    /// remaining lanes are zero-filled. This is the source-agnostic byte
    /// primitive `vector_cast_generated_matrix` exercises across 32768 cases; a
    /// guard that rejected f32 here (as an over-broad earlier version did)
    /// regresses that matrix. validate still gates f32->vector out at the
    /// program level, so only direct byte-encoding callers reach this.
    #[test]
    fn cast_f32_to_fixed_width_vector_byte_packs() {
        // `Value::Float` holds an f64, and its canonical encoding is the f64's
        // 8 little-endian bytes (copy_raw_bytes_prefix -> f64::to_le_bytes), so a
        // vector byte-pack lays those 8 bytes down then zero-fills to lane width.
        // 2.5 is exactly representable, so f64::from(2.5f32) == 2.5f64 =
        // 0x4004_0000_0000_0000 -> LE [00,00,00,00,00,00,04,40].
        let f = 2.5_f32;
        let f64_le = f64::from(f).to_le_bytes(); // 8 bytes
        assert_eq!(
            f64_le,
            [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04, 0x40],
            "precondition: 2.5f64 little-endian encoding"
        );

        // Vec2U32 is exactly 8 bytes -> the full f64 encoding, no padding.
        assert_eq!(
            cast_value(&DataType::Vec2U32, &Value::Float(f64::from(f)))
                .expect("f32 -> vec2<u32> must byte-pack, not fail closed")
                .to_bytes(),
            f64_le.to_vec(),
            "f32 -> Vec2U32 must be the f64 little-endian bytes filling both lanes"
        );

        // Vec4U32 is 16 bytes -> f64 bytes in the low 8, zero-fill the high 8.
        let mut expected_vec4 = f64_le.to_vec();
        expected_vec4.extend_from_slice(&[0u8; 8]);
        assert_eq!(
            cast_value(&DataType::Vec4U32, &Value::Float(f64::from(f)))
                .expect("f32 -> vec4<u32> must byte-pack, not fail closed")
                .to_bytes(),
            expected_vec4,
            "f32 -> Vec4U32 must place the f64 bytes in lanes 0..2 and zero-fill lanes 2..4"
        );
    }

    /// The positive twin: the float targets every layer permits still convert,
    /// with the saturating (truncate-toward-zero) / truthy / identity semantics.
    #[test]
    fn cast_f32_to_permitted_targets_converts() {
        assert_eq!(
            cast_value(&DataType::U32, &Value::Float(3.9)).expect("f32->u32 must succeed"),
            Value::U32(3),
            "f32 3.9 -> u32 truncates toward zero"
        );
        assert_eq!(
            cast_value(&DataType::I32, &Value::Float(-2.9)).expect("f32->i32 must succeed"),
            Value::I32(-2),
            "f32 -2.9 -> i32 truncates toward zero"
        );
        assert_eq!(
            cast_value(&DataType::Bool, &Value::Float(0.0)).expect("f32->bool must succeed"),
            Value::Bool(false),
            "f32 0.0 -> bool is false"
        );
        assert_eq!(
            cast_value(&DataType::Bool, &Value::Float(1.5)).expect("f32->bool must succeed"),
            Value::Bool(true),
            "f32 1.5 -> bool is true"
        );
        assert_eq!(
            cast_value(&DataType::F32, &Value::Float(2.5)).expect("f32->f32 must succeed"),
            Value::Float(2.5),
            "f32 -> f32 is identity"
        );
    }

    /// Unsigned narrowing truncates to the low `width` bits (Rust `as u8/u16`),
    /// keeping the masked magnitude in a u32 slot. Before the explicit arm these
    /// targets fell to the `_ => to_bytes()` catch-all and returned a 4-byte
    /// `Value::Bytes` (the source's full word), never the narrowed scalar.
    #[test]
    fn cast_to_unsigned_narrow_int_truncates_high_bits() {
        // 300 = 0x12C; low byte 0x2C = 44.
        assert_eq!(
            cast_value(&DataType::U8, &Value::U32(300)).expect("u32->u8 must succeed"),
            Value::U32(44),
            "300u32 as u8 must truncate to 44, not stay 300 or become Bytes"
        );
        // 0x1_2345 -> low 16 bits 0x2345.
        assert_eq!(
            cast_value(&DataType::U16, &Value::U32(0x0001_2345)).expect("u32->u16 must succeed"),
            Value::U32(0x2345),
            "0x12345u32 as u16 must keep the low 16 bits"
        );
        // A negative i32 reinterprets its bits, not rejected: -1i32 bits 0xFFFF_FFFF.
        assert_eq!(
            cast_value(&DataType::U8, &Value::I32(-1)).expect("i32->u8 must succeed"),
            Value::U32(255),
            "-1i32 as u8 must be 255 (low byte of 0xFFFFFFFF), not a rejection"
        );
        // A 64-bit source narrows through its low word.
        assert_eq!(
            cast_value(&DataType::U8, &Value::U64(0xDEAD_BEEF_0000_01FF))
                .expect("u64->u8 must succeed"),
            Value::U32(0xFF),
            "u64 as u8 must take the low byte of the low word"
        );
    }

    /// Signed narrowing truncates to the low `width` bits then SIGN-extends from
    /// the new top bit (Rust `as i8/i16`), held as a sign-extended i32.
    #[test]
    fn cast_to_signed_narrow_int_truncates_and_sign_extends() {
        // 200 & 0xFF = 200; as i8 the top bit is set -> -56.
        assert_eq!(
            cast_value(&DataType::I8, &Value::U32(200)).expect("u32->i8 must succeed"),
            Value::I32(-56),
            "200u32 as i8 must sign-extend to -56"
        );
        // 44 stays positive (top bit clear).
        assert_eq!(
            cast_value(&DataType::I8, &Value::U32(300)).expect("u32->i8 must succeed"),
            Value::I32(44),
            "300u32 as i8 truncates to 44 (positive)"
        );
        // 0xFFFF as i16 -> -1.
        assert_eq!(
            cast_value(&DataType::I16, &Value::U32(0xFFFF)).expect("u32->i16 must succeed"),
            Value::I32(-1),
            "0xFFFFu32 as i16 must sign-extend to -1"
        );
        // 0x8000 as i16 -> i16::MIN = -32768.
        assert_eq!(
            cast_value(&DataType::I16, &Value::U32(0x8000)).expect("u32->i16 must succeed"),
            Value::I32(-32768),
            "0x8000u32 as i16 is i16::MIN"
        );
        // -1i32 as i8 stays -1 (all low bits set, sign-extends back).
        assert_eq!(
            cast_value(&DataType::I8, &Value::I32(-1)).expect("i32->i8 must succeed"),
            Value::I32(-1),
            "-1i32 as i8 is -1"
        );
    }

    /// A narrowing cast of a NON-scalar/float source fails closed (a float has
    /// no defined narrow conversion; an array is not a scalar), rather than
    /// silently byte-copying through the old catch-all.
    #[test]
    fn cast_to_narrow_int_from_undefined_source_fails_closed() {
        // Float -> U8 is rejected by the upstream float guard.
        let err = cast_value(&DataType::U8, &Value::Float(3.5))
            .expect_err("f32 -> u8 must fail closed");
        assert!(
            format!("{err:?}").contains("no defined conversion"),
            "f32 -> u8 must use the float-cast message, got: {err:?}"
        );
        // Array -> I16 is rejected by narrow_low_word.
        let err = cast_value(&DataType::I16, &Value::Array(vec![Value::U32(1)]))
            .expect_err("array -> i16 must fail closed");
        assert!(
            format!("{err:?}").contains("cannot represent"),
            "array -> i16 must use the invalid-cast message, got: {err:?}"
        );
    }

    /// VRH-002: spec_output_value must apply canonical_f32 for F32 call outputs
    /// so subnormals and NaN payloads round-trip identically to from_element_bytes.
    ///
    /// Before the fix, 0x0000_0001 (positive subnormal) yielded Value::Float(1.4e-45);
    /// after the fix it yields Value::Float(0.0) — matching from_element_bytes.
    #[test]
    fn spec_output_value_canonicalizes_f32_subnormal_to_zero() {
        // Positive subnormal: smallest positive subnormal f32 = 0x0000_0001.
        let subnormal_bytes = 0x0000_0001_u32.to_le_bytes();
        let result = spec_output_value(DataType::F32, &subnormal_bytes);
        // canonical_f32 maps positive subnormal → +0.0 (preserves sign bit only).
        assert_eq!(
            result,
            Value::Float(0.0_f64),
            "Fix: spec_output_value must canonicalize positive subnormal f32 to +0.0, \
             identical to from_element_bytes — before VRH-002 fix this returned 1.4e-45"
        );
    }

    /// VRH-002: negative subnormal canonicalizes to -0.0.
    #[test]
    fn spec_output_value_canonicalizes_f32_negative_subnormal_to_negative_zero() {
        // Negative subnormal: 0x8000_0001.
        let neg_subnormal_bytes = 0x8000_0001_u32.to_le_bytes();
        let result = spec_output_value(DataType::F32, &neg_subnormal_bytes);
        // canonical_f32 maps negative subnormal → -0.0 (sign bit preserved, mantissa cleared).
        assert_eq!(
            result,
            Value::Float(f64::from(f32::from_bits(0x8000_0000))),
            "Fix: spec_output_value must canonicalize negative subnormal f32 to -0.0"
        );
    }

    /// VRH-002: NaN payload canonicalizes to the canonical quiet NaN (0x7FC0_0000).
    #[test]
    fn spec_output_value_canonicalizes_f32_nan_payload_to_canonical_nan() {
        // Payload NaN: signaling NaN with a custom payload bit.
        let payload_nan_bytes = 0x7FA0_0001_u32.to_le_bytes();
        let result = spec_output_value(DataType::F32, &payload_nan_bytes);
        // canonical_f32 maps any NaN to 0x7FC0_0000.
        let expected_bits = f32::from_bits(0x7FC0_0000);
        assert_eq!(
            result,
            Value::Float(f64::from(expected_bits)),
            "Fix: spec_output_value must canonicalize NaN f32 payloads to 0x7FC0_0000, \
             identical to from_element_bytes — before VRH-002 fix this returned the raw NaN payload"
        );
    }

    /// VRH-002: normal f32 values pass through unchanged.
    #[test]
    fn spec_output_value_normal_f32_passes_through_unchanged() {
        // 1.5f32 = 0x3FC0_0000 — a normal value, not subnormal or NaN.
        let normal_bytes = 0x3FC0_0000_u32.to_le_bytes();
        let result = spec_output_value(DataType::F32, &normal_bytes);
        assert_eq!(
            result,
            Value::Float(f64::from(1.5_f32)),
            "Fix: spec_output_value must not alter normal f32 values"
        );
    }
}
