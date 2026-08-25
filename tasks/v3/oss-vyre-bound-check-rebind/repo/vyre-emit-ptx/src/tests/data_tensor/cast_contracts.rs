use super::*;

#[test]
fn cast_emits_cvt_with_target_dtype() {
    let kernel = KernelDescriptor {
        id: "cast".into(),
        bindings: BindingLayout { slots: vec![] },
        dispatch: Dispatch::new(1, 1, 1),
        body: KernelBody {
            ops: vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::Cast {
                        target: DataType::F32,
                    },
                    operands: vec![0],
                    result: Some(1),
                },
            ],
            child_bodies: vec![],
            literals: vec![LiteralValue::U32(7)],
        },
    };
    let s = emit(&kernel).unwrap();
    assert!(s.contains("cvt.rn.f32.u32"));
}

/// U32 → U64 must zero-extend with `cvt.u64.u32`, never silently reinterpret.
#[test]
fn cast_u32_to_u64_zero_extends() {
    let kernel = KernelDescriptor {
        id: "cast_u32_u64".into(),
        bindings: BindingLayout { slots: vec![] },
        dispatch: Dispatch::new(1, 1, 1),
        body: KernelBody {
            ops: vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::Cast {
                        target: DataType::U64,
                    },
                    operands: vec![0],
                    result: Some(1),
                },
            ],
            child_bodies: vec![],
            literals: vec![LiteralValue::U32(7)],
        },
    };
    let s = emit(&kernel).unwrap();
    assert!(
        s.contains("cvt.u64.u32"),
        "U32 → U64 must zero-extend via cvt.u64.u32:\n{s}"
    );
}

/// U64 → U32 is an explicit narrowing that keeps the low 32 bits via
/// `cvt.u32.u64` (NOT a silent bit reinterpret).
#[test]
fn cast_u64_to_u32_truncates_low_word() {
    let kernel = KernelDescriptor {
        id: "cast_u64_u32".into(),
        bindings: BindingLayout { slots: vec![] },
        dispatch: Dispatch::new(1, 1, 1),
        body: KernelBody {
            ops: vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::Cast {
                        target: DataType::U64,
                    },
                    operands: vec![0],
                    result: Some(1),
                },
                KernelOp {
                    kind: KernelOpKind::Cast {
                        target: DataType::U32,
                    },
                    operands: vec![1],
                    result: Some(2),
                },
            ],
            child_bodies: vec![],
            literals: vec![LiteralValue::U32(9)],
        },
    };
    let s = emit(&kernel).unwrap();
    assert!(
        s.contains("cvt.u32.u64"),
        "U64 → U32 must narrow via cvt.u32.u64:\n{s}"
    );
}

/// U64 → I32 narrows to the low 32 bits via `cvt.u32.u64` (the low word's bit
/// pattern IS the i32). wgpu/naga supports this, so CUDA must too — never fail
/// closed.
#[test]
fn cast_u64_to_i32_narrows_low_word() {
    let kernel = KernelDescriptor {
        id: "cast_u64_i32".into(),
        bindings: BindingLayout { slots: vec![] },
        dispatch: Dispatch::new(1, 1, 1),
        body: KernelBody {
            ops: vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::Cast {
                        target: DataType::U64,
                    },
                    operands: vec![0],
                    result: Some(1),
                },
                KernelOp {
                    kind: KernelOpKind::Cast {
                        target: DataType::I32,
                    },
                    operands: vec![1],
                    result: Some(2),
                },
            ],
            child_bodies: vec![],
            literals: vec![LiteralValue::U32(9)],
        },
    };
    let s = emit(&kernel).unwrap();
    assert!(
        s.contains("cvt.u32.u64"),
        "U64 → I32 must narrow via cvt.u32.u64 (low word):\n{s}"
    );
}

