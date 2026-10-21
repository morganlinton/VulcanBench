//! Test: frozen wire contracts.

use crate::extension::{
    ExtensionAtomicOpId, ExtensionBinOpId, ExtensionDataTypeId, ExtensionTernaryOpId,
    ExtensionUnOpId,
};
use crate::{
    AtomicOp, BinOp, CollectiveOp, CommGroup, Convention, DataType, Layer, MetadataCategory,
    QuantizationScale, QuantizationZeroPoint, Semiring, TernaryOp, TypeId, UnOp,
};

mod support;

use support::{
    assert_json_roundtrip, assert_pairwise_unique_tags, assert_payload_independent_tag,
    data_type_wire_cases,
};

#[test]
fn operation_wire_tags_are_frozen_unique_and_reserved_below_extension_space() {
    let bin = [
        ("Add", BinOp::Add, 0x01),
        ("Sub", BinOp::Sub, 0x02),
        ("Mul", BinOp::Mul, 0x03),
        ("Div", BinOp::Div, 0x04),
        ("Mod", BinOp::Mod, 0x05),
        ("BitAnd", BinOp::BitAnd, 0x06),
        ("BitOr", BinOp::BitOr, 0x07),
        ("BitXor", BinOp::BitXor, 0x08),
        ("Shl", BinOp::Shl, 0x09),
        ("Shr", BinOp::Shr, 0x0A),
        ("Eq", BinOp::Eq, 0x0B),
        ("Ne", BinOp::Ne, 0x0C),
        ("Lt", BinOp::Lt, 0x0D),
        ("Gt", BinOp::Gt, 0x0E),
        ("AbsDiff", BinOp::AbsDiff, 0x0F),
        ("Le", BinOp::Le, 0x10),
        ("Ge", BinOp::Ge, 0x11),
        ("And", BinOp::And, 0x12),
        ("Or", BinOp::Or, 0x13),
        ("Min", BinOp::Min, 0x14),
        ("Max", BinOp::Max, 0x15),
        ("SaturatingAdd", BinOp::SaturatingAdd, 0x16),
        ("SaturatingSub", BinOp::SaturatingSub, 0x17),
        ("SaturatingMul", BinOp::SaturatingMul, 0x18),
        ("Shuffle", BinOp::Shuffle, 0x19),
        ("Ballot", BinOp::Ballot, 0x1A),
        ("WaveReduce", BinOp::WaveReduce, 0x1B),
        ("WaveBroadcast", BinOp::WaveBroadcast, 0x1C),
        ("RotateLeft", BinOp::RotateLeft, 0x1D),
        ("RotateRight", BinOp::RotateRight, 0x1E),
        ("WrappingAdd", BinOp::WrappingAdd, 0x1F),
        ("WrappingSub", BinOp::WrappingSub, 0x20),
        ("MulHigh", BinOp::MulHigh, 0x21),
    ];
    let bin_tags = bin
        .iter()
        .map(|(name, op, expected)| {
            assert_eq!(
                op.builtin_wire_tag(),
                Some(*expected),
                "Fix: BinOp::{name} wire tag drifted"
            );
            (*name, *expected)
        })
        .collect::<Vec<_>>();
    assert_pairwise_unique_tags("BinOp", &bin_tags);
    assert_eq!(
        BinOp::Opaque(ExtensionBinOpId::from_name("test.bin")).builtin_wire_tag(),
        None
    );

    let atomic = [
        ("Add", AtomicOp::Add, 0x01),
        ("Or", AtomicOp::Or, 0x02),
        ("And", AtomicOp::And, 0x03),
        ("Xor", AtomicOp::Xor, 0x04),
        ("Min", AtomicOp::Min, 0x05),
        ("Max", AtomicOp::Max, 0x06),
        ("Exchange", AtomicOp::Exchange, 0x07),
        ("CompareExchange", AtomicOp::CompareExchange, 0x08),
        ("CompareExchangeWeak", AtomicOp::CompareExchangeWeak, 0x09),
        ("FetchNand", AtomicOp::FetchNand, 0x0A),
        ("LruUpdate", AtomicOp::LruUpdate, 0x0B),
    ];
    let atomic_tags = atomic
        .iter()
        .map(|(name, op, expected)| {
            assert_eq!(
                op.builtin_wire_tag(),
                Some(*expected),
                "Fix: AtomicOp::{name} wire tag drifted"
            );
            (*name, *expected)
        })
        .collect::<Vec<_>>();
    assert_pairwise_unique_tags("AtomicOp", &atomic_tags);
    assert_eq!(
        AtomicOp::Opaque(ExtensionAtomicOpId::from_name("test.atomic")).builtin_wire_tag(),
        None
    );

    let un = [
        ("Negate", UnOp::Negate, 0x01),
        ("BitNot", UnOp::BitNot, 0x02),
        ("LogicalNot", UnOp::LogicalNot, 0x03),
        ("Popcount", UnOp::Popcount, 0x04),
        ("Clz", UnOp::Clz, 0x05),
        ("Ctz", UnOp::Ctz, 0x06),
        ("ReverseBits", UnOp::ReverseBits, 0x07),
        ("Cos", UnOp::Cos, 0x08),
        ("Sin", UnOp::Sin, 0x09),
        ("Abs", UnOp::Abs, 0x0A),
        ("Sqrt", UnOp::Sqrt, 0x0B),
        ("Floor", UnOp::Floor, 0x0C),
        ("Ceil", UnOp::Ceil, 0x0D),
        ("Round", UnOp::Round, 0x0E),
        ("Trunc", UnOp::Trunc, 0x0F),
        ("Sign", UnOp::Sign, 0x10),
        ("IsNan", UnOp::IsNan, 0x11),
        ("IsInf", UnOp::IsInf, 0x12),
        ("IsFinite", UnOp::IsFinite, 0x13),
        ("Exp", UnOp::Exp, 0x14),
        ("Log", UnOp::Log, 0x15),
        ("Log2", UnOp::Log2, 0x16),
        ("Exp2", UnOp::Exp2, 0x17),
        ("Tan", UnOp::Tan, 0x18),
        ("Acos", UnOp::Acos, 0x19),
        ("Asin", UnOp::Asin, 0x1A),
        ("Atan", UnOp::Atan, 0x1B),
        ("Tanh", UnOp::Tanh, 0x1C),
        ("Sinh", UnOp::Sinh, 0x1D),
        ("Cosh", UnOp::Cosh, 0x1E),
        ("InverseSqrt", UnOp::InverseSqrt, 0x1F),
        ("Unpack4Low", UnOp::Unpack4Low, 0x20),
        ("Unpack4High", UnOp::Unpack4High, 0x21),
        ("Unpack8Low", UnOp::Unpack8Low, 0x22),
        ("Unpack8High", UnOp::Unpack8High, 0x23),
        ("Reciprocal", UnOp::Reciprocal, 0x24),
    ];
    let un_tags = un
        .iter()
        .map(|(name, op, expected)| {
            assert_eq!(
                op.builtin_wire_tag(),
                Some(*expected),
                "Fix: UnOp::{name} wire tag drifted"
            );
            (*name, *expected)
        })
        .collect::<Vec<_>>();
    assert_pairwise_unique_tags("UnOp", &un_tags);
    assert_eq!(
        UnOp::Opaque(ExtensionUnOpId::from_name("test.un")).builtin_wire_tag(),
        None
    );

    let ternary = [
        ("Fma", TernaryOp::Fma, 0x01),
        ("Select", TernaryOp::Select, 0x02),
    ];
    let ternary_tags = ternary
        .iter()
        .map(|(name, op, expected)| {
            assert_eq!(
                op.builtin_wire_tag(),
                Some(*expected),
                "Fix: TernaryOp::{name} wire tag drifted"
            );
            (*name, *expected)
        })
        .collect::<Vec<_>>();
    assert_pairwise_unique_tags("TernaryOp", &ternary_tags);
    assert_eq!(
        TernaryOp::Opaque(ExtensionTernaryOpId::from_name("test.ternary")).builtin_wire_tag(),
        None
    );
}

