//! Little-endian `u32` framing helper.

/// Append one little-endian `u32` to the output buffer.
///
/// # Preconditions
///
/// `out` is a valid `Vec<u8>` that will receive four bytes.
///
/// # Return semantics
///
/// This function has no return value; it mutates `out` in place.
///
/// # Invariants
///
/// After the call, `out.len()` has increased by exactly four and the
/// last four bytes are `value` encoded in little-endian order.
/// All `u32` wire fields use this helper so the format is consistent
/// across big-endian and little-endian hosts.
#[inline]
pub fn put_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}