/// U64 → Bool tests the FULL 64 bits (`setp.ne.u64 …, 0`), matching the
/// reference `value != 0` — never just the low word.
#[test]
fn cast_u64_to_bool_tests_full_width() {
    let kernel = KernelDescriptor {
        id: "cast_u64_bool".into(),
        bindings: BindingLayout { slots: vec![] },
        dispatch: Dispatch::new(1, 1, 1),
        body: KernelBody {
            ops: vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::Cast {
                        target: DataType::U64,
                    },
                    operands: vec![0],
                    result: Some(1),
                },
                KernelOp {
                    kind: KernelOpKind::Cast {
                        target: DataType::Bool,
                    },
                    operands: vec![1],
                    result: Some(2),
                },
            ],
            child_bodies: vec![],
            literals: vec![LiteralValue::U32(9)],
        },
    };
    let s = emit(&kernel).unwrap();
    assert!(
        s.contains("setp.ne.u64"),
        "U64 → Bool must test the full 64 bits via setp.ne.u64:\n{s}"
    );
}

/// I32 → U64 must sign-extend via `cvt.s64.s32` so negative values carry their
/// full 64-bit two's-complement pattern.
#[test]
fn cast_i32_to_u64_sign_extends() {
    let kernel = KernelDescriptor {
        id: "cast_i32_u64".into(),
        bindings: BindingLayout { slots: vec![] },
        dispatch: Dispatch::new(1, 1, 1),
        body: KernelBody {
            ops: vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::Cast {
                        target: DataType::I32,
                    },
                    operands: vec![0],
                    result: Some(1),
                },
                KernelOp {
                    kind: KernelOpKind::Cast {
                        target: DataType::U64,
                    },
                    operands: vec![1],
                    result: Some(2),
                },
            ],
            child_bodies: vec![],
            literals: vec![LiteralValue::U32(3)],
        },
    };
    let s = emit(&kernel).unwrap();
    assert!(
        s.contains("cvt.s64.s32"),
        "I32 → U64 must sign-extend via cvt.s64.s32:\n{s}"
    );
}

/// I32 → I64 must sign-extend via `cvt.s64.s32`, exactly like I32 → U64: the
/// I64 target shares the 64-bit register class (`PtxType::U64`). Before
/// `from_dtype` mapped I64, this valid cast (`cast_is_valid` allows i32→i64)
/// errored with `UnsupportedDataType(I64)`.
#[test]
fn cast_i32_to_i64_sign_extends() {
    let kernel = KernelDescriptor {
        id: "cast_i32_i64".into(),
        bindings: BindingLayout { slots: vec![] },
        dispatch: Dispatch::new(1, 1, 1),
        body: KernelBody {
            ops: vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::Cast {
                        target: DataType::I32,
                    },
                    operands: vec![0],
                    result: Some(1),
                },
                KernelOp {
                    kind: KernelOpKind::Cast {
                        target: DataType::I64,
                    },
                    operands: vec![1],
                    result: Some(2),
                },
            ],
            child_bodies: vec![],
            literals: vec![LiteralValue::U32(3)],
        },
    };
    let s = emit(&kernel).unwrap();
    assert!(
        s.contains("cvt.s64.s32"),
        "I32 → I64 must sign-extend via cvt.s64.s32:\n{s}"
    );
}

/// U32 → I64 must zero-extend via `cvt.u64.u32` (non-negative source), the
/// unsigned twin of the sign-extend above.
#[test]
fn cast_u32_to_i64_zero_extends() {
    let kernel = KernelDescriptor {
        id: "cast_u32_i64".into(),
        bindings: BindingLayout { slots: vec![] },
        dispatch: Dispatch::new(1, 1, 1),
        body: KernelBody {
            ops: vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::Cast {
                        target: DataType::I64,
                    },
                    operands: vec![0],
                    result: Some(1),
                },
            ],
            child_bodies: vec![],
            literals: vec![LiteralValue::U32(9)],
        },
    };
    let s = emit(&kernel).unwrap();
    assert!(
        s.contains("cvt.u64.u32"),
        "U32 → I64 must zero-extend via cvt.u64.u32:\n{s}"
    );
}

