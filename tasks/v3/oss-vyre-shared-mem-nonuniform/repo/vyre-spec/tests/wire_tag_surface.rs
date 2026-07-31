//! External freeze tests for core builtin wire tags.
//!
//! These tests make the wire tag manifest executable. A backend or wire codec
//! must not depend on comments for numeric tags; the public spec API exposes
//! the frozen builtin tag for each core enum and excludes extension ids from
//! that builtin space.

use std::collections::BTreeSet;

use vyre_spec::extension::{
    ExtensionAtomicOpId, ExtensionBinOpId, ExtensionDataTypeId, ExtensionTernaryOpId,
    ExtensionUnOpId,
};
use vyre_spec::{
    AtomicOp, BinOp, CollectiveOp, DataType, QuantizationScale, QuantizationZeroPoint, TernaryOp,
    TypeId, UnOp,
};

#[test]
fn data_type_builtin_wire_tags_are_exact_and_unique() {
    let cases = [
        (DataType::U32, 0x01),
        (DataType::I32, 0x02),
        (DataType::U64, 0x03),
        (DataType::Vec2U32, 0x04),
        (DataType::Vec4U32, 0x05),
        (DataType::Bool, 0x06),
        (DataType::Bytes, 0x07),
        (DataType::Array { element_size: 4 }, 0x08),
        (DataType::F16, 0x09),
        (DataType::BF16, 0x0A),
        (DataType::F32, 0x0B),
        (DataType::F64, 0x0C),
        (DataType::Tensor, 0x0D),
        (DataType::U8, 0x0E),
        (DataType::U16, 0x0F),
        (DataType::I8, 0x10),
        (DataType::I16, 0x11),
        (DataType::I64, 0x12),
        (DataType::Handle(TypeId(7)), 0x13),
        (
            DataType::Vec {
                element: Box::new(DataType::U32),
                count: 4,
            },
            0x14,
        ),
        (
            DataType::TensorShaped {
                element: Box::new(DataType::F32),
                shape: [2, 3].as_slice().into(),
            },
            0x15,
        ),
        (
            DataType::SparseCsr {
                element: Box::new(DataType::F32),
            },
            0x16,
        ),
        (
            DataType::SparseCoo {
                element: Box::new(DataType::F32),
            },
            0x17,
        ),
        (
            DataType::SparseBsr {
                element: Box::new(DataType::F32),
                block_rows: 2,
                block_cols: 4,
            },
            0x18,
        ),
        (DataType::F8E4M3, 0x19),
        (DataType::F8E5M2, 0x1A),
        (DataType::I4, 0x1B),
        (DataType::FP4, 0x1C),
        (DataType::NF4, 0x1D),
        (
            DataType::DeviceMesh {
                axes: [2, 2].as_slice().into(),
            },
            0x1E,
        ),
        (
            DataType::Quantized {
                storage: Box::new(DataType::I4),
                scale: QuantizationScale::PerGroup { group_size: 128 },
                zero_point: QuantizationZeroPoint::Absent,
            },
            0x1F,
        ),
    ];

    assert_exact_unique_tags(cases.map(|(kind, tag)| (kind.builtin_wire_tag(), tag)));
}

#[test]
fn bin_op_builtin_wire_tags_are_exact_and_unique() {
    let cases = [
        (BinOp::Add, 0x01),
        (BinOp::Sub, 0x02),
        (BinOp::Mul, 0x03),
        (BinOp::Div, 0x04),
        (BinOp::Mod, 0x05),
        (BinOp::BitAnd, 0x06),
        (BinOp::BitOr, 0x07),
        (BinOp::BitXor, 0x08),
        (BinOp::Shl, 0x09),
        (BinOp::Shr, 0x0A),
        (BinOp::Eq, 0x0B),
        (BinOp::Ne, 0x0C),
        (BinOp::Lt, 0x0D),
        (BinOp::Gt, 0x0E),
        (BinOp::AbsDiff, 0x0F),
        (BinOp::Le, 0x10),
        (BinOp::Ge, 0x11),
        (BinOp::And, 0x12),
        (BinOp::Or, 0x13),
        (BinOp::Min, 0x14),
        (BinOp::Max, 0x15),
        (BinOp::SaturatingAdd, 0x16),
        (BinOp::SaturatingSub, 0x17),
        (BinOp::SaturatingMul, 0x18),
        (BinOp::Shuffle, 0x19),
        (BinOp::Ballot, 0x1A),
        (BinOp::WaveReduce, 0x1B),
        (BinOp::WaveBroadcast, 0x1C),
        (BinOp::RotateLeft, 0x1D),
        (BinOp::RotateRight, 0x1E),
        (BinOp::WrappingAdd, 0x1F),
        (BinOp::WrappingSub, 0x20),
        (BinOp::MulHigh, 0x21),
    ];

    assert_exact_unique_tags(cases.map(|(kind, tag)| (kind.builtin_wire_tag(), tag)));
}

