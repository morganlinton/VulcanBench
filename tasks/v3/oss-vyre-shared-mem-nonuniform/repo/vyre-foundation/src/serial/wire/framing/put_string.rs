//! UTF-8 string encoder for the IR wire format.

use super::put_len_u32;
use crate::serial::wire::encode::WireEncodeErr;
use crate::serial::wire::MAX_STRING_LEN;

/// Append a bounded length-prefixed UTF-8 string.
///
/// # Preconditions
///
/// `out` is a valid `Vec<u8>`. `value` must be valid UTF-8 (guaranteed
/// by the Rust `str` type).
///
/// # Return semantics
///
/// On success, the byte length of `value` is encoded as a little-endian
/// `u32` followed by the raw UTF-8 bytes, and `Ok(())` is returned.
///
/// # Errors
///
/// Returns an actionable error when the string exceeds the wire-format
/// maximum length or the length cannot fit in the encoded field.
#[inline]
#[must_use]
pub fn put_string(out: &mut Vec<u8>, value: &str) -> Result<(), WireEncodeErr> {
    if value.len() > MAX_STRING_LEN {
        return Err(WireEncodeErr::fmt_usize2(
            "Fix: string length ",
            value.len(),
            " exceeds IR wire-format limit ",
            MAX_STRING_LEN,
            "; shorten names/op ids before serialization.",
        ));
    }
    put_len_u32(out, value.len(), "string length")?;
    out.extend_from_slice(value.as_bytes());
    Ok(())
}
