use crate::ir::UnOp;
use crate::serial::wire::encode::WireEncodeErr;
use crate::serial::wire::tags::op_tag_decode::{encode_tag, UN_OP_TAGS};

/// Encode a [`UnOp`] into its stable VIR0 wire-format tag byte.
///
/// # Preconditions
///
/// `value` must be a variant known to the VIR0 encoder. Because `UnOp` is
/// `#[non_exhaustive]`, spec additions must receive a tag here before
/// they can round-trip through the wire format.
///
/// # Returns
///
/// `Ok(u8)` containing the tag value (`0..=18`).
///
/// # Failure mode
///
/// Returns `Err("unknown UnOp variant")` when the variant has no registered
/// tag. This prevents silent data loss on round-trip.
///
/// # Audit history
///
/// L.1.27 / I4: remaining f32 unary ops had no wire tags, breaking roundtrip
/// serialization for any Program that declared them. They now map to `11..=18`.
#[inline]
pub(crate) fn un_op_tag(value: &UnOp) -> Result<u8, WireEncodeErr> {
    encode_tag(value, UN_OP_TAGS, "unknown UnOp variant")
}