#[test]
fn un_op_builtin_wire_tags_are_exact_and_unique() {
    let cases = [
        (UnOp::Negate, 0x01),
        (UnOp::BitNot, 0x02),
        (UnOp::LogicalNot, 0x03),
        (UnOp::Popcount, 0x04),
        (UnOp::Clz, 0x05),
        (UnOp::Ctz, 0x06),
        (UnOp::ReverseBits, 0x07),
        (UnOp::Cos, 0x08),
        (UnOp::Sin, 0x09),
        (UnOp::Abs, 0x0A),
        (UnOp::Sqrt, 0x0B),
        (UnOp::Floor, 0x0C),
        (UnOp::Ceil, 0x0D),
        (UnOp::Round, 0x0E),
        (UnOp::Trunc, 0x0F),
        (UnOp::Sign, 0x10),
        (UnOp::IsNan, 0x11),
        (UnOp::IsInf, 0x12),
        (UnOp::IsFinite, 0x13),
        (UnOp::Exp, 0x14),
        (UnOp::Log, 0x15),
        (UnOp::Log2, 0x16),
        (UnOp::Exp2, 0x17),
        (UnOp::Tan, 0x18),
        (UnOp::Acos, 0x19),
        (UnOp::Asin, 0x1A),
        (UnOp::Atan, 0x1B),
        (UnOp::Tanh, 0x1C),
        (UnOp::Sinh, 0x1D),
        (UnOp::Cosh, 0x1E),
        (UnOp::InverseSqrt, 0x1F),
        (UnOp::Unpack4Low, 0x20),
        (UnOp::Unpack4High, 0x21),
        (UnOp::Unpack8Low, 0x22),
        (UnOp::Unpack8High, 0x23),
        (UnOp::Reciprocal, 0x24),
    ];

    assert_exact_unique_tags(cases.map(|(kind, tag)| (kind.builtin_wire_tag(), tag)));
}

#[test]
fn atomic_op_builtin_wire_tags_are_exact_and_unique() {
    let cases = [
        (AtomicOp::Add, 0x01),
        (AtomicOp::Or, 0x02),
        (AtomicOp::And, 0x03),
        (AtomicOp::Xor, 0x04),
        (AtomicOp::Min, 0x05),
        (AtomicOp::Max, 0x06),
        (AtomicOp::Exchange, 0x07),
        (AtomicOp::CompareExchange, 0x08),
        (AtomicOp::CompareExchangeWeak, 0x09),
        (AtomicOp::FetchNand, 0x0A),
        (AtomicOp::LruUpdate, 0x0B),
    ];

    assert_exact_unique_tags(cases.map(|(kind, tag)| (kind.builtin_wire_tag(), tag)));
}

#[test]
fn ternary_op_builtin_wire_tags_are_exact_and_unique() {
    let cases = [(TernaryOp::Fma, 0x01), (TernaryOp::Select, 0x02)];

    assert_exact_unique_tags(cases.map(|(kind, tag)| (kind.builtin_wire_tag(), tag)));
}

#[test]
fn collective_op_builtin_wire_tags_are_exact_unique_and_decodable() {
    let cases = [
        (CollectiveOp::Sum, 0x01),
        (CollectiveOp::Min, 0x02),
        (CollectiveOp::Max, 0x03),
        (CollectiveOp::BitAnd, 0x04),
        (CollectiveOp::BitOr, 0x05),
        (CollectiveOp::BitXor, 0x06),
    ];
    let mut seen = BTreeSet::new();

    for (op, tag) in cases {
        assert_eq!(op.builtin_wire_tag(), tag);
        assert!(
            seen.insert(tag),
            "duplicate CollectiveOp wire tag {tag:#04x}"
        );
        assert_eq!(
            CollectiveOp::from_wire_tag(tag).expect("Fix: frozen collective tag must decode"),
            op
        );
    }
}

#[test]
fn collective_op_rejects_reserved_and_extension_tag_space() {
    for tag in [0x00, 0x07, 0x7f, 0x80, 0xff] {
        let error = CollectiveOp::from_wire_tag(tag)
            .expect_err("Fix: unassigned CollectiveOp wire tag must be rejected");
        assert!(
            error.contains("Fix: unknown CollectiveOp tag"),
            "collective tag error must be actionable, got: {error}"
        );
    }
}

#[test]
fn opaque_variants_are_not_builtin_wire_tags() {
    assert_eq!(
        DataType::Opaque(ExtensionDataTypeId::from_name("custom.dtype")).builtin_wire_tag(),
        None
    );
    assert_eq!(
        BinOp::Opaque(ExtensionBinOpId::from_name("custom.binop")).builtin_wire_tag(),
        None
    );
    assert_eq!(
        UnOp::Opaque(ExtensionUnOpId::from_name("custom.unop")).builtin_wire_tag(),
        None
    );
    assert_eq!(
        AtomicOp::Opaque(ExtensionAtomicOpId::from_name("custom.atomic")).builtin_wire_tag(),
        None
    );
    assert_eq!(
        TernaryOp::Opaque(ExtensionTernaryOpId::from_name("custom.ternary")).builtin_wire_tag(),
        None
    );
}

#[test]
fn extension_ids_stay_outside_builtin_tag_range() {
    let ids = [
        ExtensionDataTypeId::from_name("custom.dtype").as_u32(),
        ExtensionBinOpId::from_name("custom.binop").as_u32(),
        ExtensionUnOpId::from_name("custom.unop").as_u32(),
        ExtensionAtomicOpId::from_name("custom.atomic").as_u32(),
        ExtensionTernaryOpId::from_name("custom.ternary").as_u32(),
    ];

    for id in ids {
        assert_ne!(
            id & ExtensionDataTypeId::EXTENSION_RANGE_MASK,
            0,
            "extension id {id:#010x} must keep the high bit set"
        );
        assert!(
            id > 0x7F,
            "extension id {id:#010x} must not overlap builtin/reserved one-byte tag space"
        );
    }
}

fn assert_exact_unique_tags<const N: usize>(cases: [(Option<u8>, u8); N]) {
    let mut seen = BTreeSet::new();

    for (actual, expected) in cases {
        assert_eq!(actual, Some(expected));
        assert!(
            (0x01..=0x7F).contains(&expected),
            "builtin tag {expected:#04x} must stay in the core one-byte range"
        );
        assert!(
            seen.insert(expected),
            "builtin tag {expected:#04x} was assigned to more than one variant"
        );
    }
}