#[test]
fn f32_to_bool_cast_uses_unordered_not_equal_for_nan_truthiness() {
    let kernel = KernelDescriptor {
        id: "cast_f32_bool".into(),
        bindings: BindingLayout { slots: vec![] },
        dispatch: Dispatch::new(1, 1, 1),
        body: KernelBody {
            ops: vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::Cast {
                        target: DataType::Bool,
                    },
                    operands: vec![0],
                    result: Some(1),
                },
            ],
            child_bodies: vec![],
            literals: vec![LiteralValue::F32(f32::NAN)],
        },
    };

    let s = emit(&kernel).unwrap();
    assert!(
        s.contains("setp.neu.f32"),
        "f32 truthiness must treat NaN as true to match reference casts:\n{s}"
    );
}

#[test]
fn f32_not_equal_comparison_uses_unordered_predicate_for_nan_truthiness() {
    let kernel = KernelDescriptor {
        id: "f32_ne_nan".into(),
        bindings: BindingLayout { slots: vec![] },
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
                    kind: KernelOpKind::BinOpKind(BinOp::Ne),
                    operands: vec![0, 1],
                    result: Some(2),
                },
            ],
            child_bodies: vec![],
            literals: vec![LiteralValue::F32(f32::NAN), LiteralValue::F32(1.0)],
        },
    };

    let s = emit(&kernel).unwrap();
    assert!(
        s.contains("setp.neu.f32"),
        "f32 Ne must be unordered-not-equal so NaN != x matches the reference oracle:\n{s}"
    );
}

#[test]
fn bool_to_f32_cast_materializes_predicate_before_numeric_conversion() {
    let kernel = KernelDescriptor {
        id: "cast_bool_f32".into(),
        bindings: BindingLayout { slots: vec![] },
        dispatch: Dispatch::new(1, 1, 1),
        body: KernelBody {
            ops: vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::Cast {
                        target: DataType::F32,
                    },
                    operands: vec![0],
                    result: Some(1),
                },
            ],
            child_bodies: vec![],
            literals: vec![LiteralValue::Bool(true)],
        },
    };

    let s = emit(&kernel).unwrap();
    assert!(
        s.contains("selp.u32") && s.contains("cvt.rn.f32.u32"),
        "Bool->F32 must materialize %p as a u32 word before cvt; PTX cannot cvt directly from predicate registers:\n{s}"
    );
}

#[test]
fn bool_to_i32_cast_materializes_predicate_word() {
    let kernel = KernelDescriptor {
        id: "cast_bool_i32".into(),
        bindings: BindingLayout { slots: vec![] },
        dispatch: Dispatch::new(1, 1, 1),
        body: KernelBody {
            ops: vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::Cast {
                        target: DataType::I32,
                    },
                    operands: vec![0],
                    result: Some(1),
                },
            ],
            child_bodies: vec![],
            literals: vec![LiteralValue::Bool(true)],
        },
    };

    let s = emit(&kernel).unwrap();
    assert!(
        s.contains("selp.u32"),
        "Bool->I32 must materialize %p as a 0/1 word:\n{s}"
    );
}

fn f32_cast_kernel(target: DataType) -> KernelDescriptor {
    KernelDescriptor {
        id: "cast_f32".into(),
        bindings: BindingLayout { slots: vec![] },
        dispatch: Dispatch::new(1, 1, 1),
        body: KernelBody {
            ops: vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::Cast { target },
                    operands: vec![0],
                    result: Some(1),
                },
            ],
            child_bodies: vec![],
            literals: vec![LiteralValue::F32(3.5)],
        },
    }
}

/// A float source has no defined narrowing integer conversion. `from_dtype`
/// collapses U8/U16->U32 and I8/I16->I32, which would otherwise let an f32->u8
/// silently emit a non-narrowing `cvt.rzi.u32.f32` (a full u32-range value
/// claimed as a u8). The validator rejects these casts; the emitter must ALSO
/// fail closed (Law 10), matching the naga emitter.
#[test]
fn f32_to_narrow_int_cast_fails_closed() {
    for target in [DataType::U8, DataType::U16, DataType::I8, DataType::I16] {
        let err = emit(&f32_cast_kernel(target.clone())).expect_err(&format!(
            "f32 -> {target:?} has no defined float conversion and must fail closed, not emit"
        ));
        let msg = format!("{err:?}");
        assert!(
            msg.contains("cast from f32 to")
                && msg.contains("no defined conversion")
                && msg.contains("Fix:"),
            "f32 -> {target:?} must fail closed with the actionable float-cast message, got: {msg}"
        );
    }
}

