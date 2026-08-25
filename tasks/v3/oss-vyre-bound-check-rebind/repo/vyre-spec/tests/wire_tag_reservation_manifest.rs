//! Executable checks for source-level wire-tag reservation manifests.
//!
//! The frozen tag comments are part of the data contract external backend
//! authors read first. These tests compare the comments to the public API so
//! a future enum edit cannot leave documentation and executable tags divergent.

use std::collections::BTreeMap;

use vyre_spec::{
    AtomicOp, BinOp, CollectiveOp, DataType, QuantizationScale, QuantizationZeroPoint, TernaryOp,
    TypeId, UnOp,
};

#[test]
fn data_type_tag_reservation_manifest_matches_public_api() {
    let cases = [
        ("U32", DataType::U32.builtin_wire_tag().unwrap()),
        ("I32", DataType::I32.builtin_wire_tag().unwrap()),
        ("U64", DataType::U64.builtin_wire_tag().unwrap()),
        ("Vec2U32", DataType::Vec2U32.builtin_wire_tag().unwrap()),
        ("Vec4U32", DataType::Vec4U32.builtin_wire_tag().unwrap()),
        ("Bool", DataType::Bool.builtin_wire_tag().unwrap()),
        ("Bytes", DataType::Bytes.builtin_wire_tag().unwrap()),
        (
            "Array",
            DataType::Array { element_size: 4 }
                .builtin_wire_tag()
                .unwrap(),
        ),
        ("F16", DataType::F16.builtin_wire_tag().unwrap()),
        ("BF16", DataType::BF16.builtin_wire_tag().unwrap()),
        ("F32", DataType::F32.builtin_wire_tag().unwrap()),
        ("F64", DataType::F64.builtin_wire_tag().unwrap()),
        ("Tensor", DataType::Tensor.builtin_wire_tag().unwrap()),
        ("U8", DataType::U8.builtin_wire_tag().unwrap()),
        ("U16", DataType::U16.builtin_wire_tag().unwrap()),
        ("I8", DataType::I8.builtin_wire_tag().unwrap()),
        ("I16", DataType::I16.builtin_wire_tag().unwrap()),
        ("I64", DataType::I64.builtin_wire_tag().unwrap()),
        (
            "Handle",
            DataType::Handle(TypeId(1)).builtin_wire_tag().unwrap(),
        ),
        (
            "Vec",
            DataType::Vec {
                element: Box::new(DataType::U32),
                count: 4,
            }
            .builtin_wire_tag()
            .unwrap(),
        ),
        (
            "TensorShaped",
            DataType::TensorShaped {
                element: Box::new(DataType::F32),
                shape: [2, 4].as_slice().into(),
            }
            .builtin_wire_tag()
            .unwrap(),
        ),
        (
            "SparseCsr",
            DataType::SparseCsr {
                element: Box::new(DataType::F32),
            }
            .builtin_wire_tag()
            .unwrap(),
        ),
        (
            "SparseCoo",
            DataType::SparseCoo {
                element: Box::new(DataType::F32),
            }
            .builtin_wire_tag()
            .unwrap(),
        ),
        (
            "SparseBsr",
            DataType::SparseBsr {
                element: Box::new(DataType::F32),
                block_rows: 2,
                block_cols: 4,
            }
            .builtin_wire_tag()
            .unwrap(),
        ),
        ("F8E4M3", DataType::F8E4M3.builtin_wire_tag().unwrap()),
        ("F8E5M2", DataType::F8E5M2.builtin_wire_tag().unwrap()),
        ("I4", DataType::I4.builtin_wire_tag().unwrap()),
        ("FP4", DataType::FP4.builtin_wire_tag().unwrap()),
        ("NF4", DataType::NF4.builtin_wire_tag().unwrap()),
        (
            "DeviceMesh",
            DataType::DeviceMesh {
                axes: [2, 2].as_slice().into(),
            }
            .builtin_wire_tag()
            .unwrap(),
        ),
        (
            "Quantized",
            DataType::Quantized {
                storage: Box::new(DataType::I4),
                scale: QuantizationScale::PerGroup { group_size: 32 },
                zero_point: QuantizationZeroPoint::Absent,
            }
            .builtin_wire_tag()
            .unwrap(),
        ),
    ];

    assert_reservation_manifest_matches("DataType", include_str!("../src/data_type.rs"), &cases);
}

