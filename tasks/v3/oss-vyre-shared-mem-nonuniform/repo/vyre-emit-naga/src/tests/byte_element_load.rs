//! Test: U8/I8 byte-element LoadGlobal emits the byte-extract pattern.
//!
//! WGSL has no native byte storage, so the emitter packs U8/I8 into
//! `array<u32>`. Without the byte-extract emit path a `Load(buffer: U8, addr)`
//! would return the u32 word at index `addr`, not the byte at byte address
//! `addr`. The reference evaluator treats U8 as byte-addressed; this test
//! pins the WGSL emit pattern that keeps both backends in agreement.
use super::*;

fn byte_load_desc(element_type: DataType) -> KernelDescriptor {
    KernelDescriptor {
        id: "byte_load".into(),
        bindings: BindingLayout {
            slots: vec![vyre_lower::BindingSlot {
                slot: 0,
                element_type,
                element_count: Some(64),
                memory_class: MemoryClass::Global,
                visibility: BindingVisibility::ReadOnly,
                name: "source".into(),
            }],
        },
        dispatch: Dispatch::new(1, 1, 1),
        body: KernelBody {
            ops: vec![
                // result 0: the byte address (literal 7)
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                },
                // result 1: source[byte=7]  -  must auto-byte-extract
                KernelOp {
                    kind: KernelOpKind::LoadGlobal,
                    operands: vec![0, 0],
                    result: Some(1),
                },
            ],
            child_bodies: vec![],
            literals: vec![LiteralValue::U32(7)],
        },
    }
}

#[test]
fn u8_load_global_emits_word_indexed_byte_extract() {
    let desc = byte_load_desc(DataType::U8);
    let module = emit(&desc).unwrap();

    // The buffer must still be array<u32> (no native u8 storage).
    let global = module
        .global_variables
        .iter()
        .find(|(_, g)| g.name.as_deref() == Some("source"))
        .map(|(_, g)| g)
        .expect("source binding must be emitted");
    let TypeInner::Array { base, .. } = &module.types[global.ty].inner else {
        panic!("source binding must lower to array<u32>");
    };
    let TypeInner::Scalar(scalar) = &module.types[*base].inner else {
        panic!("source binding element type must be scalar");
    };
    assert_eq!(
        scalar.kind,
        naga::ScalarKind::Uint,
        "U8 buffer must remain backed by array<u32> in WGSL emit"
    );
    assert_eq!(scalar.width, 4, "U8 buffer storage width must be 4 bytes");

    // The body must contain the byte-extract arithmetic chain:
    //   shr(byte_index, 2)  -  word index
    //   and(byte_index, 3)  -  lane in word
    //   mul(lane, 8)        -  bit shift
    //   shr(word, shift)    -  extracted (unmasked)
    //   and(_, 0xff)        -  masked byte
    let entry = module.entry_points.first().expect("entry point");
    let arena = &entry.function.expressions;
    let mut shr_count = 0usize;
    let mut and_count = 0usize;
    let mut mul_count = 0usize;
    for (_, expr) in arena.iter() {
        if let naga::Expression::Binary { op, .. } = expr {
            match op {
                naga::BinaryOperator::ShiftRight => shr_count += 1,
                naga::BinaryOperator::And => and_count += 1,
                naga::BinaryOperator::Multiply => mul_count += 1,
                _ => {}
            }
        }
    }
    assert!(
        shr_count >= 2,
        "U8 byte-extract must emit ≥2 ShiftRight (word index + byte shift); got {shr_count}"
    );
    assert!(
        and_count >= 2,
        "U8 byte-extract must emit ≥2 And (lane mask + 0xff mask); got {and_count}"
    );
    assert!(
        mul_count >= 1,
        "U8 byte-extract must emit ≥1 Multiply (lane*8); got {mul_count}"
    );

    // The literal 0xff must be present (the byte mask).
    let has_ff_literal = arena
        .iter()
        .any(|(_, expr)| matches!(expr, naga::Expression::Literal(naga::Literal::U32(0xff))));
    assert!(
        has_ff_literal,
        "U8 byte-extract must mask with literal 0xff"
    );
}