/// The 64-bit twins: `from_dtype` maps U64/I64 to `PtxType::U64` (!= F32), so the
/// `(F32, U64)` pair has no `emit_cast` arm and already fails closed. Pin it so a
/// future arm cannot silently reintroduce a high-word-dropping f32->u64 path.
#[test]
fn f32_to_wide_int_cast_fails_closed() {
    for target in [DataType::U64, DataType::I64] {
        assert!(
            emit(&f32_cast_kernel(target.clone())).is_err(),
            "f32 -> {target:?} must fail closed (no defined float-to-64-bit-int conversion)"
        );
    }
}

/// The positive twin: the float targets the validator permits (u32/i32 saturating,
/// bool truthy, f32 identity) must keep emitting — the fail-closed guard must not
/// over-reach.
#[test]
fn f32_to_permitted_targets_still_emit() {
    for target in [DataType::U32, DataType::I32, DataType::Bool, DataType::F32] {
        emit(&f32_cast_kernel(target.clone()))
            .unwrap_or_else(|e| panic!("f32 -> {target:?} is a permitted cast and must emit: {e}"));
    }
}

/// A `u32`-source cast kernel: load a u32 literal, cast it to `target`. Used to
/// exercise the integer narrowing path (`from_dtype` collapses u8/u16->u32 and
/// i8/i16->i32, so a same-width identity check would skip the truncation).
fn u32_cast_kernel(target: DataType) -> KernelDescriptor {
    KernelDescriptor {
        id: "cast_u32".into(),
        bindings: BindingLayout { slots: vec![] },
        dispatch: Dispatch::new(1, 1, 1),
        body: KernelBody {
            ops: vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::Cast { target },
                    operands: vec![0],
                    result: Some(1),
                },
            ],
            child_bodies: vec![],
            literals: vec![LiteralValue::U32(300)],
        },
    }
}

/// Unsigned narrowing `u32 -> u8/u16` must TRUNCATE to the low byte/half via the
/// canonical PTX zero-extending convert (`cvt.u32.u8` / `cvt.u32.u16`), NOT keep
/// the full 32-bit word. Before the narrow path, `from_dtype(U8) == U32 == src`
/// hit the `src.0 == dst_ty` early-return and emitted no narrowing at all,
/// silently diverging from Rust `as u8`, the V035 contract, and the oracle.
#[test]
fn u32_to_unsigned_narrow_emits_zero_extending_convert() {
    for (target, instr) in [
        (DataType::U8, "cvt.u32.u8"),
        (DataType::U16, "cvt.u32.u16"),
    ] {
        let s = emit(&u32_cast_kernel(target.clone()))
            .unwrap_or_else(|e| panic!("u32 -> {target:?} must emit: {e}"));
        assert!(
            s.contains(instr),
            "u32 -> {target:?} must narrow via {instr} (truncate high bits):\n{s}"
        );
    }
}

/// Signed narrowing `u32 -> i8/i16` must truncate then SIGN-extend from the new
/// top bit via the canonical PTX sign-extending convert (`cvt.s32.s8` /
/// `cvt.s32.s16`), matching Rust `as i8/i16` and the reference oracle.
#[test]
fn u32_to_signed_narrow_emits_sign_extending_convert() {
    for (target, instr) in [
        (DataType::I8, "cvt.s32.s8"),
        (DataType::I16, "cvt.s32.s16"),
    ] {
        let s = emit(&u32_cast_kernel(target.clone()))
            .unwrap_or_else(|e| panic!("u32 -> {target:?} must emit: {e}"));
        assert!(
            s.contains(instr),
            "u32 -> {target:?} must sign-extend via {instr}:\n{s}"
        );
    }
}
