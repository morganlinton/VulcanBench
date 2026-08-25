use crate::ir::BufferAccess;

/// Encode a [`BufferAccess`] into its stable VIR0 wire-format tag byte.
///
/// # Preconditions
///
/// `value` must be a variant known to the VIR0 encoder. Because
/// `BufferAccess` is `#[non_exhaustive]`, spec additions must be
/// matched here before they can round-trip through the wire format.
///
/// # Returns
///
/// `Ok(u8)` containing the tag value (`0..=4`).
///
/// # Failure mode
///
/// Returns `Err("unknown BufferAccess variant")` when the variant has no
/// registered tag. This prevents silent data loss: an unmapped variant
/// fails serialization loudly rather than producing an invalid blob.
#[inline]
pub(crate) fn access_tag(value: &BufferAccess) -> Result<u8, String> {
    match *value {
        BufferAccess::ReadOnly => Ok(0),
        BufferAccess::ReadWrite => Ok(1),
        BufferAccess::Uniform => Ok(2),
        BufferAccess::Workgroup => Ok(3),
        BufferAccess::WriteOnly => Ok(4),
        _ => Err("unknown BufferAccess variant".to_string()),
    }
}
