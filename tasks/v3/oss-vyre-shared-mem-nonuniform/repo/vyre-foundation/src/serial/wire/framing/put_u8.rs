//! Single-byte framing helper.

/// Append one raw byte to the output buffer.
///
/// # Preconditions
///
/// `out` is a valid `Vec<u8>` that will receive one byte.
///
/// # Return semantics
///
/// This function has no return value; it mutates `out` in place.
///
/// # Invariants
///
/// After the call, `out.len()` has increased by exactly one and the
/// last byte equals `value`. Used for enum discriminants and boolean
/// flags where a multi-byte encoding would waste space.
#[inline]
pub fn put_u8(out: &mut Vec<u8>, value: u8) {
    out.push(value);
}
