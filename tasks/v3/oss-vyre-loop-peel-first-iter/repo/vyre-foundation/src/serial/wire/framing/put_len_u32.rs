//! Length-field encoder for wire-format sequences.

use super::put_u32;
use crate::serial::wire::encode::WireEncodeErr;

/// Append a little-endian `u32` length converted from `usize`.
///
/// # Preconditions
///
/// `out` is a valid `Vec<u8>`. `value` is the true element or byte
/// count of the payload that will follow this length field.
///
/// # Return semantics
///
/// On success, four little-endian bytes representing `value` as `u32`
/// are appended to `out` and `Ok(())` is returned.
///
/// # Errors
///
/// Returns an actionable error when `value` cannot fit in the wire-format
/// length field. This prevents platform-dependent `usize` widths from
/// leaking into the portable VIR0 blob.
#[inline]
#[must_use]
pub fn put_len_u32(out: &mut Vec<u8>, value: usize, label: &str) -> Result<(), WireEncodeErr> {
    let encoded = u32::try_from(value).map_err(|_| {
        WireEncodeErr::fmt_usize(
            label,
            value,
            " exceeds u32::MAX. Fix: split the Program before IR wire-format serialization.",
        )
    })?;
    put_u32(out, encoded);
    Ok(())
}
