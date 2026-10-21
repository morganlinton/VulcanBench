use crate::ir::{AtomicOp, BinOp, UnOp};
use crate::serial::wire::encode::WireEncodeErr;

pub(crate) const ATOMIC_OP_TAGS: &[(u8, AtomicOp)] = &[
    (0x01, AtomicOp::Add),
    (0x02, AtomicOp::Or),
    (0x03, AtomicOp::And),
    (0x04, AtomicOp::Xor),
    (0x05, AtomicOp::Min),
    (0x06, AtomicOp::Max),
    (0x07, AtomicOp::Exchange),
    (0x08, AtomicOp::CompareExchange),
    (0x09, AtomicOp::CompareExchangeWeak),
    (0x0A, AtomicOp::FetchNand),
    (0x0B, AtomicOp::LruUpdate),
];

pub(crate) const BIN_OP_TAGS: &[(u8, BinOp)] = &[
    (0x01, BinOp::Add),
    (0x02, BinOp::Sub),
    (0x03, BinOp::Mul),
    (0x04, BinOp::Div),
    (0x05, BinOp::Mod),
    (0x06, BinOp::BitAnd),
    (0x07, BinOp::BitOr),
    (0x08, BinOp::BitXor),
    (0x09, BinOp::Shl),
    (0x0A, BinOp::Shr),
    (0x0B, BinOp::Eq),
    (0x0C, BinOp::Ne),
    (0x0D, BinOp::Lt),
    (0x0E, BinOp::Gt),
    (0x0F, BinOp::Le),
    (0x10, BinOp::Ge),
    (0x11, BinOp::And),
    (0x12, BinOp::Or),
    (0x13, BinOp::AbsDiff),
    (0x14, BinOp::Min),
    (0x15, BinOp::Max),
    (0x16, BinOp::SaturatingAdd),
    (0x17, BinOp::SaturatingSub),
    (0x18, BinOp::SaturatingMul),
    (0x19, BinOp::Shuffle),
    (0x1A, BinOp::Ballot),
    (0x1B, BinOp::WaveReduce),
    (0x1C, BinOp::WaveBroadcast),
    (0x1D, BinOp::RotateLeft),
    (0x1E, BinOp::RotateRight),
    (0x1F, BinOp::WrappingAdd),
    (0x20, BinOp::WrappingSub),
    (0x21, BinOp::MulHigh),
];

pub(crate) const UN_OP_TAGS: &[(u8, UnOp)] = &[
    (0x01, UnOp::Negate),
    (0x02, UnOp::BitNot),
    (0x03, UnOp::LogicalNot),
    (0x04, UnOp::Popcount),
    (0x05, UnOp::Clz),
    (0x06, UnOp::Ctz),
    (0x07, UnOp::ReverseBits),
    (0x08, UnOp::Cos),
    (0x09, UnOp::Sin),
    (0x0A, UnOp::Abs),
    (0x0B, UnOp::Sqrt),
    (0x0C, UnOp::Floor),
    (0x0D, UnOp::Ceil),
    (0x0E, UnOp::Round),
    (0x0F, UnOp::Trunc),
    (0x10, UnOp::Sign),
    (0x11, UnOp::IsNan),
    (0x12, UnOp::IsInf),
    (0x13, UnOp::IsFinite),
    (0x14, UnOp::Exp),
    (0x15, UnOp::Log),
    (0x16, UnOp::Log2),
    (0x17, UnOp::Exp2),
    (0x18, UnOp::Tan),
    (0x19, UnOp::Acos),
    (0x1A, UnOp::Asin),
    (0x1B, UnOp::Atan),
    (0x1C, UnOp::Tanh),
    (0x1D, UnOp::Sinh),
    (0x1E, UnOp::Cosh),
    (0x1F, UnOp::InverseSqrt),
    (0x20, UnOp::Unpack4Low),
    (0x21, UnOp::Unpack4High),
    (0x22, UnOp::Unpack8Low),
    (0x23, UnOp::Unpack8High),
    (0x24, UnOp::Reciprocal),
];

#[inline]
pub(crate) fn bin_op_from_tag(tag: u8) -> Result<BinOp, String> {
    decode_tag(tag, BIN_OP_TAGS, "binary")
}

#[inline]
pub(crate) fn un_op_from_tag(tag: u8) -> Result<UnOp, String> {
    decode_tag(tag, UN_OP_TAGS, "unary")
}

#[inline]
pub(crate) fn find_tag<T: PartialEq>(value: &T, table: &[(u8, T)]) -> Option<u8> {
    table
        .iter()
        .find_map(|(tag, candidate)| (candidate == value).then_some(*tag))
}

#[inline]
pub(crate) fn encode_tag<T: PartialEq>(
    value: &T,
    table: &[(u8, T)],
    unknown_variant: &'static str,
) -> Result<u8, WireEncodeErr> {
    find_tag(value, table).ok_or_else(|| WireEncodeErr::static_msg(unknown_variant))
}

pub(crate) fn decode_tag<T: Clone>(tag: u8, table: &[(u8, T)], kind: &str) -> Result<T, String> {
    table
        .iter()
        .find_map(|(candidate, value)| (*candidate == tag).then(|| value.clone()))
        .ok_or_else(|| format!("Fix: unknown {kind} op tag {tag}; use a compatible IR serializer."))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_op_tags_are_unique() {
        assert_unique(ATOMIC_OP_TAGS.iter().map(|(tag, _)| *tag));
    }

    #[test]
    fn bin_op_tags_are_unique() {
        assert_unique(BIN_OP_TAGS.iter().map(|(tag, _)| *tag));
    }

    #[test]
    fn un_op_tags_are_unique() {
        assert_unique(UN_OP_TAGS.iter().map(|(tag, _)| *tag));
    }

    #[test]
    fn every_declared_op_encodes_to_its_wire_tag() {
        for (tag, value) in ATOMIC_OP_TAGS {
            assert_eq!(find_tag(value, ATOMIC_OP_TAGS), Some(*tag));
        }
        for (tag, value) in BIN_OP_TAGS {
            assert_eq!(find_tag(value, BIN_OP_TAGS), Some(*tag));
        }
        for (tag, value) in UN_OP_TAGS {
            assert_eq!(find_tag(value, UN_OP_TAGS), Some(*tag));
        }
    }

    fn assert_unique(tags: impl Iterator<Item = u8>) {
        let mut seen = [false; 256];
        for tag in tags {
            assert!(!seen[tag as usize], "Fix: duplicate VIR0 op tag {tag}");
            seen[tag as usize] = true;
        }
    }
}