#[test]
fn operation_tag_reservation_manifests_match_public_api() {
    assert_reservation_manifest_matches(
        "AtomicOp",
        include_str!("../src/atomic_op.rs"),
        &[
            ("Add", AtomicOp::Add.builtin_wire_tag().unwrap()),
            ("Or", AtomicOp::Or.builtin_wire_tag().unwrap()),
            ("And", AtomicOp::And.builtin_wire_tag().unwrap()),
            ("Xor", AtomicOp::Xor.builtin_wire_tag().unwrap()),
            ("Min", AtomicOp::Min.builtin_wire_tag().unwrap()),
            ("Max", AtomicOp::Max.builtin_wire_tag().unwrap()),
            ("Exchange", AtomicOp::Exchange.builtin_wire_tag().unwrap()),
            (
                "CompareExchange",
                AtomicOp::CompareExchange.builtin_wire_tag().unwrap(),
            ),
            (
                "CompareExchangeWeak",
                AtomicOp::CompareExchangeWeak.builtin_wire_tag().unwrap(),
            ),
            ("FetchNand", AtomicOp::FetchNand.builtin_wire_tag().unwrap()),
            ("LruUpdate", AtomicOp::LruUpdate.builtin_wire_tag().unwrap()),
        ],
    );
    assert_reservation_manifest_matches(
        "BinOp",
        include_str!("../src/bin_op.rs"),
        &[
            ("Add", BinOp::Add.builtin_wire_tag().unwrap()),
            ("Sub", BinOp::Sub.builtin_wire_tag().unwrap()),
            ("Mul", BinOp::Mul.builtin_wire_tag().unwrap()),
            ("Div", BinOp::Div.builtin_wire_tag().unwrap()),
            ("Mod", BinOp::Mod.builtin_wire_tag().unwrap()),
            ("BitAnd", BinOp::BitAnd.builtin_wire_tag().unwrap()),
            ("BitOr", BinOp::BitOr.builtin_wire_tag().unwrap()),
            ("BitXor", BinOp::BitXor.builtin_wire_tag().unwrap()),
            ("Shl", BinOp::Shl.builtin_wire_tag().unwrap()),
            ("Shr", BinOp::Shr.builtin_wire_tag().unwrap()),
            ("Eq", BinOp::Eq.builtin_wire_tag().unwrap()),
            ("Ne", BinOp::Ne.builtin_wire_tag().unwrap()),
            ("Lt", BinOp::Lt.builtin_wire_tag().unwrap()),
            ("Gt", BinOp::Gt.builtin_wire_tag().unwrap()),
            ("AbsDiff", BinOp::AbsDiff.builtin_wire_tag().unwrap()),
            ("Le", BinOp::Le.builtin_wire_tag().unwrap()),
            ("Ge", BinOp::Ge.builtin_wire_tag().unwrap()),
            ("And", BinOp::And.builtin_wire_tag().unwrap()),
            ("Or", BinOp::Or.builtin_wire_tag().unwrap()),
            ("Min", BinOp::Min.builtin_wire_tag().unwrap()),
            ("Max", BinOp::Max.builtin_wire_tag().unwrap()),
            (
                "SaturatingAdd",
                BinOp::SaturatingAdd.builtin_wire_tag().unwrap(),
            ),
            (
                "SaturatingSub",
                BinOp::SaturatingSub.builtin_wire_tag().unwrap(),
            ),
            (
                "SaturatingMul",
                BinOp::SaturatingMul.builtin_wire_tag().unwrap(),
            ),
            ("Shuffle", BinOp::Shuffle.builtin_wire_tag().unwrap()),
            ("Ballot", BinOp::Ballot.builtin_wire_tag().unwrap()),
            ("WaveReduce", BinOp::WaveReduce.builtin_wire_tag().unwrap()),
            (
                "WaveBroadcast",
                BinOp::WaveBroadcast.builtin_wire_tag().unwrap(),
            ),
            ("RotateLeft", BinOp::RotateLeft.builtin_wire_tag().unwrap()),
            (
                "RotateRight",
                BinOp::RotateRight.builtin_wire_tag().unwrap(),
            ),
            (
                "WrappingAdd",
                BinOp::WrappingAdd.builtin_wire_tag().unwrap(),
            ),
            (
                "WrappingSub",
                BinOp::WrappingSub.builtin_wire_tag().unwrap(),
            ),
            ("MulHigh", BinOp::MulHigh.builtin_wire_tag().unwrap()),
        ],
    );
    assert_reservation_manifest_matches(
        "UnOp",
        include_str!("../src/un_op.rs"),
        &[
            ("Negate", UnOp::Negate.builtin_wire_tag().unwrap()),
            ("BitNot", UnOp::BitNot.builtin_wire_tag().unwrap()),
            ("LogicalNot", UnOp::LogicalNot.builtin_wire_tag().unwrap()),
            ("Popcount", UnOp::Popcount.builtin_wire_tag().unwrap()),
            ("Clz", UnOp::Clz.builtin_wire_tag().unwrap()),
            ("Ctz", UnOp::Ctz.builtin_wire_tag().unwrap()),
            ("ReverseBits", UnOp::ReverseBits.builtin_wire_tag().unwrap()),
            ("Cos", UnOp::Cos.builtin_wire_tag().unwrap()),
            ("Sin", UnOp::Sin.builtin_wire_tag().unwrap()),
            ("Abs", UnOp::Abs.builtin_wire_tag().unwrap()),
            ("Sqrt", UnOp::Sqrt.builtin_wire_tag().unwrap()),
            ("Floor", UnOp::Floor.builtin_wire_tag().unwrap()),
            ("Ceil", UnOp::Ceil.builtin_wire_tag().unwrap()),
            ("Round", UnOp::Round.builtin_wire_tag().unwrap()),
            ("Trunc", UnOp::Trunc.builtin_wire_tag().unwrap()),
            ("Sign", UnOp::Sign.builtin_wire_tag().unwrap()),
            ("IsNan", UnOp::IsNan.builtin_wire_tag().unwrap()),
            ("IsInf", UnOp::IsInf.builtin_wire_tag().unwrap()),
            ("IsFinite", UnOp::IsFinite.builtin_wire_tag().unwrap()),
            ("Exp", UnOp::Exp.builtin_wire_tag().unwrap()),
            ("Log", UnOp::Log.builtin_wire_tag().unwrap()),
            ("Log2", UnOp::Log2.builtin_wire_tag().unwrap()),
            ("Exp2", UnOp::Exp2.builtin_wire_tag().unwrap()),
            ("Tan", UnOp::Tan.builtin_wire_tag().unwrap()),
            ("Acos", UnOp::Acos.builtin_wire_tag().unwrap()),
            ("Asin", UnOp::Asin.builtin_wire_tag().unwrap()),
            ("Atan", UnOp::Atan.builtin_wire_tag().unwrap()),
            ("Tanh", UnOp::Tanh.builtin_wire_tag().unwrap()),
            ("Sinh", UnOp::Sinh.builtin_wire_tag().unwrap()),
            ("Cosh", UnOp::Cosh.builtin_wire_tag().unwrap()),
            ("InverseSqrt", UnOp::InverseSqrt.builtin_wire_tag().unwrap()),
            ("Unpack4Low", UnOp::Unpack4Low.builtin_wire_tag().unwrap()),
            ("Unpack4High", UnOp::Unpack4High.builtin_wire_tag().unwrap()),
            ("Unpack8Low", UnOp::Unpack8Low.builtin_wire_tag().unwrap()),
            ("Unpack8High", UnOp::Unpack8High.builtin_wire_tag().unwrap()),
            ("Reciprocal", UnOp::Reciprocal.builtin_wire_tag().unwrap()),
        ],
    );
    assert_reservation_manifest_matches(
        "TernaryOp",
        include_str!("../src/ternary_op.rs"),
        &[
            ("Fma", TernaryOp::Fma.builtin_wire_tag().unwrap()),
            ("Select", TernaryOp::Select.builtin_wire_tag().unwrap()),
        ],
    );
    assert_reservation_manifest_matches(
        "CollectiveOp",
        include_str!("../src/collective_op.rs"),
        &[
            ("Sum", CollectiveOp::Sum.builtin_wire_tag()),
            ("Min", CollectiveOp::Min.builtin_wire_tag()),
            ("Max", CollectiveOp::Max.builtin_wire_tag()),
            ("BitAnd", CollectiveOp::BitAnd.builtin_wire_tag()),
            ("BitOr", CollectiveOp::BitOr.builtin_wire_tag()),
            ("BitXor", CollectiveOp::BitXor.builtin_wire_tag()),
        ],
    );
}