#[test]
fn collective_wire_tags_decode_exactly_and_reject_all_reserved_values() {
    let assigned = [
        ("Sum", CollectiveOp::Sum, 0x01),
        ("Min", CollectiveOp::Min, 0x02),
        ("Max", CollectiveOp::Max, 0x03),
        ("BitAnd", CollectiveOp::BitAnd, 0x04),
        ("BitOr", CollectiveOp::BitOr, 0x05),
        ("BitXor", CollectiveOp::BitXor, 0x06),
    ];
    let tags = assigned
        .iter()
        .map(|(name, op, expected)| {
            assert_eq!(
                op.builtin_wire_tag(),
                *expected,
                "Fix: CollectiveOp::{name} wire tag drifted"
            );
            assert_eq!(
                CollectiveOp::from_wire_tag(*expected),
                Ok(*op),
                "Fix: CollectiveOp::{name} no longer decodes from its frozen tag"
            );
            (*name, *expected)
        })
        .collect::<Vec<_>>();
    assert_pairwise_unique_tags("CollectiveOp", &tags);

    let assigned_by_tag = tags
        .iter()
        .map(|(_, tag)| *tag)
        .collect::<std::collections::BTreeSet<_>>();
    let mut checked = 0usize;
    for tag in u8::MIN..=u8::MAX {
        let decoded = CollectiveOp::from_wire_tag(tag);
        if assigned_by_tag.contains(&tag) {
            assert!(
                decoded.is_ok(),
                "Fix: assigned collective tag {tag:#04x} must decode"
            );
        } else {
            let err = decoded.expect_err("Fix: reserved collective tag unexpectedly decoded");
            assert!(
                err.contains("Fix:"),
                "Fix: reserved collective tag {tag:#04x} produced a non-actionable error: {err}"
            );
        }
        checked += 1;
    }
    assert_eq!(checked, 256);
}

