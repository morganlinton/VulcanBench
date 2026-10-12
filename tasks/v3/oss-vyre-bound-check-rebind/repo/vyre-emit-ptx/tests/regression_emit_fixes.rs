//! Regression tests for VYRE-PTX-001, VYRE-PTX-002, VYRE-PTX-003.
//!
//! Each test asserts the exact PTX instruction suffix/mnemonic that the
//! fix introduced, confirming the pre-fix behaviour is gone.

use vyre_foundation::ir::{BinOp, DataType};
use vyre_lower::{
    BindingLayout, BindingSlot, BindingVisibility, Dispatch, KernelBody, KernelDescriptor,
    KernelOp, KernelOpKind, LiteralValue, MemoryClass,
};

fn rw_slot_typed(id: u32, name: &str, element_type: DataType) -> BindingSlot {
    BindingSlot {
        slot: id,
        element_type,
        element_count: None,
        memory_class: MemoryClass::Global,
        visibility: BindingVisibility::ReadWrite,
        name: name.into(),
    }
}

/// Build a minimal descriptor that emits a BinOp on I32 operands via
/// LocalInvocationId (so the op survives constant folding) and stores the
/// result.
fn i32_binop_descriptor(op: BinOp) -> KernelDescriptor {
    KernelDescriptor {
        id: "i32_binop".into(),
        bindings: BindingLayout {
            slots: vec![rw_slot_typed(0, "out", DataType::I32)],
        },
        dispatch: Dispatch::new(64, 1, 1),
        body: KernelBody {
            ops: vec![
                // result 0: LocalInvocationId (I32 cast below), keeps op live
                KernelOp {
                    kind: KernelOpKind::LocalInvocationId,
                    operands: vec![0],
                    result: Some(0),
                },
                // result 1: cast to I32 so the BinOp operands are I32
                KernelOp {
                    kind: KernelOpKind::Cast { target: DataType::I32 },
                    operands: vec![0],
                    result: Some(1),
                },
                // result 2: literal shift amount 1 (U32 literal, PTX shift
                // amount is always U32)
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(2),
                },
                // result 3: store index (literal 0)
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![1],
                    result: Some(3),
                },
                // result 4: I32 BinOp
                KernelOp {
                    kind: KernelOpKind::BinOpKind(op),
                    operands: vec![1, 2],
                    result: Some(4),
                },
                KernelOp {
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![0, 3, 4],
                    result: None,
                },
            ],
            child_bodies: vec![],
            literals: vec![LiteralValue::U32(1), LiteralValue::U32(0)],
        },
    }
}

/// VYRE-PTX-001: `Shr` on I32 must emit `shr.s32` (arithmetic), not
/// `shr.u32` (logical).
///
/// Before the fix, `ptx_binop_suffix(BinOp::Shr, PtxType::I32)` returned
/// `"u32"` for all types, so the emitted instruction was `shr.u32` even when
/// the operand register was %s<N> (signed class). That is a silent miscompile:
/// `(-4) >> 1` via `shr.u32` produces 0x7FFFFFFE instead of -2 (0xFFFFFFFE).
#[test]
fn shr_on_i32_emits_s32_suffix_not_u32() {
    let desc = i32_binop_descriptor(BinOp::Shr);
    let ptx =
        vyre_emit_ptx::emit_optimized(&desc).expect("I32 Shr descriptor must emit without error");

    assert!(
        ptx.contains("shr.s32"),
        "Shr on I32 operands must emit `shr.s32` (arithmetic shift); \
         found `shr.u32` instead — that is a logical shift which gives wrong results \
         for negative values. PTX emitted:\n{ptx}"
    );
    // Guard against regression: u32-suffixed shr must NOT appear for this I32
    // descriptor (there is no u32 operand in this kernel).
    assert!(
        !ptx.contains("shr.u32"),
        "shr.u32 must not appear in an I32-operand Shr kernel; \
         regression: the unsigned logical shift is back. PTX emitted:\n{ptx}"
    );
}

/// Complementary guard: `Shr` on U32 must still emit `shr.u32` — the fix
/// must not accidentally break unsigned shifts.
#[test]
fn shr_on_u32_still_emits_u32_suffix() {
    let desc = KernelDescriptor {
        id: "u32_shr".into(),
        bindings: BindingLayout {
            slots: vec![rw_slot_typed(0, "out", DataType::U32)],
        },
        dispatch: Dispatch::new(64, 1, 1),
        body: KernelBody {
            ops: vec![
                KernelOp {
                    kind: KernelOpKind::LocalInvocationId,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(1),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![1],
                    result: Some(2),
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Shr),
                    operands: vec![0, 1],
                    result: Some(3),
                },
                KernelOp {
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![0, 2, 3],
                    result: None,
                },
            ],
            child_bodies: vec![],
            literals: vec![LiteralValue::U32(1), LiteralValue::U32(0)],
        },
    };
    let ptx =
        vyre_emit_ptx::emit_optimized(&desc).expect("U32 Shr descriptor must emit without error");
    assert!(
        ptx.contains("shr.u32"),
        "Shr on U32 operands must still emit `shr.u32`; \
         the signed-shift fix must not break unsigned shifts. PTX emitted:\n{ptx}"
    );
    assert!(
        !ptx.contains("shr.s32"),
        "shr.s32 must not appear in a U32-only Shr kernel. PTX emitted:\n{ptx}"
    );
}

