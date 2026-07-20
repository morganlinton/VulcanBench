use crate::ir::AtomicOp;
use crate::serial::wire::tags::op_tag_decode::{decode_tag, ATOMIC_OP_TAGS};

/// Decode an [`AtomicOp`] from its VIR0 wire-format tag byte.
///
/// # Preconditions
///
/// `tag` must be a tag assigned by the VIR0 specification. Values outside
/// the defined tag space indicate a version mismatch or malformed input.
///
/// # Returns
///
/// `Ok(AtomicOp)` on a recognized tag. The mapping is stable:
/// `1 → Add`, `2 → Or`, `3 → And`, `4 → Xor`, `5 → Min`, `6 → Max`,
/// `7 → Exchange`, `8 → CompareExchange`.
///
/// # Failure mode
///
/// Returns `Err("Fix: unknown atomic op tag {tag}; use a compatible IR serializer.")`
/// for any unrecognized tag so callers reject the blob with an actionable
/// diagnostic.
#[inline]
pub(crate) fn atomic_op_from_tag(tag: u8) -> Result<AtomicOp, String> {
    decode_tag(tag, ATOMIC_OP_TAGS, "atomic")
}