#[test]
fn data_type_wire_tags_are_frozen_unique_and_payload_invariant() {
    let cases = data_type_wire_cases();
    for (name, ty, expected) in &cases {
        assert_eq!(
            ty.builtin_wire_tag(),
            Some(*expected),
            "Fix: DataType::{name} wire tag drifted"
        );
    }
    let tags = cases
        .iter()
        .map(|(name, _, tag)| (*name, *tag))
        .collect::<Vec<_>>();
    assert_pairwise_unique_tags("DataType", &tags);
    assert_eq!(
        DataType::Opaque(ExtensionDataTypeId::from_name("test.dtype")).builtin_wire_tag(),
        None
    );

    assert_payload_independent_tag(
        "Array",
        &[
            DataType::Array { element_size: 1 },
            DataType::Array { element_size: 4096 },
        ],
        0x08,
    );
    assert_payload_independent_tag(
        "Handle",
        &[
            DataType::Handle(TypeId(1)),
            DataType::Handle(TypeId(u32::MAX)),
        ],
        0x13,
    );
    assert_payload_independent_tag(
        "Vec",
        &[
            DataType::Vec {
                element: Box::new(DataType::U8),
                count: 4,
            },
            DataType::Vec {
                element: Box::new(DataType::F32),
                count: 32,
            },
        ],
        0x14,
    );
    assert_payload_independent_tag(
        "TensorShaped",
        &[
            DataType::TensorShaped {
                element: Box::new(DataType::F16),
                shape: smallvec::smallvec![1, 2, 3],
            },
            DataType::TensorShaped {
                element: Box::new(DataType::U32),
                shape: smallvec::smallvec![64, 128],
            },
        ],
        0x15,
    );
    assert_payload_independent_tag(
        "SparseBsr",
        &[
            DataType::SparseBsr {
                element: Box::new(DataType::I4),
                block_rows: 1,
                block_cols: 1,
            },
            DataType::SparseBsr {
                element: Box::new(DataType::F32),
                block_rows: 16,
                block_cols: 32,
            },
        ],
        0x18,
    );
    assert_payload_independent_tag(
        "DeviceMesh",
        &[
            DataType::DeviceMesh {
                axes: smallvec::smallvec![8],
            },
            DataType::DeviceMesh {
                axes: smallvec::smallvec![2, 4, 8],
            },
        ],
        0x1E,
    );
    assert_payload_independent_tag(
        "Quantized",
        &[
            DataType::Quantized {
                storage: Box::new(DataType::I4),
                scale: QuantizationScale::PerTensor,
                zero_point: QuantizationZeroPoint::Absent,
            },
            DataType::Quantized {
                storage: Box::new(DataType::F8E4M3),
                scale: QuantizationScale::PerGroup { group_size: 64 },
                zero_point: QuantizationZeroPoint::PerChannel { axis: 1 },
            },
        ],
        0x1F,
    );
}

