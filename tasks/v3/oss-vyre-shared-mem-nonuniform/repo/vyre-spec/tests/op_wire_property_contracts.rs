//! Generated property coverage for builtin and opaque operation wire tags.

use proptest::prelude::*;
use vyre_spec::extension::{
    ExtensionAtomicOpId, ExtensionBinOpId, ExtensionTernaryOpId, ExtensionUnOpId,
};
use vyre_spec::{AtomicOp, BinOp, CollectiveOp, TernaryOp, UnOp};

fn extension_raw_id() -> impl Strategy<Value = u32> {
    any::<u32>().prop_map(|raw| raw | 0x8000_0000)
}

fn bin_op_strategy() -> impl Strategy<Value = BinOp> {
    prop_oneof![
        Just(BinOp::Add),
        Just(BinOp::Sub),
        Just(BinOp::Mul),
        Just(BinOp::Div),
        Just(BinOp::Mod),
        Just(BinOp::WrappingAdd),
        Just(BinOp::WrappingSub),
        Just(BinOp::BitAnd),
        Just(BinOp::BitOr),
        Just(BinOp::BitXor),
        Just(BinOp::Shl),
        Just(BinOp::Shr),
        Just(BinOp::Eq),
        Just(BinOp::Ne),
        Just(BinOp::Lt),
        Just(BinOp::Gt),
        Just(BinOp::Le),
        Just(BinOp::Ge),
        Just(BinOp::And),
        Just(BinOp::Or),
        Just(BinOp::AbsDiff),
        Just(BinOp::Min),
        Just(BinOp::Max),
        Just(BinOp::SaturatingAdd),
        Just(BinOp::SaturatingSub),
        Just(BinOp::SaturatingMul),
        Just(BinOp::Shuffle),
        Just(BinOp::Ballot),
        Just(BinOp::WaveReduce),
        Just(BinOp::WaveBroadcast),
        Just(BinOp::RotateLeft),
        Just(BinOp::RotateRight),
        Just(BinOp::MulHigh),
        extension_raw_id().prop_map(|raw| BinOp::Opaque(ExtensionBinOpId(raw))),
    ]
}

fn un_op_strategy() -> impl Strategy<Value = UnOp> {
    prop_oneof![
        Just(UnOp::Negate),
        Just(UnOp::BitNot),
        Just(UnOp::LogicalNot),
        Just(UnOp::Popcount),
        Just(UnOp::Clz),
        Just(UnOp::Ctz),
        Just(UnOp::ReverseBits),
        Just(UnOp::Cos),
        Just(UnOp::Sin),
        Just(UnOp::Abs),
        Just(UnOp::Sqrt),
        Just(UnOp::Floor),
        Just(UnOp::Ceil),
        Just(UnOp::Round),
        Just(UnOp::Trunc),
        Just(UnOp::Sign),
        Just(UnOp::IsNan),
        Just(UnOp::IsInf),
        Just(UnOp::IsFinite),
        Just(UnOp::Exp),
        Just(UnOp::Log),
        Just(UnOp::Log2),
        Just(UnOp::Exp2),
        Just(UnOp::Tan),
        Just(UnOp::Acos),
        Just(UnOp::Asin),
        Just(UnOp::Atan),
        Just(UnOp::Tanh),
        Just(UnOp::Sinh),
        Just(UnOp::Cosh),
        Just(UnOp::InverseSqrt),
        Just(UnOp::Unpack4Low),
        Just(UnOp::Unpack4High),
        Just(UnOp::Unpack8Low),
        Just(UnOp::Unpack8High),
        Just(UnOp::Reciprocal),
        extension_raw_id().prop_map(|raw| UnOp::Opaque(ExtensionUnOpId(raw))),
    ]
}