#[test]
fn i8_load_global_emits_byte_extract_with_sign_extend() {
    let desc = byte_load_desc(DataType::I8);
    let module = emit(&desc).unwrap();

    // I8 sign-extend uses (byte << 24) cast i32 then >> 24. The cast
    // appears as Expression::As with kind = Sint.
    let entry = module.entry_points.first().expect("entry point");
    let arena = &entry.function.expressions;
    let has_as_sint = arena.iter().any(|(_, expr)| {
        matches!(
            expr,
            naga::Expression::As {
                kind: naga::ScalarKind::Sint,
                ..
            }
        )
    });
    assert!(
        has_as_sint,
        "I8 byte-extract must include u32→i32 cast for sign extension"
    );

    // ShiftLeft by 24 must appear.
    let has_shl_24 = arena.iter().any(|(_, expr)| {
        matches!(
            expr,
            naga::Expression::Binary {
                op: naga::BinaryOperator::ShiftLeft,
                ..
            }
        )
    });
    assert!(
        has_shl_24,
        "I8 byte-extract must include a left-shift to put byte in MSB before sign extension"
    );
}

fn byte_store_desc(element_type: DataType) -> KernelDescriptor {
    KernelDescriptor {
        id: "byte_store".into(),
        bindings: BindingLayout {
            slots: vec![vyre_lower::BindingSlot {
                slot: 0,
                element_type,
                element_count: Some(64),
                memory_class: MemoryClass::Global,
                visibility: BindingVisibility::ReadWrite,
                name: "out".into(),
            }],
        },
        dispatch: Dispatch::new(1, 1, 1),
        body: KernelBody {
            ops: vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![1],
                    result: Some(1),
                },
                KernelOp {
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![0, 0, 1],
                    result: None,
                },
            ],
            child_bodies: vec![],
            literals: vec![LiteralValue::U32(7), LiteralValue::U32(0xab)],
        },
    }
}

#[test]
fn u8_store_global_emits_byte_rmw_with_clear_and_merge() {
    let desc = byte_store_desc(DataType::U8);
    let module = emit(&desc).unwrap();
    let entry = module.entry_points.first().expect("entry point");
    let arena = &entry.function.expressions;

    let has_bitwise_not = arena.iter().any(|(_, expr)| {
        matches!(
            expr,
            naga::Expression::Unary {
                op: naga::UnaryOperator::BitwiseNot,
                ..
            }
        )
    });
    assert!(
        has_bitwise_not,
        "U8 byte-store must invert the lane mask via BitwiseNot to clear the target byte"
    );
    let has_inclusive_or = arena.iter().any(|(_, expr)| {
        matches!(
            expr,
            naga::Expression::Binary {
                op: naga::BinaryOperator::InclusiveOr,
                ..
            }
        )
    });
    assert!(
        has_inclusive_or,
        "U8 byte-store must merge cleared word with new byte via InclusiveOr"
    );
    let store_count = entry
        .function
        .body
        .iter()
        .filter(|stmt| matches!(stmt, naga::Statement::Store { .. }))
        .count();
    assert_eq!(
        store_count, 1,
        "U8 byte-store must collapse to one Statement::Store on the underlying u32 word"
    );
}

#[test]
fn i8_store_global_uses_same_rmw_path_as_u8() {
    // I8 stores share the byte-mask + RMW path; sign-extension only
    // affects loads.
    let desc = byte_store_desc(DataType::I8);
    let module = emit(&desc).unwrap();
    let entry = module.entry_points.first().expect("entry point");
    let arena = &entry.function.expressions;
    let has_bitwise_not = arena.iter().any(|(_, expr)| {
        matches!(
            expr,
            naga::Expression::Unary {
                op: naga::UnaryOperator::BitwiseNot,
                ..
            }
        )
    });
    assert!(
        has_bitwise_not,
        "I8 byte-store must invert the lane mask via BitwiseNot, same as U8"
    );
}

#[test]
fn u32_store_global_unchanged_by_byte_rmw_path() {
    // Regression guard: U32 stores must NOT trigger the byte-RMW path.
    let desc = byte_store_desc(DataType::U32);
    let module = emit(&desc).unwrap();
    let entry = module.entry_points.first().expect("entry point");
    let arena = &entry.function.expressions;
    let has_bitwise_not = arena.iter().any(|(_, expr)| {
        matches!(
            expr,
            naga::Expression::Unary {
                op: naga::UnaryOperator::BitwiseNot,
                ..
            }
        )
    });
    assert!(
        !has_bitwise_not,
        "U32 StoreGlobal must NOT emit BitwiseNot; byte-store path leaked"
    );
}