#[test]
fn extension_ids_are_deterministic_high_bit_separated_from_builtin_tags() {
    let names = [
        "dtype.quant.i4",
        "bin.mulhigh.fast",
        "un.unpack.fp4",
        "atomic.lru.update",
        "ternary.mask.merge",
    ];
    for name in names {
        let dtype = ExtensionDataTypeId::from_name(name);
        let bin = ExtensionBinOpId::from_name(name);
        let un = ExtensionUnOpId::from_name(name);
        let atomic = ExtensionAtomicOpId::from_name(name);
        let ternary = ExtensionTernaryOpId::from_name(name);

        assert_eq!(dtype, ExtensionDataTypeId::from_name(name));
        assert_eq!(bin, ExtensionBinOpId::from_name(name));
        assert_eq!(un, ExtensionUnOpId::from_name(name));
        assert_eq!(atomic, ExtensionAtomicOpId::from_name(name));
        assert_eq!(ternary, ExtensionTernaryOpId::from_name(name));

        assert!(
            dtype.is_extension(),
            "Fix: dtype extension id lost its high bit"
        );
        assert!(
            bin.is_extension(),
            "Fix: bin extension id lost its high bit"
        );
        assert!(
            un.is_extension(),
            "Fix: unary extension id lost its high bit"
        );
        assert!(
            atomic.is_extension(),
            "Fix: atomic extension id lost its high bit"
        );
        assert!(
            ternary.is_extension(),
            "Fix: ternary extension id lost its high bit"
        );

        for (_, _, builtin) in data_type_wire_cases() {
            assert!(
                u32::from(builtin) < ExtensionDataTypeId::EXTENSION_RANGE_MASK,
                "Fix: builtin tag {builtin:#04x} overlaps extension id space"
            );
        }
    }
}

#[test]
fn serde_json_roundtrips_representative_frozen_contract_values() {
    assert_json_roundtrip(BinOp::MulHigh);
    assert_json_roundtrip(UnOp::InverseSqrt);
    assert_json_roundtrip(AtomicOp::CompareExchangeWeak);
    assert_json_roundtrip(TernaryOp::Select);
    assert_json_roundtrip(CollectiveOp::BitXor);
    assert_json_roundtrip(Semiring::Gf2);
    assert_json_roundtrip(MetadataCategory::C);
    assert_json_roundtrip(Layer::L5);
    assert_json_roundtrip(Convention::V2 { lookup_binding: 7 });
    assert_json_roundtrip(CommGroup::WORLD);
    assert_json_roundtrip(CommGroup(u32::MAX));
    assert_json_roundtrip(DataType::Quantized {
        storage: Box::new(DataType::NF4),
        scale: QuantizationScale::PerGroup { group_size: 128 },
        zero_point: QuantizationZeroPoint::PerTensor,
    });
    assert_json_roundtrip(DataType::SparseBsr {
        element: Box::new(DataType::F8E5M2),
        block_rows: 16,
        block_cols: 8,
    });
    assert_json_roundtrip(DataType::DeviceMesh {
        axes: smallvec::smallvec![2, 4, 8],
    });
}
