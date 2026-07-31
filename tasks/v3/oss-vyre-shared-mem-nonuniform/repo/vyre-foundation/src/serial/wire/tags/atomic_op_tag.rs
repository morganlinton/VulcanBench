use crate::ir::AtomicOp;
use crate::serial::wire::encode::WireEncodeErr;
use crate::serial::wire::tags::op_tag_decode::{encode_tag, ATOMIC_OP_TAGS};

/// Encode an [`AtomicOp`] into its stable VIR0 wire-format tag byte.
///
/// # Preconditions
///
/// `value` must be a variant known to the VIR0 encoder. Because `AtomicOp`
/// is `#[non_exhaustive]`, spec additions must receive a tag here
/// before they can round-trip through the wire format.
///
/// # Returns
///
/// `Ok(u8)` containing the tag value (`0..=7`).
///
/// # Failure mode
///
/// Returns `Err("unknown AtomicOp variant")` when the variant has no
/// registered tag. This prevents silent data loss on round-trip.
#[inline]
pub(crate) fn atomic_op_tag(value: AtomicOp) -> Result<u8, WireEncodeErr> {
    encode_tag(&value, ATOMIC_OP_TAGS, "unknown AtomicOp variant")
}