/// A `Cast` whose target is `Bytes` is INVALID per the foundation cast table
/// (`validate::cast::cast_is_valid` rejects every cast to/from `Bytes`), but the
/// descriptor emitter does not re-run that table — so before this guard the
/// scalar-cast emit path silently mapped `Bytes → u32` (`scalar_cast_target` and
/// `type_for_data_type` both grouped `Bytes` with `U8/U16/U32`), reinterpreting
/// a packed-byte target as a 32-bit word (Law 10). `Bytes` is a buffer-element
/// marker that needs a pack-to-u32 pre-pass; the emitter must fail closed, never
/// silently widen. This is the value/cast twin of the buffer-element rejection
/// in `naga_type_buffer_followup::bytes_buffers`.
fn cast_to_bytes_desc() -> KernelDescriptor {
    KernelDescriptor {
        id: "cast_to_bytes".into(),
        bindings: BindingLayout {
            slots: vec![vyre_lower::BindingSlot {
                slot: 0,
                element_type: DataType::U32,
                element_count: Some(1),
                memory_class: MemoryClass::Global,
                visibility: BindingVisibility::ReadWrite,
                name: "out".into(),
            }],
        },
        dispatch: Dispatch::new(1, 1, 1),
        body: KernelBody {
            ops: vec![
                // result 0: a u32 literal value
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                },
                // result 1: cast that u32 -> Bytes. This is the invalid op that
                // must fail closed, NOT silently emit a u32 `As` conversion.
                KernelOp {
                    kind: KernelOpKind::Cast {
                        target: DataType::Bytes,
                    },
                    operands: vec![0],
                    result: Some(1),
                },
            ],
            child_bodies: vec![],
            literals: vec![LiteralValue::U32(0xab)],
        },
    }
}

#[test]
fn cast_to_bytes_fails_closed_instead_of_silent_u32_reinterpret() {
    let desc = cast_to_bytes_desc();
    let err = emit(&desc).expect_err(
        "Cast-to-Bytes must fail closed: `Bytes` is a packed-byte element, not a \
         castable scalar — silently emitting a u32 `As` conversion is a Law-10 \
         byte-as-word reinterpret",
    );
    let msg = err.to_string();
    assert!(
        msg.contains("Bytes") && msg.contains("pack-to-u32 pre-pass"),
        "Cast-to-Bytes rejection must name the type and the fix (pack-to-u32 \
         pre-pass); got: {msg}"
    );
}

#[test]
fn u32_load_global_unchanged_by_byte_extract_path() {
    // Regression guard: U32 buffers must NOT trigger the byte-extract
    // path. The emitted body must remain a single Access + Load pair.
    let desc = byte_load_desc(DataType::U32);
    let module = emit(&desc).unwrap();
    let entry = module.entry_points.first().expect("entry point");
    let arena = &entry.function.expressions;
    let mut and_count = 0usize;
    let mut shr_count = 0usize;
    for (_, expr) in arena.iter() {
        if let naga::Expression::Binary { op, .. } = expr {
            if matches!(op, naga::BinaryOperator::And) {
                and_count += 1;
            }
            if matches!(op, naga::BinaryOperator::ShiftRight) {
                shr_count += 1;
            }
        }
    }
    assert_eq!(
        and_count, 0,
        "U32 LoadGlobal must not emit any byte-mask And ops; byte-extract path leaked"
    );
    assert_eq!(
        shr_count, 0,
        "U32 LoadGlobal must not emit any byte-shift ShiftRight ops"
    );
}

/// The byte-extract load/store chains are emitted for REAL byte-addressed
/// buffer scans, yet the shape tests above never ran naga's validator — a
/// latent-invalidity blind spot. The I8 LOAD in particular sign-extends with an
/// arithmetic `>> 24` on an i32 value: before the shift-amount fix (commit
/// 1a8682c399), `unify_binary_operand_types` coerced that u32 shift amount to
/// i32, emitting `ShiftRight(i32, i32)` which naga REJECTS — so the I8
/// byte-extract emitted invalid WGSL that no test caught. This pins that every
/// byte-extract geometry passes naga's real `Validator`, not just an `is_ok`.
#[test]
fn byte_extract_loads_and_stores_emit_valid_wgsl() {
    use naga::valid::{Capabilities, ValidationFlags, Validator};
    let mut validator = Validator::new(ValidationFlags::all(), Capabilities::all());
    let cases = [
        ("u8 load", byte_load_desc(DataType::U8)),
        ("i8 load (arithmetic >>24 sign-extend)", byte_load_desc(DataType::I8)),
        ("u32 load", byte_load_desc(DataType::U32)),
        ("u8 store", byte_store_desc(DataType::U8)),
        ("i8 store", byte_store_desc(DataType::I8)),
        ("u32 store", byte_store_desc(DataType::U32)),
    ];
    for (label, desc) in cases {
        let module = emit(&desc).unwrap_or_else(|e| panic!("{label}: emit failed: {e}"));
        validator
            .validate(&module)
            .unwrap_or_else(|e| panic!("{label}: byte-extract emitted INVALID WGSL: {e:?}"));
    }
}
