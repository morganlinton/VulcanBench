use crate::ir::BinOp;
use crate::serial::wire::encode::WireEncodeErr;
use crate::serial::wire::tags::op_tag_decode::{encode_tag, BIN_OP_TAGS};

/// Encode a [`BinOp`] into its stable VIR0 wire-format tag byte.
///
/// # Preconditions
///
/// `value` must be a variant known to the VIR0 encoder. Because `BinOp` is
/// `#[non_exhaustive]`, spec additions must receive a tag here before
/// they can round-trip through the wire format.
///
/// # Returns
///
/// `Ok(u8)` containing the tag value (`0..=20`).
///
/// # Failure mode
///
/// Returns `Err("unknown BinOp variant")` when the variant has no registered
/// tag. This prevents silent data loss on round-trip.
///
/// # Audit history
///
/// L.1.27 / I4: Min and Max had no tag and were rejected at serialize time,
/// breaking `Program::from_wire(Program::to_wire(p))` for any program that
/// legitimately declared a Min/Max `BinOp`. They now map to `19` and `20`.
#[inline]
pub(crate) fn bin_op_tag(value: BinOp) -> Result<u8, WireEncodeErr> {
    encode_tag(&value, BIN_OP_TAGS, "unknown BinOp variant")
}