fn atomic_op_strategy() -> impl Strategy<Value = AtomicOp> {
    prop_oneof![
        Just(AtomicOp::Add),
        Just(AtomicOp::Or),
        Just(AtomicOp::And),
        Just(AtomicOp::Xor),
        Just(AtomicOp::Min),
        Just(AtomicOp::Max),
        Just(AtomicOp::Exchange),
        Just(AtomicOp::CompareExchange),
        Just(AtomicOp::CompareExchangeWeak),
        Just(AtomicOp::FetchNand),
        Just(AtomicOp::LruUpdate),
        extension_raw_id().prop_map(|raw| AtomicOp::Opaque(ExtensionAtomicOpId(raw))),
    ]
}

fn ternary_op_strategy() -> impl Strategy<Value = TernaryOp> {
    prop_oneof![
        Just(TernaryOp::Fma),
        Just(TernaryOp::Select),
        extension_raw_id().prop_map(|raw| TernaryOp::Opaque(ExtensionTernaryOpId(raw))),
    ]
}

fn collective_op_strategy() -> impl Strategy<Value = CollectiveOp> {
    prop_oneof![
        Just(CollectiveOp::Sum),
        Just(CollectiveOp::Min),
        Just(CollectiveOp::Max),
        Just(CollectiveOp::BitAnd),
        Just(CollectiveOp::BitOr),
        Just(CollectiveOp::BitXor),
    ]
}

fn assert_builtin_tag_is_reserved(tag: Option<u8>) -> Result<(), TestCaseError> {
    if let Some(tag) = tag {
        prop_assert!(
            (1..=0x7f).contains(&tag),
            "Fix: builtin op tags must stay in 0x01..=0x7f"
        );
    }
    Ok(())
}

proptest! {
    #[test]
    fn generated_bin_ops_round_trip_and_keep_builtin_tags_reserved(op in bin_op_strategy()) {
        assert_builtin_tag_is_reserved(op.builtin_wire_tag())?;
        let encoded = serde_json::to_string(&op).expect("Fix: BinOp must serialize");
        let decoded: BinOp = serde_json::from_str(&encoded).expect("Fix: BinOp must deserialize");
        prop_assert_eq!(decoded, op);
    }

    #[test]
    fn generated_un_ops_round_trip_and_keep_builtin_tags_reserved(op in un_op_strategy()) {
        assert_builtin_tag_is_reserved(op.builtin_wire_tag())?;
        let encoded = serde_json::to_string(&op).expect("Fix: UnOp must serialize");
        let decoded: UnOp = serde_json::from_str(&encoded).expect("Fix: UnOp must deserialize");
        prop_assert_eq!(decoded, op);
    }

    #[test]
    fn generated_atomic_ops_round_trip_and_keep_builtin_tags_reserved(op in atomic_op_strategy()) {
        assert_builtin_tag_is_reserved(op.builtin_wire_tag())?;
        let encoded = serde_json::to_string(&op).expect("Fix: AtomicOp must serialize");
        let decoded: AtomicOp = serde_json::from_str(&encoded).expect("Fix: AtomicOp must deserialize");
        prop_assert_eq!(decoded, op);
    }

    #[test]
    fn generated_ternary_ops_round_trip_and_keep_builtin_tags_reserved(op in ternary_op_strategy()) {
        assert_builtin_tag_is_reserved(op.builtin_wire_tag())?;
        let encoded = serde_json::to_string(&op).expect("Fix: TernaryOp must serialize");
        let decoded: TernaryOp = serde_json::from_str(&encoded).expect("Fix: TernaryOp must deserialize");
        prop_assert_eq!(decoded, op);
    }

    #[test]
    fn generated_collective_ops_round_trip_and_decode_wire_tags(op in collective_op_strategy()) {
        assert_builtin_tag_is_reserved(Some(op.builtin_wire_tag()))?;
        prop_assert_eq!(
            CollectiveOp::from_wire_tag(op.builtin_wire_tag())
                .expect("Fix: CollectiveOp builtin tag must decode"),
            op
        );
        let encoded = serde_json::to_string(&op).expect("Fix: CollectiveOp must serialize");
        let decoded: CollectiveOp = serde_json::from_str(&encoded).expect("Fix: CollectiveOp must deserialize");
        prop_assert_eq!(decoded, op);
    }
}