fn assert_reservation_manifest_matches(family: &str, source: &str, expected: &[(&str, u8)]) {
    let reservations = parse_tag_reservations(source);
    let expected_map = expected
        .iter()
        .map(|(name, tag)| ((*name).to_owned(), *tag))
        .collect::<BTreeMap<_, _>>();

    assert_eq!(
        reservations.len(),
        expected_map.len(),
        "Fix: {family} TAG RESERVATIONS entry count must match public builtin variants."
    );
    for (variant, tag) in expected_map {
        assert_eq!(
            reservations.get(&variant),
            Some(&tag),
            "Fix: {family} TAG RESERVATIONS for {variant} must match builtin_wire_tag()."
        );
    }
}

fn parse_tag_reservations(source: &str) -> BTreeMap<String, u8> {
    let mut manifest = String::new();
    let mut collecting = false;
    for line in source.lines() {
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("// TAG RESERVATIONS:") {
            collecting = true;
            manifest.push_str(rest);
            manifest.push(' ');
            continue;
        }
        if !collecting {
            continue;
        }
        let Some(rest) = trimmed.strip_prefix("//") else {
            break;
        };
        let rest = rest.trim();
        if rest.is_empty() || (!rest.contains('=') && !rest.contains("reserved")) {
            break;
        }
        manifest.push_str(rest);
        manifest.push(' ');
    }

    let mut reservations = BTreeMap::new();
    for entry in manifest.split(',') {
        let entry = entry.trim().trim_end_matches('.');
        let Some((name, tag_text)) = entry.split_once('=') else {
            continue;
        };
        let name = name.trim();
        if name == "Opaque" || name.starts_with("0x") {
            continue;
        }
        let tag = tag_text
            .split_whitespace()
            .next()
            .expect("Fix: reservation tag must have a numeric value.");
        let parsed = u8::from_str_radix(tag.trim_start_matches("0x"), 16)
            .expect("Fix: reservation tag must be hexadecimal u8.");
        assert!(
            reservations.insert(name.to_owned(), parsed).is_none(),
            "Fix: duplicate TAG RESERVATIONS entry for {name}."
        );
    }
    reservations
}