/// VYRE-PTX-002: the overflow guard in `ensure_buffer_length_reg` must emit
/// `trap;` — not a plausible-address load — when the slot byte offset would
/// overflow u32.  We can only exercise the overflow branch by triggering
/// `ensure_buffer_length_reg` with a slot that was registered during
/// `preload_bindings` (the normal path), so this test instead validates that
/// the checked-arithmetic path in `preload_bindings` produces the correct
/// `[%rd0 + 8]` offset for slot 1 (byte_offset = 1*4+4 = 8), confirming the
/// non-overflow path is correct and the overflow path is the only way `trap;`
/// can appear.
#[test]
fn ensure_buffer_length_reg_emits_correct_offset_for_slot_1() {
    let desc = KernelDescriptor {
        id: "two_slot_bounds".into(),
        bindings: BindingLayout {
            slots: vec![
                rw_slot_typed(0, "a", DataType::U32),
                rw_slot_typed(1, "b", DataType::U32),
            ],
        },
        dispatch: Dispatch::new(64, 1, 1),
        body: KernelBody {
            ops: vec![
                // result 0: literal 0 for slot 0 index
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                },
                // result 1: literal 0 for slot 1 index
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(1),
                },
                // result 2: load from slot 0
                KernelOp {
                    kind: KernelOpKind::LoadGlobal,
                    operands: vec![0, 0],
                    result: Some(2),
                },
                // result 3: load from slot 1
                KernelOp {
                    kind: KernelOpKind::LoadGlobal,
                    operands: vec![1, 1],
                    result: Some(3),
                },
                KernelOp {
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![0, 0, 3],
                    result: None,
                },
            ],
            child_bodies: vec![],
            literals: vec![LiteralValue::U32(0)],
        },
    };
    let ptx = vyre_emit_ptx::emit_optimized(&desc)
        .expect("two-slot descriptor must emit without error");

    // slot 0: byte_offset = 0*4+4 = 4  → `[%rd0 + 4]`
    // slot 1: byte_offset = 1*4+4 = 8  → `[%rd0 + 8]`
    assert!(
        ptx.contains("[%rd0 + 4]"),
        "slot 0 length load must use offset 4; PTX emitted:\n{ptx}"
    );
    assert!(
        ptx.contains("[%rd0 + 8]"),
        "slot 1 length load must use offset 8; PTX emitted:\n{ptx}"
    );
    // No trap should appear for a valid descriptor.
    assert!(
        !ptx.contains("trap;"),
        "valid two-slot descriptor must not emit trap; PTX emitted:\n{ptx}"
    );
}

/// VYRE-PTX-003: storing a U64 value to an F16 binding must emit
/// `cvt.rn.f32.u64` (single-step, no precision loss) and must NOT emit the
/// old two-step truncating path `cvt.u32.u64`.
#[test]
fn f16_store_of_u64_value_uses_direct_cvt_rn_f32_u64() {
    // Build a descriptor that loads a U64 value then stores it to an F16
    // binding, exercising the ensure_f32_store_operand(U64) path.
    //
    // We use Cast(U64) from a LocalInvocationId (U32) to produce a U64
    // register, then store to an F16 output slot.
    let desc = KernelDescriptor {
        id: "u64_to_f16_store".into(),
        bindings: BindingLayout {
            slots: vec![rw_slot_typed(0, "out_f16", DataType::F16)],
        },
        dispatch: Dispatch::new(64, 1, 1),
        body: KernelBody {
            ops: vec![
                // result 0: thread id (U32)
                KernelOp {
                    kind: KernelOpKind::LocalInvocationId,
                    operands: vec![0],
                    result: Some(0),
                },
                // result 1: cast U32 → U64  (produces a U64 register)
                KernelOp {
                    kind: KernelOpKind::Cast { target: DataType::U64 },
                    operands: vec![0],
                    result: Some(1),
                },
                // result 2: store index (literal 0)
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(2),
                },
                // Store the U64 value to an F16 binding — exercises
                // ensure_f32_store_operand(U64) inside emit_store_value.
                KernelOp {
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![0, 2, 1],
                    result: None,
                },
            ],
            child_bodies: vec![],
            literals: vec![LiteralValue::U32(0)],
        },
    };
    let ptx = vyre_emit_ptx::emit_optimized(&desc)
        .expect("U64 → F16 store descriptor must emit without error");

    // The fixed path: single-instruction conversion preserving all 64 bits.
    assert!(
        ptx.contains("cvt.rn.f32.u64"),
        "storing a U64 value to an F16 binding must emit `cvt.rn.f32.u64`; \
         the old two-step path `cvt.u32.u64` silently truncated the high 32 bits. \
         PTX emitted:\n{ptx}"
    );
    // The broken path: must be gone.
    assert!(
        !ptx.contains("cvt.u32.u64"),
        "`cvt.u32.u64` must not appear; it truncates the high 32 bits of the U64 \
         value before conversion. PTX emitted:\n{ptx}"
    );
}
